// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// Corresponds to ReactFiberThenable and ReactFlightThenable modules. Generally,
// changes to one module should be reflected in the others.

use std::cell::{Cell, RefCell};
use std::ptr;
use std::rc::Rc;

use js::{Error, Kind, Unknown, function};
use shared::environment::__DEV__;
use shared::noop::noop;
use shared::react_types::Thenable;
use webapi::ConsoleTask;

use crate::react_fizz_component_stack::ComponentStackNode;
use crate::react_fizz_current_task::CURRENT_TASK_IN_DEV;

pub type ThenableState = RefCell<Vec<&'static Thenable>>;

thread_local! {
    // An error that is thrown (e.g. by `use`) to trigger Suspense. If we
    // detect this is caught by userspace, we'll log a warning in development.
    #[cfg_attr(rust_js, rust_js::name = "SuspenseException")]
    pub static SUSPENSE_EXCEPTION: &'static Error = js::error::new(concat!(
        "Suspense Exception: This is not a real error! It's an implementation ",
        "detail of `use` to interrupt the current render. You must either ",
        "rethrow it immediately, or move the `use` call outside of the ",
        "`try/catch` block. Capturing without rethrowing will lead to ",
        "unexpected behavior.\n\n",
        "To handle async errors, wrap your component in an error boundary, or ",
        "call the promise's `.catch` method and pass the result to `use`."
    ), None);
}

pub fn create_thenable_state() -> ThenableState {
    // The ThenableState is created the first time a component suspends. If it
    // suspends again, we'll reuse the same state.
    RefCell::new(Vec::new())
}

pub fn track_used_thenable(
    thenable_state: &ThenableState,
    thenable: &'static Thenable,
    index: usize,
) -> Option<&'static Unknown> {
    let mut thenable = thenable;
    let previous = thenable_state.borrow().get(index).copied();
    match previous {
        None => thenable_state.borrow_mut().push(thenable),
        Some(previous) => {
            if !ptr::eq(previous, thenable) {
                // Reuse the previous thenable, and drop the new one. We can assume
                // they represent the same value, because components are idempotent.

                // Avoid an unhandled rejection errors for the Promises that we'll
                // intentionally ignore.
                thenable.then(|_| noop(), |_| noop());
                thenable = previous;
            }
        }
    }

    // We use an expando to track the status and result of a thenable so that we
    // can synchronously unwrap the value. Think of this as an extension of the
    // Promise API, or a custom interface that is a superset of Thenable.
    //
    // If the thenable doesn't have a status, set it to "pending" and attach
    // a listener that will update its status and result when it resolves.
    match thenable.status().as_deref() {
        Some("fulfilled") => {
            // This could be a bad instrumentation that doesn't set .value.
            // We're not type-checking since this is a hot path where you can
            // track down easily when something becomes `undefined` unexpectedly.
            thenable.value()
        }
        Some("rejected") => {
            let rejected_error = thenable.reason();

            // Rejected Promises are rarer so we're doing an extra type-check in
            // case of a bad instrumentation that doesn't set .reason
            // If we end up throwing `undefined` it becomes hard to track down
            // where that throw originated because no callstack would exist.
            // React would still have a Component stack but that could only be used
            // as an approximation.
            if rejected_error.is_none() && !js::has(thenable, "reason") {
                panic!(concat!(
                    "A rejected Promise was passed to React without a `reason` property. ",
                    "React threw a generic error from where the Promise was used to assist in identifying the problematic Promise. ",
                    "Make sure that instrumented Promises correctly set the `reason` property when setting `status` to `'rejected'`."
                ));
            }

            js::throw(rejected_error)
        }
        _ => {
            if let Kind::String(_) = js::classify(js::get(thenable, "status")) {
                // Only instrument the thenable if the status if not defined. If
                // it's defined, but an unknown value, assume it's been instrumented by
                // some custom userspace implementation. We treat it as "pending".
                // Attach a dummy listener, to ensure that any lazy initialization can
                // happen. Flight lazily parses JSON when the value is actually awaited.
                thenable.then(|_| noop(), |_| noop());
            } else {
                thenable.set_status("pending");
                thenable.then(
                    move |fulfilled_value| {
                        if thenable.status().as_deref() == Some("pending") {
                            thenable.set_status("fulfilled");
                            thenable.set_value(fulfilled_value);
                        }
                    },
                    move |error| {
                        if thenable.status().as_deref() == Some("pending") {
                            thenable.set_status("rejected");
                            thenable.set_reason(error);
                        }
                    },
                );
            }

            // Check one more time in case the thenable resolved synchronously
            match thenable.status().as_deref() {
                Some("fulfilled") => return thenable.value(),
                Some("rejected") => js::throw(thenable.reason()),
                _ => {}
            }

            // Suspend.
            //
            // Throwing here is an implementation detail that allows us to unwind the
            // call stack. But we shouldn't allow it to leak into userspace. Throw an
            // opaque placeholder value instead of the actual thenable. If it doesn't
            // get captured by the work loop, log a warning, because that means
            // something in userspace must have caught it.
            SUSPENDED_THENABLE.set(Some(thenable));
            if __DEV__ && SHOULD_CAPTURE_SUSPENDED_CALL_SITE.get() {
                capture_suspended_call_site();
            }
            SUSPENSE_EXCEPTION.with(|exception| js::throw(exception.as_unknown()))
        }
    }
}

pub fn read_previous_thenable(
    thenable_state: &ThenableState,
    index: usize,
) -> Option<&'static Unknown> {
    let previous = thenable_state.borrow().get(index).copied();
    match previous {
        None => None,
        // We assume this has been resolved already.
        Some(previous) => previous.value(),
    }
}

thread_local! {
    // This is used to track the actual thenable that suspended so it can be
    // passed to the rest of the Suspense implementation — which, for historical
    // reasons, expects to receive a thenable.
    #[cfg_attr(rust_js, rust_js::nullable)]
    static SUSPENDED_THENABLE: Cell<Option<&'static Thenable>> = const { Cell::new(None) };
}

pub fn get_suspended_thenable() -> &'static Thenable {
    // This is called right after `use` suspends by throwing an exception. `use`
    // throws an opaque value instead of the thenable itself so that it can't be
    // caught in userspace. Then the work loop accesses the actual thenable using
    // this function.
    let Some(thenable) = SUSPENDED_THENABLE.take() else {
        panic!(concat!(
            "Expected a suspended thenable. This is a bug in React. Please file ",
            "an issue."
        ));
    };
    thenable
}

thread_local! {
    static SHOULD_CAPTURE_SUSPENDED_CALL_SITE: Cell<bool> = const { Cell::new(false) };
}

#[cfg_attr(rust_js, rust_js::name = "setCaptureSuspendedCallSiteDEV")]
pub fn set_capture_suspended_call_site_dev(capture: bool) {
    if !__DEV__ {
        // eslint-disable-next-line react-internal/prod-error-codes
        panic!(concat!(
            "setCaptureSuspendedCallSiteDEV was called in a production environment. ",
            "This is a bug in React."
        ));
    }
    SHOULD_CAPTURE_SUSPENDED_CALL_SITE.set(capture);
}

thread_local! {
    // DEV-only
    #[cfg_attr(rust_js, rust_js::nullable)]
    static SUSPENDED_CALL_SITE_STACK: RefCell<Option<Rc<ComponentStackNode>>> = const { RefCell::new(None) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static SUSPENDED_CALL_SITE_DEBUG_TASK: Cell<Option<&'static ConsoleTask>> = const { Cell::new(None) };
}

fn capture_suspended_call_site() {
    // This is currently only used when aborting in Fizz.
    // You can only abort the render in Fizz and Flight.
    // In Fiber we only track suspended use via DevTools.
    // In Flight, we track suspended use via async debug info.
    let Some(current_task) = CURRENT_TASK_IN_DEV.get() else {
        // eslint-disable-next-line react-internal/prod-error-codes -- not a prod error
        panic!(concat!(
            "Expected to have a current task when tracking a suspend call site. ",
            "This is a bug in React."
        ));
    };
    let Some(current_component_stack) = current_task.component_stack() else {
        // eslint-disable-next-line react-internal/prod-error-codes -- not a prod error
        panic!(concat!(
            "Expected to have a component stack on the current task when ",
            "tracking a suspended call site. This is a bug in React."
        ));
    };
    SUSPENDED_CALL_SITE_STACK.set(Some(Rc::new(ComponentStackNode {
        parent: current_component_stack.parent.clone(),
        r#type: current_component_stack.r#type,
        owner: current_component_stack.owner,
        stack: Cell::new(Some(js::unknown(js::error::new(
            "react-stack-top-frame",
            None,
        )))),
    })));
    // TODO: If this is used in error handlers, the ConsoleTask stack
    // will just be this debugTask + the stack of the abort() call which usually means
    // it's just this debugTask.
    // Ideally we'd be able to reconstruct the owner ConsoleTask as well.
    // The stack of the debugTask would not point to the suspend location anyway.
    // The focus is really on callsite which should be used in captureOwnerStack().
    SUSPENDED_CALL_SITE_DEBUG_TASK.set(current_task.debug_task());
}

#[cfg_attr(
    rust_js,
    rust_js::name = "getSuspendedCallSiteStackDEV",
    rust_js::nullable
)]
pub fn get_suspended_call_site_stack_dev() -> Option<Rc<ComponentStackNode>> {
    if __DEV__ {
        SUSPENDED_CALL_SITE_STACK.take()
    } else {
        // eslint-disable-next-line react-internal/prod-error-codes
        panic!(concat!(
            "getSuspendedCallSiteDEV was called in a production environment. ",
            "This is a bug in React."
        ));
    }
}

#[cfg_attr(
    rust_js,
    rust_js::name = "getSuspendedCallSiteDebugTaskDEV",
    rust_js::nullable
)]
pub fn get_suspended_call_site_debug_task_dev() -> Option<&'static ConsoleTask> {
    if __DEV__ {
        SUSPENDED_CALL_SITE_DEBUG_TASK.take()
    } else {
        // eslint-disable-next-line react-internal/prod-error-codes
        panic!(concat!(
            "getSuspendedCallSiteDebugTaskDEV was called in a production environment. ",
            "This is a bug in React."
        ));
    }
}

#[cfg_attr(rust_js, rust_js::name = "ensureSuspendableThenableStateDEV")]
pub fn ensure_suspendable_thenable_state_dev(thenable_state: &ThenableState) -> Box<dyn Fn()> {
    if __DEV__ {
        let last_thenable = *thenable_state.borrow().last().unwrap();
        // Reset the last thenable back to pending.
        match last_thenable.status().as_deref() {
            Some("fulfilled") => {
                let previous_thenable_value = last_thenable.value();
                // We rebind .then immediately.
                let then: &'static dyn Fn() = unsafe { js::cast(js::get(last_thenable, "then")) };
                let previous_thenable_then = function::bind(then, Some(last_thenable), ());
                js::delete(last_thenable, "value");
                js::delete(last_thenable, "status");
                // We'll call .then again if we resuspend. Since we potentially corrupted
                // the internal state of unknown classes, we need to diffuse the potential
                // crash by replacing the .then method with a noop.
                js::set(last_thenable, "then", &noop);
                Box::new(move || {
                    js::set(last_thenable, "then", previous_thenable_then);
                    last_thenable.set_value(previous_thenable_value);
                    last_thenable.set_status("fulfilled");
                })
            }
            Some("rejected") => {
                let previous_thenable_reason = last_thenable.reason();
                // We rebind .then immediately.
                let then: &'static dyn Fn() = unsafe { js::cast(js::get(last_thenable, "then")) };
                let previous_thenable_then = function::bind(then, Some(last_thenable), ());
                js::delete(last_thenable, "reason");
                js::delete(last_thenable, "status");
                // We'll call .then again if we resuspend. Since we potentially corrupted
                // the internal state of unknown classes, we need to diffuse the potential
                // crash by replacing the .then method with a noop.
                js::set(last_thenable, "then", &noop);
                Box::new(move || {
                    js::set(last_thenable, "then", previous_thenable_then);
                    last_thenable.set_reason(previous_thenable_reason);
                    last_thenable.set_status("rejected");
                })
            }
            _ => Box::new(noop),
        }
    } else {
        // eslint-disable-next-line react-internal/prod-error-codes
        panic!(concat!(
            "ensureSuspendableThenableStateDEV was called in a production environment. ",
            "This is a bug in React."
        ));
    }
}
