// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js::Unknown;
use shared::environment::__DEV__;
use shared::react_feature_flags::ENABLE_GESTURE_TRANSITION;
use shared::react_shared_internals::Dispatcher;

type OnStartTransitionFinish = dyn Fn(&Unknown, Option<&Unknown>);
type OnStartGestureTransitionFinish = dyn Fn(&Unknown, &Unknown, Option<&Unknown>) -> Box<dyn Fn()>;

pub struct SharedStateClient {
    // ReactCurrentDispatcher for Hooks
    #[cfg_attr(rust_js, rust_js::name = "H")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub h: Cell<Option<&'static Dispatcher>>,
    // ReactCurrentCache for Cache
    #[cfg_attr(rust_js, rust_js::name = "A")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub a: Cell<Option<&'static Unknown>>,
    // ReactCurrentBatchConfig for Transitions
    #[cfg_attr(rust_js, rust_js::name = "T")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub t: Cell<Option<&'static Unknown>>,
    #[cfg_attr(rust_js, rust_js::name = "S")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub s: Cell<Option<&'static OnStartTransitionFinish>>,
    #[cfg_attr(rust_js, rust_js::name = "G")]
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub g: Cell<Option<&'static OnStartGestureTransitionFinish>>,

    // DEV-only

    // ReactCurrentActQueue
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub act_queue: RefCell<Option<ActQueue>>,

    // When zero this means we're outside an async startTransition.
    pub async_transitions: Cell<Option<f64>>,

    // Used to reproduce behavior of `batchedUpdates` in legacy mode.
    pub is_batching_legacy: Cell<Option<bool>>,
    pub did_schedule_legacy_update: Cell<Option<bool>>,

    // Tracks whether something called `use` during the current batch of work.
    // Determines whether we should yield to microtasks to unwrap already resolved
    // promises without suspending.
    pub did_use_promise: Cell<Option<bool>>,

    // Track first uncaught error within this act
    pub thrown_errors: RefCell<Option<Vec<Option<&'static Unknown>>>>,

    // ReactDebugCurrentFrame
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub get_current_stack: Cell<Option<fn() -> String>>,

    // ReactOwnerStackReset
    pub recently_created_owner_stacks: Cell<Option<f64>>,
}

/// `actQueue`: what an `act` scope's renderers do next, one array the
/// renderers push to and `act` flushes.
pub type ActQueue = Rc<RefCell<Vec<RendererTask>>>;

#[derive(Clone)]
#[repr(transparent)]
pub struct RendererTask(pub &'static dyn Fn(bool) -> Option<RendererTask>);

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "ReactSharedInternals")]
    pub(crate) static REACT_SHARED_INTERNALS: SharedStateClient = SharedStateClient {
        h: Cell::new(None),
        a: Cell::new(None),
        t: Cell::new(None),
        s: Cell::new(None),
        g: Cell::new(None),
        act_queue: RefCell::new(None),
        async_transitions: Cell::new(None),
        is_batching_legacy: Cell::new(None),
        did_schedule_legacy_update: Cell::new(None),
        did_use_promise: Cell::new(None),
        thrown_errors: RefCell::new(None),
        get_current_stack: Cell::new(None),
        recently_created_owner_stacks: Cell::new(None),
    };
}
js::on_load! {
    REACT_SHARED_INTERNALS.with(|react_shared_internals| {
        if ENABLE_GESTURE_TRANSITION.with(|&enabled| enabled) {
            react_shared_internals.g.set(None);
        }

        if __DEV__ {
            react_shared_internals.act_queue.replace(None);
            react_shared_internals.async_transitions.set(Some(0.0));
            react_shared_internals.is_batching_legacy.set(Some(false));
            react_shared_internals.did_schedule_legacy_update.set(Some(false));
            react_shared_internals.did_use_promise.set(Some(false));
            react_shared_internals.thrown_errors.replace(Some(Vec::new()));
            // Stack implementation injected by the current renderer.
            react_shared_internals.get_current_stack.set(None);
            react_shared_internals.recently_created_owner_stacks.set(Some(0.0));
        }
    });
}

js::export_default!(REACT_SHARED_INTERNALS);
