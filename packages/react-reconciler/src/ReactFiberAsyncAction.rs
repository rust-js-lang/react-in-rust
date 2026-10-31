// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::panic::{self, AssertUnwindSafe};
use std::ptr;
use std::rc::Rc;

use js::{JsError, Unknown};
use react_isomorphic::react_start_transition::Transition;
use shared::noop::noop;
use shared::react_feature_flags::{
    ENABLE_COMPONENT_PERFORMANCE_TRACK, ENABLE_DEFAULT_TRANSITION_INDICATOR, ENABLE_PROFILER_TIMER,
};
use shared::react_types::Thenable;
use shared::report_global_error::REPORT_GLOBAL_ERROR;

use crate::react_fiber_lane::{Lane, NO_LANE};
use crate::react_fiber_root_scheduler::{ensure_schedule_is_scheduled, request_transition_lane};
use crate::react_fiber_transition_types::clear_entangled_async_transition_types;
use crate::react_profiler_timer::{clear_async_transition_timer, has_scheduled_transition_work};

/// What a thenable's `then` is given, called once it settles.
type Listener = &'static dyn Fn(Option<&'static Unknown>);

/// The listeners a thenable of this module's notifies once it settles.
type Listeners = Rc<RefCell<Vec<Listener>>>;

/// A `Thenable<void>` that's only ever consumed by React, which settles it
/// itself once every entangled scope function has finished.
struct EntangledThenable {
    status: &'static str,
    value: Option<&'static Unknown>,
    then: Box<dyn Fn(Listener)>,
}

unsafe impl js::Defined for EntangledThenable {}

/// A `Thenable<T>` that's only ever consumed by React, which settles it
/// itself to the value it's chained to.
struct ThenableWithOverride {
    status: &'static str,
    #[cfg_attr(rust_js, rust_js::nullable)]
    value: Option<&'static Unknown>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    reason: Option<&'static Unknown>,
    then: Box<dyn Fn(Listener)>,
}

unsafe impl js::Defined for ThenableWithOverride {}

/// What a root's `onDefaultTransitionIndicator` is: it starts the indicator,
/// and may give what stops it.
type DefaultTransitionIndicator = &'static dyn Fn() -> Option<&'static dyn Fn()>;

/// The default indicator of every root.
#[derive(Clone, Copy)]
#[cfg_attr(rust_js, rust_js::untagged)]
enum IsomorphicDefaultTransitionIndicator {
    // We haven't had any roots registered yet.
    #[cfg_attr(rust_js, rust_js::undefined)]
    Unregistered,
    // There's more than one callback.
    #[cfg_attr(rust_js, rust_js::null)]
    Ambiguous,
    Registered(DefaultTransitionIndicator),
}

// If there are multiple, concurrent async actions, they are entangled. All
// transition updates that occur while the async action is still in progress
// are treated as part of the action.
//
// The ideal behavior would be to treat each async function as an independent
// action. However, without a mechanism like AsyncContext, we can't tell which
// action an update corresponds to. So instead, we entangle them all into one.
thread_local! {
    // The listeners to notify once the entangled scope completes.
    #[cfg_attr(rust_js, rust_js::nullable)]
    static CURRENT_ENTANGLED_LISTENERS: RefCell<Option<Listeners>> = const { RefCell::new(None) };
    // The number of pending async actions in the entangled scope.
    static CURRENT_ENTANGLED_PENDING_COUNT: Cell<i32> = const { Cell::new(0) };
    // The transition lane shared by all updates in the entangled scope.
    static CURRENT_ENTANGLED_LANE: Cell<Lane> = const { Cell::new(NO_LANE) };
    // A thenable that resolves when the entangled scope completes. It does not
    // resolve to a particular value because it's only used for suspending the UI
    // until the async action scope has completed.
    #[cfg_attr(rust_js, rust_js::nullable)]
    static CURRENT_ENTANGLED_ACTION_THENABLE: Cell<Option<&'static Thenable>> =
        const { Cell::new(None) };

    // Track the default indicator for every root. undefined means we haven't
    // had any roots registered yet. null means there's more than one callback.
    // If there's more than one callback we bailout to not supporting isomorphic
    // default indicators.
    static ISOMORPHIC_DEFAULT_TRANSITION_INDICATOR: Cell<IsomorphicDefaultTransitionIndicator> =
        const { Cell::new(IsomorphicDefaultTransitionIndicator::Unregistered) };
    // The clean up function for the currently running indicator.
    #[cfg_attr(rust_js, rust_js::nullable)]
    static PENDING_ISOMORPHIC_INDICATOR: Cell<Option<&'static dyn Fn()>> =
        const { Cell::new(None) };
    // The number of roots that have pending Transitions that depend on the
    // started isomorphic indicator.
    static PENDING_ENTANGLED_ROOTS: Cell<i32> = const { Cell::new(0) };
    static NEEDS_ISOMORPHIC_INDICATOR: Cell<bool> = const { Cell::new(false) };
}

pub fn entangle_async_action(
    transition: &Transition,
    thenable: &'static Thenable,
) -> &'static Thenable {
    // `thenable` is the return value of the async action scope function. Create
    // a combined thenable that resolves once every entangled scope function
    // has finished.
    if CURRENT_ENTANGLED_LISTENERS.with_borrow(Option::is_none) {
        // There's no outer async action scope. Create a new one.
        let entangled_listeners: Listeners = Rc::new(RefCell::new(Vec::new()));
        CURRENT_ENTANGLED_LISTENERS.set(Some(Rc::clone(&entangled_listeners)));
        CURRENT_ENTANGLED_PENDING_COUNT.set(0);
        CURRENT_ENTANGLED_LANE.set(request_transition_lane(Some(transition)));
        let entangled_thenable: &'static Thenable = unsafe {
            js::cast(Some(js::unknown_of(EntangledThenable {
                status: "pending",
                value: None,
                then: Box::new(move |resolve| entangled_listeners.borrow_mut().push(resolve)),
            })))
        };
        CURRENT_ENTANGLED_ACTION_THENABLE.set(Some(entangled_thenable));
        if ENABLE_DEFAULT_TRANSITION_INDICATOR.with(|&enabled| enabled) {
            NEEDS_ISOMORPHIC_INDICATOR.set(true);
            // We'll check if we need a default indicator in a microtask. Ensure
            // we have this scheduled even if no root is scheduled.
            ensure_schedule_is_scheduled();
        }
    }
    CURRENT_ENTANGLED_PENDING_COUNT.set(CURRENT_ENTANGLED_PENDING_COUNT.get() + 1);
    thenable.then(
        |_| ping_engtangled_action_scope(),
        |_| ping_engtangled_action_scope(),
    );
    thenable
}

fn ping_engtangled_action_scope() {
    CURRENT_ENTANGLED_PENDING_COUNT.set(CURRENT_ENTANGLED_PENDING_COUNT.get() - 1);
    if CURRENT_ENTANGLED_PENDING_COUNT.get() == 0 {
        if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
            && ENABLE_COMPONENT_PERFORMANCE_TRACK
            && !has_scheduled_transition_work()
        {
            // If we have received no updates since we started the entangled Actions
            // that means it didn't lead to a Transition being rendered. We need to
            // clear the timer so that if we start another entangled sequence we use
            // the next start timer instead of appearing like we were blocked the
            // whole time. We currently don't log a track for Actions that don't
            // render a Transition.
            clear_async_transition_timer();
        }
        clear_entangled_async_transition_types();
        if PENDING_ENTANGLED_ROOTS.get() == 0 {
            stop_isomorphic_default_indicator();
        }
        if let Some(listeners) = CURRENT_ENTANGLED_LISTENERS.take() {
            // All the actions have finished. Close the entangled async action scope
            // and notify all the listeners.
            if let Some(fulfilled_thenable) = CURRENT_ENTANGLED_ACTION_THENABLE.get() {
                fulfilled_thenable.set_status("fulfilled");
            }
            CURRENT_ENTANGLED_LANE.set(NO_LANE);
            CURRENT_ENTANGLED_ACTION_THENABLE.set(None);
            NEEDS_ISOMORPHIC_INDICATOR.set(false);
            let mut i = 0;
            while i < listeners.borrow().len() {
                let listener = listeners.borrow()[i];
                listener(None);
                i += 1;
            }
        }
    }
}

pub fn chain_thenable_value<T: Copy + 'static>(
    thenable: &Thenable,
    result: T,
) -> &'static Thenable {
    // Equivalent to: Promise.resolve(thenable).then(() => result), except we can
    // cheat a bit since we know that that this thenable is only ever consumed
    // by React.
    //
    // We don't technically require promise support on the client yet, hence this
    // extra code.
    let listeners: Listeners = Rc::new(RefCell::new(Vec::new()));
    let thenable_with_override: &'static Thenable = unsafe {
        js::cast(Some(js::unknown_of(ThenableWithOverride {
            status: "pending",
            value: None,
            reason: None,
            then: {
                let listeners = Rc::clone(&listeners);
                Box::new(move |resolve| listeners.borrow_mut().push(resolve))
            },
        })))
    };
    thenable.then(
        {
            let listeners = Rc::clone(&listeners);
            move |_value| {
                let fulfilled_thenable = thenable_with_override;
                fulfilled_thenable.set_status("fulfilled");
                fulfilled_thenable.set_value(js::any_of(result));
                let mut i = 0;
                while i < listeners.borrow().len() {
                    let listener = listeners.borrow()[i];
                    listener(js::any_of(result));
                    i += 1;
                }
            }
        },
        move |error| {
            let rejected_thenable = thenable_with_override;
            rejected_thenable.set_status("rejected");
            rejected_thenable.set_reason(error);
            let mut i = 0;
            while i < listeners.borrow().len() {
                let listener = listeners.borrow()[i];
                // This is a perf hack where we call the `onFulfill` ping function
                // instead of `onReject`, because we know that React is the only
                // consumer of these promises, and it passes the same listener to both.
                // We also know that it will read the error directly off the
                // `.reason` field.
                listener(None);
                i += 1;
            }
        },
    );
    thenable_with_override
}

pub fn peek_entangled_action_lane() -> Lane {
    CURRENT_ENTANGLED_LANE.get()
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn peek_entangled_action_thenable() -> Option<&'static Thenable> {
    CURRENT_ENTANGLED_ACTION_THENABLE.get()
}

pub fn register_default_indicator(on_default_transition_indicator: DefaultTransitionIndicator) {
    if !ENABLE_DEFAULT_TRANSITION_INDICATOR.with(|&enabled| enabled) {
        return;
    }
    match ISOMORPHIC_DEFAULT_TRANSITION_INDICATOR.get() {
        IsomorphicDefaultTransitionIndicator::Unregistered => {
            ISOMORPHIC_DEFAULT_TRANSITION_INDICATOR.set(
                IsomorphicDefaultTransitionIndicator::Registered(on_default_transition_indicator),
            );
        }
        IsomorphicDefaultTransitionIndicator::Registered(indicator)
            if ptr::addr_eq(indicator, on_default_transition_indicator) => {}
        _ => {
            ISOMORPHIC_DEFAULT_TRANSITION_INDICATOR
                .set(IsomorphicDefaultTransitionIndicator::Ambiguous);
            // Stop any on-going indicator since it's now ambiguous.
            stop_isomorphic_default_indicator();
        }
    }
}

pub fn start_isomorphic_default_indicator_if_needed() {
    if !ENABLE_DEFAULT_TRANSITION_INDICATOR.with(|&enabled| enabled) {
        return;
    }
    if !NEEDS_ISOMORPHIC_INDICATOR.get() {
        return;
    }
    if let IsomorphicDefaultTransitionIndicator::Registered(isomorphic_default_transition_indicator) =
        ISOMORPHIC_DEFAULT_TRANSITION_INDICATOR.get()
        && PENDING_ISOMORPHIC_INDICATOR.get().is_none()
    {
        match panic::catch_unwind(AssertUnwindSafe(isomorphic_default_transition_indicator)) {
            Ok(cleanup) => PENDING_ISOMORPHIC_INDICATOR.set(Some(cleanup.unwrap_or(&noop))),
            Err(x) => {
                PENDING_ISOMORPHIC_INDICATOR.set(Some(&noop));
                REPORT_GLOBAL_ERROR.with(|report_global_error| {
                    report_global_error(
                        x.downcast_ref::<&'static JsError>()
                            .and_then(|x| x.as_unknown()),
                    );
                });
            }
        }
    }
}

fn stop_isomorphic_default_indicator() {
    if !ENABLE_DEFAULT_TRANSITION_INDICATOR.with(|&enabled| enabled) {
        return;
    }
    if let Some(cleanup) = PENDING_ISOMORPHIC_INDICATOR.take() {
        cleanup();
    }
}

fn release_isomorphic_indicator() {
    PENDING_ENTANGLED_ROOTS.set(PENDING_ENTANGLED_ROOTS.get() - 1);
    if PENDING_ENTANGLED_ROOTS.get() == 0 {
        stop_isomorphic_default_indicator();
    }
}

pub fn has_ongoing_isomorphic_indicator() -> bool {
    PENDING_ISOMORPHIC_INDICATOR.get().is_some()
}

pub fn retain_isomorphic_indicator() -> &'static dyn Fn() {
    PENDING_ENTANGLED_ROOTS.set(PENDING_ENTANGLED_ROOTS.get() + 1);
    &release_isomorphic_indicator
}

pub fn mark_isomorphic_indicator_handled() {
    NEEDS_ISOMORPHIC_INDICATOR.set(false);
}
