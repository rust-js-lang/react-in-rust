// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{Kind, Unknown};
use webapi::{ErrorEventInit, console, error_event, window};

thread_local! {
    static REPORT_GLOBAL_ERROR: fn(Option<&'static Unknown>) = if js::is_defined(webapi::global::report_error::<Option<&Unknown>>) {
        // In modern browsers, reportError will dispatch an error event,
        // emulating an uncaught JavaScript error.
        webapi::global::report_error
    } else {
        |error| {
            if js::is_defined(window) && js::is_defined(error_event::new) {
                // Browser Polyfill
                let message = match error.and_then(|error| js::get(error, "message")).map(js::classify) {
                    Some(Kind::String(message)) => message.to_string(),
                    _ => js::string(error),
                };
                let event = error_event::new(
                    "error",
                    Some(ErrorEventInit {
                        bubbles: Some(true),
                        cancelable: Some(true),
                        message: Some(&message),
                        error: error.map(|error| error as &dyn js::IntoUnknown),
                        ..Default::default()
                    }),
                );
                let should_log = window.dispatch_event(event);
                if !should_log {
                    return;
                }
            } else if js::is_defined(node::process::emit::<node::event::UncaughtException>) {
                // Node Polyfill
                node::process::emit(node::event::UncaughtException, (unsafe { js::cast(error) },));
                return;
            }
            console::error((error,));
        }
    };
}

js::export_default!(REPORT_GLOBAL_ERROR);
