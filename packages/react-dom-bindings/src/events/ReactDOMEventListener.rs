// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js::{Kind, Unknown, function};
use react_isomorphic::react_start_transition::{set_t, t};
use react_reconciler::react_event_priorities::{
    CONTINUOUS_EVENT_PRIORITY, DEFAULT_EVENT_PRIORITY, DISCRETE_EVENT_PRIORITY, EventPriority,
    IDLE_EVENT_PRIORITY,
};
use react_reconciler::react_fiber_reconciler::attempt_synchronous_hydration;
use react_reconciler::react_fiber_shell_hydration::is_root_dehydrated;
use react_reconciler::react_fiber_tree_reflection::{
    get_activity_instance_from_fiber, get_container_from_fiber, get_nearest_mounted_fiber,
    get_suspense_instance_from_fiber,
};
use react_reconciler::react_internal_types::{Fiber, FiberRoot};
use react_reconciler::react_work_tags::{ACTIVITY_COMPONENT, HOST_ROOT, SUSPENSE_COMPONENT};
use react_reconciler::scheduler::{
    GET_CURRENT_PRIORITY_LEVEL as GET_CURRENT_SCHEDULER_PRIORITY_LEVEL,
    IDLE_PRIORITY as IDLE_SCHEDULER_PRIORITY, IMMEDIATE_PRIORITY as IMMEDIATE_SCHEDULER_PRIORITY,
    LOW_PRIORITY as LOW_SCHEDULER_PRIORITY, NORMAL_PRIORITY as NORMAL_SCHEDULER_PRIORITY,
    USER_BLOCKING_PRIORITY as USER_BLOCKING_SCHEDULER_PRIORITY,
};
use shared::react_shared_internals::REACT_SHARED_INTERNALS;
use webapi::{EventTarget, Node};

use crate::event_system_flags::{EventSystemFlags, IS_CAPTURE_PHASE};
use crate::get_event_target::get_event_target;
use crate::plugin_module_type::AnyNativeEvent;
use crate::react_dom_component_tree::{get_closest_instance_from_node, get_instance_from_node};
use crate::react_dom_update_priority::{get_current_update_priority, set_current_update_priority};

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/events/ReactDOMEventReplaying.js#isDiscreteEventThatRequiresHydration"
)]
fn is_discrete_event_that_requires_hydration(event_type: &str) -> bool {
    let _ = event_type;
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/events/ReactDOMEventReplaying.js#clearIfContinuousEvent"
)]
fn clear_if_continuous_event(dom_event_name: &str, native_event: &AnyNativeEvent) {
    let _ = (dom_event_name, native_event);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/events/ReactDOMEventReplaying.js#queueIfContinuousEvent"
)]
#[cfg_attr(rust_js, rust_js::nullable(blocked_on))]
fn queue_if_continuous_event(
    blocked_on: Option<&Unknown>,
    dom_event_name: &str,
    event_system_flags: EventSystemFlags,
    target_container: &EventTarget,
    native_event: &AnyNativeEvent,
) -> bool {
    let _ = (
        blocked_on,
        dom_event_name,
        event_system_flags,
        target_container,
        native_event,
    );
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/events/DOMPluginEventSystem.js#dispatchEventForPluginEventSystem"
)]
#[cfg_attr(rust_js, rust_js::nullable(target_inst))]
fn dispatch_event_for_plugin_event_system(
    dom_event_name: &str,
    event_system_flags: EventSystemFlags,
    native_event: &AnyNativeEvent,
    target_inst: Option<Rc<Fiber>>,
    target_container: &EventTarget,
) {
    let _ = (
        dom_event_name,
        event_system_flags,
        native_event,
        target_inst,
        target_container,
    );
    unreachable!()
}

thread_local! {
    // TODO: can we stop exporting these?
    #[cfg_attr(rust_js, rust_js::name = "_enabled")]
    static ENABLED: Cell<bool> = const { Cell::new(true) };
}

// This is exported in FB builds for use by legacy FB layer infra.
// We'd like to remove this but it's not clear if this is safe.
pub fn set_enabled(enabled: Option<bool>) {
    ENABLED.set(enabled == Some(true));
}

pub fn is_enabled() -> bool {
    ENABLED.get()
}

pub fn create_event_listener_wrapper(
    target_container: &'static EventTarget,
    dom_event_name: &'static str,
    event_system_flags: EventSystemFlags,
) -> Box<dyn Fn(&'static AnyNativeEvent)> {
    function::bind(
        dispatch_event,
        Some(&Kind::Null),
        (dom_event_name, event_system_flags, target_container),
    )
}

pub fn create_event_listener_wrapper_with_priority(
    target_container: &'static EventTarget,
    dom_event_name: &'static str,
    event_system_flags: EventSystemFlags,
) -> Box<dyn Fn(&'static AnyNativeEvent)> {
    let event_priority = get_event_priority(dom_event_name);
    let listener_wrapper: fn(&str, EventSystemFlags, &EventTarget, &AnyNativeEvent) =
        match event_priority {
            DISCRETE_EVENT_PRIORITY => dispatch_discrete_event,
            CONTINUOUS_EVENT_PRIORITY => dispatch_continuous_event,
            _ => dispatch_event,
        };
    function::bind(
        listener_wrapper,
        Some(&Kind::Null),
        (dom_event_name, event_system_flags, target_container),
    )
}

/// What a discrete or continuous dispatch restores when it's done, as
/// upstream's `finally` does.
struct RestorePriority {
    previous_priority: EventPriority,
    prev_transition: Option<&'static react_isomorphic::react_start_transition::Transition>,
}

impl Drop for RestorePriority {
    fn drop(&mut self) {
        set_current_update_priority(self.previous_priority, None);
        set_t(REACT_SHARED_INTERNALS, self.prev_transition);
    }
}

fn dispatch_discrete_event(
    dom_event_name: &str,
    event_system_flags: EventSystemFlags,
    container: &EventTarget,
    native_event: &AnyNativeEvent,
) {
    let prev_transition = t(REACT_SHARED_INTERNALS);
    set_t(REACT_SHARED_INTERNALS, None);
    let previous_priority = get_current_update_priority();
    let _restore = RestorePriority {
        previous_priority,
        prev_transition,
    };
    set_current_update_priority(DISCRETE_EVENT_PRIORITY, None);
    dispatch_event(dom_event_name, event_system_flags, container, native_event);
}

fn dispatch_continuous_event(
    dom_event_name: &str,
    event_system_flags: EventSystemFlags,
    container: &EventTarget,
    native_event: &AnyNativeEvent,
) {
    let prev_transition = t(REACT_SHARED_INTERNALS);
    set_t(REACT_SHARED_INTERNALS, None);
    let previous_priority = get_current_update_priority();
    let _restore = RestorePriority {
        previous_priority,
        prev_transition,
    };
    set_current_update_priority(CONTINUOUS_EVENT_PRIORITY, None);
    dispatch_event(dom_event_name, event_system_flags, container, native_event);
}

pub fn dispatch_event(
    dom_event_name: &str,
    event_system_flags: EventSystemFlags,
    target_container: &EventTarget,
    native_event: &AnyNativeEvent,
) {
    if !ENABLED.get() {
        return;
    }

    let mut blocked_on = find_instance_blocking_event(native_event);
    if blocked_on.is_none() {
        dispatch_event_for_plugin_event_system(
            dom_event_name,
            event_system_flags,
            native_event,
            RETURN_TARGET_INST.with_borrow(Clone::clone),
            target_container,
        );
        clear_if_continuous_event(dom_event_name, native_event);
        return;
    }

    if queue_if_continuous_event(
        blocked_on,
        dom_event_name,
        event_system_flags,
        target_container,
        native_event,
    ) {
        native_event.stop_propagation();
        return;
    }
    // We need to clear only if we didn't queue because
    // queueing is accumulative.
    clear_if_continuous_event(dom_event_name, native_event);

    if event_system_flags & IS_CAPTURE_PHASE != 0
        && is_discrete_event_that_requires_hydration(dom_event_name)
    {
        while blocked_on.is_some() {
            let fiber = get_instance_from_node(unsafe { js::cast(blocked_on) });
            if let Some(fiber) = &fiber {
                attempt_synchronous_hydration(fiber);
            }
            let next_blocked_on = find_instance_blocking_event(native_event);
            if next_blocked_on.is_none() {
                dispatch_event_for_plugin_event_system(
                    dom_event_name,
                    event_system_flags,
                    native_event,
                    RETURN_TARGET_INST.with_borrow(Clone::clone),
                    target_container,
                );
            }
            if js::strict_equals(next_blocked_on, blocked_on) {
                break;
            }
            blocked_on = next_blocked_on;
        }
        if blocked_on.is_some() {
            native_event.stop_propagation();
        }
        return;
    }

    // This is not replayable so we'll invoke it but without a target,
    // in case the event system needs to trace it.
    dispatch_event_for_plugin_event_system(
        dom_event_name,
        event_system_flags,
        native_event,
        js::null(),
        target_container,
    );
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn find_instance_blocking_event(native_event: &AnyNativeEvent) -> Option<&'static Unknown> {
    let native_event_target = get_event_target(native_event);
    find_instance_blocking_target(unsafe { js::cast(native_event_target.map(js::unknown)) })
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "return_targetInst")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static RETURN_TARGET_INST: RefCell<Option<Rc<Fiber>>> = const { RefCell::new(None) };
}

// Returns a SuspenseInstance, ActivityInstance or Container if it's blocked.
// The return_targetInst field above is conceptually part of the return value.
#[cfg_attr(rust_js, rust_js::nullable)]
pub fn find_instance_blocking_target(target_node: &'static Node) -> Option<&'static Unknown> {
    // TODO: Warn if _enabled is false.

    RETURN_TARGET_INST.set(None);

    let mut target_inst = get_closest_instance_from_node(target_node);

    if let Some(inst) = &target_inst {
        match get_nearest_mounted_fiber(inst) {
            None => {
                // This tree has been unmounted already. Dispatch without a target.
                target_inst = js::null();
            }
            Some(nearest_mounted) => {
                let tag = nearest_mounted.tag.get();
                if tag == SUSPENSE_COMPONENT {
                    let instance = get_suspense_instance_from_fiber(&nearest_mounted);
                    if instance.is_some() {
                        // Queue the event to be replayed later. Abort dispatching since we
                        // don't want this event dispatched twice through the event system.
                        // TODO: If this is the first discrete event in the queue. Schedule an increased
                        // priority for this boundary.
                        return instance;
                    }
                    // This shouldn't happen, something went wrong but to avoid blocking
                    // the whole system, dispatch the event without a target.
                    // TODO: Warn.
                    target_inst = js::null();
                } else if tag == ACTIVITY_COMPONENT {
                    let instance = get_activity_instance_from_fiber(&nearest_mounted);
                    if instance.is_some() {
                        // Queue the event to be replayed later. Abort dispatching since we
                        // don't want this event dispatched twice through the event system.
                        // TODO: If this is the first discrete event in the queue. Schedule an increased
                        // priority for this boundary.
                        return instance;
                    }
                    // This shouldn't happen, something went wrong but to avoid blocking
                    // the whole system, dispatch the event without a target.
                    // TODO: Warn.
                    target_inst = js::null();
                } else if tag == HOST_ROOT {
                    let root: &FiberRoot = unsafe { js::cast(nearest_mounted.state_node.get()) };
                    if is_root_dehydrated(root) {
                        // If this happens during a replay something went wrong and it might block
                        // the whole system.
                        return get_container_from_fiber(&nearest_mounted);
                    }
                    target_inst = js::null();
                } else if !Rc::ptr_eq(&nearest_mounted, inst) {
                    // If we get an event (ex: img onload) before committing that
                    // component's mount, ignore it for now (that is, treat it as if it was an
                    // event on a non-React tree). We might also consider queueing events and
                    // dispatching them after the mount.
                    target_inst = js::null();
                }
            }
        }
    }
    RETURN_TARGET_INST.set(target_inst);
    // We're not blocked on anything.
    None
}

pub fn get_event_priority(dom_event_name: &str) -> EventPriority {
    match dom_event_name {
        // Used by SimpleEventPlugin:
        "beforetoggle"
        | "cancel"
        | "click"
        | "close"
        | "contextmenu"
        | "copy"
        | "cut"
        | "auxclick"
        | "dblclick"
        | "dragend"
        | "dragstart"
        | "drop"
        | "focusin"
        | "focusout"
        | "input"
        | "invalid"
        | "keydown"
        | "keypress"
        | "keyup"
        | "mousedown"
        | "mouseup"
        | "paste"
        | "pause"
        | "play"
        | "pointercancel"
        | "pointerdown"
        | "pointerup"
        | "ratechange"
        | "reset"
        | "seeked"
        | "submit"
        | "toggle"
        | "touchcancel"
        | "touchend"
        | "touchstart"
        | "volumechange"
        // Used by polyfills: (fall through)
        | "change"
        | "selectionchange"
        | "textInput"
        | "compositionstart"
        | "compositionend"
        | "compositionupdate"
        // Only enableCreateEventHandleAPI: (fall through)
        | "beforeblur"
        | "afterblur"
        // Not used by React but could be by user code: (fall through)
        | "beforeinput"
        | "blur"
        | "fullscreenchange"
        | "fullscreenerror"
        | "focus"
        | "hashchange"
        | "popstate"
        | "select"
        | "selectstart" => DISCRETE_EVENT_PRIORITY,
        "drag"
        | "dragenter"
        | "dragexit"
        | "dragleave"
        | "dragover"
        | "mousemove"
        | "mouseout"
        | "mouseover"
        | "pointermove"
        | "pointerout"
        | "pointerover"
        | "resize"
        | "scroll"
        | "touchmove"
        | "wheel"
        // Not used by React but could be by user code: (fall through)
        | "mouseenter"
        | "mouseleave"
        | "pointerenter"
        | "pointerleave" => CONTINUOUS_EVENT_PRIORITY,
        "message" => {
            // We might be in the Scheduler callback.
            // Eventually this mechanism will be replaced by a check
            // of the current priority on the native scheduler.
            let scheduler_priority = GET_CURRENT_SCHEDULER_PRIORITY_LEVEL();
            if scheduler_priority == IMMEDIATE_SCHEDULER_PRIORITY.with(|priority| *priority) {
                DISCRETE_EVENT_PRIORITY
            } else if scheduler_priority == USER_BLOCKING_SCHEDULER_PRIORITY.with(|priority| *priority)
            {
                CONTINUOUS_EVENT_PRIORITY
            } else if scheduler_priority == NORMAL_SCHEDULER_PRIORITY.with(|priority| *priority)
                || scheduler_priority == LOW_SCHEDULER_PRIORITY.with(|priority| *priority)
            {
                // TODO: Handle LowSchedulerPriority, somehow. Maybe the same lane as hydration.
                DEFAULT_EVENT_PRIORITY
            } else if scheduler_priority == IDLE_SCHEDULER_PRIORITY.with(|priority| *priority) {
                IDLE_EVENT_PRIORITY
            } else {
                DEFAULT_EVENT_PRIORITY
            }
        }
        _ => DEFAULT_EVENT_PRIORITY,
    }
}
