// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::RefCell;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::panic::{self, AssertUnwindSafe};
use std::rc::Rc;

use js::{JsError, JsObject, Unknown};
use shared::environment::__DEV__;
use shared::noop::noop;
use shared::react_feature_flags::{
    ENABLE_GESTURE_TRANSITION, ENABLE_TRANSITION_TRACING, ENABLE_VIEW_TRANSITION,
};
use shared::react_shared_internals::{REACT_SHARED_INTERNALS, SharedStateClient};
use shared::report_global_error::REPORT_GLOBAL_ERROR;
use webapi::console;

use crate::react_hooks::StartTransitionOptions;
use crate::react_lazy::Thenable;
use crate::react_transition_type::TransitionTypes;

/// The reconciler's `Fiber`, `ReactInternalTypes`', which a Transition only
/// holds: each is itself, as a JS `Set` holds it.
pub struct Fiber(PhantomData<JsObject>);

impl PartialEq for Fiber {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Eq for Fiber {}

impl Hash for Fiber {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::ptr::hash(self, state);
    }
}

/// `shared/ReactTypes`' `GestureProvider`: intrinsic, it varies by
/// environment whether a particular renderer supports it.
pub type GestureProvider = Unknown;

/// `shared/ReactTypes`' `GestureOptions & StartTransitionOptions`.
pub struct GestureTransitionOptions {
    pub range_start: Option<f64>,
    pub range_end: Option<f64>,
    pub name: Option<String>,
}

pub struct Transition {
    // enableViewTransition
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub types: RefCell<Option<Rc<RefCell<TransitionTypes>>>>,
    // enableGestureTransition
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub gesture: Option<&'static GestureProvider>,
    // enableTransitionTracing only
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub name: Option<String>,
    // enableTransitionTracing only
    pub start_time: Option<f64>,
    // DEV-only
    #[cfg_attr(rust_js, rust_js::name = "_updatedFibers")]
    pub updated_fibers: Option<RefCell<HashSet<&'static Fiber>>>,
}

/// What the renderer does as a Transition's scope returns.
pub type OnStartTransitionFinish = dyn Fn(&Transition, Option<&'static Unknown>);

/// What the renderer does as a gesture Transition's scope returns, which
/// gives what cancels the gesture.
pub type OnStartGestureTransitionFinish =
    dyn Fn(&Transition, &GestureProvider, Option<&GestureTransitionOptions>) -> Box<dyn Fn()>;

/// `ReactSharedInternals.T`, the current Transition.
#[cfg_attr(rust_js, rust_js::link_name = "get T")]
#[cfg_attr(rust_js, rust_js::nullable)]
pub fn t(this: &SharedStateClient) -> Option<&'static Transition> {
    let _ = this;
    unreachable!()
}

/// The current Transition from now on, `None` `null`.
#[cfg_attr(rust_js, rust_js::link_name = "set T")]
#[cfg_attr(rust_js, rust_js::nullable(transition))]
pub fn set_t(this: &SharedStateClient, transition: Option<&Transition>) {
    let _ = (this, transition);
    unreachable!()
}

/// `ReactSharedInternals.S`, what the renderer does as a Transition's scope
/// returns.
#[cfg_attr(rust_js, rust_js::link_name = "get S")]
#[cfg_attr(rust_js, rust_js::nullable)]
pub fn s(this: &SharedStateClient) -> Option<&'static OnStartTransitionFinish> {
    let _ = this;
    unreachable!()
}

/// `ReactSharedInternals.S = on_start_transition_finish`.
#[cfg_attr(rust_js, rust_js::link_name = "set S")]
pub fn set_s(
    this: &SharedStateClient,
    on_start_transition_finish: &'static OnStartTransitionFinish,
) {
    let _ = (this, on_start_transition_finish);
    unreachable!()
}

/// `ReactSharedInternals.G`, what the renderer does as a gesture
/// Transition's scope returns.
#[cfg_attr(rust_js, rust_js::link_name = "get G")]
#[cfg_attr(rust_js, rust_js::nullable)]
pub fn g(this: &SharedStateClient) -> Option<&'static OnStartGestureTransitionFinish> {
    let _ = this;
    unreachable!()
}

/// `ReactSharedInternals.G = on_start_gesture_transition_finish`.
#[cfg_attr(rust_js, rust_js::link_name = "set G")]
pub fn set_g(
    this: &SharedStateClient,
    on_start_gesture_transition_finish: Box<OnStartGestureTransitionFinish>,
) {
    let _ = (this, on_start_gesture_transition_finish);
    unreachable!()
}

fn release_async_transition() {
    if __DEV__ {
        REACT_SHARED_INTERNALS
            .set_async_transitions(REACT_SHARED_INTERNALS.async_transitions() - 1.0);
    }
}

/// What `start_transition`'s `finally` does.
struct FinishTransition<'a> {
    prev_transition: Option<&'static Transition>,
    current_transition: &'a Transition,
}

impl Drop for FinishTransition<'_> {
    fn drop(&mut self) {
        warn_about_transition_subscriptions(self.prev_transition, self.current_transition);
        let current_types = self.current_transition.types.borrow().clone();
        if let Some(prev_transition) = self.prev_transition
            && let Some(current_types) = current_types
        {
            // If we created a new types set in the inner transition, we transfer it to the parent
            // since they should share the same set. They're conceptually entangled.
            let prev_types = prev_transition.types.borrow().clone();
            if __DEV__
                && let Some(prev_types) = prev_types
                && !Rc::ptr_eq(&prev_types, &current_types)
            {
                // Just assert that assumption holds that we're not overriding anything.
                console::error((concat!(
                    "We expected inner Transitions to have transferred the outer types set and ",
                    "that you cannot add to the outer Transition while inside the inner.",
                    "This is a bug in React."
                ),));
            }
            prev_transition.types.replace(Some(current_types));
        }
        set_t(REACT_SHARED_INTERNALS, self.prev_transition);
    }
}

pub fn start_transition(
    scope: impl FnOnce() -> Option<&'static Unknown>,
    options: Option<&StartTransitionOptions>,
) {
    let prev_transition = t(REACT_SHARED_INTERNALS);
    let mut current_transition = Transition {
        types: RefCell::new(None),
        gesture: None,
        name: None,
        start_time: None,
        updated_fibers: None,
    };
    if ENABLE_VIEW_TRANSITION {
        *current_transition.types.get_mut() = match prev_transition {
            // If we're a nested transition, we should use the same set as the parent
            // since we're conceptually always joined into the same entangled transition.
            // In practice, this only matters if we add transition types in the inner
            // without setting state. In that case, the inner transition can finish
            // without waiting for the outer.
            Some(prev_transition) => prev_transition.types.borrow().clone(),
            None => None,
        };
    }
    if ENABLE_GESTURE_TRANSITION.with(|&enabled| enabled) {
        current_transition.gesture = None;
    }
    if ENABLE_TRANSITION_TRACING {
        current_transition.name = options.and_then(|options| options.name.clone());
        current_transition.start_time = Some(-1.0); // TODO: This should read the timestamp.
    }
    if __DEV__ {
        current_transition.updated_fibers = Some(RefCell::new(HashSet::new()));
    }
    set_t(REACT_SHARED_INTERNALS, Some(&current_transition));

    let _finish = FinishTransition {
        prev_transition,
        current_transition: &current_transition,
    };
    if let Err(error) = panic::catch_unwind(AssertUnwindSafe(|| {
        let return_value = scope();
        if let Some(on_start_transition_finish) = s(REACT_SHARED_INTERNALS) {
            on_start_transition_finish(&current_transition, return_value);
        }
        if js::type_of(return_value) == "object"
            // $FlowFixMe[invalid-compare]
            && let Some(return_value) = return_value
            && js::type_of(js::get(return_value, "then")) == "function"
        {
            let return_value: &Thenable = unsafe { js::cast(Some(return_value)) };
            if __DEV__ {
                // Keep track of the number of async transitions still running so we can warn.
                REACT_SHARED_INTERNALS
                    .set_async_transitions(REACT_SHARED_INTERNALS.async_transitions() + 1.0);
                return_value.then(
                    |_| release_async_transition(),
                    |_| release_async_transition(),
                );
            }
            REPORT_GLOBAL_ERROR
                .with(|&report_global_error| return_value.then(|_| noop(), report_global_error));
        }
    })) {
        REPORT_GLOBAL_ERROR.with(|report_global_error| {
            report_global_error(
                error
                    .downcast_ref::<&'static JsError>()
                    .and_then(|error| error.as_unknown()),
            );
        });
    }
}

/// What `start_gesture_transition`'s `finally` does.
struct RestoreTransition {
    prev_transition: Option<&'static Transition>,
}

impl Drop for RestoreTransition {
    fn drop(&mut self) {
        set_t(REACT_SHARED_INTERNALS, self.prev_transition);
    }
}

pub fn start_gesture_transition(
    provider: Option<&'static GestureProvider>,
    scope: impl FnOnce() -> Option<&'static Unknown>,
    options: Option<&GestureTransitionOptions>,
) -> Box<dyn Fn()> {
    if !ENABLE_GESTURE_TRANSITION.with(|&enabled| enabled) {
        // eslint-disable-next-line react-internal/prod-error-codes
        panic!(
            "startGestureTransition should not be exported when the enableGestureTransition flag is off."
        );
    }
    let Some(provider) = provider else {
        // We enforce this at runtime even though the type also enforces it since we
        // use null as a signal internally so it would lead it to be treated as a
        // regular transition otherwise.
        panic!("A Timeline is required as the first argument to startGestureTransition.");
    };
    let prev_transition = t(REACT_SHARED_INTERNALS);
    let mut current_transition = Transition {
        types: RefCell::new(None),
        gesture: None,
        name: None,
        start_time: None,
        updated_fibers: None,
    };
    if ENABLE_VIEW_TRANSITION {
        *current_transition.types.get_mut() = None;
    }
    // $FlowFixMe[constant-condition]
    if ENABLE_GESTURE_TRANSITION.with(|&enabled| enabled) {
        current_transition.gesture = Some(provider);
    }
    if ENABLE_TRANSITION_TRACING {
        current_transition.name = options.and_then(|options| options.name.clone());
        current_transition.start_time = Some(-1.0); // TODO: This should read the timestamp.
    }
    if __DEV__ {
        current_transition.updated_fibers = Some(RefCell::new(HashSet::new()));
    }
    set_t(REACT_SHARED_INTERNALS, Some(&current_transition));

    let _restore = RestoreTransition { prev_transition };
    match panic::catch_unwind(AssertUnwindSafe(|| {
        let return_value = scope();
        if __DEV__
            && js::type_of(return_value) == "object"
            // $FlowFixMe[invalid-compare]
            && let Some(return_value) = return_value
            && js::type_of(js::get(return_value, "then")) == "function"
        {
            console::error((
                "Cannot use an async function in startGestureTransition. It must be able to start immediately.",
            ));
        }
        g(REACT_SHARED_INTERNALS).map(|on_start_gesture_transition_finish| {
            on_start_gesture_transition_finish(&current_transition, provider, options)
        })
    })) {
        Ok(Some(finish)) => return finish,
        Ok(None) => {}
        Err(error) => REPORT_GLOBAL_ERROR.with(|report_global_error| {
            report_global_error(
                error
                    .downcast_ref::<&'static JsError>()
                    .and_then(|error| error.as_unknown()),
            );
        }),
    }
    Box::new(noop)
}

fn warn_about_transition_subscriptions(
    prev_transition: Option<&Transition>,
    current_transition: &Transition,
) {
    if __DEV__
        && prev_transition.is_none()
        && let Some(updated_fibers) = &current_transition.updated_fibers
    {
        let updated_fibers_count = updated_fibers.borrow().len();
        updated_fibers.borrow_mut().clear();
        if updated_fibers_count > 10 {
            console::warn((concat!(
                "Detected a large number of updates inside startTransition. ",
                "If this is due to a subscription please re-write it to use React provided hooks. ",
                "Otherwise concurrent mode guarantees are off the table."
            ),));
        }
    }
}
