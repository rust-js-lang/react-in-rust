// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::marker::PhantomData;

use js::JsObject;
use webapi::{Event, EventTarget};

/// What www provides as `EventListener`.
struct EventListener(PhantomData<JsObject>);

/// What www's `EventListener` returns of a listener it adds, which removes it.
#[cfg_attr(rust_js, rust_js::types = "{ remove(): void }")]
pub struct RemovableListener(PhantomData<JsObject>);

impl EventListener {
    #[cfg_attr(rust_js, rust_js::link_name = "listen")]
    fn listen(
        &self,
        target: &EventTarget,
        event_type: &str,
        listener: &'static dyn Fn(&'static Event),
    ) -> &'static RemovableListener {
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "capture")]
    fn capture(
        &self,
        target: &EventTarget,
        event_type: &str,
        listener: &'static dyn Fn(&'static Event),
    ) -> &'static RemovableListener {
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "captureWithPassiveFlag")]
    fn capture_with_passive_flag(
        &self,
        target: &EventTarget,
        event_type: &str,
        listener: &'static dyn Fn(&'static Event),
        passive: bool,
    ) -> &'static RemovableListener {
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "bubbleWithPassiveFlag")]
    fn bubble_with_passive_flag(
        &self,
        target: &EventTarget,
        event_type: &str,
        listener: &'static dyn Fn(&'static Event),
        passive: bool,
    ) -> &'static RemovableListener {
        unreachable!()
    }
}

impl RemovableListener {
    #[cfg_attr(rust_js, rust_js::link_name = "remove")]
    fn remove(&self) {
        unreachable!()
    }
}

#[cfg_attr(rust_js, rust_js::link_name = "get EventListener#*")]
#[cfg_attr(rust_js, rust_js::require)]
fn event_listener() -> &'static EventListener {
    unreachable!()
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "EventListenerWWW")]
    static EVENT_LISTENER_WWW: &'static EventListener = event_listener();
}

pub fn add_event_bubble_listener(
    target: &EventTarget,
    event_type: &str,
    listener: &'static dyn Fn(&'static Event),
) -> &'static RemovableListener {
    EVENT_LISTENER_WWW
        .with(|event_listener_www| event_listener_www.listen(target, event_type, listener))
}

pub fn add_event_capture_listener(
    target: &EventTarget,
    event_type: &str,
    listener: &'static dyn Fn(&'static Event),
) -> &'static RemovableListener {
    EVENT_LISTENER_WWW
        .with(|event_listener_www| event_listener_www.capture(target, event_type, listener))
}

pub fn add_event_capture_listener_with_passive_flag(
    target: &EventTarget,
    event_type: &str,
    listener: &'static dyn Fn(&'static Event),
    passive: bool,
) -> &'static RemovableListener {
    EVENT_LISTENER_WWW.with(|event_listener_www| {
        event_listener_www.capture_with_passive_flag(target, event_type, listener, passive)
    })
}

pub fn add_event_bubble_listener_with_passive_flag(
    target: &EventTarget,
    event_type: &str,
    listener: &'static dyn Fn(&'static Event),
    passive: bool,
) -> &'static RemovableListener {
    EVENT_LISTENER_WWW.with(|event_listener_www| {
        event_listener_www.bubble_with_passive_flag(target, event_type, listener, passive)
    })
}

pub fn remove_event_listener(
    _target: &EventTarget,
    _event_type: &str,
    listener: &RemovableListener,
    _capture: bool,
) {
    listener.remove();
}
