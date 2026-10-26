// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::panic::{self, AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::rc::Rc;

use js::{Error, JsError, Kind, Unknown};
use react_isomorphic::react_lazy::{LazyComponent, Thenable};
use shared::environment::__DEV__;
use shared::noop::noop;
use shared::react_feature_flags::ENABLE_CONDITIONAL_USE_WARNING;
use shared::react_shared_internals::REACT_SHARED_INTERNALS;
use webapi::{console, performance};

use crate::react_fiber_call_user_space::CALL_LAZY_INIT_IN_DEV;
use crate::react_internal_types::{Fiber, FiberRoot};
use crate::react_work_tags::HOST_ROOT;

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#getWorkInProgressRoot"
)]
#[cfg_attr(rust_js, rust_js::nullable)]
fn get_work_in_progress_root() -> Option<Rc<FiberRoot>> {
    unreachable!()
}

/// The thenables a component used, in the order it used them, and whether
/// it was warned of an uncached one (DEV only).
pub struct ThenableState {
    did_warn_about_uncached_promise: Cell<bool>,
    thenables: RefCell<Vec<&'static Thenable>>,
}

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
    ));

    #[cfg_attr(rust_js, rust_js::name = "SuspenseyCommitException")]
    pub static SUSPENSEY_COMMIT_EXCEPTION: &'static Error = js::error::new(concat!(
        "Suspense Exception: This is not a real error, and should not leak into ",
        "userspace. If you're seeing this, it's likely a bug in React."
    ));

    #[cfg_attr(rust_js, rust_js::name = "SuspenseActionException")]
    pub static SUSPENSE_ACTION_EXCEPTION: &'static Error = js::error::new(concat!(
        "Suspense Exception: This is not a real error! It's an implementation ",
        "detail of `useActionState` to interrupt the current render. You must either ",
        "rethrow it immediately, or move the `useActionState` call outside of the ",
        "`try/catch` block. Capturing without rethrowing will lead to ",
        "unexpected behavior.\n\n",
        "To handle async errors, wrap your component in an error boundary."
    ));
}

/// A thenable that's never listened to: `then` is only a warning.
pub struct NoopSuspenseyCommitThenable {
    pub then: fn(),
}

unsafe impl js::Defined for NoopSuspenseyCommitThenable {}

// This is a noop thenable that we use to trigger a fallback in throwException.
// TODO: It would be better to refactor throwException into multiple functions
// so we can trigger a fallback directly without having to check the type. But
// for now this will do.
#[cfg_attr(rust_js, rust_js::name = "noopSuspenseyCommitThenable")]
pub static NOOP_SUSPENSEY_COMMIT_THENABLE: NoopSuspenseyCommitThenable =
    NoopSuspenseyCommitThenable {
        then: || {
            if __DEV__ {
                console::error((concat!(
                    "Internal React error: A listener was unexpectedly attached to a ",
                    "\"noop\" thenable. This is a bug in React. Please file an issue."
                ),));
            }
        },
    };

pub fn create_thenable_state() -> ThenableState {
    // The ThenableState is created the first time a component suspends. If it
    // suspends again, we'll reuse the same state.
    ThenableState {
        did_warn_about_uncached_promise: Cell::new(false),
        thenables: RefCell::new(Vec::new()),
    }
}

pub fn is_thenable_resolved(thenable: &Thenable) -> bool {
    let status = thenable.status();
    matches!(status.as_deref(), Some("fulfilled" | "rejected"))
}

thread_local! {
    // DEV-only
    #[cfg_attr(rust_js, rust_js::nullable)]
    static LAST_SUSPENDED_FIBER: RefCell<Option<Rc<Fiber>>> = const { RefCell::new(None) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static LAST_SUSPENDED_STACK: Cell<Option<&'static Error>> = const { Cell::new(None) };
    static DID_ISSUE_USE_WARNING: Cell<bool> = const { Cell::new(false) };
}

pub fn has_potential_use_warnings() -> bool {
    ENABLE_CONDITIONAL_USE_WARNING && LAST_SUSPENDED_FIBER.with_borrow(Option::is_some)
}

pub fn clear_use_warnings() {
    LAST_SUSPENDED_FIBER.set(None);
}

/// shared/ReactTypes' `ReactIOInfo`, of the fields React infers of a
/// thenable that has no debug info.
struct InferredIOInfo {
    name: &'static str,
    start: f64,
    end: Cell<f64>,
    value: &'static Thenable,
}

/// shared/ReactTypes' `ReactAsyncInfo`, of an `InferredIOInfo`.
struct InferredAsyncInfo {
    awaited: Rc<InferredIOInfo>,
}

#[cfg_attr(rust_js, rust_js::nullable(fiber))]
pub fn track_used_thenable(
    thenable_state: &ThenableState,
    thenable: &'static Thenable,
    index: usize,
    fiber: Option<&Rc<Fiber>>, // DEV-only
) -> Option<&'static Unknown> {
    let mut thenable = thenable;
    if __DEV__ && REACT_SHARED_INTERNALS.act_queue().is_some() {
        js::set(REACT_SHARED_INTERNALS, "didUsePromise", true);
    }
    let tracked_thenables = &thenable_state.thenables;
    let previous = tracked_thenables.borrow().get(index).copied();
    match previous {
        None => tracked_thenables.borrow_mut().push(thenable),
        Some(previous) => {
            if !ptr::eq(previous, thenable) {
                // Reuse the previous thenable, and drop the new one. We can assume
                // they represent the same value, because components are idempotent.

                if __DEV__ && !thenable_state.did_warn_about_uncached_promise.get() {
                    // We should only warn the first time an uncached thenable is
                    // discovered per component, because if there are multiple, the
                    // subsequent ones are likely derived from the first.
                    //
                    // We track this on the thenableState instead of deduping using the
                    // component name like we usually do, because in the case of a
                    // promise-as-React-node, the owner component is likely different from
                    // the parent that's currently being reconciled. We'd have to track
                    // the owner using state, which we're trying to move away from. Though
                    // since this is dev-only, maybe that'd be OK.
                    //
                    // However, another benefit of doing it this way is we might
                    // eventually have a thenableState per memo/Forget boundary instead
                    // of per component, so this would allow us to have more
                    // granular warnings.
                    thenable_state.did_warn_about_uncached_promise.set(true);

                    // TODO: This warning should link to a corresponding docs page.
                    console::error((concat!(
                        "A component was suspended by an uncached promise. Creating ",
                        "promises inside a Client Component or hook is not yet ",
                        "supported, except via a Suspense-compatible library or framework."
                    ),));
                }

                // Avoid an unhandled rejection errors for the Promises that we'll
                // intentionally ignore.
                thenable.then(|_| noop(), |_| noop());
                thenable = previous;
            }
        }
    }

    if __DEV__
        && let Kind::Undefined = js::classify(js::get(thenable, "_debugInfo"))
        // Debug info is inferred by attaching a `_debugInfo` expando to the
        // thenable. If the thenable doesn't allow new properties (e.g. it's
        // frozen), the assignment would throw, so we skip the instrumentation.
        && js::object::is_extensible(js::unknown(thenable))
    {
        // In DEV mode if the thenable that we observed had no debug info, then we add
        // an inferred debug info so that we're able to track its potential I/O uniquely.
        // We don't know the real start time since the I/O could have started much
        // earlier and this could even be a cached Promise. Could be misleading.
        let start_time = performance.now();
        let display_name = thenable.display_name();
        let io_info = Rc::new(InferredIOInfo {
            name: match js::classify(display_name) {
                Kind::String(display_name) => display_name,
                _ => "Promise",
            },
            start: start_time,
            end: Cell::new(start_time),
            value: thenable,
            // We don't know the requesting owner nor stack.
        });
        // We can infer the await owner/stack lazily from where this promise ends up
        // used. It can be used in more than one place so we can't assign it here.
        js::set(
            thenable,
            "_debugInfo",
            vec![InferredAsyncInfo {
                awaited: Rc::clone(&io_info),
            }],
        );
        // Track when we resolved the Promise as the approximate end time.
        if !matches!(thenable.status().as_deref(), Some("fulfilled" | "rejected")) {
            let track_end_time = move |_| {
                io_info.end.set(performance.now());
            };
            thenable.then(track_end_time.clone(), track_end_time);
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
            check_if_use_wrapped_in_async_catch(rejected_error);

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
                // This is an uncached thenable that we haven't seen before.

                // Detect infinite ping loops caused by uncached promises.
                if let Some(root) = get_work_in_progress_root()
                    && root.shell_suspend_counter.get() > 100
                {
                    // This root has suspended repeatedly in the shell without making any
                    // progress (i.e. committing something). This is highly suggestive of
                    // an infinite ping loop, often caused by an accidental Async Client
                    // Component.
                    //
                    // During a transition, we can suspend the work loop until the promise
                    // to resolve, but this is a sync render, so that's not an option. We
                    // also can't show a fallback, because none was provided. So our last
                    // resort is to throw an error.
                    //
                    // TODO: Remove this error in a future release. Other ways of handling
                    // this case include forcing a concurrent render, or putting the whole
                    // root into offscreen mode.
                    panic!(concat!(
                        "An unknown Component is an async Client Component. ",
                        "Only Server Components can be async at the moment. ",
                        "This error is often caused by accidentally ",
                        "adding `'use client'` to a module that was originally written ",
                        "for the server."
                    ));
                }

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

            // Check one more time in case the thenable resolved synchronously.
            match thenable.status().as_deref() {
                Some("fulfilled") => return thenable.value(),
                Some("rejected") => {
                    let rejected_error = thenable.reason();
                    check_if_use_wrapped_in_async_catch(rejected_error);
                    js::throw(rejected_error)
                }
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
            if __DEV__ {
                NEEDS_TO_RESET_SUSPENDED_THENABLE_DEV.set(true);
                if ENABLE_CONDITIONAL_USE_WARNING
                    && !DID_ISSUE_USE_WARNING.get()
                    && let Some(fiber) = fiber
                    // Only track initial mount for now to avoid warning too much for updates.
                    && fiber.alternate.borrow().is_none()
                {
                    LAST_SUSPENDED_FIBER.set(Some(Rc::clone(fiber)));
                    // Stash an error in case we end up triggering the use() warning.
                    // This ensures that we have a stack trace at the location of the first use()
                    // call since there won't be a second one we have to do that eagerly.
                    LAST_SUSPENDED_STACK.set(Some(js::error::new(concat!(
                        "This library called use() to suspend in a previous render but ",
                        "did not call use() when it finished. This indicates an incorrect use of use(). ",
                        "Learn more: https://react.dev/warnings/conditional-use-of-use"
                    ))));
                }
            }
            SUSPENSE_EXCEPTION.with(|exception| js::throw(exception.as_unknown()))
        }
    }
}

pub fn suspend_commit() -> ! {
    // This extra indirection only exists so it can handle passing
    // noopSuspenseyCommitThenable through to throwException.
    // TODO: Factor the thenable check out of throwException
    SUSPENDED_THENABLE.set(Some(unsafe {
        js::cast(Some(js::unknown(&NOOP_SUSPENSEY_COMMIT_THENABLE)))
    }));
    SUSPENSEY_COMMIT_EXCEPTION.with(|exception| js::throw(exception.as_unknown()))
}

pub fn resolve_lazy(lazy_type: &LazyComponent) -> Option<&'static Unknown> {
    match catch_unwind(AssertUnwindSafe(|| {
        if __DEV__ {
            return CALL_LAZY_INIT_IN_DEV
                .with(|call_lazy_init_in_dev| call_lazy_init_in_dev.unwrap()(lazy_type));
        }
        let payload = &lazy_type.payload;
        let init = lazy_type.init;
        init(payload)
    })) {
        Ok(value) => value,
        Err(x) => {
            if let Some(&x) = x.downcast_ref::<&'static JsError>()
                && matches!(
                    js::classify(x.as_unknown()),
                    Kind::Object(_) | Kind::Array(_)
                )
                && let Kind::Function(_) = js::classify(js::get(x.as_unknown(), "then"))
            {
                // This lazy Suspended. Treat this as if we called use() to unwrap it.
                SUSPENDED_THENABLE.set(Some(unsafe { js::cast(x.as_unknown()) }));
                if __DEV__ {
                    NEEDS_TO_RESET_SUSPENDED_THENABLE_DEV.set(true);
                }
                SUSPENSE_EXCEPTION.with(|exception| js::throw(exception.as_unknown()));
            }
            panic::resume_unwind(x)
        }
    }
}

thread_local! {
    // This is used to track the actual thenable that suspended so it can be
    // passed to the rest of the Suspense implementation — which, for historical
    // reasons, expects to receive a thenable.
    #[cfg_attr(rust_js, rust_js::nullable)]
    static SUSPENDED_THENABLE: Cell<Option<&'static Thenable>> = const { Cell::new(None) };
    #[cfg_attr(rust_js, rust_js::name = "needsToResetSuspendedThenableDEV")]
    static NEEDS_TO_RESET_SUSPENDED_THENABLE_DEV: Cell<bool> = const { Cell::new(false) };
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
    if __DEV__ {
        NEEDS_TO_RESET_SUSPENDED_THENABLE_DEV.set(false);
    }
    thenable
}

pub fn check_if_use_wrapped_in_try_catch() -> bool {
    // This was set right before SuspenseException was thrown, and it should
    // have been cleared when the exception was handled. If it wasn't,
    // it must have been caught by userspace.
    if __DEV__ && NEEDS_TO_RESET_SUSPENDED_THENABLE_DEV.get() {
        NEEDS_TO_RESET_SUSPENDED_THENABLE_DEV.set(false);
        return true;
    }
    false
}

pub fn check_if_use_wrapped_in_async_catch(rejected_reason: Option<&Unknown>) {
    // This check runs in prod, too, because it prevents a more confusing
    // downstream error, where SuspenseException is caught by a promise and
    // thrown asynchronously.
    // TODO: Another way to prevent SuspenseException from leaking into an async
    // execution context is to check the dispatcher every time `use` is called,
    // or some equivalent. That might be preferable for other reasons, too, since
    // it matches how we prevent similar mistakes for other hooks.
    if SUSPENSE_EXCEPTION.with(|exception| js::strict_equals(rejected_reason, *exception))
        || SUSPENSE_ACTION_EXCEPTION
            .with(|exception| js::strict_equals(rejected_reason, *exception))
    {
        panic!(concat!(
            "Hooks are not supported inside an async component. This ",
            "error is often caused by accidentally adding `'use client'` ",
            "to a module that was originally written for the server."
        ));
    }
}

fn are_same_key_path(a: &Fiber, b: &Fiber) -> bool {
    if ptr::eq(a, b) {
        return true;
    }
    if a.tag.get() != b.tag.get()
        || !js::strict_equals(a.r#type.get(), b.r#type.get())
        || *a.key.borrow() != *b.key.borrow()
        || a.index.get() != b.index.get()
    {
        return false;
    }
    if a.tag.get() == HOST_ROOT && !js::strict_equals(a.state_node.get(), b.state_node.get()) {
        // These are both roots but they're different roots so they're not in the same tree.
        return false;
    }
    let (Some(a_return), Some(b_return)) =
        (a.r#return.borrow().clone(), b.r#return.borrow().clone())
    else {
        return false;
    };
    are_same_key_path(&a_return, &b_return)
}

#[cfg_attr(rust_js, rust_js::nullable(thenable_state))]
pub fn check_if_use_was_used_before(
    unsuspended_fiber: &Fiber,
    thenable_state: Option<&ThenableState>,
) {
    if __DEV__
        && ENABLE_CONDITIONAL_USE_WARNING
        && let Some(last_suspended_fiber) = LAST_SUSPENDED_FIBER.with_borrow(Clone::clone)
        && are_same_key_path(&last_suspended_fiber, unsuspended_fiber)
    {
        if thenable_state.is_some() {
            // It's still using use() ever after resolving. We could warn for different number of them but for
            // now we treat this as ok and clear the state.
            LAST_SUSPENDED_FIBER.set(None);
            LAST_SUSPENDED_STACK.set(None);
        } else {
            // The last suspended Fiber using use() is no longer using use() in the same position.
            // That's suspicious. Likely it was unblocked by conditionally using use() which is incorrect.
            if let Some(last_suspended_stack) = LAST_SUSPENDED_STACK.get()
                && !DID_ISSUE_USE_WARNING.get()
            {
                DID_ISSUE_USE_WARNING.set(true);
                // We pass the error object instead of custom message so that the browser displays the error natively.
                console::error((last_suspended_stack,));
            }
            LAST_SUSPENDED_FIBER.set(None);
            LAST_SUSPENDED_STACK.set(None);
        }
    }
}
