// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use crate::environment::__VARIANT__;

// In www, these flags are controlled by GKs. Because most GKs have some
// population running in either mode, we should run our tests that way, too,
//
// Use __VARIANT__ to simulate a GK. The tests will be run twice: once
// with the __VARIANT__ set to `true`, and once set to `false`.

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "alwaysThrottleRetries")]
    pub static ALWAYS_THROTTLE_RETRIES: bool = __VARIANT__;
}
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "disableLegacyContextForFunctionComponents")]
    pub static DISABLE_LEGACY_CONTEXT_FOR_FUNCTION_COMPONENTS: bool = __VARIANT__;
}
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "disableSchedulerTimeoutInWorkLoop")]
    pub static DISABLE_SCHEDULER_TIMEOUT_IN_WORK_LOOP: bool = __VARIANT__;
}
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableNoCloningMemoCache")]
    pub static ENABLE_NO_CLONING_MEMO_CACHE: bool = __VARIANT__;
}
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableObjectFiber")]
    pub static ENABLE_OBJECT_FIBER: bool = __VARIANT__;
}
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableRetryLaneExpiration")]
    pub static ENABLE_RETRY_LANE_EXPIRATION: bool = __VARIANT__;
}
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableTransitionTracing")]
    pub static ENABLE_TRANSITION_TRACING: bool = __VARIANT__;
}
#[cfg_attr(rust_js, rust_js::name = "retryLaneExpirationMs")]
pub const RETRY_LANE_EXPIRATION_MS: f64 = 5000.0;
#[cfg_attr(rust_js, rust_js::name = "syncLaneExpirationMs")]
pub const SYNC_LANE_EXPIRATION_MS: f64 = 250.0;
#[cfg_attr(rust_js, rust_js::name = "transitionLaneExpirationMs")]
pub const TRANSITION_LANE_EXPIRATION_MS: f64 = 5000.0;

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableSchedulingProfiler")]
    pub static ENABLE_SCHEDULING_PROFILER: bool = __VARIANT__;
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableInfiniteRenderLoopDetection")]
    pub static ENABLE_INFINITE_RENDER_LOOP_DETECTION: bool = __VARIANT__;
}
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableInfiniteRenderLoopDetectionForceThrow")]
    pub static ENABLE_INFINITE_RENDER_LOOP_DETECTION_FORCE_THROW: bool = __VARIANT__;
}
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableConditionalUseWarning")]
    pub static ENABLE_CONDITIONAL_USE_WARNING: bool = __VARIANT__;
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableSuspenseyImages")]
    pub static ENABLE_SUSPENSEY_IMAGES: bool = __VARIANT__;
}
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableViewTransition")]
    pub static ENABLE_VIEW_TRANSITION: bool = __VARIANT__;
}
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableViewTransitionParentEnterExit")]
    pub static ENABLE_VIEW_TRANSITION_PARENT_ENTER_EXIT: bool = __VARIANT__;
}
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableScrollEndPolyfill")]
    pub static ENABLE_SCROLL_END_POLYFILL: bool = __VARIANT__;
}
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableInternalInstanceMap")]
    pub static ENABLE_INTERNAL_INSTANCE_MAP: bool = __VARIANT__;
}

// TODO: These flags are hard-coded to the default values used in open source.
// Update the tests so that they pass in either mode, then set these
// to __VARIANT__.
// You probably *don't* want to add more hardcoded ones.
// Instead, try to add them above with the __VARIANT__ value.
