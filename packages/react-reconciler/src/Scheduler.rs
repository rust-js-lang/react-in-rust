// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// This module only exists as an ESM wrapper around the external CommonJS
// Scheduler dependency. Notice that we're intentionally not using named imports
// because Rollup would use dynamic dispatch for CommonJS interop named imports.
// When we switch to ESM, we can delete this module.

use std::marker::PhantomData;

use js::{JsObject, Unknown};

/// The scheduler's `PriorityLevel`.
pub type PriorityLevel = u8;

/// A task the scheduler was given, which `cancelCallback` cancels.
pub struct Task(PhantomData<JsObject>);

unsafe impl js::Defined for Task {}

/// What `scheduleCallback` is given: how long to wait first.
pub struct SchedulerOptions {
    pub delay: f64,
}

/// A task's work: given whether it must finish now, it does some, and
/// gives what's left of it, or nothing where it's done.
#[repr(transparent)]
pub struct SchedulerCallback(pub Box<dyn FnMut(bool) -> Option<SchedulerCallback>>);

pub type ScheduleCallback =
    fn(PriorityLevel, SchedulerCallback, Option<&SchedulerOptions>) -> &'static Task;

/// SchedulerMock's `log`: what a test logs, any value.
pub type Log = dyn Fn(Option<&'static Unknown>);

#[cfg_attr(rust_js, rust_js::link_name = "scheduler#*.unstable_scheduleCallback")]
fn unstable_schedule_callback(
    priority_level: PriorityLevel,
    callback: SchedulerCallback,
    options: Option<&SchedulerOptions>,
) -> &'static Task {
    unreachable!()
}

#[cfg_attr(rust_js, rust_js::link_name = "scheduler#*.unstable_cancelCallback")]
fn unstable_cancel_callback(task: &Task) {
    unreachable!()
}

#[cfg_attr(rust_js, rust_js::link_name = "scheduler#*.unstable_shouldYield")]
fn unstable_should_yield() -> bool {
    unreachable!()
}

#[cfg_attr(rust_js, rust_js::link_name = "scheduler#*.unstable_requestPaint")]
fn unstable_request_paint() {
    unreachable!()
}

#[cfg_attr(rust_js, rust_js::link_name = "scheduler#*.unstable_now")]
fn unstable_now() -> f64 {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "scheduler#*.unstable_getCurrentPriorityLevel"
)]
fn unstable_get_current_priority_level() -> PriorityLevel {
    unreachable!()
}

unsafe extern "Rust" {
    #[link_name = "scheduler#*.unstable_ImmediatePriority"]
    safe static UNSTABLE_IMMEDIATE_PRIORITY: PriorityLevel;
    #[link_name = "scheduler#*.unstable_UserBlockingPriority"]
    safe static UNSTABLE_USER_BLOCKING_PRIORITY: PriorityLevel;
    #[link_name = "scheduler#*.unstable_NormalPriority"]
    safe static UNSTABLE_NORMAL_PRIORITY: PriorityLevel;
    #[link_name = "scheduler#*.unstable_LowPriority"]
    safe static UNSTABLE_LOW_PRIORITY: PriorityLevel;
    #[link_name = "scheduler#*.unstable_IdlePriority"]
    safe static UNSTABLE_IDLE_PRIORITY: PriorityLevel;

    // SchedulerMock's alone.
    #[link_name = "scheduler#*.log"]
    safe static SCHEDULER_LOG: Option<&'static Log>;
    #[link_name = "scheduler#*.unstable_setDisableYieldValue"]
    safe static SCHEDULER_SET_DISABLE_YIELD_VALUE: Option<&'static dyn Fn(bool)>;
}

#[cfg_attr(rust_js, rust_js::name = "scheduleCallback")]
pub static SCHEDULE_CALLBACK: ScheduleCallback = unstable_schedule_callback;
#[cfg_attr(rust_js, rust_js::name = "cancelCallback")]
pub static CANCEL_CALLBACK: fn(&Task) = unstable_cancel_callback;
#[cfg_attr(rust_js, rust_js::name = "shouldYield")]
pub static SHOULD_YIELD: fn() -> bool = unstable_should_yield;
#[cfg_attr(rust_js, rust_js::name = "requestPaint")]
pub static REQUEST_PAINT: fn() = unstable_request_paint;
#[cfg_attr(rust_js, rust_js::name = "now")]
pub static NOW: fn() -> f64 = unstable_now;
#[cfg_attr(rust_js, rust_js::name = "getCurrentPriorityLevel")]
pub static GET_CURRENT_PRIORITY_LEVEL: fn() -> PriorityLevel = unstable_get_current_priority_level;

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "ImmediatePriority")]
    pub static IMMEDIATE_PRIORITY: PriorityLevel = UNSTABLE_IMMEDIATE_PRIORITY;
    #[cfg_attr(rust_js, rust_js::name = "UserBlockingPriority")]
    pub static USER_BLOCKING_PRIORITY: PriorityLevel = UNSTABLE_USER_BLOCKING_PRIORITY;
    #[cfg_attr(rust_js, rust_js::name = "NormalPriority")]
    pub static NORMAL_PRIORITY: PriorityLevel = UNSTABLE_NORMAL_PRIORITY;
    #[cfg_attr(rust_js, rust_js::name = "LowPriority")]
    pub static LOW_PRIORITY: PriorityLevel = UNSTABLE_LOW_PRIORITY;
    #[cfg_attr(rust_js, rust_js::name = "IdlePriority")]
    pub static IDLE_PRIORITY: PriorityLevel = UNSTABLE_IDLE_PRIORITY;

    // this doesn't actually exist on the scheduler, but it *does*
    // on scheduler/unstable_mock, which we'll need for internal testing
    pub static LOG: Option<&'static Log> = SCHEDULER_LOG;
    #[cfg_attr(rust_js, rust_js::name = "unstable_setDisableYieldValue")]
    pub static UNSTABLE_SET_DISABLE_YIELD_VALUE: Option<&'static dyn Fn(bool)> =
        SCHEDULER_SET_DISABLE_YIELD_VALUE;
}
