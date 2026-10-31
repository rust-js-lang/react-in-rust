// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;
use std::panic::{self, AssertUnwindSafe};
use std::rc::Rc;

use js::{TimeoutId, function};
use webapi::{
    Performance, Scheduler, SchedulerPostTaskOptions, TaskController, TaskControllerInit, Window,
    task_controller, window,
};

use crate::scheduler_priorities::{
    IDLE_PRIORITY, IMMEDIATE_PRIORITY, LOW_PRIORITY, NORMAL_PRIORITY, PriorityLevel,
    USER_BLOCKING_PRIORITY,
};
use crate::{Callback, Options, Profiling};

/// A scheduled callback's handle: the controller that aborts its task.
pub struct CallbackNode {
    _controller: &'static TaskController,
}

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

thread_local! {
    // Capture local references to native APIs, in case a polyfill overrides them.
    static PERF: &'static Performance = window.performance();
    static SET_TIMEOUT: &'static dyn Fn(Box<dyn FnOnce()>, Option<f64>) -> &'static TimeoutId =
        js::method((window, Window::set_timeout));

    // Use experimental Chrome Scheduler postTask API.
    static SCHEDULER: &'static Scheduler = webapi::global_this.scheduler();

    static GET_CURRENT_TIME: &'static dyn Fn() -> f64 = PERF.with(|&perf| {
        function::bind(js::method((perf, Performance::now)), Some(perf), ())
    });

    #[cfg_attr(rust_js, rust_js::name = "unstable_now")]
    pub static UNSTABLE_NOW: &'static dyn Fn() -> f64 =
        GET_CURRENT_TIME.with(|&get_current_time| get_current_time);
}

// Scheduler periodically yields in case there is other work on the main
// thread, like user events. By default, it yields multiple times per frame.
// It does not attempt to align with frame boundaries, since most tasks don't
// need to be frame aligned; for those that do, use requestAnimationFrame.
#[cfg_attr(rust_js, rust_js::name = "yieldInterval")]
const YIELD_INTERVAL: f64 = 5.0;

thread_local! {
    static DEADLINE: Cell<f64> = const { Cell::new(0.0) };

    #[cfg_attr(rust_js, rust_js::name = "currentPriorityLevel_DEPRECATED")]
    static CURRENT_PRIORITY_LEVEL_DEPRECATED: Cell<PriorityLevel> =
        const { Cell::new(NORMAL_PRIORITY) };
}

// Always yield at the end of the frame.
#[cfg_attr(rust_js, rust_js::name = "unstable_shouldYield")]
pub fn unstable_should_yield() -> bool {
    GET_CURRENT_TIME.with(|get_current_time| get_current_time()) >= DEADLINE.get()
}

#[cfg_attr(rust_js, rust_js::name = "unstable_requestPaint")]
pub fn unstable_request_paint() {
    // Since we yield every frame regardless, `requestPaint` has no effect.
}

#[cfg_attr(rust_js, rust_js::name = "unstable_scheduleCallback")]
pub fn unstable_schedule_callback(
    priority_level: PriorityLevel,
    callback: Callback,
    options: Option<&Options>,
) -> Rc<CallbackNode> {
    let post_task_priority = match priority_level {
        IMMEDIATE_PRIORITY | USER_BLOCKING_PRIORITY => "user-blocking",
        LOW_PRIORITY | NORMAL_PRIORITY => "user-visible",
        IDLE_PRIORITY => "background",
        _ => "user-visible",
    };

    let controller = task_controller::new(Some(TaskControllerInit {
        priority: Some(post_task_priority),
    }));
    let post_task_options = SchedulerPostTaskOptions {
        delay: match options {
            Some(options) => options.delay,
            None => Some(0.0),
        },
        signal: Some(controller.signal()),
        ..Default::default()
    };

    let node = Rc::new(CallbackNode {
        _controller: controller,
    });

    let task_node = Rc::clone(&node);
    SCHEDULER.with(|scheduler| {
        scheduler
            .post_task(
                Box::new(move || {
                    run_task(priority_level, post_task_priority, task_node, callback);
                    None
                }),
                Some(post_task_options),
            )
            .catch(handle_abort_error);
    });

    node
}

fn run_task(
    priority_level: PriorityLevel,
    #[cfg_attr(rust_js, rust_js::name = "postTaskPriority")] _post_task_priority: &'static str,
    node: Rc<CallbackNode>,
    callback: Callback,
) {
    let Callback(mut callback) = callback;
    DEADLINE.set(GET_CURRENT_TIME.with(|get_current_time| get_current_time()) + YIELD_INTERVAL);
    let _reset = RestorePriority(NORMAL_PRIORITY);
    if let Err(error) = panic::catch_unwind(AssertUnwindSafe(|| {
        CURRENT_PRIORITY_LEVEL_DEPRECATED.set(priority_level);
        #[cfg_attr(rust_js, rust_js::name = "didTimeout_DEPRECATED")]
        let did_timeout_deprecated = false;
        let result = callback(did_timeout_deprecated);
        if js::is_function(&result)
            && let Some(continuation) = result
        {
            // Assume this is a continuation
            let continuation_options = SchedulerPostTaskOptions {
                signal: Some(node._controller.signal()),
                ..Default::default()
            };

            let next_task =
                move || run_task(priority_level, _post_task_priority, node, continuation);

            SCHEDULER.with(|scheduler| {
                if js::is_defined((scheduler, Scheduler::r#yield)) {
                    scheduler
                        .r#yield(Some(continuation_options))
                        .then::<(), ()>(move |()| next_task())
                        .catch(handle_abort_error);
                } else {
                    scheduler
                        .post_task(
                            Box::new(move || {
                                next_task();
                                None
                            }),
                            Some(continuation_options),
                        )
                        .catch(handle_abort_error);
                }
            });
        }
    })) {
        // We're inside a `postTask` promise. If we don't handle this error, then it
        // will trigger an "Unhandled promise rejection" error. We don't want that,
        // but we do want the default error reporting behavior that normal
        // (non-Promise) tasks get for unhandled errors.
        //
        // So we'll re-throw the error inside a regular browser task.
        SET_TIMEOUT.with(|set_timeout| {
            set_timeout(Box::new(move || panic::resume_unwind(error)), None);
        });
    }
}

fn handle_abort_error(_error: &'static js::JsError) {
    // Abort errors are an implementation detail. We don't expose the
    // TaskController to the user, nor do we expose the promise that is returned
    // from `postTask`. So we should suppress them, since there's no way for the
    // user to handle them.
}

#[cfg_attr(rust_js, rust_js::name = "unstable_cancelCallback")]
pub fn unstable_cancel_callback(node: &CallbackNode) {
    let controller = node._controller;
    controller.abort(None);
}

#[cfg_attr(rust_js, rust_js::name = "unstable_runWithPriority")]
pub fn unstable_run_with_priority<T>(
    priority_level: PriorityLevel,
    callback: impl FnOnce() -> T,
) -> T {
    let _restore = RestorePriority(CURRENT_PRIORITY_LEVEL_DEPRECATED.get());
    CURRENT_PRIORITY_LEVEL_DEPRECATED.set(priority_level);
    callback()
}

#[cfg_attr(rust_js, rust_js::name = "unstable_getCurrentPriorityLevel")]
pub fn unstable_get_current_priority_level() -> PriorityLevel {
    CURRENT_PRIORITY_LEVEL_DEPRECATED.get()
}

#[cfg_attr(rust_js, rust_js::name = "unstable_next")]
pub fn unstable_next<T>(callback: impl FnOnce() -> T) -> T {
    let priority_level = match CURRENT_PRIORITY_LEVEL_DEPRECATED.get() {
        // Shift down to normal priority
        IMMEDIATE_PRIORITY | USER_BLOCKING_PRIORITY | NORMAL_PRIORITY => NORMAL_PRIORITY,
        // Anything lower than normal priority should remain at the current level.
        current => current,
    };

    let _restore = RestorePriority(CURRENT_PRIORITY_LEVEL_DEPRECATED.get());
    CURRENT_PRIORITY_LEVEL_DEPRECATED.set(priority_level);
    callback()
}

#[cfg_attr(rust_js, rust_js::name = "unstable_wrapCallback")]
pub fn unstable_wrap_callback<T>(callback: impl Fn() -> T) -> impl Fn() -> T {
    let parent_priority_level = CURRENT_PRIORITY_LEVEL_DEPRECATED.get();
    move || {
        let _restore = RestorePriority(CURRENT_PRIORITY_LEVEL_DEPRECATED.get());
        CURRENT_PRIORITY_LEVEL_DEPRECATED.set(parent_priority_level);
        callback()
    }
}

#[cfg_attr(rust_js, rust_js::name = "unstable_forceFrameRate")]
pub fn unstable_force_frame_rate() {}

// Currently no profiling build
#[cfg_attr(rust_js, rust_js::name = "unstable_Profiling")]
#[cfg_attr(rust_js, rust_js::nullable)]
pub const UNSTABLE_PROFILING: Option<Profiling> = None;

/// What sets the current priority level to the one it holds however the
/// work it was made for is left.
struct RestorePriority(PriorityLevel);

impl Drop for RestorePriority {
    fn drop(&mut self) {
        CURRENT_PRIORITY_LEVEL_DEPRECATED.set(self.0);
    }
}
