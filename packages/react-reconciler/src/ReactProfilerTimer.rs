// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use shared::environment::__DEV__;
use shared::react_feature_flags::{
    ENABLE_COMPONENT_PERFORMANCE_TRACK, ENABLE_PROFILER_COMMIT_HOOKS,
    ENABLE_PROFILER_NESTED_UPDATE_PHASE, ENABLE_PROFILER_TIMER,
};
use webapi::{ConsoleTask, console};

use crate::get_component_name_from_fiber::get_component_name_from_fiber;
use crate::react_captured_value::CapturedValue;
use crate::react_fiber_lane::{
    Lane, Lanes, NO_LANES, includes_blocking_lane, includes_transition_lane, is_blocking_lane,
    is_gesture_render, is_transition_lane,
};
use crate::react_internal_types::Fiber;

#[cfg_attr(rust_js, rust_js::link_name = "./src/ReactFiberConfig.js#resolveEventType")]
#[cfg_attr(rust_js, rust_js::nullable)]
fn resolve_event_type() -> Option<String> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#resolveEventTimeStamp"
)]
fn resolve_event_time_stamp() -> f64 {
    unreachable!()
}

#[cfg_attr(rust_js, rust_js::link_name = "./src/ReactFiberWorkLoop.js#isAlreadyRendering")]
fn is_already_rendering() -> bool {
    unreachable!()
}

// Intentionally not named imports because Rollup would use dynamic dispatch for
// CommonJS interop named imports.
#[cfg_attr(rust_js, rust_js::link_name = "scheduler#*.unstable_now")]
fn unstable_now() -> f64 {
    unreachable!()
}

/// ReactFiberWorkLoop's `SuspendedReason`: why the work loop yielded.
pub type SuspendedReason = u8;

type CaptureErrors = Vec<Rc<CapturedValue<Option<&'static js::Unknown>>>>;

/// What `console.createTask` is: given a name, the task DevTools shows the
/// stack of where it was made, or `None` where it makes none.
type CreateTask = dyn Fn(&str) -> Option<&'static ConsoleTask>;

#[cfg_attr(rust_js, rust_js::nullable)]
fn no_task(_name: &str) -> Option<&'static ConsoleTask> {
    None
}

thread_local! {
    static NOW: fn() -> f64 = unstable_now;

    static CREATE_TASK: Box<CreateTask> =
        // eslint-disable-next-line react-internal/no-production-logging
        if __DEV__ && let Some(create_task) = console::create_task() {
            // eslint-disable-next-line react-internal/no-production-logging
            Box::new(move |name| Some(create_task(name)))
        } else {
            Box::new(no_task)
        };
}

pub type UpdateType = u8;

pub const REGULAR_UPDATE: UpdateType = 0;
pub const SPAWNED_UPDATE: UpdateType = 1;
pub const PINGED_UPDATE: UpdateType = 2;

thread_local! {
    pub static RENDER_START_TIME: Cell<f64> = const { Cell::new(-0.0) };
    pub static COMMIT_START_TIME: Cell<f64> = const { Cell::new(-0.0) };
    pub static COMMIT_END_TIME: Cell<f64> = const { Cell::new(-0.0) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static COMMIT_ERRORS: RefCell<Option<CaptureErrors>> = const { RefCell::new(None) };
    pub static PROFILER_START_TIME: Cell<f64> = const { Cell::new(-1.1) };
    pub static PROFILER_EFFECT_DURATION: Cell<f64> = const { Cell::new(-0.0) };
    pub static COMPONENT_EFFECT_DURATION: Cell<f64> = const { Cell::new(-0.0) };
    pub static COMPONENT_EFFECT_START_TIME: Cell<f64> = const { Cell::new(-1.1) };
    pub static COMPONENT_EFFECT_END_TIME: Cell<f64> = const { Cell::new(-1.1) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static COMPONENT_EFFECT_ERRORS: RefCell<Option<CaptureErrors>> = const { RefCell::new(None) };
    pub static COMPONENT_EFFECT_SPAWNED_UPDATE: Cell<bool> = const { Cell::new(false) };

    pub static BLOCKING_CLAMP_TIME: Cell<f64> = const { Cell::new(-0.0) };
    pub static BLOCKING_UPDATE_TIME: Cell<f64> = const { Cell::new(-1.1) }; // First sync setState scheduled.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static BLOCKING_UPDATE_TASK: Cell<Option<&'static ConsoleTask>> = const { Cell::new(None) }; // First sync setState's stack trace.
    pub static BLOCKING_UPDATE_TYPE: Cell<UpdateType> = const { Cell::new(0) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static BLOCKING_UPDATE_METHOD_NAME: RefCell<Option<String>> = const { RefCell::new(None) }; // The name of the method that caused first sync update.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static BLOCKING_UPDATE_COMPONENT_NAME: RefCell<Option<String>> = const { RefCell::new(None) }; // The name of the component where first sync update happened.
    pub static BLOCKING_EVENT_TIME: Cell<f64> = const { Cell::new(-1.1) }; // Event timeStamp of the first setState.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static BLOCKING_EVENT_TYPE: RefCell<Option<String>> = const { RefCell::new(None) }; // Event type of the first setState.
    pub static BLOCKING_EVENT_REPEAT_TIME: Cell<f64> = const { Cell::new(-1.1) };
    pub static BLOCKING_SUSPENDED_TIME: Cell<f64> = const { Cell::new(-1.1) };

    pub static GESTURE_CLAMP_TIME: Cell<f64> = const { Cell::new(-0.0) };
    pub static GESTURE_UPDATE_TIME: Cell<f64> = const { Cell::new(-1.1) }; // First setOptimistic scheduled inside startGestureTransition.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static GESTURE_UPDATE_TASK: Cell<Option<&'static ConsoleTask>> = const { Cell::new(None) }; // First sync setState's stack trace.
    pub static GESTURE_UPDATE_TYPE: Cell<UpdateType> = const { Cell::new(0) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static GESTURE_UPDATE_METHOD_NAME: RefCell<Option<String>> = const { RefCell::new(None) }; // The name of the method that caused first gesture update.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static GESTURE_UPDATE_COMPONENT_NAME: RefCell<Option<String>> = const { RefCell::new(None) }; // The name of the component where first gesture update happened.
    pub static GESTURE_EVENT_TIME: Cell<f64> = const { Cell::new(-1.1) }; // Event timeStamp of the first setState.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static GESTURE_EVENT_TYPE: RefCell<Option<String>> = const { RefCell::new(None) }; // Event type of the first setState.
    pub static GESTURE_EVENT_REPEAT_TIME: Cell<f64> = const { Cell::new(-1.1) };
    pub static GESTURE_SUSPENDED_TIME: Cell<f64> = const { Cell::new(-1.1) };

    // TODO: This should really be one per Transition lane.
    pub static TRANSITION_CLAMP_TIME: Cell<f64> = const { Cell::new(-0.0) };
    pub static TRANSITION_START_TIME: Cell<f64> = const { Cell::new(-1.1) }; // First startTransition call before setState.
    pub static TRANSITION_UPDATE_TIME: Cell<f64> = const { Cell::new(-1.1) }; // First transition setState scheduled.
    pub static TRANSITION_UPDATE_TYPE: Cell<UpdateType> = const { Cell::new(0) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static TRANSITION_UPDATE_TASK: Cell<Option<&'static ConsoleTask>> = const { Cell::new(None) }; // First transition setState's stack trace.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static TRANSITION_UPDATE_METHOD_NAME: RefCell<Option<String>> = const { RefCell::new(None) }; // The name of the method that caused first transition update.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static TRANSITION_UPDATE_COMPONENT_NAME: RefCell<Option<String>> = const { RefCell::new(None) }; // The name of the component where first transition update happened.
    pub static TRANSITION_EVENT_TIME: Cell<f64> = const { Cell::new(-1.1) }; // Event timeStamp of the first transition.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static TRANSITION_EVENT_TYPE: RefCell<Option<String>> = const { RefCell::new(None) }; // Event type of the first transition.
    pub static TRANSITION_EVENT_REPEAT_TIME: Cell<f64> = const { Cell::new(-1.1) };
    pub static TRANSITION_SUSPENDED_TIME: Cell<f64> = const { Cell::new(-1.1) };

    pub static RETRY_CLAMP_TIME: Cell<f64> = const { Cell::new(-0.0) };
    pub static IDLE_CLAMP_TIME: Cell<f64> = const { Cell::new(-0.0) };

    pub static ANIMATING_LANES: Cell<Lanes> = const { Cell::new(NO_LANES) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static ANIMATING_TASK: Cell<Option<&'static ConsoleTask>> = const { Cell::new(None) }; // First ViewTransition applying an Animation.

    pub static YIELD_REASON: Cell<SuspendedReason> = const { Cell::new(0) };
    pub static YIELD_START_TIME: Cell<f64> = const { Cell::new(-1.1) }; // The time when we yielded to the event loop
}

fn now() -> f64 {
    NOW.with(|now| now())
}

fn create_task(name: &str) -> Option<&'static ConsoleTask> {
    CREATE_TASK.with(|create_task| create_task(name))
}

pub fn start_yield_timer(reason: SuspendedReason) {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled) || !ENABLE_COMPONENT_PERFORMANCE_TRACK {
        return;
    }
    YIELD_START_TIME.set(now());
    YIELD_REASON.set(reason);
}

#[cfg_attr(rust_js, rust_js::nullable(fiber))]
pub fn start_update_timer_by_lane(lane: Lane, method: &str, fiber: Option<&Fiber>) {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled) || !ENABLE_COMPONENT_PERFORMANCE_TRACK {
        return;
    }
    if is_gesture_render(lane) {
        if GESTURE_UPDATE_TIME.get() < 0.0 {
            GESTURE_UPDATE_TIME.set(now());
            GESTURE_UPDATE_TASK.set(create_task(method));
            GESTURE_UPDATE_METHOD_NAME.set(Some(method.to_string()));
            if __DEV__ && let Some(fiber) = fiber {
                GESTURE_UPDATE_COMPONENT_NAME.set(get_component_name_from_fiber(fiber));
            }
            let new_event_time = resolve_event_time_stamp();
            let new_event_type = resolve_event_type();
            if new_event_time != GESTURE_EVENT_REPEAT_TIME.get()
                || GESTURE_EVENT_TYPE.with_borrow(|event_type| new_event_type != *event_type)
            {
                GESTURE_EVENT_REPEAT_TIME.set(-1.1);
            }
            GESTURE_EVENT_TIME.set(new_event_time);
            GESTURE_EVENT_TYPE.set(new_event_type);
        }
    } else if is_blocking_lane(lane) {
        if BLOCKING_UPDATE_TIME.get() < 0.0 {
            BLOCKING_UPDATE_TIME.set(now());
            BLOCKING_UPDATE_TASK.set(create_task(method));
            BLOCKING_UPDATE_METHOD_NAME.set(Some(method.to_string()));
            if __DEV__ && let Some(fiber) = fiber {
                BLOCKING_UPDATE_COMPONENT_NAME.set(get_component_name_from_fiber(fiber));
            }
            if is_already_rendering() {
                COMPONENT_EFFECT_SPAWNED_UPDATE.set(true);
                BLOCKING_UPDATE_TYPE.set(SPAWNED_UPDATE);
            }
            let new_event_time = resolve_event_time_stamp();
            let new_event_type = resolve_event_type();
            if new_event_time != BLOCKING_EVENT_REPEAT_TIME.get()
                || BLOCKING_EVENT_TYPE.with_borrow(|event_type| new_event_type != *event_type)
            {
                BLOCKING_EVENT_REPEAT_TIME.set(-1.1);
            } else if new_event_type.is_some() {
                // If this is a second update in the same event, we treat it as a spawned update.
                // This might be a microtask spawned from useEffect, multiple flushSync or
                // a setState in a microtask spawned after the first setState. Regardless it's bad.
                BLOCKING_UPDATE_TYPE.set(SPAWNED_UPDATE);
            }
            BLOCKING_EVENT_TIME.set(new_event_time);
            BLOCKING_EVENT_TYPE.set(new_event_type);
        }
    } else if is_transition_lane(lane) && TRANSITION_UPDATE_TIME.get() < 0.0 {
        TRANSITION_UPDATE_TIME.set(now());
        TRANSITION_UPDATE_TASK.set(create_task(method));
        TRANSITION_UPDATE_METHOD_NAME.set(Some(method.to_string()));
        if __DEV__ && let Some(fiber) = fiber {
            TRANSITION_UPDATE_COMPONENT_NAME.set(get_component_name_from_fiber(fiber));
        }
        if TRANSITION_START_TIME.get() < 0.0 {
            let new_event_time = resolve_event_time_stamp();
            let new_event_type = resolve_event_type();
            if new_event_time != TRANSITION_EVENT_REPEAT_TIME.get()
                || TRANSITION_EVENT_TYPE.with_borrow(|event_type| new_event_type != *event_type)
            {
                TRANSITION_EVENT_REPEAT_TIME.set(-1.1);
            }
            TRANSITION_EVENT_TIME.set(new_event_time);
            TRANSITION_EVENT_TYPE.set(new_event_type);
        }
    }
}

pub fn start_host_action_timer(fiber: &Fiber) {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled) || !ENABLE_COMPONENT_PERFORMANCE_TRACK {
        return;
    }
    // This schedules an update on both the blocking lane for the pending state and on the
    // transition lane for the action update. Using the debug task from the host fiber.
    if BLOCKING_UPDATE_TIME.get() < 0.0 {
        BLOCKING_UPDATE_TIME.set(now());
        BLOCKING_UPDATE_TASK.set(if __DEV__ { fiber.debug_task.get() } else { None });
        if is_already_rendering() {
            BLOCKING_UPDATE_TYPE.set(SPAWNED_UPDATE);
        }
        let new_event_time = resolve_event_time_stamp();
        let new_event_type = resolve_event_type();
        if new_event_time != BLOCKING_EVENT_REPEAT_TIME.get()
            || BLOCKING_EVENT_TYPE.with_borrow(|event_type| new_event_type != *event_type)
        {
            BLOCKING_EVENT_REPEAT_TIME.set(-1.1);
        } else if new_event_type.is_some() {
            // If this is a second update in the same event, we treat it as a spawned update.
            // This might be a microtask spawned from useEffect, multiple flushSync or
            // a setState in a microtask spawned after the first setState. Regardless it's bad.
            BLOCKING_UPDATE_TYPE.set(SPAWNED_UPDATE);
        }
        BLOCKING_EVENT_TIME.set(new_event_time);
        BLOCKING_EVENT_TYPE.set(new_event_type);
    }
    if TRANSITION_UPDATE_TIME.get() < 0.0 {
        TRANSITION_UPDATE_TIME.set(now());
        TRANSITION_UPDATE_TASK.set(if __DEV__ { fiber.debug_task.get() } else { None });
        if TRANSITION_START_TIME.get() < 0.0 {
            let new_event_time = resolve_event_time_stamp();
            let new_event_type = resolve_event_type();
            if new_event_time != TRANSITION_EVENT_REPEAT_TIME.get()
                || TRANSITION_EVENT_TYPE.with_borrow(|event_type| new_event_type != *event_type)
            {
                TRANSITION_EVENT_REPEAT_TIME.set(-1.1);
            }
            TRANSITION_EVENT_TIME.set(new_event_time);
            TRANSITION_EVENT_TYPE.set(new_event_type);
        }
    }
}

pub fn start_ping_timer_by_lanes(lanes: Lanes) {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled) || !ENABLE_COMPONENT_PERFORMANCE_TRACK {
        return;
    }
    // Mark the update time and clamp anything before it because we don't want
    // to show the event time for pings but we also don't want to clear it
    // because we still need to track if this was a repeat.
    if is_gesture_render(lanes) {
        if GESTURE_UPDATE_TIME.get() < 0.0 {
            GESTURE_UPDATE_TIME.set(now());
            GESTURE_CLAMP_TIME.set(GESTURE_UPDATE_TIME.get());
            GESTURE_UPDATE_TASK.set(create_task("Promise Resolved"));
            GESTURE_UPDATE_TYPE.set(PINGED_UPDATE);
        }
    } else if includes_blocking_lane(lanes) {
        if BLOCKING_UPDATE_TIME.get() < 0.0 {
            BLOCKING_UPDATE_TIME.set(now());
            BLOCKING_CLAMP_TIME.set(BLOCKING_UPDATE_TIME.get());
            BLOCKING_UPDATE_TASK.set(create_task("Promise Resolved"));
            BLOCKING_UPDATE_TYPE.set(PINGED_UPDATE);
        }
    } else if includes_transition_lane(lanes) && TRANSITION_UPDATE_TIME.get() < 0.0 {
        TRANSITION_UPDATE_TIME.set(now());
        TRANSITION_CLAMP_TIME.set(TRANSITION_UPDATE_TIME.get());
        TRANSITION_UPDATE_TASK.set(create_task("Promise Resolved"));
        TRANSITION_UPDATE_TYPE.set(PINGED_UPDATE);
    }
}

pub fn track_suspended_time(lanes: Lanes, render_end_time: f64) {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled) || !ENABLE_COMPONENT_PERFORMANCE_TRACK {
        return;
    }
    if is_gesture_render(lanes) {
        GESTURE_SUSPENDED_TIME.set(render_end_time);
    } else if includes_blocking_lane(lanes) {
        BLOCKING_SUSPENDED_TIME.set(render_end_time);
    } else if includes_transition_lane(lanes) {
        TRANSITION_SUSPENDED_TIME.set(render_end_time);
    }
}

pub fn clear_blocking_timers() {
    BLOCKING_UPDATE_TIME.set(-1.1);
    BLOCKING_UPDATE_TYPE.set(0);
    BLOCKING_UPDATE_METHOD_NAME.set(None);
    BLOCKING_UPDATE_COMPONENT_NAME.set(None);
    BLOCKING_SUSPENDED_TIME.set(-1.1);
    BLOCKING_EVENT_REPEAT_TIME.set(BLOCKING_EVENT_TIME.get());
    BLOCKING_EVENT_TIME.set(-1.1);
    BLOCKING_CLAMP_TIME.set(now());
}

pub fn start_async_transition_timer() {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled) || !ENABLE_COMPONENT_PERFORMANCE_TRACK {
        return;
    }
    if TRANSITION_START_TIME.get() < 0.0 && TRANSITION_UPDATE_TIME.get() < 0.0 {
        TRANSITION_START_TIME.set(now());
        let new_event_time = resolve_event_time_stamp();
        let new_event_type = resolve_event_type();
        if new_event_time != TRANSITION_EVENT_REPEAT_TIME.get()
            || TRANSITION_EVENT_TYPE.with_borrow(|event_type| new_event_type != *event_type)
        {
            TRANSITION_EVENT_REPEAT_TIME.set(-1.1);
        }
        TRANSITION_EVENT_TIME.set(new_event_time);
        TRANSITION_EVENT_TYPE.set(new_event_type);
    }
}

pub fn has_scheduled_transition_work() -> bool {
    // If we have setState on a transition or scheduled useActionState update.
    TRANSITION_UPDATE_TIME.get() > -1.0
}

pub fn clear_async_transition_timer() {
    TRANSITION_START_TIME.set(-1.1);
}

pub fn clear_transition_timers() {
    TRANSITION_START_TIME.set(-1.1);
    TRANSITION_UPDATE_TIME.set(-1.1);
    TRANSITION_UPDATE_TYPE.set(0);
    TRANSITION_SUSPENDED_TIME.set(-1.1);
    TRANSITION_EVENT_REPEAT_TIME.set(TRANSITION_EVENT_TIME.get());
    TRANSITION_EVENT_TIME.set(-1.1);
    TRANSITION_CLAMP_TIME.set(now());
}

pub fn has_scheduled_gesture_transition_work() -> bool {
    // If we have call setOptimistic on a gesture
    GESTURE_UPDATE_TIME.get() > -1.0
}

pub fn clear_gesture_timers() {
    GESTURE_UPDATE_TIME.set(-1.1);
    GESTURE_UPDATE_TYPE.set(0);
    GESTURE_SUSPENDED_TIME.set(-1.1);
    GESTURE_EVENT_REPEAT_TIME.set(GESTURE_EVENT_TIME.get());
    GESTURE_EVENT_TIME.set(-1.1);
    GESTURE_CLAMP_TIME.set(now());
}

pub fn clear_gesture_updates() {
    // Same as clearGestureTimers but doesn't reset the clamp time because we didn't
    // actually emit a render.
    GESTURE_UPDATE_TIME.set(-1.1);
    GESTURE_UPDATE_TYPE.set(0);
    GESTURE_SUSPENDED_TIME.set(-1.1);
    GESTURE_EVENT_REPEAT_TIME.set(GESTURE_EVENT_TIME.get());
    GESTURE_EVENT_TIME.set(-1.1);
}

pub fn clamp_blocking_timers(final_time: f64) {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled) || !ENABLE_COMPONENT_PERFORMANCE_TRACK {
        return;
    }
    // If we had new updates come in while we were still rendering or committing, we don't want
    // those update times to create overlapping tracks in the performance timeline so we clamp
    // them to the end of the commit phase.
    BLOCKING_CLAMP_TIME.set(final_time);
}

pub fn clamp_gesture_timers(final_time: f64) {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled) || !ENABLE_COMPONENT_PERFORMANCE_TRACK {
        return;
    }
    // If we had new updates come in while we were still rendering or committing, we don't want
    // those update times to create overlapping tracks in the performance timeline so we clamp
    // them to the end of the commit phase.
    GESTURE_CLAMP_TIME.set(final_time);
}

pub fn clamp_transition_timers(final_time: f64) {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled) || !ENABLE_COMPONENT_PERFORMANCE_TRACK {
        return;
    }
    // If we had new updates come in while we were still rendering or committing, we don't want
    // those update times to create overlapping tracks in the performance timeline so we clamp
    // them to the end of the commit phase.
    TRANSITION_CLAMP_TIME.set(final_time);
}

pub fn clamp_retry_timers(final_time: f64) {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled) || !ENABLE_COMPONENT_PERFORMANCE_TRACK {
        return;
    }
    RETRY_CLAMP_TIME.set(final_time);
}

pub fn clamp_idle_timers(final_time: f64) {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled) || !ENABLE_COMPONENT_PERFORMANCE_TRACK {
        return;
    }
    IDLE_CLAMP_TIME.set(final_time);
}

pub fn push_nested_effect_durations() -> f64 {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        || !ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
    {
        return 0.0;
    }
    let prev_effect_duration = PROFILER_EFFECT_DURATION.get();
    PROFILER_EFFECT_DURATION.set(0.0); // Reset counter.
    prev_effect_duration
}

pub fn pop_nested_effect_durations(prev_effect_duration: f64) -> f64 {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        || !ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
    {
        return 0.0;
    }
    let elapsed_time = PROFILER_EFFECT_DURATION.get();
    PROFILER_EFFECT_DURATION.set(prev_effect_duration);
    elapsed_time
}

// Like pop but it also adds the current elapsed time to the parent scope.
pub fn bubble_nested_effect_durations(prev_effect_duration: f64) -> f64 {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        || !ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
    {
        return 0.0;
    }
    let elapsed_time = PROFILER_EFFECT_DURATION.get();
    PROFILER_EFFECT_DURATION.set(PROFILER_EFFECT_DURATION.get() + prev_effect_duration);
    elapsed_time
}

pub fn reset_component_effect_timers() {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        || !ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
    {
        return;
    }
    COMPONENT_EFFECT_START_TIME.set(-1.1);
    COMPONENT_EFFECT_END_TIME.set(-1.1);
}

pub fn push_component_effect_start() -> f64 {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        || !ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
    {
        return 0.0;
    }
    let prev_effect_start = COMPONENT_EFFECT_START_TIME.get();
    COMPONENT_EFFECT_START_TIME.set(-1.1); // Track the next start.
    prev_effect_start
}

pub fn pop_component_effect_start(prev_effect_start: f64) {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        || !ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
    {
        return;
    }
    // If the parent component didn't have a start time, we let this current time persist.
    if prev_effect_start >= 0.0 {
        // Otherwise, we restore the previous parent's start time.
        COMPONENT_EFFECT_START_TIME.set(prev_effect_start);
    }
}

pub fn push_component_effect_duration() -> f64 {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        || !ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
    {
        return 0.0;
    }
    let prev_effect_duration = COMPONENT_EFFECT_DURATION.get();
    COMPONENT_EFFECT_DURATION.set(-0.0); // Reset component level duration.
    prev_effect_duration
}

pub fn pop_component_effect_duration(prev_effect_duration: f64) {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        || !ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
    {
        return;
    }
    // If the parent component didn't have a start time, we let this current time persist.
    if prev_effect_duration >= 0.0 {
        // Otherwise, we restore the previous parent's start time.
        COMPONENT_EFFECT_DURATION.set(prev_effect_duration);
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn push_component_effect_errors() -> Option<CaptureErrors> {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        || !ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
    {
        return None;
    }
    COMPONENT_EFFECT_ERRORS.take()
}

#[cfg_attr(rust_js, rust_js::nullable(prev_errors))]
pub fn pop_component_effect_errors(prev_errors: Option<CaptureErrors>) {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        || !ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
    {
        return;
    }
    COMPONENT_EFFECT_ERRORS.set(prev_errors);
}

pub fn push_component_effect_did_spawn_update() -> bool {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        || !ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
    {
        return false;
    }

    let prev = COMPONENT_EFFECT_SPAWNED_UPDATE.get();
    COMPONENT_EFFECT_SPAWNED_UPDATE.set(false); // Reset.
    prev
}

pub fn pop_component_effect_did_spawn_update(previous_value: bool) {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        || !ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
    {
        return;
    }

    COMPONENT_EFFECT_SPAWNED_UPDATE.set(previous_value);
}

thread_local! {
    /**
     * Tracks whether the current update was a nested/cascading update (scheduled from a layout effect).
     *
     * The overall sequence is:
     *   1. render
     *   2. commit (and call `onRender`, `onCommit`)
     *   3. check for nested updates
     *   4. flush passive effects (and call `onPostCommit`)
     *
     * Nested updates are identified in step 3 above,
     * but step 4 still applies to the work that was just committed.
     * We use two flags to track nested updates then:
     * one tracks whether the upcoming update is a nested update,
     * and the other tracks whether the current update was a nested update.
     * The first value gets synced to the second at the start of the render phase.
     */
    static CURRENT_UPDATE_IS_NESTED: Cell<bool> = const { Cell::new(false) };
    static NESTED_UPDATE_SCHEDULED: Cell<bool> = const { Cell::new(false) };
}

pub fn is_current_update_nested() -> bool {
    CURRENT_UPDATE_IS_NESTED.get()
}

pub fn mark_nested_update_scheduled() {
    if ENABLE_PROFILER_NESTED_UPDATE_PHASE.with(|&enabled| enabled) {
        NESTED_UPDATE_SCHEDULED.set(true);
    }
}

pub fn reset_nested_update_flag() {
    if ENABLE_PROFILER_NESTED_UPDATE_PHASE.with(|&enabled| enabled) {
        CURRENT_UPDATE_IS_NESTED.set(false);
        NESTED_UPDATE_SCHEDULED.set(false);
    }
}

pub fn sync_nested_update_flag() {
    if ENABLE_PROFILER_NESTED_UPDATE_PHASE.with(|&enabled| enabled) {
        CURRENT_UPDATE_IS_NESTED.set(NESTED_UPDATE_SCHEDULED.get());
        NESTED_UPDATE_SCHEDULED.set(false);
    }
}

pub fn record_render_time() {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled) || !ENABLE_COMPONENT_PERFORMANCE_TRACK {
        return;
    }
    RENDER_START_TIME.set(now());
}

pub fn record_commit_time() {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
        return;
    }
    COMMIT_START_TIME.set(now());
}

pub fn record_commit_end_time() {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
        return;
    }
    COMMIT_END_TIME.set(now());
}

pub fn start_profiler_timer(fiber: &Fiber) {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
        return;
    }

    PROFILER_START_TIME.set(now());

    if fiber
        .actual_start_time
        .get()
        .is_some_and(|actual_start_time| actual_start_time < 0.0)
    {
        fiber.actual_start_time.set(Some(PROFILER_START_TIME.get()));
    }
}

pub fn stop_profiler_timer_if_running(_fiber: &Fiber) {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
        return;
    }
    PROFILER_START_TIME.set(-1.0);
}

pub fn stop_profiler_timer_if_running_and_record_duration(fiber: &Fiber) {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
        return;
    }

    if PROFILER_START_TIME.get() >= 0.0 {
        let elapsed_time = now() - PROFILER_START_TIME.get();
        fiber.actual_duration.set(
            fiber
                .actual_duration
                .get()
                .map(|actual_duration| actual_duration + elapsed_time),
        );
        fiber.self_base_duration.set(Some(elapsed_time));
        PROFILER_START_TIME.set(-1.0);
    }
}

pub fn stop_profiler_timer_if_running_and_record_incomplete_duration(fiber: &Fiber) {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
        return;
    }

    if PROFILER_START_TIME.get() >= 0.0 {
        let elapsed_time = now() - PROFILER_START_TIME.get();
        fiber.actual_duration.set(
            fiber
                .actual_duration
                .get()
                .map(|actual_duration| actual_duration + elapsed_time),
        );
        // We don't update the selfBaseDuration here because we errored.
        PROFILER_START_TIME.set(-1.0);
    }
}

pub fn record_effect_duration(_fiber: &Fiber) {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        || !ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
    {
        return;
    }

    if PROFILER_START_TIME.get() >= 0.0 {
        let end_time = now();
        let elapsed_time = end_time - PROFILER_START_TIME.get();

        PROFILER_START_TIME.set(-1.0);

        // Store duration on the next nearest Profiler ancestor
        // Or the root (for the DevTools Profiler to read)
        PROFILER_EFFECT_DURATION.set(PROFILER_EFFECT_DURATION.get() + elapsed_time);
        COMPONENT_EFFECT_DURATION.set(COMPONENT_EFFECT_DURATION.get() + elapsed_time);

        // Keep track of the last end time of the effects.
        COMPONENT_EFFECT_END_TIME.set(end_time);
    }
}

pub fn record_effect_error(error_info: &Rc<CapturedValue<Option<&'static js::Unknown>>>) {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        || !ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
    {
        return;
    }
    COMPONENT_EFFECT_ERRORS.with_borrow_mut(|component_effect_errors| {
        component_effect_errors
            .get_or_insert_with(Vec::new)
            .push(Rc::clone(error_info));
    });
    COMMIT_ERRORS.with_borrow_mut(|commit_errors| {
        commit_errors
            .get_or_insert_with(Vec::new)
            .push(Rc::clone(error_info));
    });
}

pub fn reset_commit_errors() {
    COMMIT_ERRORS.set(None);
}

pub fn start_effect_timer() {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        || !ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
    {
        return;
    }
    PROFILER_START_TIME.set(now());
    if COMPONENT_EFFECT_START_TIME.get() < 0.0 {
        // Keep track of the first time we start an effect as the component's effect start time.
        COMPONENT_EFFECT_START_TIME.set(PROFILER_START_TIME.get());
    }
}

pub fn transfer_actual_duration(fiber: &Fiber) {
    // Transfer time spent rendering these children so we don't lose it
    // after we rerender. This is used as a helper in special cases
    // where we should count the work of multiple passes.
    let mut child = fiber.child.borrow().clone();
    while let Some(current) = child {
        if let Some(actual_duration) = fiber.actual_duration.get()
            && let Some(child_duration) = current.actual_duration.get()
        {
            fiber
                .actual_duration
                .set(Some(actual_duration + child_duration));
        }
        child = current.sibling.borrow().clone();
    }
}

pub fn start_animating(lanes: Lanes) {
    ANIMATING_LANES.set(ANIMATING_LANES.get() | lanes);
    ANIMATING_TASK.set(None);
}

pub fn stop_animating(lanes: Lanes) {
    ANIMATING_LANES.set(ANIMATING_LANES.get() & !lanes);
    ANIMATING_TASK.set(None);
}

pub fn track_animating_task(task: &'static ConsoleTask) {
    if ANIMATING_TASK.get().is_none() {
        ANIMATING_TASK.set(Some(task));
    }
}
