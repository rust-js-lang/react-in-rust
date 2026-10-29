// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::panic::{self, AssertUnwindSafe};
use std::rc::Rc;

use js::{ArrayBuffer, IntoUnknown, Rest, Unknown, function};

#[path = "../SchedulerFeatureFlags.rs"]
pub mod scheduler_feature_flags;
#[path = "../SchedulerMinHeap.rs"]
pub mod scheduler_min_heap;
#[path = "SchedulerMock.rs"]
pub mod scheduler_mock;
#[path = "../SchedulerPriorities.rs"]
pub mod scheduler_priorities;
#[path = "../SchedulerProfiling.rs"]
pub mod scheduler_profiling;

use scheduler_feature_flags::{
    ENABLE_ALWAYS_YIELD_SCHEDULER, ENABLE_PROFILING, ENABLE_REQUEST_PAINT, FRAME_YIELD_MS,
    LOW_PRIORITY_TIMEOUT, NORMAL_PRIORITY_TIMEOUT, USER_BLOCKING_PRIORITY_TIMEOUT,
};
use scheduler_min_heap::{Node, peek, pop, push};
use scheduler_priorities::{
    IDLE_PRIORITY, IMMEDIATE_PRIORITY, LOW_PRIORITY, NORMAL_PRIORITY, PriorityLevel,
    USER_BLOCKING_PRIORITY,
};

use scheduler_profiling::{
    mark_scheduler_suspended, mark_scheduler_unsuspended, mark_task_canceled, mark_task_completed,
    mark_task_errored, mark_task_run, mark_task_start, mark_task_yield,
    start_logging_profiling_events, stop_logging_profiling_events,
};

/// A task's work: given whether it timed out, it does some, and gives
/// what's left of it, or nothing where it's done.
#[repr(transparent)]
pub struct Callback(pub Box<dyn FnMut(bool) -> Option<Callback>>);

pub struct Task {
    pub id: f64,
    #[cfg_attr(rust_js, rust_js::nullable)]
    callback: RefCell<Option<Callback>>,
    priority_level: PriorityLevel,
    start_time: f64,
    expiration_time: f64,
    sort_index: Cell<f64>,
    is_queued: Cell<Option<bool>>,
}

impl Node for Rc<Task> {}

/// What `unstable_scheduleCallback` is given: how long to wait first.
pub struct Options {
    pub delay: Option<f64>,
}

thread_local! {
    static GET_CURRENT_TIME: Box<dyn Fn() -> f64> = if js::is_defined(webapi::Performance::now) {
        let local_performance = webapi::performance;
        Box::new(move || local_performance.now())
    } else {
        let local_date_now: fn() -> f64 = js::date::now;
        let initial_time = local_date_now();
        Box::new(move || local_date_now() - initial_time)
    };
}

pub fn get_current_time() -> f64 {
    GET_CURRENT_TIME.with(|get_current_time| get_current_time())
}

// Max 31 bit integer. The max integer size in V8 for 32-bit systems.
// Math.pow(2, 30) - 1
// 0b111111111111111111111111111111
const MAX_SIGNED_31_BIT_INT: f64 = 1073741823.0;

thread_local! {
    // Tasks are stored on a min heap
    static TASK_QUEUE: RefCell<Vec<Rc<Task>>> = const { RefCell::new(Vec::new()) };
    static TIMER_QUEUE: RefCell<Vec<Rc<Task>>> = const { RefCell::new(Vec::new()) };

    // Incrementing id counter. Used to maintain insertion order.
    static TASK_ID_COUNTER: Cell<f64> = const { Cell::new(1.0) };

    #[cfg_attr(rust_js, rust_js::nullable)]
    static CURRENT_TASK: RefCell<Option<Rc<Task>>> = const { RefCell::new(None) };
    static CURRENT_PRIORITY_LEVEL: Cell<PriorityLevel> = const { Cell::new(NORMAL_PRIORITY) };

    // This is set while performing work, to prevent re-entrance.
    static IS_PERFORMING_WORK: Cell<bool> = const { Cell::new(false) };

    static IS_HOST_CALLBACK_SCHEDULED: Cell<bool> = const { Cell::new(false) };
    static IS_HOST_TIMEOUT_SCHEDULED: Cell<bool> = const { Cell::new(false) };

    static NEEDS_PAINT: Cell<bool> = const { Cell::new(false) };
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
            request_host_callback();
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

fn flush_work(initial_time: f64) -> bool {
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
        match panic::catch_unwind(AssertUnwindSafe(|| work_loop(initial_time))) {
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
        work_loop(initial_time)
    }
}

fn work_loop(initial_time: f64) -> bool {
    let mut current_time = initial_time;
    advance_timers(current_time);
    CURRENT_TASK.set(TASK_QUEUE.with_borrow(|task_queue| peek(task_queue).cloned()));
    while let Some(current_task) = CURRENT_TASK.with_borrow(Clone::clone) {
        if !ENABLE_ALWAYS_YIELD_SCHEDULER.with(|&always| always)
            && current_task.expiration_time > current_time
            && should_yield_to_host()
        {
            // This currentTask hasn't expired, and we've reached the deadline.
            break;
        }
        let callback = current_task.callback.take();
        if let Some(Callback(mut callback)) = callback {
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
                return true;
            }
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
        } else {
            TASK_QUEUE.with_borrow_mut(pop);
        }
        CURRENT_TASK.set(TASK_QUEUE.with_borrow(|task_queue| peek(task_queue).cloned()));
        if ENABLE_ALWAYS_YIELD_SCHEDULER.with(|&always| always)
            && CURRENT_TASK
                .with_borrow(|t| t.as_ref().is_none_or(|t| t.expiration_time > current_time))
        {
            // This currentTask hasn't expired we yield to the browser task.
            break;
        }
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
        // Times out immediately
        IMMEDIATE_PRIORITY => -1.0,
        // Eventually times out
        USER_BLOCKING_PRIORITY => USER_BLOCKING_PRIORITY_TIMEOUT,
        // Never times out
        IDLE_PRIORITY => MAX_SIGNED_31_BIT_INT,
        // Eventually times out
        LOW_PRIORITY => LOW_PRIORITY_TIMEOUT,
        // Eventually times out
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
        TIMER_QUEUE.with_borrow_mut(|timer_queue| push(timer_queue, new_task.clone()));
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
        TASK_QUEUE.with_borrow_mut(|task_queue| push(task_queue, new_task.clone()));
        if ENABLE_PROFILING {
            mark_task_start(&new_task, current_time);
            new_task.is_queued.set(Some(true));
        }
        // Schedule a host callback, if needed. If we're already performing work,
        // wait until the next time we yield.
        if !IS_HOST_CALLBACK_SCHEDULED.get() && !IS_PERFORMING_WORK.get() {
            IS_HOST_CALLBACK_SCHEDULED.set(true);
            request_host_callback();
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

pub fn should_yield_to_host() -> bool {
    if !ENABLE_ALWAYS_YIELD_SCHEDULER.with(|&always| always)
        && ENABLE_REQUEST_PAINT
        && NEEDS_PAINT.get()
    {
        // Yield now.
        return true;
    }
    let time_elapsed = get_current_time() - START_TIME.get();
    // The main thread has only been blocked for a really short amount of time;
    // smaller than a single frame. Don't yield yet.
    time_elapsed >= FRAME_INTERVAL.get()
}

#[cfg_attr(rust_js, rust_js::name = "unstable_requestPaint")]
pub fn request_paint() {
    if ENABLE_REQUEST_PAINT {
        NEEDS_PAINT.set(true);
    }
}

thread_local! {
    static IS_MESSAGE_LOOP_RUNNING: Cell<bool> = const { Cell::new(false) };
    // Scheduler periodically yields in case there is other work on the main
    // thread, like user events. By default, it yields multiple times per frame.
    // It does not attempt to align with frame boundaries, since most tasks don't
    // need to be frame aligned; for those that do, use requestAnimationFrame.
    static FRAME_INTERVAL: Cell<f64> = const { Cell::new(FRAME_YIELD_MS) };
    static START_TIME: Cell<f64> = const { Cell::new(-1.0) };
}

#[cfg_attr(rust_js, rust_js::name = "unstable_forceFrameRate")]
pub fn force_frame_rate(fps: f64) {
    if !(0.0..=125.0).contains(&fps) {
        eprintln!(
            "forceFrameRate takes a positive int between 0 and 125, forcing frame rates higher than 125 fps is not supported"
        );
        return;
    }
    if fps > 0.0 {
        FRAME_INTERVAL.set((1000.0 / fps).floor());
    } else {
        // reset the framerate
        FRAME_INTERVAL.set(FRAME_YIELD_MS);
    }
}

/// What schedules the next slice of work however it's left: where the work
/// it was given threw, there's more of it.
struct ScheduleMore {
    has_more_work: bool,
}

impl Drop for ScheduleMore {
    fn drop(&mut self) {
        if self.has_more_work {
            // If there's more work, schedule the next message event at the end
            // of the preceding one.
            SCHEDULE_PERFORM_WORK_UNTIL_DEADLINE.with(|schedule| schedule());
        } else {
            IS_MESSAGE_LOOP_RUNNING.set(false);
        }
    }
}

fn perform_work_until_deadline() {
    if ENABLE_REQUEST_PAINT {
        NEEDS_PAINT.set(false);
    }
    if IS_MESSAGE_LOOP_RUNNING.get() {
        let current_time = get_current_time();
        // Keep track of the start time so we can measure how long the main thread
        // has been blocked.
        START_TIME.set(current_time);

        // If a scheduler task throws, exit the current browser task so the
        // error can be observed.
        //
        // Intentionally not using a try-catch, since that makes some debugging
        // techniques harder. Instead, if `flushWork` errors, then `hasMoreWork` will
        // remain true, and we'll continue the work loop.
        let mut more = ScheduleMore {
            has_more_work: true,
        };
        more.has_more_work = flush_work(current_time);
    }
}

type SetTimeout = fn(Box<dyn FnOnce()>, Option<f64>) -> &'static js::TimeoutId;
type ClearTimeout = fn(Option<&js::TimeoutId>);
type SetImmediate = fn(Box<dyn FnOnce()>) -> &'static js::Immediate;

thread_local! {
    // Capture local references to native APIs, in case a polyfill overrides them.
    static LOCAL_SET_TIMEOUT: Option<SetTimeout> = js::is_defined(js::set_timeout).then_some(js::set_timeout);
    static LOCAL_CLEAR_TIMEOUT: Option<ClearTimeout> =
        js::is_defined(js::clear_timeout).then_some(js::clear_timeout);
    static LOCAL_SET_IMMEDIATE: Option<SetImmediate> =
        js::is_defined(js::set_immediate).then_some(js::set_immediate); // IE and Node.js + jsdom
}

thread_local! {
    static SCHEDULE_PERFORM_WORK_UNTIL_DEADLINE: Box<dyn Fn()> = if let Some(local_set_immediate) = LOCAL_SET_IMMEDIATE.with(|f| *f) {
        // Node.js and old IE.
        // There's a few reasons for why we prefer setImmediate.
        //
        // Unlike MessageChannel, it doesn't prevent a Node.js process from exiting.
        // (Even though this is a DOM fork of the Scheduler, you could get here
        // with a mix of Node.js 15+, which has a MessageChannel, and jsdom.)
        // https://github.com/react/react/issues/20756
        //
        // But also, it runs earlier which is the semantic we want.
        // If other browsers ever implement it, it's better to use it.
        // Although both of these would be inferior to native scheduling.
        Box::new(move || {
            local_set_immediate(Box::new(perform_work_until_deadline));
        })
    } else if js::is_defined(webapi::message_channel::new) {
        // DOM and Worker environments.
        // We prefer MessageChannel because of the 4ms setTimeout clamping.
        let channel = webapi::message_channel::new();
        let port = channel.port2();
        channel.port1().set_onmessage(Some(&|_| perform_work_until_deadline()));
        Box::new(move || port.post_message(None::<u8>, None))
    } else {
        // We should only fallback here in non-browser environments.
        Box::new(|| {
            if let Some(local_set_timeout) = LOCAL_SET_TIMEOUT.with(|f| *f) {
                local_set_timeout(Box::new(perform_work_until_deadline), Some(0.0));
            }
        })
    };

    static TASK_TIMEOUT_ID: Cell<Option<&'static js::TimeoutId>> = const { Cell::new(None) };
}

fn request_host_callback() {
    if !IS_MESSAGE_LOOP_RUNNING.get() {
        IS_MESSAGE_LOOP_RUNNING.set(true);
        SCHEDULE_PERFORM_WORK_UNTIL_DEADLINE.with(|schedule| schedule());
    }
}

fn request_host_timeout(callback: fn(f64), ms: f64) {
    if let Some(local_set_timeout) = LOCAL_SET_TIMEOUT.with(|f| *f) {
        TASK_TIMEOUT_ID.set(Some(local_set_timeout(
            Box::new(move || callback(get_current_time())),
            Some(ms),
        )));
    }
}

fn cancel_host_timeout() {
    if let Some(local_clear_timeout) = LOCAL_CLEAR_TIMEOUT.with(|f| *f) {
        local_clear_timeout(TASK_TIMEOUT_ID.get());
    }
    TASK_TIMEOUT_ID.set(None);
}

#[cfg_attr(rust_js, rust_js::name = "unstable_now")]
pub use self::get_current_time as unstable_now;
#[cfg_attr(rust_js, rust_js::name = "unstable_shouldYield")]
pub use self::should_yield_to_host as unstable_should_yield;
#[cfg_attr(rust_js, rust_js::name = "unstable_IdlePriority")]
pub use scheduler_priorities::IDLE_PRIORITY as UNSTABLE_IDLE_PRIORITY;
#[cfg_attr(rust_js, rust_js::name = "unstable_ImmediatePriority")]
pub use scheduler_priorities::IMMEDIATE_PRIORITY as UNSTABLE_IMMEDIATE_PRIORITY;
#[cfg_attr(rust_js, rust_js::name = "unstable_LowPriority")]
pub use scheduler_priorities::LOW_PRIORITY as UNSTABLE_LOW_PRIORITY;
#[cfg_attr(rust_js, rust_js::name = "unstable_NormalPriority")]
pub use scheduler_priorities::NORMAL_PRIORITY as UNSTABLE_NORMAL_PRIORITY;
#[cfg_attr(rust_js, rust_js::name = "unstable_UserBlockingPriority")]
pub use scheduler_priorities::USER_BLOCKING_PRIORITY as UNSTABLE_USER_BLOCKING_PRIORITY;

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
