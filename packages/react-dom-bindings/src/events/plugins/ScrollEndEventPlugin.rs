// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;
use std::rc::Rc;

use js::{Kind, function};
use react_reconciler::react_internal_types::Fiber;
use shared::execution_environment::CAN_USE_DOM;
use shared::react_feature_flags::ENABLE_SCROLL_END_POLYFILL;
use webapi::EventTarget;

use crate::dom_plugin_event_system::{
    DispatchEntry, DispatchQueue, accumulate_single_phase_listeners,
    accumulate_two_phase_listeners, process_dispatch_queue,
};
use crate::event_registry::register_two_phase_event;
use crate::event_system_flags::{EventSystemFlags, IS_CAPTURE_PHASE};
use crate::is_event_supported::is_event_supported;
use crate::plugin_module_type::AnyNativeEvent;
use crate::react_dom_component_tree::{
    clear_scroll_end_timer, get_scroll_end_timer, set_scroll_end_timer,
};
use crate::react_dom_update_batching::batched_updates;
use crate::react_synthetic_event_type::ReactSyntheticEvent;
use crate::synthetic_event::SYNTHETIC_UI_EVENT;

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "isScrollEndEventSupported")]
    static IS_SCROLL_END_EVENT_SUPPORTED: bool =
        ENABLE_SCROLL_END_POLYFILL.with(|enable_scroll_end_polyfill| *enable_scroll_end_polyfill)
            && CAN_USE_DOM.with(|can_use_dom| *can_use_dom)
            && is_event_supported("scrollend");

    static IS_TOUCH_STARTED: Cell<bool> = const { Cell::new(false) };
    static IS_MOUSE_DOWN: Cell<bool> = const { Cell::new(false) };
}

pub fn register_events() {
    register_two_phase_event(
        "onScrollEnd",
        Rc::new(vec![
            "scroll".to_owned(),
            "scrollend".to_owned(),
            "touchstart".to_owned(),
            "touchcancel".to_owned(),
            "touchend".to_owned(),
            "mousedown".to_owned(),
            "mouseup".to_owned(),
        ]),
    );
}

fn manual_dispatch_scroll_end_event(
    inst: &Rc<Fiber>,
    native_event: &'static AnyNativeEvent,
    target: &'static EventTarget,
) {
    let mut dispatch_queue: DispatchQueue = vec![];
    let listeners = accumulate_two_phase_listeners(Some(Rc::clone(inst)), "onScrollEnd");
    if !listeners.is_empty() {
        let event: &'static ReactSyntheticEvent = js::new(
            SYNTHETIC_UI_EVENT.with(|ctor| *ctor),
            (
                Some("onScrollEnd".to_owned()),
                "scrollend".to_owned(),
                js::null(),
                native_event, // This will be the "scroll" event.
                Some(target),
            ),
        );
        dispatch_queue.push(DispatchEntry { event, listeners });
    }
    batched_updates(run_event_in_batch, dispatch_queue, None);
}

fn run_event_in_batch(dispatch_queue: DispatchQueue, _: Option<()>) {
    process_dispatch_queue(&dispatch_queue, 0);
}

fn fire_scroll_end(
    target_inst: &Rc<Fiber>,
    native_event: &'static AnyNativeEvent,
    native_event_target: &'static EventTarget,
) {
    clear_scroll_end_timer(native_event_target);
    if IS_MOUSE_DOWN.get() || IS_TOUCH_STARTED.get() {
        // If mouse or touch is down, try again later in case this is due to having an
        // active scroll but it's not currently moving.
        debounce_scroll_end(
            Some(Rc::clone(target_inst)),
            native_event,
            native_event_target,
        );
        return;
    }
    manual_dispatch_scroll_end_event(target_inst, native_event, native_event_target);
}

// When scrolling slows down the frequency of new scroll events can be quite low.
// This timeout seems high enough to cover those cases but short enough to not
// fire the event way too late.
const DEBOUNCE_TIMEOUT: f64 = 200.0;

fn debounce_scroll_end(
    target_inst: Option<Rc<Fiber>>,
    native_event: &'static AnyNativeEvent,
    native_event_target: &'static EventTarget,
) {
    let existing_timer = get_scroll_end_timer(native_event_target);
    if existing_timer.is_some() {
        js::clear_timeout(existing_timer);
    }
    if let Some(target_inst) = target_inst {
        let new_timer = js::set_timeout(
            function::bind(
                fire_scroll_end,
                Some(&Kind::Null),
                (target_inst, native_event, native_event_target),
            ),
            Some(DEBOUNCE_TIMEOUT),
        );
        set_scroll_end_timer(native_event_target, new_timer);
    }
}

/// This plugin creates an `onScrollEnd` event polyfill when the native one
/// is not available.
#[cfg_attr(
    rust_js,
    rust_js::nullable(target_inst, native_event_target, _target_container)
)]
pub fn extract_events(
    dispatch_queue: &mut DispatchQueue,
    dom_event_name: &str,
    target_inst: Option<Rc<Fiber>>,
    native_event: &'static AnyNativeEvent,
    native_event_target: Option<&'static EventTarget>,
    event_system_flags: EventSystemFlags,
    _target_container: Option<&'static EventTarget>,
) {
    if !ENABLE_SCROLL_END_POLYFILL.with(|enable_scroll_end_polyfill| *enable_scroll_end_polyfill) {
        return;
    }

    let in_capture_phase = (event_system_flags & IS_CAPTURE_PHASE) != 0;

    if dom_event_name != "scrollend" {
        if !IS_SCROLL_END_EVENT_SUPPORTED
            .with(|is_scroll_end_event_supported| *is_scroll_end_event_supported)
            && in_capture_phase
        {
            match dom_event_name {
                "scroll" => {
                    if let Some(native_event_target) = native_event_target {
                        debounce_scroll_end(target_inst, native_event, native_event_target);
                    }
                }
                "touchstart" => {
                    IS_TOUCH_STARTED.set(true);
                }
                "touchcancel" | "touchend" => {
                    // Note we cannot use pointer events for this because they get
                    // cancelled when native scrolling takes control.
                    IS_TOUCH_STARTED.set(false);
                }
                "mousedown" => {
                    IS_MOUSE_DOWN.set(true);
                }
                "mouseup" => {
                    IS_MOUSE_DOWN.set(false);
                }
                _ => {}
            }
        }
        return;
    }

    if !IS_SCROLL_END_EVENT_SUPPORTED
        .with(|is_scroll_end_event_supported| *is_scroll_end_event_supported)
        && let Some(native_event_target) = native_event_target
    {
        let existing_timer = get_scroll_end_timer(native_event_target);
        if existing_timer.is_some() {
            // If we do get a native scrollend event fired, we cancel the polyfill.
            // This could happen if our feature detection is broken or if there's another
            // polyfill calling dispatchEvent to fire it before we fire ours.
            js::clear_timeout(existing_timer);
            clear_scroll_end_timer(native_event_target);
        } else {
            // If we didn't receive a 'scroll' event first, we ignore this event to avoid
            // double firing. Such as if we fired our onScrollEnd polyfill and then
            // we also observed a native one afterwards.
            return;
        }
    }

    // In React onScrollEnd doesn't bubble.
    let accumulate_target_only = !in_capture_phase;

    let listeners = accumulate_single_phase_listeners(
        target_inst,
        Some("onScrollEnd"),
        "scrollend",
        in_capture_phase,
        accumulate_target_only,
        native_event,
    );

    if !listeners.is_empty() {
        // Intentionally create event lazily.
        let event: &'static ReactSyntheticEvent = js::new(
            SYNTHETIC_UI_EVENT.with(|ctor| *ctor),
            (
                Some("onScrollEnd".to_owned()),
                "scrollend".to_owned(),
                js::null(),
                native_event,
                native_event_target,
            ),
        );
        dispatch_queue.push(DispatchEntry { event, listeners });
    }
}
