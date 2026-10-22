// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use crate::environment::{__EXPERIMENTAL__, __PROFILE__};

// -----------------------------------------------------------------------------
// Land or remove (zero effort)
//
// Flags that can likely be deleted or landed without consequences
// -----------------------------------------------------------------------------

// None

// -----------------------------------------------------------------------------
// Killswitch
//
// Flags that exist solely to turn off a change in case it causes a regression
// when it rolls out to prod. We should remove these as soon as possible.
// -----------------------------------------------------------------------------

// None

// -----------------------------------------------------------------------------
// Land or remove (moderate effort)
//
// Flags that can be probably deleted or landed, but might require extra effort
// like migrating internal callers or performance testing.
// -----------------------------------------------------------------------------

// Need to remove didTimeout argument from Scheduler before landing
#[cfg_attr(rust_js, rust_js::name = "disableSchedulerTimeoutInWorkLoop")]
pub const DISABLE_SCHEDULER_TIMEOUT_IN_WORK_LOOP: bool = false;

// -----------------------------------------------------------------------------
// Slated for removal in the future (significant effort)
//
// These are experiments that didn't work out, and never shipped, but we can't
// delete from the codebase until we migrate internal callers.
// -----------------------------------------------------------------------------

// Add a callback property to suspense to notify which promises are currently
// in the update queue. This allows reporting and tracing of what is causing
// the user to see a loading state.
//
// Also allows hydration callbacks to fire when a dehydrated boundary gets
// hydrated or deleted.
//
// This will eventually be replaced by the Transition Tracing proposal.
#[cfg_attr(rust_js, rust_js::name = "enableSuspenseCallback")]
pub const ENABLE_SUSPENSE_CALLBACK: bool = false;

// Experimental Scope support.
#[cfg_attr(rust_js, rust_js::name = "enableScopeAPI")]
pub const ENABLE_SCOPE_API: bool = false;

// Experimental Create Event Handle API.
#[cfg_attr(rust_js, rust_js::name = "enableCreateEventHandleAPI")]
pub const ENABLE_CREATE_EVENT_HANDLE_API: bool = false;

// Support legacy Primer support on internal FB www
#[cfg_attr(rust_js, rust_js::name = "enableLegacyFBSupport")]
pub const ENABLE_LEGACY_FB_SUPPORT: bool = false;

// -----------------------------------------------------------------------------
// Ongoing experiments
//
// These are features that we're either actively exploring or are reasonably
// likely to include in an upcoming release.
// -----------------------------------------------------------------------------

// Yield to the browser event loop and not just the scheduler event loop before passive effects.
// Fix gated tests that fail with this flag enabled before turning it back on.
#[cfg_attr(rust_js, rust_js::name = "enableYieldingBeforePassive")]
pub const ENABLE_YIELDING_BEFORE_PASSIVE: bool = false;

// Experiment to intentionally yield less to block high framerate animations.
#[cfg_attr(rust_js, rust_js::name = "enableThrottledScheduling")]
pub const ENABLE_THROTTLED_SCHEDULING: bool = false;

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableLegacyCache")]
    pub static ENABLE_LEGACY_CACHE: bool = __EXPERIMENTAL__;
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableAsyncIterableChildren")]
    pub static ENABLE_ASYNC_ITERABLE_CHILDREN: bool = __EXPERIMENTAL__;
}

// Support thenables with status 'pending_weak' in Flight. A weak-pending
// thenable doesn't keep the stream open; if it resolves before the stream
// closes for other reasons, its value is emitted, otherwise its reference is
// left unfulfilled.
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableFlightWeakThenables")]
    pub static ENABLE_FLIGHT_WEAK_THENABLES: bool = __EXPERIMENTAL__;
}

// Support Server References that point to objects (including Promises), not
// just functions. On the server the reference is the real object; on the
// client it's an opaque handle that can only be passed back to the server,
// where it resolves via the manifest.
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableFlightObjectReferences")]
    pub static ENABLE_FLIGHT_OBJECT_REFERENCES: bool = __EXPERIMENTAL__;
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableTaint")]
    pub static ENABLE_TAINT: bool = __EXPERIMENTAL__;
}

#[cfg_attr(rust_js, rust_js::name = "enableViewTransition")]
pub const ENABLE_VIEW_TRANSITION: bool = true;

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableViewTransitionParentEnterExit")]
    pub static ENABLE_VIEW_TRANSITION_PARENT_ENTER_EXIT: bool = __EXPERIMENTAL__;
}

#[cfg_attr(rust_js, rust_js::name = "enableViewTransitionForPersistenceMode")]
pub const ENABLE_VIEW_TRANSITION_FOR_PERSISTENCE_MODE: bool = false;

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableGestureTransition")]
    pub static ENABLE_GESTURE_TRANSITION: bool = __EXPERIMENTAL__;
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableScrollEndPolyfill")]
    pub static ENABLE_SCROLL_END_POLYFILL: bool = __EXPERIMENTAL__;
}

#[cfg_attr(rust_js, rust_js::name = "enableSuspenseyImages")]
pub const ENABLE_SUSPENSEY_IMAGES: bool = false;

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableFizzBlockingRender")]
    pub static ENABLE_FIZZ_BLOCKING_RENDER: bool = __EXPERIMENTAL__; // rel="expect"
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableSrcObject")]
    pub static ENABLE_SRC_OBJECT: bool = __EXPERIMENTAL__;
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableHydrationChangeEvent")]
    pub static ENABLE_HYDRATION_CHANGE_EVENT: bool = __EXPERIMENTAL__;
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableDefaultTransitionIndicator")]
    pub static ENABLE_DEFAULT_TRANSITION_INDICATOR: bool = __EXPERIMENTAL__;
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableOptimisticKey")]
    pub static ENABLE_OPTIMISTIC_KEY: bool = __EXPERIMENTAL__;
}

/**
 * Switches Fiber creation to a simple object instead of a constructor.
 */
#[cfg_attr(rust_js, rust_js::name = "enableObjectFiber")]
pub const ENABLE_OBJECT_FIBER: bool = false;

#[cfg_attr(rust_js, rust_js::name = "enableTransitionTracing")]
pub const ENABLE_TRANSITION_TRACING: bool = false;

// FB-only usage. The new API has different semantics.
#[cfg_attr(rust_js, rust_js::name = "enableLegacyHidden")]
pub const ENABLE_LEGACY_HIDDEN: bool = false;

// Enables unstable_avoidThisFallback feature in Fiber
#[cfg_attr(rust_js, rust_js::name = "enableSuspenseAvoidThisFallback")]
pub const ENABLE_SUSPENSE_AVOID_THIS_FALLBACK: bool = false;

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableCPUSuspense")]
    pub static ENABLE_CPU_SUSPENSE: bool = __EXPERIMENTAL__;
}

// Test this at Meta before enabling.
#[cfg_attr(rust_js, rust_js::name = "enableNoCloningMemoCache")]
pub const ENABLE_NO_CLONING_MEMO_CACHE: bool = false;

// Test in www before enabling in open source.
// Enables DOM-server to stream its instruction set as data-attributes
// (handled with an MutationObserver) instead of inline-scripts
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableFizzExternalRuntime")]
    pub static ENABLE_FIZZ_EXTERNAL_RUNTIME: bool = __EXPERIMENTAL__;
}

#[cfg_attr(rust_js, rust_js::name = "alwaysThrottleRetries")]
pub const ALWAYS_THROTTLE_RETRIES: bool = true;

#[cfg_attr(rust_js, rust_js::name = "passChildrenWhenCloningPersistedNodes")]
pub const PASS_CHILDREN_WHEN_CLONING_PERSISTED_NODES: bool = false;

/**
 * Enables an expiration time for retry lanes to avoid starvation.
 */
#[cfg_attr(rust_js, rust_js::name = "enableRetryLaneExpiration")]
pub const ENABLE_RETRY_LANE_EXPIRATION: bool = false;
#[cfg_attr(rust_js, rust_js::name = "retryLaneExpirationMs")]
pub const RETRY_LANE_EXPIRATION_MS: f64 = 5000.0;
#[cfg_attr(rust_js, rust_js::name = "syncLaneExpirationMs")]
pub const SYNC_LANE_EXPIRATION_MS: f64 = 250.0;
#[cfg_attr(rust_js, rust_js::name = "transitionLaneExpirationMs")]
pub const TRANSITION_LANE_EXPIRATION_MS: f64 = 5000.0;

/**
 * Enables a new error detection for infinite render loops from updates caused
 * by setState or similar outside of the component owning the state.
 */
#[cfg_attr(rust_js, rust_js::name = "enableInfiniteRenderLoopDetection")]
pub const ENABLE_INFINITE_RENDER_LOOP_DETECTION: bool = false;
/**
 * When `enableInfiniteRenderLoopDetection` is on, forces the detection
 * mechanism to throw instead of only warning in cases where it would
 * otherwise downgrade to a warning.
 */
#[cfg_attr(rust_js, rust_js::name = "enableInfiniteRenderLoopDetectionForceThrow")]
pub const ENABLE_INFINITE_RENDER_LOOP_DETECTION_FORCE_THROW: bool = false;

#[cfg_attr(rust_js, rust_js::name = "enableConditionalUseWarning")]
pub const ENABLE_CONDITIONAL_USE_WARNING: bool = true;

#[cfg_attr(rust_js, rust_js::name = "enableInternalInstanceMap")]
pub const ENABLE_INTERNAL_INSTANCE_MAP: bool = false;

// -----------------------------------------------------------------------------
// Ready for next major.
//
// Alias __NEXT_MAJOR__ to __EXPERIMENTAL__ for easier skimming.
// -----------------------------------------------------------------------------

// TODO: Anything that's set to `true` in this section should either be cleaned
// up (if it's on everywhere, including Meta and RN builds) or moved to a
// different section of this file.

// const __NEXT_MAJOR__ = __EXPERIMENTAL__;

/**
 * Removes legacy style context defined using static `contextTypes` and consumed with static `childContextTypes`.
 */
#[cfg_attr(rust_js, rust_js::name = "disableLegacyContext")]
pub const DISABLE_LEGACY_CONTEXT: bool = true;
/**
 * Removes legacy style context just from function components.
 */
#[cfg_attr(rust_js, rust_js::name = "disableLegacyContextForFunctionComponents")]
pub const DISABLE_LEGACY_CONTEXT_FOR_FUNCTION_COMPONENTS: bool = true;

// Enable the moveBefore() alternative to insertBefore(). This preserves states of moves.
#[cfg_attr(rust_js, rust_js::name = "enableMoveBefore")]
pub const ENABLE_MOVE_BEFORE: bool = false;

// Disabled caching behavior of `react/cache` in client runtimes.
#[cfg_attr(rust_js, rust_js::name = "disableClientCache")]
pub const DISABLE_CLIENT_CACHE: bool = true;

// Warn on any usage of ReactTestRenderer
#[cfg_attr(rust_js, rust_js::name = "enableReactTestRendererWarning")]
pub const ENABLE_REACT_TEST_RENDERER_WARNING: bool = true;

// Disables legacy mode
// This allows us to land breaking changes to remove legacy mode APIs in experimental builds
// before removing them in stable in the next Major
#[cfg_attr(rust_js, rust_js::name = "disableLegacyMode")]
pub const DISABLE_LEGACY_MODE: bool = true;

// -----------------------------------------------------------------------------
// Chopping Block
//
// Planned feature deprecations and breaking changes. Sorted roughly in order of
// when we plan to enable them.
// -----------------------------------------------------------------------------

// -----------------------------------------------------------------------------
// React DOM Chopping Block
//
// Similar to main Chopping Block but only flags related to React DOM. These are
// grouped because we will likely batch all of them into a single major release.
// -----------------------------------------------------------------------------

// Disable support for comment nodes as React DOM containers. Already disabled
// in open source, but www codebase still relies on it. Need to remove.
#[cfg_attr(rust_js, rust_js::name = "disableCommentsAsDOMContainers")]
pub const DISABLE_COMMENTS_AS_DOM_CONTAINERS: bool = true;

// Prevent the value and checked attributes from syncing with their related
// DOM properties
#[cfg_attr(rust_js, rust_js::name = "disableInputAttributeSyncing")]
pub const DISABLE_INPUT_ATTRIBUTE_SYNCING: bool = false;

// Disables children for <textarea> elements
#[cfg_attr(rust_js, rust_js::name = "disableTextareaChildren")]
pub const DISABLE_TEXTAREA_CHILDREN: bool = false;

// -----------------------------------------------------------------------------
// Debugging and DevTools
// -----------------------------------------------------------------------------

// Gather advanced timing metrics for Profiler subtrees.
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableProfilerTimer")]
    pub static ENABLE_PROFILER_TIMER: bool = __PROFILE__;
}

// Adds performance.measure() marks using Chrome extensions to allow formatted
// Component rendering tracks to show up in the Performance tab.
// This flag will be used for both Server Component and Client Component tracks.
// All calls should also be gated on enableProfilerTimer.
#[cfg_attr(rust_js, rust_js::name = "enableComponentPerformanceTrack")]
pub const ENABLE_COMPONENT_PERFORMANCE_TRACK: bool = true;

// Enables annotating of React performance track events with `performanceIssue`
// metadata, to more prominently highlight performance issues to users
// (initially, an experimental feature in React Native).
#[cfg_attr(rust_js, rust_js::name = "enablePerformanceIssueReporting")]
pub const ENABLE_PERFORMANCE_ISSUE_REPORTING: bool = false;

// Adds user timing marks for e.g. state updates, suspense, and work loop stuff,
// for an experimental timeline tool.
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableSchedulingProfiler")]
    pub static ENABLE_SCHEDULING_PROFILER: bool = !ENABLE_COMPONENT_PERFORMANCE_TRACK && __PROFILE__;
}

// Record durations for commit and passive effects phases.
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableProfilerCommitHooks")]
    pub static ENABLE_PROFILER_COMMIT_HOOKS: bool = __PROFILE__;
}

// Phase param passed to onRender callback differentiates between an "update" and a "cascading-update".
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableProfilerNestedUpdatePhase")]
    pub static ENABLE_PROFILER_NESTED_UPDATE_PHASE: bool = __PROFILE__;
}

// Track which Fiber(s) schedule render work.
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "enableUpdaterTracking")]
    pub static ENABLE_UPDATER_TRACKING: bool = __PROFILE__;
}

#[cfg_attr(rust_js, rust_js::name = "ownerStackLimit")]
pub const OWNER_STACK_LIMIT: f64 = 1e4;

// -----------------------------------------------------------------------------
// eslint-plugin-react-hooks
// -----------------------------------------------------------------------------
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

/// Which effect dependencies the exhaustive-deps compiler lint reports.
pub enum CompilerLint {
    #[cfg_attr(rust_js, rust_js::name = "off")]
    Off,
    #[cfg_attr(rust_js, rust_js::name = "all")]
    All,
    #[cfg_attr(rust_js, rust_js::name = "extra-only")]
    ExtraOnly,
    #[cfg_attr(rust_js, rust_js::name = "missing-only")]
    MissingOnly,
}
