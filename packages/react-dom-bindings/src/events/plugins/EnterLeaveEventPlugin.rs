// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::rc::Rc;

use js::Unknown;
use react_reconciler::react_fiber_tree_reflection::get_nearest_mounted_fiber;
use react_reconciler::react_internal_types::Fiber;
use react_reconciler::react_work_tags::{HOST_COMPONENT, HOST_SINGLETON, HOST_TEXT};
use webapi::{EventTarget, mouse_event, window};

use crate::current_replaying_event::is_replaying_event;
use crate::dom_plugin_event_system::{DispatchQueue, accumulate_enter_leave_two_phase_listeners};
use crate::event_registry::register_direct_event;
use crate::event_system_flags::EventSystemFlags;
use crate::plugin_module_type::AnyNativeEvent;
use crate::react_dom_component_tree::{
    get_closest_instance_from_node, get_node_from_instance, is_container_marked_as_root,
};
use crate::react_synthetic_event_type::KnownReactSyntheticEvent;
use crate::synthetic_event::{SYNTHETIC_MOUSE_EVENT, SYNTHETIC_POINTER_EVENT};

pub fn register_events() {
    register_direct_event(
        "onMouseEnter",
        Rc::new(vec!["mouseout".to_owned(), "mouseover".to_owned()]),
    );
    register_direct_event(
        "onMouseLeave",
        Rc::new(vec!["mouseout".to_owned(), "mouseover".to_owned()]),
    );
    register_direct_event(
        "onPointerEnter",
        Rc::new(vec!["pointerout".to_owned(), "pointerover".to_owned()]),
    );
    register_direct_event(
        "onPointerLeave",
        Rc::new(vec!["pointerout".to_owned(), "pointerover".to_owned()]),
    );
}

/// For almost every interaction we care about, there will be both a top-level
/// `mouseover` and `mouseout` event that occurs. Only use `mouseout` so that
/// we do not extract duplicate events. However, moving the mouse into the
/// browser from outside will not fire a `mouseout` event. In this case, we use
/// the `mouseover` top-level event.
#[cfg_attr(rust_js, rust_js::nullable(target_inst, native_event_target))]
pub fn extract_events(
    dispatch_queue: &mut DispatchQueue,
    dom_event_name: &str,
    target_inst: Option<Rc<Fiber>>,
    native_event: &'static AnyNativeEvent,
    native_event_target: Option<&'static EventTarget>,
    event_system_flags: EventSystemFlags,
    target_container: &'static EventTarget,
) {
    let _ = (event_system_flags, target_container);
    let is_over_event = dom_event_name == "mouseover" || dom_event_name == "pointerover";
    let is_out_event = dom_event_name == "mouseout" || dom_event_name == "pointerout";

    if is_over_event && !is_replaying_event(native_event) {
        // If this is an over event with a target, we might have already dispatched
        // the event in the out event of the other target. If this is replayed,
        // then it's because we couldn't dispatch against this target previously
        // so we have to do it now instead.
        let related_target = js::get(native_event, "relatedTarget");
        let related = if js::truthy(related_target) {
            related_target
        } else {
            js::get(native_event, "fromElement")
        };
        if js::truthy(related) {
            let related: &'static Unknown = unsafe { js::cast(related) };
            // If the related node is managed by React, we can assume that we have
            // already dispatched the corresponding events during its mouseout.
            if get_closest_instance_from_node(unsafe { js::cast(Some(related)) }).is_some()
                || is_container_marked_as_root(related)
            {
                return;
            }
        }
    }

    if !is_out_event && !is_over_event {
        // Must not be a mouse or pointer in or out - ignoring.
        return;
    }

    let win: Option<&'static Unknown>;
    // TODO: why is this nullable in the types but we read from it?
    if js::strict_equals(js::get(native_event_target, "window"), native_event_target) {
        // `nativeEventTarget` is probably a window object.
        win = native_event_target.map(js::unknown);
    } else {
        // TODO: Figure out why `ownerDocument` is sometimes undefined in IE8.
        let doc = js::get(native_event_target, "ownerDocument");
        if js::truthy(doc) {
            win = if js::truthy(js::get(doc, "defaultView")) {
                js::get(doc, "defaultView")
            } else {
                js::get(doc, "parentWindow")
            };
        } else {
            win = Some(js::unknown(window));
        }
    }

    let from: Option<Rc<Fiber>>;
    let mut to: Option<Rc<Fiber>>;
    if is_out_event {
        let related_target = mouse_event::unchecked_from(native_event)
            .related_target()
            .map(js::unknown);
        let related = if js::truthy(related_target) {
            related_target
        } else {
            js::get(native_event, "toElement")
        };
        from = target_inst.clone();
        to = if js::truthy(related) {
            get_closest_instance_from_node(unsafe { js::cast(related) })
        } else {
            js::null()
        };
        if let Some(to_fiber) = &to {
            let nearest_mounted = get_nearest_mounted_fiber(to_fiber);
            let tag = to_fiber.tag.get();
            if nearest_mounted.as_ref() != Some(to_fiber)
                || (tag != HOST_COMPONENT && tag != HOST_SINGLETON && tag != HOST_TEXT)
            {
                to = js::null();
            }
        }
    } else {
        // Moving to a node from outside the window.
        from = js::null();
        to = target_inst.clone();
    }

    if from == to {
        // Nothing pertains to our managed components.
        return;
    }

    #[cfg_attr(rust_js, rust_js::name = "SyntheticEventCtor")]
    let mut synthetic_event_ctor = SYNTHETIC_MOUSE_EVENT.with(|ctor| *ctor);
    let mut leave_event_type = "onMouseLeave";
    let mut enter_event_type = "onMouseEnter";
    let mut event_type_prefix = "mouse";
    if dom_event_name == "pointerout" || dom_event_name == "pointerover" {
        synthetic_event_ctor = SYNTHETIC_POINTER_EVENT.with(|ctor| *ctor);
        leave_event_type = "onPointerLeave";
        enter_event_type = "onPointerEnter";
        event_type_prefix = "pointer";
    }

    let from_node = match &from {
        None => win,
        Some(from) => get_node_from_instance(from),
    };
    let to_node = match &to {
        None => win,
        Some(to) => get_node_from_instance(to),
    };

    let leave: &'static KnownReactSyntheticEvent = js::new(
        synthetic_event_ctor,
        (
            Some(leave_event_type.to_owned()),
            event_type_prefix.to_owned() + "leave",
            from.clone(),
            native_event,
            native_event_target,
        ),
    );
    leave.target.set(from_node);
    leave.related_target.set(to_node);

    let mut enter: Option<&'static KnownReactSyntheticEvent> = js::null();

    // We should only process this nativeEvent if we are processing
    // the first ancestor. Next time, we will ignore the event.
    let native_target_inst =
        get_closest_instance_from_node(unsafe { js::cast(native_event_target.map(js::unknown)) });
    if native_target_inst == target_inst {
        let enter_event: &'static KnownReactSyntheticEvent = js::new(
            synthetic_event_ctor,
            (
                Some(enter_event_type.to_owned()),
                event_type_prefix.to_owned() + "enter",
                to.clone(),
                native_event,
                native_event_target,
            ),
        );
        enter_event.target.set(to_node);
        enter_event.related_target.set(from_node);
        enter = Some(enter_event);
    }

    accumulate_enter_leave_two_phase_listeners(dispatch_queue, leave, enter, from, to);
}
