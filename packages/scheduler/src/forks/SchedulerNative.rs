// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::rc::Rc;

use react_native::{NativeScheduler, native_runtime_scheduler};

use crate as scheduler;
use crate::scheduler_priorities::PriorityLevel;
pub use crate::{Callback, Task};
use crate::{Options, Profiling};

/// What `unstable_scheduleCallback` is: the host's, or Scheduler's.
type ScheduleCallback = dyn Fn(PriorityLevel, Callback, Option<&Options>) -> Rc<Task>;

// This type is supposed to reflect the actual methods and arguments currently supported by the C++ implementation:
// https://github.com/react/react-native/blob/main/packages/react-native/ReactCommon/react/renderer/runtimescheduler/RuntimeSchedulerBinding.cpp
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "unstable_UserBlockingPriority")]
    pub static UNSTABLE_USER_BLOCKING_PRIORITY: PriorityLevel = if js::is_defined(native_runtime_scheduler) {
        native_runtime_scheduler.unstable_user_blocking_priority()
    } else {
        scheduler::UNSTABLE_USER_BLOCKING_PRIORITY
    };

    #[cfg_attr(rust_js, rust_js::name = "unstable_NormalPriority")]
    pub static UNSTABLE_NORMAL_PRIORITY: PriorityLevel = if js::is_defined(native_runtime_scheduler) {
        native_runtime_scheduler.unstable_normal_priority()
    } else {
        scheduler::UNSTABLE_NORMAL_PRIORITY
    };

    #[cfg_attr(rust_js, rust_js::name = "unstable_IdlePriority")]
    pub static UNSTABLE_IDLE_PRIORITY: PriorityLevel = if js::is_defined(native_runtime_scheduler) {
        native_runtime_scheduler.unstable_idle_priority()
    } else {
        scheduler::UNSTABLE_IDLE_PRIORITY
    };

    #[cfg_attr(rust_js, rust_js::name = "unstable_LowPriority")]
    pub static UNSTABLE_LOW_PRIORITY: PriorityLevel = if js::is_defined(native_runtime_scheduler) {
        native_runtime_scheduler.unstable_low_priority()
    } else {
        scheduler::UNSTABLE_LOW_PRIORITY
    };

    #[cfg_attr(rust_js, rust_js::name = "unstable_ImmediatePriority")]
    pub static UNSTABLE_IMMEDIATE_PRIORITY: PriorityLevel = if js::is_defined(native_runtime_scheduler) {
        native_runtime_scheduler.unstable_immediate_priority()
    } else {
        scheduler::UNSTABLE_IMMEDIATE_PRIORITY
    };

    #[cfg_attr(rust_js, rust_js::name = "unstable_scheduleCallback")]
    pub static UNSTABLE_SCHEDULE_CALLBACK: &'static ScheduleCallback = if js::is_defined(native_runtime_scheduler) {
        // SAFETY: the host's callback and task are Scheduler's to its callers,
        // as Flow's `NativeSchedulerType` declares them.
        unsafe {
            js::cast(Some(js::unknown(js::method((
                native_runtime_scheduler,
                NativeScheduler::unstable_schedule_callback,
            )))))
        }
    } else {
        &scheduler::unstable_schedule_callback
    };

    #[cfg_attr(rust_js, rust_js::name = "unstable_cancelCallback")]
    pub static UNSTABLE_CANCEL_CALLBACK: &'static dyn Fn(&Task) = if js::is_defined(native_runtime_scheduler) {
        // SAFETY: the task it's given is one the host's
        // `unstable_scheduleCallback` gave.
        unsafe {
            js::cast(Some(js::unknown(js::method((
                native_runtime_scheduler,
                NativeScheduler::unstable_cancel_callback,
            )))))
        }
    } else {
        &scheduler::unstable_cancel_callback
    };

    #[cfg_attr(rust_js, rust_js::name = "unstable_getCurrentPriorityLevel")]
    pub static UNSTABLE_GET_CURRENT_PRIORITY_LEVEL: &'static dyn Fn() -> PriorityLevel = if js::is_defined(native_runtime_scheduler) {
        js::method((native_runtime_scheduler, NativeScheduler::unstable_get_current_priority_level))
    } else {
        &scheduler::unstable_get_current_priority_level
    };

    #[cfg_attr(rust_js, rust_js::name = "unstable_shouldYield")]
    pub static UNSTABLE_SHOULD_YIELD: &'static dyn Fn() -> bool = if js::is_defined(native_runtime_scheduler) {
        js::method((native_runtime_scheduler, NativeScheduler::unstable_should_yield))
    } else {
        &scheduler::unstable_should_yield
    };

    #[cfg_attr(rust_js, rust_js::name = "unstable_requestPaint")]
    pub static UNSTABLE_REQUEST_PAINT: &'static dyn Fn() = if js::is_defined(native_runtime_scheduler) {
        js::method((native_runtime_scheduler, NativeScheduler::unstable_request_paint))
    } else {
        &scheduler::request_paint
    };

    #[cfg_attr(rust_js, rust_js::name = "unstable_now")]
    pub static UNSTABLE_NOW: &'static dyn Fn() -> f64 = if js::is_defined(native_runtime_scheduler) {
        js::method((native_runtime_scheduler, NativeScheduler::unstable_now))
    } else {
        &scheduler::unstable_now
    };
}

// These were never implemented on the native scheduler because React never calls them.
// For consistency, let's disable them altogether and make them throw.
#[cfg_attr(rust_js, rust_js::name = "unstable_next")]
pub const UNSTABLE_NEXT: fn() -> ! = throw_not_implemented;
#[cfg_attr(rust_js, rust_js::name = "unstable_runWithPriority")]
pub const UNSTABLE_RUN_WITH_PRIORITY: fn() -> ! = throw_not_implemented;
#[cfg_attr(rust_js, rust_js::name = "unstable_wrapCallback")]
pub const UNSTABLE_WRAP_CALLBACK: fn() -> ! = throw_not_implemented;
#[cfg_attr(rust_js, rust_js::name = "unstable_forceFrameRate")]
pub const UNSTABLE_FORCE_FRAME_RATE: fn() -> ! = throw_not_implemented;
#[cfg_attr(rust_js, rust_js::name = "unstable_Profiling")]
#[cfg_attr(rust_js, rust_js::nullable)]
pub const UNSTABLE_PROFILING: Option<Profiling> = None;

fn throw_not_implemented() -> ! {
    panic!("Not implemented.");
}
