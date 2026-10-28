// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use webapi::{AddEventListenerOptions, Event, EventTarget, EventTargetExt};

pub fn add_event_bubble_listener(
    target: &EventTarget,
    event_type: &str,
    listener: &'static dyn Fn(&Event),
) -> &'static dyn Fn(&Event) {
    target.add_event_listener_named(event_type, listener, Some(&false));
    listener
}

pub fn add_event_capture_listener(
    target: &EventTarget,
    event_type: &str,
    listener: &'static dyn Fn(&Event),
) -> &'static dyn Fn(&Event) {
    target.add_event_listener_named(event_type, listener, Some(&true));
    listener
}

pub fn add_event_capture_listener_with_passive_flag(
    target: &EventTarget,
    event_type: &str,
    listener: &'static dyn Fn(&Event),
    passive: bool,
) -> &'static dyn Fn(&Event) {
    target.add_event_listener_named(
        event_type,
        listener,
        Some(&AddEventListenerOptions {
            capture: Some(true),
            passive: Some(passive),
            once: None,
            signal: None,
        }),
    );
    listener
}

pub fn add_event_bubble_listener_with_passive_flag(
    target: &EventTarget,
    event_type: &str,
    listener: &'static dyn Fn(&Event),
    passive: bool,
) -> &'static dyn Fn(&Event) {
    target.add_event_listener_named(
        event_type,
        listener,
        Some(&AddEventListenerOptions {
            passive: Some(passive),
            capture: None,
            once: None,
            signal: None,
        }),
    );
    listener
}

pub fn remove_event_listener(
    target: &EventTarget,
    event_type: &str,
    listener: &'static dyn Fn(&Event),
    capture: bool,
) {
    target.remove_event_listener_named(event_type, listener, Some(&capture));
}
