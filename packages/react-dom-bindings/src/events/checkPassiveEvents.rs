// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};

use js::{PropertyDescriptor, object};
use shared::execution_environment::CAN_USE_DOM;
use webapi::{AddEventListenerOptions, EventListenerObject, EventTargetExt, window};

thread_local! {
    pub static PASSIVE_BROWSER_EVENTS_SUPPORTED: Cell<bool> = const { Cell::new(false) };
}

// Check if browser support events with passive listeners
// https://developer.mozilla.org/en-US/docs/Web/API/EventTarget/addEventListener#Safely_detecting_option_support
js::on_load! {
    if CAN_USE_DOM.with(|can_use_dom| *can_use_dom)
        && catch_unwind(AssertUnwindSafe(|| {
            let options = AddEventListenerOptions::default();
            object::define_property(
                &options,
                "passive",
                &PropertyDescriptor {
                    get: Some(&|| {
                        PASSIVE_BROWSER_EVENTS_SUPPORTED.set(true);
                        None
                    }),
                    ..Default::default()
                },
            );
            let listener: &EventListenerObject = unsafe { js::cast(Some(js::unknown(&options))) };
            window.add_event_listener_named("test", listener, Some(&options));
            window.remove_event_listener_named("test", listener, Some(&options));
        }))
        .is_err()
    {
        PASSIVE_BROWSER_EVENTS_SUPPORTED.set(false);
    }
}
