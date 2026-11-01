// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// Corresponds to ReactFiberWakeable and ReactFizzWakeable modules. Generally,
// changes to one module should be reflected in the others.

// TODO: Rename this module and the corresponding Fiber one to "Thenable"
// instead of "Wakeable". Or some other more appropriate name.

use std::cell::{Cell, RefCell};
use std::ptr;

use js::{Error, Kind, Unknown};
use shared::environment::__DEV__;
use shared::noop::noop;
use shared::react_types::Thenable;

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
        None => {
            thenable_state.borrow_mut().push(thenable);
            if __DEV__ {
                let stacks = match js::get(js::unknown(thenable_state), "_stacks") {
                    Some(stacks) if js::truthy(Some(stacks)) => stacks,
                    _ => {
                        let stacks = js::unknown_of(Vec::<&'static Error>::new());
                        js::set(js::unknown(thenable_state), "_stacks", stacks);
                        stacks
                    }
                };
                let stacks: &RefCell<Vec<&'static Error>> = unsafe { js::cast(Some(stacks)) };
                stacks.borrow_mut().push(js::error::new("", None));
            }
        }
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
            SUSPENSE_EXCEPTION.with(|exception| js::throw(exception.as_unknown()))
        }
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
