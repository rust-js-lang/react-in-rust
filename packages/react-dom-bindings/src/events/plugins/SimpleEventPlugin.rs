// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::rc::Rc;

use react_reconciler::react_internal_types::Fiber;
use shared::react_feature_flags::ENABLE_CREATE_EVENT_HANDLE_API;
use webapi::{EventTarget, keyboard_event, mouse_event};

use crate::dom_event_names::{
    ANIMATION_CANCEL, ANIMATION_END, ANIMATION_ITERATION, ANIMATION_START, TRANSITION_END,
};
use crate::dom_event_properties::TOP_LEVEL_EVENTS_TO_REACT_NAMES;
pub use crate::dom_event_properties::register_simple_events as register_events;
use crate::dom_plugin_event_system::{
    DispatchEntry, DispatchQueue, accumulate_event_handle_non_managed_node_listeners,
    accumulate_single_phase_listeners,
};
use crate::event_system_flags::{
    EventSystemFlags, IS_CAPTURE_PHASE, IS_EVENT_HANDLE_NON_MANAGED_NODE,
};
use crate::get_event_char_code::get_event_char_code;
use crate::plugin_module_type::AnyNativeEvent;
use crate::react_synthetic_event_type::ReactSyntheticEvent;
use crate::synthetic_event::{
    SYNTHETIC_ANIMATION_EVENT, SYNTHETIC_CLIPBOARD_EVENT, SYNTHETIC_DRAG_EVENT, SYNTHETIC_EVENT,
    SYNTHETIC_FOCUS_EVENT, SYNTHETIC_KEYBOARD_EVENT, SYNTHETIC_MOUSE_EVENT,
    SYNTHETIC_POINTER_EVENT, SYNTHETIC_SUBMIT_EVENT, SYNTHETIC_TOGGLE_EVENT, SYNTHETIC_TOUCH_EVENT,
    SYNTHETIC_TRANSITION_EVENT, SYNTHETIC_UI_EVENT, SYNTHETIC_WHEEL_EVENT, SyntheticEventClass,
};

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
    let Some(react_name) =
        TOP_LEVEL_EVENTS_TO_REACT_NAMES.with_borrow(|top_level_events_to_react_names| {
            top_level_events_to_react_names.get(dom_event_name).cloned()
        })
    else {
        return;
    };
    #[cfg_attr(rust_js, rust_js::name = "SyntheticEventCtor")]
    let mut synthetic_event_ctor: &SyntheticEventClass = SYNTHETIC_EVENT.with(|ctor| *ctor);
    let mut react_event_type: String = dom_event_name.to_owned();
    match dom_event_name {
        // Firefox creates a keypress event for function keys too. This removes
        // the unwanted keypress events. Enter is however both printable and
        // non-printable. One would expect Tab to be as well (but it isn't).
        // TODO: Fixed in https://bugzilla.mozilla.org/show_bug.cgi?id=968056. Can
        // probably remove.
        "keypress" if get_event_char_code(keyboard_event::unchecked_from(native_event)) == 0.0 => {
            return;
        }
        "keypress" | "keydown" | "keyup" => {
            synthetic_event_ctor = SYNTHETIC_KEYBOARD_EVENT.with(|ctor| *ctor);
        }
        "focusin" => {
            react_event_type = "focus".to_owned();
            synthetic_event_ctor = SYNTHETIC_FOCUS_EVENT.with(|ctor| *ctor);
        }
        "focusout" => {
            react_event_type = "blur".to_owned();
            synthetic_event_ctor = SYNTHETIC_FOCUS_EVENT.with(|ctor| *ctor);
        }
        "beforeblur" | "afterblur" => {
            synthetic_event_ctor = SYNTHETIC_FOCUS_EVENT.with(|ctor| *ctor);
        }
        // Firefox creates a click event on right mouse clicks. This removes the
        // unwanted click events.
        // TODO: Fixed in https://phabricator.services.mozilla.com/D26793. Can
        // probably remove.
        "click" if mouse_event::unchecked_from(native_event).button() == 2.0 => {
            return;
        }
        "click" | "auxclick" | "dblclick" | "mousedown" | "mousemove" | "mouseup"
        // TODO: Disabled elements should not respond to mouse events
        | "mouseout" | "mouseover" | "contextmenu" => {
            synthetic_event_ctor = SYNTHETIC_MOUSE_EVENT.with(|ctor| *ctor);
        }
        "drag" | "dragend" | "dragenter" | "dragexit" | "dragleave" | "dragover" | "dragstart"
        | "drop" => {
            synthetic_event_ctor = SYNTHETIC_DRAG_EVENT.with(|ctor| *ctor);
        }
        "touchcancel" | "touchend" | "touchmove" | "touchstart" => {
            synthetic_event_ctor = SYNTHETIC_TOUCH_EVENT.with(|ctor| *ctor);
        }
        name if ANIMATION_CANCEL.with(|event_name| event_name == name)
            || ANIMATION_END.with(|event_name| event_name == name)
            || ANIMATION_ITERATION.with(|event_name| event_name == name)
            || ANIMATION_START.with(|event_name| event_name == name) =>
        {
            synthetic_event_ctor = SYNTHETIC_ANIMATION_EVENT.with(|ctor| *ctor);
        }
        name if TRANSITION_END.with(|event_name| event_name == name) => {
            synthetic_event_ctor = SYNTHETIC_TRANSITION_EVENT.with(|ctor| *ctor);
        }
        "scroll" | "scrollend" => {
            synthetic_event_ctor = SYNTHETIC_UI_EVENT.with(|ctor| *ctor);
        }
        "wheel" => {
            synthetic_event_ctor = SYNTHETIC_WHEEL_EVENT.with(|ctor| *ctor);
        }
        "copy" | "cut" | "paste" => {
            synthetic_event_ctor = SYNTHETIC_CLIPBOARD_EVENT.with(|ctor| *ctor);
        }
        "gotpointercapture" | "lostpointercapture" | "pointercancel" | "pointerdown"
        | "pointermove" | "pointerout" | "pointerover" | "pointerup" => {
            synthetic_event_ctor = SYNTHETIC_POINTER_EVENT.with(|ctor| *ctor);
        }
        "submit" => {
            synthetic_event_ctor = SYNTHETIC_SUBMIT_EVENT.with(|ctor| *ctor);
        }
        "toggle" | "beforetoggle" => {
            // MDN claims <details> should not receive ToggleEvent contradicting the spec: https://html.spec.whatwg.org/multipage/indices.html#event-toggle
            synthetic_event_ctor = SYNTHETIC_TOGGLE_EVENT.with(|ctor| *ctor);
        }
        _ => {
            // Unknown event. This is used by createEventHandle.
        }
    }

    let in_capture_phase = (event_system_flags & IS_CAPTURE_PHASE) != 0;
    if ENABLE_CREATE_EVENT_HANDLE_API && event_system_flags & IS_EVENT_HANDLE_NON_MANAGED_NODE != 0
    {
        let listeners = accumulate_event_handle_non_managed_node_listeners(
            // TODO: this cast may not make sense for events like
            // "focus" where React listens to e.g. "focusin".
            &react_event_type,
            target_container,
            in_capture_phase,
        );
        if !listeners.is_empty() {
            // Intentionally create event lazily.
            let event: &'static ReactSyntheticEvent = js::new(
                synthetic_event_ctor,
                (
                    react_name,
                    react_event_type,
                    js::null(),
                    native_event,
                    native_event_target,
                ),
            );
            dispatch_queue.push(DispatchEntry { event, listeners });
        }
    } else {
        // Some events don't bubble in the browser.
        // In the past, React has always bubbled them, but this can be surprising.
        // We're going to try aligning closer to the browser behavior by not bubbling
        // them in React either. We'll start by not bubbling onScroll, and then expand.
        let accumulate_target_only = !in_capture_phase
            // TODO: ideally, we'd eventually add all events from
            // nonDelegatedEvents list in DOMPluginEventSystem.
            // Then we can remove this special list.
            // This is a breaking change that can wait until React 18.
            && (dom_event_name == "scroll"
                || dom_event_name == "scrollend"
                || dom_event_name == "toggle"
                || dom_event_name == "beforetoggle");

        let listeners = accumulate_single_phase_listeners(
            target_inst,
            react_name.as_deref(),
            &native_event.r#type(),
            in_capture_phase,
            accumulate_target_only,
            native_event,
        );
        if !listeners.is_empty() {
            // Intentionally create event lazily.
            let event: &'static ReactSyntheticEvent = js::new(
                synthetic_event_ctor,
                (
                    react_name,
                    react_event_type,
                    js::null(),
                    native_event,
                    native_event_target,
                ),
            );
            dispatch_queue.push(DispatchEntry { event, listeners });
        }
    }
}
