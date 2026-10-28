// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::panic::{self, AssertUnwindSafe};
use std::rc::Rc;

use js::{ArrayBuffer, IntoUnknown, Rest, Unknown, function};
use webapi::console;

use crate::scheduler_feature_flags::ENABLE_PROFILING;
use crate::scheduler_min_heap::{peek, pop, push};
use crate::{Callback, Options, Task};

// TODO: Use symbols?
use crate::scheduler_priorities::{
    IDLE_PRIORITY, IMMEDIATE_PRIORITY, LOW_PRIORITY, NORMAL_PRIORITY, PriorityLevel,
    USER_BLOCKING_PRIORITY,
};

use crate::scheduler_profiling::{
    mark_scheduler_suspended, mark_scheduler_unsuspended, mark_task_canceled, mark_task_completed,
    mark_task_errored, mark_task_run, mark_task_start, mark_task_yield,
    start_logging_profiling_events, stop_logging_profiling_events,
};

/// `console.log`, which a test may patch.
type ConsoleLog = fn(&'static dyn Rest<dyn IntoUnknown>);

/// What the host is asked to call: given whether there's time left and the
/// time it starts, it works, and says whether there's more.
type HostCallback = fn(bool, f64) -> bool;

// Max 31 bit integer. The max integer size in V8 for 32-bit systems.
// Math.pow(2, 30) - 1
// 0b111111111111111111111111111111
const MAX_SIGNED_31_BIT_INT: f64 = 1073741823.0;

// Times out immediately
const IMMEDIATE_PRIORITY_TIMEOUT: f64 = -1.0;
// Eventually times out
const USER_BLOCKING_PRIORITY_TIMEOUT: f64 = 250.0;
const NORMAL_PRIORITY_TIMEOUT: f64 = 5000.0;
const LOW_PRIORITY_TIMEOUT: f64 = 10000.0;
// Never times out
const IDLE_PRIORITY_TIMEOUT: f64 = MAX_SIGNED_31_BIT_INT;

thread_local! {
    // Tasks are stored on a min heap
    static TASK_QUEUE: RefCell<Vec<Rc<Task>>> = const { RefCell::new(Vec::new()) };
    static TIMER_QUEUE: RefCell<Vec<Rc<Task>>> = const { RefCell::new(Vec::new()) };

    // Incrementing id counter. Used to maintain insertion order.
    static TASK_ID_COUNTER: Cell<f64> = const { Cell::new(1.0) };

    static CURRENT_TASK: RefCell<Option<Rc<Task>>> = const { RefCell::new(None) };
    static CURRENT_PRIORITY_LEVEL: Cell<PriorityLevel> = const { Cell::new(NORMAL_PRIORITY) };

    // This is set while performing work, to prevent re-entrance.
    static IS_PERFORMING_WORK: Cell<bool> = const { Cell::new(false) };

    static IS_HOST_CALLBACK_SCHEDULED: Cell<bool> = const { Cell::new(false) };
    static IS_HOST_TIMEOUT_SCHEDULED: Cell<bool> = const { Cell::new(false) };

    static CURRENT_MOCK_TIME: Cell<f64> = const { Cell::new(0.0) };
    static SCHEDULED_CALLBACK: Cell<Option<HostCallback>> = const { Cell::new(None) };
    static SCHEDULED_TIMEOUT: Cell<Option<fn(f64)>> = const { Cell::new(None) };
    static TIMEOUT_TIME: Cell<f64> = const { Cell::new(-1.0) };
    static YIELDED_VALUES: RefCell<Option<Vec<Option<&'static Unknown>>>> = const { RefCell::new(None) };
    static EXPECTED_NUMBER_OF_YIELDS: Cell<f64> = const { Cell::new(-1.0) };
    static DID_STOP: Cell<bool> = const { Cell::new(false) };
    static IS_FLUSHING: Cell<bool> = const { Cell::new(false) };
    static NEEDS_PAINT: Cell<bool> = const { Cell::new(false) };
    static SHOULD_YIELD_FOR_PAINT: Cell<bool> = const { Cell::new(false) };

    static DISABLE_YIELD_VALUE: Cell<bool> = const { Cell::new(false) };
}

#[cfg_attr(rust_js, rust_js::name = "unstable_setDisableYieldValue")]
pub fn set_disable_yield_value(new_value: bool) {
    DISABLE_YIELD_VALUE.set(new_value);
}

fn advance_timers(current_time: f64) {
    // Check for tasks that are no longer delayed and add them to the queue.
    TIMER_QUEUE.with_borrow_mut(|timer_queue| {
        while let Some(timer) = peek(timer_queue) {
            if timer.callback.borrow().is_none() {
                // Timer was cancelled.
                pop(timer_queue);
            } else if timer.start_time <= current_time {
                // Timer fired. Transfer to the task queue.
                let timer = Rc::clone(timer);
                pop(timer_queue);
                timer.sort_index.set(timer.expiration_time);
                TASK_QUEUE.with_borrow_mut(|task_queue| push(task_queue, Rc::clone(&timer)));
                if ENABLE_PROFILING {
                    mark_task_start(&timer, current_time);
                    timer.is_queued.set(Some(true));
                }
            } else {
                // Remaining timers are pending.
                return;
            }
        }
    });
}

fn handle_timeout(current_time: f64) {
    IS_HOST_TIMEOUT_SCHEDULED.set(false);
    advance_timers(current_time);

    if !IS_HOST_CALLBACK_SCHEDULED.get() {
        if TASK_QUEUE.with_borrow(|task_queue| peek(task_queue).is_some()) {
            IS_HOST_CALLBACK_SCHEDULED.set(true);
            request_host_callback(flush_work);
        } else if let Some(start_time) =
            TIMER_QUEUE.with_borrow(|timer_queue| peek(timer_queue).map(|t| t.start_time))
        {
            request_host_timeout(handle_timeout, start_time - current_time);
        }
    }
}

/// What `flush_work` and its `work_loop` leave as they were, however they're
/// left.
struct Restore {
    previous_priority_level: PriorityLevel,
}

impl Drop for Restore {
    fn drop(&mut self) {
        CURRENT_TASK.set(None);
        CURRENT_PRIORITY_LEVEL.set(self.previous_priority_level);
        IS_PERFORMING_WORK.set(false);
        if ENABLE_PROFILING {
            let current_time = get_current_time();
            mark_scheduler_suspended(current_time);
        }
    }
}

fn flush_work(has_time_remaining: bool, initial_time: f64) -> bool {
    if ENABLE_PROFILING {
        mark_scheduler_unsuspended(initial_time);
    }

    // We'll need a host callback the next time work is scheduled.
    IS_HOST_CALLBACK_SCHEDULED.set(false);
    if IS_HOST_TIMEOUT_SCHEDULED.get() {
        // We scheduled a timeout but it's no longer needed. Cancel it.
        IS_HOST_TIMEOUT_SCHEDULED.set(false);
        cancel_host_timeout();
    }

    IS_PERFORMING_WORK.set(true);
    let _restore = Restore {
        previous_priority_level: CURRENT_PRIORITY_LEVEL.get(),
    };
    if ENABLE_PROFILING {
        match panic::catch_unwind(AssertUnwindSafe(|| {
            work_loop(has_time_remaining, initial_time)
        })) {
            Ok(has_more_work) => has_more_work,
            Err(error) => {
                if let Some(current_task) = CURRENT_TASK.with_borrow(Clone::clone) {
                    let current_time = get_current_time();
                    mark_task_errored(&current_task, current_time);
                    current_task.is_queued.set(Some(false));
                }
                panic::resume_unwind(error)
            }
        }
    } else {
        // No catch in prod code path.
        work_loop(has_time_remaining, initial_time)
    }
}

fn work_loop(has_time_remaining: bool, initial_time: f64) -> bool {
    let mut current_time = initial_time;
    advance_timers(current_time);
    CURRENT_TASK.set(TASK_QUEUE.with_borrow(|task_queue| peek(task_queue).cloned()));
    while let Some(current_task) = CURRENT_TASK.with_borrow(Clone::clone) {
        if current_task.expiration_time > current_time
            && (!has_time_remaining || should_yield_to_host())
        {
            // This currentTask hasn't expired, and we've reached the deadline.
            break;
        }
        if js::is_function(&*current_task.callback.borrow())
            && let Some(Callback(mut callback)) = current_task.callback.take()
        {
            CURRENT_PRIORITY_LEVEL.set(current_task.priority_level);
            let did_user_callback_timeout = current_task.expiration_time <= current_time;
            if ENABLE_PROFILING {
                mark_task_run(&current_task, current_time);
            }
            let continuation_callback = callback(did_user_callback_timeout);
            current_time = get_current_time();
            if let Some(continuation_callback) = continuation_callback {
                // If a continuation is returned, immediately yield to the main thread
                // regardless of how much time is left in the current time slice.
                current_task.callback.replace(Some(continuation_callback));
                if ENABLE_PROFILING {
                    mark_task_yield(&current_task, current_time);
                }
                advance_timers(current_time);

                if SHOULD_YIELD_FOR_PAINT.get() {
                    NEEDS_PAINT.set(true);
                    return true;
                } else {
                    // If `shouldYieldForPaint` is false, we keep flushing synchronously
                    // without yielding to the main thread. This is the behavior of the
                    // `toFlushAndYield` and `toFlushAndYieldThrough` testing helpers .
                }
            } else {
                if ENABLE_PROFILING {
                    mark_task_completed(&current_task, current_time);
                    current_task.is_queued.set(Some(false));
                }
                TASK_QUEUE.with_borrow_mut(|task_queue| {
                    if peek(task_queue).is_some_and(|first| Rc::ptr_eq(first, &current_task)) {
                        pop(task_queue);
                    }
                });
                advance_timers(current_time);
            }
        } else {
            TASK_QUEUE.with_borrow_mut(pop);
        }
        CURRENT_TASK.set(TASK_QUEUE.with_borrow(|task_queue| peek(task_queue).cloned()));
    }
    // Return whether there's additional work
    if CURRENT_TASK.with_borrow(Option::is_some) {
        true
    } else {
        if let Some(start_time) =
            TIMER_QUEUE.with_borrow(|timer_queue| peek(timer_queue).map(|t| t.start_time))
        {
            request_host_timeout(handle_timeout, start_time - current_time);
        }
        false
    }
}

/// What sets the current priority level back however the work it was set
/// for is left.
struct RestorePriority(PriorityLevel);

impl Drop for RestorePriority {
    fn drop(&mut self) {
        CURRENT_PRIORITY_LEVEL.set(self.0);
    }
}

#[cfg_attr(rust_js, rust_js::name = "unstable_runWithPriority")]
pub fn unstable_run_with_priority<T>(
    priority_level: PriorityLevel,
    event_handler: impl FnOnce() -> T,
) -> T {
    let priority_level = match priority_level {
        IMMEDIATE_PRIORITY
        | USER_BLOCKING_PRIORITY
        | NORMAL_PRIORITY
        | LOW_PRIORITY
        | IDLE_PRIORITY => priority_level,
        _ => NORMAL_PRIORITY,
    };

    let _restore = RestorePriority(CURRENT_PRIORITY_LEVEL.get());
    CURRENT_PRIORITY_LEVEL.set(priority_level);

    event_handler()
}

#[cfg_attr(rust_js, rust_js::name = "unstable_next")]
pub fn unstable_next<T>(event_handler: impl FnOnce() -> T) -> T {
    let priority_level = match CURRENT_PRIORITY_LEVEL.get() {
        // Shift down to normal priority
        IMMEDIATE_PRIORITY | USER_BLOCKING_PRIORITY | NORMAL_PRIORITY => NORMAL_PRIORITY,
        // Anything lower than normal priority should remain at the current level.
        current => current,
    };

    let _restore = RestorePriority(CURRENT_PRIORITY_LEVEL.get());
    CURRENT_PRIORITY_LEVEL.set(priority_level);

    event_handler()
}

#[cfg_attr(rust_js, rust_js::name = "unstable_wrapCallback")]
pub fn unstable_wrap_callback(
    callback: &'static dyn Fn(&dyn Rest<dyn IntoUnknown>) -> Option<&'static Unknown>,
) -> Box<dyn Fn(&'static dyn Rest<dyn IntoUnknown>) -> Option<&'static Unknown>> {
    let parent_priority_level = CURRENT_PRIORITY_LEVEL.get();
    function::with_this(
        move |this, (args,): (&'static dyn Rest<dyn IntoUnknown>,)| {
            // This is a fork of runWithPriority, inlined for performance.
            let _restore = RestorePriority(CURRENT_PRIORITY_LEVEL.get());
            CURRENT_PRIORITY_LEVEL.set(parent_priority_level);

            function::apply(
                callback,
                this.map(|this| this as &dyn IntoUnknown),
                Some(&args),
            )
        },
    )
}

#[cfg_attr(rust_js, rust_js::name = "unstable_scheduleCallback")]
pub fn unstable_schedule_callback(
    priority_level: PriorityLevel,
    callback: Callback,
    options: Option<&Options>,
) -> Rc<Task> {
    let current_time = get_current_time();

    let start_time = match options.and_then(|options| options.delay) {
        Some(delay) if delay > 0.0 => current_time + delay,
        _ => current_time,
    };

    let timeout = match priority_level {
        IMMEDIATE_PRIORITY => IMMEDIATE_PRIORITY_TIMEOUT,
        USER_BLOCKING_PRIORITY => USER_BLOCKING_PRIORITY_TIMEOUT,
        IDLE_PRIORITY => IDLE_PRIORITY_TIMEOUT,
        LOW_PRIORITY => LOW_PRIORITY_TIMEOUT,
        _ => NORMAL_PRIORITY_TIMEOUT,
    };

    let expiration_time = start_time + timeout;

    let id = TASK_ID_COUNTER.get();
    TASK_ID_COUNTER.set(id + 1.0);
    let new_task = Rc::new(Task {
        id,
        callback: RefCell::new(Some(callback)),
        priority_level,
        start_time,
        expiration_time,
        sort_index: Cell::new(-1.0),
        is_queued: Cell::new(None),
    });
    if ENABLE_PROFILING {
        new_task.is_queued.set(Some(false));
    }

    if start_time > current_time {
        // This is a delayed task.
        new_task.sort_index.set(start_time);
        TIMER_QUEUE.with_borrow_mut(|timer_queue| push(timer_queue, Rc::clone(&new_task)));
        if TASK_QUEUE.with_borrow(|task_queue| peek(task_queue).is_none())
            && TIMER_QUEUE.with_borrow(|timer_queue| {
                peek(timer_queue).is_some_and(|first| Rc::ptr_eq(first, &new_task))
            })
        {
            // All tasks are delayed, and this is the task with the earliest delay.
            if IS_HOST_TIMEOUT_SCHEDULED.get() {
                // Cancel an existing timeout.
                cancel_host_timeout();
            } else {
                IS_HOST_TIMEOUT_SCHEDULED.set(true);
            }
            // Schedule a timeout.
            request_host_timeout(handle_timeout, start_time - current_time);
        }
    } else {
        new_task.sort_index.set(expiration_time);
        TASK_QUEUE.with_borrow_mut(|task_queue| push(task_queue, Rc::clone(&new_task)));
        if ENABLE_PROFILING {
            mark_task_start(&new_task, current_time);
            new_task.is_queued.set(Some(true));
        }
        // Schedule a host callback, if needed. If we're already performing work,
        // wait until the next time we yield.
        if !IS_HOST_CALLBACK_SCHEDULED.get() && !IS_PERFORMING_WORK.get() {
            IS_HOST_CALLBACK_SCHEDULED.set(true);
            request_host_callback(flush_work);
        }
    }

    new_task
}

#[cfg_attr(rust_js, rust_js::name = "unstable_cancelCallback")]
pub fn unstable_cancel_callback(task: &Task) {
    if ENABLE_PROFILING && task.is_queued.get() == Some(true) {
        let current_time = get_current_time();
        mark_task_canceled(task, current_time);
        task.is_queued.set(Some(false));
    }

    // Null out the callback to indicate the task has been canceled. (Can't
    // remove from the queue because you can't remove arbitrary nodes from an
    // array based heap, only the first one.)
    task.callback.replace(None);
}

#[cfg_attr(rust_js, rust_js::name = "unstable_getCurrentPriorityLevel")]
pub fn unstable_get_current_priority_level() -> PriorityLevel {
    CURRENT_PRIORITY_LEVEL.get()
}

fn request_host_callback(callback: HostCallback) {
    SCHEDULED_CALLBACK.set(Some(callback));
}

fn request_host_timeout(callback: fn(f64), ms: f64) {
    SCHEDULED_TIMEOUT.set(Some(callback));
    TIMEOUT_TIME.set(CURRENT_MOCK_TIME.get() + ms);
}

fn cancel_host_timeout() {
    SCHEDULED_TIMEOUT.set(None);
    TIMEOUT_TIME.set(-1.0);
}

pub fn should_yield_to_host() -> bool {
    if (EXPECTED_NUMBER_OF_YIELDS.get() == 0.0 && YIELDED_VALUES.with_borrow(Option::is_none))
        || (EXPECTED_NUMBER_OF_YIELDS.get() != -1.0
            && YIELDED_VALUES.with_borrow(|yielded_values| {
                yielded_values.as_ref().is_some_and(|yielded_values| {
                    yielded_values.len() as f64 >= EXPECTED_NUMBER_OF_YIELDS.get()
                })
            }))
        || (SHOULD_YIELD_FOR_PAINT.get() && NEEDS_PAINT.get())
    {
        // We yielded at least as many values as expected. Stop flushing.
        DID_STOP.set(true);
        return true;
    }
    false
}

pub fn get_current_time() -> f64 {
    CURRENT_MOCK_TIME.get()
}

pub fn force_frame_rate() {
    // No-op
}

pub fn reset() {
    if IS_FLUSHING.get() {
        panic!("Cannot reset while already flushing work.");
    }
    CURRENT_MOCK_TIME.set(0.0);
    SCHEDULED_CALLBACK.set(None);
    SCHEDULED_TIMEOUT.set(None);
    TIMEOUT_TIME.set(-1.0);
    YIELDED_VALUES.set(None);
    EXPECTED_NUMBER_OF_YIELDS.set(-1.0);
    DID_STOP.set(false);
    IS_FLUSHING.set(false);
    NEEDS_PAINT.set(false);
}

/// What a flush through a number of yields leaves as it was, however it's
/// left.
struct StopFlushingYields;

impl Drop for StopFlushingYields {
    fn drop(&mut self) {
        EXPECTED_NUMBER_OF_YIELDS.set(-1.0);
        DID_STOP.set(false);
        IS_FLUSHING.set(false);
    }
}

// Should only be used via an assertion helper that inspects the yielded values.
#[cfg_attr(rust_js, rust_js::name = "unstable_flushNumberOfYields")]
pub fn unstable_flush_number_of_yields(count: f64) {
    if IS_FLUSHING.get() {
        panic!("Already flushing work.");
    }
    if let Some(cb) = SCHEDULED_CALLBACK.get() {
        EXPECTED_NUMBER_OF_YIELDS.set(count);
        IS_FLUSHING.set(true);
        let _stop = StopFlushingYields;
        let mut has_more_work;
        loop {
            has_more_work = cb(true, CURRENT_MOCK_TIME.get());
            if !has_more_work || DID_STOP.get() {
                break;
            }
        }
        if !has_more_work {
            SCHEDULED_CALLBACK.set(None);
        }
    }
}

/// What a flush until the next paint leaves as it was, however it's left.
struct StopFlushingUntilPaint;

impl Drop for StopFlushingUntilPaint {
    fn drop(&mut self) {
        SHOULD_YIELD_FOR_PAINT.set(false);
        DID_STOP.set(false);
        IS_FLUSHING.set(false);
    }
}

#[cfg_attr(rust_js, rust_js::name = "unstable_flushUntilNextPaint")]
pub fn unstable_flush_until_next_paint() -> bool {
    if IS_FLUSHING.get() {
        panic!("Already flushing work.");
    }
    if let Some(cb) = SCHEDULED_CALLBACK.get() {
        SHOULD_YIELD_FOR_PAINT.set(true);
        NEEDS_PAINT.set(false);
        IS_FLUSHING.set(true);
        let _stop = StopFlushingUntilPaint;
        let mut has_more_work;
        loop {
            has_more_work = cb(true, CURRENT_MOCK_TIME.get());
            if !has_more_work || DID_STOP.get() {
                break;
            }
        }
        if !has_more_work {
            SCHEDULED_CALLBACK.set(None);
        }
    }
    false
}

#[cfg_attr(rust_js, rust_js::name = "unstable_hasPendingWork")]
pub fn unstable_has_pending_work() -> bool {
    SCHEDULED_CALLBACK.get().is_some()
}

/// What a flush leaves as it was, however it's left.
struct StopFlushing;

impl Drop for StopFlushing {
    fn drop(&mut self) {
        IS_FLUSHING.set(false);
    }
}

#[cfg_attr(rust_js, rust_js::name = "unstable_flushExpired")]
pub fn unstable_flush_expired() {
    if IS_FLUSHING.get() {
        panic!("Already flushing work.");
    }
    if let Some(scheduled_callback) = SCHEDULED_CALLBACK.get() {
        IS_FLUSHING.set(true);
        let _stop = StopFlushing;
        let has_more_work = scheduled_callback(false, CURRENT_MOCK_TIME.get());
        if !has_more_work {
            SCHEDULED_CALLBACK.set(None);
        }
    }
}

#[cfg_attr(rust_js, rust_js::name = "unstable_flushAllWithoutAsserting")]
pub fn unstable_flush_all_without_asserting() -> bool {
    // Returns false if no work was flushed.
    if IS_FLUSHING.get() {
        panic!("Already flushing work.");
    }
    if let Some(cb) = SCHEDULED_CALLBACK.get() {
        IS_FLUSHING.set(true);
        let _stop = StopFlushing;
        let mut has_more_work;
        loop {
            has_more_work = cb(true, CURRENT_MOCK_TIME.get());
            if !has_more_work {
                break;
            }
        }
        if !has_more_work {
            SCHEDULED_CALLBACK.set(None);
        }
        true
    } else {
        false
    }
}

#[cfg_attr(rust_js, rust_js::name = "unstable_clearLog")]
pub fn unstable_clear_log() -> Vec<Option<&'static Unknown>> {
    YIELDED_VALUES.take().unwrap_or_default()
}

#[cfg_attr(rust_js, rust_js::name = "unstable_flushAll")]
pub fn unstable_flush_all() {
    if YIELDED_VALUES.with_borrow(Option::is_some) {
        panic!(concat!(
            "Log is not empty. Assert on the log of yielded values before ",
            "flushing additional work."
        ));
    }
    unstable_flush_all_without_asserting();
    if YIELDED_VALUES.with_borrow(Option::is_some) {
        panic!(concat!(
            "While flushing work, something yielded a value. Use an ",
            "assertion helper to assert on the log of yielded values, e.g. ",
            "expect(Scheduler).toFlushAndYield([...])"
        ));
    }
}

pub fn log(value: Option<&'static Unknown>) {
    // eslint-disable-next-line react-internal/no-production-logging
    if js::function::name(&(console::log as ConsoleLog)) == "disabledLog"
        || DISABLE_YIELD_VALUE.get()
    {
        // If console.log has been patched, we assume we're in render
        // replaying and we ignore any values yielding in the second pass.
        return;
    }
    YIELDED_VALUES.with_borrow_mut(|yielded_values| match yielded_values {
        None => *yielded_values = Some(vec![value]),
        Some(yielded_values) => yielded_values.push(value),
    });
}

#[cfg_attr(rust_js, rust_js::name = "unstable_advanceTime")]
pub fn unstable_advance_time(ms: f64) {
    // eslint-disable-next-line react-internal/no-production-logging
    if js::function::name(&(console::log as ConsoleLog)) == "disabledLog"
        || DISABLE_YIELD_VALUE.get()
    {
        // If console.log has been patched, we assume we're in render
        // replaying and we ignore any time advancing in the second pass.
        return;
    }
    CURRENT_MOCK_TIME.set(CURRENT_MOCK_TIME.get() + ms);
    if let Some(scheduled_timeout) = SCHEDULED_TIMEOUT.get()
        && TIMEOUT_TIME.get() <= CURRENT_MOCK_TIME.get()
    {
        scheduled_timeout(CURRENT_MOCK_TIME.get());
        TIMEOUT_TIME.set(-1.0);
        SCHEDULED_TIMEOUT.set(None);
    }
}

pub fn request_paint() {
    NEEDS_PAINT.set(true);
}

#[cfg_attr(rust_js, rust_js::name = "unstable_forceFrameRate")]
pub use self::force_frame_rate as unstable_force_frame_rate;
#[cfg_attr(rust_js, rust_js::name = "unstable_now")]
pub use self::get_current_time as unstable_now;
#[cfg_attr(rust_js, rust_js::name = "unstable_requestPaint")]
pub use self::request_paint as unstable_request_paint;
#[cfg_attr(rust_js, rust_js::name = "unstable_shouldYield")]
pub use self::should_yield_to_host as unstable_should_yield;
#[cfg_attr(rust_js, rust_js::name = "unstable_IdlePriority")]
pub use crate::scheduler_priorities::IDLE_PRIORITY as UNSTABLE_IDLE_PRIORITY;
#[cfg_attr(rust_js, rust_js::name = "unstable_ImmediatePriority")]
pub use crate::scheduler_priorities::IMMEDIATE_PRIORITY as UNSTABLE_IMMEDIATE_PRIORITY;
#[cfg_attr(rust_js, rust_js::name = "unstable_LowPriority")]
pub use crate::scheduler_priorities::LOW_PRIORITY as UNSTABLE_LOW_PRIORITY;
#[cfg_attr(rust_js, rust_js::name = "unstable_NormalPriority")]
pub use crate::scheduler_priorities::NORMAL_PRIORITY as UNSTABLE_NORMAL_PRIORITY;
#[cfg_attr(rust_js, rust_js::name = "unstable_UserBlockingPriority")]
pub use crate::scheduler_priorities::USER_BLOCKING_PRIORITY as UNSTABLE_USER_BLOCKING_PRIORITY;

/// What starts and stops logging the scheduler's profiling events.
pub struct Profiling {
    pub start_logging_profiling_events: fn(),
    pub stop_logging_profiling_events: fn() -> Option<&'static ArrayBuffer>,
}

#[cfg_attr(rust_js, rust_js::name = "unstable_Profiling")]
#[cfg_attr(rust_js, rust_js::nullable)]
pub const UNSTABLE_PROFILING: Option<Profiling> = if ENABLE_PROFILING {
    Some(Profiling {
        start_logging_profiling_events,
        stop_logging_profiling_events,
    })
} else {
    None
};
