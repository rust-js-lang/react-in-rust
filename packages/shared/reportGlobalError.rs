// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{Kind, Unknown};
use webapi::{ErrorEventInit, Window, console, window};

thread_local! {
    pub static REPORT_GLOBAL_ERROR: fn(Option<&'static Unknown>) = if js::is_function(webapi::global::report_error::<Option<&Unknown>>) {
        // In modern browsers, reportError will dispatch an error event,
        // emulating an uncaught JavaScript error.
        webapi::global::report_error
    } else {
        |error| {
            if js::is_object(window) && js::is_function(Window::new_error_event) {
                // Browser Polyfill
                let message = match js::classify(error.and_then(|error| js::get(error, "message"))) {
                    Kind::String(message) => message.to_string(),
                    _ => js::string(error),
                };
                let event = window.new_error_event(
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
