// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;

use js::{ArrayBuffer, Int32Array, array_buffer, int32_array};
use webapi::console;

use crate::Task;
use crate::scheduler_feature_flags::ENABLE_PROFILING;

thread_local! {
    static RUN_ID_COUNTER: Cell<f64> = const { Cell::new(0.0) };
    static MAIN_THREAD_ID_COUNTER: Cell<f64> = const { Cell::new(0.0) };
}

// Bytes per element is 4
const INITIAL_EVENT_LOG_SIZE: f64 = 131072.0;
const MAX_EVENT_LOG_SIZE: f64 = 524288.0; // Equivalent to 2 megabytes

thread_local! {
    static EVENT_LOG_SIZE: Cell<f64> = const { Cell::new(0.0) };
    static EVENT_LOG_BUFFER: Cell<Option<&'static ArrayBuffer>> = const { Cell::new(None) };
    static EVENT_LOG: Cell<Option<&'static Int32Array>> = const { Cell::new(None) };
    static EVENT_LOG_INDEX: Cell<f64> = const { Cell::new(0.0) };
}

#[cfg_attr(rust_js, rust_js::name = "TaskStartEvent")]
const TASK_START_EVENT: f64 = 1.0;
#[cfg_attr(rust_js, rust_js::name = "TaskCompleteEvent")]
const TASK_COMPLETE_EVENT: f64 = 2.0;
#[cfg_attr(rust_js, rust_js::name = "TaskErrorEvent")]
const TASK_ERROR_EVENT: f64 = 3.0;
#[cfg_attr(rust_js, rust_js::name = "TaskCancelEvent")]
const TASK_CANCEL_EVENT: f64 = 4.0;
#[cfg_attr(rust_js, rust_js::name = "TaskRunEvent")]
const TASK_RUN_EVENT: f64 = 5.0;
#[cfg_attr(rust_js, rust_js::name = "TaskYieldEvent")]
const TASK_YIELD_EVENT: f64 = 6.0;
#[cfg_attr(rust_js, rust_js::name = "SchedulerSuspendEvent")]
const SCHEDULER_SUSPEND_EVENT: f64 = 7.0;
#[cfg_attr(rust_js, rust_js::name = "SchedulerResumeEvent")]
const SCHEDULER_RESUME_EVENT: f64 = 8.0;

fn log_event(entries: &[f64]) {
    if let Some(mut event_log) = EVENT_LOG.get() {
        let offset = EVENT_LOG_INDEX.get();
        EVENT_LOG_INDEX.set(EVENT_LOG_INDEX.get() + entries.len() as f64);
        if EVENT_LOG_INDEX.get() + 1.0 > EVENT_LOG_SIZE.get() {
            EVENT_LOG_SIZE.set(EVENT_LOG_SIZE.get() * 2.0);
            if EVENT_LOG_SIZE.get() > MAX_EVENT_LOG_SIZE {
                // Using console['error'] to evade Babel and ESLint
                console::error((concat!(
                    "Scheduler Profiling: Event log exceeded maximum size. Don't ",
                    "forget to call `stopLoggingProfilingEvents()`."
                ),));
                stop_logging_profiling_events();
                return;
            }
            let new_event_log = int32_array::new(EVENT_LOG_SIZE.get() * 4.0);
            new_event_log.set(event_log, None);
            EVENT_LOG_BUFFER.set(Some(new_event_log.buffer()));
            EVENT_LOG.set(Some(new_event_log));
            event_log = new_event_log;
        }
        event_log.set(entries, Some(offset));
    }
}

pub fn start_logging_profiling_events() {
    EVENT_LOG_SIZE.set(INITIAL_EVENT_LOG_SIZE);
    let event_log_buffer = array_buffer::new(EVENT_LOG_SIZE.get() * 4.0);
    EVENT_LOG_BUFFER.set(Some(event_log_buffer));
    EVENT_LOG.set(Some(int32_array::new_with_buffer(
        event_log_buffer,
        None,
        None,
    )));
    EVENT_LOG_INDEX.set(0.0);
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn stop_logging_profiling_events() -> Option<&'static ArrayBuffer> {
    let buffer = EVENT_LOG_BUFFER.get();
    EVENT_LOG_SIZE.set(0.0);
    EVENT_LOG_BUFFER.set(None);
    EVENT_LOG.set(None);
    EVENT_LOG_INDEX.set(0.0);
    buffer
}

pub fn mark_task_start(task: &Task, ms: f64) {
    if ENABLE_PROFILING && EVENT_LOG.get().is_some() {
        // performance.now returns a float, representing milliseconds. When the
        // event is logged, it's coerced to an int. Convert to microseconds to
        // maintain extra degrees of precision.
        log_event(&[
            TASK_START_EVENT,
            ms * 1000.0,
            task.id,
            f64::from(task.priority_level),
        ]);
    }
}

pub fn mark_task_completed(task: &Task, ms: f64) {
    if ENABLE_PROFILING && EVENT_LOG.get().is_some() {
        log_event(&[TASK_COMPLETE_EVENT, ms * 1000.0, task.id]);
    }
}

pub fn mark_task_canceled(task: &Task, ms: f64) {
    if ENABLE_PROFILING && EVENT_LOG.get().is_some() {
        log_event(&[TASK_CANCEL_EVENT, ms * 1000.0, task.id]);
    }
}

pub fn mark_task_errored(task: &Task, ms: f64) {
    if ENABLE_PROFILING && EVENT_LOG.get().is_some() {
        log_event(&[TASK_ERROR_EVENT, ms * 1000.0, task.id]);
    }
}

pub fn mark_task_run(task: &Task, ms: f64) {
    if ENABLE_PROFILING {
        RUN_ID_COUNTER.set(RUN_ID_COUNTER.get() + 1.0);

        if EVENT_LOG.get().is_some() {
            log_event(&[TASK_RUN_EVENT, ms * 1000.0, task.id, RUN_ID_COUNTER.get()]);
        }
    }
}

pub fn mark_task_yield(task: &Task, ms: f64) {
    if ENABLE_PROFILING && EVENT_LOG.get().is_some() {
        log_event(&[TASK_YIELD_EVENT, ms * 1000.0, task.id, RUN_ID_COUNTER.get()]);
    }
}

pub fn mark_scheduler_suspended(ms: f64) {
    if ENABLE_PROFILING {
        MAIN_THREAD_ID_COUNTER.set(MAIN_THREAD_ID_COUNTER.get() + 1.0);

        if EVENT_LOG.get().is_some() {
            log_event(&[
                SCHEDULER_SUSPEND_EVENT,
                ms * 1000.0,
                MAIN_THREAD_ID_COUNTER.get(),
            ]);
        }
    }
}

pub fn mark_scheduler_unsuspended(ms: f64) {
    if ENABLE_PROFILING && EVENT_LOG.get().is_some() {
        log_event(&[
            SCHEDULER_RESUME_EVENT,
            ms * 1000.0,
            MAIN_THREAD_ID_COUNTER.get(),
        ]);
    }
}
