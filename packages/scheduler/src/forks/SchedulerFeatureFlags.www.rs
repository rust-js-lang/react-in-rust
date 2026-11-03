// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use shared::environment::__DEV__;

/// What www provides as `SchedulerFeatureFlags`: the flags its gatekeepers
/// set, SchedulerFeatureFlags.www-dynamic's.
struct DynamicFeatureFlags {
    enable_request_paint: bool,
}

#[cfg_attr(rust_js, rust_js::link_name = "get SchedulerFeatureFlags#*")]
#[cfg_attr(rust_js, rust_js::require)]
fn scheduler_feature_flags() -> &'static DynamicFeatureFlags {
    unreachable!()
}

thread_local! {
    static DYNAMIC_FEATURE_FLAGS: &'static DynamicFeatureFlags = scheduler_feature_flags();
}

// GAP rd751: upstream destructures it, `export const {enableRequestPaint} =
// dynamicFeatureFlags;`.
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableRequestPaint")]
    pub static ENABLE_REQUEST_PAINT: bool = DYNAMIC_FEATURE_FLAGS.with(|flags| flags.enable_request_paint);
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableProfiling")]
    pub static ENABLE_PROFILING: bool = __DEV__;
}
#[cfg_attr(rust_js, rust_js::name = "frameYieldMs")]
pub const FRAME_YIELD_MS: f64 = 10.0;

#[cfg_attr(rust_js, rust_js::name = "userBlockingPriorityTimeout")]
pub const USER_BLOCKING_PRIORITY_TIMEOUT: f64 = 250.0;
#[cfg_attr(rust_js, rust_js::name = "normalPriorityTimeout")]
pub const NORMAL_PRIORITY_TIMEOUT: f64 = 5000.0;
#[cfg_attr(rust_js, rust_js::name = "lowPriorityTimeout")]
pub const LOW_PRIORITY_TIMEOUT: f64 = 10000.0;

#[cfg_attr(rust_js, rust_js::name = "enableAlwaysYieldScheduler")]
pub const ENABLE_ALWAYS_YIELD_SCHEDULER: bool = false;
