// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

use js::{IntoUnknown, Kind, Unknown};
use shared::environment::__DEV__;
use shared::react_shared_internals::REACT_SHARED_INTERNALS;
use shared::report_global_error::REPORT_GLOBAL_ERROR;
use webapi::console;

use crate::get_component_name_from_fiber::get_component_name_from_fiber;
use crate::react_captured_value::CapturedValue;
use crate::react_internal_types::{CaughtErrorInfo, ErrorInfo, Fiber, FiberRoot};
use crate::react_work_tags::CLASS_COMPONENT;

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#bindToConsole"
)]
fn bind_to_console(
    method_name: &str,
    args: &[&dyn IntoUnknown],
    badge_name: &str,
) -> &'static dyn Fn() -> Option<&'static Unknown> {
    unreachable!()
}

thread_local! {
    // Side-channel since I'm not sure we want to make this part of the public API
    #[cfg_attr(rust_js, rust_js::nullable)]
    static COMPONENT_NAME: RefCell<Option<String>> = const { RefCell::new(None) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static ERROR_BOUNDARY_NAME: RefCell<Option<String>> = const { RefCell::new(None) };
}

pub fn default_on_uncaught_error(error: Option<&'static Unknown>, _error_info: &ErrorInfo) {
    // Overriding this can silence these warnings e.g. for tests.
    // See https://github.com/react/react/pull/13384

    // For uncaught root errors we report them as uncaught to the browser's
    // onerror callback. This won't have component stacks and the error addendum.
    // So we add those into a separate console.warn.
    REPORT_GLOBAL_ERROR.with(|report_global_error| report_global_error(error));
    if __DEV__ {
        let component_name_message = match COMPONENT_NAME
            .with_borrow(Clone::clone)
            .filter(|name| !name.is_empty())
        {
            Some(component_name) => {
                format!("An error occurred in the <{component_name}> component.")
            }
            None => "An error occurred in one of your React components.".to_string(),
        };

        let error_boundary_message = concat!(
            "Consider adding an error boundary to your tree to customize error handling behavior.\n",
            "Visit https://react.dev/link/error-boundaries to learn more about error boundaries."
        );

        console::warn((
            "%s\n\n%s\n",
            component_name_message,
            error_boundary_message,
            // We let our console.error wrapper add the component stack to the end.
        ));
    }
}

pub fn default_on_caught_error(error: Option<&'static Unknown>, _error_info: &CaughtErrorInfo) {
    // Overriding this can silence these warnings e.g. for tests.
    // See https://github.com/react/react/pull/13384

    // Caught by error boundary
    if __DEV__ {
        let component_name_message = match COMPONENT_NAME
            .with_borrow(Clone::clone)
            .filter(|name| !name.is_empty())
        {
            Some(component_name) => {
                format!("The above error occurred in the <{component_name}> component.")
            }
            None => "The above error occurred in one of your React components.".to_string(),
        };

        // In development, we provide our own message which includes the component stack
        // in addition to the error.
        let recreate_message = format!(
            concat!(
                "React will try to recreate this component tree from scratch ",
                "using the error boundary you provided, {}."
            ),
            ERROR_BOUNDARY_NAME
                .with_borrow(Clone::clone)
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| "Anonymous".to_string())
        );

        if let Kind::Object(_) | Kind::Array(_) = js::classify(error)
            && js::type_of(js::get(error, "environmentName")) == "string"
        {
            // This was a Server error. We print the environment name in a badge just like we do with
            // replays of console logs to indicate that the source of this throw as actually the Server.
            let environment_name: String = unsafe { js::cast(js::get(error, "environmentName")) };
            bind_to_console(
                "error",
                &[
                    &"%o\n\n%s\n\n%s\n",
                    &error,
                    &component_name_message,
                    &recreate_message,
                    // We let DevTools or console.createTask add the component stack to the end.
                ],
                &environment_name,
            )();
        } else {
            console::error((
                "%o\n\n%s\n\n%s\n",
                error,
                component_name_message,
                recreate_message,
                // We let our DevTools or console.createTask add the component stack to the end.
            ));
        }
    } else {
        // In production, we print the error directly.
        // This will include the message, the JS stack, and anything the browser wants to show.
        // We pass the error object instead of custom message so that the browser displays the error natively.
        console::error((error,)); // Don't transform to our wrapper, however, React DevTools can still add a stack.
    }
}

pub fn default_on_recoverable_error(error: Option<&'static Unknown>, _error_info: &ErrorInfo) {
    REPORT_GLOBAL_ERROR.with(|report_global_error| report_global_error(error));
}

pub fn log_uncaught_error(root: &FiberRoot, error_info: &CapturedValue<Option<&'static Unknown>>) {
    if let Err(e) = catch_unwind(AssertUnwindSafe(|| {
        if __DEV__ {
            COMPONENT_NAME.set(
                error_info
                    .source
                    .as_deref()
                    .and_then(get_component_name_from_fiber),
            );
            ERROR_BOUNDARY_NAME.set(None);
        }
        let error = error_info.value;
        if __DEV__ && REACT_SHARED_INTERNALS.act_queue().is_some() {
            // For uncaught errors inside act, we track them on the act and then
            // rethrow them into the test.
            REACT_SHARED_INTERNALS
                .thrown_errors()
                .borrow_mut()
                .push(error);
            return;
        }
        let on_uncaught_error = root.on_uncaught_error;
        on_uncaught_error(
            error,
            &ErrorInfo {
                component_stack: error_info.stack.clone(),
            },
        );
    })) {
        // This method must not throw, or React internal state will get messed up.
        // If console.error is overridden, or logCapturedError() shows a dialog that throws,
        // we want to report this error outside of the normal stack as a last resort.
        // https://github.com/react/react/issues/13188
        js::set_timeout(Box::new(move || resume_unwind(e)), 0.0);
    }
}

pub fn log_caught_error(
    root: &FiberRoot,
    boundary: &Fiber,
    error_info: &CapturedValue<Option<&'static Unknown>>,
) {
    if let Err(e) = catch_unwind(AssertUnwindSafe(|| {
        if __DEV__ {
            COMPONENT_NAME.set(
                error_info
                    .source
                    .as_deref()
                    .and_then(get_component_name_from_fiber),
            );
            ERROR_BOUNDARY_NAME.set(get_component_name_from_fiber(boundary));
        }
        let error = error_info.value;
        let on_caught_error = root.on_caught_error;
        on_caught_error(
            error,
            &CaughtErrorInfo {
                component_stack: error_info.stack.clone(),
                error_boundary: if boundary.tag.get() == CLASS_COMPONENT {
                    boundary.state_node.get() // This should always be the case as long as we only have class boundaries
                } else {
                    None
                },
            },
        );
    })) {
        // This method must not throw, or React internal state will get messed up.
        // If console.error is overridden, or logCapturedError() shows a dialog that throws,
        // we want to report this error outside of the normal stack as a last resort.
        // https://github.com/react/react/issues/13188
        js::set_timeout(Box::new(move || resume_unwind(e)), 0.0);
    }
}
