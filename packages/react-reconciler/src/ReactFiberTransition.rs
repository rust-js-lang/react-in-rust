// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::rc::Rc;

use js::Unknown;
use react_isomorphic::react_lazy::Thenable;
use react_isomorphic::react_start_transition::{
    OnStartTransitionFinish, Transition, g, s, set_g, set_s, t,
};
use shared::react_feature_flags::{
    ENABLE_GESTURE_TRANSITION, ENABLE_TRANSITION_TRACING, ENABLE_VIEW_TRANSITION,
};
use shared::react_shared_internals::REACT_SHARED_INTERNALS;

use crate::react_fiber_async_action::{entangle_async_action, peek_entangled_action_lane};
use crate::react_fiber_cache_component::{
    CACHE_CONTEXT, Cache, SpawnedCachePool, create_cache, retain_cache,
};
use crate::react_fiber_gesture_scheduler::{
    ScheduledGesture, cancel_scheduled_gesture, start_scheduled_gesture,
};
use crate::react_fiber_lane::{Lanes, NO_LANE};
use crate::react_fiber_root_scheduler::FIRST_SCHEDULED_ROOT;
use crate::react_fiber_stack::{StackCursor, create_cursor, pop, push};
use crate::react_fiber_transition_types::{
    ENTANGLED_TRANSITION_TYPES, entangle_async_transition_types, queue_transition_types,
};
use crate::react_internal_types::{Fiber, FiberRoot};
use crate::react_profiler_timer::start_async_transition_timer;

unsafe extern "Rust" {
    #[link_name = "./src/ReactFiberConfig.js#isPrimaryRenderer"]
    safe static IS_PRIMARY_RENDERER: bool;
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#getWorkInProgressRoot"
)]
#[cfg_attr(rust_js, rust_js::nullable)]
fn get_work_in_progress_root() -> Option<Rc<FiberRoot>> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#getWorkInProgressTransitions"
)]
#[cfg_attr(rust_js, rust_js::nullable)]
fn get_work_in_progress_transitions() -> Option<Rc<Vec<&'static Transition>>> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#markTransitionStarted"
)]
fn mark_transition_started() {
    unreachable!()
}

#[cfg_attr(rust_js, rust_js::name = "NoTransition")]
#[cfg_attr(rust_js, rust_js::nullable)]
pub const NO_TRANSITION: Option<&Transition> = None;

// Attach this reconciler instance's onStartTransitionFinish implementation to
// the shared internals object. This is used by the isomorphic implementation of
// startTransition to compose all the startTransitions together.
//
//   function startTransition(fn) {
//     return startTransitionDOM(() => {
//       return startTransitionART(() => {
//         return startTransitionThreeFiber(() => {
//           // and so on...
//           return fn();
//         });
//       });
//     });
//   }
//
// Currently we only compose together the code that runs at the end of each
// startTransition, because for now that's sufficient — the part that sets
// isTransition=true on the stack uses a separate shared internal field. But
// really we should delete the shared field and track isTransition per
// reconciler. Leaving this for a future PR.
thread_local! {
    static PREV_ON_START_TRANSITION_FINISH: Option<&'static OnStartTransitionFinish> =
        s(REACT_SHARED_INTERNALS);
}

fn on_start_transition_finish_for_reconciler(
    transition: &Transition,
    return_value: Option<&'static Unknown>,
) {
    mark_transition_started();
    if js::type_of(return_value) == "object"
        && let Some(return_value) = return_value
        && js::type_of(js::get(return_value, "then")) == "function"
    {
        // If we're going to wait on some async work before scheduling an update.
        // We mark the time so we can later log how long we were blocked on the Action.
        // Ideally, we'd include the sync part of the action too but since that starts
        // in isomorphic code it currently leads to tricky layering. We'd have to pass
        // in performance.now() to this callback but we sometimes use a polyfill.
        start_async_transition_timer();

        // This is an async action
        let thenable: &'static Thenable = unsafe { js::cast(Some(return_value)) };
        entangle_async_action(transition, thenable);
    }
    if ENABLE_VIEW_TRANSITION {
        if let Some(entangled_transition_types) =
            ENTANGLED_TRANSITION_TYPES.with_borrow(Clone::clone)
        {
            let entangled_transition_types = entangled_transition_types.borrow().clone();
            // If we scheduled work on any new roots, we need to add any entangled async
            // transition types to those roots too.
            let mut root = FIRST_SCHEDULED_ROOT.with_borrow(Clone::clone);
            while let Some(scheduled_root) = root {
                queue_transition_types(&scheduled_root, &entangled_transition_types);
                root = scheduled_root.next.borrow().clone();
            }
        }
        let transition_types = transition.types.borrow().clone();
        if let Some(transition_types) = transition_types {
            let transition_types = transition_types.borrow().clone();
            // Within this Transition we should've now scheduled any roots we have updates
            // to work on. If there are no updates on a root, then the Transition type won't
            // be applied to that root.
            let mut root = FIRST_SCHEDULED_ROOT.with_borrow(Clone::clone);
            while let Some(scheduled_root) = root {
                queue_transition_types(&scheduled_root, &transition_types);
                root = scheduled_root.next.borrow().clone();
            }
            if peek_entangled_action_lane() != NO_LANE {
                // If we have entangled, async actions going on, the update associated with
                // these types might come later. We need to save them for later.
                entangle_async_transition_types(&transition_types);
            }
        }
    }
    if let Some(prev_on_start_transition_finish) =
        PREV_ON_START_TRANSITION_FINISH.with(|&prev| prev)
    {
        prev_on_start_transition_finish(transition, return_value);
    }
}

js::on_load! {
    set_s(REACT_SHARED_INTERNALS, &on_start_transition_finish_for_reconciler);
}

fn chain_gesture_cancellation(
    root: Rc<FiberRoot>,
    scheduled_gesture: Rc<ScheduledGesture>,
    prev_cancel: Option<Box<dyn Fn()>>,
) -> Box<dyn Fn()> {
    Box::new(move || {
        cancel_scheduled_gesture(&root, &scheduled_gesture);
        if let Some(prev_cancel) = &prev_cancel {
            prev_cancel();
        }
    })
}

js::on_load! {
    if ENABLE_GESTURE_TRANSITION.with(|&enabled| enabled) {
        let prev_on_start_gesture_transition_finish = g(REACT_SHARED_INTERNALS);
        set_g(
            REACT_SHARED_INTERNALS,
            Box::new(move |transition, provider, options| {
                let mut cancel = None;
                if let Some(prev_on_start_gesture_transition_finish) =
                    prev_on_start_gesture_transition_finish
                {
                    cancel = Some(prev_on_start_gesture_transition_finish(
                        transition, provider, options,
                    ));
                }
                // For every root that has work scheduled, check if there's a ScheduledGesture
                // matching this provider and if so, increase its ref count so its retained by
                // this cancellation callback. We could add the roots to a temporary array as
                // we schedule them inside the callback to keep track of them. There's a slight
                // nuance here which is that if there's more than one root scheduled with the
                // same provider, but it doesn't update in this callback, then we still update
                // its options and retain it until this cancellation releases. The idea being
                // that it's conceptually started globally.
                let mut root = FIRST_SCHEDULED_ROOT.with_borrow(Clone::clone);
                while let Some(scheduled_root) = root {
                    let transition_types = transition.types.borrow().clone();
                    let scheduled_gesture =
                        start_scheduled_gesture(&scheduled_root, provider, options, transition_types);
                    if let Some(scheduled_gesture) = scheduled_gesture {
                        cancel = Some(chain_gesture_cancellation(
                            Rc::clone(&scheduled_root),
                            scheduled_gesture,
                            cancel,
                        ));
                    }
                    root = scheduled_root.next.borrow().clone();
                }
                if let Some(cancel) = cancel {
                    return cancel;
                }
                Box::new(|| {
                    // Nothing was scheduled but it could've been scheduled by another renderer.
                })
            }),
        );
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn request_current_transition() -> Option<&'static Transition> {
    t(REACT_SHARED_INTERNALS)
}

thread_local! {
    // When retrying a Suspense/Offscreen boundary, we restore the cache that was
    // used during the previous render by placing it here, on the stack.
    #[cfg_attr(rust_js, rust_js::nullable)]
    static RESUMED_CACHE: StackCursor<Option<Rc<Cache>>> = create_cursor(None);

    // During the render/synchronous commit phase, we don't actually process the
    // transitions. Therefore, we want to lazily combine transitions. Instead of
    // comparing the arrays of transitions when we combine them and storing them
    // and filtering out the duplicates, we will instead store the unprocessed transitions
    // in an array and actually filter them in the passive phase.
    #[cfg_attr(rust_js, rust_js::nullable)]
    static TRANSITION_STACK: StackCursor<Option<Rc<Vec<&'static Transition>>>> =
        create_cursor(None);
}

fn peek_cache_from_pool() -> Option<Rc<Cache>> {
    // Check if the cache pool already has a cache we can use.

    // If we're rendering inside a Suspense boundary that is currently hidden,
    // we should use the same cache that we used during the previous render, if
    // one exists.
    let cache_resumed_from_previous_render =
        RESUMED_CACHE.with(|resumed_cache| resumed_cache.current.borrow().clone());
    if cache_resumed_from_previous_render.is_some() {
        return cache_resumed_from_previous_render;
    }

    // Otherwise, check the root's cache pool.
    let root = get_work_in_progress_root().unwrap();
    root.pooled_cache.borrow().clone()
}

pub fn request_cache_from_pool(render_lanes: Lanes) -> Rc<Cache> {
    // Similar to previous function, except if there's not already a cache in the
    // pool, we allocate a new one.
    let cache_from_pool = peek_cache_from_pool();
    if let Some(cache_from_pool) = cache_from_pool {
        return cache_from_pool;
    }

    // Create a fresh cache and add it to the root cache pool. A cache can have
    // multiple owners:
    // - A cache pool that lives on the FiberRoot. This is where all fresh caches
    //   are originally created (TODO: except during refreshes, until we implement
    //   this correctly). The root takes ownership immediately when the cache is
    //   created. Conceptually, root.pooledCache is an Option<Arc<Cache>> (owned),
    //   and the return value of this function is a &Arc<Cache> (borrowed).
    // - One of several fiber types: host root, cache boundary, suspense
    //   component. These retain and release in the commit phase.

    let root = get_work_in_progress_root().unwrap();
    let fresh_cache = create_cache();
    root.pooled_cache.replace(Some(Rc::clone(&fresh_cache)));
    retain_cache(&fresh_cache);
    root.pooled_cache_lanes
        .set(root.pooled_cache_lanes.get() | render_lanes);
    fresh_cache
}

pub fn push_root_transition(work_in_progress: &Rc<Fiber>, _root: &FiberRoot, _render_lanes: Lanes) {
    if ENABLE_TRANSITION_TRACING {
        let root_transitions = get_work_in_progress_transitions();
        TRANSITION_STACK.with(|transition_stack| {
            push(transition_stack, root_transitions, work_in_progress);
        });
    }
}

pub fn pop_root_transition(work_in_progress: &Fiber, _root: &FiberRoot, _render_lanes: Lanes) {
    if ENABLE_TRANSITION_TRACING {
        TRANSITION_STACK.with(|transition_stack| pop(transition_stack, work_in_progress));
    }
}

pub fn push_transition(
    offscreen_work_in_progress: &Rc<Fiber>,
    prev_cache_pool: Option<&SpawnedCachePool>,
    new_transitions: Option<Rc<Vec<&'static Transition>>>,
) {
    RESUMED_CACHE.with(|resumed_cache| match prev_cache_pool {
        None => {
            let current = resumed_cache.current.borrow().clone();
            push(resumed_cache, current, offscreen_work_in_progress);
        }
        Some(prev_cache_pool) => push(
            resumed_cache,
            Some(Rc::clone(&prev_cache_pool.pool)),
            offscreen_work_in_progress,
        ),
    });

    if ENABLE_TRANSITION_TRACING {
        TRANSITION_STACK.with(|transition_stack| {
            let current = transition_stack.current.borrow().clone();
            match (current, new_transitions) {
                (None, new_transitions) => {
                    push(
                        transition_stack,
                        new_transitions,
                        offscreen_work_in_progress,
                    );
                }
                (current, None) => push(transition_stack, current, offscreen_work_in_progress),
                (Some(current), Some(new_transitions)) => push(
                    transition_stack,
                    Some(Rc::new([&current[..], &new_transitions[..]].concat())),
                    offscreen_work_in_progress,
                ),
            }
        });
    }
}

pub fn pop_transition(work_in_progress: &Fiber, current: Option<&Fiber>) {
    if current.is_some() {
        if ENABLE_TRANSITION_TRACING {
            TRANSITION_STACK.with(|transition_stack| pop(transition_stack, work_in_progress));
        }

        RESUMED_CACHE.with(|resumed_cache| pop(resumed_cache, work_in_progress));
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_pending_transitions() -> Option<Rc<Vec<&'static Transition>>> {
    if !ENABLE_TRANSITION_TRACING {
        return None;
    }

    TRANSITION_STACK.with(|transition_stack| transition_stack.current.borrow().clone())
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_suspended_cache() -> Option<SpawnedCachePool> {
    // This function is called when a Suspense boundary suspends. It returns the
    // cache that would have been used to render fresh data during this render,
    // if there was any, so that we can resume rendering with the same cache when
    // we receive more data.
    let cache_from_pool = peek_cache_from_pool()?;

    Some(SpawnedCachePool {
        // We must also save the parent, so that when we resume we can detect
        // a refresh.
        parent: CACHE_CONTEXT
            .with(|cache_context| {
                if IS_PRIMARY_RENDERER {
                    cache_context.current_value.borrow().clone()
                } else {
                    cache_context.current_value2.borrow().clone()
                }
            })
            .unwrap(),
        pool: cache_from_pool,
    })
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_offscreen_deferred_cache() -> Option<SpawnedCachePool> {
    let cache_from_pool = peek_cache_from_pool()?;

    Some(SpawnedCachePool {
        // We must also store the parent, so that when we resume we can detect
        // a refresh.
        parent: CACHE_CONTEXT
            .with(|cache_context| {
                if IS_PRIMARY_RENDERER {
                    cache_context.current_value.borrow().clone()
                } else {
                    cache_context.current_value2.borrow().clone()
                }
            })
            .unwrap(),
        pool: cache_from_pool,
    })
}
