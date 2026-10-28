// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::rc::Rc;

use js::{
    Dict, JsError, JsObject, Kind, PropertyDescriptor, Set, SymbolFor, Unknown, WeakMap, WeakSet,
    function, object,
};
use react_isomorphic::react_lazy::Thenable;
use react_isomorphic::react_start_transition::{Transition, set_t, t};
use react_isomorphic::react_transition_type::TransitionTypes;
use shared::environment::__DEV__;
use shared::object_is::OBJECT_IS;
use shared::react_feature_flags::{
    ALWAYS_THROTTLE_RETRIES, DISABLE_LEGACY_CONTEXT, DISABLE_LEGACY_MODE,
    ENABLE_COMPONENT_PERFORMANCE_TRACK, ENABLE_CREATE_EVENT_HANDLE_API,
    ENABLE_DEFAULT_TRANSITION_INDICATOR, ENABLE_GESTURE_TRANSITION,
    ENABLE_INFINITE_RENDER_LOOP_DETECTION, ENABLE_INFINITE_RENDER_LOOP_DETECTION_FORCE_THROW,
    ENABLE_PROFILER_COMMIT_HOOKS, ENABLE_PROFILER_NESTED_UPDATE_PHASE, ENABLE_PROFILER_TIMER,
    ENABLE_SCHEDULING_PROFILER, ENABLE_THROTTLED_SCHEDULING, ENABLE_TRANSITION_TRACING,
    ENABLE_UPDATER_TRACKING, ENABLE_VIEW_TRANSITION, ENABLE_YIELDING_BEFORE_PASSIVE,
};
use shared::react_owner_stack_reset::reset_owner_stack_limit;
use shared::react_shared_internals::{
    AsyncDispatcher, Dispatcher as SharedDispatcher, REACT_SHARED_INTERNALS,
};
use shared::react_symbols::REACT_STRICT_MODE_TYPE;
use shared::report_global_error::REPORT_GLOBAL_ERROR;
use webapi::{ConsoleTask, console};

use crate::get_component_name_from_fiber::get_component_name_from_fiber;
use crate::react_captured_value::{CapturedValue, create_captured_value_at_fiber};
use crate::react_child_fiber::reset_child_reconciler_on_unwind;
use crate::react_current_fiber::{IS_RENDERING, reset_current_fiber, run_with_fiber_in_dev};
use crate::react_event_priorities::{
    DEFAULT_EVENT_PRIORITY, DISCRETE_EVENT_PRIORITY, EventPriority, event_priority_to_lane,
    lanes_to_event_priority, lower_event_priority,
};
use crate::react_fiber::{ViewTransitionState, create_work_in_progress, reset_work_in_progress};
use crate::react_fiber_act::{is_concurrent_act_environment, is_legacy_act_environment};
use crate::react_fiber_apply_gesture::{
    apply_departure_transitions, insert_destination_clones, start_gesture_animations,
};
use crate::react_fiber_async_dispatcher::DEFAULT_ASYNC_DISPATCHER;
use crate::react_fiber_begin_work::{
    SELECTIVE_HYDRATION_EXCEPTION, begin_work, replay_function_component,
};
use crate::react_fiber_cache_component::release_cache;
use crate::react_fiber_class_update_queue::enqueue_update;
use crate::react_fiber_commit_view_transitions::{
    SHOULD_START_VIEW_TRANSITION, reset_should_start_view_transition,
};
use crate::react_fiber_commit_work::{
    SHOULD_FIRE_AFTER_ACTIVE_INSTANCE_BLUR, accumulate_suspensey_commit,
    commit_after_mutation_effects, commit_before_mutation_effects, commit_layout_effects,
    commit_mutation_effects, commit_passive_mount_effects, commit_passive_unmount_effects,
    disappear_layout_effects_for_dev_validation, disconnect_passive_effect,
    invoke_layout_effect_mount_in_dev, invoke_layout_effect_unmount_in_dev,
    invoke_passive_effect_mount_in_dev, invoke_passive_effect_unmount_in_dev,
    reappear_layout_effects_for_dev_validation, reconnect_passive_effects,
};
use crate::react_fiber_complete_work::complete_work;
use crate::react_fiber_concurrent_updates::{
    enqueue_concurrent_render_for_lane, finish_queueing_concurrent_updates,
    get_concurrently_updated_lanes,
};
#[cfg_attr(rust_js, rust_js::name = "onCommitRootDevTools")]
use crate::react_fiber_dev_tools_hook::on_commit_root as on_commit_root_dev_tools;
#[cfg_attr(rust_js, rust_js::name = "onPostCommitRootDevTools")]
use crate::react_fiber_dev_tools_hook::on_post_commit_root as on_post_commit_root_dev_tools;
use crate::react_fiber_dev_tools_hook::{
    IS_DEV_TOOLS_PRESENT, mark_commit_started, mark_commit_stopped, mark_component_errored,
    mark_component_render_stopped, mark_component_suspended, mark_layout_effects_started,
    mark_layout_effects_stopped, mark_passive_effects_started, mark_passive_effects_stopped,
    mark_render_started, mark_render_stopped, mark_render_yielded, set_is_strict_mode_for_devtools,
};
use crate::react_fiber_error_logger::log_uncaught_error;
use crate::react_fiber_flags::{
    BEFORE_MUTATION_MASK, DID_DEFER, FORCE_CLIENT_RENDER, Flags, HOST_EFFECT_MASK, INCOMPLETE,
    LAYOUT_MASK, MAY_SUSPEND_COMMIT, MOUNT_LAYOUT_DEV, MOUNT_PASSIVE_DEV, MUTATION_MASK, NO_FLAGS,
    PASSIVE_MASK, PASSIVE_TRANSITION_MASK, PLACEMENT_DEV, SCHEDULE_RETRY, SHOULD_SUSPEND_COMMIT,
    STORE_CONSISTENCY, VISIBILITY,
};
use crate::react_fiber_gesture_scheduler::{schedule_gesture_commit, stop_committed_gesture};
use crate::react_fiber_hooks::{
    CONTEXT_ONLY_DISPATCHER, FunctionComponentUpdateQueue, reset_hooks_after_throw,
    reset_hooks_on_unwind,
};
use crate::react_fiber_hydration_context::{
    get_is_hydrating, pop_hydration_state_on_interrupted_work,
};
#[cfg_attr(rust_js, rust_js::name = "_markRootPinged")]
use crate::react_fiber_lane::mark_root_pinged as _mark_root_pinged;
#[cfg_attr(rust_js, rust_js::name = "_markRootSuspended")]
use crate::react_fiber_lane::mark_root_suspended as _mark_root_suspended;
#[cfg_attr(rust_js, rust_js::name = "_markRootUpdated")]
use crate::react_fiber_lane::mark_root_updated as _mark_root_updated;
use crate::react_fiber_lane::{
    GESTURE_LANE, IDLE_LANE, Lane, Lanes, NO_LANE, NO_LANES, OFFSCREEN_LANE, SOME_RETRY_LANE,
    SOME_TRANSITION_LANE, SYNC_LANE, SYNC_UPDATE_LANES, UPDATE_LANES, add_fiber_to_lanes_map,
    add_transition_to_lanes_map, check_if_root_is_prerendering, claim_next_retry_lane,
    claim_next_transition_deferred_lane, get_entangled_lanes,
    get_lanes_to_retry_synchronously_on_error, get_next_lanes, get_transitions_for_lanes,
    includes_blocking_lane, includes_expired_lane, includes_idle_group_lanes,
    includes_non_idle_work, includes_only_retries, includes_only_transitions,
    includes_only_view_transition_eligible_lanes, includes_retry_lane, includes_some_lane,
    includes_sync_lane, includes_transition_lane, is_gesture_render, is_subset_of_lanes,
    mark_root_finished, merge_lanes, move_pending_fibers_to_memoized, pick_arbitrary_lane,
    remove_lanes, upgrade_pending_lanes_to_sync,
};
use crate::react_fiber_legacy_context::{get_masked_context, get_unmasked_context};
use crate::react_fiber_new_context::reset_context_dependencies;
use crate::react_fiber_offscreen_component::OffscreenInstance;
use crate::react_fiber_performance_track::{
    log_action_yield_time, log_animating_phase, log_apply_gesture_phase, log_blocking_start,
    log_commit_phase, log_errored_render_phase, log_gesture_start, log_inconsistent_render,
    log_interrupted_render_phase, log_paint_yield_phase, log_passive_commit_phase,
    log_recovered_render_phase, log_render_phase, log_start_view_transition_yield_phase,
    log_suspended_commit_phase, log_suspended_render_phase, log_suspended_view_transition_phase,
    log_suspended_with_delay_phase, log_suspended_yield_time, log_transition_start, log_yield_time,
    mark_all_lanes_in_order, set_current_track_from_lanes,
};
use crate::react_fiber_root::RootState;
use crate::react_fiber_root_scheduler::{
    ensure_root_is_scheduled, flush_sync_work_on_all_roots, flush_sync_work_on_legacy_roots_only,
    request_transition_lane,
};
use crate::react_fiber_shell_hydration::is_root_dehydrated;
use crate::react_fiber_suspense_component::{SuspenseState, Wakeable};
use crate::react_fiber_suspense_context::{get_shell_boundary, get_suspense_handler};
use crate::react_fiber_thenable::{
    SUSPENSE_ACTION_EXCEPTION, SUSPENSE_EXCEPTION, SUSPENSEY_COMMIT_EXCEPTION, clear_use_warnings,
    get_suspended_thenable, has_potential_use_warnings, is_thenable_resolved,
};
use crate::react_fiber_throw::{
    create_class_error_update, create_root_error_update, initialize_class_error_update,
    throw_exception,
};
use crate::react_fiber_tracing_marker_component::{
    MarkerIncomplete, MarkerProgress, PendingBoundaries, PendingTransitionCallbacks,
    TransitionAborts, process_transition_callbacks,
};
use crate::react_fiber_transition::request_current_transition;
use crate::react_fiber_transition_types::claim_queued_transition_types;
use crate::react_fiber_unwind_work::{unwind_interrupted_work, unwind_work};
use crate::react_fiber_view_transition_component::{
    GestureOptionsRequired, ViewTransitionCallback, ViewTransitionGestureCallback,
    get_view_transition_name,
};
use crate::react_internal_types::{ErrorInfo, Fiber, FiberRoot};
use crate::react_post_paint_callback::schedule_post_paint_callback;
use crate::react_profiler_timer::{
    ANIMATING_LANES, ANIMATING_TASK, BLOCKING_CLAMP_TIME, BLOCKING_EVENT_REPEAT_TIME,
    BLOCKING_EVENT_TIME, BLOCKING_EVENT_TYPE, BLOCKING_SUSPENDED_TIME,
    BLOCKING_UPDATE_COMPONENT_NAME, BLOCKING_UPDATE_METHOD_NAME, BLOCKING_UPDATE_TASK,
    BLOCKING_UPDATE_TIME, BLOCKING_UPDATE_TYPE, COMMIT_END_TIME, COMMIT_ERRORS, COMMIT_START_TIME,
    CaptureErrors, GESTURE_CLAMP_TIME, GESTURE_EVENT_REPEAT_TIME, GESTURE_EVENT_TIME,
    GESTURE_EVENT_TYPE, GESTURE_SUSPENDED_TIME, GESTURE_UPDATE_COMPONENT_NAME,
    GESTURE_UPDATE_METHOD_NAME, GESTURE_UPDATE_TASK, GESTURE_UPDATE_TIME, GESTURE_UPDATE_TYPE,
    IDLE_CLAMP_TIME, PINGED_UPDATE, RENDER_START_TIME, RETRY_CLAMP_TIME, SPAWNED_UPDATE,
    TRANSITION_CLAMP_TIME, TRANSITION_EVENT_REPEAT_TIME, TRANSITION_EVENT_TIME,
    TRANSITION_EVENT_TYPE, TRANSITION_START_TIME, TRANSITION_SUSPENDED_TIME,
    TRANSITION_UPDATE_COMPONENT_NAME, TRANSITION_UPDATE_METHOD_NAME, TRANSITION_UPDATE_TASK,
    TRANSITION_UPDATE_TIME, TRANSITION_UPDATE_TYPE, YIELD_REASON, YIELD_START_TIME,
    clamp_blocking_timers, clamp_gesture_timers, clamp_idle_timers, clamp_retry_timers,
    clamp_transition_timers, clear_blocking_timers, clear_gesture_timers, clear_gesture_updates,
    clear_transition_timers, mark_nested_update_scheduled, record_commit_end_time,
    record_commit_time, record_effect_error, record_render_time, reset_commit_errors,
    start_animating, start_ping_timer_by_lanes, start_profiler_timer, start_yield_timer,
    stop_animating, stop_profiler_timer_if_running_and_record_duration,
    stop_profiler_timer_if_running_and_record_incomplete_duration, track_suspended_time,
};
use crate::react_root_tags::{CONCURRENT_ROOT, LEGACY_ROOT};
use crate::react_strict_mode_warnings::REACT_STRICT_MODE_WARNINGS;
#[cfg_attr(rust_js, rust_js::name = "onCommitRootTestSelector")]
use crate::react_test_selectors::on_commit_root as on_commit_root_test_selector;
use crate::react_type_of_mode::{
    CONCURRENT_MODE, NO_MODE, PROFILE_MODE, STRICT_EFFECTS_MODE, STRICT_LEGACY_MODE,
};
use crate::react_work_tags::{
    ACTIVITY_COMPONENT, CLASS_COMPONENT, FORWARD_REF, FUNCTION_COMPONENT, HOST_COMPONENT,
    HOST_HOISTABLE, HOST_ROOT, HOST_SINGLETON, MEMO_COMPONENT, OFFSCREEN_COMPONENT,
    SIMPLE_MEMO_COMPONENT, SUSPENSE_COMPONENT, SUSPENSE_LIST_COMPONENT,
};
#[cfg_attr(rust_js, rust_js::name = "IdleSchedulerPriority")]
use crate::scheduler::IDLE_PRIORITY as IDLE_SCHEDULER_PRIORITY;
#[cfg_attr(rust_js, rust_js::name = "NormalSchedulerPriority")]
use crate::scheduler::NORMAL_PRIORITY as NORMAL_SCHEDULER_PRIORITY;
#[cfg_attr(rust_js, rust_js::name = "Scheduler_scheduleCallback")]
use crate::scheduler::SCHEDULE_CALLBACK as SCHEDULER_SCHEDULE_CALLBACK;
use crate::scheduler::{NOW as now, PriorityLevel, REQUEST_PAINT, SHOULD_YIELD, SchedulerCallback};

unsafe extern "Rust" {
    #[link_name = "./src/ReactFiberConfig.js#supportsHydration"]
    safe static SUPPORTS_HYDRATION: bool;
    // ReactFiberConfig's `TimeoutHandle`'s none.
    #[link_name = "./src/ReactFiberConfig.js#noTimeout"]
    safe static NO_TIMEOUT: &'static Unknown;
}

// ReactFiberConfig's host operations, on its `Container`, `Instance`,
// `Resource`, `SuspendedState`, `TimeoutHandle`, `GestureTimeline`,
// `ViewTransitionInstance` and `RunningViewTransition`.
#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#resetAfterCommit"
)]
fn reset_after_commit(container_info: &'static Unknown) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#scheduleTimeout"
)]
fn schedule_timeout(handler: Box<dyn Fn()>, timeout: f64) -> &'static Unknown {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#cancelTimeout"
)]
fn cancel_timeout(handle: &'static Unknown) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#afterActiveInstanceBlur"
)]
fn after_active_instance_blur() {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#startSuspendingCommit"
)]
fn start_suspending_commit() -> &'static Unknown {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#suspendOnActiveViewTransition"
)]
fn suspend_on_active_view_transition(state: &'static Unknown, root_container: &'static Unknown) {
    unreachable!()
}

/// What `waitForCommitToBeReady` gives where the commit isn't ready: it's
/// given the commit, and gives what cancels it.
type SchedulePendingCommit = dyn Fn(Box<dyn Fn()>) -> Box<dyn Fn()>;

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#waitForCommitToBeReady",
    rust_js::nullable
)]
fn wait_for_commit_to_be_ready(
    state: &'static Unknown,
    timeout_offset: f64,
) -> Option<&'static SchedulePendingCommit> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#getSuspendedCommitReason",
    rust_js::nullable
)]
fn get_suspended_commit_reason(
    state: &'static Unknown,
    root_container: &'static Unknown,
) -> Option<&'static str> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#preloadInstance"
)]
fn preload_instance(
    instance: Option<&'static Unknown>,
    r#type: Option<&'static Unknown>,
    props: Option<&'static Unknown>,
) -> bool {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#preloadResource"
)]
fn preload_resource(resource: &'static Unknown) -> bool {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#setCurrentUpdatePriority"
)]
fn set_current_update_priority(new_priority: EventPriority) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#getCurrentUpdatePriority"
)]
fn get_current_update_priority() -> EventPriority {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#resolveUpdatePriority"
)]
fn resolve_update_priority() -> EventPriority {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#trackSchedulerEvent"
)]
fn track_scheduler_event() {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#startViewTransition",
    rust_js::nullable,
    rust_js::nullable(
        suspended_state,
        transition_types,
        blocked_callback,
        finished_animation
    )
)]
fn start_view_transition(
    suspended_state: Option<&'static Unknown>,
    root_container: &'static Unknown,
    transition_types: Option<Rc<RefCell<TransitionTypes>>>,
    mutation_callback: fn(),
    layout_callback: fn(),
    after_mutation_callback: fn(),
    spawned_work_callback: fn(),
    passive_callback: fn() -> bool,
    error_callback: fn(Option<&'static Unknown>),
    blocked_callback: Option<fn(&'static str)>, // Profiling-only
    finished_animation: Option<Box<dyn Fn()>>,  // Profiling-only
) -> Option<&'static Unknown> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#startGestureTransition",
    rust_js::nullable,
    rust_js::nullable(suspended_state, transition_types, finished_animation)
)]
fn start_gesture_transition(
    suspended_state: Option<&'static Unknown>,
    root_container: &'static Unknown,
    timeline: &'static Unknown,
    range_start: f64,
    range_end: f64,
    transition_types: Option<Rc<RefCell<TransitionTypes>>>,
    mutation_callback: fn(),
    animate_callback: fn(),
    error_callback: fn(Option<&'static Unknown>),
    finished_animation: Option<Box<dyn Fn()>>, // Profiling-only
) -> Option<&'static Unknown> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#stopViewTransition"
)]
fn stop_view_transition(transition: &'static Unknown) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#addViewTransitionFinishedListener"
)]
fn add_view_transition_finished_listener(
    transition: &'static Unknown,
    callback: &'static dyn Fn(),
) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#createViewTransitionInstance"
)]
fn create_view_transition_instance(name: &str) -> &'static Unknown {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#flushHydrationEvents"
)]
fn flush_hydration_events() {
    unreachable!()
}

/// A root's ping cache: the lanes each wakeable has a ping listener for.
type PingCache = WeakMap<Wakeable, &'static Set<Lanes>>;

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "PossiblyWeakMap")]
    static POSSIBLY_WEAK_MAP: &'static js::Class =
        if js::is_function(js::weak_map::new::<Wakeable, &'static Set<Lanes>>) {
            js::class::<PingCache>()
        } else {
            js::class::<js::Map<Wakeable, &'static Set<Lanes>>>()
        };
}

type ExecutionContext = u8;

#[cfg_attr(rust_js, rust_js::name = "NoContext")]
pub const NO_CONTEXT: ExecutionContext = /*             */ 0b000;
#[cfg_attr(rust_js, rust_js::name = "BatchedContext")]
const BATCHED_CONTEXT: ExecutionContext = /*               */ 0b001;
#[cfg_attr(rust_js, rust_js::name = "RenderContext")]
pub const RENDER_CONTEXT: ExecutionContext = /*         */ 0b010;
#[cfg_attr(rust_js, rust_js::name = "CommitContext")]
pub const COMMIT_CONTEXT: ExecutionContext = /*         */ 0b100;

type RootExitStatus = u8;
#[cfg_attr(rust_js, rust_js::name = "RootInProgress")]
const ROOT_IN_PROGRESS: RootExitStatus = 0;
#[cfg_attr(rust_js, rust_js::name = "RootFatalErrored")]
const ROOT_FATAL_ERRORED: RootExitStatus = 1;
#[cfg_attr(rust_js, rust_js::name = "RootErrored")]
const ROOT_ERRORED: RootExitStatus = 2;
#[cfg_attr(rust_js, rust_js::name = "RootSuspended")]
const ROOT_SUSPENDED: RootExitStatus = 3;
#[cfg_attr(rust_js, rust_js::name = "RootSuspendedWithDelay")]
const ROOT_SUSPENDED_WITH_DELAY: RootExitStatus = 4;
#[cfg_attr(rust_js, rust_js::name = "RootSuspendedAtTheShell")]
const ROOT_SUSPENDED_AT_THE_SHELL: RootExitStatus = 6;
#[cfg_attr(rust_js, rust_js::name = "RootCompleted")]
const ROOT_COMPLETED: RootExitStatus = 5;

thread_local! {
    // Describes where we are in the React execution stack
    static EXECUTION_CONTEXT: Cell<ExecutionContext> = const { Cell::new(NO_CONTEXT) };
    // The root we're working on
    #[cfg_attr(rust_js, rust_js::nullable)]
    static WORK_IN_PROGRESS_ROOT: RefCell<Option<Rc<FiberRoot>>> = const { RefCell::new(js::null()) };
    // The fiber we're working on
    #[cfg_attr(rust_js, rust_js::nullable)]
    static WORK_IN_PROGRESS: RefCell<Option<Rc<Fiber>>> = const { RefCell::new(js::null()) };
    // The lanes we're rendering
    static WORK_IN_PROGRESS_ROOT_RENDER_LANES: Cell<Lanes> = const { Cell::new(NO_LANES) };
}

pub type SuspendedReason = u8;
#[cfg_attr(rust_js, rust_js::name = "NotSuspended")]
const NOT_SUSPENDED: SuspendedReason = 0;
#[cfg_attr(rust_js, rust_js::name = "SuspendedOnError")]
const SUSPENDED_ON_ERROR: SuspendedReason = 1;
#[cfg_attr(rust_js, rust_js::name = "SuspendedOnData")]
const SUSPENDED_ON_DATA: SuspendedReason = 2;
#[cfg_attr(rust_js, rust_js::name = "SuspendedOnImmediate")]
const SUSPENDED_ON_IMMEDIATE: SuspendedReason = 3;
#[cfg_attr(rust_js, rust_js::name = "SuspendedOnInstance")]
const SUSPENDED_ON_INSTANCE: SuspendedReason = 4;
#[cfg_attr(rust_js, rust_js::name = "SuspendedOnInstanceAndReadyToContinue")]
const SUSPENDED_ON_INSTANCE_AND_READY_TO_CONTINUE: SuspendedReason = 5;
#[cfg_attr(rust_js, rust_js::name = "SuspendedOnDeprecatedThrowPromise")]
const SUSPENDED_ON_DEPRECATED_THROW_PROMISE: SuspendedReason = 6;
#[cfg_attr(rust_js, rust_js::name = "SuspendedAndReadyToContinue")]
const SUSPENDED_AND_READY_TO_CONTINUE: SuspendedReason = 7;
#[cfg_attr(rust_js, rust_js::name = "SuspendedOnHydration")]
const SUSPENDED_ON_HYDRATION: SuspendedReason = 8;
#[cfg_attr(rust_js, rust_js::name = "SuspendedOnAction")]
const SUSPENDED_ON_ACTION: SuspendedReason = 9;

thread_local! {
    // When this is true, the work-in-progress fiber just suspended (or errored) and
    // we've yet to unwind the stack. In some cases, we may yield to the main thread
    // after this happens. If the fiber is pinged before we resume, we can retry
    // immediately instead of unwinding the stack.
    static WORK_IN_PROGRESS_SUSPENDED_REASON: Cell<SuspendedReason> = const { Cell::new(NOT_SUSPENDED) };
    static WORK_IN_PROGRESS_THROWN_VALUE: Cell<Option<&'static Unknown>> = const { Cell::new(js::null()) };

    // Tracks whether any siblings were skipped during the unwind phase after
    // something suspends. Used to determine whether to schedule another render
    // to prewarm the skipped siblings.
    static WORK_IN_PROGRESS_ROOT_DID_SKIP_SUSPENDED_SIBLINGS: Cell<bool> = const { Cell::new(false) };
    // Whether the work-in-progress render is the result of a prewarm/prerender.
    // This tells us whether or not we should render the siblings after
    // something suspends.
    static WORK_IN_PROGRESS_ROOT_IS_PRERENDERING: Cell<bool> = const { Cell::new(false) };

    // Whether a ping listener was attached during this render. This is slightly
    // different that whether something suspended, because we don't add multiple
    // listeners to a promise we've already seen (per root and lane).
    static WORK_IN_PROGRESS_ROOT_DID_ATTACH_PING_LISTENER: Cell<bool> = const { Cell::new(false) };

    // A contextual version of workInProgressRootRenderLanes. It is a superset of
    // the lanes that we started working on at the root. When we enter a subtree
    // that is currently hidden, we add the lanes that would have committed if
    // the hidden tree hadn't been deferred. This is modified by the
    // HiddenContext module.
    //
    // Most things in the work loop should deal with workInProgressRootRenderLanes.
    // Most things in begin/complete phases should deal with entangledRenderLanes.
    pub static ENTANGLED_RENDER_LANES: Cell<Lanes> = const { Cell::new(NO_LANES) };

    // Whether to root completed, errored, suspended, etc.
    static WORK_IN_PROGRESS_ROOT_EXIT_STATUS: Cell<RootExitStatus> = const { Cell::new(ROOT_IN_PROGRESS) };
    // The work left over by components that were visited during this render. Only
    // includes unprocessed updates, not work in bailed out children.
    static WORK_IN_PROGRESS_ROOT_SKIPPED_LANES: Cell<Lanes> = const { Cell::new(NO_LANES) };
    // Lanes that were updated (in an interleaved event) during this render.
    static WORK_IN_PROGRESS_ROOT_INTERLEAVED_UPDATED_LANES: Cell<Lanes> = const { Cell::new(NO_LANES) };
    // Lanes that were updated during the render phase (*not* an interleaved event).
    static WORK_IN_PROGRESS_ROOT_RENDER_PHASE_UPDATED_LANES: Cell<Lanes> = const { Cell::new(NO_LANES) };
    // Lanes that were pinged (in an interleaved event) during this render.
    static WORK_IN_PROGRESS_ROOT_PINGED_LANES: Cell<Lanes> = const { Cell::new(NO_LANES) };
    // If this render scheduled deferred work, this is the lane of the deferred task.
    static WORK_IN_PROGRESS_DEFERRED_LANE: Cell<Lane> = const { Cell::new(NO_LANE) };
    // Represents the retry lanes that were spawned by this render and have not
    // been pinged since, implying that they are still suspended.
    static WORK_IN_PROGRESS_SUSPENDED_RETRY_LANES: Cell<Lanes> = const { Cell::new(NO_LANES) };
    // Errors that are thrown during the render phase.
    #[cfg_attr(rust_js, rust_js::nullable)]
    static WORK_IN_PROGRESS_ROOT_CONCURRENT_ERRORS: RefCell<Option<CaptureErrors>> = const { RefCell::new(js::null()) };
    // These are errors that we recovered from without surfacing them to the UI.
    // We will log them once the tree commits.
    #[cfg_attr(rust_js, rust_js::nullable)]
    static WORK_IN_PROGRESS_ROOT_RECOVERABLE_ERRORS: RefCell<Option<CaptureErrors>> = const { RefCell::new(js::null()) };

    // Tracks when an update occurs during the render phase.
    static WORK_IN_PROGRESS_ROOT_DID_INCLUDE_RECURSIVE_RENDER_UPDATE: Cell<bool> = const { Cell::new(false) };
    // Thacks when an update occurs during the commit phase. It's a separate
    // variable from the one for renders because the commit phase may run
    // concurrently to a render phase.
    static DID_INCLUDE_COMMIT_PHASE_UPDATE: Cell<bool> = const { Cell::new(false) };
    // The most recent time we either committed a fallback, or when a fallback was
    // filled in with the resolved UI. This lets us throttle the appearance of new
    // content as it streams in, to minimize jank.
    // TODO: Think of a better name for this variable?
    static GLOBAL_MOST_RECENT_FALLBACK_TIME: Cell<f64> = const { Cell::new(0.0) };
    // Track the most recent time we started a new Transition. This lets us apply
    // heuristics like the suspensey image timeout based on how long we've waited
    // already.
    static GLOBAL_MOST_RECENT_TRANSITION_TIME: Cell<f64> = const { Cell::new(0.0) };
}

const FALLBACK_THROTTLE_MS: f64 = 300.0;

thread_local! {
    // The absolute time for when we should start giving up on rendering
    // more and prefer CPU suspense heuristics instead.
    static WORK_IN_PROGRESS_ROOT_RENDER_TARGET_TIME: Cell<f64> = const { Cell::new(f64::INFINITY) };
}
// How long a render is supposed to take before we start following CPU
// suspense heuristics and opt out of rendering more content.
const RENDER_TIMEOUT_MS: f64 = 500.0;

thread_local! {
    #[cfg_attr(rust_js, rust_js::nullable)]
    static WORK_IN_PROGRESS_TRANSITIONS: RefCell<Option<Rc<Vec<&'static Transition>>>> = const { RefCell::new(js::null()) };
}
#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_work_in_progress_transitions() -> Option<Rc<Vec<&'static Transition>>> {
    WORK_IN_PROGRESS_TRANSITIONS.with_borrow(Clone::clone)
}

thread_local! {
    // The first setState call that eventually caused the current render.
    #[cfg_attr(rust_js, rust_js::nullable)]
    static WORK_IN_PROGRESS_UPDATE_TASK: Cell<Option<&'static ConsoleTask>> = const { Cell::new(js::null()) };

    #[cfg_attr(rust_js, rust_js::nullable)]
    static CURRENT_PENDING_TRANSITION_CALLBACKS: RefCell<Option<PendingTransitionCallbacks>> = const { RefCell::new(js::null()) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static CURRENT_END_TIME: Cell<Option<f64>> = const { Cell::new(js::null()) };
}

pub fn add_transition_start_callback_to_pending_transition(transition: &'static Transition) {
    if ENABLE_TRANSITION_TRACING {
        CURRENT_PENDING_TRANSITION_CALLBACKS.with_borrow_mut(
            |current_pending_transition_callbacks| {
                let current_pending_transition_callbacks = current_pending_transition_callbacks
                    .get_or_insert_with(|| PendingTransitionCallbacks {
                        transition_start: Some(vec![]),
                        transition_progress: js::null(),
                        transition_complete: js::null(),
                        marker_progress: js::null(),
                        marker_incomplete: js::null(),
                        marker_complete: js::null(),
                    });

                current_pending_transition_callbacks
                    .transition_start
                    .get_or_insert_with(Vec::new)
                    .push(transition);
            },
        );
    }
}

pub fn add_marker_progress_callback_to_pending_transition(
    marker_name: &str,
    transitions: &'static Set<Transition>,
    pending_boundaries: &'static PendingBoundaries,
) {
    if ENABLE_TRANSITION_TRACING {
        CURRENT_PENDING_TRANSITION_CALLBACKS.with_borrow_mut(
            |current_pending_transition_callbacks| {
                let current_pending_transition_callbacks = current_pending_transition_callbacks
                    .get_or_insert_with(|| PendingTransitionCallbacks {
                        transition_start: js::null(),
                        transition_progress: js::null(),
                        transition_complete: js::null(),
                        marker_progress: Some(js::map::new(None)),
                        marker_incomplete: js::null(),
                        marker_complete: js::null(),
                    });

                current_pending_transition_callbacks
                    .marker_progress
                    .get_or_insert_with(|| js::map::new(None))
                    .set(
                        marker_name,
                        MarkerProgress {
                            pending_boundaries: Some(pending_boundaries),
                            transitions: Some(transitions),
                        },
                    );
            },
        );
    }
}

pub fn add_marker_incomplete_callback_to_pending_transition(
    marker_name: &str,
    transitions: &'static Set<Transition>,
    aborts: &TransitionAborts,
) {
    if ENABLE_TRANSITION_TRACING {
        CURRENT_PENDING_TRANSITION_CALLBACKS.with_borrow_mut(
            |current_pending_transition_callbacks| {
                let current_pending_transition_callbacks = current_pending_transition_callbacks
                    .get_or_insert_with(|| PendingTransitionCallbacks {
                        transition_start: js::null(),
                        transition_progress: js::null(),
                        transition_complete: js::null(),
                        marker_progress: js::null(),
                        marker_incomplete: Some(js::map::new(None)),
                        marker_complete: js::null(),
                    });

                current_pending_transition_callbacks
                    .marker_incomplete
                    .get_or_insert_with(|| js::map::new(None))
                    .set(
                        marker_name,
                        MarkerIncomplete {
                            transitions,
                            aborts: Rc::clone(aborts),
                        },
                    );
            },
        );
    }
}

pub fn add_marker_complete_callback_to_pending_transition(
    marker_name: &str,
    transitions: &'static Set<Transition>,
) {
    if ENABLE_TRANSITION_TRACING {
        CURRENT_PENDING_TRANSITION_CALLBACKS.with_borrow_mut(
            |current_pending_transition_callbacks| {
                let current_pending_transition_callbacks = current_pending_transition_callbacks
                    .get_or_insert_with(|| PendingTransitionCallbacks {
                        transition_start: js::null(),
                        transition_progress: js::null(),
                        transition_complete: js::null(),
                        marker_progress: js::null(),
                        marker_incomplete: js::null(),
                        marker_complete: Some(js::map::new(None)),
                    });

                current_pending_transition_callbacks
                    .marker_complete
                    .get_or_insert_with(|| js::map::new(None))
                    .set(marker_name, transitions);
            },
        );
    }
}

pub fn add_transition_progress_callback_to_pending_transition(
    transition: &'static Transition,
    boundaries: &'static PendingBoundaries,
) {
    if ENABLE_TRANSITION_TRACING {
        CURRENT_PENDING_TRANSITION_CALLBACKS.with_borrow_mut(
            |current_pending_transition_callbacks| {
                let current_pending_transition_callbacks = current_pending_transition_callbacks
                    .get_or_insert_with(|| PendingTransitionCallbacks {
                        transition_start: js::null(),
                        transition_progress: Some(js::map::new(None)),
                        transition_complete: js::null(),
                        marker_progress: js::null(),
                        marker_incomplete: js::null(),
                        marker_complete: js::null(),
                    });

                current_pending_transition_callbacks
                    .transition_progress
                    .get_or_insert_with(|| js::map::new(None))
                    .set(transition, boundaries);
            },
        );
    }
}

pub fn add_transition_complete_callback_to_pending_transition(transition: &'static Transition) {
    if ENABLE_TRANSITION_TRACING {
        CURRENT_PENDING_TRANSITION_CALLBACKS.with_borrow_mut(
            |current_pending_transition_callbacks| {
                let current_pending_transition_callbacks = current_pending_transition_callbacks
                    .get_or_insert_with(|| PendingTransitionCallbacks {
                        transition_start: js::null(),
                        transition_progress: js::null(),
                        transition_complete: Some(vec![]),
                        marker_progress: js::null(),
                        marker_incomplete: js::null(),
                        marker_complete: js::null(),
                    });

                current_pending_transition_callbacks
                    .transition_complete
                    .get_or_insert_with(Vec::new)
                    .push(transition);
            },
        );
    }
}

fn reset_render_timer() {
    WORK_IN_PROGRESS_ROOT_RENDER_TARGET_TIME.set(now() + RENDER_TIMEOUT_MS);
}

pub fn get_render_target_time() -> f64 {
    WORK_IN_PROGRESS_ROOT_RENDER_TARGET_TIME.get()
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::nullable)]
    static LEGACY_ERROR_BOUNDARIES_THAT_ALREADY_FAILED: Cell<Option<&'static Set<Unknown>>> = const { Cell::new(js::null()) };
}

type SuspendedCommitReason = Option<&'static str>;

type DelayedCommitReason = u8;
const IMMEDIATE_COMMIT: DelayedCommitReason = 0;
const ABORTED_VIEW_TRANSITION_COMMIT: DelayedCommitReason = 1;
const DELAYED_PASSIVE_COMMIT: DelayedCommitReason = 2;
const ANIMATION_STARTED_COMMIT: DelayedCommitReason = 3;

const NO_PENDING_EFFECTS: u8 = 0;
const PENDING_MUTATION_PHASE: u8 = 1;
const PENDING_LAYOUT_PHASE: u8 = 2;
const PENDING_AFTER_MUTATION_PHASE: u8 = 3;
const PENDING_SPAWNED_WORK: u8 = 4;
const PENDING_PASSIVE_PHASE: u8 = 5;
const PENDING_GESTURE_MUTATION_PHASE: u8 = 6;
const PENDING_GESTURE_ANIMATION_PHASE: u8 = 7;

/// A callback a `<ViewTransition>` scheduled for the commit, bound to its
/// instance: given the transition's types, it may return a cleanup.
type ViewTransitionEvent = Box<dyn Fn(&Rc<RefCell<TransitionTypes>>) -> Option<&'static dyn Fn()>>;

thread_local! {
    static PENDING_EFFECTS_STATUS: Cell<u8> = const { Cell::new(0) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static PENDING_EFFECTS_ROOT: RefCell<Option<Rc<FiberRoot>>> = const { RefCell::new(js::null()) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static PENDING_FINISHED_WORK: RefCell<Option<Rc<Fiber>>> = const { RefCell::new(js::null()) };
    static PENDING_EFFECTS_LANES: Cell<Lanes> = const { Cell::new(NO_LANES) };
    static PENDING_EFFECTS_REMAINING_LANES: Cell<Lanes> = const { Cell::new(NO_LANES) };
    static PENDING_EFFECTS_RENDER_END_TIME: Cell<f64> = const { Cell::new(-0.0) }; // Profiling-only
    #[cfg_attr(rust_js, rust_js::nullable)]
    static PENDING_PASSIVE_TRANSITIONS: RefCell<Option<Rc<Vec<&'static Transition>>>> = const { RefCell::new(js::null()) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static PENDING_RECOVERABLE_ERRORS: RefCell<Option<CaptureErrors>> = const { RefCell::new(js::null()) };
    // ReactFiberConfig's `RunningViewTransition`.
    #[cfg_attr(rust_js, rust_js::nullable)]
    static PENDING_VIEW_TRANSITION: Cell<Option<&'static Unknown>> = const { Cell::new(js::null()) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static PENDING_VIEW_TRANSITION_EVENTS: RefCell<Option<Vec<ViewTransitionEvent>>> = const { RefCell::new(js::null()) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static PENDING_TRANSITION_TYPES: RefCell<Option<Rc<RefCell<TransitionTypes>>>> = const { RefCell::new(js::null()) };
    static PENDING_DID_INCLUDE_RENDER_PHASE_UPDATE: Cell<bool> = const { Cell::new(false) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static PENDING_SUSPENDED_COMMIT_REASON: Cell<SuspendedCommitReason> = const { Cell::new(js::null()) }; // Profiling-only
    static PENDING_DELAYED_COMMIT_REASON: Cell<DelayedCommitReason> = const { Cell::new(IMMEDIATE_COMMIT) }; // Profiling-only
    #[cfg_attr(rust_js, rust_js::nullable)]
    static PENDING_SUSPENDED_VIEW_TRANSITION_REASON: Cell<Option<&'static str>> = const { Cell::new(js::null()) }; // Profiling-only
}

// Use these to prevent an infinite loop of nested updates
const NESTED_UPDATE_LIMIT: u32 = 50;
thread_local! {
    static NESTED_UPDATE_COUNT: Cell<u32> = const { Cell::new(0) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static ROOT_WITH_NESTED_UPDATES: RefCell<Option<Rc<FiberRoot>>> = const { RefCell::new(js::null()) };
    static IS_FLUSHING_PASSIVE_EFFECTS: Cell<bool> = const { Cell::new(false) };
    static DID_SCHEDULE_UPDATE_DURING_PASSIVE_EFFECTS: Cell<bool> = const { Cell::new(false) };
}

const NO_NESTED_UPDATE: u8 = 0;
const NESTED_UPDATE_SYNC_LANE: u8 = 1;
const NESTED_UPDATE_PHASE_SPAWN: u8 = 2;
thread_local! {
    static NESTED_UPDATE_KIND: Cell<u8> = const { Cell::new(NO_NESTED_UPDATE) };
}

const NESTED_PASSIVE_UPDATE_LIMIT: u32 = 50;
thread_local! {
    static NESTED_PASSIVE_UPDATE_COUNT: Cell<u32> = const { Cell::new(0) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static ROOT_WITH_PASSIVE_NESTED_UPDATES: RefCell<Option<Rc<FiberRoot>>> = const { RefCell::new(js::null()) };

    static IS_RUNNING_INSERTION_EFFECT: Cell<bool> = const { Cell::new(false) };
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_work_in_progress_root() -> Option<Rc<FiberRoot>> {
    WORK_IN_PROGRESS_ROOT.with_borrow(Clone::clone)
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_committing_root() -> Option<Rc<FiberRoot>> {
    PENDING_EFFECTS_ROOT.with_borrow(Clone::clone)
}

pub fn get_work_in_progress_root_render_lanes() -> Lanes {
    WORK_IN_PROGRESS_ROOT_RENDER_LANES.get()
}

pub fn has_pending_commit_effects() -> bool {
    PENDING_EFFECTS_STATUS.get() != NO_PENDING_EFFECTS
        && PENDING_EFFECTS_STATUS.get() != PENDING_PASSIVE_PHASE
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_root_with_pending_passive_effects() -> Option<Rc<FiberRoot>> {
    if PENDING_EFFECTS_STATUS.get() == PENDING_PASSIVE_PHASE {
        PENDING_EFFECTS_ROOT.with_borrow(Clone::clone)
    } else {
        js::null()
    }
}

pub fn get_pending_passive_effects_lanes() -> Lanes {
    PENDING_EFFECTS_LANES.get()
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_pending_transition_types() -> Option<Rc<RefCell<TransitionTypes>>> {
    PENDING_TRANSITION_TYPES.with_borrow(Clone::clone)
}

pub fn is_work_loop_suspended_on_data() -> bool {
    WORK_IN_PROGRESS_SUSPENDED_REASON.get() == SUSPENDED_ON_DATA
        || WORK_IN_PROGRESS_SUSPENDED_REASON.get() == SUSPENDED_ON_ACTION
}

pub fn get_current_time() -> f64 {
    now()
}

pub fn request_update_lane(fiber: &Rc<Fiber>) -> Lane {
    // Special cases
    let mode = fiber.mode.get();
    if !DISABLE_LEGACY_MODE && (mode & CONCURRENT_MODE) == NO_MODE {
        return SYNC_LANE;
    } else if (EXECUTION_CONTEXT.get() & RENDER_CONTEXT) != NO_CONTEXT
        && WORK_IN_PROGRESS_ROOT_RENDER_LANES.get() != NO_LANES
    {
        // This is a render phase update. These are not officially supported. The
        // old behavior is to give this the same "thread" (lanes) as
        // whatever is currently rendering. So if you call `setState` on a component
        // that happens later in the same render, it will flush. Ideally, we want to
        // remove the special case and treat them as if they came from an
        // interleaved event. Regardless, this pattern is not officially supported.
        // This behavior is only a fallback. The flag only exists until we can roll
        // out the setState warning, since existing code might accidentally rely on
        // the current behavior.
        return pick_arbitrary_lane(WORK_IN_PROGRESS_ROOT_RENDER_LANES.get());
    }

    let transition = request_current_transition();
    if let Some(transition) = transition {
        if ENABLE_GESTURE_TRANSITION.with(|&enabled| enabled) && transition.gesture.is_some() {
            panic!(
                "Cannot setState on regular state inside a startGestureTransition. \
                 Gestures can only update the useOptimistic() hook. There should be no \
                 side-effects associated with starting a Gesture until its Action is \
                 invoked. Move side-effects to the Action instead."
            );
        }
        if __DEV__ {
            transition
                .updated_fibers
                .borrow_mut()
                .get_or_insert_with(HashSet::new)
                .insert(unsafe { js::cast(Some(js::unknown_of(Rc::clone(fiber)))) });
            if has_potential_use_warnings() && resolve_update_priority() == DISCRETE_EVENT_PRIORITY
            {
                // If we're updating inside a discrete event, then this might be a new user interaction
                // and not just an automatically resolved loading sequence. Don't warn unless it happens again.
                clear_use_warnings();
            }
        }

        return request_transition_lane(Some(transition));
    }

    let priority = resolve_update_priority();
    if __DEV__ && priority == DISCRETE_EVENT_PRIORITY {
        clear_use_warnings();
    }
    event_priority_to_lane(priority)
}

fn request_retry_lane(fiber: &Fiber) -> Lane {
    // This is a fork of `requestUpdateLane` designed specifically for Suspense
    // "retries" — a special update that attempts to flip a Suspense boundary
    // from its placeholder state to its primary/resolved state.

    // Special cases
    let mode = fiber.mode.get();
    if !DISABLE_LEGACY_MODE && (mode & CONCURRENT_MODE) == NO_MODE {
        return SYNC_LANE;
    }

    claim_next_retry_lane()
}

pub fn request_deferred_lane() -> Lane {
    if WORK_IN_PROGRESS_DEFERRED_LANE.get() == NO_LANE {
        // If there are multiple useDeferredValue hooks in the same render, the
        // tasks that they spawn should all be batched together, so they should all
        // receive the same lane.

        // Check the priority of the current render to decide the priority of the
        // deferred task.

        // OffscreenLane is used for prerendering, but we also use OffscreenLane
        // for incremental hydration. It's given the lowest priority because the
        // initial HTML is the same as the final UI. But useDeferredValue during
        // hydration is an exception — we need to upgrade the UI to the final
        // value. So if we're currently hydrating, we treat it like a transition.
        let is_prerendering =
            includes_some_lane(WORK_IN_PROGRESS_ROOT_RENDER_LANES.get(), OFFSCREEN_LANE)
                && !get_is_hydrating();
        if is_prerendering {
            // There's only one OffscreenLane, so if it contains deferred work, we
            // should just reschedule using the same lane.
            WORK_IN_PROGRESS_DEFERRED_LANE.set(OFFSCREEN_LANE);
        } else {
            // Everything else is spawned as a transition.
            WORK_IN_PROGRESS_DEFERRED_LANE.set(claim_next_transition_deferred_lane());
        }
    }

    // Mark the parent Suspense boundary so it knows to spawn the deferred lane.
    let suspense_handler = get_suspense_handler();
    if let Some(suspense_handler) = suspense_handler {
        // TODO: As an optimization, we shouldn't entangle the lanes at the root; we
        // can entangle them using the baseLanes of the Suspense boundary instead.
        // We only need to do something special if there's no Suspense boundary.
        suspense_handler
            .flags
            .set(suspense_handler.flags.get() | DID_DEFER);
    }

    WORK_IN_PROGRESS_DEFERRED_LANE.get()
}

#[cfg_attr(rust_js, rust_js::nullable(callback))]
pub fn schedule_view_transition_event(
    fiber: &Fiber,
    callback: Option<&'static ViewTransitionCallback>,
) {
    if ENABLE_VIEW_TRANSITION && let Some(callback) = callback {
        let state: &ViewTransitionState = unsafe { js::cast(fiber.state_node.get()) };
        let instance = match state.r#ref.get() {
            Some(instance) => instance,
            None => {
                let instance = create_view_transition_instance(&get_view_transition_name(
                    unsafe { js::cast(fiber.memoized_props.get()) },
                    state,
                ));
                state.r#ref.set(Some(instance));
                instance
            }
        };
        // Upstream's `callback.bind(null, instance)`: an arrow that passes `types` on.
        let event: ViewTransitionEvent = Box::new(move |types| callback(instance, types));
        PENDING_VIEW_TRANSITION_EVENTS.with_borrow_mut(|pending_view_transition_events| {
            pending_view_transition_events
                .get_or_insert_with(Vec::new)
                .push(event);
        });
    }
}

#[cfg_attr(rust_js, rust_js::nullable(callback))]
pub fn schedule_gesture_transition_event(
    fiber: &Fiber,
    callback: Option<&'static ViewTransitionGestureCallback>,
) {
    if ENABLE_GESTURE_TRANSITION.with(|&enabled| enabled)
        && let Some(callback) = callback
    {
        let applying_gesture = PENDING_EFFECTS_ROOT.with_borrow(|pending_effects_root| {
            pending_effects_root
                .as_ref()
                .unwrap()
                .pending_gestures
                .borrow()
                .clone()
        });
        if let Some(applying_gesture) = applying_gesture {
            let state: &ViewTransitionState = unsafe { js::cast(fiber.state_node.get()) };
            let instance = match state.r#ref.get() {
                Some(instance) => instance,
                None => {
                    let instance = create_view_transition_instance(&get_view_transition_name(
                        unsafe { js::cast(fiber.memoized_props.get()) },
                        state,
                    ));
                    state.r#ref.set(Some(instance));
                    instance
                }
            };
            let timeline = applying_gesture.provider;
            let options = GestureOptionsRequired {
                range_start: applying_gesture.range_start.get(),
                range_end: applying_gesture.range_end.get(),
            };
            // Upstream's `callback.bind(null, timeline, options, instance)`: an arrow that
            // passes `types` on.
            let event: ViewTransitionEvent =
                Box::new(move |types| callback(timeline, &options, instance, types));
            PENDING_VIEW_TRANSITION_EVENTS.with_borrow_mut(|pending_view_transition_events| {
                pending_view_transition_events
                    .get_or_insert_with(Vec::new)
                    .push(event);
            });
        }
    }
}

pub fn peek_deferred_lane() -> Lane {
    WORK_IN_PROGRESS_DEFERRED_LANE.get()
}

pub fn schedule_update_on_fiber(root: &Rc<FiberRoot>, fiber: &Rc<Fiber>, lane: Lane) {
    if __DEV__ && IS_RUNNING_INSERTION_EFFECT.get() {
        console::error(("useInsertionEffect must not schedule updates.",));
    }

    if __DEV__ && IS_FLUSHING_PASSIVE_EFFECTS.get() {
        DID_SCHEDULE_UPDATE_DURING_PASSIVE_EFFECTS.set(true);
    }

    // Check if the work loop is currently suspended and waiting for data to
    // finish loading.
    if
    // Suspended render phase
    (WORK_IN_PROGRESS_ROOT.with_borrow(|work_in_progress_root| {
        work_in_progress_root
            .as_ref()
            .is_some_and(|work_in_progress_root| Rc::ptr_eq(root, work_in_progress_root))
    }) && (WORK_IN_PROGRESS_SUSPENDED_REASON.get() == SUSPENDED_ON_DATA
        || WORK_IN_PROGRESS_SUSPENDED_REASON.get() == SUSPENDED_ON_ACTION))
        // Suspended commit phase
        || root.cancel_pending_commit.borrow().is_some()
    {
        // The incoming update might unblock the current render. Interrupt the
        // current attempt and restart from the top.
        prepare_fresh_stack(root, NO_LANES);
        let did_attempt_entire_tree = false;
        mark_root_suspended(
            root,
            WORK_IN_PROGRESS_ROOT_RENDER_LANES.get(),
            WORK_IN_PROGRESS_DEFERRED_LANE.get(),
            did_attempt_entire_tree,
        );
    }

    // Mark that the root has a pending update.
    mark_root_updated(root, lane);

    if (EXECUTION_CONTEXT.get() & RENDER_CONTEXT) != NO_CONTEXT
        && WORK_IN_PROGRESS_ROOT.with_borrow(|work_in_progress_root| {
            work_in_progress_root
                .as_ref()
                .is_some_and(|work_in_progress_root| Rc::ptr_eq(root, work_in_progress_root))
        })
    {
        // This update was dispatched during the render phase. This is a mistake
        // if the update originates from user space (with the exception of local
        // hook updates, which are handled differently and don't reach this
        // function), but there are some internal React features that use this as
        // an implementation detail, like selective hydration.
        warn_about_render_phase_updates_in_dev(fiber);

        // Track lanes that were updated during the render phase
        WORK_IN_PROGRESS_ROOT_RENDER_PHASE_UPDATED_LANES.set(merge_lanes(
            WORK_IN_PROGRESS_ROOT_RENDER_PHASE_UPDATED_LANES.get(),
            lane,
        ));
    } else {
        // This is a normal update, scheduled from outside the render phase. For
        // example, during an input event.
        if ENABLE_UPDATER_TRACKING.with(|&enabled| enabled)
            && IS_DEV_TOOLS_PRESENT.with(|&present| present)
        {
            add_fiber_to_lanes_map(root, fiber, lane);
        }

        warn_if_updates_not_wrapped_with_act_dev(fiber);

        if ENABLE_TRANSITION_TRACING {
            let transition = t(REACT_SHARED_INTERNALS);
            if let Some(transition) = transition
                && transition.name.is_some()
            {
                if transition.start_time.get() == Some(-1.0) {
                    transition.start_time.set(Some(now()));
                }

                add_transition_to_lanes_map(root, transition, lane);
            }
        }

        if WORK_IN_PROGRESS_ROOT.with_borrow(|work_in_progress_root| {
            work_in_progress_root
                .as_ref()
                .is_some_and(|work_in_progress_root| Rc::ptr_eq(root, work_in_progress_root))
        }) {
            // Received an update to a tree that's in the middle of rendering. Mark
            // that there was an interleaved update work on this root.
            if (EXECUTION_CONTEXT.get() & RENDER_CONTEXT) == NO_CONTEXT {
                WORK_IN_PROGRESS_ROOT_INTERLEAVED_UPDATED_LANES.set(merge_lanes(
                    WORK_IN_PROGRESS_ROOT_INTERLEAVED_UPDATED_LANES.get(),
                    lane,
                ));
            }
            if WORK_IN_PROGRESS_ROOT_EXIT_STATUS.get() == ROOT_SUSPENDED_WITH_DELAY {
                // The root already suspended with a delay, which means this render
                // definitely won't finish. Since we have a new update, let's mark it as
                // suspended now, right before marking the incoming update. This has the
                // effect of interrupting the current render and switching to the update.
                // TODO: Make sure this doesn't override pings that happen while we've
                // already started rendering.
                let did_attempt_entire_tree = false;
                mark_root_suspended(
                    root,
                    WORK_IN_PROGRESS_ROOT_RENDER_LANES.get(),
                    WORK_IN_PROGRESS_DEFERRED_LANE.get(),
                    did_attempt_entire_tree,
                );
            }
        }

        ensure_root_is_scheduled(root);
        if lane == SYNC_LANE
            && EXECUTION_CONTEXT.get() == NO_CONTEXT
            && !DISABLE_LEGACY_MODE
            && (fiber.mode.get() & CONCURRENT_MODE) == NO_MODE
        {
            if __DEV__ && REACT_SHARED_INTERNALS.is_batching_legacy() {
                // Treat `act` as if it's inside `batchedUpdates`, even in legacy mode.
            } else {
                // Flush the synchronous work now, unless we're already working or inside
                // a batch. This is intentionally inside scheduleUpdateOnFiber instead of
                // scheduleCallbackForFiber to preserve the ability to schedule a callback
                // without immediately flushing it. We only do this for user-initiated
                // updates, to preserve historical behavior of legacy mode.
                reset_render_timer();
                flush_sync_work_on_legacy_roots_only();
            }
        }
    }
}

pub fn schedule_initial_hydration_on_root(root: &Rc<FiberRoot>, lane: Lane) {
    // This is a special fork of scheduleUpdateOnFiber that is only used to
    // schedule the initial hydration of a root that has just been created. Most
    // of the stuff in scheduleUpdateOnFiber can be skipped.
    //
    // The main reason for this separate path, though, is to distinguish the
    // initial children from subsequent updates. In fully client-rendered roots
    // (createRoot instead of hydrateRoot), all top-level renders are modeled as
    // updates, but hydration roots are special because the initial render must
    // match what was rendered on the server.
    let current = Rc::clone(&root.current.borrow());
    current.lanes.set(lane);
    mark_root_updated(root, lane);
    ensure_root_is_scheduled(root);
}

pub fn is_unsafe_class_render_phase_update(_fiber: &Fiber) -> bool {
    // Check if this is a render phase update. Only called by class components,
    // which special (deprecated) behavior for UNSAFE_componentWillReceive props.
    (EXECUTION_CONTEXT.get() & RENDER_CONTEXT) != NO_CONTEXT
}

pub fn perform_work_on_root(root: &Rc<FiberRoot>, mut lanes: Lanes, force_sync: bool) {
    if (EXECUTION_CONTEXT.get() & (RENDER_CONTEXT | COMMIT_CONTEXT)) != NO_CONTEXT {
        panic!("Should not already be working.");
    }

    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        && ENABLE_COMPONENT_PERFORMANCE_TRACK
        && WORK_IN_PROGRESS_ROOT_RENDER_LANES.get() != NO_LANES
        && let Some(yielded_fiber) = WORK_IN_PROGRESS.with_borrow(Clone::clone)
    {
        // We've returned from yielding to the event loop. Let's log the time it took.
        let yield_end_time = now();
        match YIELD_REASON.get() {
            SUSPENDED_ON_IMMEDIATE | SUSPENDED_ON_DATA => {
                log_suspended_yield_time(YIELD_START_TIME.get(), yield_end_time, &yielded_fiber);
            }
            SUSPENDED_ON_ACTION => {
                log_action_yield_time(YIELD_START_TIME.get(), yield_end_time, &yielded_fiber);
            }
            _ => {
                log_yield_time(YIELD_START_TIME.get(), yield_end_time);
            }
        }
    }

    // We disable time-slicing in some cases: if the work has been CPU-bound
    // for too long ("expired" work, to prevent starvation), or we're in
    // sync-updates-by-default mode.
    let should_time_slice = (!force_sync
        && !includes_blocking_lane(lanes)
        && !includes_expired_lane(root, lanes))
        // If we're prerendering, then we should use the concurrent work loop
        // even if the lanes are synchronous, so that prerendering never blocks
        // the main thread.
        // TODO: We should consider doing this whenever a sync lane is suspended,
        // even for regular pings.
        || check_if_root_is_prerendering(root, lanes);

    let mut exit_status: RootExitStatus = if should_time_slice {
        render_root_concurrent(root, lanes)
    } else {
        render_root_sync(root, lanes, true)
    };

    let mut render_was_concurrent = should_time_slice;

    loop {
        if exit_status == ROOT_IN_PROGRESS {
            // Render phase is still in progress.
            if WORK_IN_PROGRESS_ROOT_IS_PRERENDERING.get() && !should_time_slice {
                // We're in prerendering mode, but time slicing is not enabled. This
                // happens when something suspends during a synchronous update. Exit the
                // the work loop. When we resume, we'll use the concurrent work loop so
                // that prerendering is non-blocking.
                //
                // Mark the root as suspended. Usually we do this at the end of the
                // render phase, but we do it here so that we resume in
                // prerendering mode.
                // TODO: Consider always calling markRootSuspended immediately.
                // Needs to be *after* we attach a ping listener, though.
                let did_attempt_entire_tree = false;
                mark_root_suspended(root, lanes, NO_LANE, did_attempt_entire_tree);
            }
            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK
            {
                // We're about to yield. Let's keep track of how long we yield to the event loop.
                // We also stash the suspended reason at the time we yielded since it might have
                // changed when we resume such as when it gets pinged.
                start_yield_timer(WORK_IN_PROGRESS_SUSPENDED_REASON.get());
            }
            break;
        } else {
            let mut render_end_time = 0.0;
            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK
            {
                render_end_time = now();
            }

            // The render completed.

            // Check if this render may have yielded to a concurrent event, and if so,
            // confirm that any newly rendered stores are consistent.
            // TODO: It's possible that even a concurrent render may never have yielded
            // to the main thread, if it was fast enough, or if it expired. We could
            // skip the consistency check in that case, too.
            let finished_work = root.current.borrow().alternate.borrow().clone().unwrap();
            if render_was_concurrent && !is_render_consistent_with_external_stores(&finished_work) {
                if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                    && ENABLE_COMPONENT_PERFORMANCE_TRACK
                {
                    set_current_track_from_lanes(lanes);
                    log_inconsistent_render(
                        RENDER_START_TIME.get(),
                        render_end_time,
                        WORK_IN_PROGRESS_UPDATE_TASK.get(),
                    );
                    finalize_render(lanes, render_end_time);
                }
                // A store was mutated in an interleaved event. Render again,
                // synchronously, to block further mutations.
                exit_status = render_root_sync(root, lanes, false);
                // We assume the tree is now consistent because we didn't yield to any
                // concurrent events.
                render_was_concurrent = false;
                // Need to check the exit status again.
                continue;
            }

            // Check if something threw
            if (DISABLE_LEGACY_MODE || root.tag != LEGACY_ROOT) && exit_status == ROOT_ERRORED {
                let lanes_that_just_errored = lanes;
                let error_retry_lanes =
                    get_lanes_to_retry_synchronously_on_error(root, lanes_that_just_errored);
                if error_retry_lanes != NO_LANES {
                    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                        && ENABLE_COMPONENT_PERFORMANCE_TRACK
                    {
                        set_current_track_from_lanes(lanes);
                        log_errored_render_phase(
                            RENDER_START_TIME.get(),
                            render_end_time,
                            lanes,
                            WORK_IN_PROGRESS_UPDATE_TASK.get(),
                        );
                        finalize_render(lanes, render_end_time);
                    }
                    lanes = error_retry_lanes;
                    exit_status = recover_from_concurrent_error(
                        root,
                        lanes_that_just_errored,
                        error_retry_lanes,
                    );
                    render_was_concurrent = false;
                    // Need to check the exit status again.
                    if exit_status != ROOT_ERRORED {
                        // The root did not error this time. Restart the exit algorithm
                        // from the beginning.
                        // TODO: Refactor the exit algorithm to be less confusing. Maybe
                        // more branches + recursion instead of a loop. I think the only
                        // thing that causes it to be a loop is the RootSuspendedAtTheShell
                        // check. If that's true, then we don't need a loop/recursion
                        // at all.
                        continue;
                    } else {
                        // The root errored yet again. Proceed to commit the tree.
                        if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                            && ENABLE_COMPONENT_PERFORMANCE_TRACK
                        {
                            render_end_time = now();
                        }
                    }
                }
            }
            if exit_status == ROOT_FATAL_ERRORED {
                if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                    && ENABLE_COMPONENT_PERFORMANCE_TRACK
                {
                    set_current_track_from_lanes(lanes);
                    log_errored_render_phase(
                        RENDER_START_TIME.get(),
                        render_end_time,
                        lanes,
                        WORK_IN_PROGRESS_UPDATE_TASK.get(),
                    );
                    finalize_render(lanes, render_end_time);
                }
                prepare_fresh_stack(root, NO_LANES);
                // Since this is a fatal error, we're going to pretend we attempted
                // the entire tree, to avoid scheduling a prerender.
                let did_attempt_entire_tree = true;
                mark_root_suspended(root, lanes, NO_LANE, did_attempt_entire_tree);
                break;
            }

            // We now have a consistent tree. The next step is either to commit it,
            // or, if something suspended, wait to commit it after a timeout.
            finish_concurrent_render(root, exit_status, &finished_work, lanes, render_end_time);
        }
        break;
    }

    ensure_root_is_scheduled(root);
}

fn recover_from_concurrent_error(
    root: &Rc<FiberRoot>,
    originally_attempted_lanes: Lanes,
    error_retry_lanes: Lanes,
) -> RootExitStatus {
    // If an error occurred during hydration, discard server response and fall
    // back to client side render.

    // Before rendering again, save the errors from the previous attempt.
    let errors_from_first_attempt =
        WORK_IN_PROGRESS_ROOT_CONCURRENT_ERRORS.with_borrow(Clone::clone);

    let was_root_dehydrated = SUPPORTS_HYDRATION && is_root_dehydrated(root);
    if was_root_dehydrated {
        // The shell failed to hydrate. Set a flag to force a client rendering
        // during the next attempt. To do this, we call prepareFreshStack now
        // to create the root work-in-progress fiber. This is a bit weird in terms
        // of factoring, because it relies on renderRootSync not calling
        // prepareFreshStack again in the call below, which happens because the
        // root and lanes haven't changed.
        //
        // TODO: I think what we should do is set ForceClientRender inside
        // throwException, like we do for nested Suspense boundaries. The reason
        // it's here instead is so we can switch to the synchronous work loop, too.
        // Something to consider for a future refactor.
        let root_work_in_progress = prepare_fresh_stack(root, error_retry_lanes);
        root_work_in_progress
            .flags
            .set(root_work_in_progress.flags.get() | FORCE_CLIENT_RENDER);
    }

    let exit_status = render_root_sync(root, error_retry_lanes, false);
    if exit_status == ROOT_SUSPENDED_AT_THE_SHELL {
        // The retry unwound to the root without completing (e.g. something
        // suspended in the shell), so the tree is incomplete and must not be
        // treated as recovered — committing it would corrupt the current tree.
        // Return the status as-is so the root stays suspended.
        //
        // On a dehydrated root, the retry is the client render, not a check for
        // data races, so we leave error recovery enabled.
        if !was_root_dehydrated {
            // The retry couldn't finish, so it can't tell us whether the error was
            // caused by a data race. Disable error recovery for these lanes so the
            // next attempt commits the error boundary instead of retrying again.
            // That includes any deferred lane the retry spawned, which would
            // otherwise error and retry the same way.
            root.error_recovery_disabled_lanes.set(merge_lanes(
                root.error_recovery_disabled_lanes.get(),
                merge_lanes(
                    originally_attempted_lanes,
                    WORK_IN_PROGRESS_DEFERRED_LANE.get(),
                ),
            ));
        }
    } else if exit_status != ROOT_ERRORED {
        // Successfully finished rendering on retry

        if WORK_IN_PROGRESS_ROOT_DID_ATTACH_PING_LISTENER.get() && !was_root_dehydrated {
            // During the synchronous render, we attached additional ping listeners.
            // This is highly suggestive of an uncached promise (though it's not the
            // only reason this would happen). If it was an uncached promise, then
            // it may have masked a downstream error from ocurring without actually
            // fixing it. Example:
            //
            //    use(Promise.resolve('uncached'))
            //    throw new Error('Oops!')
            //
            // When this happens, there's a conflict between blocking potential
            // concurrent data races and unwrapping uncached promise values. We
            // have to choose one or the other. Because the data race recovery is
            // a last ditch effort, we'll disable it.
            root.error_recovery_disabled_lanes.set(merge_lanes(
                root.error_recovery_disabled_lanes.get(),
                originally_attempted_lanes,
            ));

            // Mark the current render as suspended and force it to restart. Once
            // these lanes finish successfully, we'll re-enable the error recovery
            // mechanism for subsequent updates.
            WORK_IN_PROGRESS_ROOT_INTERLEAVED_UPDATED_LANES.set(
                WORK_IN_PROGRESS_ROOT_INTERLEAVED_UPDATED_LANES.get() | originally_attempted_lanes,
            );
            return ROOT_SUSPENDED_WITH_DELAY;
        }

        // The errors from the failed first attempt have been recovered. Add
        // them to the collection of recoverable errors. We'll log them in the
        // commit phase.
        let errors_from_second_attempt =
            WORK_IN_PROGRESS_ROOT_RECOVERABLE_ERRORS.replace(errors_from_first_attempt);
        // The errors from the second attempt should be queued after the errors
        // from the first attempt, to preserve the causal sequence.
        if let Some(errors_from_second_attempt) = errors_from_second_attempt {
            queue_recoverable_errors(&errors_from_second_attempt);
        }
    } else {
        // The UI failed to recover.
    }
    exit_status
}

pub fn queue_recoverable_errors(errors: &CaptureErrors) {
    let work_in_progress_root_recoverable_errors =
        WORK_IN_PROGRESS_ROOT_RECOVERABLE_ERRORS.with_borrow(Clone::clone);
    match work_in_progress_root_recoverable_errors {
        None => {
            WORK_IN_PROGRESS_ROOT_RECOVERABLE_ERRORS.set(Some(Rc::clone(errors)));
        }
        Some(work_in_progress_root_recoverable_errors) => {
            // Upstream's `workInProgressRootRecoverableErrors.push.apply(..., errors)`:
            // one error at a time, as two arrays of errors can't be borrowed at once.
            let mut i = 0;
            while i < errors.borrow().len() {
                let error = Rc::clone(&errors.borrow()[i]);
                work_in_progress_root_recoverable_errors
                    .borrow_mut()
                    .push(error);
                i += 1;
            }
        }
    }
}

fn finish_concurrent_render(
    root: &Rc<FiberRoot>,
    exit_status: RootExitStatus,
    finished_work: &Rc<Fiber>,
    lanes: Lanes,
    render_end_time: f64, // Profiling-only
) {
    // TODO: The fact that most of these branches are identical suggests that some
    // of the exit statuses are not best modeled as exit statuses and should be
    // tracked orthogonally.
    match exit_status {
        ROOT_IN_PROGRESS | ROOT_FATAL_ERRORED => {
            panic!("Root did not complete. This is a bug in React.");
        }
        ROOT_SUSPENDED_WITH_DELAY
            if !includes_only_transitions(lanes) && !includes_only_retries(lanes) =>
        {
            // Commit the placeholder.
        }
        ROOT_SUSPENDED_WITH_DELAY | ROOT_SUSPENDED_AT_THE_SHELL => {
            // This is a transition, so we should exit without committing a
            // placeholder and without scheduling a timeout. Delay indefinitely
            // until we receive more data.
            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK
            {
                set_current_track_from_lanes(lanes);
                log_suspended_render_phase(
                    RENDER_START_TIME.get(),
                    render_end_time,
                    lanes,
                    WORK_IN_PROGRESS_UPDATE_TASK.get(),
                );
                finalize_render(lanes, render_end_time);
                track_suspended_time(lanes, render_end_time);
            }
            let did_attempt_entire_tree = !WORK_IN_PROGRESS_ROOT_DID_SKIP_SUSPENDED_SIBLINGS.get();
            mark_root_suspended(
                root,
                lanes,
                WORK_IN_PROGRESS_DEFERRED_LANE.get(),
                did_attempt_entire_tree,
            );
            return;
        }
        ROOT_ERRORED => {
            // This render errored. Ignore any recoverable errors because we weren't actually
            // able to recover. Instead, whatever the final errors were is the ones we log.
            // This ensures that we only log the actual client side error if it's just a plain
            // error thrown from a component on the server and the client.
            WORK_IN_PROGRESS_ROOT_RECOVERABLE_ERRORS.set(js::null());
        }
        ROOT_SUSPENDED | ROOT_COMPLETED => {}
        _ => {
            panic!("Unknown root exit status.");
        }
    }

    if should_force_flush_fallbacks_in_dev() {
        // We're inside an `act` scope. Commit immediately.
        complete_root(
            root,
            Some(finished_work),
            lanes,
            WORK_IN_PROGRESS_ROOT_RECOVERABLE_ERRORS
                .with_borrow(Clone::clone)
                .as_ref(),
            WORK_IN_PROGRESS_TRANSITIONS
                .with_borrow(Clone::clone)
                .as_ref(),
            WORK_IN_PROGRESS_ROOT_DID_INCLUDE_RECURSIVE_RENDER_UPDATE.get(),
            WORK_IN_PROGRESS_DEFERRED_LANE.get(),
            WORK_IN_PROGRESS_ROOT_INTERLEAVED_UPDATED_LANES.get(),
            WORK_IN_PROGRESS_SUSPENDED_RETRY_LANES.get(),
            WORK_IN_PROGRESS_ROOT_DID_SKIP_SUSPENDED_SIBLINGS.get(),
            exit_status,
            js::null(),
            js::null(),
            RENDER_START_TIME.get(),
            render_end_time,
        );
    } else {
        if includes_only_retries(lanes)
            && (ALWAYS_THROTTLE_RETRIES || exit_status == ROOT_SUSPENDED)
        {
            // This render only included retries, no updates. Throttle committing
            // retries so that we don't show too many loading states too quickly.
            let ms_until_timeout =
                GLOBAL_MOST_RECENT_FALLBACK_TIME.get() + FALLBACK_THROTTLE_MS - now();

            // Don't bother with a very short suspense time.
            if ms_until_timeout > 10.0 {
                let did_attempt_entire_tree =
                    !WORK_IN_PROGRESS_ROOT_DID_SKIP_SUSPENDED_SIBLINGS.get();
                mark_root_suspended(
                    root,
                    lanes,
                    WORK_IN_PROGRESS_DEFERRED_LANE.get(),
                    did_attempt_entire_tree,
                );

                let next_lanes = get_next_lanes(root, NO_LANES, true);
                if next_lanes != NO_LANES {
                    // There's additional work we can do on this root. We might as well
                    // attempt to work on that while we're suspended.
                    return;
                }

                // The render is suspended, it hasn't timed out, and there's no
                // lower priority work to do. Instead of committing the fallback
                // immediately, wait for more data to arrive.
                // TODO: Combine retry throttling with Suspensey commits. Right now they
                // run one after the other.
                PENDING_EFFECTS_LANES.set(lanes);
                root.timeout_handle.set(schedule_timeout(
                    function::bind(
                        complete_root_when_ready,
                        Some(&Kind::Null),
                        (
                            Rc::clone(root),
                            Rc::clone(finished_work),
                            WORK_IN_PROGRESS_ROOT_RECOVERABLE_ERRORS.with_borrow(Clone::clone),
                            WORK_IN_PROGRESS_TRANSITIONS.with_borrow(Clone::clone),
                            WORK_IN_PROGRESS_ROOT_DID_INCLUDE_RECURSIVE_RENDER_UPDATE.get(),
                            lanes,
                            WORK_IN_PROGRESS_DEFERRED_LANE.get(),
                            WORK_IN_PROGRESS_ROOT_INTERLEAVED_UPDATED_LANES.get(),
                            WORK_IN_PROGRESS_SUSPENDED_RETRY_LANES.get(),
                            WORK_IN_PROGRESS_ROOT_DID_SKIP_SUSPENDED_SIBLINGS.get(),
                            exit_status,
                            Some("Throttled"),
                            RENDER_START_TIME.get(),
                            render_end_time,
                        ),
                    ),
                    ms_until_timeout,
                ));
                return;
            }
        }
        complete_root_when_ready(
            root,
            finished_work,
            WORK_IN_PROGRESS_ROOT_RECOVERABLE_ERRORS
                .with_borrow(Clone::clone)
                .as_ref(),
            WORK_IN_PROGRESS_TRANSITIONS
                .with_borrow(Clone::clone)
                .as_ref(),
            WORK_IN_PROGRESS_ROOT_DID_INCLUDE_RECURSIVE_RENDER_UPDATE.get(),
            lanes,
            WORK_IN_PROGRESS_DEFERRED_LANE.get(),
            WORK_IN_PROGRESS_ROOT_INTERLEAVED_UPDATED_LANES.get(),
            WORK_IN_PROGRESS_SUSPENDED_RETRY_LANES.get(),
            WORK_IN_PROGRESS_ROOT_DID_SKIP_SUSPENDED_SIBLINGS.get(),
            exit_status,
            js::null(),
            RENDER_START_TIME.get(),
            render_end_time,
        );
    }
}

#[cfg_attr(
    rust_js,
    rust_js::nullable(recoverable_errors, transitions, suspended_commit_reason)
)]
fn complete_root_when_ready(
    root: &Rc<FiberRoot>,
    finished_work: &Rc<Fiber>,
    recoverable_errors: Option<&CaptureErrors>,
    transitions: Option<&Rc<Vec<&'static Transition>>>,
    did_include_render_phase_update: bool,
    lanes: Lanes,
    spawned_lane: Lane,
    updated_lanes: Lanes,
    suspended_retry_lanes: Lanes,
    did_skip_suspended_siblings: bool,
    exit_status: RootExitStatus,
    suspended_commit_reason: SuspendedCommitReason, // Profiling-only
    completed_render_start_time: f64,               // Profiling-only
    completed_render_end_time: f64,                 // Profiling-only
) {
    root.timeout_handle.set(NO_TIMEOUT);

    // TODO: Combine retry throttling with Suspensey commits. Right now they run
    // one after the other.
    #[cfg_attr(rust_js, rust_js::name = "BothVisibilityAndMaySuspendCommit")]
    let both_visibility_and_may_suspend_commit = VISIBILITY | MAY_SUSPEND_COMMIT;
    let subtree_flags = finished_work.subtree_flags.get();
    let is_view_transition_eligible =
        ENABLE_VIEW_TRANSITION && includes_only_view_transition_eligible_lanes(lanes); // TODO: Use a subtreeFlag to optimize.
    let is_gesture_transition =
        ENABLE_GESTURE_TRANSITION.with(|&enabled| enabled) && is_gesture_render(lanes);
    let may_suspend_commit = (subtree_flags & SHOULD_SUSPEND_COMMIT) != 0
        || (subtree_flags & both_visibility_and_may_suspend_commit)
            == both_visibility_and_may_suspend_commit;
    let mut suspended_state: Option<&'static Unknown> = js::null();
    if is_view_transition_eligible || may_suspend_commit || is_gesture_transition {
        // Before committing, ask the renderer whether the host tree is ready.
        // If it's not, we'll wait until it notifies us.
        let new_suspended_state = start_suspending_commit();
        suspended_state = Some(new_suspended_state);
        // This will walk the completed fiber tree and attach listeners to all
        // the suspensey resources. The renderer is responsible for accumulating
        // all the load events. This all happens in a single synchronous
        // transaction, so it track state in its own module scope.
        // This will also track any newly added or appearing ViewTransition
        // components for the purposes of forming pairs.
        accumulate_suspensey_commit(finished_work, lanes, new_suspended_state);
        if is_view_transition_eligible
            || (is_gesture_transition
                && root
                    .pending_gestures
                    .borrow()
                    .as_ref()
                    .is_some_and(|pending_gestures| {
                        // If this gesture already has a View Transition running then we don't
                        // have to wait on that one before proceeding. We may hold the commit
                        // on the gesture committing later on in completeRoot.
                        pending_gestures.running.get().is_none()
                    }))
        {
            // Wait for any pending View Transition (including gestures) to finish.
            suspend_on_active_view_transition(new_suspended_state, root.container_info);
        }
        // For timeouts we use the previous fallback commit for retries and
        // the start time of the transition for transitions. This offset
        // represents the time already passed.
        let timeout_offset = if includes_only_retries(lanes) {
            GLOBAL_MOST_RECENT_FALLBACK_TIME.get() - now()
        } else if includes_only_transitions(lanes) {
            GLOBAL_MOST_RECENT_TRANSITION_TIME.get() - now()
        } else {
            0.0
        };
        // At the end, ask the renderer if it's ready to commit, or if we should
        // suspend. If it's not ready, it will return a callback to subscribe to
        // a ready event.
        let schedule_pending_commit =
            wait_for_commit_to_be_ready(new_suspended_state, timeout_offset);
        if let Some(schedule_pending_commit) = schedule_pending_commit {
            // NOTE: waitForCommitToBeReady returns a subscribe function so that we
            // only allocate a function if the commit isn't ready yet. The other
            // pattern would be to always pass a callback to waitForCommitToBeReady.

            // Not yet ready to commit. Delay the commit until the renderer notifies
            // us that it's ready. This will be canceled if we start work on the
            // root again.
            PENDING_EFFECTS_LANES.set(lanes);
            root.cancel_pending_commit
                .replace(Some(schedule_pending_commit(function::bind(
                    complete_root,
                    Some(&Kind::Null),
                    (
                        Rc::clone(root),
                        Some(Rc::clone(finished_work)),
                        lanes,
                        recoverable_errors.cloned(),
                        transitions.cloned(),
                        did_include_render_phase_update,
                        spawned_lane,
                        updated_lanes,
                        suspended_retry_lanes,
                        did_skip_suspended_siblings,
                        exit_status,
                        suspended_state,
                        if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
                            get_suspended_commit_reason(new_suspended_state, root.container_info)
                        } else {
                            js::null()
                        },
                        completed_render_start_time,
                        completed_render_end_time,
                    ),
                ))));
            let did_attempt_entire_tree = !did_skip_suspended_siblings;
            mark_root_suspended(root, lanes, spawned_lane, did_attempt_entire_tree);
            return;
        }
    }

    // Otherwise, commit immediately.;
    complete_root(
        root,
        Some(finished_work),
        lanes,
        recoverable_errors,
        transitions,
        did_include_render_phase_update,
        spawned_lane,
        updated_lanes,
        suspended_retry_lanes,
        did_skip_suspended_siblings,
        exit_status,
        suspended_state,
        suspended_commit_reason,
        completed_render_start_time,
        completed_render_end_time,
    );
}

fn is_render_consistent_with_external_stores(finished_work: &Rc<Fiber>) -> bool {
    // Search the rendered tree for external store reads, and check whether the
    // stores were mutated in a concurrent event. Intentionally using an iterative
    // loop instead of recursion so we can exit early.
    let mut node: Rc<Fiber> = Rc::clone(finished_work);
    loop {
        let tag = node.tag.get();
        if (tag == FUNCTION_COMPONENT || tag == FORWARD_REF || tag == SIMPLE_MEMO_COMPONENT)
            && (node.flags.get() & STORE_CONSISTENCY) != 0
        {
            let update_queue: Option<&FunctionComponentUpdateQueue> =
                unsafe { js::cast(node.update_queue.get()) };
            if let Some(update_queue) = update_queue {
                let checks = update_queue.stores.borrow().clone();
                if let Some(checks) = checks {
                    let mut i = 0;
                    while i < checks.borrow().len() {
                        let check = checks.borrow()[i];
                        let get_snapshot = check.get_snapshot;
                        let rendered_value = check.value;
                        match catch_unwind(AssertUnwindSafe(|| {
                            OBJECT_IS.with(|is| is(&get_snapshot(), &rendered_value))
                        })) {
                            Ok(true) => {}
                            Ok(false) => {
                                // Found an inconsistent store.
                                return false;
                            }
                            Err(_) => {
                                // If `getSnapshot` throws, return `false`. This will schedule
                                // a re-render, and the error will be rethrown during render.
                                return false;
                            }
                        }
                        i += 1;
                    }
                }
            }
        }
        let child = node.child.borrow().clone();
        if (node.subtree_flags.get() & STORE_CONSISTENCY) != 0
            && let Some(child) = child
        {
            child.r#return.replace(Some(Rc::clone(&node)));
            node = child;
            continue;
        }
        if Rc::ptr_eq(&node, finished_work) {
            return true;
        }
        while node.sibling.borrow().is_none() {
            let r#return = node.r#return.borrow().clone();
            match r#return {
                Some(r#return) if !Rc::ptr_eq(&r#return, finished_work) => node = r#return,
                _ => return true,
            }
        }
        let sibling = node.sibling.borrow().clone().unwrap();
        let r#return = node.r#return.borrow().clone();
        sibling.r#return.replace(r#return);
        node = sibling;
    }
}

// The extra indirections around markRootUpdated and markRootSuspended is
// needed to avoid a circular dependency between this module and
// ReactFiberLane. There's probably a better way to split up these modules and
// avoid this problem. Perhaps all the root-marking functions should move into
// the work loop.

fn mark_root_updated(root: &FiberRoot, updated_lanes: Lanes) {
    _mark_root_updated(root, updated_lanes);

    if ENABLE_INFINITE_RENDER_LOOP_DETECTION {
        // Check for recursive updates
        if (EXECUTION_CONTEXT.get() & RENDER_CONTEXT) != 0 {
            WORK_IN_PROGRESS_ROOT_DID_INCLUDE_RECURSIVE_RENDER_UPDATE.set(true);
        } else if (EXECUTION_CONTEXT.get() & COMMIT_CONTEXT) != 0 {
            DID_INCLUDE_COMMIT_PHASE_UPDATE.set(true);
        }

        throw_if_infinite_update_loop_detected(true);
    }
}

fn mark_root_pinged(root: &FiberRoot, pinged_lanes: Lanes) {
    _mark_root_pinged(root, pinged_lanes);

    if ENABLE_INFINITE_RENDER_LOOP_DETECTION {
        // Check for recursive pings. Pings are conceptually different from updates in
        // other contexts but we call it an "update" in this context because
        // repeatedly pinging a suspended render can cause a recursive render loop.
        // The relevant property is that it can result in a new render attempt
        // being scheduled.
        if (EXECUTION_CONTEXT.get() & RENDER_CONTEXT) != 0 {
            WORK_IN_PROGRESS_ROOT_DID_INCLUDE_RECURSIVE_RENDER_UPDATE.set(true);
        } else if (EXECUTION_CONTEXT.get() & COMMIT_CONTEXT) != 0 {
            DID_INCLUDE_COMMIT_PHASE_UPDATE.set(true);
        }

        throw_if_infinite_update_loop_detected(true);
    }
}

fn mark_root_suspended(
    root: &FiberRoot,
    mut suspended_lanes: Lanes,
    spawned_lane: Lane,
    did_attempt_entire_tree: bool,
) {
    // When suspending, we should always mark the entangled lanes as suspended.
    suspended_lanes = get_entangled_lanes(root, suspended_lanes);

    // When suspending, we should always exclude lanes that were pinged or (more
    // rarely, since we try to avoid it) updated during the render phase.
    suspended_lanes = remove_lanes(suspended_lanes, WORK_IN_PROGRESS_ROOT_PINGED_LANES.get());
    suspended_lanes = remove_lanes(
        suspended_lanes,
        WORK_IN_PROGRESS_ROOT_INTERLEAVED_UPDATED_LANES.get(),
    );
    _mark_root_suspended(root, suspended_lanes, spawned_lane, did_attempt_entire_tree);
}

pub fn flush_root(root: &Rc<FiberRoot>, lanes: Lanes) {
    if lanes != NO_LANES {
        upgrade_pending_lanes_to_sync(root, lanes);
        ensure_root_is_scheduled(root);
        if (EXECUTION_CONTEXT.get() & (RENDER_CONTEXT | COMMIT_CONTEXT)) == NO_CONTEXT {
            reset_render_timer();
            // TODO: For historical reasons this flushes all sync work across all
            // roots. It shouldn't really matter either way, but we could change this
            // to only flush the given root.
            flush_sync_work_on_all_roots();
        }
    }
}

pub fn get_execution_context() -> ExecutionContext {
    EXECUTION_CONTEXT.get()
}

/// Puts back the update priority and the Transition an update ran with.
struct RestoreUpdatePriority {
    previous_priority: EventPriority,
    prev_transition: Option<&'static Transition>,
}

impl Drop for RestoreUpdatePriority {
    fn drop(&mut self) {
        set_current_update_priority(self.previous_priority);
        set_t(REACT_SHARED_INTERNALS, self.prev_transition);
    }
}

pub fn deferred_updates<A>(f: impl FnOnce() -> A) -> A {
    let prev_transition = t(REACT_SHARED_INTERNALS);
    let previous_priority = get_current_update_priority();
    let _restore = RestoreUpdatePriority {
        previous_priority,
        prev_transition,
    };
    set_current_update_priority(DEFAULT_EVENT_PRIORITY);
    set_t(REACT_SHARED_INTERNALS, js::null());
    f()
}

/// Leaves `batchedUpdates`: puts back the execution context, and flushes the
/// legacy sync updates at the end of the outer most batch.
struct ExitBatchedUpdates(ExecutionContext);

impl Drop for ExitBatchedUpdates {
    fn drop(&mut self) {
        EXECUTION_CONTEXT.set(self.0);
        // If there were legacy sync updates, flush them at the end of the outer
        // most batchedUpdates-like method.
        if EXECUTION_CONTEXT.get() == NO_CONTEXT
            // Treat `act` as if it's inside `batchedUpdates`, even in legacy mode.
            && !(__DEV__ && REACT_SHARED_INTERNALS.is_batching_legacy())
        {
            reset_render_timer();
            flush_sync_work_on_legacy_roots_only();
        }
    }
}

pub fn batched_updates<A, R>(f: impl FnOnce(A) -> R, a: A) -> R {
    if DISABLE_LEGACY_MODE {
        // batchedUpdates is a no-op now, but there's still some internal react-dom
        // code calling it, that we can't remove until we remove legacy mode.
        f(a)
    } else {
        let prev_execution_context = EXECUTION_CONTEXT.get();
        EXECUTION_CONTEXT.set(EXECUTION_CONTEXT.get() | BATCHED_CONTEXT);
        let _exit = ExitBatchedUpdates(prev_execution_context);
        f(a)
    }
}

/// Leaves `discreteUpdates`: puts back the update priority and the
/// Transition, and resets the render timer outside of React's work.
struct ExitDiscreteUpdates {
    previous_priority: EventPriority,
    prev_transition: Option<&'static Transition>,
}

impl Drop for ExitDiscreteUpdates {
    fn drop(&mut self) {
        set_current_update_priority(self.previous_priority);
        set_t(REACT_SHARED_INTERNALS, self.prev_transition);
        if EXECUTION_CONTEXT.get() == NO_CONTEXT {
            reset_render_timer();
        }
    }
}

pub fn discrete_updates<A, B, C, D, R>(
    f: impl FnOnce(A, B, C, D) -> R,
    a: A,
    b: B,
    c: C,
    d: D,
) -> R {
    let prev_transition = t(REACT_SHARED_INTERNALS);
    let previous_priority = get_current_update_priority();
    let _exit = ExitDiscreteUpdates {
        previous_priority,
        prev_transition,
    };
    set_current_update_priority(DISCRETE_EVENT_PRIORITY);
    set_t(REACT_SHARED_INTERNALS, js::null());
    f(a, b, c, d)
}

/// Leaves `flushSync`: puts back the update priority, the Transition and
/// the execution context, then flushes the sync work scheduled in its batch.
struct ExitFlushSync {
    previous_priority: EventPriority,
    prev_transition: Option<&'static Transition>,
    prev_execution_context: ExecutionContext,
}

impl Drop for ExitFlushSync {
    fn drop(&mut self) {
        set_current_update_priority(self.previous_priority);
        set_t(REACT_SHARED_INTERNALS, self.prev_transition);

        EXECUTION_CONTEXT.set(self.prev_execution_context);
        // Flush the immediate callbacks that were scheduled during this batch.
        // Note that this will happen even if batchedUpdates is higher up
        // the stack.
        if (EXECUTION_CONTEXT.get() & (RENDER_CONTEXT | COMMIT_CONTEXT)) == NO_CONTEXT {
            flush_sync_work_on_all_roots();
        }
    }
}

pub fn flush_sync_from_reconciler<R>(f: Option<impl FnOnce() -> R>) -> Option<R> {
    // In legacy mode, we flush pending passive effects at the beginning of the
    // next event, not at the end of the previous one.
    if PENDING_EFFECTS_STATUS.get() != NO_PENDING_EFFECTS
        && !DISABLE_LEGACY_MODE
        && PENDING_EFFECTS_ROOT.with_borrow(|pending_effects_root| {
            pending_effects_root.as_ref().unwrap().tag == LEGACY_ROOT
        })
        && (EXECUTION_CONTEXT.get() & (RENDER_CONTEXT | COMMIT_CONTEXT)) == NO_CONTEXT
    {
        flush_pending_effects();
    }

    let prev_execution_context = EXECUTION_CONTEXT.get();
    EXECUTION_CONTEXT.set(EXECUTION_CONTEXT.get() | BATCHED_CONTEXT);

    let prev_transition = t(REACT_SHARED_INTERNALS);
    let previous_priority = get_current_update_priority();

    let _exit = ExitFlushSync {
        previous_priority,
        prev_transition,
        prev_execution_context,
    };
    set_current_update_priority(DISCRETE_EVENT_PRIORITY);
    set_t(REACT_SHARED_INTERNALS, js::null());
    f.map(|f| f())
}

// If called outside of a render or commit will flush all sync work on all roots
// Returns whether the the call was during a render or not
pub fn flush_sync_work() -> bool {
    if (EXECUTION_CONTEXT.get() & (RENDER_CONTEXT | COMMIT_CONTEXT)) == NO_CONTEXT {
        flush_sync_work_on_all_roots();
        return false;
    }
    true
}

pub fn is_already_rendering() -> bool {
    // Used by the renderer to print a warning if certain APIs are called from
    // the wrong context, and for profiling warnings.
    (EXECUTION_CONTEXT.get() & (RENDER_CONTEXT | COMMIT_CONTEXT)) != NO_CONTEXT
}

pub fn is_invalid_execution_context_for_event_function() -> bool {
    // Used to throw if certain APIs are called from the wrong context.
    (EXECUTION_CONTEXT.get() & RENDER_CONTEXT) != NO_CONTEXT
}

// This is called by the HiddenContext module when we enter or leave a
// hidden subtree. The stack logic is managed there because that's the only
// place that ever modifies it. Which module it lives in doesn't matter for
// performance because this function will get inlined regardless
pub fn set_entangled_render_lanes(new_entangled_render_lanes: Lanes) {
    ENTANGLED_RENDER_LANES.set(new_entangled_render_lanes);
}

pub fn get_entangled_render_lanes() -> Lanes {
    ENTANGLED_RENDER_LANES.get()
}

fn reset_work_in_progress_stack() {
    let Some(work_in_progress) = WORK_IN_PROGRESS.with_borrow(Clone::clone) else {
        return;
    };
    let mut interrupted_work;
    if WORK_IN_PROGRESS_SUSPENDED_REASON.get() == NOT_SUSPENDED {
        // Normal case. Work-in-progress hasn't started yet. Unwind all
        // its parents.
        interrupted_work = work_in_progress.r#return.borrow().clone();
    } else {
        // Work-in-progress is in suspended state. Reset the work loop and unwind
        // both the suspended fiber and all its parents.
        reset_suspended_work_loop_on_unwind(&work_in_progress);
        interrupted_work = Some(work_in_progress);
    }
    while let Some(work) = interrupted_work {
        let current = work.alternate.borrow().clone();
        unwind_interrupted_work(
            current.as_deref(),
            &work,
            WORK_IN_PROGRESS_ROOT_RENDER_LANES.get(),
        );
        interrupted_work = work.r#return.borrow().clone();
    }
    WORK_IN_PROGRESS.set(js::null());
}

fn finalize_render(lanes: Lanes, finalization_time: f64) {
    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK {
        if is_gesture_render(lanes) {
            clamp_gesture_timers(finalization_time);
        } else if includes_blocking_lane(lanes) {
            clamp_blocking_timers(finalization_time);
        }
        if includes_transition_lane(lanes) {
            clamp_transition_timers(finalization_time);
        }
        if includes_retry_lane(lanes) {
            clamp_retry_timers(finalization_time);
        }
        if includes_idle_group_lanes(lanes) {
            clamp_idle_timers(finalization_time);
        }
    }
}

fn prepare_fresh_stack(root: &Rc<FiberRoot>, lanes: Lanes) -> Rc<Fiber> {
    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK {
        // The order of tracks within a group are determined by the earliest start time.
        // Are tracks should show up in priority order and we should ideally always show
        // every track. This is a hack to ensure that we're displaying all tracks in the
        // right order. Ideally we could do this only once but because calls that aren't
        // recorded aren't considered for ordering purposes, we need to keep adding these
        // over and over again in case recording has just started. We can't tell when
        // recording starts.
        mark_all_lanes_in_order();

        let previous_render_start_time = RENDER_START_TIME.get();
        // Starting a new render. Log the end of any previous renders and the
        // blocked time before the render started.
        record_render_time();
        // If this was a restart, e.g. due to an interrupting update, then there's no space
        // in the track to log the cause since we'll have rendered all the way up until the
        // restart so we need to clamp that.
        if WORK_IN_PROGRESS_ROOT_RENDER_LANES.get() != NO_LANES && previous_render_start_time > 0.0
        {
            set_current_track_from_lanes(WORK_IN_PROGRESS_ROOT_RENDER_LANES.get());
            if WORK_IN_PROGRESS_ROOT_EXIT_STATUS.get() == ROOT_SUSPENDED
                || WORK_IN_PROGRESS_ROOT_EXIT_STATUS.get() == ROOT_SUSPENDED_WITH_DELAY
            {
                // If the root was already suspended when it got interrupted and restarted,
                // then this is considered a prewarm and not an interrupted render because
                // we couldn't have shown anything anyway so it's not a bad thing that we
                // got interrupted.
                log_suspended_render_phase(
                    previous_render_start_time,
                    RENDER_START_TIME.get(),
                    lanes,
                    WORK_IN_PROGRESS_UPDATE_TASK.get(),
                );
            } else {
                log_interrupted_render_phase(
                    previous_render_start_time,
                    RENDER_START_TIME.get(),
                    lanes,
                    WORK_IN_PROGRESS_UPDATE_TASK.get(),
                );
            }
            finalize_render(
                WORK_IN_PROGRESS_ROOT_RENDER_LANES.get(),
                RENDER_START_TIME.get(),
            );
        }
        let previous_update_task = WORK_IN_PROGRESS_UPDATE_TASK.get();

        WORK_IN_PROGRESS_UPDATE_TASK.set(js::null());
        if is_gesture_render(lanes) {
            WORK_IN_PROGRESS_UPDATE_TASK.set(GESTURE_UPDATE_TASK.get());
            let clamped_update_time = if GESTURE_UPDATE_TIME.get() >= 0.0
                && GESTURE_UPDATE_TIME.get() < GESTURE_CLAMP_TIME.get()
            {
                GESTURE_CLAMP_TIME.get()
            } else {
                GESTURE_UPDATE_TIME.get()
            };
            let clamped_event_time = if GESTURE_EVENT_TIME.get() >= 0.0
                && GESTURE_EVENT_TIME.get() < GESTURE_CLAMP_TIME.get()
            {
                GESTURE_CLAMP_TIME.get()
            } else {
                GESTURE_EVENT_TIME.get()
            };
            let clamped_render_start_time =
                // Clamp the suspended time to the first event/update.
                if clamped_event_time >= 0.0 {
                    clamped_event_time
                } else if clamped_update_time >= 0.0 {
                    clamped_update_time
                } else {
                    RENDER_START_TIME.get()
                };
            if GESTURE_SUSPENDED_TIME.get() >= 0.0 {
                set_current_track_from_lanes(GESTURE_LANE);
                log_suspended_with_delay_phase(
                    GESTURE_SUSPENDED_TIME.get(),
                    clamped_render_start_time,
                    lanes,
                    WORK_IN_PROGRESS_UPDATE_TASK.get(),
                );
            } else if is_gesture_render(ANIMATING_LANES.get()) {
                // If this lane is still animating, log the time from previous render finishing to now as animating.
                set_current_track_from_lanes(GESTURE_LANE);
                log_animating_phase(
                    GESTURE_CLAMP_TIME.get(),
                    clamped_render_start_time,
                    ANIMATING_TASK.get(),
                );
            }
            log_gesture_start(
                clamped_update_time,
                clamped_event_time,
                GESTURE_EVENT_TYPE.with_borrow(Clone::clone).as_deref(),
                GESTURE_EVENT_REPEAT_TIME.get() > 0.0,
                GESTURE_UPDATE_TYPE.get() == PINGED_UPDATE,
                RENDER_START_TIME.get(),
                GESTURE_UPDATE_TASK.get(),
                GESTURE_UPDATE_METHOD_NAME
                    .with_borrow(Clone::clone)
                    .as_deref(),
                GESTURE_UPDATE_COMPONENT_NAME
                    .with_borrow(Clone::clone)
                    .as_deref(),
            );
            clear_gesture_timers();
        } else if includes_blocking_lane(lanes) {
            WORK_IN_PROGRESS_UPDATE_TASK.set(BLOCKING_UPDATE_TASK.get());
            let clamped_update_time = if BLOCKING_UPDATE_TIME.get() >= 0.0
                && BLOCKING_UPDATE_TIME.get() < BLOCKING_CLAMP_TIME.get()
            {
                BLOCKING_CLAMP_TIME.get()
            } else {
                BLOCKING_UPDATE_TIME.get()
            };
            let clamped_event_time = if BLOCKING_EVENT_TIME.get() >= 0.0
                && BLOCKING_EVENT_TIME.get() < BLOCKING_CLAMP_TIME.get()
            {
                BLOCKING_CLAMP_TIME.get()
            } else {
                BLOCKING_EVENT_TIME.get()
            };
            let clamped_render_start_time = // Clamp the suspended time to the first event/update.
                if clamped_event_time >= 0.0 {
                    clamped_event_time
                } else if clamped_update_time >= 0.0 {
                    clamped_update_time
                } else {
                    RENDER_START_TIME.get()
                };
            if BLOCKING_SUSPENDED_TIME.get() >= 0.0 {
                set_current_track_from_lanes(SYNC_LANE);
                log_suspended_with_delay_phase(
                    BLOCKING_SUSPENDED_TIME.get(),
                    clamped_render_start_time,
                    lanes,
                    previous_update_task,
                );
            } else if !is_gesture_render(ANIMATING_LANES.get())
                && includes_blocking_lane(ANIMATING_LANES.get())
            {
                // If this lane is still animating, log the time from previous render finishing to now as animating.
                set_current_track_from_lanes(SYNC_LANE);
                log_animating_phase(
                    BLOCKING_CLAMP_TIME.get(),
                    clamped_render_start_time,
                    ANIMATING_TASK.get(),
                );
            }
            log_blocking_start(
                clamped_update_time,
                clamped_event_time,
                BLOCKING_EVENT_TYPE.with_borrow(Clone::clone).as_deref(),
                BLOCKING_EVENT_REPEAT_TIME.get() > 0.0,
                BLOCKING_UPDATE_TYPE.get() == SPAWNED_UPDATE,
                BLOCKING_UPDATE_TYPE.get() == PINGED_UPDATE,
                RENDER_START_TIME.get(),
                lanes,
                BLOCKING_UPDATE_TASK.get(),
                BLOCKING_UPDATE_METHOD_NAME
                    .with_borrow(Clone::clone)
                    .as_deref(),
                BLOCKING_UPDATE_COMPONENT_NAME
                    .with_borrow(Clone::clone)
                    .as_deref(),
            );
            clear_blocking_timers();
        }
        if includes_transition_lane(lanes) {
            WORK_IN_PROGRESS_UPDATE_TASK.set(TRANSITION_UPDATE_TASK.get());
            let clamped_start_time = if TRANSITION_START_TIME.get() >= 0.0
                && TRANSITION_START_TIME.get() < TRANSITION_CLAMP_TIME.get()
            {
                TRANSITION_CLAMP_TIME.get()
            } else {
                TRANSITION_START_TIME.get()
            };
            let clamped_update_time = if TRANSITION_UPDATE_TIME.get() >= 0.0
                && TRANSITION_UPDATE_TIME.get() < TRANSITION_CLAMP_TIME.get()
            {
                TRANSITION_CLAMP_TIME.get()
            } else {
                TRANSITION_UPDATE_TIME.get()
            };
            let clamped_event_time = if TRANSITION_EVENT_TIME.get() >= 0.0
                && TRANSITION_EVENT_TIME.get() < TRANSITION_CLAMP_TIME.get()
            {
                TRANSITION_CLAMP_TIME.get()
            } else {
                TRANSITION_EVENT_TIME.get()
            };
            let clamped_render_start_time =
                // Clamp the suspended time to the first event/update.
                if clamped_event_time >= 0.0 {
                    clamped_event_time
                } else if clamped_update_time >= 0.0 {
                    clamped_update_time
                } else {
                    RENDER_START_TIME.get()
                };
            if TRANSITION_SUSPENDED_TIME.get() >= 0.0 {
                set_current_track_from_lanes(SOME_TRANSITION_LANE);
                log_suspended_with_delay_phase(
                    TRANSITION_SUSPENDED_TIME.get(),
                    clamped_render_start_time,
                    lanes,
                    WORK_IN_PROGRESS_UPDATE_TASK.get(),
                );
            } else if includes_transition_lane(ANIMATING_LANES.get()) {
                // If this lane is still animating, log the time from previous render finishing to now as animating.
                set_current_track_from_lanes(SOME_TRANSITION_LANE);
                log_animating_phase(
                    TRANSITION_CLAMP_TIME.get(),
                    clamped_render_start_time,
                    ANIMATING_TASK.get(),
                );
            }
            log_transition_start(
                clamped_start_time,
                clamped_update_time,
                clamped_event_time,
                TRANSITION_EVENT_TYPE.with_borrow(Clone::clone).as_deref(),
                TRANSITION_EVENT_REPEAT_TIME.get() > 0.0,
                TRANSITION_UPDATE_TYPE.get() == PINGED_UPDATE,
                RENDER_START_TIME.get(),
                TRANSITION_UPDATE_TASK.get(),
                TRANSITION_UPDATE_METHOD_NAME
                    .with_borrow(Clone::clone)
                    .as_deref(),
                TRANSITION_UPDATE_COMPONENT_NAME
                    .with_borrow(Clone::clone)
                    .as_deref(),
            );
            clear_transition_timers();
        }
        if includes_retry_lane(lanes) && includes_retry_lane(ANIMATING_LANES.get()) {
            // If this lane is still animating, log the time from previous render finishing to now as animating.
            set_current_track_from_lanes(SOME_RETRY_LANE);
            log_animating_phase(
                RETRY_CLAMP_TIME.get(),
                RENDER_START_TIME.get(),
                ANIMATING_TASK.get(),
            );
        }
        if includes_idle_group_lanes(lanes) && includes_idle_group_lanes(ANIMATING_LANES.get()) {
            // If this lane is still animating, log the time from previous render finishing to now as animating.
            set_current_track_from_lanes(IDLE_LANE);
            log_animating_phase(
                IDLE_CLAMP_TIME.get(),
                RENDER_START_TIME.get(),
                ANIMATING_TASK.get(),
            );
        }
    }

    let timeout_handle = root.timeout_handle.get();
    if !js::strict_equals(timeout_handle, NO_TIMEOUT) {
        // The root previous suspended and scheduled a timeout to commit a fallback
        // state. Now that we have additional work, cancel the timeout.
        root.timeout_handle.set(NO_TIMEOUT);
        cancel_timeout(timeout_handle);
    }
    let cancel_pending_commit = root.cancel_pending_commit.take();
    if let Some(cancel_pending_commit) = cancel_pending_commit {
        cancel_pending_commit();
    }

    PENDING_EFFECTS_LANES.set(NO_LANES);

    reset_work_in_progress_stack();
    WORK_IN_PROGRESS_ROOT.set(Some(Rc::clone(root)));
    let current = root.current.borrow().clone();
    let root_work_in_progress = create_work_in_progress(&current, js::null());
    WORK_IN_PROGRESS.set(Some(Rc::clone(&root_work_in_progress)));
    WORK_IN_PROGRESS_ROOT_RENDER_LANES.set(lanes);
    WORK_IN_PROGRESS_SUSPENDED_REASON.set(NOT_SUSPENDED);
    WORK_IN_PROGRESS_THROWN_VALUE.set(js::null());
    WORK_IN_PROGRESS_ROOT_DID_SKIP_SUSPENDED_SIBLINGS.set(false);
    WORK_IN_PROGRESS_ROOT_IS_PRERENDERING.set(check_if_root_is_prerendering(root, lanes));
    WORK_IN_PROGRESS_ROOT_DID_ATTACH_PING_LISTENER.set(false);
    WORK_IN_PROGRESS_ROOT_EXIT_STATUS.set(ROOT_IN_PROGRESS);
    WORK_IN_PROGRESS_ROOT_SKIPPED_LANES.set(NO_LANES);
    WORK_IN_PROGRESS_ROOT_INTERLEAVED_UPDATED_LANES.set(NO_LANES);
    WORK_IN_PROGRESS_ROOT_RENDER_PHASE_UPDATED_LANES.set(NO_LANES);
    WORK_IN_PROGRESS_ROOT_PINGED_LANES.set(NO_LANES);
    WORK_IN_PROGRESS_DEFERRED_LANE.set(NO_LANE);
    WORK_IN_PROGRESS_SUSPENDED_RETRY_LANES.set(NO_LANES);
    WORK_IN_PROGRESS_ROOT_CONCURRENT_ERRORS.set(js::null());
    WORK_IN_PROGRESS_ROOT_RECOVERABLE_ERRORS.set(js::null());
    WORK_IN_PROGRESS_ROOT_DID_INCLUDE_RECURSIVE_RENDER_UPDATE.set(false);

    // Get the lanes that are entangled with whatever we're about to render. We
    // track these separately so we can distinguish the priority of the render
    // task from the priority of the lanes it is entangled with. For example, a
    // transition may not be allowed to finish unless it includes the Sync lane,
    // which is currently suspended. We should be able to render the Transition
    // and Sync lane in the same batch, but at Transition priority, because the
    // Sync lane already suspended.
    ENTANGLED_RENDER_LANES.set(get_entangled_lanes(root, lanes));

    finish_queueing_concurrent_updates();

    if __DEV__ {
        reset_owner_stack_limit();

        REACT_STRICT_MODE_WARNINGS.with(|react_strict_mode_warnings| {
            (react_strict_mode_warnings.discard_pending_warnings.get())();
        });
    }

    root_work_in_progress
}

fn reset_suspended_work_loop_on_unwind(fiber: &Fiber) {
    // Reset module-level state that was set during the render phase.
    reset_context_dependencies();
    reset_hooks_on_unwind(fiber);
    reset_child_reconciler_on_unwind();
}

fn handle_throw(root: &FiberRoot, mut thrown_value: Option<&'static Unknown>) {
    // A component threw an exception. Usually this is because it suspended, but
    // it also includes regular program errors.
    //
    // We're either going to unwind the stack to show a Suspense or error
    // boundary, or we're going to replay the component again. Like after a
    // promise resolves.
    //
    // Until we decide whether we're going to unwind or replay, we should preserve
    // the current state of the work loop without resetting anything.
    //
    // If we do decide to unwind the stack, module-level variables will be reset
    // in resetSuspendedWorkLoopOnUnwind.

    // These should be reset immediately because they're only supposed to be set
    // when React is executing user code.
    reset_hooks_after_throw();
    if __DEV__ {
        reset_current_fiber();
    }

    if SUSPENSE_EXCEPTION.with(|exception| js::strict_equals(thrown_value, *exception))
        || SUSPENSE_ACTION_EXCEPTION.with(|exception| js::strict_equals(thrown_value, *exception))
    {
        // This is a special type of exception used for Suspense. For historical
        // reasons, the rest of the Suspense implementation expects the thrown value
        // to be a thenable, because before `use` existed that was the (unstable)
        // API for suspending. This implementation detail can change later, once we
        // deprecate the old API in favor of `use`.
        thrown_value = Some(js::unknown(get_suspended_thenable()));
        // TODO: Suspending the work loop during the render phase is
        // currently not compatible with sibling prerendering. We will add
        // this optimization back in a later step.
        // Don't suspend work loop, except to check if the data has
        // immediately resolved (i.e. in a microtask). Otherwise, trigger the
        // nearest Suspense fallback.
        WORK_IN_PROGRESS_SUSPENDED_REASON.set(SUSPENDED_ON_IMMEDIATE);
    } else if SUSPENSEY_COMMIT_EXCEPTION
        .with(|exception| js::strict_equals(thrown_value, *exception))
    {
        thrown_value = Some(js::unknown(get_suspended_thenable()));
        WORK_IN_PROGRESS_SUSPENDED_REASON.set(SUSPENDED_ON_INSTANCE);
    } else if SELECTIVE_HYDRATION_EXCEPTION
        .with(|exception| js::strict_equals(thrown_value, *exception))
    {
        // An update flowed into a dehydrated boundary. Before we can apply the
        // update, we need to finish hydrating. Interrupt the work-in-progress
        // render so we can restart at the hydration lane.
        //
        // The ideal implementation would be able to switch contexts without
        // unwinding the current stack.
        //
        // We could name this something more general but as of now it's the only
        // case where we think this should happen.
        WORK_IN_PROGRESS_SUSPENDED_REASON.set(SUSPENDED_ON_HYDRATION);
    } else {
        // This is a regular error.
        let is_wakeable = thrown_value.is_some()
            && js::type_of(thrown_value) == "object"
            && js::type_of(js::get(thrown_value, "then")) == "function";

        WORK_IN_PROGRESS_SUSPENDED_REASON.set(if is_wakeable {
            // A wakeable object was thrown by a legacy Suspense implementation.
            // This has slightly different behavior than suspending with `use`.
            SUSPENDED_ON_DEPRECATED_THROW_PROMISE
        } else {
            // This is a regular error. If something earlier in the component already
            // suspended, we must clear the thenable state to unblock the work loop.
            SUSPENDED_ON_ERROR
        });
    }

    WORK_IN_PROGRESS_THROWN_VALUE.set(thrown_value);

    let errored_work = WORK_IN_PROGRESS.with_borrow(Clone::clone);
    let Some(errored_work) = errored_work else {
        // This is a fatal error
        WORK_IN_PROGRESS_ROOT_EXIT_STATUS.set(ROOT_FATAL_ERRORED);
        let current = root.current.borrow().clone();
        log_uncaught_error(
            root,
            &create_captured_value_at_fiber(thrown_value, &current),
        );
        return;
    };

    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        && (errored_work.mode.get() & PROFILE_MODE) != 0
    {
        // Record the time spent rendering before an error was thrown. This
        // avoids inaccurate Profiler durations in the case of a
        // suspended render.
        stop_profiler_timer_if_running_and_record_duration(&errored_work);
    }

    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled) {
        mark_component_render_stopped();
        match WORK_IN_PROGRESS_SUSPENDED_REASON.get() {
            SUSPENDED_ON_ERROR => {
                mark_component_errored(
                    &errored_work,
                    thrown_value,
                    WORK_IN_PROGRESS_ROOT_RENDER_LANES.get(),
                );
            }
            SUSPENDED_ON_DATA
            | SUSPENDED_ON_ACTION
            | SUSPENDED_ON_IMMEDIATE
            | SUSPENDED_ON_DEPRECATED_THROW_PROMISE
            | SUSPENDED_AND_READY_TO_CONTINUE => {
                let wakeable: &'static Unknown = unsafe { js::cast(thrown_value) };
                mark_component_suspended(
                    &errored_work,
                    wakeable,
                    WORK_IN_PROGRESS_ROOT_RENDER_LANES.get(),
                );
            }
            SUSPENDED_ON_INSTANCE => {
                // This is conceptually like a suspend, but it's not associated with
                // a particular wakeable. It's associated with a host resource (e.g.
                // a CSS file or an image) that hasn't loaded yet. DevTools doesn't
                // handle this currently.
            }
            SUSPENDED_ON_HYDRATION => {
                // This is conceptually like a suspend, but it's not associated with
                // a particular wakeable. DevTools doesn't seem to care about this case,
                // currently. It's similar to if the component were interrupted, which
                // we don't mark with a special function.
            }
            _ => {}
        }
    }
}

pub fn should_remain_on_previous_screen() -> bool {
    // This is asking whether it's better to suspend the transition and remain
    // on the previous screen, versus showing a fallback as soon as possible. It
    // takes into account both the priority of render and also whether showing a
    // fallback would produce a desirable user experience.

    let handler = get_suspense_handler();
    let Some(handler) = handler else {
        // There's no Suspense boundary that can provide a fallback. We have no
        // choice but to remain on the previous screen.
        // NOTE: We do this even for sync updates, for lack of any better option. In
        // the future, we may change how we handle this, like by putting the whole
        // root into a "detached" mode.
        return true;
    };

    // TODO: Once `use` has fully replaced the `throw promise` pattern, we should
    // be able to remove the equivalent check in finishConcurrentRender, and rely
    // just on this one.
    if includes_only_transitions(WORK_IN_PROGRESS_ROOT_RENDER_LANES.get()) {
        if get_shell_boundary().is_none() {
            // We're rendering inside the "shell" of the app. Activating the nearest
            // fallback would cause visible content to disappear. It's better to
            // suspend the transition and remain on the previous screen.
            return true;
        } else {
            // We're rendering content that wasn't part of the previous screen.
            // Rather than block the transition, it's better to show a fallback as
            // soon as possible. The appearance of any nested fallbacks will be
            // throttled to avoid jank.
            return false;
        }
    }

    if includes_only_retries(WORK_IN_PROGRESS_ROOT_RENDER_LANES.get())
        // In this context, an OffscreenLane counts as a Retry
        // TODO: It's become increasingly clear that Retries and Offscreen are
        // deeply connected. They probably can be unified further.
        || includes_some_lane(WORK_IN_PROGRESS_ROOT_RENDER_LANES.get(), OFFSCREEN_LANE)
    {
        // During a retry, we can suspend rendering if the nearest Suspense boundary
        // is the boundary of the "shell", because we're guaranteed not to block
        // any new content from appearing.
        //
        // The reason we must check if this is a retry is because it guarantees
        // that suspending the work loop won't block an actual update, because
        // retries don't "update" anything; they fill in fallbacks that were left
        // behind by a previous transition.
        return get_shell_boundary()
            .is_some_and(|shell_boundary| Rc::ptr_eq(&handler, &shell_boundary));
    }

    // For all other Lanes besides Transitions and Retries, we should not wait
    // for the data to load.
    false
}

fn push_dispatcher(_container: &'static Unknown) -> &'static SharedDispatcher {
    let prev_dispatcher = REACT_SHARED_INTERNALS.h();
    REACT_SHARED_INTERNALS.set_h(Some(&CONTEXT_ONLY_DISPATCHER));
    match prev_dispatcher {
        // The React isomorphic package does not include a default dispatcher.
        // Instead the first renderer will lazily attach one, in order to give
        // nicer error messages.
        None => unsafe { js::cast(Some(js::unknown(&CONTEXT_ONLY_DISPATCHER))) },
        Some(prev_dispatcher) => prev_dispatcher,
    }
}

fn pop_dispatcher(prev_dispatcher: &'static SharedDispatcher) {
    REACT_SHARED_INTERNALS.set_h(Some(prev_dispatcher));
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn push_async_dispatcher() -> Option<&'static AsyncDispatcher> {
    let prev_async_dispatcher = REACT_SHARED_INTERNALS.a();
    DEFAULT_ASYNC_DISPATCHER.with(|default_async_dispatcher| {
        REACT_SHARED_INTERNALS.set_a(Some(default_async_dispatcher));
    });
    prev_async_dispatcher
}

#[cfg_attr(rust_js, rust_js::nullable(prev_async_dispatcher))]
fn pop_async_dispatcher(prev_async_dispatcher: Option<&'static AsyncDispatcher>) {
    REACT_SHARED_INTERNALS.set_a(prev_async_dispatcher);
}

pub fn mark_render_derived_cause(fiber: &Fiber) {
    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        && ENABLE_COMPONENT_PERFORMANCE_TRACK
        && __DEV__
        && WORK_IN_PROGRESS_UPDATE_TASK.get().is_none()
    {
        // If we don't have a cause associated with this render, it's likely because some
        // other render left work behind on this Fiber. The real cause is this Fiber itself.
        // We use its debugTask as the cause for this render. This might not be the only
        // one when multiple siblings are rendered but they ideally shouldn't be.
        WORK_IN_PROGRESS_UPDATE_TASK.set(fiber.debug_task.get());
    }
}

pub fn mark_transition_started() {
    GLOBAL_MOST_RECENT_TRANSITION_TIME.set(now());
}

pub fn mark_commit_time_of_fallback() {
    GLOBAL_MOST_RECENT_FALLBACK_TIME.set(now());
}

pub fn mark_skipped_update_lanes(lane: Lanes) {
    WORK_IN_PROGRESS_ROOT_SKIPPED_LANES
        .set(merge_lanes(lane, WORK_IN_PROGRESS_ROOT_SKIPPED_LANES.get()));
}

pub fn render_did_suspend() {
    if WORK_IN_PROGRESS_ROOT_EXIT_STATUS.get() == ROOT_IN_PROGRESS {
        WORK_IN_PROGRESS_ROOT_EXIT_STATUS.set(ROOT_SUSPENDED);
    }
}

pub fn render_did_suspend_delay_if_possible() {
    WORK_IN_PROGRESS_ROOT_EXIT_STATUS.set(ROOT_SUSPENDED_WITH_DELAY);

    if !WORK_IN_PROGRESS_ROOT_DID_SKIP_SUSPENDED_SIBLINGS.get()
        // Check if the root will be blocked from committing.
        // TODO: Consider aligning this better with the rest of the logic. Maybe
        // we should only set the exit status to RootSuspendedWithDelay if this
        // condition is true? And remove the equivalent checks elsewhere.
        && (includes_only_transitions(WORK_IN_PROGRESS_ROOT_RENDER_LANES.get())
            || get_suspense_handler().is_none())
    {
        // This render may not have originally been scheduled as a prerender, but
        // something suspended inside the visible part of the tree, which means we
        // won't be able to commit a fallback anyway. Let's proceed as if this were
        // a prerender so that we can warm up the siblings without scheduling a
        // separate pass.
        WORK_IN_PROGRESS_ROOT_IS_PRERENDERING.set(true);
    }

    // Check if there are updates that we skipped tree that might have unblocked
    // this render.
    if (includes_non_idle_work(WORK_IN_PROGRESS_ROOT_SKIPPED_LANES.get())
        || includes_non_idle_work(WORK_IN_PROGRESS_ROOT_INTERLEAVED_UPDATED_LANES.get()))
        && let Some(work_in_progress_root) = WORK_IN_PROGRESS_ROOT.with_borrow(Clone::clone)
    {
        // Mark the current render as suspended so that we switch to working on
        // the updates that were skipped. Usually we only suspend at the end of
        // the render phase.
        // TODO: We should probably always mark the root as suspended immediately
        // (inside this function), since by suspending at the end of the render
        // phase introduces a potential mistake where we suspend lanes that were
        // pinged or updated while we were rendering.
        // TODO: Consider unwinding immediately, using the
        // SuspendedOnHydration mechanism.
        let did_attempt_entire_tree = false;
        mark_root_suspended(
            &work_in_progress_root,
            WORK_IN_PROGRESS_ROOT_RENDER_LANES.get(),
            WORK_IN_PROGRESS_DEFERRED_LANE.get(),
            did_attempt_entire_tree,
        );
    }
}

pub fn render_did_error() {
    if WORK_IN_PROGRESS_ROOT_EXIT_STATUS.get() != ROOT_SUSPENDED_WITH_DELAY {
        WORK_IN_PROGRESS_ROOT_EXIT_STATUS.set(ROOT_ERRORED);
    }
}

pub fn queue_concurrent_error(error: Rc<CapturedValue<Option<&'static Unknown>>>) {
    WORK_IN_PROGRESS_ROOT_CONCURRENT_ERRORS.with_borrow_mut(
        |work_in_progress_root_concurrent_errors| match work_in_progress_root_concurrent_errors {
            None => {
                *work_in_progress_root_concurrent_errors = Some(Rc::new(RefCell::new(vec![error])))
            }
            Some(work_in_progress_root_concurrent_errors) => {
                work_in_progress_root_concurrent_errors
                    .borrow_mut()
                    .push(error);
            }
        },
    );
}

// Called during render to determine if anything has suspended.
// Returns false if we're not sure.
pub fn render_has_not_suspended_yet() -> bool {
    // If something errored or completed, we can't really be sure,
    // so those are false.
    WORK_IN_PROGRESS_ROOT_EXIT_STATUS.get() == ROOT_IN_PROGRESS
}

// TODO: Over time, this function and renderRootConcurrent have become more
// and more similar. Not sure it makes sense to maintain forked paths. Consider
// unifying them again.
fn render_root_sync(
    root: &Rc<FiberRoot>,
    lanes: Lanes,
    should_yield_for_prerendering: bool,
) -> RootExitStatus {
    let prev_execution_context = EXECUTION_CONTEXT.get();
    EXECUTION_CONTEXT.set(EXECUTION_CONTEXT.get() | RENDER_CONTEXT);
    let prev_dispatcher = push_dispatcher(root.container_info);
    let prev_async_dispatcher = push_async_dispatcher();

    // If the root or lanes have changed, throw out the existing stack
    // and prepare a fresh one. Otherwise we'll continue where we left off.
    if !WORK_IN_PROGRESS_ROOT.with_borrow(|work_in_progress_root| {
        work_in_progress_root
            .as_ref()
            .is_some_and(|work_in_progress_root| Rc::ptr_eq(work_in_progress_root, root))
    }) || WORK_IN_PROGRESS_ROOT_RENDER_LANES.get() != lanes
    {
        if ENABLE_UPDATER_TRACKING.with(|&enabled| enabled)
            && IS_DEV_TOOLS_PRESENT.with(|&present| present)
        {
            let memoized_updaters = &root.memoized_updaters;
            if !memoized_updaters.borrow().is_empty() {
                restore_pending_updaters(root, WORK_IN_PROGRESS_ROOT_RENDER_LANES.get());
                memoized_updaters.borrow_mut().clear();
            }

            // At this point, move Fibers that scheduled the upcoming work from the Map to the Set.
            // If we bailout on this work, we'll move them back (like above).
            // It's important to move them now in case the work spawns more work at the same priority with different updaters.
            // That way we can keep the current update and future updates separate.
            move_pending_fibers_to_memoized(root, lanes);
        }

        WORK_IN_PROGRESS_TRANSITIONS.set(get_transitions_for_lanes(root, lanes).map(Rc::new));
        prepare_fresh_stack(root, lanes);
    }

    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled) {
        mark_render_started(lanes);
    }

    let mut did_suspend_in_shell = false;
    let mut exit_status = WORK_IN_PROGRESS_ROOT_EXIT_STATUS.get();
    loop {
        match catch_unwind(AssertUnwindSafe(|| {
            if WORK_IN_PROGRESS_SUSPENDED_REASON.get() != NOT_SUSPENDED
                && let Some(unit_of_work) = WORK_IN_PROGRESS.with_borrow(Clone::clone)
            {
                // The work loop is suspended. During a synchronous render, we don't
                // yield to the main thread. Immediately unwind the stack. This will
                // trigger either a fallback or an error boundary.
                // TODO: For discrete and "default" updates (anything that's not
                // flushSync), we want to wait for the microtasks the flush before
                // unwinding. Will probably implement this using renderRootConcurrent,
                // or merge renderRootSync and renderRootConcurrent into the same
                // function and fork the behavior some other way.
                let thrown_value = WORK_IN_PROGRESS_THROWN_VALUE.get();
                match WORK_IN_PROGRESS_SUSPENDED_REASON.get() {
                    SUSPENDED_ON_HYDRATION => {
                        // Selective hydration. An update flowed into a dehydrated tree.
                        // Interrupt the current render so the work loop can switch to the
                        // hydration lane.
                        // TODO: I think we might not need to reset the stack here; we can
                        // just yield and reset the stack when we re-enter the work loop,
                        // like normal.
                        reset_work_in_progress_stack();
                        exit_status = ROOT_SUSPENDED_AT_THE_SHELL;
                        return;
                    }
                    SUSPENDED_ON_IMMEDIATE
                    | SUSPENDED_ON_DATA
                    | SUSPENDED_ON_ACTION
                    | SUSPENDED_ON_DEPRECATED_THROW_PROMISE => {
                        if get_suspense_handler().is_none() {
                            did_suspend_in_shell = true;
                        }
                        let reason = WORK_IN_PROGRESS_SUSPENDED_REASON.get();
                        WORK_IN_PROGRESS_SUSPENDED_REASON.set(NOT_SUSPENDED);
                        WORK_IN_PROGRESS_THROWN_VALUE.set(js::null());
                        throw_and_unwind_work_loop(root, &unit_of_work, thrown_value, reason);
                        if should_yield_for_prerendering
                            && WORK_IN_PROGRESS_ROOT_IS_PRERENDERING.get()
                        {
                            // We've switched into prerendering mode. This implies that we
                            // suspended outside of a Suspense boundary, which means this
                            // render will be blocked from committing. Yield to the main
                            // thread so we can switch to prerendering using the concurrent
                            // work loop.
                            exit_status = ROOT_IN_PROGRESS;
                            return;
                        }
                    }
                    _ => {
                        // Unwind then continue with the normal work loop.
                        let reason = WORK_IN_PROGRESS_SUSPENDED_REASON.get();
                        WORK_IN_PROGRESS_SUSPENDED_REASON.set(NOT_SUSPENDED);
                        WORK_IN_PROGRESS_THROWN_VALUE.set(js::null());
                        throw_and_unwind_work_loop(root, &unit_of_work, thrown_value, reason);
                    }
                }
            }
            work_loop_sync();
            exit_status = WORK_IN_PROGRESS_ROOT_EXIT_STATUS.get();
        })) {
            Ok(()) => break,
            Err(thrown_value) => {
                handle_throw(
                    root,
                    thrown_value
                        .downcast_ref::<&'static JsError>()
                        .and_then(|error| error.as_unknown()),
                );
            }
        }
    }

    // Check if something suspended in the shell. We use this to detect an
    // infinite ping loop caused by an uncached promise.
    //
    // Only increment this counter once per synchronous render attempt across the
    // whole tree. Even if there are many sibling components that suspend, this
    // counter only gets incremented once.
    if did_suspend_in_shell {
        root.shell_suspend_counter
            .set(root.shell_suspend_counter.get() + 1);
    }

    reset_context_dependencies();

    EXECUTION_CONTEXT.set(prev_execution_context);
    pop_dispatcher(prev_dispatcher);
    pop_async_dispatcher(prev_async_dispatcher);

    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled) {
        mark_render_stopped();
    }

    if WORK_IN_PROGRESS.with_borrow(Option::is_some) {
        // Did not complete the tree. This can happen if something suspended in
        // the shell.
    } else {
        // Normal case. We completed the whole tree.

        // Set this to null to indicate there's no in-progress render.
        WORK_IN_PROGRESS_ROOT.set(js::null());
        WORK_IN_PROGRESS_ROOT_RENDER_LANES.set(NO_LANES);

        // It's safe to process the queue now that the render phase is complete.
        finish_queueing_concurrent_updates();
    }

    exit_status
}

// The work loop is an extremely hot path. Tell Closure not to inline it.
/** @noinline */
fn work_loop_sync() {
    // Perform work without checking if we need to yield between fiber.
    while let Some(work_in_progress) = WORK_IN_PROGRESS.with_borrow(Clone::clone) {
        perform_unit_of_work(&work_in_progress);
    }
}

fn render_root_concurrent(root: &Rc<FiberRoot>, lanes: Lanes) -> RootExitStatus {
    let prev_execution_context = EXECUTION_CONTEXT.get();
    EXECUTION_CONTEXT.set(EXECUTION_CONTEXT.get() | RENDER_CONTEXT);
    let prev_dispatcher = push_dispatcher(root.container_info);
    let prev_async_dispatcher = push_async_dispatcher();

    // If the root or lanes have changed, throw out the existing stack
    // and prepare a fresh one. Otherwise we'll continue where we left off.
    if !WORK_IN_PROGRESS_ROOT.with_borrow(|work_in_progress_root| {
        work_in_progress_root
            .as_ref()
            .is_some_and(|work_in_progress_root| Rc::ptr_eq(work_in_progress_root, root))
    }) || WORK_IN_PROGRESS_ROOT_RENDER_LANES.get() != lanes
    {
        if ENABLE_UPDATER_TRACKING.with(|&enabled| enabled)
            && IS_DEV_TOOLS_PRESENT.with(|&present| present)
        {
            let memoized_updaters = &root.memoized_updaters;
            if !memoized_updaters.borrow().is_empty() {
                restore_pending_updaters(root, WORK_IN_PROGRESS_ROOT_RENDER_LANES.get());
                memoized_updaters.borrow_mut().clear();
            }

            // At this point, move Fibers that scheduled the upcoming work from the Map to the Set.
            // If we bailout on this work, we'll move them back (like above).
            // It's important to move them now in case the work spawns more work at the same priority with different updaters.
            // That way we can keep the current update and future updates separate.
            move_pending_fibers_to_memoized(root, lanes);
        }

        WORK_IN_PROGRESS_TRANSITIONS.set(get_transitions_for_lanes(root, lanes).map(Rc::new));

        reset_render_timer();
        prepare_fresh_stack(root, lanes);
    } else {
        // This is a continuation of an existing work-in-progress.
        //
        // If we were previously in prerendering mode, check if we received any new
        // data during an interleaved event.
        WORK_IN_PROGRESS_ROOT_IS_PRERENDERING.set(check_if_root_is_prerendering(root, lanes));
    }

    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled) {
        mark_render_started(lanes);
    }

    loop {
        match catch_unwind(AssertUnwindSafe(|| {
            if WORK_IN_PROGRESS_SUSPENDED_REASON.get() != NOT_SUSPENDED
                && let Some(unit_of_work) = WORK_IN_PROGRESS.with_borrow(Clone::clone)
            {
                // The work loop is suspended. We need to either unwind the stack or
                // replay the suspended component.
                let thrown_value = WORK_IN_PROGRESS_THROWN_VALUE.get();
                match WORK_IN_PROGRESS_SUSPENDED_REASON.get() {
                    SUSPENDED_ON_ERROR => {
                        // Unwind then continue with the normal work loop.
                        WORK_IN_PROGRESS_SUSPENDED_REASON.set(NOT_SUSPENDED);
                        WORK_IN_PROGRESS_THROWN_VALUE.set(js::null());
                        throw_and_unwind_work_loop(
                            root,
                            &unit_of_work,
                            thrown_value,
                            SUSPENDED_ON_ERROR,
                        );
                    }
                    SUSPENDED_ON_DATA | SUSPENDED_ON_ACTION => {
                        let thenable: &'static Thenable = unsafe { js::cast(thrown_value) };
                        if is_thenable_resolved(thenable) {
                            // The data resolved. Try rendering the component again.
                            WORK_IN_PROGRESS_SUSPENDED_REASON.set(NOT_SUSPENDED);
                            WORK_IN_PROGRESS_THROWN_VALUE.set(js::null());
                            replay_suspended_unit_of_work(&unit_of_work);
                        } else {
                            // The work loop is suspended on data. We should wait for it to
                            // resolve before continuing to render.
                            // TODO: Handle the case where the promise resolves synchronously.
                            // Usually this is handled when we instrument the promise to add a
                            // `status` field, but if the promise already has a status, we won't
                            // have added a listener until right here.
                            let root = Rc::clone(root);
                            let on_resolution = move |_: Option<&'static Unknown>| {
                                // Check if the root is still suspended on this promise.
                                if (WORK_IN_PROGRESS_SUSPENDED_REASON.get() == SUSPENDED_ON_DATA
                                    || WORK_IN_PROGRESS_SUSPENDED_REASON.get()
                                        == SUSPENDED_ON_ACTION)
                                    && WORK_IN_PROGRESS_ROOT.with_borrow(|work_in_progress_root| {
                                        work_in_progress_root.as_ref().is_some_and(
                                            |work_in_progress_root| {
                                                Rc::ptr_eq(work_in_progress_root, &root)
                                            },
                                        )
                                    })
                                {
                                    // Mark the root as ready to continue rendering.
                                    WORK_IN_PROGRESS_SUSPENDED_REASON
                                        .set(SUSPENDED_AND_READY_TO_CONTINUE);
                                }
                                // Ensure the root is scheduled. We should do this even if we're
                                // currently working on a different root, so that we resume
                                // rendering later.
                                ensure_root_is_scheduled(&root);
                            };
                            thenable.then(on_resolution.clone(), on_resolution);
                            return;
                        }
                    }
                    SUSPENDED_ON_IMMEDIATE => {
                        // If this fiber just suspended, it's possible the data is already
                        // cached. Yield to the main thread to give it a chance to ping. If
                        // it does, we can retry immediately without unwinding the stack.
                        WORK_IN_PROGRESS_SUSPENDED_REASON.set(SUSPENDED_AND_READY_TO_CONTINUE);
                        return;
                    }
                    SUSPENDED_ON_INSTANCE => {
                        WORK_IN_PROGRESS_SUSPENDED_REASON
                            .set(SUSPENDED_ON_INSTANCE_AND_READY_TO_CONTINUE);
                        return;
                    }
                    SUSPENDED_AND_READY_TO_CONTINUE => {
                        let thenable: &Thenable = unsafe { js::cast(thrown_value) };
                        if is_thenable_resolved(thenable) {
                            // The data resolved. Try rendering the component again.
                            WORK_IN_PROGRESS_SUSPENDED_REASON.set(NOT_SUSPENDED);
                            WORK_IN_PROGRESS_THROWN_VALUE.set(js::null());
                            replay_suspended_unit_of_work(&unit_of_work);
                        } else {
                            // Otherwise, unwind then continue with the normal work loop.
                            WORK_IN_PROGRESS_SUSPENDED_REASON.set(NOT_SUSPENDED);
                            WORK_IN_PROGRESS_THROWN_VALUE.set(js::null());
                            throw_and_unwind_work_loop(
                                root,
                                &unit_of_work,
                                thrown_value,
                                SUSPENDED_AND_READY_TO_CONTINUE,
                            );
                        }
                    }
                    SUSPENDED_ON_INSTANCE_AND_READY_TO_CONTINUE => 'resume_or_unwind: {
                        // ReactFiberConfig's `Resource`.
                        let mut resource: Option<&'static Unknown> = js::null();
                        match unit_of_work.tag.get() {
                            tag @ (HOST_HOISTABLE | HOST_COMPONENT | HOST_SINGLETON) => {
                                if tag == HOST_HOISTABLE {
                                    resource = unit_of_work.memoized_state.get();
                                }
                                // Before unwinding the stack, check one more time if the
                                // instance is ready. It may have loaded when React yielded to
                                // the main thread.

                                // Assigning this to a constant so Flow knows the binding won't
                                // be mutated by `preloadInstance`.
                                let host_fiber = &unit_of_work;
                                let r#type = host_fiber.r#type.get();
                                let props = host_fiber.pending_props.get();
                                let is_ready = if let Some(resource) = resource {
                                    preload_resource(resource)
                                } else {
                                    preload_instance(host_fiber.state_node.get(), r#type, props)
                                };
                                if is_ready {
                                    // The data resolved. Resume the work loop as if nothing
                                    // suspended. Unlike when a user component suspends, we don't
                                    // have to replay anything because the host fiber
                                    // already completed.
                                    WORK_IN_PROGRESS_SUSPENDED_REASON.set(NOT_SUSPENDED);
                                    WORK_IN_PROGRESS_THROWN_VALUE.set(js::null());
                                    let sibling = host_fiber.sibling.borrow().clone();
                                    if let Some(sibling) = sibling {
                                        WORK_IN_PROGRESS.set(Some(sibling));
                                    } else {
                                        let return_fiber = host_fiber.r#return.borrow().clone();
                                        if let Some(return_fiber) = return_fiber {
                                            WORK_IN_PROGRESS.set(Some(Rc::clone(&return_fiber)));
                                            complete_unit_of_work(&return_fiber);
                                        } else {
                                            WORK_IN_PROGRESS.set(js::null());
                                        }
                                    }
                                    break 'resume_or_unwind;
                                }
                            }
                            _ => {
                                // This will fail gracefully but it's not correct, so log a
                                // warning in dev.
                                if __DEV__ {
                                    console::error((
                                        "Unexpected type of fiber triggered a suspensey commit. \
                                         This is a bug in React.",
                                    ));
                                }
                            }
                        }
                        // Otherwise, unwind then continue with the normal work loop.
                        WORK_IN_PROGRESS_SUSPENDED_REASON.set(NOT_SUSPENDED);
                        WORK_IN_PROGRESS_THROWN_VALUE.set(js::null());
                        throw_and_unwind_work_loop(
                            root,
                            &unit_of_work,
                            thrown_value,
                            SUSPENDED_ON_INSTANCE_AND_READY_TO_CONTINUE,
                        );
                    }
                    SUSPENDED_ON_DEPRECATED_THROW_PROMISE => {
                        // Suspended by an old implementation that uses the `throw promise`
                        // pattern. The newer replaying behavior can cause subtle issues
                        // like infinite ping loops. So we maintain the old behavior and
                        // always unwind.
                        WORK_IN_PROGRESS_SUSPENDED_REASON.set(NOT_SUSPENDED);
                        WORK_IN_PROGRESS_THROWN_VALUE.set(js::null());
                        throw_and_unwind_work_loop(
                            root,
                            &unit_of_work,
                            thrown_value,
                            SUSPENDED_ON_DEPRECATED_THROW_PROMISE,
                        );
                    }
                    SUSPENDED_ON_HYDRATION => {
                        // Selective hydration. An update flowed into a dehydrated tree.
                        // Interrupt the current render so the work loop can switch to the
                        // hydration lane.
                        reset_work_in_progress_stack();
                        WORK_IN_PROGRESS_ROOT_EXIT_STATUS.set(ROOT_SUSPENDED_AT_THE_SHELL);
                        return;
                    }
                    _ => {
                        panic!("Unexpected SuspendedReason. This is a bug in React.");
                    }
                }
            }

            if __DEV__ && REACT_SHARED_INTERNALS.act_queue().is_some() {
                // `act` special case: If we're inside an `act` scope, don't consult
                // `shouldYield`. Always keep working until the render is complete.
                // This is not just an optimization: in a unit test environment, we
                // can't trust the result of `shouldYield`, because the host I/O is
                // likely mocked.
                work_loop_sync();
            } else if ENABLE_THROTTLED_SCHEDULING {
                work_loop_concurrent(includes_non_idle_work(lanes));
            } else {
                work_loop_concurrent_by_scheduler();
            }
        })) {
            Ok(()) => break,
            Err(thrown_value) => {
                handle_throw(
                    root,
                    thrown_value
                        .downcast_ref::<&'static JsError>()
                        .and_then(|error| error.as_unknown()),
                );
            }
        }
    }
    reset_context_dependencies();

    pop_dispatcher(prev_dispatcher);
    pop_async_dispatcher(prev_async_dispatcher);
    EXECUTION_CONTEXT.set(prev_execution_context);

    // Check if the tree has completed.
    if WORK_IN_PROGRESS.with_borrow(Option::is_some) {
        // Still work remaining.
        if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled) {
            mark_render_yielded();
        }
        ROOT_IN_PROGRESS
    } else {
        // Completed the tree.
        if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled) {
            mark_render_stopped();
        }

        // Set this to null to indicate there's no in-progress render.
        WORK_IN_PROGRESS_ROOT.set(js::null());
        WORK_IN_PROGRESS_ROOT_RENDER_LANES.set(NO_LANES);

        // It's safe to process the queue now that the render phase is complete.
        finish_queueing_concurrent_updates();

        // Return the final exit status.
        WORK_IN_PROGRESS_ROOT_EXIT_STATUS.get()
    }
}

/** @noinline */
fn work_loop_concurrent(non_idle: bool) {
    // We yield every other "frame" when rendering Transition or Retries. Those are blocking
    // revealing new content. The purpose of this yield is not to avoid the overhead of yielding,
    // which is very low, but rather to intentionally block any frequently occurring other main
    // thread work like animations from starving our work. In other words, the purpose of this
    // is to reduce the framerate of animations to 30 frames per second.
    // For Idle work we yield every 5ms to keep animations going smooth.
    if let Some(mut work_in_progress) = WORK_IN_PROGRESS.with_borrow(Clone::clone) {
        let yield_after = now() + if non_idle { 25.0 } else { 5.0 };
        loop {
            perform_unit_of_work(&work_in_progress);
            match WORK_IN_PROGRESS.with_borrow(Clone::clone) {
                Some(next) if now() < yield_after => work_in_progress = next,
                _ => break,
            }
        }
    }
}

/** @noinline */
fn work_loop_concurrent_by_scheduler() {
    // Perform work until Scheduler asks us to yield
    while let Some(work_in_progress) = WORK_IN_PROGRESS.with_borrow(Clone::clone)
        && !SHOULD_YIELD()
    {
        perform_unit_of_work(&work_in_progress);
    }
}

fn perform_unit_of_work(unit_of_work: &Rc<Fiber>) {
    // The current, flushed, state of this fiber is the alternate. Ideally
    // nothing should rely on this, but relying on it here means that we don't
    // need an additional field on the work in progress.
    let current = unit_of_work.alternate.borrow().clone();

    let next;
    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        && (unit_of_work.mode.get() & PROFILE_MODE) != NO_MODE
    {
        start_profiler_timer(unit_of_work);
        if __DEV__ {
            next = run_with_fiber_in_dev(
                Some(Rc::clone(unit_of_work)),
                |current: Option<&Rc<Fiber>>,
                 unit_of_work: &Rc<Fiber>,
                 lanes: Lanes,
                 _: (),
                 _: ()| { begin_work(current, unit_of_work, lanes) },
                current.as_ref(),
                unit_of_work,
                ENTANGLED_RENDER_LANES.get(),
                (),
                (),
            );
        } else {
            next = begin_work(current.as_ref(), unit_of_work, ENTANGLED_RENDER_LANES.get());
        }
        stop_profiler_timer_if_running_and_record_duration(unit_of_work);
    } else if __DEV__ {
        next = run_with_fiber_in_dev(
            Some(Rc::clone(unit_of_work)),
            |current: Option<&Rc<Fiber>>, unit_of_work: &Rc<Fiber>, lanes: Lanes, _: (), _: ()| {
                begin_work(current, unit_of_work, lanes)
            },
            current.as_ref(),
            unit_of_work,
            ENTANGLED_RENDER_LANES.get(),
            (),
            (),
        );
    } else {
        next = begin_work(current.as_ref(), unit_of_work, ENTANGLED_RENDER_LANES.get());
    }

    unit_of_work
        .memoized_props
        .set(unit_of_work.pending_props.get());
    if next.is_none() {
        // If this doesn't spawn new work, complete the current work.
        complete_unit_of_work(unit_of_work);
    } else {
        WORK_IN_PROGRESS.set(next);
    }
}

fn replay_suspended_unit_of_work(unit_of_work: &Rc<Fiber>) {
    // This is a fork of performUnitOfWork specifcally for replaying a fiber that
    // just suspended.
    let next = if __DEV__ {
        run_with_fiber_in_dev(
            Some(Rc::clone(unit_of_work)),
            |unit_of_work: &Rc<Fiber>, _: (), _: (), _: (), _: ()| replay_begin_work(unit_of_work),
            unit_of_work,
            (),
            (),
            (),
            (),
        )
    } else {
        replay_begin_work(unit_of_work)
    };

    unit_of_work
        .memoized_props
        .set(unit_of_work.pending_props.get());
    if next.is_none() {
        // If this doesn't spawn new work, complete the current work.
        complete_unit_of_work(unit_of_work);
    } else {
        WORK_IN_PROGRESS.set(next);
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn replay_begin_work(unit_of_work: &Rc<Fiber>) -> Option<Rc<Fiber>> {
    // This is a fork of beginWork specifcally for replaying a fiber that
    // just suspended.

    let current = unit_of_work.alternate.borrow().clone();

    let mut unit_of_work = Rc::clone(unit_of_work);
    let is_profiling_mode = ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        && (unit_of_work.mode.get() & PROFILE_MODE) != NO_MODE;
    if is_profiling_mode {
        start_profiler_timer(&unit_of_work);
    }
    let next = match unit_of_work.tag.get() {
        SIMPLE_MEMO_COMPONENT | FUNCTION_COMPONENT => {
            // Resolve `defaultProps`. This logic is copied from `beginWork`.
            // TODO: Consider moving this switch statement into that module. Also,
            // could maybe use this as an opportunity to say `use` doesn't work with
            // `defaultProps` :)
            let component = unit_of_work.r#type.get();
            let mut context: Option<&'static Unknown> = None;
            if !DISABLE_LEGACY_CONTEXT {
                let unmasked_context =
                    get_unmasked_context(&unit_of_work, unsafe { js::cast(component) }, true);
                context = Some(js::unknown(get_masked_context(
                    &unit_of_work,
                    unmasked_context,
                )));
            }
            replay_function_component(
                current.as_ref(),
                &unit_of_work,
                unit_of_work.pending_props.get(),
                component,
                context,
                WORK_IN_PROGRESS_ROOT_RENDER_LANES.get(),
            )
        }
        FORWARD_REF => {
            // Resolve `defaultProps`. This logic is copied from `beginWork`.
            // TODO: Consider moving this switch statement into that module. Also,
            // could maybe use this as an opportunity to say `use` doesn't work with
            // `defaultProps` :)
            let component = js::get(unit_of_work.r#type.get(), "render");
            replay_function_component(
                current.as_ref(),
                &unit_of_work,
                unit_of_work.pending_props.get(),
                component,
                unit_of_work.r#ref.get().map(js::unknown_of),
                WORK_IN_PROGRESS_ROOT_RENDER_LANES.get(),
            )
        }
        tag => {
            if tag == HOST_COMPONENT {
                // Some host components are stateful (that's how we implement form
                // actions) but we don't bother to reuse the memoized state because it's
                // not worth the extra code. The main reason to reuse the previous hooks
                // is to reuse uncached promises, but we happen to know that the only
                // promises that a host component might suspend on are definitely cached
                // because they are controlled by us. So don't bother.
                reset_hooks_on_unwind(&unit_of_work);
                // We are about to retry this host component and need to ensure the hydration
                // state is appropriate for hydrating this unit. Other fiber types hydrate differently
                // and aren't reliant on the cursor positioning so this function is only for HostComponent
                pop_hydration_state_on_interrupted_work(&unit_of_work);
                // Fallthrough to the next branch.
            }
            // Other types besides function components are reset completely before
            // being replayed. Currently this only happens when a Usable type is
            // reconciled — the reconciler will suspend.
            //
            // We reset the fiber back to its original state; however, this isn't
            // a full "unwind" because we're going to reuse the promises that were
            // reconciled previously. So it's intentional that we don't call
            // resetSuspendedWorkLoopOnUnwind here.
            unwind_interrupted_work(
                current.as_deref(),
                &unit_of_work,
                WORK_IN_PROGRESS_ROOT_RENDER_LANES.get(),
            );
            unit_of_work = reset_work_in_progress(&unit_of_work, ENTANGLED_RENDER_LANES.get());
            WORK_IN_PROGRESS.set(Some(Rc::clone(&unit_of_work)));
            begin_work(
                current.as_ref(),
                &unit_of_work,
                ENTANGLED_RENDER_LANES.get(),
            )
        }
    };
    if is_profiling_mode {
        stop_profiler_timer_if_running_and_record_duration(&unit_of_work);
    }

    next
}

fn throw_and_unwind_work_loop(
    root: &Rc<FiberRoot>,
    unit_of_work: &Rc<Fiber>,
    thrown_value: Option<&'static Unknown>,
    suspended_reason: SuspendedReason,
) {
    // This is a fork of performUnitOfWork specifcally for unwinding a fiber
    // that threw an exception.
    //
    // Return to the normal work loop. This will unwind the stack, and potentially
    // result in showing a fallback.
    reset_suspended_work_loop_on_unwind(unit_of_work);

    let return_fiber = unit_of_work.r#return.borrow().clone();
    let did_fatal = match catch_unwind(AssertUnwindSafe(|| {
        // Find and mark the nearest Suspense or error boundary that can handle
        // this "exception".
        let did_fatal = throw_exception(
            root,
            return_fiber.as_ref(),
            unit_of_work,
            thrown_value,
            WORK_IN_PROGRESS_ROOT_RENDER_LANES.get(),
        );
        if did_fatal {
            panic_on_root_error(root, thrown_value);
        }
        did_fatal
    })) {
        Ok(did_fatal) => did_fatal,
        Err(error) => {
            // We had trouble processing the error. An example of this happening is
            // when accessing the `componentDidCatch` property of an error boundary
            // throws an error. A weird edge case. There's a regression test for this.
            // To prevent an infinite loop, bubble the error up to the next parent.
            if let Some(return_fiber) = return_fiber {
                WORK_IN_PROGRESS.set(Some(return_fiber));
                resume_unwind(error);
            } else {
                panic_on_root_error(root, thrown_value);
                return;
            }
        }
    };
    if did_fatal {
        return;
    }

    if (unit_of_work.flags.get() & INCOMPLETE) != 0 {
        // Unwind the stack until we reach the nearest boundary.
        let skip_siblings;

        if
        // The current algorithm for both hydration and error handling assumes
        // that the tree is rendered sequentially. So we always skip the siblings.
        get_is_hydrating() || suspended_reason == SUSPENDED_ON_ERROR {
            skip_siblings = true;
            // We intentionally don't set workInProgressRootDidSkipSuspendedSiblings,
            // because we don't want to trigger another prerender attempt.
        } else if
        // Check whether this is a prerender
        !WORK_IN_PROGRESS_ROOT_IS_PRERENDERING.get()
            // Offscreen rendering is also a form of speculative rendering
            && !includes_some_lane(WORK_IN_PROGRESS_ROOT_RENDER_LANES.get(), OFFSCREEN_LANE)
        {
            // This is not a prerender. Skip the siblings during this render. A
            // separate prerender will be scheduled for later.
            skip_siblings = true;
            WORK_IN_PROGRESS_ROOT_DID_SKIP_SUSPENDED_SIBLINGS.set(true);

            // Because we're skipping the siblings, schedule an immediate retry of
            // this boundary.
            //
            // The reason we do this is because a prerender is only scheduled when
            // the root is blocked from committing, i.e. RootSuspendedWithDelay.
            // When the root is not blocked, as in the case when we render a
            // fallback, the original lane is considered to be finished, and
            // therefore no longer in need of being prerendered. However, there's
            // still a pending retry that will happen once the data streams in.
            // We should start rendering that even before the data streams in so we
            // can prerender the siblings.
            if suspended_reason == SUSPENDED_ON_DATA
                || suspended_reason == SUSPENDED_ON_ACTION
                || suspended_reason == SUSPENDED_ON_IMMEDIATE
                || suspended_reason == SUSPENDED_ON_DEPRECATED_THROW_PROMISE
            {
                let boundary = get_suspense_handler();
                if let Some(boundary) = boundary
                    && boundary.tag.get() == SUSPENSE_COMPONENT
                {
                    boundary.flags.set(boundary.flags.get() | SCHEDULE_RETRY);
                }
            }
        } else {
            // This is a prerender. Don't skip the siblings.
            skip_siblings = false;
        }

        unwind_unit_of_work(unit_of_work, skip_siblings);
    } else {
        // Although the fiber suspended, we're intentionally going to commit it in
        // an inconsistent state. We can do this safely in cases where we know the
        // inconsistent tree will be hidden.
        //
        // This currently only applies to Legacy Suspense implementation, but we may
        // port a version of this to concurrent roots, too, when performing a
        // synchronous render. Because that will allow us to mutate the tree as we
        // go instead of buffering mutations until the end. Though it's unclear if
        // this particular path is how that would be implemented.
        complete_unit_of_work(unit_of_work);
    }
}

pub fn mark_spawned_retry_lane(lane: Lane) {
    // Keep track of the retry lanes that were spawned by a fallback during the
    // current render and were not later pinged. This will represent the lanes
    // that are known to still be suspended.
    WORK_IN_PROGRESS_SUSPENDED_RETRY_LANES.set(merge_lanes(
        WORK_IN_PROGRESS_SUSPENDED_RETRY_LANES.get(),
        lane,
    ));
}

fn panic_on_root_error(root: &FiberRoot, error: Option<&'static Unknown>) {
    // There's no ancestor that can handle this exception. This should never
    // happen because the root is supposed to capture all errors that weren't
    // caught by an error boundary. This is a fatal error, or panic condition,
    // because we've run out of ways to recover.
    WORK_IN_PROGRESS_ROOT_EXIT_STATUS.set(ROOT_FATAL_ERRORED);
    let current = root.current.borrow().clone();
    log_uncaught_error(root, &create_captured_value_at_fiber(error, &current));
    // Set `workInProgress` to null. This represents advancing to the next
    // sibling, or the parent if there are no siblings. But since the root
    // has no siblings nor a parent, we set it to null. Usually this is
    // handled by `completeUnitOfWork` or `unwindWork`, but since we're
    // intentionally not calling those, we need set it here.
    // TODO: Consider calling `unwindWork` to pop the contexts.
    WORK_IN_PROGRESS.set(js::null());
}

fn complete_unit_of_work(unit_of_work: &Rc<Fiber>) {
    // Attempt to complete the current unit of work, then move to the next
    // sibling. If there are no more siblings, return to the parent fiber.
    let mut completed_work: Rc<Fiber> = Rc::clone(unit_of_work);
    loop {
        if (completed_work.flags.get() & INCOMPLETE) != NO_FLAGS {
            // This fiber did not complete, because one of its children did not
            // complete. Switch to unwinding the stack instead of completing it.
            //
            // The reason "unwind" and "complete" is interleaved is because when
            // something suspends, we continue rendering the siblings even though
            // they will be replaced by a fallback.
            let skip_siblings = WORK_IN_PROGRESS_ROOT_DID_SKIP_SUSPENDED_SIBLINGS.get();
            unwind_unit_of_work(&completed_work, skip_siblings);
            return;
        }

        // The current, flushed, state of this fiber is the alternate. Ideally
        // nothing should rely on this, but relying on it here means that we don't
        // need an additional field on the work in progress.
        let current = completed_work.alternate.borrow().clone();
        let return_fiber = completed_work.r#return.borrow().clone();

        start_profiler_timer(&completed_work);
        let next = if __DEV__ {
            run_with_fiber_in_dev(
                Some(Rc::clone(&completed_work)),
                |current: Option<&Fiber>,
                 completed_work: &Rc<Fiber>,
                 lanes: Lanes,
                 _: (),
                 _: ()| { complete_work(current, completed_work, lanes) },
                current.as_deref(),
                &completed_work,
                ENTANGLED_RENDER_LANES.get(),
                (),
                (),
            )
        } else {
            complete_work(
                current.as_deref(),
                &completed_work,
                ENTANGLED_RENDER_LANES.get(),
            )
        };
        if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
            && (completed_work.mode.get() & PROFILE_MODE) != NO_MODE
        {
            // Update render duration assuming we didn't error.
            stop_profiler_timer_if_running_and_record_incomplete_duration(&completed_work);
        }
        if next.is_some() {
            // Completing this fiber spawned new work. Work on that next.
            WORK_IN_PROGRESS.set(next);
            return;
        }

        let sibling_fiber = completed_work.sibling.borrow().clone();
        if sibling_fiber.is_some() {
            // If there is more work to do in this returnFiber, do that next.
            WORK_IN_PROGRESS.set(sibling_fiber);
            return;
        }
        // Otherwise, return to the parent
        // Update the next thing we're working on in case something throws.
        WORK_IN_PROGRESS.set(return_fiber.clone());
        match return_fiber {
            Some(return_fiber) => completed_work = return_fiber,
            None => break,
        }
    }

    // We've reached the root.
    if WORK_IN_PROGRESS_ROOT_EXIT_STATUS.get() == ROOT_IN_PROGRESS {
        WORK_IN_PROGRESS_ROOT_EXIT_STATUS.set(ROOT_COMPLETED);
    }
}

fn unwind_unit_of_work(unit_of_work: &Rc<Fiber>, skip_siblings: bool) {
    let mut incomplete_work: Rc<Fiber> = Rc::clone(unit_of_work);
    loop {
        // The current, flushed, state of this fiber is the alternate. Ideally
        // nothing should rely on this, but relying on it here means that we don't
        // need an additional field on the work in progress.
        let current = incomplete_work.alternate.borrow().clone();

        // This fiber did not complete because something threw. Pop values off
        // the stack without entering the complete phase. If this is a boundary,
        // capture values if possible.
        let next = unwind_work(
            current.as_deref(),
            &incomplete_work,
            ENTANGLED_RENDER_LANES.get(),
        );

        // Because this fiber did not complete, don't reset its lanes.

        if let Some(next) = next {
            // Found a boundary that can handle this exception. Re-renter the
            // begin phase. This branch will return us to the normal work loop.
            //
            // Since we're restarting, remove anything that is not a host effect
            // from the effect tag.
            next.flags.set(next.flags.get() & HOST_EFFECT_MASK);
            WORK_IN_PROGRESS.set(Some(next));
            return;
        }

        // Keep unwinding until we reach either a boundary or the root.

        if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
            && (incomplete_work.mode.get() & PROFILE_MODE) != NO_MODE
        {
            // Record the render duration for the fiber that errored.
            stop_profiler_timer_if_running_and_record_incomplete_duration(&incomplete_work);

            // Include the time spent working on failed children before continuing.
            let mut actual_duration = incomplete_work.actual_duration.get().unwrap();
            let mut child = incomplete_work.child.borrow().clone();
            while let Some(node) = child {
                actual_duration += node.actual_duration.get().unwrap();
                child = node.sibling.borrow().clone();
            }
            incomplete_work.actual_duration.set(Some(actual_duration));
        }

        // TODO: Once we stop prerendering siblings, instead of resetting the parent
        // of the node being unwound, we should be able to reset node itself as we
        // unwind the stack. Saves an additional null check.
        let return_fiber = incomplete_work.r#return.borrow().clone();
        if let Some(return_fiber) = &return_fiber {
            // Mark the parent fiber as incomplete and clear its subtree flags.
            // TODO: Once we stop prerendering siblings, we may be able to get rid of
            // the Incomplete flag because unwinding to the nearest boundary will
            // happen synchronously.
            return_fiber
                .flags
                .set(return_fiber.flags.get() | INCOMPLETE);
            return_fiber.subtree_flags.set(NO_FLAGS);
            return_fiber.deletions.replace(js::null());
        }

        if !skip_siblings {
            let sibling_fiber = incomplete_work.sibling.borrow().clone();
            if sibling_fiber.is_some() {
                // This branch will return us to the normal work loop.
                WORK_IN_PROGRESS.set(sibling_fiber);
                return;
            }
        }

        // Otherwise, return to the parent
        // Update the next thing we're working on in case something throws.
        WORK_IN_PROGRESS.set(return_fiber.clone());
        match return_fiber {
            Some(return_fiber) => incomplete_work = return_fiber,
            None => break,
        }
    }

    // We've unwound all the way to the root.
    WORK_IN_PROGRESS_ROOT_EXIT_STATUS.set(ROOT_SUSPENDED_AT_THE_SHELL);
    WORK_IN_PROGRESS.set(js::null());
}

#[cfg_attr(
    rust_js,
    rust_js::nullable(
        finished_work,
        recoverable_errors,
        transitions,
        suspended_state,
        suspended_commit_reason
    )
)]
fn complete_root(
    root: &Rc<FiberRoot>,
    finished_work: Option<&Rc<Fiber>>,
    lanes: Lanes,
    recoverable_errors: Option<&CaptureErrors>,
    transitions: Option<&Rc<Vec<&'static Transition>>>,
    did_include_render_phase_update: bool,
    spawned_lane: Lane,
    updated_lanes: Lanes,
    suspended_retry_lanes: Lanes,
    did_skip_suspended_siblings: bool,
    exit_status: RootExitStatus,
    suspended_state: Option<&'static Unknown>,
    suspended_commit_reason: SuspendedCommitReason, // Profiling-only
    mut completed_render_start_time: f64,           // Profiling-only
    mut completed_render_end_time: f64,             // Profiling-only
) {
    root.cancel_pending_commit.replace(js::null());

    loop {
        // `flushPassiveEffects` will call `flushSyncUpdateQueue` at the end, which
        // means `flushPassiveEffects` will sometimes result in additional
        // passive effects. So we need to keep flushing in a loop until there are
        // no more pending effects.
        // TODO: Might be better if `flushPassiveEffects` did not automatically
        // flush synchronous work at the end, to avoid factoring hazards like this.
        flush_pending_effects();
        if PENDING_EFFECTS_STATUS.get() == NO_PENDING_EFFECTS {
            break;
        }
    }
    flush_render_phase_strict_mode_warnings_in_dev();

    if (EXECUTION_CONTEXT.get() & (RENDER_CONTEXT | COMMIT_CONTEXT)) != NO_CONTEXT {
        panic!("Should not already be working.");
    }

    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK {
        // Log the previous render phase once we commit. I.e. we weren't interrupted.
        set_current_track_from_lanes(lanes);
        if is_gesture_render(lanes) {
            // Clamp the render start time in case if something else on this lane was committed
            // (such as this same tree before).
            if completed_render_start_time < GESTURE_CLAMP_TIME.get() {
                completed_render_start_time = GESTURE_CLAMP_TIME.get();
            }
            if completed_render_end_time < GESTURE_CLAMP_TIME.get() {
                completed_render_end_time = GESTURE_CLAMP_TIME.get();
            }
        }
        if exit_status == ROOT_ERRORED {
            log_errored_render_phase(
                completed_render_start_time,
                completed_render_end_time,
                lanes,
                WORK_IN_PROGRESS_UPDATE_TASK.get(),
            );
        } else if let Some(recoverable_errors) = recoverable_errors {
            let hydration_failed = finished_work.is_some_and(|finished_work| {
                finished_work
                    .alternate
                    .borrow()
                    .as_ref()
                    .is_some_and(|alternate| {
                        let root_state: &RootState =
                            unsafe { js::cast(alternate.memoized_state.get()) };
                        root_state.is_dehydrated
                    })
                    && (finished_work.flags.get() & FORCE_CLIENT_RENDER) != NO_FLAGS
            });
            log_recovered_render_phase(
                completed_render_start_time,
                completed_render_end_time,
                lanes,
                recoverable_errors,
                hydration_failed,
                WORK_IN_PROGRESS_UPDATE_TASK.get(),
            );
        } else {
            log_render_phase(
                completed_render_start_time,
                completed_render_end_time,
                lanes,
                WORK_IN_PROGRESS_UPDATE_TASK.get(),
            );
        }
    }

    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled) {
        mark_commit_started(lanes);
    }

    let Some(finished_work) = finished_work else {
        if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled) {
            mark_commit_stopped();
        }
        // Stop any gestures that were committed.
        if ENABLE_GESTURE_TRANSITION.with(|&enabled| enabled) && is_gesture_render(lanes) {
            stop_committed_gesture(root);
        }
        return;
    };
    if __DEV__ && lanes == NO_LANES {
        console::error((
            "finishedLanes should not be empty during a commit. This is a bug in React.",
        ));
    }

    if Rc::ptr_eq(finished_work, &root.current.borrow()) {
        panic!(
            "Cannot commit the same tree as before. This error is likely caused by \
             a bug in React. Please file an issue."
        );
    }

    if WORK_IN_PROGRESS_ROOT.with_borrow(|work_in_progress_root| {
        work_in_progress_root
            .as_ref()
            .is_some_and(|work_in_progress_root| Rc::ptr_eq(root, work_in_progress_root))
    }) {
        // We can reset these now that they are finished.
        WORK_IN_PROGRESS_ROOT.set(js::null());
        WORK_IN_PROGRESS.set(js::null());
        WORK_IN_PROGRESS_ROOT_RENDER_LANES.set(NO_LANES);
    } else {
        // This indicates that the last root we worked on is not the same one that
        // we're committing now. This most commonly happens when a suspended root
        // times out.
    }

    // workInProgressX might be overwritten, so we want
    // to store it in pendingPassiveX until they get processed
    // We need to pass this through as an argument to completeRoot
    // because workInProgressX might have changed between
    // the previous render and commit if we throttle the commit
    // with setTimeout
    PENDING_FINISHED_WORK.set(Some(Rc::clone(finished_work)));
    PENDING_EFFECTS_ROOT.set(Some(Rc::clone(root)));
    PENDING_EFFECTS_LANES.set(lanes);
    PENDING_PASSIVE_TRANSITIONS.set(transitions.cloned());
    PENDING_RECOVERABLE_ERRORS.set(recoverable_errors.cloned());
    PENDING_DID_INCLUDE_RENDER_PHASE_UPDATE.set(did_include_render_phase_update);
    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
        PENDING_EFFECTS_RENDER_END_TIME.set(completed_render_end_time);
        PENDING_SUSPENDED_COMMIT_REASON.set(suspended_commit_reason);
        PENDING_DELAYED_COMMIT_REASON.set(IMMEDIATE_COMMIT);
        PENDING_SUSPENDED_VIEW_TRANSITION_REASON.set(js::null());
    }

    if ENABLE_GESTURE_TRANSITION.with(|&enabled| enabled) && is_gesture_render(lanes) {
        let committing_gesture = root.pending_gestures.borrow().clone();
        if let Some(committing_gesture) = committing_gesture
            && !committing_gesture.committing.get()
        {
            // This gesture is not ready to commit yet. We'll mark it as suspended and
            // start a gesture transition which isn't really a side-effect. Then later
            // we might come back around to actually committing the root.
            let did_attempt_entire_tree = !did_skip_suspended_siblings;
            mark_root_suspended(root, lanes, spawned_lane, did_attempt_entire_tree);
            if committing_gesture.running.get().is_none() {
                apply_gesture_on_root(
                    root,
                    finished_work,
                    recoverable_errors,
                    suspended_state,
                    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
                        if suspended_commit_reason.is_none() {
                            completed_render_end_time
                        } else {
                            COMMIT_START_TIME.get()
                        }
                    } else {
                        0.0
                    },
                );
            } else {
                // If we already have a gesture running, we don't update it in place
                // even if we have a new tree. Instead we wait until we can commit.
                if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                    && ENABLE_COMPONENT_PERFORMANCE_TRACK
                {
                    // Clamp at the render time since we're not going to finish the rest
                    // of this commit or apply yet.
                    finalize_render(lanes, completed_render_end_time);
                }
                // We are no longer committing.
                PENDING_EFFECTS_ROOT.set(js::null()); // Clear for GC purposes.
                PENDING_FINISHED_WORK.set(js::null()); // Clear for GC purposes.
                PENDING_EFFECTS_LANES.set(NO_LANES);
            }
            // Schedule the root to be committed when the gesture completes.
            root.cancel_pending_commit
                .replace(Some(schedule_gesture_commit(
                    &committing_gesture,
                    function::bind(
                        complete_root,
                        Some(&Kind::Null),
                        (
                            Rc::clone(root),
                            Some(Rc::clone(finished_work)),
                            lanes,
                            recoverable_errors.cloned(),
                            transitions.cloned(),
                            did_include_render_phase_update,
                            spawned_lane,
                            updated_lanes,
                            suspended_retry_lanes,
                            did_skip_suspended_siblings,
                            exit_status,
                            suspended_state,
                            Some("Waiting for the Gesture to finish"), /* suspendedCommitReason */
                            completed_render_start_time,
                            completed_render_end_time,
                        ),
                    ),
                )));
            return;
        }
    }

    // If we're not starting a gesture we now actually commit the root.
    commit_root(
        root,
        finished_work,
        lanes,
        spawned_lane,
        updated_lanes,
        suspended_retry_lanes,
        suspended_state,
        suspended_commit_reason,
        completed_render_end_time,
    );
}

/// Leaves a commit phase run at discrete priority: puts back the execution context, the
/// update priority and the Transition.
struct ExitCommitPhase {
    prev_execution_context: ExecutionContext,
    previous_priority: EventPriority,
    prev_transition: Option<&'static Transition>,
}

impl Drop for ExitCommitPhase {
    fn drop(&mut self) {
        // Reset the priority to the previous non-sync value.
        EXECUTION_CONTEXT.set(self.prev_execution_context);
        set_current_update_priority(self.previous_priority);
        set_t(REACT_SHARED_INTERNALS, self.prev_transition);
    }
}

#[cfg_attr(rust_js, rust_js::nullable(suspended_state, suspended_commit_reason))]
fn commit_root(
    root: &Rc<FiberRoot>,
    finished_work: &Rc<Fiber>,
    lanes: Lanes,
    spawned_lane: Lane,
    updated_lanes: Lanes,
    suspended_retry_lanes: Lanes,
    suspended_state: Option<&'static Unknown>,
    suspended_commit_reason: SuspendedCommitReason, // Profiling-only
    completed_render_end_time: f64,                 // Profiling-only
) {
    // Check which lanes no longer have any work scheduled on them, and mark
    // those as finished.
    let mut remaining_lanes =
        merge_lanes(finished_work.lanes.get(), finished_work.child_lanes.get());

    PENDING_EFFECTS_REMAINING_LANES.set(remaining_lanes);

    // Make sure to account for lanes that were updated by a concurrent event
    // during the render phase; don't mark them as finished.
    let concurrently_updated_lanes = get_concurrently_updated_lanes();
    remaining_lanes = merge_lanes(remaining_lanes, concurrently_updated_lanes);

    if ENABLE_GESTURE_TRANSITION.with(|&enabled| enabled)
        && root.pending_gestures.borrow().is_none()
    {
        // Gestures don't clear their lanes while the gesture is still active but it
        // might not be scheduled to do any more renders and so we shouldn't schedule
        // any more gesture lane work until a new gesture is scheduled.
        if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
            && (remaining_lanes & GESTURE_LANE) != NO_LANES
        {
            // We need to clear any updates scheduled so that we can treat future updates
            // as the cause of the render.
            clear_gesture_updates();
        }
        remaining_lanes &= !GESTURE_LANE;
    }

    mark_root_finished(
        root,
        lanes,
        remaining_lanes,
        spawned_lane,
        updated_lanes,
        suspended_retry_lanes,
    );

    // Reset this before firing side effects so we can detect recursive updates.
    DID_INCLUDE_COMMIT_PHASE_UPDATE.set(false);

    // If there are pending passive effects, schedule a callback to process them.
    // Do this as early as possible, so it is queued before anything else that
    // might get scheduled in the commit phase. (See #16714.)
    // TODO: Delete all other places that schedule the passive effect callback
    // They're redundant.
    let passive_subtree_mask;
    if ENABLE_VIEW_TRANSITION {
        PENDING_VIEW_TRANSITION_EVENTS.set(js::null());
        if includes_only_view_transition_eligible_lanes(lanes) {
            // Claim any pending Transition Types for this commit.
            PENDING_TRANSITION_TYPES.set(claim_queued_transition_types(root));
            passive_subtree_mask = PASSIVE_TRANSITION_MASK;
        } else {
            PENDING_TRANSITION_TYPES.set(js::null());
            passive_subtree_mask = PASSIVE_MASK;
        }
    } else {
        passive_subtree_mask = PASSIVE_MASK;
    }
    if
    // If this subtree rendered with profiling this commit, we need to visit it to log it.
    (ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        && ENABLE_COMPONENT_PERFORMANCE_TRACK
        && finished_work.actual_duration.get() != Some(0.0))
        || (finished_work.subtree_flags.get() & passive_subtree_mask) != NO_FLAGS
        || (finished_work.flags.get() & passive_subtree_mask) != NO_FLAGS
    {
        if ENABLE_YIELDING_BEFORE_PASSIVE {
            // We don't schedule a separate task for flushing passive effects.
            // Instead, we just rely on ensureRootIsScheduled below to schedule
            // a callback for us to flush the passive effects.
        } else {
            // So we can clear these now to allow a new callback to be scheduled.
            root.callback_node.set(js::null());
            root.callback_priority.set(NO_LANE);
            schedule_callback(
                NORMAL_SCHEDULER_PRIORITY.with(|&priority| priority),
                SchedulerCallback(Box::new(|_| {
                    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                        && ENABLE_COMPONENT_PERFORMANCE_TRACK
                    {
                        // Track the currently executing event if there is one so we can ignore this
                        // event when logging events.
                        track_scheduler_event();
                    }
                    if PENDING_DELAYED_COMMIT_REASON.get() == IMMEDIATE_COMMIT {
                        PENDING_DELAYED_COMMIT_REASON.set(DELAYED_PASSIVE_COMMIT);
                    }
                    flush_passive_effects();
                    // This render triggered passive effects: release the root cache pool
                    // *after* passive effects fire to avoid freeing a cache pool that may
                    // be referenced by a node in the tree (HostRoot, Cache boundary etc)
                    js::null()
                })),
            );
        }
    } else {
        // If we don't have passive effects, we're not going to need to perform more work
        // so we can clear the callback now.
        root.callback_node.set(js::null());
        root.callback_priority.set(NO_LANE);
    }

    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
        // Mark the current commit time to be shared by all Profilers in this
        // batch. This enables them to be grouped later.
        reset_commit_errors();
        record_commit_time();
        if ENABLE_COMPONENT_PERFORMANCE_TRACK
            && let Some(suspended_commit_reason) = suspended_commit_reason
        {
            log_suspended_commit_phase(
                completed_render_end_time,
                COMMIT_START_TIME.get(),
                suspended_commit_reason,
                WORK_IN_PROGRESS_UPDATE_TASK.get(),
            );
        }
    }

    reset_should_start_view_transition();

    // The commit phase is broken into several sub-phases. We do a separate pass
    // of the effect list for each phase: all mutation effects come before all
    // layout effects, and so on.

    // Check if there are any effects in the whole tree.
    // TODO: This is left over from the effect list implementation, where we had
    // to check for the existence of `firstEffect` to satisfy Flow. I think the
    // only other reason this optimization exists is because it affects profiling.
    // Reconsider whether this is necessary.
    let subtree_has_before_mutation_effects =
        (finished_work.subtree_flags.get() & (BEFORE_MUTATION_MASK | MUTATION_MASK)) != NO_FLAGS;
    let root_has_before_mutation_effect =
        (finished_work.flags.get() & (BEFORE_MUTATION_MASK | MUTATION_MASK)) != NO_FLAGS;

    if subtree_has_before_mutation_effects || root_has_before_mutation_effect {
        let prev_transition = t(REACT_SHARED_INTERNALS);
        set_t(REACT_SHARED_INTERNALS, js::null());
        let previous_priority = get_current_update_priority();
        set_current_update_priority(DISCRETE_EVENT_PRIORITY);
        let prev_execution_context = EXECUTION_CONTEXT.get();
        EXECUTION_CONTEXT.set(EXECUTION_CONTEXT.get() | COMMIT_CONTEXT);
        let _exit = ExitCommitPhase {
            prev_execution_context,
            previous_priority,
            prev_transition,
        };
        // The first phase a "before mutation" phase. We use this phase to read the
        // state of the host tree right before we mutate it. This is where
        // getSnapshotBeforeUpdate is called.
        commit_before_mutation_effects(root, Rc::clone(finished_work), lanes);
    }

    // Stop any gestures that were committed.
    if ENABLE_GESTURE_TRANSITION.with(|&enabled| enabled) && is_gesture_render(lanes) {
        stop_committed_gesture(root);
        // Note that shouldStartViewTransition should always be false here because
        // committing a gesture never starts a new View Transition itself since it's
        // not a View Transition eligible lane. Only follow up Transition commits can
        // cause animate.
    }

    PENDING_EFFECTS_STATUS.set(PENDING_MUTATION_PHASE);
    if ENABLE_VIEW_TRANSITION && SHOULD_START_VIEW_TRANSITION.get() {
        if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK {
            start_animating(lanes);
        }
        PENDING_VIEW_TRANSITION.set(start_view_transition(
            suspended_state,
            root.container_info,
            PENDING_TRANSITION_TYPES.with_borrow(Clone::clone),
            flush_mutation_effects,
            flush_layout_effects,
            flush_after_mutation_effects,
            flush_spawned_work,
            flush_passive_effects,
            report_view_transition_error,
            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
                Some(suspended_view_transition)
            } else {
                js::null()
            },
            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
                // This callback fires after "pendingEffects" so we need to snapshot the arguments.
                Some(function::bind(
                    finished_view_transition,
                    Some(&Kind::Null),
                    (lanes,),
                ))
            } else {
                js::null()
            },
        ));
    } else {
        // Flush synchronously.
        flush_mutation_effects();
        flush_layout_effects();
        // Skip flushAfterMutationEffects
        flush_spawned_work();
    }
}

#[cfg_attr(rust_js, rust_js::nullable(error))]
fn report_view_transition_error(error: Option<&'static Unknown>) {
    // Report errors that happens while preparing a View Transition.
    if PENDING_EFFECTS_STATUS.get() == NO_PENDING_EFFECTS {
        return;
    }
    let root = PENDING_EFFECTS_ROOT.with_borrow(Clone::clone).unwrap();
    let on_recoverable_error = root.on_recoverable_error;
    on_recoverable_error(error, &make_error_info(js::null()));
}

fn suspended_view_transition(reason: &'static str) {
    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK {
        // We'll split the commit into two phases, because we're suspended in the middle.
        record_commit_end_time();
        log_commit_phase(
            if PENDING_SUSPENDED_COMMIT_REASON.get().is_none() {
                PENDING_EFFECTS_RENDER_END_TIME.get()
            } else {
                COMMIT_START_TIME.get()
            },
            COMMIT_END_TIME.get(),
            COMMIT_ERRORS.with_borrow(Clone::clone).as_deref(),
            PENDING_DELAYED_COMMIT_REASON.get() == ABORTED_VIEW_TRANSITION_COMMIT,
            WORK_IN_PROGRESS_UPDATE_TASK.get(), // TODO: Use a ViewTransition Task and this is not safe to read in this phase.
        );
        PENDING_SUSPENDED_VIEW_TRANSITION_REASON.set(Some(reason));
        PENDING_SUSPENDED_COMMIT_REASON.set(Some(reason));
    }
}

fn finished_view_transition(lanes: Lanes) {
    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK {
        if (ANIMATING_LANES.get() & lanes) == NO_LANES {
            // Was already stopped by some other action or maybe other root.
            return;
        }
        let task = ANIMATING_TASK.get();
        stop_animating(lanes);
        // If an affected track isn't in the middle of rendering or committing, log from the previous
        // finished render until the end of the animation.
        if is_gesture_render(lanes)
            && !is_gesture_render(WORK_IN_PROGRESS_ROOT_RENDER_LANES.get())
            && !is_gesture_render(PENDING_EFFECTS_LANES.get())
        {
            set_current_track_from_lanes(GESTURE_LANE);
            log_animating_phase(GESTURE_CLAMP_TIME.get(), now(), task);
        }
        if includes_transition_lane(lanes)
            && !includes_transition_lane(WORK_IN_PROGRESS_ROOT_RENDER_LANES.get())
            && !includes_transition_lane(PENDING_EFFECTS_LANES.get())
        {
            set_current_track_from_lanes(SOME_TRANSITION_LANE);
            log_animating_phase(TRANSITION_CLAMP_TIME.get(), now(), task);
        }
        if includes_retry_lane(lanes)
            && !includes_retry_lane(WORK_IN_PROGRESS_ROOT_RENDER_LANES.get())
            && !includes_retry_lane(PENDING_EFFECTS_LANES.get())
        {
            set_current_track_from_lanes(SOME_RETRY_LANE);
            log_animating_phase(RETRY_CLAMP_TIME.get(), now(), task);
        }
        if includes_idle_group_lanes(lanes)
            && !includes_idle_group_lanes(WORK_IN_PROGRESS_ROOT_RENDER_LANES.get())
            && !includes_idle_group_lanes(PENDING_EFFECTS_LANES.get())
        {
            set_current_track_from_lanes(IDLE_LANE);
            log_animating_phase(IDLE_CLAMP_TIME.get(), now(), task);
        }
    }
}

fn flush_after_mutation_effects() {
    if PENDING_EFFECTS_STATUS.get() != PENDING_AFTER_MUTATION_PHASE {
        return;
    }
    PENDING_EFFECTS_STATUS.set(NO_PENDING_EFFECTS);
    let root = PENDING_EFFECTS_ROOT.with_borrow(Clone::clone).unwrap();
    let finished_work = PENDING_FINISHED_WORK.with_borrow(Clone::clone).unwrap();
    let lanes = PENDING_EFFECTS_LANES.get();
    commit_after_mutation_effects(&root, &finished_work, lanes);
    PENDING_EFFECTS_STATUS.set(PENDING_SPAWNED_WORK);
}

fn flush_mutation_effects() {
    if PENDING_EFFECTS_STATUS.get() != PENDING_MUTATION_PHASE {
        return;
    }
    PENDING_EFFECTS_STATUS.set(NO_PENDING_EFFECTS);

    let root = PENDING_EFFECTS_ROOT.with_borrow(Clone::clone).unwrap();
    let finished_work = PENDING_FINISHED_WORK.with_borrow(Clone::clone).unwrap();
    let lanes = PENDING_EFFECTS_LANES.get();
    let subtree_mutation_has_effects =
        (finished_work.subtree_flags.get() & MUTATION_MASK) != NO_FLAGS;
    let root_mutation_has_effect = (finished_work.flags.get() & MUTATION_MASK) != NO_FLAGS;

    if subtree_mutation_has_effects || root_mutation_has_effect {
        let prev_transition = t(REACT_SHARED_INTERNALS);
        set_t(REACT_SHARED_INTERNALS, js::null());
        let previous_priority = get_current_update_priority();
        set_current_update_priority(DISCRETE_EVENT_PRIORITY);
        let prev_execution_context = EXECUTION_CONTEXT.get();
        EXECUTION_CONTEXT.set(EXECUTION_CONTEXT.get() | COMMIT_CONTEXT);
        let _exit = ExitCommitPhase {
            prev_execution_context,
            previous_priority,
            prev_transition,
        };
        // The next phase is the mutation phase, where we mutate the host tree.
        commit_mutation_effects(&root, &finished_work, lanes);

        if ENABLE_CREATE_EVENT_HANDLE_API && SHOULD_FIRE_AFTER_ACTIVE_INSTANCE_BLUR.get() {
            after_active_instance_blur();
        }
        reset_after_commit(root.container_info);
    }

    // The work-in-progress tree is now the current tree. This must come after
    // the mutation phase, so that the previous tree is still current during
    // componentWillUnmount, but before the layout phase, so that the finished
    // work is current during componentDidMount/Update.
    root.current.replace(finished_work);
    PENDING_EFFECTS_STATUS.set(PENDING_LAYOUT_PHASE);
}

fn flush_layout_effects() {
    if PENDING_EFFECTS_STATUS.get() != PENDING_LAYOUT_PHASE {
        return;
    }
    PENDING_EFFECTS_STATUS.set(NO_PENDING_EFFECTS);

    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        && ENABLE_COMPONENT_PERFORMANCE_TRACK
        && let Some(suspended_view_transition_reason) =
            PENDING_SUSPENDED_VIEW_TRANSITION_REASON.get()
    {
        // We suspended in the middle of the commit for the view transition.
        // We'll start a new commit track now.
        record_commit_time();
        log_suspended_view_transition_phase(
            COMMIT_END_TIME.get(),   // The start is the end of the first commit part.
            COMMIT_START_TIME.get(), // The end is the start of the second commit part.
            suspended_view_transition_reason,
            ANIMATING_TASK.get(),
        );
    }

    let root = PENDING_EFFECTS_ROOT.with_borrow(Clone::clone).unwrap();
    let finished_work = PENDING_FINISHED_WORK.with_borrow(Clone::clone).unwrap();
    let lanes = PENDING_EFFECTS_LANES.get();

    if ENABLE_DEFAULT_TRANSITION_INDICATOR.with(|&enabled| enabled)
        && let Some(clean_up_indicator) = root.pending_indicator.get()
        && root.indicator_lanes.get() == NO_LANES
    {
        // We have now committed all Transitions that needed the default indicator
        // so we can now run the clean up function. We do this in the layout phase
        // so it has the same semantics as if you did it with a useLayoutEffect or
        // if it was reset automatically with useOptimistic.
        let prev_transition = t(REACT_SHARED_INTERNALS);
        set_t(REACT_SHARED_INTERNALS, js::null());
        let previous_priority = get_current_update_priority();
        set_current_update_priority(DISCRETE_EVENT_PRIORITY);
        let prev_execution_context = EXECUTION_CONTEXT.get();
        EXECUTION_CONTEXT.set(EXECUTION_CONTEXT.get() | COMMIT_CONTEXT);
        root.pending_indicator.set(js::null());
        let _exit = ExitCommitPhase {
            prev_execution_context,
            previous_priority,
            prev_transition,
        };
        if let Err(x) = catch_unwind(AssertUnwindSafe(clean_up_indicator)) {
            REPORT_GLOBAL_ERROR.with(|report_global_error| {
                report_global_error(
                    x.downcast_ref::<&'static JsError>()
                        .and_then(|x| x.as_unknown()),
                )
            });
        }
    }

    let subtree_has_layout_effects = (finished_work.subtree_flags.get() & LAYOUT_MASK) != NO_FLAGS;
    let root_has_layout_effect = (finished_work.flags.get() & LAYOUT_MASK) != NO_FLAGS;

    if subtree_has_layout_effects || root_has_layout_effect {
        let prev_transition = t(REACT_SHARED_INTERNALS);
        set_t(REACT_SHARED_INTERNALS, js::null());
        let previous_priority = get_current_update_priority();
        set_current_update_priority(DISCRETE_EVENT_PRIORITY);
        let prev_execution_context = EXECUTION_CONTEXT.get();
        EXECUTION_CONTEXT.set(EXECUTION_CONTEXT.get() | COMMIT_CONTEXT);
        let _exit = ExitCommitPhase {
            prev_execution_context,
            previous_priority,
            prev_transition,
        };
        // The next phase is the layout phase, where we call effects that read
        // the host tree after it's been mutated. The idiomatic use case for this is
        // layout, but class component lifecycles also fire here for legacy reasons.
        if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled) {
            mark_layout_effects_started(lanes);
        }
        commit_layout_effects(&finished_work, &root, lanes);
        if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled) {
            mark_layout_effects_stopped();
        }
    }

    let completed_render_end_time = PENDING_EFFECTS_RENDER_END_TIME.get();
    let suspended_commit_reason = PENDING_SUSPENDED_COMMIT_REASON.get();

    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK {
        record_commit_end_time();
        log_commit_phase(
            if suspended_commit_reason.is_none() {
                completed_render_end_time
            } else {
                COMMIT_START_TIME.get()
            },
            COMMIT_END_TIME.get(),
            COMMIT_ERRORS.with_borrow(Clone::clone).as_deref(),
            PENDING_DELAYED_COMMIT_REASON.get() == ABORTED_VIEW_TRANSITION_COMMIT,
            WORK_IN_PROGRESS_UPDATE_TASK.get(),
        );
    }

    PENDING_EFFECTS_STATUS.set(PENDING_AFTER_MUTATION_PHASE);
}

/// Puts back the Transition and the update priority the recoverable errors
/// were reported with.
struct RestoreTransitionAndUpdatePriority {
    prev_transition: Option<&'static Transition>,
    previous_update_lane_priority: EventPriority,
}

impl Drop for RestoreTransitionAndUpdatePriority {
    fn drop(&mut self) {
        set_t(REACT_SHARED_INTERNALS, self.prev_transition);
        set_current_update_priority(self.previous_update_lane_priority);
    }
}

fn flush_spawned_work() {
    if PENDING_EFFECTS_STATUS.get() != PENDING_SPAWNED_WORK
        // If a startViewTransition times out, we might flush this earlier than
        // after mutation phase. In that case, we just skip the after mutation phase.
        && PENDING_EFFECTS_STATUS.get() != PENDING_AFTER_MUTATION_PHASE
    {
        return;
    }
    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK {
        // If we didn't skip the after mutation phase, when is means we started an animation.
        let started_animation = PENDING_EFFECTS_STATUS.get() == PENDING_SPAWNED_WORK;
        if started_animation {
            let start_view_transition_start_time = COMMIT_END_TIME.get();
            // Update the new commitEndTime to when we started the animation.
            record_commit_end_time();
            log_start_view_transition_yield_phase(
                start_view_transition_start_time,
                COMMIT_END_TIME.get(),
                PENDING_DELAYED_COMMIT_REASON.get() == ABORTED_VIEW_TRANSITION_COMMIT,
                ANIMATING_TASK.get(),
            );
            if PENDING_DELAYED_COMMIT_REASON.get() != ABORTED_VIEW_TRANSITION_COMMIT {
                PENDING_DELAYED_COMMIT_REASON.set(ANIMATION_STARTED_COMMIT);
            }
        }
    }

    PENDING_EFFECTS_STATUS.set(NO_PENDING_EFFECTS);

    let committed_view_transition = PENDING_VIEW_TRANSITION.get();
    PENDING_VIEW_TRANSITION.set(js::null()); // The view transition has now fully started.

    // Tell Scheduler to yield at the end of the frame, so the browser has an
    // opportunity to paint.
    REQUEST_PAINT();

    let root = PENDING_EFFECTS_ROOT.with_borrow(Clone::clone).unwrap();
    let finished_work = PENDING_FINISHED_WORK.with_borrow(Clone::clone).unwrap();
    let lanes = PENDING_EFFECTS_LANES.get();
    let recoverable_errors = PENDING_RECOVERABLE_ERRORS.with_borrow(Clone::clone);
    let did_include_render_phase_update = PENDING_DID_INCLUDE_RENDER_PHASE_UPDATE.get();

    let passive_subtree_mask =
        if ENABLE_VIEW_TRANSITION && includes_only_view_transition_eligible_lanes(lanes) {
            PASSIVE_TRANSITION_MASK
        } else {
            PASSIVE_MASK
        };
    let root_did_have_passive_effects = // If this subtree rendered with profiling this commit, we need to visit it to log it.
        (ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
            && ENABLE_COMPONENT_PERFORMANCE_TRACK
            && finished_work.actual_duration.get() != Some(0.0))
            || (finished_work.subtree_flags.get() & passive_subtree_mask) != NO_FLAGS
            || (finished_work.flags.get() & passive_subtree_mask) != NO_FLAGS;

    if root_did_have_passive_effects {
        PENDING_EFFECTS_STATUS.set(PENDING_PASSIVE_PHASE);
    } else {
        PENDING_EFFECTS_STATUS.set(NO_PENDING_EFFECTS);
        PENDING_EFFECTS_ROOT.set(js::null()); // Clear for GC purposes.
        PENDING_FINISHED_WORK.set(js::null()); // Clear for GC purposes.
        // There were no passive effects, so we can immediately release the cache
        // pool for this render.
        release_root_pooled_cache(&root, root.pending_lanes.get());
        if __DEV__ {
            NESTED_PASSIVE_UPDATE_COUNT.set(0);
            ROOT_WITH_PASSIVE_NESTED_UPDATES.set(js::null());
        }
    }

    // Read this again, since an effect might have updated it
    let mut remaining_lanes = root.pending_lanes.get();

    // Check if there's remaining work on this root
    // TODO: This is part of the `componentDidCatch` implementation. Its purpose
    // is to detect whether something might have called setState inside
    // `componentDidCatch`. The mechanism is known to be flawed because `setState`
    // inside `componentDidCatch` is itself flawed — that's why we recommend
    // `getDerivedStateFromError` instead. However, it could be improved by
    // checking if remainingLanes includes Sync work, instead of whether there's
    // any work remaining at all (which would also include stuff like Suspense
    // retries or transitions). It's been like this for a while, though, so fixing
    // it probably isn't that urgent.
    if remaining_lanes == NO_LANES {
        // If there's no remaining work, we can clear the set of already failed
        // error boundaries.
        LEGACY_ERROR_BOUNDARIES_THAT_ALREADY_FAILED.set(js::null());
    }

    if __DEV__ && !root_did_have_passive_effects {
        commit_double_invoke_effects_in_dev(&root, false);
    }

    let render_priority = lanes_to_event_priority(lanes);
    on_commit_root_dev_tools(
        unsafe { js::cast(finished_work.state_node.get()) },
        render_priority,
    );

    if ENABLE_UPDATER_TRACKING.with(|&enabled| enabled)
        && IS_DEV_TOOLS_PRESENT.with(|&present| present)
    {
        root.memoized_updaters.borrow_mut().clear();
    }

    if __DEV__ {
        on_commit_root_test_selector();
    }

    if let Some(recoverable_errors) = recoverable_errors {
        let prev_transition = t(REACT_SHARED_INTERNALS);
        let previous_update_lane_priority = get_current_update_priority();
        set_current_update_priority(DISCRETE_EVENT_PRIORITY);
        set_t(REACT_SHARED_INTERNALS, js::null());
        let _restore = RestoreTransitionAndUpdatePriority {
            prev_transition,
            previous_update_lane_priority,
        };
        // There were errors during this render, but recovered from them without
        // needing to surface it to the UI. We log them here.
        let on_recoverable_error = root.on_recoverable_error;
        let mut i = 0;
        while i < recoverable_errors.borrow().len() {
            let recoverable_error = Rc::clone(&recoverable_errors.borrow()[i]);
            let error_info = make_error_info(recoverable_error.stack.clone());
            if __DEV__ {
                run_with_fiber_in_dev(
                    recoverable_error.source.clone(),
                    |value: Option<&'static Unknown>,
                     error_info: &ErrorInfo,
                     _: (),
                     _: (),
                     _: ()| { on_recoverable_error(value, error_info) },
                    recoverable_error.value,
                    &error_info,
                    (),
                    (),
                    (),
                );
            } else {
                on_recoverable_error(recoverable_error.value, &error_info);
            }
            i += 1;
        }
    }

    if ENABLE_VIEW_TRANSITION {
        // We should now be after the startViewTransition's .ready call which is late enough
        // to start animating any pseudo-elements. We do this before flushing any passive
        // effects or spawned sync work since this is still part of the previous commit.
        // Even though conceptually it's like its own task between layout effets and passive.
        let pending_events = PENDING_VIEW_TRANSITION_EVENTS.take();
        let pending_types = PENDING_TRANSITION_TYPES.take();
        if let Some(pending_events) = pending_events {
            // Normalize the type. This is lazily created only for events.
            let pending_types = pending_types.unwrap_or_default();
            if let Some(committed_view_transition) = committed_view_transition {
                for view_transition_event in &pending_events {
                    let cleanup = view_transition_event(&pending_types);
                    if let Some(cleanup) = cleanup {
                        add_view_transition_finished_listener(committed_view_transition, cleanup);
                    }
                }
            }
        }
    }

    // If the passive effects are the result of a discrete render, flush them
    // synchronously at the end of the current task so that the result is
    // immediately observable. Otherwise, we assume that they are not
    // order-dependent and do not need to be observed by external systems, so we
    // can wait until after paint.
    // TODO: We can optimize this by not scheduling the callback earlier. Since we
    // currently schedule the callback in multiple places, will wait until those
    // are consolidated.
    if includes_sync_lane(PENDING_EFFECTS_LANES.get())
        && (DISABLE_LEGACY_MODE || root.tag != LEGACY_ROOT)
    {
        flush_pending_effects();
    }

    // Always call this before exiting `completeRoot`, to ensure that any
    // additional work on this root is scheduled.
    ensure_root_is_scheduled(&root);

    // Read this again, since a passive effect might have updated it
    remaining_lanes = root.pending_lanes.get();

    // Check if this render scheduled a cascading synchronous update. This is a
    // heurstic to detect infinite update loops. We are intentionally excluding
    // hydration lanes in this check, because render triggered by selective
    // hydration is conceptually not an update.
    if
    // Was the finished render the result of an update (not hydration)?
    includes_some_lane(lanes, UPDATE_LANES)
        // Did it schedule a sync update?
        && includes_some_lane(remaining_lanes, SYNC_UPDATE_LANES)
    {
        if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
            && ENABLE_PROFILER_NESTED_UPDATE_PHASE.with(|&enabled| enabled)
        {
            mark_nested_update_scheduled();
        }

        // Count the number of times the root synchronously re-renders without
        // finishing. If there are too many, it indicates an infinite update loop.
        if ROOT_WITH_NESTED_UPDATES.with_borrow(|root_with_nested_updates| {
            root_with_nested_updates
                .as_ref()
                .is_some_and(|root_with_nested_updates| Rc::ptr_eq(&root, root_with_nested_updates))
        }) {
            NESTED_UPDATE_COUNT.set(NESTED_UPDATE_COUNT.get() + 1);
        } else {
            NESTED_UPDATE_COUNT.set(0);
            ROOT_WITH_NESTED_UPDATES.set(Some(Rc::clone(&root)));
        }
        NESTED_UPDATE_KIND.set(NESTED_UPDATE_SYNC_LANE);
    } else if
    // Check if there was a recursive update spawned by this render, in either
    // the render phase or the commit phase. We track these explicitly because
    // we can't infer from the remaining lanes alone.
    ENABLE_INFINITE_RENDER_LOOP_DETECTION
        && (did_include_render_phase_update || DID_INCLUDE_COMMIT_PHASE_UPDATE.get())
    {
        if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
            && ENABLE_PROFILER_NESTED_UPDATE_PHASE.with(|&enabled| enabled)
        {
            mark_nested_update_scheduled();
        }

        // Count the number of times the root synchronously re-renders without
        // finishing. If there are too many, it indicates an infinite update loop.
        if ROOT_WITH_NESTED_UPDATES.with_borrow(|root_with_nested_updates| {
            root_with_nested_updates
                .as_ref()
                .is_some_and(|root_with_nested_updates| Rc::ptr_eq(&root, root_with_nested_updates))
        }) {
            NESTED_UPDATE_COUNT.set(NESTED_UPDATE_COUNT.get() + 1);
        } else {
            NESTED_UPDATE_COUNT.set(0);
            ROOT_WITH_NESTED_UPDATES.set(Some(Rc::clone(&root)));
        }
        NESTED_UPDATE_KIND.set(NESTED_UPDATE_PHASE_SPAWN);
    } else {
        NESTED_UPDATE_COUNT.set(0);
        ROOT_WITH_NESTED_UPDATES.set(js::null());
        NESTED_UPDATE_KIND.set(NO_NESTED_UPDATE);
    }

    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        && ENABLE_COMPONENT_PERFORMANCE_TRACK
        && !root_did_have_passive_effects
    {
        finalize_render(lanes, COMMIT_END_TIME.get());
    }

    // Eagerly flush any event replaying that we unblocked within this commit.
    // This ensures that those are observed before we render any new changes.
    if SUPPORTS_HYDRATION {
        flush_hydration_events();
    }

    // If layout work was scheduled, flush it now.
    flush_sync_work_on_all_roots();

    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled) {
        mark_commit_stopped();
    }

    if ENABLE_TRANSITION_TRACING {
        // We process transitions during passive effects. However, passive effects can be
        // processed synchronously during the commit phase as well as asynchronously after
        // paint. At the end of the commit phase, we schedule a callback that will be called
        // after the next paint. If the transitions have already been processed (passive
        // effect phase happened synchronously), we will schedule a callback to process
        // the transitions. However, if we don't have any pending transition callbacks, this
        // means that the transitions have yet to be processed (passive effects processed after paint)
        // so we will store the end time of paint so that we can process the transitions
        // and then call the callback via the correct end time.
        let prev_root_transition_callbacks = root.transition_callbacks.get();
        if let Some(prev_root_transition_callbacks) = prev_root_transition_callbacks {
            schedule_post_paint_callback(Rc::new(move |end_time| {
                let prev_pending_transition_callbacks = CURRENT_PENDING_TRANSITION_CALLBACKS.take();
                if let Some(prev_pending_transition_callbacks) = prev_pending_transition_callbacks {
                    schedule_callback(
                        IDLE_SCHEDULER_PRIORITY.with(|&priority| priority),
                        SchedulerCallback(Box::new(move |_| {
                            process_transition_callbacks(
                                &prev_pending_transition_callbacks,
                                end_time,
                                prev_root_transition_callbacks,
                            );
                            None
                        })),
                    );
                } else {
                    CURRENT_END_TIME.set(Some(end_time));
                }
            }));
        }
    }
}

#[cfg_attr(rust_js, rust_js::nullable(_recoverable_errors, suspended_state))]
fn apply_gesture_on_root(
    root: &Rc<FiberRoot>,
    finished_work: &Rc<Fiber>,
    _recoverable_errors: Option<&CaptureErrors>,
    suspended_state: Option<&'static Unknown>,
    _render_end_time: f64, // Profiling-only
) {
    // We assume that the gesture we just rendered was the first one in the queue.
    let finished_gesture = root.pending_gestures.borrow().clone();
    let Some(finished_gesture) = finished_gesture else {
        // We must have already cancelled this gesture before we had a chance to
        // render it. Let's schedule work on the next set of lanes.
        ensure_root_is_scheduled(root);
        return;
    };

    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK {
        start_animating(PENDING_EFFECTS_LANES.get());
    }

    PENDING_VIEW_TRANSITION_EVENTS.set(js::null());

    let prev_transition = t(REACT_SHARED_INTERNALS);
    set_t(REACT_SHARED_INTERNALS, js::null());
    let previous_priority = get_current_update_priority();
    set_current_update_priority(DISCRETE_EVENT_PRIORITY);
    let prev_execution_context = EXECUTION_CONTEXT.get();
    EXECUTION_CONTEXT.set(EXECUTION_CONTEXT.get() | COMMIT_CONTEXT);
    {
        let _exit = ExitCommitPhase {
            prev_execution_context,
            previous_priority,
            prev_transition,
        };
        insert_destination_clones(root, finished_work);
    }
    PENDING_TRANSITION_TYPES.set(finished_gesture.types.borrow().clone());
    PENDING_EFFECTS_STATUS.set(PENDING_GESTURE_MUTATION_PHASE);

    let running = start_gesture_transition(
        suspended_state,
        root.container_info,
        finished_gesture.provider,
        finished_gesture.range_start.get(),
        finished_gesture.range_end.get(),
        PENDING_TRANSITION_TYPES.with_borrow(Clone::clone),
        flush_gesture_mutations,
        flush_gesture_animations,
        report_view_transition_error,
        if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
            // This callback fires after "pendingEffects" so we need to snapshot the arguments.
            Some(function::bind(
                finished_view_transition,
                Some(&Kind::Null),
                (PENDING_EFFECTS_LANES.get(),),
            ))
        } else {
            js::null()
        },
    );
    finished_gesture.running.set(running);
    PENDING_VIEW_TRANSITION.set(running);
}

fn flush_gesture_mutations() {
    if !ENABLE_GESTURE_TRANSITION.with(|&enabled| enabled) {
        return;
    }
    if PENDING_EFFECTS_STATUS.get() != PENDING_GESTURE_MUTATION_PHASE {
        return;
    }
    PENDING_EFFECTS_STATUS.set(NO_PENDING_EFFECTS);
    let root = PENDING_EFFECTS_ROOT.with_borrow(Clone::clone).unwrap();
    let finished_work = PENDING_FINISHED_WORK.with_borrow(Clone::clone).unwrap();

    let prev_transition = t(REACT_SHARED_INTERNALS);
    set_t(REACT_SHARED_INTERNALS, js::null());
    let previous_priority = get_current_update_priority();
    set_current_update_priority(DISCRETE_EVENT_PRIORITY);
    let prev_execution_context = EXECUTION_CONTEXT.get();
    EXECUTION_CONTEXT.set(EXECUTION_CONTEXT.get() | COMMIT_CONTEXT);
    {
        let _exit = ExitCommitPhase {
            prev_execution_context,
            previous_priority,
            prev_transition,
        };
        apply_departure_transitions(&root, &finished_work);
    }

    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK {
        record_commit_end_time();
        log_apply_gesture_phase(
            PENDING_EFFECTS_RENDER_END_TIME.get(),
            COMMIT_END_TIME.get(),
            ANIMATING_TASK.get(),
        );
    }

    PENDING_EFFECTS_STATUS.set(PENDING_GESTURE_ANIMATION_PHASE);
}

fn flush_gesture_animations() {
    if !ENABLE_GESTURE_TRANSITION.with(|&enabled| enabled) {
        return;
    }
    // If we get canceled before we start we might not have applied
    // mutations yet. We need to apply them first.
    flush_gesture_mutations();
    if PENDING_EFFECTS_STATUS.get() != PENDING_GESTURE_ANIMATION_PHASE {
        return;
    }

    let lanes = PENDING_EFFECTS_LANES.get();

    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK {
        let start_view_transition_start_time = COMMIT_END_TIME.get();
        // Update the new commitEndTime to when we started the animation.
        record_commit_end_time();
        log_start_view_transition_yield_phase(
            start_view_transition_start_time,
            COMMIT_END_TIME.get(),
            PENDING_DELAYED_COMMIT_REASON.get() == ABORTED_VIEW_TRANSITION_COMMIT,
            ANIMATING_TASK.get(),
        );
        if PENDING_DELAYED_COMMIT_REASON.get() != ABORTED_VIEW_TRANSITION_COMMIT {
            PENDING_DELAYED_COMMIT_REASON.set(ANIMATION_STARTED_COMMIT);
        }
    }

    PENDING_EFFECTS_STATUS.set(NO_PENDING_EFFECTS);
    let root = PENDING_EFFECTS_ROOT.take().unwrap(); // Clear for GC purposes.
    let finished_work = PENDING_FINISHED_WORK.take().unwrap(); // Clear for GC purposes.
    PENDING_EFFECTS_LANES.set(NO_LANES);

    PENDING_VIEW_TRANSITION.set(js::null()); // The view transition has now fully started.

    let prev_transition = t(REACT_SHARED_INTERNALS);
    set_t(REACT_SHARED_INTERNALS, js::null());
    let previous_priority = get_current_update_priority();
    set_current_update_priority(DISCRETE_EVENT_PRIORITY);
    let prev_execution_context = EXECUTION_CONTEXT.get();
    EXECUTION_CONTEXT.set(EXECUTION_CONTEXT.get() | COMMIT_CONTEXT);
    {
        let _exit = ExitCommitPhase {
            prev_execution_context,
            previous_priority,
            prev_transition,
        };
        start_gesture_animations(&root, &finished_work);
    }

    if ENABLE_VIEW_TRANSITION {
        // We should now be after the startGestureTransition's .ready call which is late enough
        // to start animating any pseudo-elements. We have also already applied any adjustments
        // we do to the built-in animations which can now be read by the refs.
        let pending_events = PENDING_VIEW_TRANSITION_EVENTS.take();
        let pending_types = PENDING_TRANSITION_TYPES.take();
        if let Some(pending_events) = pending_events {
            // Normalize the type. This is lazily created only for events.
            let pending_types = pending_types.unwrap_or_default();
            let applied_gesture = root.pending_gestures.borrow().clone();
            if let Some(applied_gesture) = applied_gesture
                && let Some(running_transition) = applied_gesture.running.get()
            {
                for view_transition_event in &pending_events {
                    let cleanup = view_transition_event(&pending_types);
                    if let Some(cleanup) = cleanup {
                        add_view_transition_finished_listener(running_transition, cleanup);
                    }
                }
            }
        }
    }

    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK {
        finalize_render(lanes, COMMIT_END_TIME.get());
    }

    // Now that we've rendered this lane. Start working on the next lane.
    ensure_root_is_scheduled(&root);
}

fn make_error_info(component_stack: Option<String>) -> ErrorInfo {
    let error_info = ErrorInfo { component_stack };
    if __DEV__ {
        object::define_property(
            &error_info,
            "digest",
            &PropertyDescriptor {
                get: Some(&|| {
                    console::error((
                        "You are accessing \"digest\" from the errorInfo object passed to onRecoverableError. \
                         This property is no longer provided as part of errorInfo but can be accessed as a property \
                         of the Error instance itself.",
                    ));
                    None
                }),
                ..Default::default()
            },
        );
    }
    error_info
}

fn release_root_pooled_cache(root: &FiberRoot, remaining_lanes: Lanes) {
    root.pooled_cache_lanes
        .set(root.pooled_cache_lanes.get() & remaining_lanes);
    let pooled_cache_lanes = root.pooled_cache_lanes.get();
    if pooled_cache_lanes == NO_LANES {
        // None of the remaining work relies on the cache pool. Clear it so
        // subsequent requests get a new cache
        let pooled_cache = root.pooled_cache.take();
        if let Some(pooled_cache) = pooled_cache {
            release_cache(&pooled_cache);
        }
    }
}

thread_local! {
    static DID_WARN_ABOUT_INTERRUPTED_VIEW_TRANSITIONS: Cell<bool> = const { Cell::new(false) };
}

pub fn flush_pending_effects_delayed() -> bool {
    if PENDING_DELAYED_COMMIT_REASON.get() == IMMEDIATE_COMMIT {
        PENDING_DELAYED_COMMIT_REASON.set(DELAYED_PASSIVE_COMMIT);
    }
    flush_pending_effects()
}

pub fn flush_pending_effects() -> bool {
    // Returns whether passive effects were flushed.
    if ENABLE_VIEW_TRANSITION && let Some(pending_view_transition) = PENDING_VIEW_TRANSITION.get() {
        // If we forced a flush before the View Transition full started then we skip it.
        // This ensures that we're not running a partial animation.
        stop_view_transition(pending_view_transition);
        if __DEV__ && !DID_WARN_ABOUT_INTERRUPTED_VIEW_TRANSITIONS.get() {
            DID_WARN_ABOUT_INTERRUPTED_VIEW_TRANSITIONS.set(true);
            console::warn((
                "A flushSync update cancelled a View Transition because it was called \
                 while the View Transition was still preparing. To preserve the synchronous \
                 semantics, React had to skip the View Transition. If you can, try to avoid \
                 flushSync() in a scenario that's likely to interfere.",
            ));
        }
        PENDING_VIEW_TRANSITION.set(js::null());
        PENDING_DELAYED_COMMIT_REASON.set(ABORTED_VIEW_TRANSITION_COMMIT);
    }
    flush_gesture_mutations();
    flush_gesture_animations();
    flush_mutation_effects();
    flush_layout_effects();
    // Skip flushAfterMutation if we're forcing this early.
    flush_spawned_work();
    flush_passive_effects()
}

/// Leaves `flushPassiveEffects`: puts back the update priority and the
/// Transition, and releases the root's pooled cache once passive effects ran.
struct ExitFlushPassiveEffects {
    previous_priority: EventPriority,
    prev_transition: Option<&'static Transition>,
    root: Rc<FiberRoot>,
    remaining_lanes: Lanes,
}

impl Drop for ExitFlushPassiveEffects {
    fn drop(&mut self) {
        set_current_update_priority(self.previous_priority);
        set_t(REACT_SHARED_INTERNALS, self.prev_transition);

        // Once passive effects have run for the tree - giving components a
        // chance to retain cache instances they use - release the pooled
        // cache at the root (if there is one)
        release_root_pooled_cache(&self.root, self.remaining_lanes);
    }
}

fn flush_passive_effects() -> bool {
    if PENDING_EFFECTS_STATUS.get() != PENDING_PASSIVE_PHASE {
        return false;
    }
    // TODO: Merge flushPassiveEffectsImpl into this function. I believe they were only separate
    // in the first place because we used to wrap it with
    // `Scheduler.runWithPriority`, which accepts a function. But now we track the
    // priority within React itself, so we can mutate the variable directly.
    // Cache the root since pendingEffectsRoot is cleared in
    // flushPassiveEffectsImpl
    let root = PENDING_EFFECTS_ROOT.with_borrow(Clone::clone).unwrap();
    // Cache and clear the remaining lanes flag; it must be reset since this
    // method can be called from various places, not always from completeRoot
    // where the remaining lanes are known
    let remaining_lanes = PENDING_EFFECTS_REMAINING_LANES.get();
    PENDING_EFFECTS_REMAINING_LANES.set(NO_LANES);

    let render_priority = lanes_to_event_priority(PENDING_EFFECTS_LANES.get());
    let priority = lower_event_priority(DEFAULT_EVENT_PRIORITY, render_priority);
    let prev_transition = t(REACT_SHARED_INTERNALS);
    let previous_priority = get_current_update_priority();

    let _exit = ExitFlushPassiveEffects {
        previous_priority,
        prev_transition,
        root,
        remaining_lanes,
    };
    set_current_update_priority(priority);
    set_t(REACT_SHARED_INTERNALS, js::null());
    flush_passive_effects_impl()
}

fn flush_passive_effects_impl() -> bool {
    // Cache and clear the transitions flag
    let transitions = PENDING_PASSIVE_TRANSITIONS.take();

    let root = PENDING_EFFECTS_ROOT.take().unwrap(); // Clear for GC purposes.
    let lanes = PENDING_EFFECTS_LANES.get();
    PENDING_EFFECTS_STATUS.set(NO_PENDING_EFFECTS);
    PENDING_FINISHED_WORK.set(js::null()); // Clear for GC purposes.
    // TODO: This is sometimes out of sync with pendingEffectsRoot.
    // Figure out why and fix it. It's not causing any known issues (probably
    // because it's only used for profiling), but it's a refactor hazard.
    PENDING_EFFECTS_LANES.set(NO_LANES);

    if ENABLE_YIELDING_BEFORE_PASSIVE {
        // We've finished our work for this render pass.
        root.callback_node.set(js::null());
        root.callback_priority.set(NO_LANE);
    }

    if (EXECUTION_CONTEXT.get() & (RENDER_CONTEXT | COMMIT_CONTEXT)) != NO_CONTEXT {
        panic!("Cannot flush passive effects while already rendering.");
    }

    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK {
        // We're about to log a lot of profiling for this commit.
        // We set this once so we don't have to recompute it for every log.
        set_current_track_from_lanes(lanes);
    }

    if __DEV__ {
        IS_FLUSHING_PASSIVE_EFFECTS.set(true);
        DID_SCHEDULE_UPDATE_DURING_PASSIVE_EFFECTS.set(false);
    }

    let mut passive_effect_start_time = 0.0;
    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK {
        reset_commit_errors();
        passive_effect_start_time = now();
        if PENDING_DELAYED_COMMIT_REASON.get() == ANIMATION_STARTED_COMMIT {
            // The animation was started, so we've been animating since that happened.
            log_animating_phase(
                COMMIT_END_TIME.get(),
                passive_effect_start_time,
                ANIMATING_TASK.get(),
            );
        } else {
            log_paint_yield_phase(
                COMMIT_END_TIME.get(),
                passive_effect_start_time,
                PENDING_DELAYED_COMMIT_REASON.get() == DELAYED_PASSIVE_COMMIT,
                WORK_IN_PROGRESS_UPDATE_TASK.get(),
            );
        }
    }

    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled) {
        mark_passive_effects_started(lanes);
    }

    let prev_execution_context = EXECUTION_CONTEXT.get();
    EXECUTION_CONTEXT.set(EXECUTION_CONTEXT.get() | COMMIT_CONTEXT);

    let current = root.current.borrow().clone();
    commit_passive_unmount_effects(&current);
    let current = root.current.borrow().clone();
    commit_passive_mount_effects(
        &root,
        &current,
        lanes,
        transitions.as_deref().map(Vec::as_slice),
        PENDING_EFFECTS_RENDER_END_TIME.get(),
    );

    if ENABLE_SCHEDULING_PROFILER.with(|&enabled| enabled) {
        mark_passive_effects_stopped();
    }

    if __DEV__ {
        commit_double_invoke_effects_in_dev(&root, true);
    }

    EXECUTION_CONTEXT.set(prev_execution_context);

    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK {
        let passive_effects_end_time = now();
        log_passive_commit_phase(
            passive_effect_start_time,
            passive_effects_end_time,
            COMMIT_ERRORS.with_borrow(Clone::clone).as_deref(),
            WORK_IN_PROGRESS_UPDATE_TASK.get(),
        );
        finalize_render(lanes, passive_effects_end_time);
    }

    flush_sync_work_on_all_roots();

    if ENABLE_TRANSITION_TRACING {
        let prev_root_transition_callbacks = root.transition_callbacks.get();
        let prev_end_time = CURRENT_END_TIME.get();
        if CURRENT_PENDING_TRANSITION_CALLBACKS.with_borrow(Option::is_some)
            && let Some(prev_root_transition_callbacks) = prev_root_transition_callbacks
            && let Some(prev_end_time) = prev_end_time
        {
            let prev_pending_transition_callbacks =
                CURRENT_PENDING_TRANSITION_CALLBACKS.take().unwrap();
            CURRENT_END_TIME.set(js::null());
            schedule_callback(
                IDLE_SCHEDULER_PRIORITY.with(|&priority| priority),
                SchedulerCallback(Box::new(move |_| {
                    process_transition_callbacks(
                        &prev_pending_transition_callbacks,
                        prev_end_time,
                        prev_root_transition_callbacks,
                    );
                    None
                })),
            );
        }
    }

    if __DEV__ {
        // If additional passive effects were scheduled, increment a counter. If this
        // exceeds the limit, we'll fire a warning.
        if DID_SCHEDULE_UPDATE_DURING_PASSIVE_EFFECTS.get() {
            if ROOT_WITH_PASSIVE_NESTED_UPDATES.with_borrow(|root_with_passive_nested_updates| {
                root_with_passive_nested_updates.as_ref().is_some_and(
                    |root_with_passive_nested_updates| {
                        Rc::ptr_eq(&root, root_with_passive_nested_updates)
                    },
                )
            }) {
                NESTED_PASSIVE_UPDATE_COUNT.set(NESTED_PASSIVE_UPDATE_COUNT.get() + 1);
            } else {
                NESTED_PASSIVE_UPDATE_COUNT.set(0);
                ROOT_WITH_PASSIVE_NESTED_UPDATES.set(Some(Rc::clone(&root)));
            }
        } else {
            NESTED_PASSIVE_UPDATE_COUNT.set(0);
        }
        IS_FLUSHING_PASSIVE_EFFECTS.set(false);
        DID_SCHEDULE_UPDATE_DURING_PASSIVE_EFFECTS.set(false);
    }

    if ENABLE_YIELDING_BEFORE_PASSIVE {
        // Next, we reschedule any remaining work in a new task since it's a new
        // sequence of work. We wait until the end to do this in case the passive
        // effect schedules higher priority work than we had remaining. That way
        // we don't schedule an early callback that gets cancelled anyway.
        ensure_root_is_scheduled(&root);
    }

    // TODO: Move to commitPassiveMountEffects
    on_post_commit_root_dev_tools(&root);
    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        && ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
    {
        let state_node: &FiberRoot = unsafe { js::cast(root.current.borrow().state_node.get()) };
        state_node.effect_duration.set(Some(0.0));
        state_node.passive_effect_duration.set(Some(0.0));
    }

    true
}

pub fn is_already_failed_legacy_error_boundary(instance: &'static Unknown) -> bool {
    LEGACY_ERROR_BOUNDARIES_THAT_ALREADY_FAILED
        .get()
        .is_some_and(|legacy_error_boundaries_that_already_failed| {
            legacy_error_boundaries_that_already_failed.has(instance)
        })
}

pub fn mark_legacy_error_boundary_as_failed(instance: &'static Unknown) {
    match LEGACY_ERROR_BOUNDARIES_THAT_ALREADY_FAILED.get() {
        None => {
            LEGACY_ERROR_BOUNDARIES_THAT_ALREADY_FAILED.set(Some(js::set::new(Some(&[instance]))))
        }
        Some(legacy_error_boundaries_that_already_failed) => {
            legacy_error_boundaries_that_already_failed.add(instance);
        }
    }
}

fn capture_commit_phase_error_on_root(
    root_fiber: &Rc<Fiber>,
    source_fiber: &Rc<Fiber>,
    error: Option<&'static Unknown>,
) {
    let error_info = create_captured_value_at_fiber(error, source_fiber);
    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK {
        record_effect_error(&error_info);
    }
    let update = create_root_error_update(
        unsafe { js::cast(root_fiber.state_node.get()) },
        &error_info,
        SYNC_LANE,
    );
    let root = enqueue_update(root_fiber, &update, SYNC_LANE);
    if let Some(root) = root {
        mark_root_updated(&root, SYNC_LANE);
        ensure_root_is_scheduled(&root);
    }
}

#[cfg_attr(rust_js, rust_js::nullable(nearest_mounted_ancestor, error))]
pub fn capture_commit_phase_error(
    source_fiber: &Rc<Fiber>,
    nearest_mounted_ancestor: Option<&Rc<Fiber>>,
    error: Option<&'static Unknown>,
) {
    if __DEV__ {
        set_is_running_insertion_effect(false);
    }
    if source_fiber.tag.get() == HOST_ROOT {
        // Error was thrown at the root. There is no parent, so the root
        // itself should capture it.
        capture_commit_phase_error_on_root(source_fiber, source_fiber, error);
        return;
    }

    let mut fiber = nearest_mounted_ancestor.cloned();
    while let Some(node) = fiber {
        if node.tag.get() == HOST_ROOT {
            capture_commit_phase_error_on_root(&node, source_fiber, error);
            return;
        } else if node.tag.get() == CLASS_COMPONENT {
            let ctor = node.r#type.get();
            let instance = node.state_node.get();
            if js::type_of(js::get(ctor, "getDerivedStateFromError")) == "function"
                || (js::type_of(js::get(instance, "componentDidCatch")) == "function"
                    && !is_already_failed_legacy_error_boundary(unsafe { js::cast(instance) }))
            {
                let error_info = create_captured_value_at_fiber(error, source_fiber);
                if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                    && ENABLE_COMPONENT_PERFORMANCE_TRACK
                {
                    record_effect_error(&error_info);
                }
                let update = create_class_error_update(SYNC_LANE);
                let root = enqueue_update(&node, &update, SYNC_LANE);
                if let Some(root) = root {
                    initialize_class_error_update(&update, &root, &node, &error_info);
                    mark_root_updated(&root, SYNC_LANE);
                    ensure_root_is_scheduled(&root);
                }
                return;
            }
        }
        fiber = node.r#return.borrow().clone();
    }

    if __DEV__ {
        console::error((
            "Internal React error: Attempted to capture a commit phase error \
             inside a detached tree. This indicates a bug in React. Potential \
             causes include deleting the same fiber more than once, committing an \
             already-finished tree, or an inconsistent return pointer.\n\n\
             Error message:\n\n%s",
            error,
        ));
    }
}

pub fn attach_ping_listener(root: &Rc<FiberRoot>, wakeable: &'static Wakeable, lanes: Lanes) {
    // Attach a ping listener
    //
    // The data might resolve before we have a chance to commit the fallback. Or,
    // in the case of a refresh, we'll never commit a fallback. So we need to
    // attach a listener now. When it resolves ("pings"), we can decide whether to
    // try rendering the tree again.
    //
    // Only attach a listener if one does not already exist for the lanes
    // we're currently rendering (which acts like a "thread ID" here).
    //
    // We only need to do this in concurrent mode. Legacy Suspense always
    // commits fallbacks synchronously, so there are no pings.
    let ping_cache = root.ping_cache.get();
    let thread_ids;
    match ping_cache {
        None => {
            let ping_cache: &'static PingCache = unsafe {
                js::cast(Some(js::new(
                    js::unknown(POSSIBLY_WEAK_MAP.with(|c| *c)),
                    (),
                )))
            };
            root.ping_cache.set(Some(ping_cache));
            thread_ids = js::set::new(None);
            ping_cache.set(wakeable, thread_ids);
        }
        Some(ping_cache) => match ping_cache.get(wakeable) {
            Some(existing_thread_ids) => thread_ids = existing_thread_ids,
            None => {
                thread_ids = js::set::new(None);
                ping_cache.set(wakeable, thread_ids);
            }
        },
    }
    if !thread_ids.has(&lanes) {
        WORK_IN_PROGRESS_ROOT_DID_ATTACH_PING_LISTENER.set(true);

        // Memoize using the thread ID to prevent redundant listeners.
        thread_ids.add(&lanes);
        let ping = function::bind(
            ping_suspended_root,
            Some(&Kind::Null),
            (Rc::clone(root), wakeable, lanes),
        );
        if ENABLE_UPDATER_TRACKING.with(|&enabled| enabled)
            && IS_DEV_TOOLS_PRESENT.with(|&present| present)
        {
            // If we have pending work still, restore the original updaters
            restore_pending_updaters(root, lanes);
        }
        wakeable.then(&ping, &ping);
    }
}

fn ping_suspended_root(root: &Rc<FiberRoot>, wakeable: &'static Wakeable, pinged_lanes: Lanes) {
    let ping_cache = root.ping_cache.get();
    if let Some(ping_cache) = ping_cache {
        // The wakeable resolved, so we no longer need to memoize, because it will
        // never be thrown again.
        ping_cache.delete(wakeable);
    }

    mark_root_pinged(root, pinged_lanes);

    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK {
        start_ping_timer_by_lanes(pinged_lanes);
    }

    warn_if_suspense_resolution_not_wrapped_with_act_dev(root);

    if WORK_IN_PROGRESS_ROOT.with_borrow(|work_in_progress_root| {
        work_in_progress_root
            .as_ref()
            .is_some_and(|work_in_progress_root| Rc::ptr_eq(work_in_progress_root, root))
    }) && is_subset_of_lanes(WORK_IN_PROGRESS_ROOT_RENDER_LANES.get(), pinged_lanes)
    {
        // Received a ping at the same priority level at which we're currently
        // rendering. We might want to restart this render. This should mirror
        // the logic of whether or not a root suspends once it completes.
        // TODO: If we're rendering sync either due to Sync, Batched or expired,
        // we should probably never restart.
        // TODO: Attach different listeners depending on whether the listener was
        // attached during prerendering. Prerender pings should not interrupt
        // normal renders.

        // If we're suspended with delay, or if it's a retry, we'll always suspend
        // so we can always restart.
        if WORK_IN_PROGRESS_ROOT_EXIT_STATUS.get() == ROOT_SUSPENDED_WITH_DELAY
            || (WORK_IN_PROGRESS_ROOT_EXIT_STATUS.get() == ROOT_SUSPENDED
                && includes_only_retries(WORK_IN_PROGRESS_ROOT_RENDER_LANES.get())
                && now() - GLOBAL_MOST_RECENT_FALLBACK_TIME.get() < FALLBACK_THROTTLE_MS)
        {
            // Force a restart from the root by unwinding the stack. Unless this is
            // being called from the render phase, because that would cause a crash.
            if (EXECUTION_CONTEXT.get() & RENDER_CONTEXT) == NO_CONTEXT {
                prepare_fresh_stack(root, NO_LANES);
            } else {
                // TODO: If this does happen during the render phase, we should throw
                // the special internal exception that we use to interrupt the stack for
                // selective hydration. That was temporarily reverted but we once we add
                // it back we can use it here.
                //
                // In the meantime, record the pinged lanes so markRootSuspended won't
                // mark them as suspended, allowing a retry.
                WORK_IN_PROGRESS_ROOT_PINGED_LANES.set(merge_lanes(
                    WORK_IN_PROGRESS_ROOT_PINGED_LANES.get(),
                    pinged_lanes,
                ));
            }
        } else {
            // Even though we can't restart right now, we might get an
            // opportunity later. So we mark this render as having a ping.
            WORK_IN_PROGRESS_ROOT_PINGED_LANES.set(merge_lanes(
                WORK_IN_PROGRESS_ROOT_PINGED_LANES.get(),
                pinged_lanes,
            ));
        }

        // If something pings the work-in-progress render, any work that suspended
        // up to this point may now be unblocked; in other words, no
        // longer suspended.
        //
        // Unlike the broader check above, we only need do this if the lanes match
        // exactly. If the lanes don't exactly match, that implies the promise
        // was created by an older render.
        if WORK_IN_PROGRESS_SUSPENDED_RETRY_LANES.get() == WORK_IN_PROGRESS_ROOT_RENDER_LANES.get()
        {
            WORK_IN_PROGRESS_SUSPENDED_RETRY_LANES.set(NO_LANES);
        }
    }

    ensure_root_is_scheduled(root);
}

pub fn ping_gesture_root(root: &Rc<FiberRoot>) {
    let gesture = root.pending_gestures.borrow().clone();
    if gesture.is_none() {
        return;
    }
    // Ping it for rerender and commit.
    mark_root_pinged(root, GESTURE_LANE);
    ensure_root_is_scheduled(root);
}

pub fn restart_gesture_root(root: &Rc<FiberRoot>) {
    if WORK_IN_PROGRESS_ROOT.with_borrow(|work_in_progress_root| {
        work_in_progress_root
            .as_ref()
            .is_some_and(|work_in_progress_root| Rc::ptr_eq(work_in_progress_root, root))
    }) && is_gesture_render(WORK_IN_PROGRESS_ROOT_RENDER_LANES.get())
    {
        // The current render was a gesture but it's now defunct. We need to restart the render.
        if (EXECUTION_CONTEXT.get() & RENDER_CONTEXT) == NO_CONTEXT {
            prepare_fresh_stack(root, NO_LANES);
        } else {
            // TODO: Throw interruption when supported again.
        }
    } else if root.cancel_pending_commit.borrow().is_some()
        && is_gesture_render(PENDING_EFFECTS_LANES.get())
    {
        // We have a suspended commit which we'll discard.
        let cancel_pending_commit = root.cancel_pending_commit.take();
        if let Some(cancel_pending_commit) = cancel_pending_commit {
            cancel_pending_commit();
        }
    }
    if root.pending_gestures.borrow().is_some() {
        // We still have gestures to work on. Let's schedule a restart.
        mark_root_pinged(root, GESTURE_LANE);
    }
    ensure_root_is_scheduled(root);
}

fn retry_timed_out_boundary(boundary_fiber: &Rc<Fiber>, mut retry_lane: Lane) {
    // The boundary fiber (a Suspense component or SuspenseList component)
    // previously was rendered in its fallback state. One of the promises that
    // suspended it has resolved, which means at least part of the tree was
    // likely unblocked. Try rendering again, at a new lanes.
    if retry_lane == NO_LANE {
        // TODO: Assign this to `suspenseState.retryLane`? to avoid
        // unnecessary entanglement?
        retry_lane = request_retry_lane(boundary_fiber);
    }
    // TODO: Special case idle priority?
    let root = enqueue_concurrent_render_for_lane(boundary_fiber, retry_lane);
    if let Some(root) = root {
        mark_root_updated(&root, retry_lane);
        ensure_root_is_scheduled(&root);
    }
}

pub fn retry_dehydrated_suspense_boundary(boundary_fiber: &Rc<Fiber>) {
    let suspense_state: Option<&SuspenseState> =
        unsafe { js::cast(boundary_fiber.memoized_state.get()) };
    let mut retry_lane: Lane = NO_LANE;
    if let Some(suspense_state) = suspense_state {
        retry_lane = suspense_state.retry_lane.get();
    }
    retry_timed_out_boundary(boundary_fiber, retry_lane);
}

pub fn resolve_retry_wakeable(boundary_fiber: &Rc<Fiber>, wakeable: &'static Unknown) {
    let mut retry_lane: Lane = NO_LANE; // Default
    let retry_cache: Option<&WeakSet<JsObject>>;
    match boundary_fiber.tag.get() {
        ACTIVITY_COMPONENT | SUSPENSE_COMPONENT => {
            retry_cache = unsafe { js::cast(boundary_fiber.state_node.get()) };
            let suspense_state: Option<&SuspenseState> =
                unsafe { js::cast(boundary_fiber.memoized_state.get()) };
            if let Some(suspense_state) = suspense_state {
                retry_lane = suspense_state.retry_lane.get();
            }
        }
        SUSPENSE_LIST_COMPONENT => {
            retry_cache = unsafe { js::cast(boundary_fiber.state_node.get()) };
        }
        OFFSCREEN_COMPONENT => {
            let instance: &OffscreenInstance = unsafe { js::cast(boundary_fiber.state_node.get()) };
            retry_cache = instance.retry_cache.get();
        }
        _ => {
            panic!("Pinged unknown suspense boundary type. This is probably a bug in React.");
        }
    }

    if let Some(retry_cache) = retry_cache {
        // The wakeable resolved, so we no longer need to memoize, because it will
        // never be thrown again.
        retry_cache.delete(unsafe { js::cast(Some(wakeable)) });
    }

    retry_timed_out_boundary(boundary_fiber, retry_lane);
}

#[cfg_attr(rust_js, rust_js::nullable(root))]
fn throw_forced_infinite_render_loop_error(root: Option<&FiberRoot>, render_lanes: Lanes) -> ! {
    if let Some(root) = root {
        // Disable concurrent error recovery for the in-progress render so the thrown
        // error reaches the nearest error boundary and breaks the infinite update
        // loop instead of being silently retried by the recovery mechanism.
        root.error_recovery_disabled_lanes.set(merge_lanes(
            root.error_recovery_disabled_lanes.get(),
            render_lanes,
        ));
    }
    panic!(
        "Maximum update depth exceeded. This could be an infinite loop. This can happen when a component \
         repeatedly calls setState during render phase or inside useLayoutEffect, \
         causing infinite render loop. React limits the number of nested updates to \
         prevent infinite loops."
    );
}

pub fn throw_if_infinite_update_loop_detected(
    is_from_infinite_render_loop_detection_instrumentation: bool,
) {
    if NESTED_UPDATE_COUNT.get() > NESTED_UPDATE_LIMIT {
        NESTED_UPDATE_COUNT.set(0);
        NESTED_PASSIVE_UPDATE_COUNT.set(0);
        ROOT_WITH_NESTED_UPDATES.set(js::null());
        ROOT_WITH_PASSIVE_NESTED_UPDATES.set(js::null());

        let update_kind = NESTED_UPDATE_KIND.get();
        NESTED_UPDATE_KIND.set(NO_NESTED_UPDATE);

        if ENABLE_INFINITE_RENDER_LOOP_DETECTION {
            if update_kind == NESTED_UPDATE_SYNC_LANE {
                if is_from_infinite_render_loop_detection_instrumentation
                    || (EXECUTION_CONTEXT.get() & RENDER_CONTEXT) != NO_CONTEXT
                {
                    // This loop was identified only because of the instrumentation gated with enableInfiniteRenderLoopDetection,
                    // warn instead of throwing, unless enableInfiniteRenderLoopDetectionForceThrow.
                    if ENABLE_INFINITE_RENDER_LOOP_DETECTION_FORCE_THROW {
                        throw_forced_infinite_render_loop_error(
                            WORK_IN_PROGRESS_ROOT.with_borrow(Clone::clone).as_deref(),
                            WORK_IN_PROGRESS_ROOT_RENDER_LANES.get(),
                        );
                    } else if __DEV__ {
                        console::error((
                            "Maximum update depth exceeded. This could be an infinite loop. This can happen when a component \
                             repeatedly calls setState during render phase or inside useLayoutEffect, \
                             causing infinite render loop. React limits the number of nested updates to \
                             prevent infinite loops.",
                        ));
                    }
                } else {
                    panic!(
                        "Maximum update depth exceeded. This can happen when a component \
                         repeatedly calls setState inside componentWillUpdate or \
                         componentDidUpdate. React limits the number of nested updates to \
                         prevent infinite loops."
                    );
                }
            } else if update_kind == NESTED_UPDATE_PHASE_SPAWN {
                if ENABLE_INFINITE_RENDER_LOOP_DETECTION_FORCE_THROW {
                    throw_forced_infinite_render_loop_error(
                        WORK_IN_PROGRESS_ROOT.with_borrow(Clone::clone).as_deref(),
                        WORK_IN_PROGRESS_ROOT_RENDER_LANES.get(),
                    );
                } else if __DEV__ {
                    console::error((
                        "Maximum update depth exceeded. This could be an infinite loop. This can happen when a component \
                         repeatedly calls setState during render phase or inside useLayoutEffect, \
                         causing infinite render loop. React limits the number of nested updates to \
                         prevent infinite loops.",
                    ));
                }
            }
        } else {
            panic!(
                "Maximum update depth exceeded. This can happen when a component \
                 repeatedly calls setState inside componentWillUpdate or \
                 componentDidUpdate. React limits the number of nested updates to \
                 prevent infinite loops."
            );
        }
    }

    if __DEV__ && NESTED_PASSIVE_UPDATE_COUNT.get() > NESTED_PASSIVE_UPDATE_LIMIT {
        NESTED_PASSIVE_UPDATE_COUNT.set(0);
        ROOT_WITH_PASSIVE_NESTED_UPDATES.set(js::null());

        console::error((
            "Maximum update depth exceeded. This can happen when a component \
             calls setState inside useEffect, but useEffect either doesn't \
             have a dependency array, or one of the dependencies changes on \
             every render.",
        ));
    }
}

#[cfg_attr(rust_js, rust_js::name = "flushRenderPhaseStrictModeWarningsInDEV")]
fn flush_render_phase_strict_mode_warnings_in_dev() {
    if __DEV__ {
        REACT_STRICT_MODE_WARNINGS.with(|react_strict_mode_warnings| {
            (react_strict_mode_warnings
                .flush_legacy_context_warning
                .get())();
            (react_strict_mode_warnings
                .flush_pending_unsafe_lifecycle_warnings
                .get())();
        });
    }
}

#[cfg_attr(
    rust_js,
    rust_js::name = "recursivelyTraverseAndDoubleInvokeEffectsInDEV"
)]
fn recursively_traverse_and_double_invoke_effects_in_dev(
    root: &FiberRoot,
    parent_fiber: &Fiber,
    is_in_strict_mode: bool,
) {
    if (parent_fiber.subtree_flags.get() & (PLACEMENT_DEV | VISIBILITY)) == NO_FLAGS {
        // Parent's descendants have already had effects double invoked.
        // Early exit to avoid unnecessary tree traversal.
        return;
    }
    let mut child = parent_fiber.child.borrow().clone();
    while let Some(node) = child {
        double_invoke_effects_in_dev_if_necessary(root, &node, is_in_strict_mode);
        child = node.sibling.borrow().clone();
    }
}

/// Leaves strict mode for DevTools.
struct ExitStrictModeForDevtools;

impl Drop for ExitStrictModeForDevtools {
    fn drop(&mut self) {
        set_is_strict_mode_for_devtools(false);
    }
}

// Unconditionally disconnects and connects passive and layout effects.
fn double_invoke_effects_on_fiber(root: &FiberRoot, fiber: &Rc<Fiber>) {
    set_is_strict_mode_for_devtools(true);
    let _exit = ExitStrictModeForDevtools;
    disappear_layout_effects_for_dev_validation(fiber);
    disconnect_passive_effect(fiber);
    let alternate = fiber.alternate.borrow().clone();
    reappear_layout_effects_for_dev_validation(root, alternate.as_ref(), fiber);
    reconnect_passive_effects(root, fiber, NO_LANES, js::null(), false, 0.0);
}

#[cfg_attr(rust_js, rust_js::name = "doubleInvokeEffectsInDEVIfNecessary")]
fn double_invoke_effects_in_dev_if_necessary(
    root: &FiberRoot,
    fiber: &Rc<Fiber>,
    parent_is_in_strict_mode: bool,
) {
    let is_strict_mode_fiber = matches!(
        SymbolFor::from_value(fiber.r#type.get()),
        Some(REACT_STRICT_MODE_TYPE)
    );
    let is_in_strict_mode = parent_is_in_strict_mode || is_strict_mode_fiber;

    // First case: the fiber **is not** of type OffscreenComponent. No
    // special rules apply to double invoking effects.
    if fiber.tag.get() != OFFSCREEN_COMPONENT {
        if (fiber.flags.get() & PLACEMENT_DEV) != 0 {
            if is_in_strict_mode {
                run_with_fiber_in_dev(
                    Some(Rc::clone(fiber)),
                    |root: &FiberRoot, fiber: &Rc<Fiber>, _: (), _: (), _: ()| {
                        double_invoke_effects_on_fiber(root, fiber)
                    },
                    root,
                    fiber,
                    (),
                    (),
                    (),
                );
            }
        } else {
            recursively_traverse_and_double_invoke_effects_in_dev(root, fiber, is_in_strict_mode);
        }
        return;
    }

    // Second case: the fiber **is** of type OffscreenComponent.
    // This branch contains cases specific to Offscreen.
    if fiber.memoized_state.get().is_none() {
        // Only consider Offscreen that is visible.
        // TODO (Offscreen) Handle manual mode.
        if is_in_strict_mode && (fiber.flags.get() & (VISIBILITY | PLACEMENT_DEV)) != 0 {
            // Double invoke effects on Offscreen's subtree
            // if it is visible and its visibility has changed.
            // However, we also need to consider newly hydrated Offscreen because their
            // visibility flags might not have changed.
            run_with_fiber_in_dev(
                Some(Rc::clone(fiber)),
                |root: &FiberRoot, fiber: &Rc<Fiber>, _: (), _: (), _: ()| {
                    double_invoke_effects_on_fiber(root, fiber)
                },
                root,
                fiber,
                (),
                (),
                (),
            );
        } else if (fiber.subtree_flags.get() & PLACEMENT_DEV) != 0 {
            // Something in the subtree could have been suspended.
            // We need to continue traversal and find newly inserted fibers.
            run_with_fiber_in_dev(
                Some(Rc::clone(fiber)),
                |root: &FiberRoot, fiber: &Rc<Fiber>, is_in_strict_mode: bool, _: (), _: ()| {
                    recursively_traverse_and_double_invoke_effects_in_dev(
                        root,
                        fiber,
                        is_in_strict_mode,
                    )
                },
                root,
                fiber,
                is_in_strict_mode,
                (),
                (),
            );
        }
    }
}

#[cfg_attr(rust_js, rust_js::name = "commitDoubleInvokeEffectsInDEV")]
fn commit_double_invoke_effects_in_dev(root: &FiberRoot, has_passive_effects: bool) {
    if __DEV__ {
        let current = root.current.borrow().clone();
        if DISABLE_LEGACY_MODE || root.tag != LEGACY_ROOT {
            let mut double_invoke_effects = true;

            if (DISABLE_LEGACY_MODE || root.tag == CONCURRENT_ROOT)
                && (current.mode.get() & (STRICT_LEGACY_MODE | STRICT_EFFECTS_MODE)) == 0
            {
                double_invoke_effects = false;
            }
            recursively_traverse_and_double_invoke_effects_in_dev(
                root,
                &current,
                double_invoke_effects,
            );
        } else {
            // TODO: Is this runWithFiberInDEV needed since the other effect functions do it too?
            run_with_fiber_in_dev(
                Some(Rc::clone(&current)),
                |fiber: &Rc<Fiber>, has_passive_effects: bool, _: (), _: (), _: ()| {
                    legacy_commit_double_invoke_effects_in_dev(fiber, has_passive_effects)
                },
                &current,
                has_passive_effects,
                (),
                (),
                (),
            );
        }
    }
}

#[cfg_attr(rust_js, rust_js::name = "legacyCommitDoubleInvokeEffectsInDEV")]
fn legacy_commit_double_invoke_effects_in_dev(fiber: &Rc<Fiber>, has_passive_effects: bool) {
    // TODO (StrictEffects) Should we set a marker on the root if it contains strict effects
    // so we don't traverse unnecessarily? similar to subtreeFlags but just at the root level.
    // Maybe not a big deal since this is DEV only behavior.

    invoke_effects_in_dev(fiber, MOUNT_LAYOUT_DEV, invoke_layout_effect_unmount_in_dev);
    if has_passive_effects {
        invoke_effects_in_dev(
            fiber,
            MOUNT_PASSIVE_DEV,
            invoke_passive_effect_unmount_in_dev,
        );
    }

    invoke_effects_in_dev(fiber, MOUNT_LAYOUT_DEV, invoke_layout_effect_mount_in_dev);
    if has_passive_effects {
        invoke_effects_in_dev(fiber, MOUNT_PASSIVE_DEV, invoke_passive_effect_mount_in_dev);
    }
}

fn invoke_effects_in_dev(
    first_child: &Rc<Fiber>,
    fiber_flags: Flags,
    invoke_effect_fn: fn(&Rc<Fiber>),
) {
    let mut current: Option<Rc<Fiber>> = Some(Rc::clone(first_child));
    let mut subtree_root: Option<Rc<Fiber>> = js::null();
    while let Some(node) = current {
        let primary_subtree_flag = node.subtree_flags.get() & fiber_flags;
        if !subtree_root
            .as_ref()
            .is_some_and(|subtree_root| Rc::ptr_eq(&node, subtree_root))
            && node.child.borrow().is_some()
            && primary_subtree_flag != NO_FLAGS
        {
            current = node.child.borrow().clone();
        } else {
            if (node.flags.get() & fiber_flags) != NO_FLAGS {
                invoke_effect_fn(&node);
            }

            let sibling = node.sibling.borrow().clone();
            if sibling.is_some() {
                current = sibling;
            } else {
                subtree_root = node.r#return.borrow().clone();
                current = subtree_root.clone();
            }
        }
    }
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::nullable)]
    static DID_WARN_STATE_UPDATE_FOR_NOT_YET_MOUNTED_COMPONENT: RefCell<Option<HashSet<String>>> = const { RefCell::new(js::null()) };
}

#[cfg_attr(rust_js, rust_js::name = "warnAboutUpdateOnNotYetMountedFiberInDEV")]
pub fn warn_about_update_on_not_yet_mounted_fiber_in_dev(fiber: &Rc<Fiber>) {
    if __DEV__ {
        if (EXECUTION_CONTEXT.get() & RENDER_CONTEXT) != NO_CONTEXT {
            // We let the other warning about render phase updates deal with this one.
            return;
        }

        if !DISABLE_LEGACY_MODE && (fiber.mode.get() & CONCURRENT_MODE) == 0 {
            return;
        }

        let tag = fiber.tag.get();
        if tag != HOST_ROOT
            && tag != CLASS_COMPONENT
            && tag != FUNCTION_COMPONENT
            && tag != FORWARD_REF
            && tag != MEMO_COMPONENT
            && tag != SIMPLE_MEMO_COMPONENT
        {
            // Only warn for user-defined components, not internal ones like Suspense.
            return;
        }

        // We show the whole stack but dedupe on the top component's name because
        // the problematic code almost always lies inside that component.
        let component_name = get_component_name_from_fiber(fiber)
            // Upstream's `|| 'ReactComponent'`: an empty name is none.
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "ReactComponent".to_string());
        let did_warn = DID_WARN_STATE_UPDATE_FOR_NOT_YET_MOUNTED_COMPONENT.with_borrow_mut(
            |did_warn_state_update_for_not_yet_mounted_component| {
                match did_warn_state_update_for_not_yet_mounted_component {
                    Some(did_warn_state_update_for_not_yet_mounted_component) => {
                        if did_warn_state_update_for_not_yet_mounted_component
                            .contains(&component_name)
                        {
                            return true;
                        }
                        did_warn_state_update_for_not_yet_mounted_component.insert(component_name);
                    }
                    None => {
                        *did_warn_state_update_for_not_yet_mounted_component =
                            Some(HashSet::from([component_name]));
                    }
                }
                false
            },
        );
        if did_warn {
            return;
        }

        run_with_fiber_in_dev(
            Some(Rc::clone(fiber)),
            |_: (), _: (), _: (), _: (), _: ()| {
                console::error((
                    "Can't perform a React state update on a component that hasn't mounted yet. \
                     This indicates that you have a side-effect in your render function that \
                     asynchronously tries to update the component. Move this work to \
                     useEffect instead.",
                ));
            },
            (),
            (),
            (),
            (),
            (),
        );
    }
}

thread_local! {
    static DID_WARN_ABOUT_UPDATE_IN_RENDER: Cell<bool> = const { Cell::new(false) };
    static DID_WARN_ABOUT_UPDATE_IN_RENDER_FOR_ANOTHER_COMPONENT: RefCell<HashSet<String>> =
        RefCell::new(HashSet::new());
}

#[cfg_attr(rust_js, rust_js::name = "warnAboutRenderPhaseUpdatesInDEV")]
fn warn_about_render_phase_updates_in_dev(fiber: &Fiber) {
    if __DEV__ && IS_RENDERING.get() {
        match fiber.tag.get() {
            FUNCTION_COMPONENT | FORWARD_REF | SIMPLE_MEMO_COMPONENT => {
                let rendering_component_name = WORK_IN_PROGRESS
                    .with_borrow(Clone::clone)
                    .and_then(|work_in_progress| get_component_name_from_fiber(&work_in_progress))
                    // Upstream's `|| 'Unknown'`: an empty name is none.
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| "Unknown".to_string());
                // Dedupe by the rendering component because it's the one that needs to be fixed.
                let dedupe_key = rendering_component_name.clone();
                if !DID_WARN_ABOUT_UPDATE_IN_RENDER_FOR_ANOTHER_COMPONENT
                    .with_borrow(|did_warn| did_warn.contains(&dedupe_key))
                {
                    DID_WARN_ABOUT_UPDATE_IN_RENDER_FOR_ANOTHER_COMPONENT
                        .with_borrow_mut(|did_warn| did_warn.insert(dedupe_key));
                    let set_state_component_name = get_component_name_from_fiber(fiber)
                        // Upstream's `|| 'Unknown'`: an empty name is none.
                        .filter(|name| !name.is_empty())
                        .unwrap_or_else(|| "Unknown".to_string());
                    console::error((
                        "Cannot update a component (`%s`) while rendering a \
                         different component (`%s`). To locate the bad setState() call inside `%s`, \
                         follow the stack trace as described in https://react.dev/link/setstate-in-render",
                        set_state_component_name,
                        &rendering_component_name,
                        &rendering_component_name,
                    ));
                }
            }
            CLASS_COMPONENT if !DID_WARN_ABOUT_UPDATE_IN_RENDER.get() => {
                console::error((
                    "Cannot update during an existing state transition (such as \
                     within `render`). Render methods should be a pure \
                     function of props and state.",
                ));
                DID_WARN_ABOUT_UPDATE_IN_RENDER.set(true);
            }
            _ => {}
        }
    }
}

pub fn restore_pending_updaters(root: &FiberRoot, lanes: Lanes) {
    if ENABLE_UPDATER_TRACKING.with(|&enabled| enabled)
        && IS_DEV_TOOLS_PRESENT.with(|&present| present)
    {
        let memoized_updaters = &root.memoized_updaters;
        for scheduling_fiber in memoized_updaters.borrow().iter() {
            add_fiber_to_lanes_map(root, scheduling_fiber, lanes);
        }

        // This function intentionally does not clear memoized updaters.
        // Those may still be relevant to the current commit
        // and a future one (e.g. Suspense).
    }
}

thread_local! {
    static FAKE_ACT_CALLBACK_NODE: &'static Dict<()> = js::dict::new();
}

fn schedule_callback(
    priority_level: PriorityLevel,
    callback: SchedulerCallback,
) -> &'static Unknown {
    if __DEV__ {
        // If we're currently inside an `act` scope, bypass Scheduler and push to
        // the `act` queue instead.
        let act_queue = REACT_SHARED_INTERNALS.act_queue();
        if let Some(act_queue) = act_queue {
            let act_queue: &RefCell<Vec<SchedulerCallback>> = unsafe { js::cast(Some(act_queue)) };
            act_queue.borrow_mut().push(callback);
            FAKE_ACT_CALLBACK_NODE
                .with(|fake_act_callback_node| js::unknown(*fake_act_callback_node))
        } else {
            js::unknown(SCHEDULER_SCHEDULE_CALLBACK(priority_level, callback, None))
        }
    } else {
        // In production, always call Scheduler. This function will be stripped out.
        js::unknown(SCHEDULER_SCHEDULE_CALLBACK(priority_level, callback, None))
    }
}

#[cfg_attr(rust_js, rust_js::name = "shouldForceFlushFallbacksInDEV")]
fn should_force_flush_fallbacks_in_dev() -> bool {
    // Never force flush in production. This function should get stripped out.
    __DEV__ && REACT_SHARED_INTERNALS.act_queue().is_some()
}

#[cfg_attr(rust_js, rust_js::name = "warnIfUpdatesNotWrappedWithActDEV")]
fn warn_if_updates_not_wrapped_with_act_dev(fiber: &Rc<Fiber>) {
    if __DEV__ {
        if DISABLE_LEGACY_MODE || (fiber.mode.get() & CONCURRENT_MODE) != 0 {
            if is_concurrent_act_environment() != Some(true) {
                // Not in an act environment. No need to warn.
                return;
            }
        } else {
            // Legacy mode has additional cases where we suppress a warning.
            if !is_legacy_act_environment(fiber) {
                // Not in an act environment. No need to warn.
                return;
            }
            if EXECUTION_CONTEXT.get() != NO_CONTEXT {
                // Legacy mode doesn't warn if the update is batched, i.e.
                // batchedUpdates or flushSync.
                return;
            }
            if fiber.tag.get() != FUNCTION_COMPONENT
                && fiber.tag.get() != FORWARD_REF
                && fiber.tag.get() != SIMPLE_MEMO_COMPONENT
            {
                // For backwards compatibility with pre-hooks code, legacy mode only
                // warns for updates that originate from a hook.
                return;
            }
        }

        if REACT_SHARED_INTERNALS.act_queue().is_none() {
            run_with_fiber_in_dev(
                Some(Rc::clone(fiber)),
                |fiber: &Rc<Fiber>, _: (), _: (), _: (), _: ()| {
                    console::error((
                        "An update to %s inside a test was not wrapped in act(...).\n\n\
                         When testing, code that causes React state updates should be \
                         wrapped into act(...):\n\n\
                         act(() => {\n  /* fire events that update state */\n});\n\
                         /* assert on the output */\n\n\
                         This ensures that you're testing the behavior the user would see \
                         in the browser. Learn more at https://react.dev/link/wrap-tests-with-act",
                        get_component_name_from_fiber(fiber),
                    ));
                },
                fiber,
                (),
                (),
                (),
                (),
            );
        }
    }
}

#[cfg_attr(
    rust_js,
    rust_js::name = "warnIfSuspenseResolutionNotWrappedWithActDEV"
)]
fn warn_if_suspense_resolution_not_wrapped_with_act_dev(root: &FiberRoot) {
    if __DEV__
        && (DISABLE_LEGACY_MODE || root.tag != LEGACY_ROOT)
        && is_concurrent_act_environment() == Some(true)
        && REACT_SHARED_INTERNALS.act_queue().is_none()
    {
        console::error((
            "A suspended resource finished loading inside a test, but the event \
             was not wrapped in act(...).\n\n\
             When testing, code that resolves suspended data should be wrapped \
             into act(...):\n\n\
             act(() => {\n  /* finish loading suspended data */\n});\n\
             /* assert on the output */\n\n\
             This ensures that you're testing the behavior the user would see \
             in the browser. Learn more at https://react.dev/link/wrap-tests-with-act",
        ));
    }
}

pub fn set_is_running_insertion_effect(is_running: bool) {
    if __DEV__ {
        IS_RUNNING_INSERTION_EFFECT.set(is_running);
    }
}
