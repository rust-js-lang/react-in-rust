// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;

use shared::environment::__DEV__;
use webapi::{Event, console};

// This exists to avoid circular dependency between ReactDOMEventReplaying
// and DOMPluginEventSystem.

thread_local! {
    #[cfg_attr(rust_js, rust_js::nullable)]
    static CURRENT_REPLAYING_EVENT: Cell<Option<&'static Event>> = const { Cell::new(None) };
}

pub fn set_replaying_event(event: &'static Event) {
    if __DEV__ && CURRENT_REPLAYING_EVENT.get().is_some() {
        console::error((concat!(
            "Expected currently replaying event to be null. This error ",
            "is likely caused by a bug in React. Please file an issue."
        ),));
    }
    CURRENT_REPLAYING_EVENT.set(Some(event));
}

pub fn reset_replaying_event() {
    if __DEV__ && CURRENT_REPLAYING_EVENT.get().is_none() {
        console::error((concat!(
            "Expected currently replaying event to not be null. This error ",
            "is likely caused by a bug in React. Please file an issue."
        ),));
    }
    CURRENT_REPLAYING_EVENT.set(None);
}

pub fn is_replaying_event(event: &Event) -> bool {
    Some(event) == CURRENT_REPLAYING_EVENT.get()
}
