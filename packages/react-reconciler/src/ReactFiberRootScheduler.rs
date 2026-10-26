// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use js::{Dict, JsError, Unknown};
use react_isomorphic::react_start_transition::Transition;
use shared::environment::__DEV__;
use shared::noop::noop;
use shared::react_feature_flags::{
    DISABLE_LEGACY_MODE, DISABLE_SCHEDULER_TIMEOUT_IN_WORK_LOOP,
    ENABLE_COMPONENT_PERFORMANCE_TRACK, ENABLE_DEFAULT_TRANSITION_INDICATOR,
    ENABLE_GESTURE_TRANSITION, ENABLE_PROFILER_NESTED_UPDATE_PHASE, ENABLE_PROFILER_TIMER,
    ENABLE_YIELDING_BEFORE_PASSIVE,
};
use shared::react_shared_internals::REACT_SHARED_INTERNALS;
use shared::report_global_error::REPORT_GLOBAL_ERROR;

use crate::react_event_priorities::{
    CONTINUOUS_EVENT_PRIORITY, DEFAULT_EVENT_PRIORITY, DISCRETE_EVENT_PRIORITY,
    IDLE_EVENT_PRIORITY, lanes_to_event_priority,
};
use crate::react_fiber_async_action::{
    has_ongoing_isomorphic_indicator, mark_isomorphic_indicator_handled,
    peek_entangled_action_lane, retain_isomorphic_indicator,
    start_isomorphic_default_indicator_if_needed,
};
use crate::react_fiber_lane::{
    DEFAULT_LANE, Lane, Lanes, NO_LANE, NO_LANES, SYNC_LANE, check_if_root_is_prerendering,
    claim_next_transition_update_lane, get_highest_priority_lane, get_next_lanes,
    get_next_lanes_to_flush_sync, includes_sync_lane, is_gesture_render,
    mark_starved_lanes_as_expired,
};
use crate::react_internal_types::FiberRoot;
use crate::react_profiler_timer::{reset_nested_update_flag, sync_nested_update_flag};
use crate::react_root_tags::LEGACY_ROOT;
use crate::scheduler::{
    CANCEL_CALLBACK as SCHEDULER_CANCEL_CALLBACK, IDLE_PRIORITY as IDLE_SCHEDULER_PRIORITY,
    IMMEDIATE_PRIORITY as IMMEDIATE_SCHEDULER_PRIORITY,
    NORMAL_PRIORITY as NORMAL_SCHEDULER_PRIORITY, NOW as now, PriorityLevel,
    SCHEDULE_CALLBACK as SCHEDULER_SCHEDULE_CALLBACK, SchedulerCallback,
    USER_BLOCKING_PRIORITY as USER_BLOCKING_SCHEDULER_PRIORITY,
};

unsafe extern "Rust" {
    #[link_name = "./src/ReactFiberWorkLoop.js#CommitContext"]
    safe static COMMIT_CONTEXT: u8;
    #[link_name = "./src/ReactFiberWorkLoop.js#NoContext"]
    safe static NO_CONTEXT: u8;
    #[link_name = "./src/ReactFiberWorkLoop.js#RenderContext"]
    safe static RENDER_CONTEXT: u8;
    #[link_name = "./src/ReactFiberConfig.js#supportsMicrotasks"]
    safe static SUPPORTS_MICROTASKS: bool;
    // ReactFiberConfig's `TimeoutHandle`'s none.
    #[link_name = "./src/ReactFiberConfig.js#noTimeout"]
    safe static NO_TIMEOUT: &'static Unknown;
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#flushPendingEffects"
)]
fn flush_pending_effects() -> bool {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#flushPendingEffectsDelayed"
)]
fn flush_pending_effects_delayed() -> bool {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#getExecutionContext"
)]
fn get_execution_context() -> u8 {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#getWorkInProgressRoot",
    rust_js::nullable
)]
fn get_work_in_progress_root() -> Option<Rc<FiberRoot>> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#getWorkInProgressRootRenderLanes"
)]
fn get_work_in_progress_root_render_lanes() -> Lanes {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#getRootWithPendingPassiveEffects",
    rust_js::nullable
)]
fn get_root_with_pending_passive_effects() -> Option<Rc<FiberRoot>> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#getPendingPassiveEffectsLanes"
)]
fn get_pending_passive_effects_lanes() -> Lanes {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#hasPendingCommitEffects"
)]
fn has_pending_commit_effects() -> bool {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#isWorkLoopSuspendedOnData"
)]
fn is_work_loop_suspended_on_data() -> bool {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#performWorkOnRoot"
)]
fn perform_work_on_root(root: &FiberRoot, lanes: Lanes, force_sync: bool) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#scheduleMicrotask"
)]
fn schedule_microtask(callback: Box<dyn FnOnce()>) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#shouldAttemptEagerTransition"
)]
fn should_attempt_eager_transition() -> bool {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#trackSchedulerEvent"
)]
fn track_scheduler_event() {
    unreachable!()
}

thread_local! {
    // A linked list of all the roots with pending work. In an idiomatic app,
    // there's only a single root, but we do support multi root apps, hence this
    // extra complexity. But this module is optimized for the single root case.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static FIRST_SCHEDULED_ROOT: RefCell<Option<Rc<FiberRoot>>> = const { RefCell::new(None) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static LAST_SCHEDULED_ROOT: RefCell<Option<Rc<FiberRoot>>> = const { RefCell::new(None) };

    // Used to prevent redundant mircotasks from being scheduled.
    static DID_SCHEDULE_MICROTASK: Cell<bool> = const { Cell::new(false) };
    // `act` "microtasks" are scheduled on the `act` queue instead of an actual
    // microtask, so we have to dedupe those separately. This wouldn't be an issue
    // if we required all `act` calls to be awaited, which we might in the future.
    #[cfg_attr(rust_js, rust_js::name = "didScheduleMicrotask_act")]
    static DID_SCHEDULE_MICROTASK_ACT: Cell<bool> = const { Cell::new(false) };

    // Used to quickly bail out of flushSync if there's no sync work to do.
    static MIGHT_HAVE_PENDING_SYNC_WORK: Cell<bool> = const { Cell::new(false) };

    static IS_FLUSHING_WORK: Cell<bool> = const { Cell::new(false) };

    static CURRENT_EVENT_TRANSITION_LANE: Cell<Lane> = const { Cell::new(NO_LANE) };
}

pub fn ensure_root_is_scheduled(root: &Rc<FiberRoot>) {
    // This function is called whenever a root receives an update. It does two
    // things 1) it ensures the root is in the root schedule, and 2) it ensures
    // there's a pending microtask to process the root schedule.
    //
    // Most of the actual scheduling logic does not happen until
    // `scheduleTaskForRootDuringMicrotask` runs.

    // Add the root to the schedule
    if LAST_SCHEDULED_ROOT.with_borrow(|last_scheduled_root| {
        last_scheduled_root
            .as_ref()
            .is_some_and(|last_scheduled_root| Rc::ptr_eq(last_scheduled_root, root))
    }) || root.next.borrow().is_some()
    {
        // Fast path. This root is already scheduled.
    } else {
        let last_scheduled_root = LAST_SCHEDULED_ROOT.with_borrow(Clone::clone);
        match last_scheduled_root {
            None => {
                FIRST_SCHEDULED_ROOT.set(Some(Rc::clone(root)));
                LAST_SCHEDULED_ROOT.set(Some(Rc::clone(root)));
            }
            Some(last_scheduled_root) => {
                last_scheduled_root.next.replace(Some(Rc::clone(root)));
                LAST_SCHEDULED_ROOT.set(Some(Rc::clone(root)));
            }
        }
    }

    // Any time a root received an update, we set this to true until the next time
    // we process the schedule. If it's false, then we can quickly exit flushSync
    // without consulting the schedule.
    MIGHT_HAVE_PENDING_SYNC_WORK.set(true);

    ensure_schedule_is_scheduled();

    if __DEV__
        && !DISABLE_LEGACY_MODE
        && REACT_SHARED_INTERNALS.is_batching_legacy()
        && root.tag == LEGACY_ROOT
    {
        // Special `act` case: Record whenever a legacy update is scheduled.
        REACT_SHARED_INTERNALS.set_did_schedule_legacy_update(true);
    }
}

pub fn ensure_schedule_is_scheduled() {
    // At the end of the current event, go through each of the roots and ensure
    // there's a task scheduled for each one at the correct priority.
    if __DEV__ && REACT_SHARED_INTERNALS.act_queue().is_some() {
        // We're inside an `act` scope.
        if !DID_SCHEDULE_MICROTASK_ACT.get() {
            DID_SCHEDULE_MICROTASK_ACT.set(true);
            schedule_immediate_root_schedule_task();
        }
    } else if !DID_SCHEDULE_MICROTASK.get() {
        DID_SCHEDULE_MICROTASK.set(true);
        schedule_immediate_root_schedule_task();
    }
}

pub fn flush_sync_work_on_all_roots() {
    // This is allowed to be called synchronously, but the caller should check
    // the execution context first.
    flush_sync_work_across_roots_impl(NO_LANES, false);
}

pub fn flush_sync_work_on_legacy_roots_only() {
    // This is allowed to be called synchronously, but the caller should check
    // the execution context first.
    if !DISABLE_LEGACY_MODE {
        flush_sync_work_across_roots_impl(NO_LANES, true);
    }
}

#[cfg_attr(rust_js, rust_js::name = "flushSyncWorkAcrossRoots_impl")]
fn flush_sync_work_across_roots_impl(sync_transition_lanes: Lanes, only_legacy: bool) {
    if IS_FLUSHING_WORK.get() {
        // Prevent reentrancy.
        // TODO: Is this overly defensive? The callers must check the execution
        // context first regardless.
        return;
    }

    if !MIGHT_HAVE_PENDING_SYNC_WORK.get() {
        // Fast path. There's no sync work to do.
        return;
    }

    // There may or may not be synchronous work scheduled. Let's check.
    let mut did_perform_some_work;
    IS_FLUSHING_WORK.set(true);
    loop {
        did_perform_some_work = false;
        let mut root = FIRST_SCHEDULED_ROOT.with_borrow(Clone::clone);
        while let Some(current) = root {
            if only_legacy && (DISABLE_LEGACY_MODE || current.tag != LEGACY_ROOT) {
                // Skip non-legacy roots.
            } else if sync_transition_lanes != NO_LANES {
                let next_lanes = get_next_lanes_to_flush_sync(&current, sync_transition_lanes);
                if next_lanes != NO_LANES {
                    // This root has pending sync work. Flush it now.
                    did_perform_some_work = true;
                    perform_sync_work_on_root(&current, next_lanes);
                }
            } else {
                let work_in_progress_root = get_work_in_progress_root();
                let work_in_progress_root_render_lanes = get_work_in_progress_root_render_lanes();
                let root_has_pending_commit = current.cancel_pending_commit.get().is_some()
                    || !js::strict_equals(current.timeout_handle.get(), NO_TIMEOUT);
                let next_lanes = get_next_lanes(
                    &current,
                    if work_in_progress_root.is_some_and(|work_in_progress_root| {
                        Rc::ptr_eq(&current, &work_in_progress_root)
                    }) {
                        work_in_progress_root_render_lanes
                    } else {
                        NO_LANES
                    },
                    root_has_pending_commit,
                );
                if (includes_sync_lane(next_lanes)
                    || (ENABLE_GESTURE_TRANSITION.with(|&enabled| enabled)
                        && is_gesture_render(next_lanes)))
                    && !check_if_root_is_prerendering(&current, next_lanes)
                {
                    // This root has pending sync work. Flush it now.
                    did_perform_some_work = true;
                    perform_sync_work_on_root(&current, next_lanes);
                }
            }
            root = current.next.borrow().clone();
        }
        if !did_perform_some_work {
            break;
        }
    }
    IS_FLUSHING_WORK.set(false);
}

fn process_root_schedule_in_immediate_task() {
    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK {
        // Track the currently executing event if there is one so we can ignore this
        // event when logging events.
        track_scheduler_event();
    }

    process_root_schedule_in_microtask();
}

fn process_root_schedule_in_microtask() {
    // This function is always called inside a microtask. It should never be
    // called synchronously.
    DID_SCHEDULE_MICROTASK.set(false);
    if __DEV__ {
        DID_SCHEDULE_MICROTASK_ACT.set(false);
    }

    // We'll recompute this as we iterate through all the roots and schedule them.
    MIGHT_HAVE_PENDING_SYNC_WORK.set(false);

    let mut sync_transition_lanes = NO_LANES;
    if CURRENT_EVENT_TRANSITION_LANE.get() != NO_LANE {
        if should_attempt_eager_transition() {
            // A transition was scheduled during an event, but we're going to try to
            // render it synchronously anyway. We do this during a popstate event to
            // preserve the scroll position of the previous page.
            sync_transition_lanes = CURRENT_EVENT_TRANSITION_LANE.get();
        } else if ENABLE_DEFAULT_TRANSITION_INDICATOR.with(|&enabled| enabled) {
            // If we have a Transition scheduled by this event it might be paired
            // with Default lane scheduled loading indicators. To unbatch it from
            // other events later on, flush it early to determine whether it
            // rendered an indicator. This ensures that setState in default priority
            // event doesn't trigger onDefaultTransitionIndicator.
            sync_transition_lanes = DEFAULT_LANE;
        }
    }

    let current_time = now();

    let mut prev: Option<Rc<FiberRoot>> = None;
    let mut root = FIRST_SCHEDULED_ROOT.with_borrow(Clone::clone);
    while let Some(current) = root {
        let next = current.next.borrow().clone();
        let next_lanes = schedule_task_for_root_during_microtask(&current, current_time);
        if next_lanes == NO_LANE {
            // This root has no more pending work. Remove it from the schedule. To
            // guard against subtle reentrancy bugs, this microtask is the only place
            // we do this — you can add roots to the schedule whenever, but you can
            // only remove them here.

            // Null this out so we know it's been removed from the schedule.
            current.next.replace(None);
            match &prev {
                None => {
                    // This is the new head of the list
                    FIRST_SCHEDULED_ROOT.set(next.clone());
                }
                Some(prev) => {
                    prev.next.replace(next.clone());
                }
            }
            if next.is_none() {
                // This is the new tail of the list
                LAST_SCHEDULED_ROOT.set(prev.clone());
            }
        } else {
            // This root still has work. Keep it in the list.
            prev = Some(current);

            // This is a fast-path optimization to early exit from
            // flushSyncWorkOnAllRoots if we can be certain that there is no remaining
            // synchronous work to perform. Set this to true if there might be sync
            // work left.
            if
            // Skip the optimization if syncTransitionLanes is set
            sync_transition_lanes != NO_LANES
                // Common case: we're not treating any extra lanes as synchronous, so we
                // can just check if the next lanes are sync.
                || includes_sync_lane(next_lanes)
                || (ENABLE_GESTURE_TRANSITION.with(|&enabled| enabled) && is_gesture_render(next_lanes))
            {
                MIGHT_HAVE_PENDING_SYNC_WORK.set(true);
            }
        }
        root = next;
    }

    // At the end of the microtask, flush any pending synchronous work. This has
    // to come at the end, because it does actual rendering work that might throw.
    // If we're in the middle of a View Transition async sequence, we don't want to
    // interrupt that sequence. Instead, we'll flush any remaining work when it
    // completes.
    if !has_pending_commit_effects() {
        flush_sync_work_across_roots_impl(sync_transition_lanes, false);
    }

    if CURRENT_EVENT_TRANSITION_LANE.get() != NO_LANE {
        // Reset Event Transition Lane so that we allocate a new one next time.
        CURRENT_EVENT_TRANSITION_LANE.set(NO_LANE);
        start_default_transition_indicator_if_needed();
    }
}

fn start_default_transition_indicator_if_needed() {
    if !ENABLE_DEFAULT_TRANSITION_INDICATOR.with(|&enabled| enabled) {
        return;
    }
    // Check if we need to start an isomorphic indicator like if an async action
    // was started.
    start_isomorphic_default_indicator_if_needed();
    // Check all the roots if there are any new indicators needed.
    let mut root = FIRST_SCHEDULED_ROOT.with_borrow(Clone::clone);
    while let Some(current) = root {
        if current.indicator_lanes.get() != NO_LANES && current.pending_indicator.get().is_none() {
            // We have new indicator lanes that requires a loading state. Start the
            // default transition indicator.
            if has_ongoing_isomorphic_indicator() {
                // We already have an isomorphic indicator going which means it has to
                // also apply to this root since it implies all roots have the same one.
                // We retain this indicator so that it keeps going until we commit this
                // root.
                current
                    .pending_indicator
                    .set(Some(retain_isomorphic_indicator()));
            } else {
                match catch_unwind(AssertUnwindSafe(|| {
                    let on_default_transition_indicator =
                        current.on_default_transition_indicator.get();
                    on_default_transition_indicator.unwrap()().unwrap_or(&noop)
                })) {
                    Ok(pending_indicator) => current.pending_indicator.set(Some(pending_indicator)),
                    Err(x) => {
                        current.pending_indicator.set(Some(&noop));
                        REPORT_GLOBAL_ERROR.with(|report_global_error| {
                            report_global_error(
                                x.downcast_ref::<&'static JsError>()
                                    .and_then(|x| x.as_unknown()),
                            )
                        });
                    }
                }
            }
        }
        root = current.next.borrow().clone();
    }
}

fn schedule_task_for_root_during_microtask(root: &Rc<FiberRoot>, current_time: f64) -> Lane {
    // This function is always called inside a microtask, or at the very end of a
    // rendering task right before we yield to the main thread. It should never be
    // called synchronously.

    // This function also never performs React work synchronously; it should
    // only schedule work to be performed later, in a separate task or microtask.

    // Check if any lanes are being starved by other work. If so, mark them as
    // expired so we know to work on those next.
    mark_starved_lanes_as_expired(root, current_time);

    // Determine the next lanes to work on, and their priority.
    let root_with_pending_passive_effects = get_root_with_pending_passive_effects();
    let pending_passive_effects_lanes = get_pending_passive_effects_lanes();
    let work_in_progress_root = get_work_in_progress_root();
    let work_in_progress_root_render_lanes = get_work_in_progress_root_render_lanes();
    let root_has_pending_commit = root.cancel_pending_commit.get().is_some()
        || !js::strict_equals(root.timeout_handle.get(), NO_TIMEOUT);
    let next_lanes = if ENABLE_YIELDING_BEFORE_PASSIVE
        && root_with_pending_passive_effects.is_some_and(|root_with_pending_passive_effects| {
            Rc::ptr_eq(root, &root_with_pending_passive_effects)
        }) {
        // This will schedule the callback at the priority of the lane but we used to
        // always schedule it at NormalPriority. Discrete will flush it sync anyway.
        // So the only difference is Idle and it doesn't seem necessarily right for that
        // to get upgraded beyond something important just because we're past commit.
        pending_passive_effects_lanes
    } else {
        get_next_lanes(
            root,
            if work_in_progress_root
                .as_ref()
                .is_some_and(|work_in_progress_root| Rc::ptr_eq(root, work_in_progress_root))
            {
                work_in_progress_root_render_lanes
            } else {
                NO_LANES
            },
            root_has_pending_commit,
        )
    };

    let existing_callback_node = root.callback_node.get();
    if
    // Check if there's nothing to work on
    next_lanes == NO_LANES
        // If this root is currently suspended and waiting for data to resolve, don't
        // schedule a task to render it. We'll either wait for a ping, or wait to
        // receive an update.
        //
        // Suspended render phase
        || (work_in_progress_root
            .is_some_and(|work_in_progress_root| Rc::ptr_eq(root, &work_in_progress_root))
            && is_work_loop_suspended_on_data())
        // Suspended commit phase
        || root.cancel_pending_commit.get().is_some()
    {
        // Fast path: There's nothing to work on.
        if existing_callback_node.is_some() {
            cancel_callback(existing_callback_node);
        }
        root.callback_node.set(None);
        root.callback_priority.set(NO_LANE);
        return NO_LANE;
    }

    // Schedule a new callback in the host environment.
    if includes_sync_lane(next_lanes)
        // If we're prerendering, then we should use the concurrent work loop
        // even if the lanes are synchronous, so that prerendering never blocks
        // the main thread.
        && !check_if_root_is_prerendering(root, next_lanes)
    {
        // Synchronous work is always flushed at the end of the microtask, so we
        // don't need to schedule an additional task.
        if existing_callback_node.is_some() {
            cancel_callback(existing_callback_node);
        }
        root.callback_priority.set(SYNC_LANE);
        root.callback_node.set(None);
        SYNC_LANE
    } else {
        // We use the highest priority lane to represent the priority of the callback.
        let existing_callback_priority = root.callback_priority.get();
        let new_callback_priority = get_highest_priority_lane(next_lanes);

        if new_callback_priority == existing_callback_priority
            // Special case related to `act`. If the currently scheduled task is a
            // Scheduler task, rather than an `act` task, cancel it and re-schedule
            // on the `act` queue.
            && !(__DEV__
                && REACT_SHARED_INTERNALS.act_queue().is_some()
                && !FAKE_ACT_CALLBACK_NODE.with(|fake_act_callback_node| {
                    js::strict_equals(existing_callback_node, *fake_act_callback_node)
                }))
        {
            // The priority hasn't changed. We can reuse the existing task.
            return new_callback_priority;
        } else {
            // Cancel the existing callback. We'll schedule a new one below.
            cancel_callback(existing_callback_node);
        }

        let scheduler_priority_level = match lanes_to_event_priority(next_lanes) {
            // Scheduler does have an "ImmediatePriority", but now that we use
            // microtasks for sync work we no longer use that. Any sync work that
            // reaches this path is meant to be time sliced.
            DISCRETE_EVENT_PRIORITY | CONTINUOUS_EVENT_PRIORITY => {
                USER_BLOCKING_SCHEDULER_PRIORITY.with(|&priority| priority)
            }
            DEFAULT_EVENT_PRIORITY => NORMAL_SCHEDULER_PRIORITY.with(|&priority| priority),
            IDLE_EVENT_PRIORITY => IDLE_SCHEDULER_PRIORITY.with(|&priority| priority),
            _ => NORMAL_SCHEDULER_PRIORITY.with(|&priority| priority),
        };

        let task_root = Rc::clone(root);
        let new_callback_node = schedule_callback(
            scheduler_priority_level,
            SchedulerCallback(Box::new(move |did_timeout| {
                perform_work_on_root_via_scheduler_task(&task_root, did_timeout)
            })),
        );

        root.callback_priority.set(new_callback_priority);
        root.callback_node.set(Some(new_callback_node));
        new_callback_priority
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn perform_work_on_root_via_scheduler_task(
    root: &Rc<FiberRoot>,
    did_timeout: bool,
) -> Option<SchedulerCallback> {
    // This is the entry point for concurrent tasks scheduled via Scheduler (and
    // postTask, in the future).

    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        && ENABLE_PROFILER_NESTED_UPDATE_PHASE.with(|&enabled| enabled)
    {
        reset_nested_update_flag();
    }

    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK {
        // Track the currently executing event if there is one so we can ignore this
        // event when logging events.
        track_scheduler_event();
    }

    if has_pending_commit_effects() {
        // We are currently in the middle of an async committing (such as a View Transition).
        // We could force these to flush eagerly but it's better to defer any work until
        // it finishes. This may not be the same root as we're waiting on.
        // TODO: This relies on the commit eventually calling ensureRootIsScheduled which
        // always calls processRootScheduleInMicrotask which in turn always loops through
        // all the roots to figure out. This is all a bit inefficient and if optimized
        // it'll need to consider rescheduling a task for any skipped roots.
        root.callback_node.set(None);
        root.callback_priority.set(NO_LANE);
        return None;
    }

    // Flush any pending passive effects before deciding which lanes to work on,
    // in case they schedule additional work.
    let original_callback_node = root.callback_node.get();
    let did_flush_passive_effects = flush_pending_effects_delayed();
    if did_flush_passive_effects {
        // Something in the passive effect phase may have canceled the current task.
        // Check if the task node for this root was changed.
        if !js::strict_equals(root.callback_node.get(), original_callback_node) {
            // The current task was canceled. Exit. We don't need to call
            // `ensureRootIsScheduled` because the check above implies either that
            // there's a new task, or that there's no remaining work on this root.
            return None;
        } else {
            // Current task was not canceled. Continue.
        }
    }

    // Determine the next lanes to work on, using the fields stored on the root.
    // TODO: We already called getNextLanes when we scheduled the callback; we
    // should be able to avoid calling it again by stashing the result on the
    // root object. However, because we always schedule the callback during
    // a microtask (scheduleTaskForRootDuringMicrotask), it's possible that
    // an update was scheduled earlier during this same browser task (and
    // therefore before the microtasks have run). That's because Scheduler batches
    // together multiple callbacks into a single browser macrotask, without
    // yielding to microtasks in between. We should probably change this to align
    // with the postTask behavior (and literally use postTask when
    // it's available).
    let work_in_progress_root = get_work_in_progress_root();
    let work_in_progress_root_render_lanes = get_work_in_progress_root_render_lanes();
    let root_has_pending_commit = root.cancel_pending_commit.get().is_some()
        || !js::strict_equals(root.timeout_handle.get(), NO_TIMEOUT);
    let lanes = get_next_lanes(
        root,
        if work_in_progress_root
            .is_some_and(|work_in_progress_root| Rc::ptr_eq(root, &work_in_progress_root))
        {
            work_in_progress_root_render_lanes
        } else {
            NO_LANES
        },
        root_has_pending_commit,
    );
    if lanes == NO_LANES {
        // No more work on this root.
        return None;
    }

    // Enter the work loop.
    // TODO: We only check `didTimeout` defensively, to account for a Scheduler
    // bug we're still investigating. Once the bug in Scheduler is fixed,
    // we can remove this, since we track expiration ourselves.
    let force_sync = !DISABLE_SCHEDULER_TIMEOUT_IN_WORK_LOOP && did_timeout;
    perform_work_on_root(root, lanes, force_sync);

    // The work loop yielded, but there may or may not be work left at the current
    // priority. Need to determine whether we need to schedule a continuation.
    // Usually `scheduleTaskForRootDuringMicrotask` only runs inside a microtask;
    // however, since most of the logic for determining if we need a continuation
    // versus a new task is the same, we cheat a bit and call it here. This is
    // only safe to do because we know we're at the end of the browser task.
    // So although it's not an actual microtask, it might as well be.
    schedule_task_for_root_during_microtask(root, now());
    if root.callback_node.get().is_some()
        && js::strict_equals(root.callback_node.get(), original_callback_node)
    {
        // The task node scheduled for this root is the same one that's
        // currently executed. Need to return a continuation.
        let root = Rc::clone(root);
        return Some(SchedulerCallback(Box::new(move |did_timeout| {
            perform_work_on_root_via_scheduler_task(&root, did_timeout)
        })));
    }
    None
}

fn perform_sync_work_on_root(root: &FiberRoot, lanes: Lanes) {
    // This is the entry point for synchronous tasks that don't go
    // through Scheduler.
    let did_flush_passive_effects = flush_pending_effects();
    if did_flush_passive_effects {
        // If passive effects were flushed, exit to the outer work loop in the root
        // scheduler, so we can recompute the priority.
        return;
    }
    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        && ENABLE_PROFILER_NESTED_UPDATE_PHASE.with(|&enabled| enabled)
    {
        sync_nested_update_flag();
    }
    let force_sync = true;
    perform_work_on_root(root, lanes, force_sync);
}

thread_local! {
    static FAKE_ACT_CALLBACK_NODE: &'static Dict<()> = js::dict::new();
}

fn schedule_callback(
    priority_level: PriorityLevel,
    callback: SchedulerCallback,
) -> &'static Unknown {
    if __DEV__ && let Some(act_queue) = REACT_SHARED_INTERNALS.act_queue() {
        // Special case: We're inside an `act` scope (a testing utility).
        // Instead of scheduling work in the host environment, add it to a
        // fake internal queue that's managed by the `act` implementation.
        let act_queue: &RefCell<Vec<SchedulerCallback>> = unsafe { js::cast(Some(act_queue)) };
        act_queue.borrow_mut().push(callback);
        FAKE_ACT_CALLBACK_NODE.with(|fake_act_callback_node| js::unknown(*fake_act_callback_node))
    } else {
        js::unknown(SCHEDULER_SCHEDULE_CALLBACK(priority_level, callback, None))
    }
}

fn cancel_callback(callback_node: Option<&'static Unknown>) {
    if __DEV__
        && FAKE_ACT_CALLBACK_NODE.with(|fake_act_callback_node| {
            js::strict_equals(callback_node, *fake_act_callback_node)
        })
    {
        // Special `act` case: check if this is the fake callback node used by
        // the `act` implementation.
    } else if let Some(callback_node) = callback_node {
        SCHEDULER_CANCEL_CALLBACK(unsafe { js::cast(Some(callback_node)) });
    }
}

fn schedule_immediate_root_schedule_task() {
    if __DEV__ && let Some(act_queue) = REACT_SHARED_INTERNALS.act_queue() {
        // Special case: Inside an `act` scope, we push microtasks to the fake `act`
        // callback queue. This is because we currently support calling `act`
        // without awaiting the result. The plan is to deprecate that, and require
        // that you always await the result so that the microtasks have a chance to
        // run. But it hasn't happened yet.
        let act_queue: &RefCell<Vec<SchedulerCallback>> = unsafe { js::cast(Some(act_queue)) };
        act_queue.borrow_mut().push(SchedulerCallback(Box::new(|_| {
            process_root_schedule_in_microtask();
            None
        })));
    }

    // TODO: Can we land supportsMicrotasks? Which environments don't support it?
    // Alternatively, can we move this check to the host config?
    if SUPPORTS_MICROTASKS {
        schedule_microtask(Box::new(|| {
            // In Safari, appending an iframe forces microtasks to run.
            // https://github.com/react/react/issues/22459
            // We don't support running callbacks in the middle of render
            // or commit so we need to check against that.
            let execution_context = get_execution_context();
            if (execution_context & (RENDER_CONTEXT | COMMIT_CONTEXT)) != NO_CONTEXT {
                // Note that this would still prematurely flush the callbacks
                // if this happens outside render or commit phase (e.g. in an event).

                // Intentionally using a macrotask instead of a microtask here. This is
                // wrong semantically but it prevents an infinite loop. The bug is
                // Safari's, not ours, so we just do our best to not crash even though
                // the behavior isn't completely correct.
                SCHEDULER_SCHEDULE_CALLBACK(
                    IMMEDIATE_SCHEDULER_PRIORITY.with(|&priority| priority),
                    SchedulerCallback(Box::new(|_| {
                        process_root_schedule_in_immediate_task();
                        None
                    })),
                    None,
                );
                return;
            }
            process_root_schedule_in_microtask();
        }));
    } else {
        // If microtasks are not supported, use Scheduler.
        SCHEDULER_SCHEDULE_CALLBACK(
            IMMEDIATE_SCHEDULER_PRIORITY.with(|&priority| priority),
            SchedulerCallback(Box::new(|_| {
                process_root_schedule_in_immediate_task();
                None
            })),
            None,
        );
    }
}

#[cfg_attr(rust_js, rust_js::nullable(_transition))]
pub fn request_transition_lane(
    // This argument isn't used, it's only here to encourage the caller to
    // check that it's inside a transition before calling this function.
    // TODO: Make this non-nullable. Requires a tweak to useOptimistic.
    _transition: Option<&Transition>,
) -> Lane {
    // The algorithm for assigning an update to a lane should be stable for all
    // updates at the same priority within the same event. To do this, the
    // inputs to the algorithm must be the same.
    //
    // The trick we use is to cache the first of each of these inputs within an
    // event. Then reset the cached values once we can be sure the event is
    // over. Our heuristic for that is whenever we enter a concurrent work loop.
    if CURRENT_EVENT_TRANSITION_LANE.get() == NO_LANE {
        // All transitions within the same event are assigned the same lane.
        let action_scope_lane = peek_entangled_action_lane();
        CURRENT_EVENT_TRANSITION_LANE.set(if action_scope_lane != NO_LANE {
            // We're inside an async action scope. Reuse the same lane.
            action_scope_lane
        } else {
            // We may or may not be inside an async action scope. If we are, this
            // is the first update in that scope. Either way, we need to get a
            // fresh transition lane.
            claim_next_transition_update_lane()
        });
    }
    CURRENT_EVENT_TRANSITION_LANE.get()
}

pub fn did_current_event_schedule_transition() -> bool {
    CURRENT_EVENT_TRANSITION_LANE.get() != NO_LANE
}

pub fn mark_indicator_handled(root: &FiberRoot) {
    if ENABLE_DEFAULT_TRANSITION_INDICATOR.with(|&enabled| enabled) {
        // The current transition event rendered a synchronous loading state.
        // Clear it from the indicator lanes. We don't need to show a separate
        // loading state for this lane.
        root.indicator_lanes
            .set(root.indicator_lanes.get() & !CURRENT_EVENT_TRANSITION_LANE.get());
        mark_isomorphic_indicator_handled();
    }
}
