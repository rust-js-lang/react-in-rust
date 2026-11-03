// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use crate::environment::{__EXPERIMENTAL__, __PROFILE__};
use crate::react_feature_flags::CompilerLint;

/// What www provides as `ReactFeatureFlags`: the flags its gatekeepers set,
/// ReactFeatureFlags.www-dynamic's.
struct DynamicFeatureFlags {
    always_throttle_retries: bool,
    disable_legacy_context_for_function_components: bool,
    disable_scheduler_timeout_in_work_loop: bool,
    enable_no_cloning_memo_cache: bool,
    enable_object_fiber: bool,
    enable_retry_lane_expiration: bool,
    enable_transition_tracing: bool,
    retry_lane_expiration_ms: f64,
    sync_lane_expiration_ms: f64,
    transition_lane_expiration_ms: f64,
    enable_scheduling_profiler: bool,
    enable_infinite_render_loop_detection: bool,
    enable_infinite_render_loop_detection_force_throw: bool,
    enable_conditional_use_warning: bool,
    enable_suspensey_images: bool,
    enable_view_transition: bool,
    enable_view_transition_parent_enter_exit: bool,
    enable_scroll_end_polyfill: bool,
    enable_internal_instance_map: bool,
}

#[cfg_attr(rust_js, rust_js::link_name = "get ReactFeatureFlags#*")]
#[cfg_attr(rust_js, rust_js::require)]
fn react_feature_flags() -> &'static DynamicFeatureFlags {
    unreachable!()
}

thread_local! {
    // Re-export dynamic flags from the www version.
    static DYNAMIC_FEATURE_FLAGS: &'static DynamicFeatureFlags = react_feature_flags();
}

// GAP rd751: upstream destructures them in one statement, `export const {..}
// = dynamicFeatureFlags;`; each is its own `export const` here.
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "alwaysThrottleRetries")]
    pub static ALWAYS_THROTTLE_RETRIES: bool = DYNAMIC_FEATURE_FLAGS.with(|flags| flags.always_throttle_retries);
    #[cfg_attr(rust_js, rust_js::name = "disableLegacyContextForFunctionComponents")]
    pub static DISABLE_LEGACY_CONTEXT_FOR_FUNCTION_COMPONENTS: bool = DYNAMIC_FEATURE_FLAGS.with(|flags| flags.disable_legacy_context_for_function_components);
    #[cfg_attr(rust_js, rust_js::name = "disableSchedulerTimeoutInWorkLoop")]
    pub static DISABLE_SCHEDULER_TIMEOUT_IN_WORK_LOOP: bool = DYNAMIC_FEATURE_FLAGS.with(|flags| flags.disable_scheduler_timeout_in_work_loop);
    #[cfg_attr(rust_js, rust_js::name = "enableInfiniteRenderLoopDetection")]
    pub static ENABLE_INFINITE_RENDER_LOOP_DETECTION: bool = DYNAMIC_FEATURE_FLAGS.with(|flags| flags.enable_infinite_render_loop_detection);
    #[cfg_attr(rust_js, rust_js::name = "enableInfiniteRenderLoopDetectionForceThrow")]
    pub static ENABLE_INFINITE_RENDER_LOOP_DETECTION_FORCE_THROW: bool = DYNAMIC_FEATURE_FLAGS.with(|flags| flags.enable_infinite_render_loop_detection_force_throw);
    #[cfg_attr(rust_js, rust_js::name = "enableConditionalUseWarning")]
    pub static ENABLE_CONDITIONAL_USE_WARNING: bool = DYNAMIC_FEATURE_FLAGS.with(|flags| flags.enable_conditional_use_warning);
    #[cfg_attr(rust_js, rust_js::name = "enableNoCloningMemoCache")]
    pub static ENABLE_NO_CLONING_MEMO_CACHE: bool = DYNAMIC_FEATURE_FLAGS.with(|flags| flags.enable_no_cloning_memo_cache);
    #[cfg_attr(rust_js, rust_js::name = "enableObjectFiber")]
    pub static ENABLE_OBJECT_FIBER: bool = DYNAMIC_FEATURE_FLAGS.with(|flags| flags.enable_object_fiber);
    #[cfg_attr(rust_js, rust_js::name = "enableRetryLaneExpiration")]
    pub static ENABLE_RETRY_LANE_EXPIRATION: bool = DYNAMIC_FEATURE_FLAGS.with(|flags| flags.enable_retry_lane_expiration);
    #[cfg_attr(rust_js, rust_js::name = "enableTransitionTracing")]
    pub static ENABLE_TRANSITION_TRACING: bool = DYNAMIC_FEATURE_FLAGS.with(|flags| flags.enable_transition_tracing);
    #[cfg_attr(rust_js, rust_js::name = "retryLaneExpirationMs")]
    pub static RETRY_LANE_EXPIRATION_MS: f64 = DYNAMIC_FEATURE_FLAGS.with(|flags| flags.retry_lane_expiration_ms);
    #[cfg_attr(rust_js, rust_js::name = "syncLaneExpirationMs")]
    pub static SYNC_LANE_EXPIRATION_MS: f64 = DYNAMIC_FEATURE_FLAGS.with(|flags| flags.sync_lane_expiration_ms);
    #[cfg_attr(rust_js, rust_js::name = "transitionLaneExpirationMs")]
    pub static TRANSITION_LANE_EXPIRATION_MS: f64 = DYNAMIC_FEATURE_FLAGS.with(|flags| flags.transition_lane_expiration_ms);
    #[cfg_attr(rust_js, rust_js::name = "enableSuspenseyImages")]
    pub static ENABLE_SUSPENSEY_IMAGES: bool = DYNAMIC_FEATURE_FLAGS.with(|flags| flags.enable_suspensey_images);
    #[cfg_attr(rust_js, rust_js::name = "enableViewTransition")]
    pub static ENABLE_VIEW_TRANSITION: bool = DYNAMIC_FEATURE_FLAGS.with(|flags| flags.enable_view_transition);
    #[cfg_attr(rust_js, rust_js::name = "enableScrollEndPolyfill")]
    pub static ENABLE_SCROLL_END_POLYFILL: bool = DYNAMIC_FEATURE_FLAGS.with(|flags| flags.enable_scroll_end_polyfill);
    #[cfg_attr(rust_js, rust_js::name = "enableInternalInstanceMap")]
    pub static ENABLE_INTERNAL_INSTANCE_MAP: bool = DYNAMIC_FEATURE_FLAGS.with(|flags| flags.enable_internal_instance_map);
    #[cfg_attr(rust_js, rust_js::name = "enableViewTransitionParentEnterExit")]
    pub static ENABLE_VIEW_TRANSITION_PARENT_ENTER_EXIT: bool = DYNAMIC_FEATURE_FLAGS.with(|flags| flags.enable_view_transition_parent_enter_exit);
}

// On WWW, __EXPERIMENTAL__ is used for a new modern build.
// It's not used anywhere in production yet.

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableProfilerTimer")]
    pub static ENABLE_PROFILER_TIMER: bool = __PROFILE__;
}
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableProfilerCommitHooks")]
    pub static ENABLE_PROFILER_COMMIT_HOOKS: bool = __PROFILE__;
}
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableProfilerNestedUpdatePhase")]
    pub static ENABLE_PROFILER_NESTED_UPDATE_PHASE: bool = __PROFILE__;
}
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableUpdaterTracking")]
    pub static ENABLE_UPDATER_TRACKING: bool = __PROFILE__;
}
#[cfg_attr(rust_js, rust_js::name = "enableSuspenseAvoidThisFallback")]
pub const ENABLE_SUSPENSE_AVOID_THIS_FALLBACK: bool = true;

#[cfg_attr(rust_js, rust_js::name = "enableCPUSuspense")]
pub const ENABLE_CPU_SUSPENSE: bool = true;
#[cfg_attr(rust_js, rust_js::name = "enableMoveBefore")]
pub const ENABLE_MOVE_BEFORE: bool = false;
#[cfg_attr(rust_js, rust_js::name = "disableInputAttributeSyncing")]
pub const DISABLE_INPUT_ATTRIBUTE_SYNCING: bool = false;
#[cfg_attr(rust_js, rust_js::name = "enableLegacyFBSupport")]
pub const ENABLE_LEGACY_FB_SUPPORT: bool = true;

#[cfg_attr(rust_js, rust_js::name = "enableYieldingBeforePassive")]
pub const ENABLE_YIELDING_BEFORE_PASSIVE: bool = false;

#[cfg_attr(rust_js, rust_js::name = "enableThrottledScheduling")]
pub const ENABLE_THROTTLED_SCHEDULING: bool = false;

#[cfg_attr(rust_js, rust_js::name = "enableComponentPerformanceTrack")]
pub const ENABLE_COMPONENT_PERFORMANCE_TRACK: bool = true;

#[cfg_attr(rust_js, rust_js::name = "enablePerformanceIssueReporting")]
pub const ENABLE_PERFORMANCE_ISSUE_REPORTING: bool = false;

// Logs additional User Timing API marks for use with an experimental profiling tool.
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableSchedulingProfiler")]
    pub static ENABLE_SCHEDULING_PROFILER: bool =
        __PROFILE__ && DYNAMIC_FEATURE_FLAGS.with(|flags| flags.enable_scheduling_profiler);
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "disableLegacyContext")]
    pub static DISABLE_LEGACY_CONTEXT: bool = __EXPERIMENTAL__;
}

#[cfg_attr(rust_js, rust_js::name = "enableLegacyCache")]
pub const ENABLE_LEGACY_CACHE: bool = true;

#[cfg_attr(rust_js, rust_js::name = "enableAsyncIterableChildren")]
pub const ENABLE_ASYNC_ITERABLE_CHILDREN: bool = false;
#[cfg_attr(rust_js, rust_js::name = "enableFlightWeakThenables")]
pub const ENABLE_FLIGHT_WEAK_THENABLES: bool = false;
#[cfg_attr(rust_js, rust_js::name = "enableFlightObjectReferences")]
pub const ENABLE_FLIGHT_OBJECT_REFERENCES: bool = false;

#[cfg_attr(rust_js, rust_js::name = "enableTaint")]
pub const ENABLE_TAINT: bool = false;

// TODO: www currently relies on this feature. It's disabled in open source.
// Need to remove it.
#[cfg_attr(rust_js, rust_js::name = "disableCommentsAsDOMContainers")]
pub const DISABLE_COMMENTS_AS_DOM_CONTAINERS: bool = false;

#[cfg_attr(rust_js, rust_js::name = "enableCreateEventHandleAPI")]
pub const ENABLE_CREATE_EVENT_HANDLE_API: bool = true;

#[cfg_attr(rust_js, rust_js::name = "enableScopeAPI")]
pub const ENABLE_SCOPE_API: bool = true;

#[cfg_attr(rust_js, rust_js::name = "enableSuspenseCallback")]
pub const ENABLE_SUSPENSE_CALLBACK: bool = true;

#[cfg_attr(rust_js, rust_js::name = "enableLegacyHidden")]
pub const ENABLE_LEGACY_HIDDEN: bool = true;

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "disableTextareaChildren")]
    pub static DISABLE_TEXTAREA_CHILDREN: bool = __EXPERIMENTAL__;
}

#[cfg_attr(rust_js, rust_js::name = "enableFizzExternalRuntime")]
pub const ENABLE_FIZZ_EXTERNAL_RUNTIME: bool = true;

#[cfg_attr(rust_js, rust_js::name = "passChildrenWhenCloningPersistedNodes")]
pub const PASS_CHILDREN_WHEN_CLONING_PERSISTED_NODES: bool = false;

#[cfg_attr(rust_js, rust_js::name = "disableClientCache")]
pub const DISABLE_CLIENT_CACHE: bool = true;

#[cfg_attr(rust_js, rust_js::name = "enableReactTestRendererWarning")]
pub const ENABLE_REACT_TEST_RENDERER_WARNING: bool = false;

#[cfg_attr(rust_js, rust_js::name = "disableLegacyMode")]
pub const DISABLE_LEGACY_MODE: bool = true;

#[cfg_attr(rust_js, rust_js::name = "enableViewTransitionForPersistenceMode")]
pub const ENABLE_VIEW_TRANSITION_FOR_PERSISTENCE_MODE: bool = false;

#[cfg_attr(rust_js, rust_js::name = "enableGestureTransition")]
pub const ENABLE_GESTURE_TRANSITION: bool = false;

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

// These flags are only used by eslint-plugin-react-hooks, which has its own
// fork at ReactFeatureFlags.eslint-plugin.www.js with the www-specific values.
// Edit that file to change the www values for these flags.
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
