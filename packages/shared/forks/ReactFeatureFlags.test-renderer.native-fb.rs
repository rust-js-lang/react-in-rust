// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use crate::environment::__PROFILE__;
use crate::react_feature_flags::CompilerLint;

#[cfg_attr(rust_js, rust_js::name = "alwaysThrottleRetries")]
pub const ALWAYS_THROTTLE_RETRIES: bool = false;
#[cfg_attr(rust_js, rust_js::name = "disableClientCache")]
pub const DISABLE_CLIENT_CACHE: bool = true;
#[cfg_attr(rust_js, rust_js::name = "disableCommentsAsDOMContainers")]
pub const DISABLE_COMMENTS_AS_DOM_CONTAINERS: bool = true;
#[cfg_attr(rust_js, rust_js::name = "disableInputAttributeSyncing")]
pub const DISABLE_INPUT_ATTRIBUTE_SYNCING: bool = false;
#[cfg_attr(rust_js, rust_js::name = "disableLegacyContext")]
pub const DISABLE_LEGACY_CONTEXT: bool = false;
#[cfg_attr(rust_js, rust_js::name = "disableLegacyContextForFunctionComponents")]
pub const DISABLE_LEGACY_CONTEXT_FOR_FUNCTION_COMPONENTS: bool = false;
#[cfg_attr(rust_js, rust_js::name = "disableLegacyMode")]
pub const DISABLE_LEGACY_MODE: bool = false;
#[cfg_attr(rust_js, rust_js::name = "disableSchedulerTimeoutInWorkLoop")]
pub const DISABLE_SCHEDULER_TIMEOUT_IN_WORK_LOOP: bool = false;
#[cfg_attr(rust_js, rust_js::name = "disableTextareaChildren")]
pub const DISABLE_TEXTAREA_CHILDREN: bool = false;
#[cfg_attr(rust_js, rust_js::name = "enableAsyncIterableChildren")]
pub const ENABLE_ASYNC_ITERABLE_CHILDREN: bool = false;
#[cfg_attr(rust_js, rust_js::name = "enableFlightWeakThenables")]
pub const ENABLE_FLIGHT_WEAK_THENABLES: bool = false;
#[cfg_attr(rust_js, rust_js::name = "enableFlightObjectReferences")]
pub const ENABLE_FLIGHT_OBJECT_REFERENCES: bool = false;
#[cfg_attr(rust_js, rust_js::name = "enableCPUSuspense")]
pub const ENABLE_CPU_SUSPENSE: bool = true;
#[cfg_attr(rust_js, rust_js::name = "enableCreateEventHandleAPI")]
pub const ENABLE_CREATE_EVENT_HANDLE_API: bool = false;
#[cfg_attr(rust_js, rust_js::name = "enableMoveBefore")]
pub const ENABLE_MOVE_BEFORE: bool = false;
#[cfg_attr(rust_js, rust_js::name = "enableFizzExternalRuntime")]
pub const ENABLE_FIZZ_EXTERNAL_RUNTIME: bool = true;
#[cfg_attr(rust_js, rust_js::name = "enableInfiniteRenderLoopDetection")]
pub const ENABLE_INFINITE_RENDER_LOOP_DETECTION: bool = false;
#[cfg_attr(rust_js, rust_js::name = "enableInfiniteRenderLoopDetectionForceThrow")]
pub const ENABLE_INFINITE_RENDER_LOOP_DETECTION_FORCE_THROW: bool = false;
#[cfg_attr(rust_js, rust_js::name = "enableConditionalUseWarning")]
pub const ENABLE_CONDITIONAL_USE_WARNING: bool = false;
#[cfg_attr(rust_js, rust_js::name = "enableLegacyCache")]
pub const ENABLE_LEGACY_CACHE: bool = false;
#[cfg_attr(rust_js, rust_js::name = "enableLegacyFBSupport")]
pub const ENABLE_LEGACY_FB_SUPPORT: bool = false;
#[cfg_attr(rust_js, rust_js::name = "enableLegacyHidden")]
pub const ENABLE_LEGACY_HIDDEN: bool = false;
#[cfg_attr(rust_js, rust_js::name = "enableNoCloningMemoCache")]
pub const ENABLE_NO_CLONING_MEMO_CACHE: bool = false;
#[cfg_attr(rust_js, rust_js::name = "enableObjectFiber")]
pub const ENABLE_OBJECT_FIBER: bool = false;
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableProfilerCommitHooks")]
    pub static ENABLE_PROFILER_COMMIT_HOOKS: bool = __PROFILE__;
}
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableProfilerNestedUpdatePhase")]
    pub static ENABLE_PROFILER_NESTED_UPDATE_PHASE: bool = __PROFILE__;
}
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableProfilerTimer")]
    pub static ENABLE_PROFILER_TIMER: bool = __PROFILE__;
}
#[cfg_attr(rust_js, rust_js::name = "enableReactTestRendererWarning")]
pub const ENABLE_REACT_TEST_RENDERER_WARNING: bool = false;
#[cfg_attr(rust_js, rust_js::name = "enableRetryLaneExpiration")]
pub const ENABLE_RETRY_LANE_EXPIRATION: bool = false;
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableSchedulingProfiler")]
    pub static ENABLE_SCHEDULING_PROFILER: bool = __PROFILE__;
}
#[cfg_attr(rust_js, rust_js::name = "enableComponentPerformanceTrack")]
pub const ENABLE_COMPONENT_PERFORMANCE_TRACK: bool = false;
#[cfg_attr(rust_js, rust_js::name = "enablePerformanceIssueReporting")]
pub const ENABLE_PERFORMANCE_ISSUE_REPORTING: bool = false;
#[cfg_attr(rust_js, rust_js::name = "enableScopeAPI")]
pub const ENABLE_SCOPE_API: bool = false;
#[cfg_attr(rust_js, rust_js::name = "enableSuspenseAvoidThisFallback")]
pub const ENABLE_SUSPENSE_AVOID_THIS_FALLBACK: bool = false;
#[cfg_attr(rust_js, rust_js::name = "enableSuspenseCallback")]
pub const ENABLE_SUSPENSE_CALLBACK: bool = false;
#[cfg_attr(rust_js, rust_js::name = "enableTaint")]
pub const ENABLE_TAINT: bool = true;
#[cfg_attr(rust_js, rust_js::name = "enableTransitionTracing")]
pub const ENABLE_TRANSITION_TRACING: bool = false;
#[cfg_attr(rust_js, rust_js::name = "enableUpdaterTracking")]
pub const ENABLE_UPDATER_TRACKING: bool = false;
#[cfg_attr(rust_js, rust_js::name = "passChildrenWhenCloningPersistedNodes")]
pub const PASS_CHILDREN_WHEN_CLONING_PERSISTED_NODES: bool = false;
#[cfg_attr(rust_js, rust_js::name = "retryLaneExpirationMs")]
pub const RETRY_LANE_EXPIRATION_MS: f64 = 5000.0;
#[cfg_attr(rust_js, rust_js::name = "syncLaneExpirationMs")]
pub const SYNC_LANE_EXPIRATION_MS: f64 = 250.0;
#[cfg_attr(rust_js, rust_js::name = "transitionLaneExpirationMs")]
pub const TRANSITION_LANE_EXPIRATION_MS: f64 = 5000.0;
#[cfg_attr(rust_js, rust_js::name = "enableYieldingBeforePassive")]
pub const ENABLE_YIELDING_BEFORE_PASSIVE: bool = false;
#[cfg_attr(rust_js, rust_js::name = "enableThrottledScheduling")]
pub const ENABLE_THROTTLED_SCHEDULING: bool = false;
#[cfg_attr(rust_js, rust_js::name = "enableViewTransition")]
pub const ENABLE_VIEW_TRANSITION: bool = true;
#[cfg_attr(rust_js, rust_js::name = "enableViewTransitionParentEnterExit")]
pub const ENABLE_VIEW_TRANSITION_PARENT_ENTER_EXIT: bool = false;
#[cfg_attr(rust_js, rust_js::name = "enableViewTransitionForPersistenceMode")]
pub const ENABLE_VIEW_TRANSITION_FOR_PERSISTENCE_MODE: bool = false;
#[cfg_attr(rust_js, rust_js::name = "enableGestureTransition")]
pub const ENABLE_GESTURE_TRANSITION: bool = false;
#[cfg_attr(rust_js, rust_js::name = "enableScrollEndPolyfill")]
pub const ENABLE_SCROLL_END_POLYFILL: bool = true;
#[cfg_attr(rust_js, rust_js::name = "enableSuspenseyImages")]
pub const ENABLE_SUSPENSEY_IMAGES: bool = false;
#[cfg_attr(rust_js, rust_js::name = "enableFizzBlockingRender")]
pub const ENABLE_FIZZ_BLOCKING_RENDER: bool = true;
#[cfg_attr(rust_js, rust_js::name = "enableSrcObject")]
pub const ENABLE_SRC_OBJECT: bool = false;
#[cfg_attr(rust_js, rust_js::name = "enableHydrationChangeEvent")]
pub const ENABLE_HYDRATION_CHANGE_EVENT: bool = false;
#[cfg_attr(rust_js, rust_js::name = "enableDefaultTransitionIndicator")]
pub const ENABLE_DEFAULT_TRANSITION_INDICATOR: bool = true;
#[cfg_attr(rust_js, rust_js::name = "ownerStackLimit")]
pub const OWNER_STACK_LIMIT: f64 = 1e4;
#[cfg_attr(rust_js, rust_js::name = "enableOptimisticKey")]
pub const ENABLE_OPTIMISTIC_KEY: bool = false;

#[cfg_attr(rust_js, rust_js::name = "eprh_enableUseKeyedStateCompilerLint")]
pub const EPRH_ENABLE_USE_KEYED_STATE_COMPILER_LINT: bool = false;
#[cfg_attr(
    rust_js,
    rust_js::name = "eprh_enableVerboseNoSetStateInEffectCompilerLint"
)]
pub const EPRH_ENABLE_VERBOSE_NO_SET_STATE_IN_EFFECT_COMPILER_LINT: bool = false;
#[cfg_attr(
    rust_js,
    rust_js::name = "eprh_enableExhaustiveEffectDependenciesCompilerLint"
)]
pub const EPRH_ENABLE_EXHAUSTIVE_EFFECT_DEPENDENCIES_COMPILER_LINT: CompilerLint =
    CompilerLint::Off;
