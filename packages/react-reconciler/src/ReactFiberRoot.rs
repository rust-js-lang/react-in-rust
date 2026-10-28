// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use js::Unknown;
use shared::environment::__DEV__;
use shared::react_feature_flags::{
    DISABLE_LEGACY_MODE, ENABLE_DEFAULT_TRANSITION_INDICATOR, ENABLE_GESTURE_TRANSITION,
    ENABLE_PROFILER_COMMIT_HOOKS, ENABLE_PROFILER_TIMER, ENABLE_SUSPENSE_CALLBACK,
    ENABLE_TRANSITION_TRACING, ENABLE_UPDATER_TRACKING, ENABLE_VIEW_TRANSITION,
};

use crate::react_fiber::create_host_root_fiber;
use crate::react_fiber_cache_component::{Cache, create_cache, retain_cache};
use crate::react_fiber_class_update_queue::initialize_update_queue;
use crate::react_fiber_lane::{NO_LANE, NO_LANES, NO_TIMESTAMP, TOTAL_LANES, create_lane_map};
use crate::react_internal_types::{
    FiberRoot, OnCaughtError, OnDefaultTransitionIndicator, OnError, SuspenseHydrationCallbacks,
    TransitionTracingCallbacks,
};
use crate::react_root_tags::{CONCURRENT_ROOT, LEGACY_ROOT, RootTag};

unsafe extern "Rust" {
    // ReactFiberConfig's `TimeoutHandle | NoTimeout`.
    #[link_name = "./src/ReactFiberConfig.js#noTimeout"]
    safe static NO_TIMEOUT: &'static Unknown;
}

pub struct RootState {
    // shared/ReactTypes' `ReactNodeList`.
    pub element: Option<&'static Unknown>,
    pub is_dehydrated: bool,
    pub cache: Rc<Cache>,
}

unsafe impl js::Defined for RootState {}

#[cfg_attr(rust_js, rust_js::nullable(form_state))]
pub fn create_fiber_root(
    // ReactFiberConfig's `Container`.
    container_info: &'static Unknown,
    tag: RootTag,
    hydrate: bool,
    // shared/ReactTypes' `ReactNodeList`.
    initial_children: Option<&'static Unknown>,
    hydration_callbacks: Option<&'static SuspenseHydrationCallbacks>,
    is_strict_mode: bool,
    // TODO: We have several of these arguments that are conceptually part of the
    // host config, but because they are passed in at runtime, we have to thread
    // them through the root constructor. Perhaps we should put them all into a
    // single type, like a DynamicHostConfig that is defined by the renderer.
    identifier_prefix: String,
    // shared/ReactTypes' `ReactFormState<any, any>`.
    form_state: Option<&'static Unknown>,
    on_uncaught_error: &'static OnError,
    on_caught_error: &'static OnCaughtError,
    on_recoverable_error: &'static OnError,
    on_default_transition_indicator: &'static OnDefaultTransitionIndicator,
    transition_callbacks: Option<&'static TransitionTracingCallbacks>,
) -> Rc<FiberRoot> {
    // Cyclic construction. This cheats the type system right now because
    // stateNode is any.
    let uninitialized_fiber = create_host_root_fiber(tag, is_strict_mode);
    let root = Rc::new(FiberRoot {
        tag: if DISABLE_LEGACY_MODE {
            CONCURRENT_ROOT
        } else {
            tag
        },
        container_info,
        pending_children: Cell::new(None),
        current: RefCell::new(Rc::clone(&uninitialized_fiber)),
        ping_cache: Cell::new(None),
        timeout_handle: Cell::new(NO_TIMEOUT),
        cancel_pending_commit: RefCell::new(None),
        context: Cell::new(None),
        pending_context: Cell::new(None),
        next: RefCell::new(None),
        callback_node: Cell::new(None),
        callback_priority: Cell::new(NO_LANE),
        expiration_times: RefCell::new(create_lane_map(NO_TIMESTAMP)),

        pending_lanes: Cell::new(NO_LANES),
        suspended_lanes: Cell::new(NO_LANES),
        pinged_lanes: Cell::new(NO_LANES),
        warm_lanes: Cell::new(NO_LANES),
        expired_lanes: Cell::new(NO_LANES),
        indicator_lanes: Cell::new(NO_LANES),
        error_recovery_disabled_lanes: Cell::new(NO_LANES),
        shell_suspend_counter: Cell::new(0),

        entangled_lanes: Cell::new(NO_LANES),
        entanglements: RefCell::new(create_lane_map(NO_LANES)),

        hidden_updates: RefCell::new(vec![None; TOTAL_LANES]),

        identifier_prefix,
        on_uncaught_error,
        on_caught_error,
        on_recoverable_error,

        on_default_transition_indicator: Cell::new(None),
        pending_indicator: Cell::new(None),

        pooled_cache: RefCell::new(None),
        pooled_cache_lanes: Cell::new(NO_LANES),

        hydration_callbacks: Cell::new(None),

        form_state,

        transition_types: RefCell::new(None),

        pending_gestures: RefCell::new(None),
        gesture_clone: Cell::new(None),

        incomplete_transitions: js::map::new(None),
        transition_callbacks: Cell::new(None),
        transition_lanes: RefCell::new(create_lane_map(None)),

        effect_duration: Cell::new(None),
        passive_effect_duration: Cell::new(None),

        memoized_updaters: RefCell::new(HashSet::new()),
        pending_updaters_lane_map: RefCell::new(Vec::new()),

        debug_root_type: Cell::new(None),
    });

    if ENABLE_DEFAULT_TRANSITION_INDICATOR.with(|&enabled| enabled) {
        root.on_default_transition_indicator
            .set(Some(on_default_transition_indicator));
        root.pending_indicator.set(None);
    }

    if ENABLE_SUSPENSE_CALLBACK {
        root.hydration_callbacks.set(None);
    }

    if ENABLE_VIEW_TRANSITION {
        root.transition_types.replace(None);
    }

    if ENABLE_GESTURE_TRANSITION.with(|&enabled| enabled) {
        root.pending_gestures.replace(None);
        root.gesture_clone.set(None);
    }

    if ENABLE_TRANSITION_TRACING {
        root.transition_callbacks.set(None);
    }

    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        && ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
    {
        root.effect_duration.set(Some(-0.0));
        root.passive_effect_duration.set(Some(-0.0));
    }

    if ENABLE_UPDATER_TRACKING.with(|&enabled| enabled) {
        let mut pending_updaters_lane_map = root.pending_updaters_lane_map.borrow_mut();
        for _ in 0..TOTAL_LANES {
            pending_updaters_lane_map.push(HashSet::new());
        }
    }

    if __DEV__ {
        if DISABLE_LEGACY_MODE {
            // TODO: This varies by each renderer.
            root.debug_root_type.set(Some(if hydrate {
                "hydrateRoot()"
            } else {
                "createRoot()"
            }));
        } else {
            match tag {
                CONCURRENT_ROOT => {
                    root.debug_root_type.set(Some(if hydrate {
                        "hydrateRoot()"
                    } else {
                        "createRoot()"
                    }));
                }
                LEGACY_ROOT => {
                    root.debug_root_type
                        .set(Some(if hydrate { "hydrate()" } else { "render()" }));
                }
                _ => {}
            }
        }
    }

    if ENABLE_SUSPENSE_CALLBACK {
        root.hydration_callbacks.set(hydration_callbacks);
    }

    if ENABLE_TRANSITION_TRACING {
        root.transition_callbacks.set(transition_callbacks);
    }

    uninitialized_fiber
        .state_node
        .set(Some(js::unknown_of(Rc::clone(&root))));

    let initial_cache = create_cache();
    retain_cache(&initial_cache);

    // The pooledCache is a fresh cache instance that is used temporarily
    // for newly mounted boundaries during a render. In general, the
    // pooledCache is always cleared from the root at the end of a render:
    // it is either released when render commits, or moved to an Offscreen
    // component if rendering suspends. Because the lifetime of the pooled
    // cache is distinct from the main memoizedState.cache, it must be
    // retained separately.
    root.pooled_cache.replace(Some(Rc::clone(&initial_cache)));
    retain_cache(&initial_cache);
    let initial_state = RootState {
        element: initial_children,
        is_dehydrated: hydrate,
        cache: initial_cache,
    };
    uninitialized_fiber
        .memoized_state
        .set(Some(js::unknown_of(initial_state)));

    initialize_update_queue(&uninitialized_fiber);

    root
}
