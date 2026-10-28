// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use shared::react_feature_flags::{ENABLE_CREATE_EVENT_HANDLE_API, ENABLE_SCROLL_END_POLYFILL};

use crate::dom_event_names::{
    ANIMATION_CANCEL, ANIMATION_END, ANIMATION_ITERATION, ANIMATION_START, DOMEventName,
    TRANSITION_CANCEL, TRANSITION_END, TRANSITION_RUN, TRANSITION_START,
};
use crate::event_registry::register_two_phase_event;

thread_local! {
    pub static TOP_LEVEL_EVENTS_TO_REACT_NAMES: RefCell<HashMap<DOMEventName, Option<String>>> =
        RefCell::new(HashMap::new());

    // NOTE: Capitalization is important in this list!
    //
    // E.g. it needs "pointerDown", not "pointerdown".
    // This is because we derive both React name ("onPointerDown")
    // and DOM name ("pointerdown") from the same list.
    //
    // Exceptions that don't match this convention are listed separately.
    //
    // prettier-ignore
    static SIMPLE_EVENT_PLUGIN_EVENTS: RefCell<Vec<&'static str>> = RefCell::new(vec![
        "abort",
        "auxClick",
        "beforeToggle",
        "cancel",
        "canPlay",
        "canPlayThrough",
        "click",
        "close",
        "contextMenu",
        "copy",
        "cut",
        "drag",
        "dragEnd",
        "dragEnter",
        "dragExit",
        "dragLeave",
        "dragOver",
        "dragStart",
        "drop",
        "durationChange",
        "emptied",
        "encrypted",
        "ended",
        "error",
        "fullscreenChange",
        "fullscreenError",
        "gotPointerCapture",
        "input",
        "invalid",
        "keyDown",
        "keyPress",
        "keyUp",
        "load",
        "loadedData",
        "loadedMetadata",
        "loadStart",
        "lostPointerCapture",
        "mouseDown",
        "mouseMove",
        "mouseOut",
        "mouseOver",
        "mouseUp",
        "paste",
        "pause",
        "play",
        "playing",
        "pointerCancel",
        "pointerDown",
        "pointerMove",
        "pointerOut",
        "pointerOver",
        "pointerUp",
        "progress",
        "rateChange",
        "reset",
        "resize",
        "seeked",
        "seeking",
        "stalled",
        "submit",
        "suspend",
        "timeUpdate",
        "touchCancel",
        "touchEnd",
        "touchStart",
        "volumeChange",
        "scroll",
        "toggle",
        "touchMove",
        "waiting",
        "wheel",
    ]);
}

js::on_load! {
    if !ENABLE_SCROLL_END_POLYFILL.with(|enable_scroll_end_polyfill| *enable_scroll_end_polyfill) {
        SIMPLE_EVENT_PLUGIN_EVENTS.with_borrow_mut(|simple_event_plugin_events| {
            simple_event_plugin_events.push("scrollEnd");
        });
    }

    if ENABLE_CREATE_EVENT_HANDLE_API {
        // Special case: these two events don't have on* React handler
        // and are only accessible via the createEventHandle API.
        TOP_LEVEL_EVENTS_TO_REACT_NAMES.with_borrow_mut(|top_level_events_to_react_names| {
            top_level_events_to_react_names.insert("beforeblur".to_owned(), js::null());
            top_level_events_to_react_names.insert("afterblur".to_owned(), js::null());
        });
    }
}

fn register_simple_event(dom_event_name: DOMEventName, react_name: String) {
    TOP_LEVEL_EVENTS_TO_REACT_NAMES.with_borrow_mut(|top_level_events_to_react_names| {
        top_level_events_to_react_names.insert(dom_event_name.clone(), Some(react_name.clone()));
    });
    register_two_phase_event(&react_name, Rc::new(vec![dom_event_name]));
}

pub fn register_simple_events() {
    SIMPLE_EVENT_PLUGIN_EVENTS.with_borrow(|simple_event_plugin_events| {
        for event_name in simple_event_plugin_events {
            let dom_event_name = event_name.to_lowercase();
            let capitalized_event = js::string::get(event_name, 0.0).unwrap().to_uppercase()
                + &js::string::slice(event_name, 1.0, None);
            register_simple_event(dom_event_name, "on".to_owned() + &capitalized_event);
        }
    });
    // Special cases where event names don't match.
    register_simple_event(
        ANIMATION_CANCEL.with(Clone::clone),
        "onAnimationCancel".to_owned(),
    );
    register_simple_event(
        ANIMATION_END.with(Clone::clone),
        "onAnimationEnd".to_owned(),
    );
    register_simple_event(
        ANIMATION_ITERATION.with(Clone::clone),
        "onAnimationIteration".to_owned(),
    );
    register_simple_event(
        ANIMATION_START.with(Clone::clone),
        "onAnimationStart".to_owned(),
    );
    register_simple_event("dblclick".to_owned(), "onDoubleClick".to_owned());
    register_simple_event("focusin".to_owned(), "onFocus".to_owned());
    register_simple_event("focusout".to_owned(), "onBlur".to_owned());

    register_simple_event(
        TRANSITION_RUN.with(Clone::clone),
        "onTransitionRun".to_owned(),
    );
    register_simple_event(
        TRANSITION_START.with(Clone::clone),
        "onTransitionStart".to_owned(),
    );
    register_simple_event(
        TRANSITION_CANCEL.with(Clone::clone),
        "onTransitionCancel".to_owned(),
    );
    register_simple_event(
        TRANSITION_END.with(Clone::clone),
        "onTransitionEnd".to_owned(),
    );
}
