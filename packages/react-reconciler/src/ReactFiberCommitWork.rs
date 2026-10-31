// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use js::{Class, JsError, JsObject, Kind, Set, Unknown, WeakSet, function};
use react_isomorphic::react_start_transition::Transition;
use shared::environment::__DEV__;
use shared::react_feature_flags::{
    ALWAYS_THROTTLE_RETRIES, DISABLE_LEGACY_MODE, ENABLE_COMPONENT_PERFORMANCE_TRACK,
    ENABLE_CREATE_EVENT_HANDLE_API, ENABLE_DEFAULT_TRANSITION_INDICATOR, ENABLE_LEGACY_HIDDEN,
    ENABLE_PROFILER_COMMIT_HOOKS, ENABLE_PROFILER_TIMER, ENABLE_SCOPE_API,
    ENABLE_SUSPENSE_CALLBACK, ENABLE_TRANSITION_TRACING, ENABLE_UPDATER_TRACKING,
    ENABLE_VIEW_TRANSITION,
};
use shared::react_types::{ViewTransitionProps, Wakeable};
use webapi::console;

use crate::react_fiber::{ProfilerStateNode, ViewTransitionState};
use crate::react_fiber_activity_component::ActivityState;
use crate::react_fiber_cache_component::{
    Cache, CacheComponentState, SpawnedCachePool, release_cache, retain_cache,
};
use crate::react_fiber_call_user_space::ClassInstance;
use crate::react_fiber_class_update_queue::{UpdateQueue, defer_hidden_callbacks};
use crate::react_fiber_commit_effects::{
    commit_class_callbacks, commit_class_did_mount, commit_class_hidden_callbacks,
    commit_class_layout_lifecycles, commit_class_snapshot, commit_hook_effect_list_mount,
    commit_hook_effect_list_unmount, commit_hook_layout_effects,
    commit_hook_layout_unmount_effects, commit_hook_passive_mount_effects,
    commit_hook_passive_unmount_effects, commit_profiler_post_commit, commit_profiler_update,
    commit_root_callbacks, safely_attach_ref, safely_call_component_will_unmount,
    safely_detach_ref,
};
use crate::react_fiber_commit_host_effects::{
    commit_host_hydrated_activity, commit_host_hydrated_container, commit_host_hydrated_instance,
    commit_host_hydrated_suspense, commit_host_mount, commit_host_placement,
    commit_host_portal_container_children, commit_host_remove_child,
    commit_host_remove_child_from_container, commit_host_reset_text_content,
    commit_host_root_container_children, commit_host_singleton_acquisition,
    commit_host_singleton_release, commit_host_text_update, commit_host_update,
    commit_show_hide_host_instance, commit_show_hide_host_text_instance,
    commit_show_hide_suspense_boundary,
};
use crate::react_fiber_commit_view_transitions::{
    VIEW_TRANSITION_CANCELABLE_CHILDREN, commit_before_update_view_transition,
    commit_enter_view_transitions, commit_exit_view_transitions, commit_nested_view_transitions,
    measure_nested_view_transitions, measure_update_view_transition,
    pop_view_transition_cancelable_scope, push_view_transition_cancelable_scope,
    reset_appearing_view_transitions, restore_enter_or_exit_view_transitions,
    restore_nested_view_transitions, restore_update_view_transition,
    track_appearing_view_transition, track_enter_view_transitions,
};
use crate::react_fiber_dev_tools_hook::{IS_DEV_TOOLS_PRESENT, on_commit_unmount};
use crate::react_fiber_duplicate_view_transitions::{
    track_named_view_transition, untrack_named_view_transition,
};
use crate::react_fiber_flags::{
    AFFECTED_PARENT_LAYOUT, BEFORE_AND_AFTER_MUTATION_TRANSITION_MASK, BEFORE_MUTATION_MASK,
    CALLBACK, CHILD_DELETION, CLONED, CONTENT_RESET, DID_CAPTURE, FORCE_CLIENT_RENDER, FORM_RESET,
    Flags, HYDRATE, HYDRATING, LAYOUT_MASK, MAY_SUSPEND_COMMIT, MUTATION_MASK, NO_FLAGS, PASSIVE,
    PASSIVE_MASK, PASSIVE_TRANSITION_MASK, PERFORMED_WORK, PLACEMENT, PORTAL_STATIC, REF,
    SHOULD_SUSPEND_COMMIT, SNAPSHOT, UPDATE, VIEW_TRANSITION_NAMED_STATIC, VISIBILITY,
};
use crate::react_fiber_fragment_instance::{
    commit_fragment_instance_deletion_effects, commit_fragment_instance_insertion_effects,
};
use crate::react_fiber_hooks::{EventFunctionPayload, FunctionComponentUpdateQueue};
use crate::react_fiber_lane::{
    Lanes, clear_transitions_for_lanes, includes_loading_indicator_lanes,
    includes_only_suspensey_commit_eligible_lanes, includes_only_view_transition_eligible_lanes,
};
use crate::react_fiber_mutation_tracking::{
    ROOT_MUTATION_CONTEXT, VIEW_TRANSITION_MUTATION_CONTEXT, pop_mutation_context,
    push_mutation_context, push_root_mutation_context,
};
use crate::react_fiber_offscreen_component::{
    OFFSCREEN_PASSIVE_EFFECTS_CONNECTED, OFFSCREEN_VISIBLE, OffscreenInstance, OffscreenQueue,
    OffscreenState,
};
use crate::react_fiber_performance_track::{
    log_component_disappeared, log_component_effect, log_component_errored, log_component_mount,
    log_component_reappeared, log_component_render, log_component_unmount, pop_deep_equality,
    push_deep_equality,
};
use crate::react_fiber_root::RootState;
use crate::react_fiber_root_scheduler::mark_indicator_handled;
use crate::react_fiber_suspense_component::{RetryQueue, SuspenseState};
use crate::react_fiber_tracing_marker_component::{
    SuspenseInfo, TRANSITION_ROOT, TRANSITION_TRACING_MARKER, TracingMarkerInstance,
    TransitionAbort, TransitionAbortReason, TransitionAborts,
};
use crate::react_fiber_tree_reflection::does_fiber_contain;
use crate::react_fiber_view_transition_component::get_view_transition_class_name;
use crate::react_fiber_work_loop::{
    add_marker_complete_callback_to_pending_transition,
    add_marker_incomplete_callback_to_pending_transition,
    add_marker_progress_callback_to_pending_transition,
    add_transition_complete_callback_to_pending_transition,
    add_transition_progress_callback_to_pending_transition,
    add_transition_start_callback_to_pending_transition, capture_commit_phase_error,
    mark_commit_time_of_fallback, resolve_retry_wakeable, restore_pending_updaters,
    retry_dehydrated_suspense_boundary, schedule_view_transition_event,
};
#[cfg_attr(rust_js, rust_js::name = "HookHasEffect")]
use crate::react_hook_effect_tags::HAS_EFFECT as HOOK_HAS_EFFECT;
#[cfg_attr(rust_js, rust_js::name = "HookInsertion")]
use crate::react_hook_effect_tags::INSERTION as HOOK_INSERTION;
#[cfg_attr(rust_js, rust_js::name = "HookLayout")]
use crate::react_hook_effect_tags::LAYOUT as HOOK_LAYOUT;
#[cfg_attr(rust_js, rust_js::name = "HookPassive")]
use crate::react_hook_effect_tags::PASSIVE as HOOK_PASSIVE;
use crate::react_internal_types::{Fiber, FiberRoot};
use crate::react_profiler_timer::{
    COMMIT_START_TIME, COMPONENT_EFFECT_DURATION, COMPONENT_EFFECT_END_TIME,
    COMPONENT_EFFECT_ERRORS, COMPONENT_EFFECT_SPAWNED_UPDATE, COMPONENT_EFFECT_START_TIME,
    bubble_nested_effect_durations, pop_component_effect_did_spawn_update,
    pop_component_effect_duration, pop_component_effect_errors, pop_component_effect_start,
    pop_nested_effect_durations, push_component_effect_did_spawn_update,
    push_component_effect_duration, push_component_effect_errors, push_component_effect_start,
    push_nested_effect_durations, reset_component_effect_timers,
};
use crate::react_type_of_mode::{CONCURRENT_MODE, NO_MODE, PROFILE_MODE};
use crate::react_work_tags::{
    ACTIVITY_COMPONENT, CACHE_COMPONENT, CLASS_COMPONENT, DEHYDRATED_FRAGMENT, FORWARD_REF,
    FRAGMENT, FUNCTION_COMPONENT, HOST_COMPONENT, HOST_HOISTABLE, HOST_PORTAL, HOST_ROOT,
    HOST_SINGLETON, HOST_TEXT, INCOMPLETE_CLASS_COMPONENT, LEGACY_HIDDEN_COMPONENT, MEMO_COMPONENT,
    OFFSCREEN_COMPONENT, PROFILER, SCOPE_COMPONENT, SIMPLE_MEMO_COMPONENT, SUSPENSE_COMPONENT,
    SUSPENSE_LIST_COMPONENT, TRACING_MARKER_COMPONENT, VIEW_TRANSITION_COMPONENT,
};

unsafe extern "Rust" {
    #[link_name = "./src/ReactFiberConfig.js#supportsMutation"]
    safe static SUPPORTS_MUTATION: bool;
    #[link_name = "./src/ReactFiberConfig.js#supportsPersistence"]
    safe static SUPPORTS_PERSISTENCE: bool;
    #[link_name = "./src/ReactFiberConfig.js#supportsHydration"]
    safe static SUPPORTS_HYDRATION: bool;
    #[link_name = "./src/ReactFiberConfig.js#supportsResources"]
    safe static SUPPORTS_RESOURCES: bool;
    #[link_name = "./src/ReactFiberConfig.js#supportsSingletons"]
    safe static SUPPORTS_SINGLETONS: bool;
}

// ReactFiberConfig's host operations, on its `Instance`, `TextInstance`,
// `Container`, `HoistableRoot`, `Resource`, `SuspenseInstance`,
// `FormInstance`, `Props` and `SuspendedState`.
#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#clearSuspenseBoundary"
)]
fn clear_suspense_boundary(
    parent_instance: &'static Unknown,
    suspense_instance: Option<&'static Unknown>,
) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#clearSuspenseBoundaryFromContainer"
)]
fn clear_suspense_boundary_from_container(
    container: &'static Unknown,
    suspense_instance: Option<&'static Unknown>,
) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#createContainerChildSet"
)]
fn create_container_child_set() -> &'static Unknown {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#clearContainer"
)]
fn clear_container(container: &'static Unknown) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#prepareScopeUpdate"
)]
fn prepare_scope_update(scope_instance: Option<&'static Unknown>, instance: &Fiber) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#prepareForCommit"
)]
#[cfg_attr(rust_js, rust_js::nullable)]
fn prepare_for_commit(container_info: &'static Unknown) -> Option<Rc<Fiber>> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#beforeActiveInstanceBlur"
)]
fn before_active_instance_blur(internal_instance_handle: &Fiber) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#detachDeletedInstance"
)]
fn detach_deleted_instance(node: &'static Unknown) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#getHoistableRoot"
)]
fn get_hoistable_root(container: &'static Unknown) -> &'static Unknown {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#acquireResource"
)]
fn acquire_resource(
    hoistable_root: &'static Unknown,
    resource: &'static Unknown,
    props: Option<&'static Unknown>,
) -> &'static Unknown {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#releaseResource"
)]
fn release_resource(resource: Option<&'static Unknown>) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#hydrateHoistable"
)]
fn hydrate_hoistable(
    hoistable_root: &'static Unknown,
    r#type: Option<&'static Unknown>,
    props: Option<&'static Unknown>,
    internal_instance_handle: &Fiber,
) -> &'static Unknown {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#createHoistableInstance"
)]
fn create_hoistable_instance(
    r#type: Option<&'static Unknown>,
    props: Option<&'static Unknown>,
    root_container_instance: Option<&'static Unknown>,
    internal_instance_handle: &Fiber,
) -> &'static Unknown {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#mountHoistable"
)]
fn mount_hoistable(
    hoistable_root: &'static Unknown,
    r#type: Option<&'static Unknown>,
    instance: Option<&'static Unknown>,
) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#unmountHoistable"
)]
fn unmount_hoistable(instance: Option<&'static Unknown>) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#prepareToCommitHoistables"
)]
fn prepare_to_commit_hoistables() {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#maySuspendCommitInSyncRender"
)]
fn may_suspend_commit_in_sync_render(
    r#type: Option<&'static Unknown>,
    props: Option<&'static Unknown>,
) -> bool {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#suspendInstance"
)]
fn suspend_instance(
    state: &'static Unknown,
    instance: Option<&'static Unknown>,
    r#type: Option<&'static Unknown>,
    props: Option<&'static Unknown>,
) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#suspendResource"
)]
fn suspend_resource(
    state: &'static Unknown,
    hoistable_root: &'static Unknown,
    resource: &'static Unknown,
    props: Option<&'static Unknown>,
) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#resetFormInstance"
)]
fn reset_form_instance(form: Option<&'static Unknown>) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#registerSuspenseInstanceRetry"
)]
fn register_suspense_instance_retry(instance: &'static Unknown, callback: &'static dyn Fn()) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#cancelViewTransitionName"
)]
fn cancel_view_transition_name(
    instance: Option<&'static Unknown>,
    old_name: &str,
    props: Option<&'static Unknown>,
) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#cancelRootViewTransitionName"
)]
fn cancel_root_view_transition_name(root_container: &'static Unknown) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#restoreRootViewTransitionName"
)]
fn restore_root_view_transition_name(root_container: &'static Unknown) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#isSingletonScope"
)]
fn is_singleton_scope(r#type: Option<&'static Unknown>) -> bool {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#updateFragmentInstanceFiber"
)]
fn update_fragment_instance_fiber(fiber: &Fiber, instance: &'static Unknown) {
    unreachable!()
}

// shared/ReactTypes' `Wakeable`.
type LayoutEffectTraversalFlags = i32;

#[cfg_attr(rust_js, rust_js::name = "NoLayoutEffectTraversalFlags")]
const NO_LAYOUT_EFFECT_TRAVERSAL_FLAGS: LayoutEffectTraversalFlags = /*        */ 0b00;
#[cfg_attr(rust_js, rust_js::name = "IncludeWorkInProgressEffects")]
const INCLUDE_WORK_IN_PROGRESS_EFFECTS: LayoutEffectTraversalFlags = /*       */ 0b01;
#[cfg_attr(rust_js, rust_js::name = "IncludeHostSingletons")]
const INCLUDE_HOST_SINGLETONS: LayoutEffectTraversalFlags = /*              */ 0b10;

thread_local! {
    // Used during the commit phase to track the state of the Offscreen component stack.
    // Allows us to avoid traversing the return path to find the nearest Offscreen ancestor.
    static OFFSCREEN_SUBTREE_IS_HIDDEN: Cell<bool> = const { Cell::new(false) };
    static OFFSCREEN_SUBTREE_WAS_HIDDEN: Cell<bool> = const { Cell::new(false) };
    // Track whether there's a hidden offscreen above with no HostComponent between. If so,
    // it overrides the hiddenness of the HostComponent below.
    static OFFSCREEN_DIRECT_PARENT_IS_HIDDEN: Cell<bool> = const { Cell::new(false) };

    // Used to track if a form needs to be reset at the end of the mutation phase.
    static NEEDS_FORM_RESET: Cell<bool> = const { Cell::new(false) };

    #[cfg_attr(rust_js, rust_js::name = "PossiblyWeakSet")]
    static POSSIBLY_WEAK_SET: &'static Class = if js::is_function(js::weak_set::new::<JsObject>) {
        js::class::<WeakSet<JsObject>>()
    } else {
        js::class::<Set<JsObject>>()
    };

    #[cfg_attr(rust_js, rust_js::nullable)]
    static NEXT_EFFECT: RefCell<Option<Rc<Fiber>>> = const { RefCell::new(js::null()) };

    // Used for Profiling builds to track updaters.
    #[cfg_attr(rust_js, rust_js::nullable)]
    static IN_PROGRESS_LANES: Cell<Option<Lanes>> = const { Cell::new(js::null()) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static IN_PROGRESS_ROOT: RefCell<Option<Rc<FiberRoot>>> = const { RefCell::new(js::null()) };

    #[cfg_attr(rust_js, rust_js::nullable)]
    static FOCUSED_INSTANCE_HANDLE: RefCell<Option<Rc<Fiber>>> = const { RefCell::new(js::null()) };
    pub static SHOULD_FIRE_AFTER_ACTIVE_INSTANCE_BLUR: Cell<bool> = const { Cell::new(false) };

    // Used during the commit phase to track whether a parent ViewTransition component
    // might have been affected by any mutations / relayouts below.
    static VIEW_TRANSITION_CONTEXT_CHANGED: Cell<bool> = const { Cell::new(false) };
    static IN_UPDATE_VIEW_TRANSITION: Cell<bool> = const { Cell::new(false) };
    static ROOT_VIEW_TRANSITION_AFFECTED: Cell<bool> = const { Cell::new(false) };
    static ROOT_VIEW_TRANSITION_NAME_CANCELED: Cell<bool> = const { Cell::new(false) };
}

fn is_hydrating_parent(current: &Fiber, finished_work: &Fiber) -> bool {
    if finished_work.tag.get() == ACTIVITY_COMPONENT {
        let prev_state: Option<&ActivityState> = unsafe { js::cast(current.memoized_state.get()) };
        let next_state: Option<&ActivityState> =
            unsafe { js::cast(finished_work.memoized_state.get()) };
        prev_state.is_some() && next_state.is_none()
    } else if finished_work.tag.get() == SUSPENSE_COMPONENT {
        let prev_state: Option<&SuspenseState> = unsafe { js::cast(current.memoized_state.get()) };
        let next_state: Option<&SuspenseState> =
            unsafe { js::cast(finished_work.memoized_state.get()) };
        prev_state.is_some_and(|prev_state| prev_state.dehydrated.is_some())
            && next_state.is_none_or(|next_state| next_state.dehydrated.is_none())
    } else if finished_work.tag.get() == HOST_ROOT {
        unsafe { js::cast::<&RootState>(current.memoized_state.get()) }.is_dehydrated
            && (finished_work.flags.get() & FORCE_CLIENT_RENDER) == NO_FLAGS
    } else {
        false
    }
}

pub fn commit_before_mutation_effects(
    root: &FiberRoot,
    first_child: Rc<Fiber>,
    committed_lanes: Lanes,
) {
    FOCUSED_INSTANCE_HANDLE.set(prepare_for_commit(root.container_info));
    SHOULD_FIRE_AFTER_ACTIVE_INSTANCE_BLUR.set(false);

    let is_view_transition_eligible =
        ENABLE_VIEW_TRANSITION && includes_only_view_transition_eligible_lanes(committed_lanes);

    NEXT_EFFECT.set(Some(first_child));
    commit_before_mutation_effects_begin(is_view_transition_eligible);

    // We no longer need to track the active instance fiber
    FOCUSED_INSTANCE_HANDLE.set(js::null());
    // We've found any matched pairs and can now reset.
    reset_appearing_view_transitions();
}

#[cfg_attr(rust_js, rust_js::name = "commitBeforeMutationEffects_begin")]
fn commit_before_mutation_effects_begin(is_view_transition_eligible: bool) {
    // If this commit is eligible for a View Transition we look into all mutated subtrees.
    // TODO: We could optimize this by marking these with the Snapshot subtree flag in the render phase.
    let subtree_mask = if is_view_transition_eligible {
        BEFORE_AND_AFTER_MUTATION_TRANSITION_MASK
    } else {
        BEFORE_MUTATION_MASK
    };
    while let Some(fiber) = NEXT_EFFECT.with_borrow(Clone::clone) {
        // This phase is only used for beforeActiveInstanceBlur.
        // Let's skip the whole loop if it's off.
        if ENABLE_CREATE_EVENT_HANDLE_API || is_view_transition_eligible {
            // TODO: Should wrap this in flags check, too, as optimization
            let deletions = fiber.deletions.borrow().clone();
            if let Some(deletions) = &deletions {
                let mut i = 0;
                while i < deletions.borrow().len() {
                    let deletion = Rc::clone(&deletions.borrow()[i]);
                    commit_before_mutation_effects_deletion(&deletion, is_view_transition_eligible);
                    i += 1;
                }
            }
        }

        if ENABLE_VIEW_TRANSITION
            && fiber.alternate.borrow().is_none()
            && (fiber.flags.get() & PLACEMENT) != NO_FLAGS
        {
            // Skip before mutation effects of the children because we don't want
            // to trigger updates of any nested view transitions and we shouldn't
            // have any other before mutation effects since snapshot effects are
            // only applied to updates. TODO: Model this using only flags.
            if is_view_transition_eligible {
                track_enter_view_transitions(&fiber);
            }
            commit_before_mutation_effects_complete(is_view_transition_eligible);
            continue;
        }

        // TODO: This should really unify with the switch in commitBeforeMutationEffectsOnFiber recursively.
        if ENABLE_VIEW_TRANSITION && fiber.tag.get() == OFFSCREEN_COMPONENT {
            let is_modern_root =
                DISABLE_LEGACY_MODE || (fiber.mode.get() & CONCURRENT_MODE) != NO_MODE;
            if is_modern_root {
                let current = fiber.alternate.borrow().clone();
                let is_hidden = fiber.memoized_state.get().is_some();
                if is_hidden {
                    if let Some(current) = &current
                        && current.memoized_state.get().is_none()
                        && is_view_transition_eligible
                    {
                        // Was previously mounted as visible but is now hidden.
                        commit_exit_view_transitions(current);
                    }
                    // Skip before mutation effects of the children because they're hidden.
                    commit_before_mutation_effects_complete(is_view_transition_eligible);
                    continue;
                } else if current
                    .as_ref()
                    .is_some_and(|current| current.memoized_state.get().is_some())
                {
                    // Was previously mounted as hidden but is now visible.
                    // Skip before mutation effects of the children because we don't want
                    // to trigger updates of any nested view transitions and we shouldn't
                    // have any other before mutation effects since snapshot effects are
                    // only applied to updates. TODO: Model this using only flags.
                    if is_view_transition_eligible {
                        track_enter_view_transitions(&fiber);
                    }
                    commit_before_mutation_effects_complete(is_view_transition_eligible);
                    continue;
                }
            }
        }

        let child = fiber.child.borrow().clone();
        if (fiber.subtree_flags.get() & subtree_mask) != NO_FLAGS
            && let Some(child) = child
        {
            child.r#return.replace(Some(Rc::clone(&fiber)));
            NEXT_EFFECT.set(Some(child));
        } else {
            if is_view_transition_eligible {
                // We are inside an updated subtree. Any mutations that affected the
                // parent HostInstance's layout or set of children (such as reorders)
                // might have also affected the positioning or size of the inner
                // ViewTransitions. Therefore we need to find them inside.
                commit_nested_view_transitions(&fiber);
            }
            commit_before_mutation_effects_complete(is_view_transition_eligible);
        }
    }
}

#[cfg_attr(rust_js, rust_js::name = "commitBeforeMutationEffects_complete")]
fn commit_before_mutation_effects_complete(is_view_transition_eligible: bool) {
    while let Some(fiber) = NEXT_EFFECT.with_borrow(Clone::clone) {
        commit_before_mutation_effects_on_fiber(&fiber, is_view_transition_eligible);

        let sibling = fiber.sibling.borrow().clone();
        if let Some(sibling) = sibling {
            let return_fiber = fiber.r#return.borrow().clone();
            sibling.r#return.replace(return_fiber);
            NEXT_EFFECT.set(Some(sibling));
            return;
        }

        let return_fiber = fiber.r#return.borrow().clone();
        NEXT_EFFECT.set(return_fiber);
    }
}

fn commit_before_mutation_effects_on_fiber(
    finished_work: &Rc<Fiber>,
    is_view_transition_eligible: bool,
) {
    let current = finished_work.alternate.borrow().clone();
    let flags = finished_work.flags.get();

    if ENABLE_CREATE_EVENT_HANDLE_API
        && !SHOULD_FIRE_AFTER_ACTIVE_INSTANCE_BLUR.get()
        && let Some(focused_instance_handle) = FOCUSED_INSTANCE_HANDLE.with_borrow(Clone::clone)
    {
        // Check to see if the focused element was inside of a hidden (Suspense) subtree.
        // TODO: Move this out of the hot path using a dedicated effect tag.
        // TODO: This should consider Offscreen in general and not just SuspenseComponent.
        if finished_work.tag.get() == SUSPENSE_COMPONENT
            && is_suspense_boundary_being_hidden(current.as_deref(), finished_work)
            && does_fiber_contain(finished_work, &focused_instance_handle)
        {
            SHOULD_FIRE_AFTER_ACTIVE_INSTANCE_BLUR.set(true);
            before_active_instance_blur(finished_work);
        }
    }

    match finished_work.tag.get() {
        CLASS_COMPONENT => {
            if (flags & SNAPSHOT) != NO_FLAGS
                && let Some(current) = &current
            {
                commit_class_snapshot(finished_work, current);
            }
        }
        HOST_ROOT => {
            if (flags & SNAPSHOT) != NO_FLAGS && SUPPORTS_MUTATION {
                let root: &FiberRoot = unsafe { js::cast(finished_work.state_node.get()) };
                clear_container(root.container_info);
            }
        }
        FUNCTION_COMPONENT
        | FORWARD_REF
        | SIMPLE_MEMO_COMPONENT
        | HOST_COMPONENT
        | HOST_HOISTABLE
        | HOST_SINGLETON
        | HOST_TEXT
        | HOST_PORTAL
        | INCOMPLETE_CLASS_COMPONENT => {
            // Nothing to do for these component types
        }
        VIEW_TRANSITION_COMPONENT if ENABLE_VIEW_TRANSITION => {
            if is_view_transition_eligible {
                match &current {
                    None => {
                        // This is a new mount. We should have handled this as part of the
                        // Placement effect or it is deeper inside a entering transition.
                    }
                    Some(current) => {
                        // Something may have mutated within this subtree. This might need to cause
                        // a cross-fade of this parent. We first assign old names to the
                        // previous tree in the before mutation phase in case we need to.
                        // TODO: This walks the tree that we might continue walking anyway.
                        // We should just stash the parent ViewTransitionComponent and continue
                        // walking the tree until we find HostComponent but to do that we need
                        // to use a stack which requires refactoring this phase.
                        commit_before_update_view_transition(current, finished_work);
                    }
                }
            }
        }
        // Fallthrough
        _ => {
            if (flags & SNAPSHOT) != NO_FLAGS {
                panic!(concat!(
                    "This unit of work tag should not have side-effects. This error is ",
                    "likely caused by a bug in React. Please file an issue."
                ));
            }
        }
    }
}

fn commit_before_mutation_effects_deletion(
    deletion: &Rc<Fiber>,
    is_view_transition_eligible: bool,
) {
    if ENABLE_CREATE_EVENT_HANDLE_API {
        // TODO (effects) It would be nice to avoid calling doesFiberContain()
        // Maybe we can repurpose one of the subtreeFlags positions for this instead?
        // Use it to store which part of the tree the focused instance is in?
        // This assumes we can safely determine that instance during the "render" phase.
        // doesFiberContain of no focused instance handle is false.
        if FOCUSED_INSTANCE_HANDLE
            .with_borrow(Clone::clone)
            .is_some_and(|focused_instance_handle| {
                does_fiber_contain(deletion, &focused_instance_handle)
            })
        {
            SHOULD_FIRE_AFTER_ACTIVE_INSTANCE_BLUR.set(true);
            before_active_instance_blur(deletion);
        }
    }
    if is_view_transition_eligible {
        commit_exit_view_transitions(deletion);
    }
}

fn commit_layout_effect_on_fiber(
    finished_root: &FiberRoot,
    current: Option<&Rc<Fiber>>,
    finished_work: &Rc<Fiber>,
    committed_lanes: Lanes,
) {
    let prev_effect_start = push_component_effect_start();
    let prev_effect_duration = push_component_effect_duration();
    let prev_effect_errors = push_component_effect_errors();
    let prev_effect_did_spawn_update = push_component_effect_did_spawn_update();
    // When updating this function, also update reappearLayoutEffects, which does
    // most of the same things when an offscreen tree goes from hidden -> visible.
    let flags = finished_work.flags.get();
    match finished_work.tag.get() {
        FUNCTION_COMPONENT | FORWARD_REF | SIMPLE_MEMO_COMPONENT => {
            recursively_traverse_layout_effects(finished_root, finished_work, committed_lanes);
            if (flags & UPDATE) != 0 {
                commit_hook_layout_effects(finished_work, HOOK_LAYOUT | HOOK_HAS_EFFECT);
            }
        }
        CLASS_COMPONENT => {
            recursively_traverse_layout_effects(finished_root, finished_work, committed_lanes);
            if (flags & UPDATE) != 0 {
                commit_class_layout_lifecycles(finished_work, current.map(Rc::as_ref));
            }

            if (flags & CALLBACK) != 0 {
                commit_class_callbacks(finished_work);
            }

            if (flags & REF) != 0 {
                let return_fiber = finished_work.r#return.borrow().clone();
                safely_attach_ref(finished_work, return_fiber.as_ref());
            }
        }
        HOST_ROOT => {
            let prev_profiler_effect_duration = push_nested_effect_durations();
            recursively_traverse_layout_effects(finished_root, finished_work, committed_lanes);
            if (flags & CALLBACK) != 0 {
                commit_root_callbacks(finished_work);
            }
            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                && ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
            {
                finished_root.effect_duration.set(Some(
                    finished_root.effect_duration.get().unwrap()
                        + pop_nested_effect_durations(prev_profiler_effect_duration),
                ));
            }
        }
        HOST_SINGLETON | HOST_HOISTABLE | HOST_COMPONENT => {
            if finished_work.tag.get() == HOST_SINGLETON && SUPPORTS_SINGLETONS {
                // We acquire the singleton instance first so it has appropriate
                // styles before other layout effects run. This isn't perfect because
                // an early sibling of the singleton may have an effect that can
                // observe the singleton before it is acquired.
                // @TODO move this to the mutation phase. The reason it isn't there yet
                // is it seemingly requires an extra traversal because we need to move the
                // disappear effect into a phase before the appear phase
                if current.is_none() && (flags & UPDATE) != 0 {
                    // Unlike in the reappear path we only acquire on new mount
                    commit_host_singleton_acquisition(finished_work);
                }
                // We fall through to the HostComponent case below.
            }
            // Fallthrough
            recursively_traverse_layout_effects(finished_root, finished_work, committed_lanes);

            // Renderers may schedule work to be done after host components are mounted
            // (eg DOM renderer may schedule auto-focus for inputs and form controls).
            // These effects should only be committed when components are first mounted,
            // aka when there is no current/alternate.
            if current.is_none() {
                if (flags & UPDATE) != 0 {
                    commit_host_mount(finished_work);
                } else if (flags & HYDRATE) != 0 {
                    commit_host_hydrated_instance(finished_work);
                }
            }

            if (flags & REF) != 0 {
                let return_fiber = finished_work.r#return.borrow().clone();
                safely_attach_ref(finished_work, return_fiber.as_ref());
            }
        }
        PROFILER => {
            // TODO: Should this fire inside an offscreen tree? Or should it wait to
            // fire when the tree becomes visible again.
            if (flags & UPDATE) != 0 {
                let prev_profiler_effect_duration = push_nested_effect_durations();

                recursively_traverse_layout_effects(finished_root, finished_work, committed_lanes);

                let profiler_instance: &ProfilerStateNode =
                    unsafe { js::cast(finished_work.state_node.get()) };

                if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                    && ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
                {
                    // Propagate layout effect durations to the next nearest Profiler ancestor.
                    // Do not reset these values until the next render so DevTools has a chance to read them first.
                    profiler_instance.effect_duration.set(
                        profiler_instance.effect_duration.get()
                            + bubble_nested_effect_durations(prev_profiler_effect_duration),
                    );
                }

                commit_profiler_update(
                    finished_work,
                    current.map(Rc::as_ref),
                    COMMIT_START_TIME.get(),
                    profiler_instance.effect_duration.get(),
                );
            } else {
                recursively_traverse_layout_effects(finished_root, finished_work, committed_lanes);
            }
        }
        ACTIVITY_COMPONENT => {
            recursively_traverse_layout_effects(finished_root, finished_work, committed_lanes);
            if (flags & UPDATE) != 0 {
                commit_activity_hydration_callbacks(finished_root, finished_work);
            }
        }
        SUSPENSE_COMPONENT => {
            recursively_traverse_layout_effects(finished_root, finished_work, committed_lanes);
            if (flags & UPDATE) != 0 {
                commit_suspense_hydration_callbacks(finished_root, finished_work);
            }
            if (flags & CALLBACK) != 0 {
                // This Boundary is in fallback and has a dehydrated Suspense instance.
                // We could in theory assume the dehydrated state but we recheck it for
                // certainty.
                let finished_state: Option<&SuspenseState> =
                    unsafe { js::cast(finished_work.memoized_state.get()) };
                if let Some(finished_state) = finished_state {
                    let dehydrated = finished_state.dehydrated;
                    if let Some(dehydrated) = dehydrated {
                        // Register a callback to retry this boundary once the server has sent the result.
                        let retry = function::bind(
                            retry_dehydrated_suspense_boundary,
                            Some(&Kind::Null),
                            (Rc::clone(finished_work),),
                        );
                        register_suspense_instance_retry(dehydrated, retry);
                    }
                }
            }
        }
        OFFSCREEN_COMPONENT => {
            let is_modern_root =
                DISABLE_LEGACY_MODE || (finished_work.mode.get() & CONCURRENT_MODE) != NO_MODE;
            if is_modern_root {
                let is_hidden = finished_work.memoized_state.get().is_some();
                let new_offscreen_subtree_is_hidden =
                    is_hidden || OFFSCREEN_SUBTREE_IS_HIDDEN.get();
                if new_offscreen_subtree_is_hidden {
                    // The Offscreen tree is hidden. Skip over its layout effects.
                } else {
                    // The Offscreen tree is visible.

                    let was_hidden = current
                        .as_ref()
                        .is_some_and(|current| current.memoized_state.get().is_some());
                    let new_offscreen_subtree_was_hidden =
                        was_hidden || OFFSCREEN_SUBTREE_WAS_HIDDEN.get();
                    let prev_offscreen_subtree_is_hidden = OFFSCREEN_SUBTREE_IS_HIDDEN.get();
                    let prev_offscreen_subtree_was_hidden = OFFSCREEN_SUBTREE_WAS_HIDDEN.get();
                    OFFSCREEN_SUBTREE_IS_HIDDEN.set(new_offscreen_subtree_is_hidden);
                    OFFSCREEN_SUBTREE_WAS_HIDDEN.set(new_offscreen_subtree_was_hidden);

                    if OFFSCREEN_SUBTREE_WAS_HIDDEN.get() && !prev_offscreen_subtree_was_hidden {
                        // This is the root of a reappearing boundary. As we continue
                        // traversing the layout effects, we must also re-mount layout
                        // effects that were unmounted when the Offscreen subtree was
                        // hidden. So this is a superset of the normal commitLayoutEffects.
                        let mut layout_effect_traversal_flags: LayoutEffectTraversalFlags;
                        if SUPPORTS_SINGLETONS {
                            layout_effect_traversal_flags = INCLUDE_HOST_SINGLETONS;
                        } else {
                            layout_effect_traversal_flags = NO_LAYOUT_EFFECT_TRAVERSAL_FLAGS;
                        }
                        if (finished_work.subtree_flags.get() & LAYOUT_MASK) != NO_FLAGS {
                            layout_effect_traversal_flags |= INCLUDE_WORK_IN_PROGRESS_EFFECTS;
                        }
                        recursively_traverse_reappear_layout_effects(
                            finished_root,
                            finished_work,
                            layout_effect_traversal_flags,
                        );
                        if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                            && ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
                            && ENABLE_COMPONENT_PERFORMANCE_TRACK
                            && (finished_work.mode.get() & PROFILE_MODE) != NO_MODE
                            && COMPONENT_EFFECT_START_TIME.get() >= 0.0
                            && COMPONENT_EFFECT_END_TIME.get() >= 0.0
                            && COMPONENT_EFFECT_END_TIME.get() - COMPONENT_EFFECT_START_TIME.get()
                                > 0.05
                        {
                            log_component_reappeared(
                                finished_work,
                                COMPONENT_EFFECT_START_TIME.get(),
                                COMPONENT_EFFECT_END_TIME.get(),
                            );
                        }
                    } else {
                        recursively_traverse_layout_effects(
                            finished_root,
                            finished_work,
                            committed_lanes,
                        );
                    }
                    OFFSCREEN_SUBTREE_IS_HIDDEN.set(prev_offscreen_subtree_is_hidden);
                    OFFSCREEN_SUBTREE_WAS_HIDDEN.set(prev_offscreen_subtree_was_hidden);
                }
            } else {
                recursively_traverse_layout_effects(finished_root, finished_work, committed_lanes);
            }
        }
        VIEW_TRANSITION_COMPONENT => {
            if ENABLE_VIEW_TRANSITION {
                if __DEV__ && (flags & VIEW_TRANSITION_NAMED_STATIC) != 0 {
                    track_named_view_transition(finished_work);
                }
                recursively_traverse_layout_effects(finished_root, finished_work, committed_lanes);
                if (flags & REF) != 0 {
                    let return_fiber = finished_work.r#return.borrow().clone();
                    safely_attach_ref(finished_work, return_fiber.as_ref());
                }
            }
        }
        _ => {
            if finished_work.tag.get() == FRAGMENT && (flags & REF) != 0 {
                let return_fiber = finished_work.r#return.borrow().clone();
                safely_attach_ref(finished_work, return_fiber.as_ref());
            }
            // Fallthrough
            recursively_traverse_layout_effects(finished_root, finished_work, committed_lanes);
        }
    }

    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        && ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
        && ENABLE_COMPONENT_PERFORMANCE_TRACK
        && (finished_work.mode.get() & PROFILE_MODE) != NO_MODE
        && COMPONENT_EFFECT_START_TIME.get() >= 0.0
        && COMPONENT_EFFECT_END_TIME.get() >= 0.0
    {
        if COMPONENT_EFFECT_SPAWNED_UPDATE.get() || COMPONENT_EFFECT_DURATION.get() > 0.05 {
            log_component_effect(
                finished_work,
                COMPONENT_EFFECT_START_TIME.get(),
                COMPONENT_EFFECT_END_TIME.get(),
                COMPONENT_EFFECT_DURATION.get(),
                COMPONENT_EFFECT_ERRORS.with_borrow(Clone::clone).as_deref(),
            );
        }
        let return_fiber = finished_work.r#return.borrow().clone();
        let return_alternate = return_fiber
            .as_ref()
            .and_then(|return_fiber| return_fiber.alternate.borrow().clone());
        if
        // Insertion
        finished_work.alternate.borrow().is_none()
            && let Some(return_fiber) = return_fiber
            && let Some(return_alternate) = return_alternate
            && COMPONENT_EFFECT_END_TIME.get() - COMPONENT_EFFECT_START_TIME.get() > 0.05
        {
            let is_hydration = is_hydrating_parent(&return_alternate, &return_fiber);
            if !is_hydration {
                log_component_mount(
                    finished_work,
                    COMPONENT_EFFECT_START_TIME.get(),
                    COMPONENT_EFFECT_END_TIME.get(),
                );
            }
        }
    }

    pop_component_effect_start(prev_effect_start);
    pop_component_effect_duration(prev_effect_duration);
    pop_component_effect_errors(prev_effect_errors);
    pop_component_effect_did_spawn_update(prev_effect_did_spawn_update);
}

fn abort_root_transitions(
    root: &FiberRoot,
    abort: &Rc<TransitionAbort>,
    deleted_transitions: &Set<Transition>,
    deleted_offscreen_instance: Option<&'static OffscreenInstance>,
    _is_in_deleted_tree: bool,
) {
    if ENABLE_TRANSITION_TRACING {
        let root_transitions = root.incomplete_transitions;
        deleted_transitions.for_each(|transition| {
            if root_transitions.has(transition) {
                let transition_instance = root_transitions.get(transition).unwrap();
                if transition_instance.aborts.borrow().is_none() {
                    transition_instance
                        .aborts
                        .replace(Some(Rc::new(RefCell::new(Vec::new()))));
                }
                let aborts = transition_instance.aborts.borrow().clone().unwrap();
                aborts.borrow_mut().push(Rc::clone(abort));

                if let Some(deleted_offscreen_instance) = deleted_offscreen_instance
                    && let Some(pending_boundaries) = transition_instance.pending_boundaries.get()
                    && pending_boundaries.has(deleted_offscreen_instance)
                {
                    pending_boundaries.delete(deleted_offscreen_instance);
                }
            }
        });
    }
}

fn abort_tracing_marker_transitions(
    aborted_fiber: &Fiber,
    abort: &Rc<TransitionAbort>,
    deleted_transitions: &'static Set<Transition>,
    deleted_offscreen_instance: Option<&'static OffscreenInstance>,
    is_in_deleted_tree: bool,
) {
    if ENABLE_TRANSITION_TRACING {
        let marker_instance: &TracingMarkerInstance =
            unsafe { js::cast(aborted_fiber.state_node.get()) };
        let marker_transitions = marker_instance.transitions.get();
        let pending_boundaries = marker_instance.pending_boundaries.get();
        if let Some(marker_transitions) = marker_transitions {
            // TODO: Refactor this code. Is there a way to move this code to
            // the deletions phase instead of calculating it here while making sure
            // complete is called appropriately?
            deleted_transitions.for_each(|transition| {
                // If one of the transitions on the tracing marker is a transition
                // that was in an aborted subtree, we will abort that tracing marker
                if marker_transitions.has(transition)
                    && marker_instance
                        .aborts
                        .borrow()
                        .as_ref()
                        .is_none_or(|aborts| {
                            !aborts
                                .borrow()
                                .iter()
                                .any(|marker_abort| Rc::ptr_eq(marker_abort, abort))
                        })
                    && let Some(transitions) = marker_instance.transitions.get()
                {
                    let aborts = marker_instance.aborts.borrow().clone();
                    match aborts {
                        None => {
                            let aborts: TransitionAborts =
                                Rc::new(RefCell::new(vec![Rc::clone(abort)]));
                            marker_instance.aborts.replace(Some(Rc::clone(&aborts)));
                            add_marker_incomplete_callback_to_pending_transition(
                                unsafe {
                                    js::cast(js::get(aborted_fiber.memoized_props.get(), "name"))
                                },
                                transitions,
                                &aborts,
                            );
                        }
                        Some(aborts) => aborts.borrow_mut().push(Rc::clone(abort)),
                    }

                    // We only want to call onTransitionProgress when the marker hasn't been
                    // deleted
                    if let Some(deleted_offscreen_instance) = deleted_offscreen_instance
                        && !is_in_deleted_tree
                        && let Some(pending_boundaries) = pending_boundaries
                        && pending_boundaries.has(deleted_offscreen_instance)
                    {
                        pending_boundaries.delete(deleted_offscreen_instance);

                        add_marker_progress_callback_to_pending_transition(
                            unsafe {
                                js::cast(js::get(aborted_fiber.memoized_props.get(), "name"))
                            },
                            deleted_transitions,
                            pending_boundaries,
                        );
                    }
                }
            });
        }
    }
}

fn abort_parent_marker_transitions_for_deleted_fiber(
    aborted_fiber: &Rc<Fiber>,
    abort: &Rc<TransitionAbort>,
    deleted_transitions: &'static Set<Transition>,
    deleted_offscreen_instance: Option<&'static OffscreenInstance>,
    is_in_deleted_tree: bool,
) {
    if ENABLE_TRANSITION_TRACING {
        // Find all pending markers that are waiting on child suspense boundaries in the
        // aborted subtree and cancels them
        let mut fiber = Some(Rc::clone(aborted_fiber));
        while let Some(current) = fiber {
            match current.tag.get() {
                TRACING_MARKER_COMPONENT => {
                    abort_tracing_marker_transitions(
                        &current,
                        abort,
                        deleted_transitions,
                        deleted_offscreen_instance,
                        is_in_deleted_tree,
                    );
                }
                HOST_ROOT => {
                    let root: &FiberRoot = unsafe { js::cast(current.state_node.get()) };
                    abort_root_transitions(
                        root,
                        abort,
                        deleted_transitions,
                        deleted_offscreen_instance,
                        is_in_deleted_tree,
                    );
                }
                _ => {}
            }

            fiber = current.r#return.borrow().clone();
        }
    }
}

fn commit_transition_progress(offscreen_fiber: &Fiber) {
    if ENABLE_TRANSITION_TRACING {
        // This function adds suspense boundaries to the root
        // or tracing marker's pendingBoundaries map.
        // When a suspense boundary goes from a resolved to a fallback
        // state we add the boundary to the map, and when it goes from
        // a fallback to a resolved state, we remove the boundary from
        // the map.

        // We use stateNode on the Offscreen component as a stable object
        // that doesnt change from render to render. This way we can
        // distinguish between different Offscreen instances (vs. the same
        // Offscreen instance with different fibers)
        let offscreen_instance: &'static OffscreenInstance =
            unsafe { js::cast(offscreen_fiber.state_node.get()) };

        let mut prev_state: Option<&SuspenseState> = js::null();
        let previous_fiber = offscreen_fiber.alternate.borrow().clone();
        if let Some(previous_fiber) = previous_fiber
            && previous_fiber.memoized_state.get().is_some()
        {
            prev_state = unsafe { js::cast(previous_fiber.memoized_state.get()) };
        }
        let next_state: Option<&SuspenseState> =
            unsafe { js::cast(offscreen_fiber.memoized_state.get()) };

        let was_hidden = prev_state.is_some();
        let is_hidden = next_state.is_some();

        let pending_markers = offscreen_instance.pending_markers.get();
        // If there is a name on the suspense boundary, store that in
        // the pending boundaries.
        let mut name: Option<String> = js::null();
        let parent = offscreen_fiber.r#return.borrow().clone();
        if let Some(parent) = parent
            && parent.tag.get() == SUSPENSE_COMPONENT
            && js::truthy(js::get(parent.memoized_props.get(), "name"))
        {
            name = unsafe { js::cast(js::get(parent.memoized_props.get(), "name")) };
        }

        if !was_hidden && is_hidden {
            // The suspense boundaries was just hidden. Add the boundary
            // to the pending boundary set if it's there
            if let Some(pending_markers) = pending_markers {
                pending_markers.for_each(|marker_instance| {
                    let pending_boundaries = marker_instance.pending_boundaries.get();
                    let transitions = marker_instance.transitions.get();
                    let marker_name = marker_instance.name.borrow().clone();
                    if let Some(pending_boundaries) = pending_boundaries
                        && !pending_boundaries.has(offscreen_instance)
                    {
                        pending_boundaries
                            .set(offscreen_instance, SuspenseInfo { name: name.clone() });
                        if let Some(transitions) = transitions {
                            if marker_instance.tag == Some(TRANSITION_TRACING_MARKER)
                                && let Some(marker_name) = &marker_name
                            {
                                add_marker_progress_callback_to_pending_transition(
                                    marker_name,
                                    transitions,
                                    pending_boundaries,
                                );
                            } else if marker_instance.tag == Some(TRANSITION_ROOT) {
                                transitions.for_each(|transition| {
                                    add_transition_progress_callback_to_pending_transition(
                                        transition,
                                        pending_boundaries,
                                    );
                                });
                            }
                        }
                    }
                });
            }
        } else if was_hidden && !is_hidden {
            // The suspense boundary went from hidden to visible. Remove
            // the boundary from the pending suspense boundaries set
            // if it's there
            if let Some(pending_markers) = pending_markers {
                pending_markers.for_each(|marker_instance| {
                    let pending_boundaries = marker_instance.pending_boundaries.get();
                    let transitions = marker_instance.transitions.get();
                    let marker_name = marker_instance.name.borrow().clone();
                    if let Some(pending_boundaries) = pending_boundaries
                        && pending_boundaries.has(offscreen_instance)
                    {
                        pending_boundaries.delete(offscreen_instance);
                        if let Some(transitions) = transitions {
                            if marker_instance.tag == Some(TRANSITION_TRACING_MARKER)
                                && let Some(marker_name) = &marker_name
                            {
                                add_marker_progress_callback_to_pending_transition(
                                    marker_name,
                                    transitions,
                                    pending_boundaries,
                                );

                                // If there are no more unresolved suspense boundaries, the interaction
                                // is considered finished
                                if pending_boundaries.size() == 0 {
                                    if marker_instance.aborts.borrow().is_none() {
                                        add_marker_complete_callback_to_pending_transition(
                                            marker_name,
                                            transitions,
                                        );
                                    }
                                    marker_instance.transitions.set(js::null());
                                    marker_instance.pending_boundaries.set(js::null());
                                    marker_instance.aborts.replace(js::null());
                                }
                            } else if marker_instance.tag == Some(TRANSITION_ROOT) {
                                transitions.for_each(|transition| {
                                    add_transition_progress_callback_to_pending_transition(
                                        transition,
                                        pending_boundaries,
                                    );
                                });
                            }
                        }
                    }
                });
            }
        }
    }
}

fn hide_or_unhide_all_children(parent_fiber: &Fiber, is_hidden: bool) {
    if !SUPPORTS_MUTATION {
        return;
    }
    // Finds the nearest host component children and updates their visibility
    // to either hidden or visible.
    let mut child = parent_fiber.child.borrow().clone();
    while let Some(current) = child {
        hide_or_unhide_all_children_on_fiber(&current, is_hidden);
        child = current.sibling.borrow().clone();
    }
}

fn hide_or_unhide_all_children_on_fiber(fiber: &Rc<Fiber>, is_hidden: bool) {
    if !SUPPORTS_MUTATION {
        return;
    }
    match fiber.tag.get() {
        HOST_COMPONENT | HOST_HOISTABLE => {
            // Found the nearest host component. Hide it.
            commit_show_hide_host_instance(fiber, is_hidden);
            // Typically, only the nearest host nodes need to be hidden, since that
            // has the effect of also hiding everything inside of them.
            //
            // However, there's a special case for portals, because portals do not
            // exist in the regular host tree hierarchy; we can't assume that just
            // because a portal's HostComponent parent in the React tree will also be
            // a parent in the actual host tree.
            //
            // So, if any portals exist within the tree, regardless of how deeply
            // nested they are, we need to repeat this algorithm for its children.
            hide_or_unhide_nearest_portals(fiber, is_hidden);
        }
        HOST_TEXT => {
            commit_show_hide_host_text_instance(fiber, is_hidden);
        }
        DEHYDRATED_FRAGMENT => {
            commit_show_hide_suspense_boundary(fiber, is_hidden);
        }
        OFFSCREEN_COMPONENT | LEGACY_HIDDEN_COMPONENT => {
            let offscreen_state: Option<&OffscreenState> =
                unsafe { js::cast(fiber.memoized_state.get()) };
            if offscreen_state.is_some() {
                // Found a nested Offscreen component that is hidden.
                // Don't search any deeper. This tree should remain hidden.
            } else {
                hide_or_unhide_all_children(fiber, is_hidden);
            }
        }
        _ => {
            hide_or_unhide_all_children(fiber, is_hidden);
        }
    }
}

fn hide_or_unhide_nearest_portals(parent_fiber: &Fiber, is_hidden: bool) {
    if !SUPPORTS_MUTATION {
        return;
    }
    if (parent_fiber.subtree_flags.get() & PORTAL_STATIC) != 0 {
        let mut child = parent_fiber.child.borrow().clone();
        while let Some(current) = child {
            hide_or_unhide_nearest_portals_on_fiber(&current, is_hidden);
            child = current.sibling.borrow().clone();
        }
    }
}

fn hide_or_unhide_nearest_portals_on_fiber(fiber: &Rc<Fiber>, is_hidden: bool) {
    if !SUPPORTS_MUTATION {
        return;
    }
    match fiber.tag.get() {
        HOST_PORTAL => {
            // Found a portal. Switch back to the normal hide/unhide algorithm to
            // toggle the visibility of its children.
            hide_or_unhide_all_children_on_fiber(fiber, is_hidden);
        }
        OFFSCREEN_COMPONENT => {
            let offscreen_state: Option<&OffscreenState> =
                unsafe { js::cast(fiber.memoized_state.get()) };
            if offscreen_state.is_some() {
                // Found a nested Offscreen component that is hidden. Don't search any
                // deeper. This tree should remain hidden.
            } else {
                hide_or_unhide_nearest_portals(fiber, is_hidden);
            }
        }
        _ => {
            hide_or_unhide_nearest_portals(fiber, is_hidden);
        }
    }
}

fn detach_fiber_mutation(fiber: &Fiber) {
    // Cut off the return pointer to disconnect it from the tree.
    // This enables us to detect and warn against state updates on an unmounted component.
    // It also prevents events from bubbling from within disconnected components.
    //
    // Ideally, we should also clear the child pointer of the parent alternate to let this
    // get GC:ed but we don't know which for sure which parent is the current
    // one so we'll settle for GC:ing the subtree of this child.
    // This child itself will be GC:ed when the parent updates the next time.
    //
    // Note that we can't clear child or sibling pointers yet.
    // They're needed for passive effects and for findDOMNode.
    // We defer those fields, and all other cleanup, to the passive phase (see detachFiberAfterEffects).
    //
    // Don't reset the alternate yet, either. We need that so we can detach the
    // alternate's fields in the passive phase. Clearing the return pointer is
    // sufficient for findDOMNode semantics.
    let alternate = fiber.alternate.borrow().clone();
    if let Some(alternate) = alternate {
        alternate.r#return.replace(js::null());
    }
    fiber.r#return.replace(js::null());
}

fn detach_fiber_after_effects(fiber: &Fiber) {
    let alternate = fiber.alternate.borrow().clone();
    if let Some(alternate) = alternate {
        fiber.alternate.replace(js::null());
        detach_fiber_after_effects(&alternate);
    }

    // Clear cyclical Fiber fields. This level alone is designed to roughly
    // approximate the planned Fiber refactor. In that world, `setState` will be
    // bound to a special "instance" object instead of a Fiber. The Instance
    // object will not have any of these fields. It will only be connected to
    // the fiber tree via a single link at the root. So if this level alone is
    // sufficient to fix memory issues, that bodes well for our plans.
    fiber.child.replace(js::null());
    fiber.deletions.replace(js::null());
    fiber.sibling.replace(js::null());

    // The `stateNode` is cyclical because on host nodes it points to the host
    // tree, which has its own pointers to children, parents, and siblings.
    // The other host nodes also point back to fibers, so we should detach that
    // one, too.
    if fiber.tag.get() == HOST_COMPONENT {
        let host_instance = fiber.state_node.get();
        if let Some(host_instance) = host_instance {
            detach_deleted_instance(host_instance);
        }
    }
    fiber.state_node.set(js::null());

    if __DEV__ {
        fiber.debug_owner.set(js::null());
    }

    // Theoretically, nothing in here should be necessary, because we already
    // disconnected the fiber from the tree. So even if something leaks this
    // particular fiber, it won't leak anything else.
    fiber.r#return.replace(js::null());
    fiber.dependencies.replace(js::null());
    fiber.memoized_props.set(js::null());
    fiber.memoized_state.set(js::null());
    fiber.pending_props.set(js::null());
    fiber.state_node.set(js::null());
    // TODO: Move to `commitPassiveUnmountInsideDeletedTreeOnFiber` instead.
    fiber.update_queue.set(js::null());
}

thread_local! {
    // These are tracked on the stack as we recursively traverse a
    // deleted subtree.
    // TODO: Update these during the whole mutation phase, not just during
    // a deletion.
    // ReactFiberConfig's `Instance` or `Container`.
    #[cfg_attr(rust_js, rust_js::nullable)]
    static HOST_PARENT: Cell<Option<&'static Unknown>> = const { Cell::new(js::null()) };
    static HOST_PARENT_IS_CONTAINER: Cell<bool> = const { Cell::new(false) };
}

fn commit_deletion_effects(root: &FiberRoot, return_fiber: &Rc<Fiber>, deleted_fiber: &Rc<Fiber>) {
    let prev_effect_start = push_component_effect_start();

    if SUPPORTS_MUTATION {
        // We only have the top Fiber that was deleted but we need to recurse down its
        // children to find all the terminal nodes.

        // Recursively delete all host nodes from the parent, detach refs, clean
        // up mounted layout effects, and call componentWillUnmount.

        // We only need to remove the topmost host child in each branch. But then we
        // still need to keep traversing to unmount effects, refs, and cWU. TODO: We
        // could split this into two separate traversals functions, where the second
        // one doesn't include any removeChild logic. This is maybe the same
        // function as "disappearLayoutEffects" (or whatever that turns into after
        // the layout phase is refactored to use recursion).

        // Before starting, find the nearest host parent on the stack so we know
        // which instance/container to remove the children from.
        // TODO: Instead of searching up the fiber return path on every deletion, we
        // can track the nearest host component on the JS stack as we traverse the
        // tree during the commit phase. This would make insertions faster, too.
        let mut parent = Some(Rc::clone(return_fiber));
        'find_parent: while let Some(current) = parent {
            match current.tag.get() {
                HOST_SINGLETON if SUPPORTS_SINGLETONS => {
                    if is_singleton_scope(current.r#type.get()) {
                        HOST_PARENT.set(current.state_node.get());
                        HOST_PARENT_IS_CONTAINER.set(false);
                        break 'find_parent;
                    }
                }
                // Expected fallthrough when supportsSingletons is false
                HOST_SINGLETON | HOST_COMPONENT => {
                    HOST_PARENT.set(current.state_node.get());
                    HOST_PARENT_IS_CONTAINER.set(false);
                    break 'find_parent;
                }
                HOST_ROOT | HOST_PORTAL => {
                    HOST_PARENT.set(js::get(current.state_node.get(), "containerInfo"));
                    HOST_PARENT_IS_CONTAINER.set(true);
                    break 'find_parent;
                }
                _ => {}
            }
            parent = current.r#return.borrow().clone();
        }
        if HOST_PARENT.get().is_none() {
            panic!(concat!(
                "Expected to find a host parent. This error is likely caused by ",
                "a bug in React. Please file an issue."
            ));
        }

        commit_deletion_effects_on_fiber(root, return_fiber, deleted_fiber);
        HOST_PARENT.set(js::null());
        HOST_PARENT_IS_CONTAINER.set(false);
    } else {
        // Detach refs and call componentWillUnmount() on the whole subtree.
        commit_deletion_effects_on_fiber(root, return_fiber, deleted_fiber);
    }

    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        && ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
        && ENABLE_COMPONENT_PERFORMANCE_TRACK
        && (deleted_fiber.mode.get() & PROFILE_MODE) != NO_MODE
        && COMPONENT_EFFECT_START_TIME.get() >= 0.0
        && COMPONENT_EFFECT_END_TIME.get() >= 0.0
        && COMPONENT_EFFECT_END_TIME.get() - COMPONENT_EFFECT_START_TIME.get() > 0.05
    {
        log_component_unmount(
            deleted_fiber,
            COMPONENT_EFFECT_START_TIME.get(),
            COMPONENT_EFFECT_END_TIME.get(),
        );
    }
    pop_component_effect_start(prev_effect_start);

    detach_fiber_mutation(deleted_fiber);
}

fn recursively_traverse_deletion_effects(
    finished_root: &FiberRoot,
    nearest_mounted_ancestor: &Rc<Fiber>,
    parent: &Fiber,
) {
    // TODO: Use a static flag to skip trees that don't have unmount effects
    let mut child = parent.child.borrow().clone();
    while let Some(current) = child {
        commit_deletion_effects_on_fiber(finished_root, nearest_mounted_ancestor, &current);
        child = current.sibling.borrow().clone();
    }
}

fn commit_deletion_effects_on_fiber(
    finished_root: &FiberRoot,
    nearest_mounted_ancestor: &Rc<Fiber>,
    deleted_fiber: &Rc<Fiber>,
) {
    // TODO: Delete this Hook once new DevTools ships everywhere. No longer needed.
    on_commit_unmount(deleted_fiber);

    let prev_effect_start = push_component_effect_start();
    let prev_effect_duration = push_component_effect_duration();
    let prev_effect_errors = push_component_effect_errors();
    let prev_effect_did_spawn_update = push_component_effect_did_spawn_update();

    // The cases in this outer switch modify the stack before they traverse
    // into their subtree. There are simpler cases in the inner switch
    // that don't modify the stack.
    match deleted_fiber.tag.get() {
        HOST_HOISTABLE if SUPPORTS_RESOURCES => {
            if !OFFSCREEN_SUBTREE_WAS_HIDDEN.get() {
                safely_detach_ref(deleted_fiber, Some(nearest_mounted_ancestor));
            }
            recursively_traverse_deletion_effects(
                finished_root,
                nearest_mounted_ancestor,
                deleted_fiber,
            );
            if js::truthy(deleted_fiber.memoized_state.get()) {
                release_resource(deleted_fiber.memoized_state.get());
            } else if js::truthy(deleted_fiber.state_node.get()) {
                // A Hoistable Instance lives in document.head only when its enclosing
                // Activity is visible. If the Activity is hidden (or has been hidden
                // since mount), the instance was either never inserted or was
                // detached by the disappear traversal. Skip in those cases.
                if !OFFSCREEN_SUBTREE_WAS_HIDDEN.get() {
                    unmount_hoistable(deleted_fiber.state_node.get());
                }
            }
        }
        // Fall through
        HOST_HOISTABLE | HOST_SINGLETON if SUPPORTS_SINGLETONS => {
            if !OFFSCREEN_SUBTREE_WAS_HIDDEN.get() {
                safely_detach_ref(deleted_fiber, Some(nearest_mounted_ancestor));
            }
            commit_fragment_instance_deletion_effects(deleted_fiber);

            let prev_host_parent = HOST_PARENT.get();
            let prev_host_parent_is_container = HOST_PARENT_IS_CONTAINER.get();
            if is_singleton_scope(deleted_fiber.r#type.get()) {
                HOST_PARENT.set(deleted_fiber.state_node.get());
                HOST_PARENT_IS_CONTAINER.set(false);
            }
            recursively_traverse_deletion_effects(
                finished_root,
                nearest_mounted_ancestor,
                deleted_fiber,
            );

            // Normally this is called in passive unmount effect phase however with
            // HostSingleton we warn if you acquire one that is already associated to
            // a different fiber. To increase our chances of avoiding this, specifically
            // if you keyed a HostSingleton so there will be a delete followed by a Placement
            // we treat detach eagerly here
            commit_host_singleton_release(deleted_fiber);

            HOST_PARENT.set(prev_host_parent);
            HOST_PARENT_IS_CONTAINER.set(prev_host_parent_is_container);
        }
        // Fall through
        HOST_HOISTABLE | HOST_SINGLETON | HOST_COMPONENT | HOST_TEXT => {
            if deleted_fiber.tag.get() != HOST_TEXT {
                if !OFFSCREEN_SUBTREE_WAS_HIDDEN.get() {
                    safely_detach_ref(deleted_fiber, Some(nearest_mounted_ancestor));
                }
                commit_fragment_instance_deletion_effects(deleted_fiber);
                // Intentional fallthrough to next branch
            }
            // HostComponent falls through into this case.
            if deleted_fiber.tag.get() == HOST_TEXT {
                commit_fragment_instance_deletion_effects(deleted_fiber);
            }
            // We only need to remove the nearest host child. Set the host parent
            // to `null` on the stack to indicate that nested children don't
            // need to be removed.
            if SUPPORTS_MUTATION {
                let prev_host_parent = HOST_PARENT.get();
                let prev_host_parent_is_container = HOST_PARENT_IS_CONTAINER.get();
                HOST_PARENT.set(js::null());
                recursively_traverse_deletion_effects(
                    finished_root,
                    nearest_mounted_ancestor,
                    deleted_fiber,
                );
                HOST_PARENT.set(prev_host_parent);
                HOST_PARENT_IS_CONTAINER.set(prev_host_parent_is_container);

                if let Some(host_parent) = HOST_PARENT.get() {
                    // Now that all the child effects have unmounted, we can remove the
                    // node from the tree.
                    if HOST_PARENT_IS_CONTAINER.get() {
                        commit_host_remove_child_from_container(
                            deleted_fiber,
                            nearest_mounted_ancestor,
                            host_parent,
                            unsafe { js::cast(deleted_fiber.state_node.get()) },
                        );
                    } else {
                        commit_host_remove_child(
                            deleted_fiber,
                            nearest_mounted_ancestor,
                            host_parent,
                            unsafe { js::cast(deleted_fiber.state_node.get()) },
                        );
                    }
                }
            } else {
                recursively_traverse_deletion_effects(
                    finished_root,
                    nearest_mounted_ancestor,
                    deleted_fiber,
                );
            }
        }
        DEHYDRATED_FRAGMENT => {
            if ENABLE_SUSPENSE_CALLBACK {
                let hydration_callbacks = finished_root.hydration_callbacks.get();
                if let Some(hydration_callbacks) = hydration_callbacks
                    && let Err(error) = catch_unwind(AssertUnwindSafe(|| {
                        let on_deleted = hydration_callbacks.on_deleted;
                        if let Some(on_deleted) = on_deleted {
                            on_deleted(unsafe { js::cast(deleted_fiber.state_node.get()) });
                        }
                    }))
                    && let Some(error) = error.downcast_ref::<&'static JsError>()
                {
                    capture_commit_phase_error(
                        deleted_fiber,
                        Some(nearest_mounted_ancestor),
                        error.as_unknown(),
                    );
                }
            }

            // Dehydrated fragments don't have any children

            // Delete the dehydrated suspense boundary and all of its content.
            if SUPPORTS_MUTATION && let Some(host_parent) = HOST_PARENT.get() {
                if HOST_PARENT_IS_CONTAINER.get() {
                    clear_suspense_boundary_from_container(
                        host_parent,
                        deleted_fiber.state_node.get(),
                    );
                } else {
                    clear_suspense_boundary(host_parent, deleted_fiber.state_node.get());
                }
            }
        }
        HOST_PORTAL => {
            if SUPPORTS_MUTATION {
                // When we go into a portal, it becomes the parent to remove from.
                let prev_host_parent = HOST_PARENT.get();
                let prev_host_parent_is_container = HOST_PARENT_IS_CONTAINER.get();
                HOST_PARENT.set(js::get(deleted_fiber.state_node.get(), "containerInfo"));
                HOST_PARENT_IS_CONTAINER.set(true);
                recursively_traverse_deletion_effects(
                    finished_root,
                    nearest_mounted_ancestor,
                    deleted_fiber,
                );
                HOST_PARENT.set(prev_host_parent);
                HOST_PARENT_IS_CONTAINER.set(prev_host_parent_is_container);
            } else {
                if SUPPORTS_PERSISTENCE {
                    commit_host_portal_container_children(
                        unsafe { js::cast(deleted_fiber.state_node.get()) },
                        deleted_fiber,
                        Some(create_container_child_set()),
                    );
                }

                recursively_traverse_deletion_effects(
                    finished_root,
                    nearest_mounted_ancestor,
                    deleted_fiber,
                );
            }
        }
        FUNCTION_COMPONENT | FORWARD_REF | MEMO_COMPONENT | SIMPLE_MEMO_COMPONENT => {
            // TODO: Use a commitHookInsertionUnmountEffects wrapper to record timings.
            commit_hook_effect_list_unmount(
                HOOK_INSERTION,
                deleted_fiber,
                Some(nearest_mounted_ancestor),
            );
            if !OFFSCREEN_SUBTREE_WAS_HIDDEN.get() {
                commit_hook_layout_unmount_effects(
                    deleted_fiber,
                    Some(nearest_mounted_ancestor),
                    HOOK_LAYOUT,
                );
            }
            recursively_traverse_deletion_effects(
                finished_root,
                nearest_mounted_ancestor,
                deleted_fiber,
            );
        }
        CLASS_COMPONENT => {
            if !OFFSCREEN_SUBTREE_WAS_HIDDEN.get() {
                safely_detach_ref(deleted_fiber, Some(nearest_mounted_ancestor));
                let instance: &'static ClassInstance =
                    unsafe { js::cast(deleted_fiber.state_node.get()) };
                if js::type_of(js::get(instance, "componentWillUnmount")) == "function" {
                    safely_call_component_will_unmount(
                        deleted_fiber,
                        Some(nearest_mounted_ancestor),
                        instance,
                    );
                }
            }
            recursively_traverse_deletion_effects(
                finished_root,
                nearest_mounted_ancestor,
                deleted_fiber,
            );
        }
        SCOPE_COMPONENT => {
            if ENABLE_SCOPE_API && !OFFSCREEN_SUBTREE_WAS_HIDDEN.get() {
                safely_detach_ref(deleted_fiber, Some(nearest_mounted_ancestor));
            }
            recursively_traverse_deletion_effects(
                finished_root,
                nearest_mounted_ancestor,
                deleted_fiber,
            );
        }
        OFFSCREEN_COMPONENT => {
            if DISABLE_LEGACY_MODE || (deleted_fiber.mode.get() & CONCURRENT_MODE) != 0 {
                // If this offscreen component is hidden, we already unmounted it. Before
                // deleting the children, track that it's already unmounted so that we
                // don't attempt to unmount the effects again.
                // TODO: If the tree is hidden, in most cases we should be able to skip
                // over the nested children entirely. An exception is we haven't yet found
                // the topmost host node to delete, which we already track on the stack.
                // But the other case is portals, which need to be detached no matter how
                // deeply they are nested. We should use a subtree flag to track whether a
                // subtree includes a nested portal.
                let prev_offscreen_subtree_was_hidden = OFFSCREEN_SUBTREE_WAS_HIDDEN.get();
                OFFSCREEN_SUBTREE_WAS_HIDDEN.set(
                    prev_offscreen_subtree_was_hidden
                        || deleted_fiber.memoized_state.get().is_some(),
                );

                recursively_traverse_deletion_effects(
                    finished_root,
                    nearest_mounted_ancestor,
                    deleted_fiber,
                );
                OFFSCREEN_SUBTREE_WAS_HIDDEN.set(prev_offscreen_subtree_was_hidden);
            } else {
                recursively_traverse_deletion_effects(
                    finished_root,
                    nearest_mounted_ancestor,
                    deleted_fiber,
                );
            }
        }
        VIEW_TRANSITION_COMPONENT if ENABLE_VIEW_TRANSITION => {
            if __DEV__ && (deleted_fiber.flags.get() & VIEW_TRANSITION_NAMED_STATIC) != 0 {
                untrack_named_view_transition(deleted_fiber);
            }
            safely_detach_ref(deleted_fiber, Some(nearest_mounted_ancestor));
            recursively_traverse_deletion_effects(
                finished_root,
                nearest_mounted_ancestor,
                deleted_fiber,
            );
        }
        // Fallthrough
        VIEW_TRANSITION_COMPONENT | FRAGMENT => {
            if !OFFSCREEN_SUBTREE_WAS_HIDDEN.get() {
                safely_detach_ref(deleted_fiber, Some(nearest_mounted_ancestor));
            }
            recursively_traverse_deletion_effects(
                finished_root,
                nearest_mounted_ancestor,
                deleted_fiber,
            );
        }
        _ => {
            recursively_traverse_deletion_effects(
                finished_root,
                nearest_mounted_ancestor,
                deleted_fiber,
            );
        }
    }

    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        && ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
        && ENABLE_COMPONENT_PERFORMANCE_TRACK
        && (deleted_fiber.mode.get() & PROFILE_MODE) != NO_MODE
        && COMPONENT_EFFECT_START_TIME.get() >= 0.0
        && COMPONENT_EFFECT_END_TIME.get() >= 0.0
        && (COMPONENT_EFFECT_SPAWNED_UPDATE.get() || COMPONENT_EFFECT_DURATION.get() > 0.05)
    {
        log_component_effect(
            deleted_fiber,
            COMPONENT_EFFECT_START_TIME.get(),
            COMPONENT_EFFECT_END_TIME.get(),
            COMPONENT_EFFECT_DURATION.get(),
            COMPONENT_EFFECT_ERRORS.with_borrow(Clone::clone).as_deref(),
        );
    }

    pop_component_effect_start(prev_effect_start);
    pop_component_effect_duration(prev_effect_duration);
    pop_component_effect_errors(prev_effect_errors);
    pop_component_effect_did_spawn_update(prev_effect_did_spawn_update);
}

fn commit_suspense_callback(finished_work: &Fiber) {
    // TODO: Delete this feature. It's not properly covered by DEV features.
    let new_state: Option<&SuspenseState> = unsafe { js::cast(finished_work.memoized_state.get()) };
    if ENABLE_SUSPENSE_CALLBACK && new_state.is_some() {
        let suspense_callback = js::get(finished_work.memoized_props.get(), "suspenseCallback");
        if js::type_of(suspense_callback) == "function" {
            let retry_queue: Option<&RetryQueue> =
                unsafe { js::cast(finished_work.update_queue.get()) };
            if let Some(retry_queue) = retry_queue {
                let suspense_callback: &dyn Fn(&'static RetryQueue) =
                    unsafe { js::cast(suspense_callback) };
                suspense_callback(js::set::new(Some(retry_queue)));
            }
        } else if __DEV__ && !matches!(js::classify(suspense_callback), Kind::Undefined) {
            console::error(("Unexpected type for suspenseCallback.",));
        }
    }
}

fn commit_activity_hydration_callbacks(finished_root: &FiberRoot, finished_work: &Rc<Fiber>) {
    if !SUPPORTS_HYDRATION {
        return;
    }
    let new_state: Option<&ActivityState> = unsafe { js::cast(finished_work.memoized_state.get()) };
    if new_state.is_none() {
        let current = finished_work.alternate.borrow().clone();
        if let Some(current) = current {
            let prev_state: Option<&ActivityState> =
                unsafe { js::cast(current.memoized_state.get()) };
            if let Some(prev_state) = prev_state {
                let activity_instance = prev_state.dehydrated;
                commit_host_hydrated_activity(activity_instance, finished_work);
                if ENABLE_SUSPENSE_CALLBACK
                    && let Err(error) = catch_unwind(AssertUnwindSafe(|| {
                        // TODO: Delete this feature. It's not properly covered by DEV features.
                        let hydration_callbacks = finished_root.hydration_callbacks.get();
                        if let Some(hydration_callbacks) = hydration_callbacks {
                            let on_hydrated = hydration_callbacks.on_hydrated;
                            if let Some(on_hydrated) = on_hydrated {
                                on_hydrated(activity_instance);
                            }
                        }
                    }))
                    && let Some(error) = error.downcast_ref::<&'static JsError>()
                {
                    let return_fiber = finished_work.r#return.borrow().clone();
                    capture_commit_phase_error(
                        finished_work,
                        return_fiber.as_ref(),
                        error.as_unknown(),
                    );
                }
            }
        }
    }
}

fn commit_suspense_hydration_callbacks(finished_root: &FiberRoot, finished_work: &Rc<Fiber>) {
    if !SUPPORTS_HYDRATION {
        return;
    }
    let new_state: Option<&SuspenseState> = unsafe { js::cast(finished_work.memoized_state.get()) };
    if new_state.is_none() {
        let current = finished_work.alternate.borrow().clone();
        if let Some(current) = current {
            let prev_state: Option<&SuspenseState> =
                unsafe { js::cast(current.memoized_state.get()) };
            if let Some(prev_state) = prev_state {
                let suspense_instance = prev_state.dehydrated;
                if let Some(suspense_instance) = suspense_instance {
                    commit_host_hydrated_suspense(suspense_instance, finished_work);
                    if ENABLE_SUSPENSE_CALLBACK
                        && let Err(error) = catch_unwind(AssertUnwindSafe(|| {
                            // TODO: Delete this feature. It's not properly covered by DEV features.
                            let hydration_callbacks = finished_root.hydration_callbacks.get();
                            if let Some(hydration_callbacks) = hydration_callbacks {
                                let on_hydrated = hydration_callbacks.on_hydrated;
                                if let Some(on_hydrated) = on_hydrated {
                                    on_hydrated(suspense_instance);
                                }
                            }
                        }))
                        && let Some(error) = error.downcast_ref::<&'static JsError>()
                    {
                        let return_fiber = finished_work.r#return.borrow().clone();
                        capture_commit_phase_error(
                            finished_work,
                            return_fiber.as_ref(),
                            error.as_unknown(),
                        );
                    }
                }
            }
        }
    }
}

/// shared/ReactTypes' `Wakeable`s a boundary already listens to: a `WeakSet`
/// where the host has one, a `Set` where it hasn't.
type RetryCache = WeakSet<JsObject>;

fn get_retry_cache(finished_work: &Fiber) -> &'static RetryCache {
    // TODO: Unify the interface for the retry cache so we don't have to switch
    // on the tag like this.
    match finished_work.tag.get() {
        ACTIVITY_COMPONENT | SUSPENSE_COMPONENT | SUSPENSE_LIST_COMPONENT => {
            let mut retry_cache: Option<&'static RetryCache> =
                unsafe { js::cast(finished_work.state_node.get()) };
            if retry_cache.is_none() {
                let new_retry_cache: &'static RetryCache =
                    POSSIBLY_WEAK_SET.with(|class| class.call_static(js::weak_set::new, ()));
                finished_work
                    .state_node
                    .set(Some(js::unknown(new_retry_cache)));
                retry_cache = Some(new_retry_cache);
            }
            retry_cache.unwrap()
        }
        OFFSCREEN_COMPONENT => {
            let instance: &OffscreenInstance = unsafe { js::cast(finished_work.state_node.get()) };
            let mut retry_cache = instance.retry_cache.get();
            if retry_cache.is_none() {
                let new_retry_cache: &'static RetryCache =
                    POSSIBLY_WEAK_SET.with(|class| class.call_static(js::weak_set::new, ()));
                instance.retry_cache.set(Some(new_retry_cache));
                retry_cache = Some(new_retry_cache);
            }
            retry_cache.unwrap()
        }
        _ => {
            panic!(
                "Unexpected Suspense handler tag ({}). This is a bug in React.",
                finished_work.tag.get()
            );
        }
    }
}

fn attach_suspense_retry_listeners(finished_work: &Rc<Fiber>, wakeables: &RetryQueue) {
    // If this boundary just timed out, then it will have a set of wakeables.
    // For each wakeable, attach a listener so that when it resolves, React
    // attempts to re-render the boundary in the primary (pre-timeout) state.
    let retry_cache = get_retry_cache(finished_work);
    wakeables.for_each(|wakeable| {
        // Memoize using the boundary fiber to prevent redundant listeners.
        if !retry_cache.has(unsafe { js::cast(Some(wakeable)) }) {
            retry_cache.add(unsafe { js::cast(Some(wakeable)) });

            if ENABLE_UPDATER_TRACKING.with(|&enabled| enabled)
                && IS_DEV_TOOLS_PRESENT.with(|&present| present)
            {
                if let Some(in_progress_lanes) = IN_PROGRESS_LANES.get()
                    && let Some(in_progress_root) = IN_PROGRESS_ROOT.with_borrow(Clone::clone)
                {
                    // If we have pending work still, associate the original updaters with it.
                    restore_pending_updaters(&in_progress_root, in_progress_lanes);
                } else {
                    panic!("Expected finished root and lanes to be set. This is a bug in React.");
                }
            }

            let retry = function::bind(
                resolve_retry_wakeable,
                Some(&Kind::Null),
                (Rc::clone(finished_work), wakeable),
            );
            let wakeable: &Wakeable = unsafe { js::cast(Some(wakeable)) };
            wakeable.then(retry, retry);
        }
    });
}

// This function detects when a Suspense boundary goes from visible to hidden.
// It returns false if the boundary is already hidden.
// TODO: Use an effect tag.
fn is_suspense_boundary_being_hidden(current: Option<&Fiber>, finished_work: &Fiber) -> bool {
    if let Some(current) = current {
        let old_state: Option<&SuspenseState> = unsafe { js::cast(current.memoized_state.get()) };
        if old_state.is_none_or(|old_state| old_state.dehydrated.is_some()) {
            let new_state: Option<&SuspenseState> =
                unsafe { js::cast(finished_work.memoized_state.get()) };
            return new_state.is_some_and(|new_state| new_state.dehydrated.is_none());
        }
    }
    false
}

pub fn commit_mutation_effects(
    root: &Rc<FiberRoot>,
    finished_work: &Rc<Fiber>,
    committed_lanes: Lanes,
) {
    IN_PROGRESS_LANES.set(Some(committed_lanes));
    IN_PROGRESS_ROOT.set(Some(Rc::clone(root)));

    ROOT_VIEW_TRANSITION_AFFECTED.set(false);
    IN_UPDATE_VIEW_TRANSITION.set(false);

    reset_component_effect_timers();

    commit_mutation_effects_on_fiber(finished_work, root, committed_lanes);

    IN_PROGRESS_LANES.set(js::null());
    IN_PROGRESS_ROOT.set(js::null());
}

fn recursively_traverse_mutation_effects(root: &FiberRoot, parent_fiber: &Rc<Fiber>, lanes: Lanes) {
    // Deletions effects can be scheduled on any fiber type. They need to happen
    // before the children effects have fired.
    let deletions = parent_fiber.deletions.borrow().clone();
    if let Some(deletions) = &deletions {
        let mut i = 0;
        while i < deletions.borrow().len() {
            let child_to_delete = Rc::clone(&deletions.borrow()[i]);
            commit_deletion_effects(root, parent_fiber, &child_to_delete);
            i += 1;
        }
    }

    if (parent_fiber.subtree_flags.get() & (MUTATION_MASK | CLONED)) != 0 {
        let mut child = parent_fiber.child.borrow().clone();
        while let Some(current) = child {
            commit_mutation_effects_on_fiber(&current, root, lanes);
            child = current.sibling.borrow().clone();
        }
    }
}

thread_local! {
    // ReactFiberConfig's `HoistableRoot`.
    #[cfg_attr(rust_js, rust_js::nullable)]
    static CURRENT_HOISTABLE_ROOT: Cell<Option<&'static Unknown>> = const { Cell::new(js::null()) };
}

fn commit_mutation_effects_on_fiber(finished_work: &Rc<Fiber>, root: &FiberRoot, lanes: Lanes) {
    let prev_effect_start = push_component_effect_start();
    let prev_effect_duration = push_component_effect_duration();
    let prev_effect_errors = push_component_effect_errors();
    let prev_effect_did_spawn_update = push_component_effect_did_spawn_update();
    let current = finished_work.alternate.borrow().clone();
    let flags = finished_work.flags.get();

    // The effect flag should be checked *after* we refine the type of fiber,
    // because the fiber tag is more specific. An exception is any flag related
    // to reconciliation, because those can be set on all fiber types.
    match finished_work.tag.get() {
        FUNCTION_COMPONENT | FORWARD_REF | MEMO_COMPONENT | SIMPLE_MEMO_COMPONENT => {
            // Mutate event effect callbacks on the way down, before mutation effects.
            // This ensures that parent event effects are mutated before child effects.
            // This isn't a supported use case, so we can re-consider it,
            // but this was the behavior we originally shipped.
            if (flags & UPDATE) != 0 {
                let update_queue: Option<&FunctionComponentUpdateQueue> =
                    unsafe { js::cast(finished_work.update_queue.get()) };
                if let Some(update_queue) = update_queue
                    && let Some(event_payloads) = &*update_queue.events.borrow()
                {
                    for EventFunctionPayload { r#ref, next_impl } in event_payloads {
                        r#ref.r#impl.set(*next_impl);
                    }
                }
            }
            recursively_traverse_mutation_effects(root, finished_work, lanes);
            commit_reconciliation_effects(finished_work, lanes);

            if (flags & UPDATE) != 0 {
                let return_fiber = finished_work.r#return.borrow().clone();
                commit_hook_effect_list_unmount(
                    HOOK_INSERTION | HOOK_HAS_EFFECT,
                    finished_work,
                    return_fiber.as_ref(),
                );
                // TODO: Use a commitHookInsertionUnmountEffects wrapper to record timings.
                commit_hook_effect_list_mount(HOOK_INSERTION | HOOK_HAS_EFFECT, finished_work);
                let return_fiber = finished_work.r#return.borrow().clone();
                commit_hook_layout_unmount_effects(
                    finished_work,
                    return_fiber.as_ref(),
                    HOOK_LAYOUT | HOOK_HAS_EFFECT,
                );
            }
        }
        CLASS_COMPONENT => {
            recursively_traverse_mutation_effects(root, finished_work, lanes);
            commit_reconciliation_effects(finished_work, lanes);

            if (flags & REF) != 0
                && !OFFSCREEN_SUBTREE_WAS_HIDDEN.get()
                && let Some(current) = &current
            {
                let return_fiber = current.r#return.borrow().clone();
                safely_detach_ref(current, return_fiber.as_ref());
            }

            if (flags & CALLBACK) != 0 && OFFSCREEN_SUBTREE_IS_HIDDEN.get() {
                let update_queue: Option<&UpdateQueue> =
                    unsafe { js::cast(finished_work.update_queue.get()) };
                if let Some(update_queue) = update_queue {
                    defer_hidden_callbacks(update_queue);
                }
            }
        }
        HOST_HOISTABLE if SUPPORTS_RESOURCES => {
            // We cast because we always set the root at the React root and so it cannot be
            // null while we are processing mutation effects
            let hoistable_root: &'static Unknown =
                unsafe { js::cast(CURRENT_HOISTABLE_ROOT.get()) };
            recursively_traverse_mutation_effects(root, finished_work, lanes);
            commit_reconciliation_effects(finished_work, lanes);

            if (flags & REF) != 0
                && !OFFSCREEN_SUBTREE_WAS_HIDDEN.get()
                && let Some(current) = &current
            {
                let return_fiber = current.r#return.borrow().clone();
                safely_detach_ref(current, return_fiber.as_ref());
            }

            if (flags & UPDATE) != 0 {
                let current_resource = match &current {
                    Some(current) => current.memoized_state.get(),
                    None => js::null(),
                };
                let new_resource = finished_work.memoized_state.get();
                match &current {
                    None => {
                        // We are mounting a new HostHoistable Fiber. We fork the mount
                        // behavior based on whether this instance is a Hoistable Instance
                        // or a Hoistable Resource
                        match new_resource {
                            None => {
                                if finished_work.state_node.get().is_none() {
                                    // Initial mount. The instance has not been created yet, which
                                    // happens during hydration (createHoistableInstance is normally
                                    // called in beginWork's updateHostHoistable, but is skipped
                                    // when hydrating).
                                    if OFFSCREEN_SUBTREE_IS_HIDDEN.get() {
                                        // We're inside a hidden Activity boundary. Create the
                                        // instance off-document so we don't leak metadata into
                                        // the head. It will be mounted by the reappear path when
                                        // the Activity becomes visible.
                                        finished_work.state_node.set(Some(
                                            create_hoistable_instance(
                                                finished_work.r#type.get(),
                                                finished_work.memoized_props.get(),
                                                Some(root.container_info),
                                                finished_work,
                                            ),
                                        ));
                                    } else {
                                        finished_work.state_node.set(Some(hydrate_hoistable(
                                            hoistable_root,
                                            finished_work.r#type.get(),
                                            finished_work.memoized_props.get(),
                                            finished_work,
                                        )));
                                    }
                                } else if !OFFSCREEN_SUBTREE_IS_HIDDEN.get() {
                                    // The instance was created in beginWork. Only mount it into
                                    // the document if we're not inside a hidden Activity boundary.
                                    mount_hoistable(
                                        hoistable_root,
                                        finished_work.r#type.get(),
                                        finished_work.state_node.get(),
                                    );
                                }
                            }
                            Some(new_resource) => {
                                finished_work.state_node.set(Some(acquire_resource(
                                    hoistable_root,
                                    new_resource,
                                    finished_work.memoized_props.get(),
                                )));
                            }
                        }
                    }
                    Some(current) if !js::strict_equals(current_resource, new_resource) => {
                        // We are moving to or from Hoistable Resource, or between different Hoistable Resources
                        match current_resource {
                            None => {
                                // Transitioning from Instance to Resource. Only unmount when the
                                // Instance is currently mounted in the document; hidden Activity
                                // boundaries keep instances off-document or detach them before
                                // this update is processed.
                                let instance = current.state_node.get();
                                if instance.is_some() && !OFFSCREEN_SUBTREE_WAS_HIDDEN.get() {
                                    unmount_hoistable(instance);
                                }
                            }
                            Some(current_resource) => {
                                release_resource(Some(current_resource));
                            }
                        }
                        match new_resource {
                            None => {
                                // Transitioning to an Instance. Only mount if visible; hidden
                                // Activity boundaries will mount via the reappear path.
                                if !OFFSCREEN_SUBTREE_IS_HIDDEN.get() {
                                    mount_hoistable(
                                        hoistable_root,
                                        finished_work.r#type.get(),
                                        finished_work.state_node.get(),
                                    );
                                }
                            }
                            Some(new_resource) => {
                                acquire_resource(
                                    hoistable_root,
                                    new_resource,
                                    finished_work.memoized_props.get(),
                                );
                            }
                        }
                    }
                    Some(current) => {
                        if new_resource.is_none() && finished_work.state_node.get().is_some() {
                            commit_host_update(
                                finished_work,
                                finished_work.memoized_props.get(),
                                current.memoized_props.get(),
                            );
                        }
                    }
                }
            }
        }
        // Fall through
        HOST_HOISTABLE | HOST_SINGLETON if SUPPORTS_SINGLETONS => {
            recursively_traverse_mutation_effects(root, finished_work, lanes);
            commit_reconciliation_effects(finished_work, lanes);
            if (flags & REF) != 0
                && !OFFSCREEN_SUBTREE_WAS_HIDDEN.get()
                && let Some(current) = &current
            {
                let return_fiber = current.r#return.borrow().clone();
                safely_detach_ref(current, return_fiber.as_ref());
            }
            if let Some(current) = &current
                && (flags & UPDATE) != 0
            {
                let new_props = finished_work.memoized_props.get();
                let old_props = current.memoized_props.get();
                commit_host_update(finished_work, new_props, old_props);
            }
        }
        // Fall through
        HOST_HOISTABLE | HOST_SINGLETON | HOST_COMPONENT => {
            // We've hit a host component, so it's no longer a direct parent.
            let prev_offscreen_direct_parent_is_hidden = OFFSCREEN_DIRECT_PARENT_IS_HIDDEN.get();
            OFFSCREEN_DIRECT_PARENT_IS_HIDDEN.set(false);

            recursively_traverse_mutation_effects(root, finished_work, lanes);

            OFFSCREEN_DIRECT_PARENT_IS_HIDDEN.set(prev_offscreen_direct_parent_is_hidden);

            commit_reconciliation_effects(finished_work, lanes);

            if (flags & REF) != 0
                && !OFFSCREEN_SUBTREE_WAS_HIDDEN.get()
                && let Some(current) = &current
            {
                let return_fiber = current.r#return.borrow().clone();
                safely_detach_ref(current, return_fiber.as_ref());
            }
            if SUPPORTS_MUTATION {
                // TODO: ContentReset gets cleared by the children during the commit
                // phase. This is a refactor hazard because it means we must read
                // flags the flags after `commitReconciliationEffects` has already run;
                // the order matters. We should refactor so that ContentReset does not
                // rely on mutating the flag during commit. Like by setting a flag
                // during the render phase instead.
                if (finished_work.flags.get() & CONTENT_RESET) != 0 {
                    commit_host_reset_text_content(finished_work);
                }

                if (flags & UPDATE) != 0 {
                    let instance = finished_work.state_node.get();
                    if instance.is_some() {
                        // Commit the work prepared earlier.
                        // For hydration we reuse the update path but we treat the oldProps
                        // as the newProps. The updatePayload will contain the real change in
                        // this case.
                        let new_props = finished_work.memoized_props.get();
                        let old_props = match &current {
                            Some(current) => current.memoized_props.get(),
                            None => new_props,
                        };
                        commit_host_update(finished_work, new_props, old_props);
                    }
                }

                if (flags & FORM_RESET) != 0 {
                    NEEDS_FORM_RESET.set(true);
                    if __DEV__ && !js::strict_equals(finished_work.r#type.get(), "form") {
                        // Paranoid coding. In case we accidentally start using the
                        // FormReset bit for something else.
                        console::error((concat!(
                            "Unexpected host component type. Expected a form. This is a ",
                            "bug in React."
                        ),));
                    }
                }
            } else if SUPPORTS_PERSISTENCE
                && let Some(alternate) = finished_work.alternate.borrow().as_ref()
            {
                // `finishedWork.alternate.stateNode` is pointing to a stale shadow
                // node at this point, retaining it and its subtree. To reclaim
                // memory, point `alternate.stateNode` to new shadow node. This
                // prevents shadow node from staying in memory longer than it
                // needs to. The correct behaviour of this is checked by test in
                // React Native: ShadowNodeReferenceCounter-itest.js#L150
                alternate.state_node.set(finished_work.state_node.get());
            }
        }
        HOST_TEXT => {
            recursively_traverse_mutation_effects(root, finished_work, lanes);
            commit_reconciliation_effects(finished_work, lanes);

            if (flags & UPDATE) != 0 && SUPPORTS_MUTATION {
                if finished_work.state_node.get().is_none() {
                    panic!(concat!(
                        "This should have a text node initialized. This error is likely ",
                        "caused by a bug in React. Please file an issue."
                    ));
                }

                let new_text: &str = unsafe { js::cast(finished_work.memoized_props.get()) };
                // For hydration we reuse the update path but we treat the oldProps
                // as the newProps. The updatePayload will contain the real change in
                // this case.
                let old_text: &str = match &current {
                    Some(current) => unsafe { js::cast(current.memoized_props.get()) },
                    None => new_text,
                };

                commit_host_text_update(finished_work, new_text, old_text);
            }
        }
        HOST_ROOT => {
            let prev_profiler_effect_duration = push_nested_effect_durations();

            push_root_mutation_context();
            if SUPPORTS_RESOURCES {
                prepare_to_commit_hoistables();

                let previous_hoistable_root = CURRENT_HOISTABLE_ROOT.get();
                CURRENT_HOISTABLE_ROOT.set(Some(get_hoistable_root(root.container_info)));

                recursively_traverse_mutation_effects(root, finished_work, lanes);
                CURRENT_HOISTABLE_ROOT.set(previous_hoistable_root);

                commit_reconciliation_effects(finished_work, lanes);
            } else {
                recursively_traverse_mutation_effects(root, finished_work, lanes);
                commit_reconciliation_effects(finished_work, lanes);
            }

            if (flags & UPDATE) != 0 {
                if SUPPORTS_MUTATION
                    && SUPPORTS_HYDRATION
                    && let Some(current) = &current
                {
                    let prev_root_state: &RootState =
                        unsafe { js::cast(current.memoized_state.get()) };
                    if prev_root_state.is_dehydrated {
                        commit_host_hydrated_container(root, finished_work);
                    }
                }
                if SUPPORTS_PERSISTENCE {
                    commit_host_root_container_children(root, finished_work);
                }
            }

            if NEEDS_FORM_RESET.get() {
                // A form component requested to be reset during this commit. We do this
                // after all mutations in the rest of the tree so that `defaultValue`
                // will already be updated. This way you can update `defaultValue` using
                // data sent by the server as a result of the form submission.
                //
                // Theoretically we could check finishedWork.subtreeFlags & FormReset,
                // but the FormReset bit is overloaded with other flags used by other
                // fiber types. So this extra variable lets us skip traversing the tree
                // except when a form was actually submitted.
                NEEDS_FORM_RESET.set(false);
                recursively_reset_forms(finished_work);
            }

            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                && ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
            {
                root.effect_duration.set(Some(
                    root.effect_duration.get().unwrap()
                        + pop_nested_effect_durations(prev_profiler_effect_duration),
                ));
            }

            pop_mutation_context(false);

            if ENABLE_DEFAULT_TRANSITION_INDICATOR.with(|&enabled| enabled)
                && ROOT_MUTATION_CONTEXT.get()
                && includes_loading_indicator_lanes(lanes)
            {
                // This root had a mutation. Mark this root as having rendered a manual
                // loading state.
                mark_indicator_handled(root);
            }
        }
        HOST_PORTAL => {
            // For the purposes of visibility toggling, the direct children of a
            // portal are considered "children" of the nearest hidden
            // OffscreenComponent, regardless of whether there are any host components
            // in between them. This is because portals are not part of the regular
            // host tree hierarchy; we can't assume that just because a portal's
            // HostComponent parent in the React tree will also be a parent in the
            // actual host tree. So we must hide all of them.
            let prev_offscreen_direct_parent_is_hidden = OFFSCREEN_DIRECT_PARENT_IS_HIDDEN.get();
            OFFSCREEN_DIRECT_PARENT_IS_HIDDEN.set(OFFSCREEN_SUBTREE_IS_HIDDEN.get());
            let prev_mutation_context = push_mutation_context();
            if SUPPORTS_RESOURCES {
                let previous_hoistable_root = CURRENT_HOISTABLE_ROOT.get();
                CURRENT_HOISTABLE_ROOT.set(Some(get_hoistable_root(unsafe {
                    js::cast(js::get(finished_work.state_node.get(), "containerInfo"))
                })));
                recursively_traverse_mutation_effects(root, finished_work, lanes);
                commit_reconciliation_effects(finished_work, lanes);
                CURRENT_HOISTABLE_ROOT.set(previous_hoistable_root);
            } else {
                recursively_traverse_mutation_effects(root, finished_work, lanes);
                commit_reconciliation_effects(finished_work, lanes);
            }
            if VIEW_TRANSITION_MUTATION_CONTEXT.get() && IN_UPDATE_VIEW_TRANSITION.get() {
                // A Portal doesn't necessarily exist within the context of this subtree.
                // Ideally we would track which React ViewTransition component nests the container
                // but that's costly. Instead, we treat each Portal as if it's a new React root.
                // Therefore any leaked mutation means that the root should animate.
                ROOT_VIEW_TRANSITION_AFFECTED.set(true);
            }
            pop_mutation_context(prev_mutation_context);
            OFFSCREEN_DIRECT_PARENT_IS_HIDDEN.set(prev_offscreen_direct_parent_is_hidden);

            if (flags & UPDATE) != 0 && SUPPORTS_PERSISTENCE {
                commit_host_portal_container_children(
                    unsafe { js::cast(finished_work.state_node.get()) },
                    finished_work,
                    js::get(finished_work.state_node.get(), "pendingChildren"),
                );
            }
        }
        PROFILER => {
            let prev_profiler_effect_duration = push_nested_effect_durations();

            recursively_traverse_mutation_effects(root, finished_work, lanes);
            commit_reconciliation_effects(finished_work, lanes);

            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                && ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
            {
                let profiler_instance: &ProfilerStateNode =
                    unsafe { js::cast(finished_work.state_node.get()) };
                // Propagate layout effect durations to the next nearest Profiler ancestor.
                // Do not reset these values until the next render so DevTools has a chance to read them first.
                profiler_instance.effect_duration.set(
                    profiler_instance.effect_duration.get()
                        + bubble_nested_effect_durations(prev_profiler_effect_duration),
                );
            }
        }
        ACTIVITY_COMPONENT => {
            recursively_traverse_mutation_effects(root, finished_work, lanes);
            commit_reconciliation_effects(finished_work, lanes);
            if (flags & UPDATE) != 0 {
                let retry_queue: Option<&RetryQueue> =
                    unsafe { js::cast(finished_work.update_queue.get()) };
                if let Some(retry_queue) = retry_queue {
                    finished_work.update_queue.set(js::null());
                    attach_suspense_retry_listeners(finished_work, retry_queue);
                }
            }
        }
        SUSPENSE_COMPONENT => {
            recursively_traverse_mutation_effects(root, finished_work, lanes);
            commit_reconciliation_effects(finished_work, lanes);

            // TODO: We should mark a flag on the Suspense fiber itself, rather than
            // relying on the Offscreen fiber having a flag also being marked. The
            // reason is that this offscreen fiber might not be part of the work-in-
            // progress tree! It could have been reused from a previous render. This
            // doesn't lead to incorrect behavior because we don't rely on the flag
            // check alone; we also compare the states explicitly below. But for
            // modeling purposes, we _should_ be able to rely on the flag check alone.
            // So this is a bit fragile.
            //
            // Also, all this logic could/should move to the passive phase so it
            // doesn't block paint.
            let offscreen_fiber = finished_work.child.borrow().clone().unwrap();
            if (offscreen_fiber.flags.get() & VISIBILITY) != 0 {
                // Throttle the appearance and disappearance of Suspense fallbacks.
                let is_showing_fallback = finished_work.memoized_state.get().is_some();
                let was_showing_fallback = current
                    .as_ref()
                    .is_some_and(|current| current.memoized_state.get().is_some());

                if ALWAYS_THROTTLE_RETRIES {
                    if is_showing_fallback != was_showing_fallback {
                        // A fallback is either appearing or disappearing.
                        mark_commit_time_of_fallback();
                    }
                } else if is_showing_fallback && !was_showing_fallback {
                    // Old behavior. Only mark when a fallback appears, not when
                    // it disappears.
                    mark_commit_time_of_fallback();
                }
            }

            if (flags & UPDATE) != 0 {
                if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
                    commit_suspense_callback(finished_work);
                })) && let Some(error) = error.downcast_ref::<&'static JsError>()
                {
                    let return_fiber = finished_work.r#return.borrow().clone();
                    capture_commit_phase_error(
                        finished_work,
                        return_fiber.as_ref(),
                        error.as_unknown(),
                    );
                }
                let retry_queue: Option<&RetryQueue> =
                    unsafe { js::cast(finished_work.update_queue.get()) };
                if let Some(retry_queue) = retry_queue {
                    finished_work.update_queue.set(js::null());
                    attach_suspense_retry_listeners(finished_work, retry_queue);
                }
            }
        }
        OFFSCREEN_COMPONENT => {
            let new_state: Option<&OffscreenState> =
                unsafe { js::cast(finished_work.memoized_state.get()) };
            let is_hidden = new_state.is_some();
            let was_hidden = current
                .as_ref()
                .is_some_and(|current| current.memoized_state.get().is_some());

            if DISABLE_LEGACY_MODE || (finished_work.mode.get() & CONCURRENT_MODE) != 0 {
                // Before committing the children, track on the stack whether this
                // offscreen subtree was already hidden, so that we don't unmount the
                // effects again.
                let prev_offscreen_subtree_is_hidden = OFFSCREEN_SUBTREE_IS_HIDDEN.get();
                let prev_offscreen_subtree_was_hidden = OFFSCREEN_SUBTREE_WAS_HIDDEN.get();
                let prev_offscreen_direct_parent_is_hidden =
                    OFFSCREEN_DIRECT_PARENT_IS_HIDDEN.get();
                OFFSCREEN_SUBTREE_IS_HIDDEN.set(prev_offscreen_subtree_is_hidden || is_hidden);
                OFFSCREEN_DIRECT_PARENT_IS_HIDDEN
                    .set(prev_offscreen_direct_parent_is_hidden || is_hidden);
                OFFSCREEN_SUBTREE_WAS_HIDDEN.set(prev_offscreen_subtree_was_hidden || was_hidden);
                recursively_traverse_mutation_effects(root, finished_work, lanes);
                OFFSCREEN_SUBTREE_WAS_HIDDEN.set(prev_offscreen_subtree_was_hidden);
                OFFSCREEN_DIRECT_PARENT_IS_HIDDEN.set(prev_offscreen_direct_parent_is_hidden);
                OFFSCREEN_SUBTREE_IS_HIDDEN.set(prev_offscreen_subtree_is_hidden);

                if
                // If this was the root of the reappear.
                was_hidden
                    && !is_hidden
                    && !prev_offscreen_subtree_is_hidden
                    && !prev_offscreen_subtree_was_hidden
                    && ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                    && ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
                    && ENABLE_COMPONENT_PERFORMANCE_TRACK
                    && (finished_work.mode.get() & PROFILE_MODE) != NO_MODE
                    && COMPONENT_EFFECT_START_TIME.get() >= 0.0
                    && COMPONENT_EFFECT_END_TIME.get() >= 0.0
                    && COMPONENT_EFFECT_END_TIME.get() - COMPONENT_EFFECT_START_TIME.get() > 0.05
                {
                    log_component_reappeared(
                        finished_work,
                        COMPONENT_EFFECT_START_TIME.get(),
                        COMPONENT_EFFECT_END_TIME.get(),
                    );
                }
            } else {
                recursively_traverse_mutation_effects(root, finished_work, lanes);
            }

            commit_reconciliation_effects(finished_work, lanes);

            if (flags & VISIBILITY) != 0 {
                let offscreen_instance: &OffscreenInstance =
                    unsafe { js::cast(finished_work.state_node.get()) };

                // Track the current state on the Offscreen instance so we can
                // read it during an event
                if is_hidden {
                    offscreen_instance
                        .visibility
                        .set(offscreen_instance.visibility.get() & !OFFSCREEN_VISIBLE);
                } else {
                    offscreen_instance
                        .visibility
                        .set(offscreen_instance.visibility.get() | OFFSCREEN_VISIBLE);
                }

                let is_update = current.is_some();
                // Only trigger disappear layout effects if:
                //   - This is an update, not first mount.
                //   - This Offscreen was not hidden before.
                //   - Ancestor Offscreen was not hidden in previous commit or in this commit
                if is_hidden
                    && is_update
                    && !was_hidden
                    && !OFFSCREEN_SUBTREE_IS_HIDDEN.get()
                    && !OFFSCREEN_SUBTREE_WAS_HIDDEN.get()
                    && (DISABLE_LEGACY_MODE
                        || (finished_work.mode.get() & CONCURRENT_MODE) != NO_MODE)
                {
                    // Disappear the layout effects of all the children
                    let layout_effect_traversal_flags: LayoutEffectTraversalFlags =
                        if SUPPORTS_SINGLETONS {
                            INCLUDE_HOST_SINGLETONS
                        } else {
                            NO_LAYOUT_EFFECT_TRAVERSAL_FLAGS
                        };
                    let new_offscreen_subtree_is_hidden =
                        is_hidden || OFFSCREEN_SUBTREE_IS_HIDDEN.get();
                    let new_offscreen_subtree_was_hidden =
                        was_hidden || OFFSCREEN_SUBTREE_WAS_HIDDEN.get();
                    let prev_offscreen_subtree_is_hidden = OFFSCREEN_SUBTREE_IS_HIDDEN.get();
                    let prev_offscreen_subtree_was_hidden = OFFSCREEN_SUBTREE_WAS_HIDDEN.get();
                    OFFSCREEN_SUBTREE_IS_HIDDEN.set(new_offscreen_subtree_is_hidden);
                    OFFSCREEN_SUBTREE_WAS_HIDDEN.set(new_offscreen_subtree_was_hidden);
                    recursively_traverse_disappear_layout_effects(
                        finished_work,
                        layout_effect_traversal_flags,
                    );

                    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                        && ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
                        && ENABLE_COMPONENT_PERFORMANCE_TRACK
                        && (finished_work.mode.get() & PROFILE_MODE) != NO_MODE
                        && COMPONENT_EFFECT_START_TIME.get() >= 0.0
                        && COMPONENT_EFFECT_END_TIME.get() >= 0.0
                        && COMPONENT_EFFECT_END_TIME.get() - COMPONENT_EFFECT_START_TIME.get()
                            > 0.05
                    {
                        log_component_disappeared(
                            finished_work,
                            COMPONENT_EFFECT_START_TIME.get(),
                            COMPONENT_EFFECT_END_TIME.get(),
                        );
                    }
                    OFFSCREEN_SUBTREE_IS_HIDDEN.set(prev_offscreen_subtree_is_hidden);
                    OFFSCREEN_SUBTREE_WAS_HIDDEN.set(prev_offscreen_subtree_was_hidden);
                }

                // If it's trying to unhide but the parent is still hidden, then we should not unhide.
                if SUPPORTS_MUTATION && (is_hidden || !OFFSCREEN_DIRECT_PARENT_IS_HIDDEN.get()) {
                    hide_or_unhide_all_children(finished_work, is_hidden);
                }
            }

            // TODO: Move to passive phase
            if (flags & UPDATE) != 0 {
                let offscreen_queue: Option<&OffscreenQueue> =
                    unsafe { js::cast(finished_work.update_queue.get()) };
                if let Some(offscreen_queue) = offscreen_queue {
                    let retry_queue = offscreen_queue.retry_queue.get();
                    if let Some(retry_queue) = retry_queue {
                        offscreen_queue.retry_queue.set(js::null());
                        attach_suspense_retry_listeners(finished_work, retry_queue);
                    }
                }
            }
        }
        SUSPENSE_LIST_COMPONENT => {
            recursively_traverse_mutation_effects(root, finished_work, lanes);
            commit_reconciliation_effects(finished_work, lanes);

            if (flags & UPDATE) != 0 {
                let retry_queue: Option<&RetryQueue> =
                    unsafe { js::cast(finished_work.update_queue.get()) };
                if let Some(retry_queue) = retry_queue {
                    finished_work.update_queue.set(js::null());
                    attach_suspense_retry_listeners(finished_work, retry_queue);
                }
            }
        }
        VIEW_TRANSITION_COMPONENT => {
            if ENABLE_VIEW_TRANSITION {
                if (flags & REF) != 0
                    && !OFFSCREEN_SUBTREE_WAS_HIDDEN.get()
                    && let Some(current) = &current
                {
                    let return_fiber = current.r#return.borrow().clone();
                    safely_detach_ref(current, return_fiber.as_ref());
                }
                let prev_mutation_context = push_mutation_context();
                let prev_update = IN_UPDATE_VIEW_TRANSITION.get();
                let is_view_transition_eligible =
                    ENABLE_VIEW_TRANSITION && includes_only_view_transition_eligible_lanes(lanes);
                let props: &ViewTransitionProps =
                    unsafe { js::cast(finished_work.memoized_props.get()) };
                IN_UPDATE_VIEW_TRANSITION.set(
                    is_view_transition_eligible
                        && get_view_transition_class_name(props.default, props.update).as_deref()
                            != Some("none"),
                );
                recursively_traverse_mutation_effects(root, finished_work, lanes);
                commit_reconciliation_effects(finished_work, lanes);
                if is_view_transition_eligible {
                    if current.is_none() {
                        // This is a new mount. We should have handled this as part of the
                        // Placement effect or it is deeper inside a entering transition.
                    } else if VIEW_TRANSITION_MUTATION_CONTEXT.get() {
                        // Something mutated in this tree so we need to animate this regardless
                        // what the measurements say. We use the Update flag to track this.
                        // If diffing was done in the render phase, like we used, this could have
                        // been done in the render already.
                        finished_work.flags.set(finished_work.flags.get() | UPDATE);
                    }
                }
                IN_UPDATE_VIEW_TRANSITION.set(prev_update);
                pop_mutation_context(prev_mutation_context);
            }
        }
        SCOPE_COMPONENT => {
            if ENABLE_SCOPE_API {
                recursively_traverse_mutation_effects(root, finished_work, lanes);
                commit_reconciliation_effects(finished_work, lanes);

                // TODO: This is a temporary solution that allowed us to transition away
                // from React Flare on www.
                if (flags & REF) != 0 {
                    let return_fiber = finished_work.r#return.borrow().clone();
                    if !OFFSCREEN_SUBTREE_WAS_HIDDEN.get() && current.is_some() {
                        safely_detach_ref(finished_work, return_fiber.as_ref());
                    }
                    if !OFFSCREEN_SUBTREE_IS_HIDDEN.get() {
                        safely_attach_ref(finished_work, return_fiber.as_ref());
                    }
                }
                if (flags & UPDATE) != 0 {
                    let scope_instance = finished_work.state_node.get();
                    prepare_scope_update(scope_instance, finished_work);
                }
            }
        }
        _ => {
            if finished_work.tag.get() == FRAGMENT {
                if (flags & REF) != 0
                    && !OFFSCREEN_SUBTREE_WAS_HIDDEN.get()
                    && let Some(current) = &current
                {
                    let return_fiber = current.r#return.borrow().clone();
                    safely_detach_ref(current, return_fiber.as_ref());
                }
                if let Some(current) = &current
                    && let Some(state_node) = current.state_node.get()
                {
                    update_fragment_instance_fiber(finished_work, state_node);
                }
            }
            // Fallthrough
            recursively_traverse_mutation_effects(root, finished_work, lanes);
            commit_reconciliation_effects(finished_work, lanes);
        }
    }

    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        && ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
        && ENABLE_COMPONENT_PERFORMANCE_TRACK
        && (finished_work.mode.get() & PROFILE_MODE) != NO_MODE
        && COMPONENT_EFFECT_START_TIME.get() >= 0.0
        && COMPONENT_EFFECT_END_TIME.get() >= 0.0
    {
        if COMPONENT_EFFECT_SPAWNED_UPDATE.get() || COMPONENT_EFFECT_DURATION.get() > 0.05 {
            log_component_effect(
                finished_work,
                COMPONENT_EFFECT_START_TIME.get(),
                COMPONENT_EFFECT_END_TIME.get(),
                COMPONENT_EFFECT_DURATION.get(),
                COMPONENT_EFFECT_ERRORS.with_borrow(Clone::clone).as_deref(),
            );
        }
        let return_fiber = finished_work.r#return.borrow().clone();
        let return_alternate = return_fiber
            .as_ref()
            .and_then(|return_fiber| return_fiber.alternate.borrow().clone());
        if
        // Insertion
        finished_work.alternate.borrow().is_none()
            && let Some(return_fiber) = return_fiber
            && let Some(return_alternate) = return_alternate
            && COMPONENT_EFFECT_END_TIME.get() - COMPONENT_EFFECT_START_TIME.get() > 0.05
        {
            let is_hydration = is_hydrating_parent(&return_alternate, &return_fiber);
            if !is_hydration {
                log_component_mount(
                    finished_work,
                    COMPONENT_EFFECT_START_TIME.get(),
                    COMPONENT_EFFECT_END_TIME.get(),
                );
            }
        }
    }

    pop_component_effect_start(prev_effect_start);
    pop_component_effect_duration(prev_effect_duration);
    pop_component_effect_errors(prev_effect_errors);
    pop_component_effect_did_spawn_update(prev_effect_did_spawn_update);
}

fn commit_reconciliation_effects(finished_work: &Rc<Fiber>, _committed_lanes: Lanes) {
    // Placement effects (insertions, reorders) can be scheduled on any fiber
    // type. They needs to happen after the children effects have fired, but
    // before the effects on this fiber have fired.
    let flags = finished_work.flags.get();
    if (flags & PLACEMENT) != 0 {
        commit_host_placement(finished_work);
        // Clear the "placement" from effect tag so that we know that this is
        // inserted, before any life-cycles like componentDidMount gets called.
        // TODO: findDOMNode doesn't rely on this any more but isMounted does
        // and isMounted is deprecated anyway so we should be able to kill this.
        finished_work
            .flags
            .set(finished_work.flags.get() & !PLACEMENT);
    }
    if (flags & HYDRATING) != 0 {
        finished_work
            .flags
            .set(finished_work.flags.get() & !HYDRATING);
    }
}

fn recursively_reset_forms(parent_fiber: &Fiber) {
    if (parent_fiber.subtree_flags.get() & FORM_RESET) != 0 {
        let mut child = parent_fiber.child.borrow().clone();
        while let Some(current) = child {
            reset_form_on_fiber(&current);
            child = current.sibling.borrow().clone();
        }
    }
}

fn reset_form_on_fiber(fiber: &Fiber) {
    recursively_reset_forms(fiber);
    if fiber.tag.get() == HOST_COMPONENT && (fiber.flags.get() & FORM_RESET) != 0 {
        let form_instance = fiber.state_node.get();
        reset_form_instance(form_instance);
    }
}

pub fn commit_after_mutation_effects(
    root: &FiberRoot,
    finished_work: &Rc<Fiber>,
    committed_lanes: Lanes,
) {
    if !ENABLE_VIEW_TRANSITION {
        // This phase is only used for view transitions.
        return;
    }
    commit_after_mutation_effects_on_fiber(finished_work, root, committed_lanes);
}

fn recursively_traverse_after_mutation_effects(
    root: &FiberRoot,
    parent_fiber: &Fiber,
    lanes: Lanes,
) {
    // We need to visit the same nodes that we visited in the before mutation phase.
    if (parent_fiber.subtree_flags.get() & BEFORE_AND_AFTER_MUTATION_TRANSITION_MASK) != 0 {
        let mut child = parent_fiber.child.borrow().clone();
        while let Some(current) = child {
            commit_after_mutation_effects_on_fiber(&current, root, lanes);
            child = current.sibling.borrow().clone();
        }
    } else {
        // Nothing has changed in this subtree, but the parent may have still affected
        // its size and position. We need to measure this and if not, restore it to
        // not animate.
        measure_nested_view_transitions(parent_fiber, false);
    }
}

fn commit_after_mutation_effects_on_fiber(
    finished_work: &Rc<Fiber>,
    root: &FiberRoot,
    lanes: Lanes,
) {
    let current = finished_work.alternate.borrow().clone();
    let Some(current) = current else {
        // This is a newly inserted subtree. We can't use Placement flags to detect
        // this since they get removed in the mutation phase. Usually it's not enough
        // to just check current because that can also happen deeper in the same tree.
        // However, since we don't need to visit newly inserted subtrees in AfterMutation
        // we can just bail after we're done with the first one.
        // The first ViewTransition inside a newly mounted tree runs an enter transition
        // but other nested ones don't unless they have a named pair.
        commit_enter_view_transitions(finished_work, false);
        return;
    };

    match finished_work.tag.get() {
        HOST_ROOT => {
            VIEW_TRANSITION_CONTEXT_CHANGED.set(false);
            ROOT_VIEW_TRANSITION_NAME_CANCELED.set(false);
            push_view_transition_cancelable_scope();
            recursively_traverse_after_mutation_effects(root, finished_work, lanes);
            if !VIEW_TRANSITION_CONTEXT_CHANGED.get() && !ROOT_VIEW_TRANSITION_AFFECTED.get() {
                // If we didn't leak any resizing out to the root, we don't have to transition
                // the root itself. This means that we can now safely cancel any cancellations
                // that bubbled all the way up.
                let cancelable_children =
                    VIEW_TRANSITION_CANCELABLE_CHILDREN.with_borrow(Clone::clone);
                if let Some(cancelable_children) = cancelable_children {
                    let mut i = 0;
                    while i < cancelable_children.borrow().len() {
                        let instance = cancelable_children.borrow()[i];
                        let old_name = cancelable_children.borrow()[i + 1];
                        let props = cancelable_children.borrow()[i + 2];
                        cancel_view_transition_name(instance, unsafe { js::cast(old_name) }, props);
                        i += 3;
                    }
                }
                // We also cancel the root itself.
                cancel_root_view_transition_name(root.container_info);
                ROOT_VIEW_TRANSITION_NAME_CANCELED.set(true);
            }
            pop_view_transition_cancelable_scope(js::null());
        }
        HOST_COMPONENT => {
            recursively_traverse_after_mutation_effects(root, finished_work, lanes);
        }
        HOST_PORTAL => {
            let prev_context_changed = VIEW_TRANSITION_CONTEXT_CHANGED.get();
            VIEW_TRANSITION_CONTEXT_CHANGED.set(false);
            recursively_traverse_after_mutation_effects(root, finished_work, lanes);
            if VIEW_TRANSITION_CONTEXT_CHANGED.get() {
                // A Portal doesn't necessarily exist within the context of this subtree.
                // Ideally we would track which React ViewTransition component nests the container
                // but that's costly. Instead, we treat each Portal as if it's a new React root.
                // Therefore any leaked resize of a child could affect the root so the root should animate.
                // We only do this if the Portal is inside a ViewTransition and it is not disabled
                // with update="none". Otherwise the Portal is considered not animating.
                ROOT_VIEW_TRANSITION_AFFECTED.set(true);
            }
            VIEW_TRANSITION_CONTEXT_CHANGED.set(prev_context_changed);
        }
        OFFSCREEN_COMPONENT => {
            let is_modern_root =
                DISABLE_LEGACY_MODE || (finished_work.mode.get() & CONCURRENT_MODE) != NO_MODE;
            if is_modern_root {
                let is_hidden = finished_work.memoized_state.get().is_some();
                if is_hidden {
                    // The Offscreen tree is hidden. Skip over its after mutation effects.
                } else {
                    // The Offscreen tree is visible.
                    let was_hidden = current.memoized_state.get().is_some();
                    if was_hidden {
                        commit_enter_view_transitions(finished_work, false);
                        // If it was previous hidden then the children are treated as enter
                        // not updates so we don't need to visit these children.
                    } else {
                        recursively_traverse_after_mutation_effects(root, finished_work, lanes);
                    }
                }
            } else {
                recursively_traverse_after_mutation_effects(root, finished_work, lanes);
            }
        }
        VIEW_TRANSITION_COMPONENT => {
            let prev_context_changed = VIEW_TRANSITION_CONTEXT_CHANGED.get();
            let prev_cancelable_children = push_view_transition_cancelable_scope();
            VIEW_TRANSITION_CONTEXT_CHANGED.set(false);
            recursively_traverse_after_mutation_effects(root, finished_work, lanes);

            if VIEW_TRANSITION_CONTEXT_CHANGED.get() {
                finished_work.flags.set(finished_work.flags.get() | UPDATE);
            }

            let in_viewport = measure_update_view_transition(&current, finished_work, false);

            if (finished_work.flags.get() & UPDATE) == NO_FLAGS || !in_viewport {
                // If this boundary didn't update, then we may be able to cancel its children.
                // We bubble them up to the parent set to be determined later if we can cancel.
                // Similarly, if old and new state was outside the viewport, we can skip it
                // even if it did update.
                match prev_cancelable_children {
                    None => {
                        // Bubbling up this whole set to the parent.
                    }
                    Some(prev_cancelable_children) => {
                        // Merge with parent set.
                        let cancelable_children =
                            VIEW_TRANSITION_CANCELABLE_CHILDREN.with_borrow(Clone::clone);
                        if let Some(cancelable_children) = cancelable_children {
                            let mut i = 0;
                            while i < cancelable_children.borrow().len() {
                                let child = cancelable_children.borrow()[i];
                                prev_cancelable_children.borrow_mut().push(child);
                                i += 1;
                            }
                        }
                        pop_view_transition_cancelable_scope(Some(prev_cancelable_children));
                    }
                }
                // TODO: If this doesn't end up canceled, because a parent animates,
                // then we should probably issue an event since this instance is part of it.
            } else {
                let props: &ViewTransitionProps =
                    unsafe { js::cast(finished_work.memoized_props.get()) };
                schedule_view_transition_event(finished_work, props.on_update);

                // If this boundary did update, we cannot cancel its children so those are dropped.
                pop_view_transition_cancelable_scope(prev_cancelable_children);
            }

            if (finished_work.flags.get() & AFFECTED_PARENT_LAYOUT) != NO_FLAGS {
                // This boundary changed size in a way that may have caused its parent to
                // relayout. We need to bubble this information up to the parent.
                VIEW_TRANSITION_CONTEXT_CHANGED.set(true);
            } else {
                // Otherwise, we restore it to whatever the parent had found so far.
                VIEW_TRANSITION_CONTEXT_CHANGED.set(prev_context_changed);
            }
        }
        _ => {
            recursively_traverse_after_mutation_effects(root, finished_work, lanes);
        }
    }
}

pub fn commit_layout_effects(
    finished_work: &Rc<Fiber>,
    root: &Rc<FiberRoot>,
    committed_lanes: Lanes,
) {
    IN_PROGRESS_LANES.set(Some(committed_lanes));
    IN_PROGRESS_ROOT.set(Some(Rc::clone(root)));

    reset_component_effect_timers();

    let current = finished_work.alternate.borrow().clone();
    commit_layout_effect_on_fiber(root, current.as_ref(), finished_work, committed_lanes);

    IN_PROGRESS_LANES.set(js::null());
    IN_PROGRESS_ROOT.set(js::null());
}

fn recursively_traverse_layout_effects(root: &FiberRoot, parent_fiber: &Fiber, lanes: Lanes) {
    if (parent_fiber.subtree_flags.get() & LAYOUT_MASK) != 0 {
        let mut child = parent_fiber.child.borrow().clone();
        while let Some(current_child) = child {
            let current = current_child.alternate.borrow().clone();
            commit_layout_effect_on_fiber(root, current.as_ref(), &current_child, lanes);
            child = current_child.sibling.borrow().clone();
        }
    }
}

#[cfg_attr(rust_js, rust_js::name = "disappearLayoutEffectsForDEVValidation")]
pub fn disappear_layout_effects_for_dev_validation(finished_work: &Rc<Fiber>) {
    if __DEV__ {
        disappear_layout_effects(finished_work, NO_LAYOUT_EFFECT_TRAVERSAL_FLAGS);
    }
}

fn disappear_layout_effects(
    finished_work: &Rc<Fiber>,
    layout_effect_traversal_flags: LayoutEffectTraversalFlags,
) {
    let prev_effect_start = push_component_effect_start();
    let prev_effect_duration = push_component_effect_duration();
    let prev_effect_errors = push_component_effect_errors();
    let prev_effect_did_spawn_update = push_component_effect_did_spawn_update();
    let return_fiber = finished_work.r#return.borrow().clone();
    match finished_work.tag.get() {
        FUNCTION_COMPONENT | FORWARD_REF | MEMO_COMPONENT | SIMPLE_MEMO_COMPONENT => {
            // TODO (Offscreen) Check: flags & LayoutStatic
            commit_hook_layout_unmount_effects(finished_work, return_fiber.as_ref(), HOOK_LAYOUT);
            recursively_traverse_disappear_layout_effects(
                finished_work,
                layout_effect_traversal_flags,
            );
        }
        CLASS_COMPONENT => {
            // TODO (Offscreen) Check: flags & RefStatic
            safely_detach_ref(finished_work, return_fiber.as_ref());

            let instance: &'static ClassInstance =
                unsafe { js::cast(finished_work.state_node.get()) };
            if js::type_of(js::get(instance, "componentWillUnmount")) == "function" {
                safely_call_component_will_unmount(finished_work, return_fiber.as_ref(), instance);
            }

            recursively_traverse_disappear_layout_effects(
                finished_work,
                layout_effect_traversal_flags,
            );
        }
        HOST_SINGLETON | HOST_COMPONENT => {
            if finished_work.tag.get() == HOST_SINGLETON && SUPPORTS_SINGLETONS {
                let include_host_singletons = (layout_effect_traversal_flags
                    & INCLUDE_HOST_SINGLETONS)
                    != NO_LAYOUT_EFFECT_TRAVERSAL_FLAGS;
                if include_host_singletons {
                    // TODO (Offscreen) Check: flags & RefStatic
                    commit_host_singleton_release(finished_work);
                }
            }
            // Expected fallthrough to HostComponent
            // TODO (Offscreen) Check: flags & RefStatic
            safely_detach_ref(finished_work, return_fiber.as_ref());

            if
            // HostHoistable shares this case via fallthrough but must not be
            // attributed to fragment instances. HostText has its own case below.
            finished_work.tag.get() == HOST_COMPONENT
                || finished_work.tag.get() == HOST_SINGLETON
            {
                commit_fragment_instance_deletion_effects(finished_work);
            }

            recursively_traverse_disappear_layout_effects(
                finished_work,
                layout_effect_traversal_flags,
            );
        }
        HOST_TEXT => {
            commit_fragment_instance_deletion_effects(finished_work);
        }
        HOST_HOISTABLE => {
            // TODO (Offscreen) Check: flags & RefStatic
            safely_detach_ref(finished_work, return_fiber.as_ref());

            if SUPPORTS_RESOURCES {
                // We only act on Hoistable Instances (memoizedState === null).
                // Resources (memoizedState !== null) are ref-counted and intentionally
                // remain in the document across Activity visibility transitions;
                // they are released only on actual deletion.
                let instance = finished_work.state_node.get();
                if finished_work.memoized_state.get().is_none()
                    && instance.is_some()
                    && !OFFSCREEN_SUBTREE_WAS_HIDDEN.get()
                {
                    unmount_hoistable(instance);
                }
            }

            recursively_traverse_disappear_layout_effects(
                finished_work,
                layout_effect_traversal_flags,
            );
        }
        OFFSCREEN_COMPONENT => {
            let is_hidden = finished_work.memoized_state.get().is_some();
            if is_hidden {
                // Nested Offscreen tree is already hidden. Don't disappear
                // its effects.
            } else {
                recursively_traverse_disappear_layout_effects(
                    finished_work,
                    layout_effect_traversal_flags,
                );
            }
        }
        VIEW_TRANSITION_COMPONENT => {
            if ENABLE_VIEW_TRANSITION {
                if __DEV__ && (finished_work.flags.get() & VIEW_TRANSITION_NAMED_STATIC) != 0 {
                    untrack_named_view_transition(finished_work);
                }
                safely_detach_ref(finished_work, return_fiber.as_ref());
            }
            recursively_traverse_disappear_layout_effects(
                finished_work,
                layout_effect_traversal_flags,
            );
        }
        _ => {
            if finished_work.tag.get() == FRAGMENT {
                safely_detach_ref(finished_work, return_fiber.as_ref());
            }
            // Fallthrough
            recursively_traverse_disappear_layout_effects(
                finished_work,
                layout_effect_traversal_flags,
            );
        }
    }

    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        && ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
        && ENABLE_COMPONENT_PERFORMANCE_TRACK
        && (finished_work.mode.get() & PROFILE_MODE) != NO_MODE
        && COMPONENT_EFFECT_START_TIME.get() >= 0.0
        && COMPONENT_EFFECT_END_TIME.get() >= 0.0
        && (COMPONENT_EFFECT_SPAWNED_UPDATE.get() || COMPONENT_EFFECT_DURATION.get() > 0.05)
    {
        log_component_effect(
            finished_work,
            COMPONENT_EFFECT_START_TIME.get(),
            COMPONENT_EFFECT_END_TIME.get(),
            COMPONENT_EFFECT_DURATION.get(),
            COMPONENT_EFFECT_ERRORS.with_borrow(Clone::clone).as_deref(),
        );
    }

    pop_component_effect_start(prev_effect_start);
    pop_component_effect_duration(prev_effect_duration);
    pop_component_effect_errors(prev_effect_errors);
    pop_component_effect_did_spawn_update(prev_effect_did_spawn_update);
}

fn recursively_traverse_disappear_layout_effects(
    parent_fiber: &Fiber,
    layout_effect_traversal_flags: LayoutEffectTraversalFlags,
) {
    // TODO (Offscreen) Check: subtreeflags & (RefStatic | LayoutStatic)
    let mut child = parent_fiber.child.borrow().clone();
    while let Some(current) = child {
        disappear_layout_effects(&current, layout_effect_traversal_flags);
        child = current.sibling.borrow().clone();
    }
}

#[cfg_attr(rust_js, rust_js::name = "reappearLayoutEffectsForDEVValidation")]
pub fn reappear_layout_effects_for_dev_validation(
    finished_root: &FiberRoot,
    current: Option<&Rc<Fiber>>,
    finished_work: &Rc<Fiber>,
) {
    if __DEV__ {
        reappear_layout_effects(
            finished_root,
            current,
            finished_work,
            NO_LAYOUT_EFFECT_TRAVERSAL_FLAGS,
        );
    }
}

fn reappear_layout_effects(
    finished_root: &FiberRoot,
    current: Option<&Rc<Fiber>>,
    finished_work: &Rc<Fiber>,
    // This function visits both newly finished work and nodes that were re-used
    // from a previously committed tree. We cannot check non-static flags if the
    // node was reused.
    layout_effect_traversal_flags: LayoutEffectTraversalFlags,
) {
    let prev_effect_start = push_component_effect_start();
    let prev_effect_duration = push_component_effect_duration();
    let prev_effect_errors = push_component_effect_errors();
    let prev_effect_did_spawn_update = push_component_effect_did_spawn_update();
    // Turn on layout effects in a tree that previously disappeared.
    let flags = finished_work.flags.get();
    let include_work_in_progress_effects = (layout_effect_traversal_flags
        & INCLUDE_WORK_IN_PROGRESS_EFFECTS)
        != NO_LAYOUT_EFFECT_TRAVERSAL_FLAGS;
    let return_fiber = finished_work.r#return.borrow().clone();
    match finished_work.tag.get() {
        FUNCTION_COMPONENT | FORWARD_REF | SIMPLE_MEMO_COMPONENT => {
            recursively_traverse_reappear_layout_effects(
                finished_root,
                finished_work,
                layout_effect_traversal_flags,
            );
            // TODO: Check flags & LayoutStatic
            commit_hook_layout_effects(finished_work, HOOK_LAYOUT);
        }
        CLASS_COMPONENT => {
            recursively_traverse_reappear_layout_effects(
                finished_root,
                finished_work,
                layout_effect_traversal_flags,
            );

            commit_class_did_mount(finished_work);

            commit_class_hidden_callbacks(finished_work);

            // If this is newly finished work, check for setState callbacks
            if include_work_in_progress_effects && (flags & CALLBACK) != 0 {
                commit_class_callbacks(finished_work);
            }

            // TODO: Check flags & RefStatic
            safely_attach_ref(finished_work, return_fiber.as_ref());
        }
        // Unlike commitLayoutEffectsOnFiber, we don't need to handle HostRoot
        // because this function only visits nodes that are inside an
        // Offscreen fiber.
        // case HostRoot: {
        //  ...
        // }
        HOST_SINGLETON | HOST_COMPONENT => {
            if finished_work.tag.get() == HOST_SINGLETON && SUPPORTS_SINGLETONS {
                let include_host_singletons = (layout_effect_traversal_flags
                    & INCLUDE_HOST_SINGLETONS)
                    != NO_LAYOUT_EFFECT_TRAVERSAL_FLAGS;
                if include_host_singletons {
                    // We acquire the singleton instance first so it has appropriate
                    // styles before other layout effects run. This isn't perfect because
                    // an early sibling of the singleton may have an effect that can
                    // observe the singleton before it is acquired.
                    // @TODO move this to the mutation phase. The reason it isn't there yet
                    // is it seemingly requires an extra traversal because we need to move the
                    // disappear effect into a phase before the appear phase
                    commit_host_singleton_acquisition(finished_work);
                    // We fall through to the HostComponent case below.
                }
            }
            // Fallthrough
            if finished_work.tag.get() == HOST_COMPONENT
                || finished_work.tag.get() == HOST_SINGLETON
            {
                commit_fragment_instance_insertion_effects(finished_work);
            }
            recursively_traverse_reappear_layout_effects(
                finished_root,
                finished_work,
                layout_effect_traversal_flags,
            );

            // Renderers may schedule work to be done after host components are mounted
            // (eg DOM renderer may schedule auto-focus for inputs and form controls).
            // These effects should only be committed when components are first mounted,
            // aka when there is no current/alternate.
            if include_work_in_progress_effects && current.is_none() && (flags & UPDATE) != 0 {
                commit_host_mount(finished_work);
            }

            // TODO: Check flags & Ref
            safely_attach_ref(finished_work, return_fiber.as_ref());
        }
        HOST_TEXT => {
            commit_fragment_instance_insertion_effects(finished_work);
        }
        HOST_HOISTABLE => {
            if SUPPORTS_RESOURCES {
                // The reappear traversal runs whenever an Activity transitions from
                // hidden to visible. We piggy-back on it (rather than adding a
                // separate recursive traversal) to insert hoistable metadata such as
                // <title> and <meta> into the document.
                //
                // We only act on Hoistable Instances (memoizedState === null).
                // Resources stay mounted across Activity visibility transitions.
                //
                // The parentNode guard makes this idempotent and safe under StrictMode
                // dev double-invoke: if the instance is already attached we skip.
                //
                // Note: this runs in the layout phase. A useLayoutEffect on an earlier
                // sibling can therefore observe document.title before the hoistable
                // is re-attached. Moving this to the mutation phase would require an
                // additional unconditional traversal of the Activity subtree (the
                // mutation traversal is gated by subtreeFlags and would skip an
                // unchanged hoistable). This is the same tradeoff as for HostSingleton.
                let instance = finished_work.state_node.get();
                if finished_work.memoized_state.get().is_none()
                    && instance.is_some()
                    && !OFFSCREEN_SUBTREE_IS_HIDDEN.get()
                {
                    // currentHoistableRoot is only maintained during the mutation phase.
                    // Derive the hoistable root from the instance's owner document so
                    // this works in the layout phase too. Hoistable Instances are
                    // hoisted to document.head, which always lives in ownerDocument.
                    mount_hoistable(
                        get_hoistable_root(unsafe { js::cast(js::get(instance, "ownerDocument")) }),
                        finished_work.r#type.get(),
                        instance,
                    );
                }
            }
            recursively_traverse_reappear_layout_effects(
                finished_root,
                finished_work,
                layout_effect_traversal_flags,
            );

            if include_work_in_progress_effects && current.is_none() && (flags & UPDATE) != 0 {
                commit_host_mount(finished_work);
            }

            // TODO: Check flags & Ref
            safely_attach_ref(finished_work, return_fiber.as_ref());
        }
        PROFILER => {
            // TODO: Figure out how Profiler updates should work with Offscreen
            if include_work_in_progress_effects && (flags & UPDATE) != 0 {
                let prev_profiler_effect_duration = push_nested_effect_durations();

                recursively_traverse_reappear_layout_effects(
                    finished_root,
                    finished_work,
                    layout_effect_traversal_flags,
                );

                let profiler_instance: &ProfilerStateNode =
                    unsafe { js::cast(finished_work.state_node.get()) };

                if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                    && ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
                {
                    // Propagate layout effect durations to the next nearest Profiler ancestor.
                    // Do not reset these values until the next render so DevTools has a chance to read them first.
                    profiler_instance.effect_duration.set(
                        profiler_instance.effect_duration.get()
                            + bubble_nested_effect_durations(prev_profiler_effect_duration),
                    );
                }

                commit_profiler_update(
                    finished_work,
                    current.map(Rc::as_ref),
                    COMMIT_START_TIME.get(),
                    profiler_instance.effect_duration.get(),
                );
            } else {
                recursively_traverse_reappear_layout_effects(
                    finished_root,
                    finished_work,
                    layout_effect_traversal_flags,
                );
            }
        }
        ACTIVITY_COMPONENT => {
            recursively_traverse_reappear_layout_effects(
                finished_root,
                finished_work,
                layout_effect_traversal_flags,
            );

            if include_work_in_progress_effects && (flags & UPDATE) != 0 {
                // TODO: Delete this feature.
                commit_activity_hydration_callbacks(finished_root, finished_work);
            }
        }
        SUSPENSE_COMPONENT => {
            recursively_traverse_reappear_layout_effects(
                finished_root,
                finished_work,
                layout_effect_traversal_flags,
            );

            if include_work_in_progress_effects && (flags & UPDATE) != 0 {
                // TODO: Delete this feature.
                commit_suspense_hydration_callbacks(finished_root, finished_work);
            }
        }
        OFFSCREEN_COMPONENT => {
            let offscreen_state: Option<&OffscreenState> =
                unsafe { js::cast(finished_work.memoized_state.get()) };
            let is_hidden = offscreen_state.is_some();
            if is_hidden {
                // Nested Offscreen tree is still hidden. Don't re-appear its effects.
            } else {
                recursively_traverse_reappear_layout_effects(
                    finished_root,
                    finished_work,
                    layout_effect_traversal_flags,
                );
            }
            // TODO: Check flags & Ref
            safely_attach_ref(finished_work, return_fiber.as_ref());
        }
        VIEW_TRANSITION_COMPONENT => {
            if ENABLE_VIEW_TRANSITION {
                recursively_traverse_reappear_layout_effects(
                    finished_root,
                    finished_work,
                    layout_effect_traversal_flags,
                );
                if __DEV__ && (flags & VIEW_TRANSITION_NAMED_STATIC) != 0 {
                    track_named_view_transition(finished_work);
                }
                safely_attach_ref(finished_work, return_fiber.as_ref());
            }
        }
        _ => {
            if finished_work.tag.get() == FRAGMENT {
                safely_attach_ref(finished_work, return_fiber.as_ref());
            }
            // Fallthrough
            recursively_traverse_reappear_layout_effects(
                finished_root,
                finished_work,
                layout_effect_traversal_flags,
            );
        }
    }

    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        && ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
        && ENABLE_COMPONENT_PERFORMANCE_TRACK
        && (finished_work.mode.get() & PROFILE_MODE) != NO_MODE
        && COMPONENT_EFFECT_START_TIME.get() >= 0.0
        && COMPONENT_EFFECT_END_TIME.get() >= 0.0
        && (COMPONENT_EFFECT_SPAWNED_UPDATE.get() || COMPONENT_EFFECT_DURATION.get() > 0.05)
    {
        log_component_effect(
            finished_work,
            COMPONENT_EFFECT_START_TIME.get(),
            COMPONENT_EFFECT_END_TIME.get(),
            COMPONENT_EFFECT_DURATION.get(),
            COMPONENT_EFFECT_ERRORS.with_borrow(Clone::clone).as_deref(),
        );
    }

    pop_component_effect_start(prev_effect_start);
    pop_component_effect_duration(prev_effect_duration);
    pop_component_effect_errors(prev_effect_errors);
    pop_component_effect_did_spawn_update(prev_effect_did_spawn_update);
}

fn recursively_traverse_reappear_layout_effects(
    finished_root: &FiberRoot,
    parent_fiber: &Fiber,
    layout_effect_traversal_flags: LayoutEffectTraversalFlags,
) {
    // This function visits both newly finished work and nodes that were re-used
    // from a previously committed tree. We cannot check non-static flags if the
    // node was reused.
    let child_layout_effect_traversal_flags =
        if (parent_fiber.subtree_flags.get() & LAYOUT_MASK) != NO_FLAGS {
            layout_effect_traversal_flags
        } else {
            layout_effect_traversal_flags & !INCLUDE_WORK_IN_PROGRESS_EFFECTS
        };

    // TODO (Offscreen) Check: flags & (RefStatic | LayoutStatic)
    let mut child = parent_fiber.child.borrow().clone();
    while let Some(current_child) = child {
        let current = current_child.alternate.borrow().clone();
        reappear_layout_effects(
            finished_root,
            current.as_ref(),
            &current_child,
            child_layout_effect_traversal_flags,
        );
        child = current_child.sibling.borrow().clone();
    }
}

/// What the work loop committed: the transitions of this render, if any.
type CommittedTransitions<'a> = Option<&'a [&'static Transition]>;

fn commit_offscreen_passive_mount_effects(
    current: Option<&Fiber>,
    finished_work: &Fiber,
    instance: &OffscreenInstance,
) {
    let mut previous_cache: Option<&Rc<Cache>> = js::null();
    if let Some(current) = current
        && let Some(memoized_state) =
            unsafe { js::cast::<Option<&OffscreenState>>(current.memoized_state.get()) }
        && let Some(cache_pool) =
            unsafe { js::cast::<Option<&SpawnedCachePool>>(memoized_state.cache_pool) }
    {
        previous_cache = Some(&cache_pool.pool);
    }
    let mut next_cache: Option<&Rc<Cache>> = js::null();
    if let Some(memoized_state) =
        unsafe { js::cast::<Option<&OffscreenState>>(finished_work.memoized_state.get()) }
        && let Some(cache_pool) =
            unsafe { js::cast::<Option<&SpawnedCachePool>>(memoized_state.cache_pool) }
    {
        next_cache = Some(&cache_pool.pool);
    }
    // Retain/release the cache used for pending (suspended) nodes.
    // Note that this is only reached in the non-suspended/visible case:
    // when the content is suspended/hidden, the retain/release occurs
    // via the parent Suspense component (see case above).
    if !js::strict_equals(next_cache, previous_cache) {
        if let Some(next_cache) = next_cache {
            retain_cache(next_cache);
        }
        if let Some(previous_cache) = previous_cache {
            release_cache(previous_cache);
        }
    }

    if ENABLE_TRANSITION_TRACING {
        // TODO: Pre-rendering should not be counted as part of a transition. We
        // may add separate logs for pre-rendering, but it's not part of the
        // primary metrics.
        let offscreen_state: Option<&OffscreenState> =
            unsafe { js::cast(finished_work.memoized_state.get()) };
        let queue: Option<&OffscreenQueue> = unsafe { js::cast(finished_work.update_queue.get()) };

        let is_hidden = offscreen_state.is_some();
        if let Some(queue) = queue {
            if is_hidden {
                let transitions = queue.transitions.borrow().clone();
                if let Some(transitions) = transitions {
                    transitions.iter().for_each(|&transition| {
                        // Add all the transitions saved in the update queue during
                        // the render phase (ie the transitions associated with this boundary)
                        // into the transitions set.
                        if instance.transitions.get().is_none() {
                            instance.transitions.set(Some(js::set::new(None)));
                        }
                        instance.transitions.get().unwrap().add(transition);
                    });
                }

                let marker_instances = queue.marker_instances.borrow().clone();
                if let Some(marker_instances) = marker_instances {
                    marker_instances.iter().for_each(|marker_instance| {
                        let marker_transitions = marker_instance.transitions.get();
                        // There should only be a few tracing marker transitions because
                        // they should be only associated with the transition that
                        // caused them
                        if let Some(marker_transitions) = marker_transitions {
                            marker_transitions.for_each(|transition| {
                                match instance.transitions.get() {
                                    None => instance.transitions.set(Some(js::set::new(None))),
                                    Some(transitions) if transitions.has(transition) => {
                                        if marker_instance.pending_boundaries.get().is_none() {
                                            marker_instance
                                                .pending_boundaries
                                                .set(Some(js::map::new(None)));
                                        }
                                        if instance.pending_markers.get().is_none() {
                                            instance.pending_markers.set(Some(js::set::new(None)));
                                        }

                                        instance
                                            .pending_markers
                                            .get()
                                            .unwrap()
                                            .add(marker_instance);
                                    }
                                    Some(_) => {}
                                }
                            });
                        }
                    });
                }
            }

            finished_work.update_queue.set(js::null());
        }

        commit_transition_progress(finished_work);

        // TODO: Refactor this into an if/else branch
        if !is_hidden {
            instance.transitions.set(js::null());
            instance.pending_markers.set(js::null());
        }
    }
}

fn commit_cache_passive_mount_effect(_current: Option<&Fiber>, finished_work: &Fiber) {
    let mut previous_cache: Option<&Rc<Cache>> = js::null();
    let alternate = finished_work.alternate.borrow().clone();
    if let Some(alternate) = &alternate {
        previous_cache = Some(
            &unsafe { js::cast::<&CacheComponentState>(alternate.memoized_state.get()) }.cache,
        );
    }
    let next_cache =
        &unsafe { js::cast::<&CacheComponentState>(finished_work.memoized_state.get()) }.cache;
    // Retain/release the cache. In theory the cache component
    // could be "borrowing" a cache instance owned by some parent,
    // in which case we could avoid retaining/releasing. But it
    // is non-trivial to determine when that is the case, so we
    // always retain/release.
    if !js::strict_equals(Some(next_cache), previous_cache) {
        retain_cache(next_cache);
        if let Some(previous_cache) = previous_cache {
            release_cache(previous_cache);
        }
    }
}

fn commit_tracing_marker_passive_mount_effect(finished_work: &Fiber) {
    // Get the transitions that were initiatized during the render
    // and add a start transition callback for each of them
    // We will only call this on initial mount of the tracing marker
    // only if there are no suspense children
    let instance: &TracingMarkerInstance = unsafe { js::cast(finished_work.state_node.get()) };
    if let Some(transitions) = instance.transitions.get()
        && instance.pending_boundaries.get().is_none()
    {
        add_marker_complete_callback_to_pending_transition(
            unsafe { js::cast(js::get(finished_work.memoized_props.get(), "name")) },
            transitions,
        );
        instance.transitions.set(js::null());
        instance.pending_boundaries.set(js::null());
        instance.aborts.replace(js::null());
        instance.name.replace(js::null());
    }
}

#[cfg_attr(rust_js, rust_js::nullable(committed_transitions))]
pub fn commit_passive_mount_effects(
    root: &FiberRoot,
    finished_work: &Rc<Fiber>,
    committed_lanes: Lanes,
    committed_transitions: CommittedTransitions,
    render_end_time: f64, // Profiling-only
) {
    reset_component_effect_timers();

    commit_passive_mount_on_fiber(
        root,
        finished_work,
        committed_lanes,
        committed_transitions,
        if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK {
            render_end_time
        } else {
            0.0
        },
    );
}

fn recursively_traverse_passive_mount_effects(
    root: &FiberRoot,
    parent_fiber: &Fiber,
    committed_lanes: Lanes,
    committed_transitions: CommittedTransitions,
    end_time: f64, // Profiling-only. The start time of the next Fiber or root completion.
) {
    let is_view_transition_eligible =
        ENABLE_VIEW_TRANSITION && includes_only_view_transition_eligible_lanes(committed_lanes);
    // TODO: We could optimize this by marking these with the Passive subtree flag in the render phase.
    let subtree_mask = if is_view_transition_eligible {
        PASSIVE_TRANSITION_MASK
    } else {
        PASSIVE_MASK
    };
    if (parent_fiber.subtree_flags.get() & subtree_mask) != 0
        // If this subtree rendered with profiling this commit, we need to visit it to log it.
        || (ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
            && ENABLE_COMPONENT_PERFORMANCE_TRACK
            && parent_fiber.actual_duration.get() != Some(0.0)
            && parent_fiber
                .alternate
                .borrow()
                .as_ref()
                .is_none_or(|alternate| *alternate.child.borrow() != *parent_fiber.child.borrow()))
    {
        let mut child = parent_fiber.child.borrow().clone();
        while let Some(current) = child {
            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK
            {
                let next_sibling = current.sibling.borrow().clone();
                commit_passive_mount_on_fiber(
                    root,
                    &current,
                    committed_lanes,
                    committed_transitions,
                    match &next_sibling {
                        Some(next_sibling) => next_sibling.actual_start_time.get().unwrap(),
                        None => end_time,
                    },
                );
                child = next_sibling;
            } else {
                commit_passive_mount_on_fiber(
                    root,
                    &current,
                    committed_lanes,
                    committed_transitions,
                    0.0,
                );
                child = current.sibling.borrow().clone();
            }
        }
    } else if is_view_transition_eligible {
        // We are inside an updated subtree. Any mutations that affected the
        // parent HostInstance's layout or set of children (such as reorders)
        // might have also affected the positioning or size of the inner
        // ViewTransitions. Therefore we need to restore those too.
        restore_nested_view_transitions(parent_fiber);
    }
}

thread_local! {
    static IN_HYDRATED_SUBTREE: Cell<bool> = const { Cell::new(false) };
}

fn commit_passive_mount_on_fiber(
    finished_root: &FiberRoot,
    finished_work: &Rc<Fiber>,
    committed_lanes: Lanes,
    committed_transitions: CommittedTransitions,
    end_time: f64, // Profiling-only. The start time of the next Fiber or root completion.
) {
    let prev_effect_start = push_component_effect_start();
    let prev_effect_duration = push_component_effect_duration();
    let prev_effect_errors = push_component_effect_errors();
    let prev_effect_did_spawn_update = push_component_effect_did_spawn_update();
    let prev_deep_equality = push_deep_equality();

    let is_view_transition_eligible = if ENABLE_VIEW_TRANSITION {
        includes_only_view_transition_eligible_lanes(committed_lanes)
    } else {
        false
    };

    if is_view_transition_eligible
        && finished_work.alternate.borrow().is_none()
        // We can't use the Placement flag here because it gets reset earlier. Instead,
        // we check if this is the root of the insertion by checking if the parent
        // was previous existing.
        && finished_work
            .r#return
            .borrow()
            .as_ref()
            .is_some_and(|return_fiber| return_fiber.alternate.borrow().is_some())
    {
        // This was a new mount. This means we could've triggered an enter animation on
        // the content. Restore the view transitions if there were any assigned in the
        // snapshot phase.
        restore_enter_or_exit_view_transitions(finished_work);
    }

    // When updating this function, also update reconnectPassiveEffects, which does
    // most of the same things when an offscreen tree goes from hidden -> visible,
    // or when toggling effects inside a hidden tree.
    let flags = finished_work.flags.get();
    match finished_work.tag.get() {
        FUNCTION_COMPONENT | FORWARD_REF | SIMPLE_MEMO_COMPONENT => {
            // If this component rendered in Profiling mode (DEV or in Profiler component) then log its
            // render time. We do this after the fact in the passive effect to avoid the overhead of this
            // getting in the way of the render characteristics and avoid the overhead of unwinding
            // uncommitted renders.
            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                && ENABLE_COMPONENT_PERFORMANCE_TRACK
                && (finished_work.mode.get() & PROFILE_MODE) != NO_MODE
                && finished_work.actual_start_time.get().unwrap() > 0.0
                && (finished_work.flags.get() & PERFORMED_WORK) != NO_FLAGS
            {
                log_component_render(
                    finished_work,
                    finished_work.actual_start_time.get().unwrap(),
                    end_time,
                    IN_HYDRATED_SUBTREE.get(),
                    committed_lanes,
                );
            }

            recursively_traverse_passive_mount_effects(
                finished_root,
                finished_work,
                committed_lanes,
                committed_transitions,
                end_time,
            );
            if (flags & PASSIVE) != 0 {
                commit_hook_passive_mount_effects(finished_work, HOOK_PASSIVE | HOOK_HAS_EFFECT);
            }
        }
        CLASS_COMPONENT => {
            // If this component rendered in Profiling mode (DEV or in Profiler component) then log its
            // render time. We do this after the fact in the passive effect to avoid the overhead of this
            // getting in the way of the render characteristics and avoid the overhead of unwinding
            // uncommitted renders.
            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                && ENABLE_COMPONENT_PERFORMANCE_TRACK
                && (finished_work.mode.get() & PROFILE_MODE) != NO_MODE
                && finished_work.actual_start_time.get().unwrap() > 0.0
            {
                if (finished_work.flags.get() & DID_CAPTURE) != NO_FLAGS {
                    log_component_errored(
                        finished_work,
                        finished_work.actual_start_time.get().unwrap(),
                        end_time,
                        // TODO: The captured values are all hidden inside the updater/callback closures so
                        // we can't get to the errors but they're there so we should be able to log them.
                        &RefCell::new(Vec::new()),
                    );
                } else if (finished_work.flags.get() & PERFORMED_WORK) != NO_FLAGS {
                    log_component_render(
                        finished_work,
                        finished_work.actual_start_time.get().unwrap(),
                        end_time,
                        IN_HYDRATED_SUBTREE.get(),
                        committed_lanes,
                    );
                }
            }

            recursively_traverse_passive_mount_effects(
                finished_root,
                finished_work,
                committed_lanes,
                committed_transitions,
                end_time,
            );
        }
        HOST_ROOT => {
            let prev_profiler_effect_duration = push_nested_effect_durations();

            let was_in_hydrated_subtree = IN_HYDRATED_SUBTREE.get();
            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK
            {
                // Detect if this was a hydration commit by look at if the previous state was
                // dehydrated and this wasn't a forced client render.
                IN_HYDRATED_SUBTREE.set(
                    finished_work
                        .alternate
                        .borrow()
                        .as_ref()
                        .is_some_and(|alternate| {
                            unsafe { js::cast::<&RootState>(alternate.memoized_state.get()) }
                                .is_dehydrated
                        })
                        && (finished_work.flags.get() & FORCE_CLIENT_RENDER) == NO_FLAGS,
                );
            }

            recursively_traverse_passive_mount_effects(
                finished_root,
                finished_work,
                committed_lanes,
                committed_transitions,
                end_time,
            );

            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK
            {
                IN_HYDRATED_SUBTREE.set(was_in_hydrated_subtree);
            }

            if is_view_transition_eligible
                && SUPPORTS_MUTATION
                && ROOT_VIEW_TRANSITION_NAME_CANCELED.get()
            {
                restore_root_view_transition_name(finished_root.container_info);
            }

            if (flags & PASSIVE) != 0 {
                let mut previous_cache: Option<&Rc<Cache>> = js::null();
                let alternate = finished_work.alternate.borrow().clone();
                if let Some(alternate) = &alternate {
                    previous_cache = Some(
                        &unsafe { js::cast::<&RootState>(alternate.memoized_state.get()) }.cache,
                    );
                }
                let next_cache =
                    &unsafe { js::cast::<&RootState>(finished_work.memoized_state.get()) }.cache;
                // Retain/release the root cache.
                // Note that on initial mount, previousCache and nextCache will be the same
                // and this retain won't occur. To counter this, we instead retain the HostRoot's
                // initial cache when creating the root itself (see createFiberRoot() in
                // ReactFiberRoot.js). Subsequent updates that change the cache are reflected
                // here, such that previous/next caches are retained correctly.
                if !js::strict_equals(Some(next_cache), previous_cache) {
                    retain_cache(next_cache);
                    if let Some(previous_cache) = previous_cache {
                        release_cache(previous_cache);
                    }
                }

                if ENABLE_TRANSITION_TRACING {
                    // Get the transitions that were initiatized during the render
                    // and add a start transition callback for each of them
                    let root: &FiberRoot = unsafe { js::cast(finished_work.state_node.get()) };
                    let incomplete_transitions = root.incomplete_transitions;
                    // Initial render
                    if let Some(committed_transitions) = committed_transitions {
                        committed_transitions.iter().for_each(|&transition| {
                            add_transition_start_callback_to_pending_transition(transition);
                        });

                        clear_transitions_for_lanes(finished_root, committed_lanes);
                    }

                    incomplete_transitions.for_each(|marker_instance, transition, _| {
                        let pending_boundaries = marker_instance.pending_boundaries.get();
                        if pending_boundaries
                            .is_none_or(|pending_boundaries| pending_boundaries.size() == 0)
                        {
                            if marker_instance.aborts.borrow().is_none() {
                                add_transition_complete_callback_to_pending_transition(transition);
                            }
                            incomplete_transitions.delete(transition);
                        }
                    });

                    clear_transitions_for_lanes(finished_root, committed_lanes);
                }
            }
            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                && ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
            {
                finished_root.passive_effect_duration.set(Some(
                    finished_root.passive_effect_duration.get().unwrap()
                        + pop_nested_effect_durations(prev_profiler_effect_duration),
                ));
            }
        }
        PROFILER => {
            // Only Profilers with work in their subtree will have a Passive effect scheduled.
            if (flags & PASSIVE) != 0 {
                let prev_profiler_effect_duration = push_nested_effect_durations();

                recursively_traverse_passive_mount_effects(
                    finished_root,
                    finished_work,
                    committed_lanes,
                    committed_transitions,
                    end_time,
                );

                let profiler_instance: &ProfilerStateNode =
                    unsafe { js::cast(finished_work.state_node.get()) };

                if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                    && ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
                {
                    // Bubble times to the next nearest ancestor Profiler.
                    // After we process that Profiler, we'll bubble further up.
                    profiler_instance.passive_effect_duration.set(
                        profiler_instance.passive_effect_duration.get()
                            + bubble_nested_effect_durations(prev_profiler_effect_duration),
                    );
                }

                let alternate = finished_work.alternate.borrow().clone();
                commit_profiler_post_commit(
                    finished_work,
                    alternate.as_deref(),
                    // This value will still reflect the previous commit phase.
                    // It does not get reset until the start of the next commit phase.
                    COMMIT_START_TIME.get(),
                    profiler_instance.passive_effect_duration.get(),
                );
            } else {
                recursively_traverse_passive_mount_effects(
                    finished_root,
                    finished_work,
                    committed_lanes,
                    committed_transitions,
                    end_time,
                );
            }
        }
        ACTIVITY_COMPONENT => {
            let was_in_hydrated_subtree = IN_HYDRATED_SUBTREE.get();
            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK
            {
                let alternate = finished_work.alternate.borrow().clone();
                let prev_state: Option<&ActivityState> = match &alternate {
                    Some(alternate) => unsafe { js::cast(alternate.memoized_state.get()) },
                    None => js::null(),
                };
                let next_state: Option<&ActivityState> =
                    unsafe { js::cast(finished_work.memoized_state.get()) };
                if let Some(prev_state) = prev_state
                    && next_state.is_none()
                {
                    // This was dehydrated but is no longer dehydrated. We may have now either hydrated it
                    // or client rendered it.
                    let deletions = finished_work.deletions.borrow().clone();
                    if let Some(deletions) = &deletions
                        && !deletions.borrow().is_empty()
                        && deletions.borrow()[0].tag.get() == DEHYDRATED_FRAGMENT
                    {
                        // This was an abandoned hydration that deleted the dehydrated fragment. That means we
                        // are not hydrating this Suspense boundary.
                        IN_HYDRATED_SUBTREE.set(false);
                        let hydration_errors = prev_state.hydration_errors.borrow().clone();
                        // If there were no hydration errors, that suggests that this was an intentional client
                        // rendered boundary.
                        if let Some(hydration_errors) = hydration_errors {
                            let start_time = finished_work.actual_start_time.get().unwrap();
                            log_component_errored(
                                finished_work,
                                start_time,
                                end_time,
                                &hydration_errors,
                            );
                        }
                    } else {
                        // If any children committed they were hydrated.
                        IN_HYDRATED_SUBTREE.set(true);
                    }
                } else {
                    IN_HYDRATED_SUBTREE.set(false);
                }
            }

            recursively_traverse_passive_mount_effects(
                finished_root,
                finished_work,
                committed_lanes,
                committed_transitions,
                end_time,
            );

            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK
            {
                IN_HYDRATED_SUBTREE.set(was_in_hydrated_subtree);
            }
        }
        SUSPENSE_COMPONENT => {
            let was_in_hydrated_subtree = IN_HYDRATED_SUBTREE.get();
            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK
            {
                let alternate = finished_work.alternate.borrow().clone();
                let prev_state: Option<&SuspenseState> = match &alternate {
                    Some(alternate) => unsafe { js::cast(alternate.memoized_state.get()) },
                    None => js::null(),
                };
                let next_state: Option<&SuspenseState> =
                    unsafe { js::cast(finished_work.memoized_state.get()) };
                if let Some(prev_state) = prev_state
                    && prev_state.dehydrated.is_some()
                    && next_state.is_none_or(|next_state| next_state.dehydrated.is_none())
                {
                    // This was dehydrated but is no longer dehydrated. We may have now either hydrated it
                    // or client rendered it.
                    let deletions = finished_work.deletions.borrow().clone();
                    if let Some(deletions) = &deletions
                        && !deletions.borrow().is_empty()
                        && deletions.borrow()[0].tag.get() == DEHYDRATED_FRAGMENT
                    {
                        // This was an abandoned hydration that deleted the dehydrated fragment. That means we
                        // are not hydrating this Suspense boundary.
                        IN_HYDRATED_SUBTREE.set(false);
                        let hydration_errors = prev_state.hydration_errors.borrow().clone();
                        // If there were no hydration errors, that suggests that this was an intentional client
                        // rendered boundary.
                        if let Some(hydration_errors) = hydration_errors {
                            let start_time = finished_work.actual_start_time.get().unwrap();
                            log_component_errored(
                                finished_work,
                                start_time,
                                end_time,
                                &hydration_errors,
                            );
                        }
                    } else {
                        // If any children committed they were hydrated.
                        IN_HYDRATED_SUBTREE.set(true);
                    }
                } else {
                    IN_HYDRATED_SUBTREE.set(false);
                }
            }

            recursively_traverse_passive_mount_effects(
                finished_root,
                finished_work,
                committed_lanes,
                committed_transitions,
                end_time,
            );

            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK
            {
                IN_HYDRATED_SUBTREE.set(was_in_hydrated_subtree);
            }
        }
        LEGACY_HIDDEN_COMPONENT => {
            if ENABLE_LEGACY_HIDDEN {
                recursively_traverse_passive_mount_effects(
                    finished_root,
                    finished_work,
                    committed_lanes,
                    committed_transitions,
                    end_time,
                );

                if (flags & PASSIVE) != 0 {
                    let current = finished_work.alternate.borrow().clone();
                    let instance: &OffscreenInstance =
                        unsafe { js::cast(finished_work.state_node.get()) };
                    commit_offscreen_passive_mount_effects(
                        current.as_deref(),
                        finished_work,
                        instance,
                    );
                }
            }
        }
        OFFSCREEN_COMPONENT => {
            // TODO: Pass `current` as argument to this function
            let instance: &OffscreenInstance = unsafe { js::cast(finished_work.state_node.get()) };
            let current = finished_work.alternate.borrow().clone();
            let next_state: Option<&OffscreenState> =
                unsafe { js::cast(finished_work.memoized_state.get()) };

            let is_hidden = next_state.is_some();

            if is_hidden {
                if is_view_transition_eligible
                    && let Some(current) = &current
                    && current.memoized_state.get().is_none()
                {
                    // Content is now hidden but wasn't before. This means we could've
                    // triggered an exit animation on the content. Restore the view
                    // transitions if there were any assigned in the snapshot phase.
                    restore_enter_or_exit_view_transitions(current);
                }
                if (instance.visibility.get() & OFFSCREEN_PASSIVE_EFFECTS_CONNECTED) != 0 {
                    // The effects are currently connected. Update them.
                    recursively_traverse_passive_mount_effects(
                        finished_root,
                        finished_work,
                        committed_lanes,
                        committed_transitions,
                        end_time,
                    );
                } else if DISABLE_LEGACY_MODE || (finished_work.mode.get() & CONCURRENT_MODE) != 0 {
                    // The effects are currently disconnected. Since the tree is hidden,
                    // don't connect them. This also applies to the initial render.
                    // "Atomic" effects are ones that need to fire on every commit,
                    // even during pre-rendering. An example is updating the reference
                    // count on cache instances.
                    recursively_traverse_atomic_passive_effects(
                        finished_root,
                        finished_work,
                        committed_lanes,
                        committed_transitions,
                        end_time,
                    );
                } else {
                    // Legacy Mode: Fire the effects even if the tree is hidden.
                    instance
                        .visibility
                        .set(instance.visibility.get() | OFFSCREEN_PASSIVE_EFFECTS_CONNECTED);
                    recursively_traverse_passive_mount_effects(
                        finished_root,
                        finished_work,
                        committed_lanes,
                        committed_transitions,
                        end_time,
                    );
                }
            } else {
                // Tree is visible
                if is_view_transition_eligible
                    && current
                        .as_ref()
                        .is_some_and(|current| current.memoized_state.get().is_some())
                {
                    // Content is now visible but wasn't before. This means we could've
                    // triggered an enter animation on the content. Restore the view
                    // transitions if there were any assigned in the snapshot phase.
                    restore_enter_or_exit_view_transitions(finished_work);
                }
                if (instance.visibility.get() & OFFSCREEN_PASSIVE_EFFECTS_CONNECTED) != 0 {
                    // The effects are currently connected. Update them.
                    recursively_traverse_passive_mount_effects(
                        finished_root,
                        finished_work,
                        committed_lanes,
                        committed_transitions,
                        end_time,
                    );
                } else {
                    // The effects are currently disconnected. Reconnect them, while also
                    // firing effects inside newly mounted trees. This also applies to
                    // the initial render.
                    instance
                        .visibility
                        .set(instance.visibility.get() | OFFSCREEN_PASSIVE_EFFECTS_CONNECTED);

                    let include_work_in_progress_effects =
                        (finished_work.subtree_flags.get() & PASSIVE_MASK) != NO_FLAGS
                            || (ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                                && ENABLE_COMPONENT_PERFORMANCE_TRACK
                                && finished_work.actual_duration.get() != Some(0.0)
                                && finished_work.alternate.borrow().as_ref().is_none_or(
                                    |alternate| {
                                        *alternate.child.borrow() != *finished_work.child.borrow()
                                    },
                                ));
                    recursively_traverse_reconnect_passive_effects(
                        finished_root,
                        finished_work,
                        committed_lanes,
                        committed_transitions,
                        include_work_in_progress_effects,
                        end_time,
                    );

                    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                        && ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
                        && ENABLE_COMPONENT_PERFORMANCE_TRACK
                        && (finished_work.mode.get() & PROFILE_MODE) != NO_MODE
                        && !IN_HYDRATED_SUBTREE.get()
                    {
                        // Log the reappear in the render phase.
                        let start_time = finished_work.actual_start_time.get().unwrap();
                        if start_time >= 0.0 && end_time - start_time > 0.05 {
                            log_component_reappeared(finished_work, start_time, end_time);
                        }
                        if COMPONENT_EFFECT_START_TIME.get() >= 0.0
                            && COMPONENT_EFFECT_END_TIME.get() >= 0.0
                            && COMPONENT_EFFECT_END_TIME.get() - COMPONENT_EFFECT_START_TIME.get()
                                > 0.05
                        {
                            log_component_reappeared(
                                finished_work,
                                COMPONENT_EFFECT_START_TIME.get(),
                                COMPONENT_EFFECT_END_TIME.get(),
                            );
                        }
                    }
                }
            }

            if (flags & PASSIVE) != 0 {
                commit_offscreen_passive_mount_effects(current.as_deref(), finished_work, instance);
            }
        }
        CACHE_COMPONENT => {
            recursively_traverse_passive_mount_effects(
                finished_root,
                finished_work,
                committed_lanes,
                committed_transitions,
                end_time,
            );
            if (flags & PASSIVE) != 0 {
                // TODO: Pass `current` as argument to this function
                let current = finished_work.alternate.borrow().clone();
                commit_cache_passive_mount_effect(current.as_deref(), finished_work);
            }
        }
        VIEW_TRANSITION_COMPONENT if ENABLE_VIEW_TRANSITION => {
            if is_view_transition_eligible {
                let current = finished_work.alternate.borrow().clone();
                match &current {
                    None => {
                        // This is a new mount. We should have handled this as part of the
                        // Placement effect or it is deeper inside a entering transition.
                    }
                    Some(current) => {
                        // Something mutated within this subtree. This might have caused
                        // something to cross-fade if we didn't already cancel it.
                        // If not, restore it.
                        restore_update_view_transition(current, finished_work);
                    }
                }
            }
            recursively_traverse_passive_mount_effects(
                finished_root,
                finished_work,
                committed_lanes,
                committed_transitions,
                end_time,
            );
        }
        // Fallthrough
        VIEW_TRANSITION_COMPONENT | TRACING_MARKER_COMPONENT if ENABLE_TRANSITION_TRACING => {
            recursively_traverse_passive_mount_effects(
                finished_root,
                finished_work,
                committed_lanes,
                committed_transitions,
                end_time,
            );
            if (flags & PASSIVE) != 0 {
                commit_tracing_marker_passive_mount_effect(finished_work);
            }
        }
        // Intentional fallthrough to next branch
        _ => {
            recursively_traverse_passive_mount_effects(
                finished_root,
                finished_work,
                committed_lanes,
                committed_transitions,
                end_time,
            );
        }
    }

    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        && ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
        && ENABLE_COMPONENT_PERFORMANCE_TRACK
        && (finished_work.mode.get() & PROFILE_MODE) != NO_MODE
    {
        let is_mount = !IN_HYDRATED_SUBTREE.get()
            && finished_work.alternate.borrow().is_none()
            && finished_work
                .r#return
                .borrow()
                .as_ref()
                .is_some_and(|return_fiber| return_fiber.alternate.borrow().is_some());
        if is_mount {
            // Log the mount in the render phase.
            let start_time = finished_work.actual_start_time.get().unwrap();
            if start_time >= 0.0 && end_time - start_time > 0.05 {
                log_component_mount(finished_work, start_time, end_time);
            }
        }
        if COMPONENT_EFFECT_START_TIME.get() >= 0.0 && COMPONENT_EFFECT_END_TIME.get() >= 0.0 {
            if COMPONENT_EFFECT_SPAWNED_UPDATE.get() || COMPONENT_EFFECT_DURATION.get() > 0.05 {
                log_component_effect(
                    finished_work,
                    COMPONENT_EFFECT_START_TIME.get(),
                    COMPONENT_EFFECT_END_TIME.get(),
                    COMPONENT_EFFECT_DURATION.get(),
                    COMPONENT_EFFECT_ERRORS.with_borrow(Clone::clone).as_deref(),
                );
            }
            if is_mount
                && COMPONENT_EFFECT_END_TIME.get() - COMPONENT_EFFECT_START_TIME.get() > 0.05
            {
                log_component_mount(
                    finished_work,
                    COMPONENT_EFFECT_START_TIME.get(),
                    COMPONENT_EFFECT_END_TIME.get(),
                );
            }
        }
    }

    pop_component_effect_start(prev_effect_start);
    pop_component_effect_duration(prev_effect_duration);
    pop_component_effect_errors(prev_effect_errors);
    pop_component_effect_did_spawn_update(prev_effect_did_spawn_update);
    pop_deep_equality(prev_deep_equality);
}

fn recursively_traverse_reconnect_passive_effects(
    finished_root: &FiberRoot,
    parent_fiber: &Fiber,
    committed_lanes: Lanes,
    committed_transitions: CommittedTransitions,
    include_work_in_progress_effects: bool,
    end_time: f64,
) {
    // This function visits both newly finished work and nodes that were re-used
    // from a previously committed tree. We cannot check non-static flags if the
    // node was reused.
    let child_should_include_work_in_progress_effects = include_work_in_progress_effects
        && ((parent_fiber.subtree_flags.get() & PASSIVE_MASK) != NO_FLAGS
            || (ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                && ENABLE_COMPONENT_PERFORMANCE_TRACK
                && parent_fiber.actual_duration.get() != Some(0.0)
                && parent_fiber
                    .alternate
                    .borrow()
                    .as_ref()
                    .is_none_or(|alternate| {
                        *alternate.child.borrow() != *parent_fiber.child.borrow()
                    })));

    // TODO (Offscreen) Check: flags & (RefStatic | LayoutStatic)
    let mut child = parent_fiber.child.borrow().clone();
    while let Some(current) = child {
        if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK {
            let next_sibling = current.sibling.borrow().clone();
            reconnect_passive_effects(
                finished_root,
                &current,
                committed_lanes,
                committed_transitions,
                child_should_include_work_in_progress_effects,
                match &next_sibling {
                    Some(next_sibling) => next_sibling.actual_start_time.get().unwrap(),
                    None => end_time,
                },
            );
            child = next_sibling;
        } else {
            reconnect_passive_effects(
                finished_root,
                &current,
                committed_lanes,
                committed_transitions,
                child_should_include_work_in_progress_effects,
                end_time,
            );
            child = current.sibling.borrow().clone();
        }
    }
}

#[cfg_attr(rust_js, rust_js::nullable(committed_transitions))]
pub fn reconnect_passive_effects(
    finished_root: &FiberRoot,
    finished_work: &Rc<Fiber>,
    committed_lanes: Lanes,
    committed_transitions: CommittedTransitions,
    // This function visits both newly finished work and nodes that were re-used
    // from a previously committed tree. We cannot check non-static flags if the
    // node was reused.
    include_work_in_progress_effects: bool,
    end_time: f64, // Profiling-only. The start time of the next Fiber or root completion.
) {
    let prev_effect_start = push_component_effect_start();
    let prev_effect_duration = push_component_effect_duration();
    let prev_effect_errors = push_component_effect_errors();
    let prev_effect_did_spawn_update = push_component_effect_did_spawn_update();
    let prev_deep_equality = push_deep_equality();

    // If this component rendered in Profiling mode (DEV or in Profiler component) then log its
    // render time. We do this after the fact in the passive effect to avoid the overhead of this
    // getting in the way of the render characteristics and avoid the overhead of unwinding
    // uncommitted renders.
    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        && ENABLE_COMPONENT_PERFORMANCE_TRACK
        && include_work_in_progress_effects
        && (finished_work.mode.get() & PROFILE_MODE) != NO_MODE
        && finished_work.actual_start_time.get().unwrap() > 0.0
        && (finished_work.flags.get() & PERFORMED_WORK) != NO_FLAGS
    {
        log_component_render(
            finished_work,
            finished_work.actual_start_time.get().unwrap(),
            end_time,
            IN_HYDRATED_SUBTREE.get(),
            committed_lanes,
        );
    }

    let flags = finished_work.flags.get();
    match finished_work.tag.get() {
        FUNCTION_COMPONENT | FORWARD_REF | SIMPLE_MEMO_COMPONENT => {
            recursively_traverse_reconnect_passive_effects(
                finished_root,
                finished_work,
                committed_lanes,
                committed_transitions,
                include_work_in_progress_effects,
                end_time,
            );
            // TODO: Check for PassiveStatic flag
            commit_hook_passive_mount_effects(finished_work, HOOK_PASSIVE);
        }
        // Unlike commitPassiveMountOnFiber, we don't need to handle HostRoot
        // because this function only visits nodes that are inside an
        // Offscreen fiber.
        // case HostRoot: {
        //  ...
        // }
        LEGACY_HIDDEN_COMPONENT => {
            if ENABLE_LEGACY_HIDDEN {
                recursively_traverse_reconnect_passive_effects(
                    finished_root,
                    finished_work,
                    committed_lanes,
                    committed_transitions,
                    include_work_in_progress_effects,
                    end_time,
                );
                if include_work_in_progress_effects && (flags & PASSIVE) != 0 {
                    // TODO: Pass `current` as argument to this function
                    let current = finished_work.alternate.borrow().clone();
                    let instance: &OffscreenInstance =
                        unsafe { js::cast(finished_work.state_node.get()) };
                    commit_offscreen_passive_mount_effects(
                        current.as_deref(),
                        finished_work,
                        instance,
                    );
                }
            }
        }
        OFFSCREEN_COMPONENT => {
            let instance: &OffscreenInstance = unsafe { js::cast(finished_work.state_node.get()) };
            let next_state: Option<&OffscreenState> =
                unsafe { js::cast(finished_work.memoized_state.get()) };

            let is_hidden = next_state.is_some();

            if is_hidden {
                if (instance.visibility.get() & OFFSCREEN_PASSIVE_EFFECTS_CONNECTED) != 0 {
                    // The effects are currently connected. Update them.
                    recursively_traverse_reconnect_passive_effects(
                        finished_root,
                        finished_work,
                        committed_lanes,
                        committed_transitions,
                        include_work_in_progress_effects,
                        end_time,
                    );
                } else if DISABLE_LEGACY_MODE || (finished_work.mode.get() & CONCURRENT_MODE) != 0 {
                    // The effects are currently disconnected. Since the tree is hidden,
                    // don't connect them. This also applies to the initial render.
                    // "Atomic" effects are ones that need to fire on every commit,
                    // even during pre-rendering. An example is updating the reference
                    // count on cache instances.
                    recursively_traverse_atomic_passive_effects(
                        finished_root,
                        finished_work,
                        committed_lanes,
                        committed_transitions,
                        end_time,
                    );
                } else {
                    // Legacy Mode: Fire the effects even if the tree is hidden.
                    instance
                        .visibility
                        .set(instance.visibility.get() | OFFSCREEN_PASSIVE_EFFECTS_CONNECTED);
                    recursively_traverse_reconnect_passive_effects(
                        finished_root,
                        finished_work,
                        committed_lanes,
                        committed_transitions,
                        include_work_in_progress_effects,
                        end_time,
                    );
                }
            } else {
                // Tree is visible
                // Since we're already inside a reconnecting tree, it doesn't matter
                // whether the effects are currently connected. In either case, we'll
                // continue traversing the tree and firing all the effects.
                //
                // We do need to set the "connected" flag on the instance, though.
                instance
                    .visibility
                    .set(instance.visibility.get() | OFFSCREEN_PASSIVE_EFFECTS_CONNECTED);
                recursively_traverse_reconnect_passive_effects(
                    finished_root,
                    finished_work,
                    committed_lanes,
                    committed_transitions,
                    include_work_in_progress_effects,
                    end_time,
                );
            }
            if include_work_in_progress_effects && (flags & PASSIVE) != 0 {
                // TODO: Pass `current` as argument to this function
                let current = finished_work.alternate.borrow().clone();
                commit_offscreen_passive_mount_effects(current.as_deref(), finished_work, instance);
            }
        }
        CACHE_COMPONENT => {
            recursively_traverse_reconnect_passive_effects(
                finished_root,
                finished_work,
                committed_lanes,
                committed_transitions,
                include_work_in_progress_effects,
                end_time,
            );
            if include_work_in_progress_effects && (flags & PASSIVE) != 0 {
                // TODO: Pass `current` as argument to this function
                let current = finished_work.alternate.borrow().clone();
                commit_cache_passive_mount_effect(current.as_deref(), finished_work);
            }
        }
        TRACING_MARKER_COMPONENT if ENABLE_TRANSITION_TRACING => {
            recursively_traverse_reconnect_passive_effects(
                finished_root,
                finished_work,
                committed_lanes,
                committed_transitions,
                include_work_in_progress_effects,
                end_time,
            );
            if include_work_in_progress_effects && (flags & PASSIVE) != 0 {
                commit_tracing_marker_passive_mount_effect(finished_work);
            }
        }
        // Intentional fallthrough to next branch
        _ => {
            recursively_traverse_reconnect_passive_effects(
                finished_root,
                finished_work,
                committed_lanes,
                committed_transitions,
                include_work_in_progress_effects,
                end_time,
            );
        }
    }

    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        && ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
        && ENABLE_COMPONENT_PERFORMANCE_TRACK
        && (finished_work.mode.get() & PROFILE_MODE) != NO_MODE
        && COMPONENT_EFFECT_START_TIME.get() >= 0.0
        && COMPONENT_EFFECT_END_TIME.get() >= 0.0
        && (COMPONENT_EFFECT_SPAWNED_UPDATE.get() || COMPONENT_EFFECT_DURATION.get() > 0.05)
    {
        log_component_effect(
            finished_work,
            COMPONENT_EFFECT_START_TIME.get(),
            COMPONENT_EFFECT_END_TIME.get(),
            COMPONENT_EFFECT_DURATION.get(),
            COMPONENT_EFFECT_ERRORS.with_borrow(Clone::clone).as_deref(),
        );
    }

    pop_component_effect_start(prev_effect_start);
    pop_component_effect_duration(prev_effect_duration);
    pop_component_effect_errors(prev_effect_errors);
    pop_component_effect_did_spawn_update(prev_effect_did_spawn_update);
    pop_deep_equality(prev_deep_equality);
}

fn recursively_traverse_atomic_passive_effects(
    finished_root: &FiberRoot,
    parent_fiber: &Fiber,
    committed_lanes: Lanes,
    committed_transitions: CommittedTransitions,
    end_time: f64, // Profiling-only. The start time of the next Fiber or root completion.
) {
    // "Atomic" effects are ones that need to fire on every commit, even during
    // pre-rendering. We call this function when traversing a hidden tree whose
    // regular effects are currently disconnected.
    // TODO: Add special flag for atomic effects
    if (parent_fiber.subtree_flags.get() & PASSIVE_MASK) != 0
        || (ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
            && ENABLE_COMPONENT_PERFORMANCE_TRACK
            && parent_fiber.actual_duration.get() != Some(0.0)
            && parent_fiber
                .alternate
                .borrow()
                .as_ref()
                .is_none_or(|alternate| *alternate.child.borrow() != *parent_fiber.child.borrow()))
    {
        let mut child = parent_fiber.child.borrow().clone();
        while let Some(current) = child {
            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK
            {
                let next_sibling = current.sibling.borrow().clone();
                commit_atomic_passive_effects(
                    finished_root,
                    &current,
                    committed_lanes,
                    committed_transitions,
                    match &next_sibling {
                        Some(next_sibling) => next_sibling.actual_start_time.get().unwrap(),
                        None => end_time,
                    },
                );
                child = next_sibling;
            } else {
                commit_atomic_passive_effects(
                    finished_root,
                    &current,
                    committed_lanes,
                    committed_transitions,
                    end_time,
                );
                child = current.sibling.borrow().clone();
            }
        }
    }
}

fn commit_atomic_passive_effects(
    finished_root: &FiberRoot,
    finished_work: &Rc<Fiber>,
    committed_lanes: Lanes,
    committed_transitions: CommittedTransitions,
    end_time: f64, // Profiling-only. The start time of the next Fiber or root completion.
) {
    let prev_deep_equality = push_deep_equality();
    // If this component rendered in Profiling mode (DEV or in Profiler component) then log its
    // render time. A render can happen even if the subtree is offscreen.
    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        && ENABLE_COMPONENT_PERFORMANCE_TRACK
        && (finished_work.mode.get() & PROFILE_MODE) != NO_MODE
        && finished_work.actual_start_time.get().unwrap() > 0.0
        && (finished_work.flags.get() & PERFORMED_WORK) != NO_FLAGS
    {
        log_component_render(
            finished_work,
            finished_work.actual_start_time.get().unwrap(),
            end_time,
            IN_HYDRATED_SUBTREE.get(),
            committed_lanes,
        );
    }

    // "Atomic" effects are ones that need to fire on every commit, even during
    // pre-rendering. We call this function when traversing a hidden tree whose
    // regular effects are currently disconnected.
    let flags = finished_work.flags.get();
    match finished_work.tag.get() {
        OFFSCREEN_COMPONENT => {
            recursively_traverse_atomic_passive_effects(
                finished_root,
                finished_work,
                committed_lanes,
                committed_transitions,
                end_time,
            );
            if (flags & PASSIVE) != 0 {
                // TODO: Pass `current` as argument to this function
                let current = finished_work.alternate.borrow().clone();
                let instance: &OffscreenInstance =
                    unsafe { js::cast(finished_work.state_node.get()) };
                commit_offscreen_passive_mount_effects(current.as_deref(), finished_work, instance);
            }
        }
        CACHE_COMPONENT => {
            recursively_traverse_atomic_passive_effects(
                finished_root,
                finished_work,
                committed_lanes,
                committed_transitions,
                end_time,
            );
            if (flags & PASSIVE) != 0 {
                // TODO: Pass `current` as argument to this function
                let current = finished_work.alternate.borrow().clone();
                commit_cache_passive_mount_effect(current.as_deref(), finished_work);
            }
        }
        _ => {
            recursively_traverse_atomic_passive_effects(
                finished_root,
                finished_work,
                committed_lanes,
                committed_transitions,
                end_time,
            );
        }
    }
    pop_deep_equality(prev_deep_equality);
}

pub fn commit_passive_unmount_effects(finished_work: &Rc<Fiber>) {
    reset_component_effect_timers();
    commit_passive_unmount_on_fiber(finished_work);
}

thread_local! {
    // If we're inside a brand new tree, or a tree that was already visible, then we
    // should only suspend host components that have a ShouldSuspendCommit flag.
    // Components without it haven't changed since the last commit, so we can skip
    // over those.
    //
    // When we enter a tree that is being revealed (going from hidden -> visible),
    // we need to suspend _any_ component that _may_ suspend. Even if they're
    // already in the "current" tree. Because their visibility has changed, the
    // browser may not have prerendered them yet. So we check the MaySuspendCommit
    // flag instead.
    //
    // Note that MaySuspendCommit and ShouldSuspendCommit also includes named
    // ViewTransitions so that we know to also visit those to collect appearing
    // pairs.
    static SUSPENSEY_COMMIT_FLAG: Cell<Flags> = const { Cell::new(SHOULD_SUSPEND_COMMIT) };
}

// ReactFiberConfig's `SuspendedState`.
pub fn accumulate_suspensey_commit(
    finished_work: &Rc<Fiber>,
    committed_lanes: Lanes,
    suspended_state: &'static Unknown,
) {
    reset_appearing_view_transitions();
    accumulate_suspensey_commit_on_fiber(finished_work, committed_lanes, suspended_state);
}

fn recursively_accumulate_suspensey_commit(
    parent_fiber: &Fiber,
    committed_lanes: Lanes,
    suspended_state: &'static Unknown,
) {
    if (parent_fiber.subtree_flags.get() & SUSPENSEY_COMMIT_FLAG.get()) != 0 {
        let mut child = parent_fiber.child.borrow().clone();
        while let Some(current) = child {
            accumulate_suspensey_commit_on_fiber(&current, committed_lanes, suspended_state);
            child = current.sibling.borrow().clone();
        }
    }
}

fn accumulate_suspensey_commit_on_fiber(
    fiber: &Rc<Fiber>,
    committed_lanes: Lanes,
    suspended_state: &'static Unknown,
) {
    match fiber.tag.get() {
        HOST_HOISTABLE => {
            recursively_accumulate_suspensey_commit(fiber, committed_lanes, suspended_state);
            if (fiber.flags.get() & SUSPENSEY_COMMIT_FLAG.get()) != 0 {
                if let Some(memoized_state) = fiber.memoized_state.get() {
                    suspend_resource(
                        suspended_state,
                        // This should always be set by visiting HostRoot first
                        unsafe { js::cast(CURRENT_HOISTABLE_ROOT.get()) },
                        memoized_state,
                        fiber.memoized_props.get(),
                    );
                } else {
                    let instance = fiber.state_node.get();
                    let r#type = fiber.r#type.get();
                    let props = fiber.memoized_props.get();
                    // TODO: Allow sync lanes to suspend too with an opt-in.
                    if includes_only_suspensey_commit_eligible_lanes(committed_lanes)
                        || may_suspend_commit_in_sync_render(r#type, props)
                    {
                        suspend_instance(suspended_state, instance, r#type, props);
                    }
                }
            }
        }
        HOST_COMPONENT => {
            recursively_accumulate_suspensey_commit(fiber, committed_lanes, suspended_state);
            if (fiber.flags.get() & SUSPENSEY_COMMIT_FLAG.get()) != 0 {
                let instance = fiber.state_node.get();
                let r#type = fiber.r#type.get();
                let props = fiber.memoized_props.get();
                // TODO: Allow sync lanes to suspend too with an opt-in.
                if includes_only_suspensey_commit_eligible_lanes(committed_lanes)
                    || may_suspend_commit_in_sync_render(r#type, props)
                {
                    suspend_instance(suspended_state, instance, r#type, props);
                }
            }
        }
        HOST_ROOT | HOST_PORTAL => {
            if SUPPORTS_RESOURCES {
                let previous_hoistable_root = CURRENT_HOISTABLE_ROOT.get();
                // ReactFiberConfig's `Container`.
                let container: &'static Unknown =
                    unsafe { js::cast(js::get(fiber.state_node.get(), "containerInfo")) };
                CURRENT_HOISTABLE_ROOT.set(Some(get_hoistable_root(container)));
                recursively_accumulate_suspensey_commit(fiber, committed_lanes, suspended_state);
                CURRENT_HOISTABLE_ROOT.set(previous_hoistable_root);
            } else {
                recursively_accumulate_suspensey_commit(fiber, committed_lanes, suspended_state);
            }
        }
        OFFSCREEN_COMPONENT => {
            let is_hidden = fiber.memoized_state.get().is_some();
            if is_hidden {
                // Don't suspend in hidden trees
            } else {
                let current = fiber.alternate.borrow().clone();
                let was_hidden = current
                    .as_ref()
                    .is_some_and(|current| current.memoized_state.get().is_some());
                if was_hidden {
                    // This tree is being revealed. Visit all newly visible suspensey
                    // instances, even if they're in the current tree.
                    let prev_flags = SUSPENSEY_COMMIT_FLAG.get();
                    SUSPENSEY_COMMIT_FLAG.set(MAY_SUSPEND_COMMIT);
                    recursively_accumulate_suspensey_commit(
                        fiber,
                        committed_lanes,
                        suspended_state,
                    );
                    SUSPENSEY_COMMIT_FLAG.set(prev_flags);
                } else {
                    recursively_accumulate_suspensey_commit(
                        fiber,
                        committed_lanes,
                        suspended_state,
                    );
                }
            }
        }
        VIEW_TRANSITION_COMPONENT if ENABLE_VIEW_TRANSITION => {
            if (fiber.flags.get() & SUSPENSEY_COMMIT_FLAG.get()) != NO_FLAGS {
                let props: &ViewTransitionProps = unsafe { js::cast(fiber.memoized_props.get()) };
                let name = &props.name;
                if let Some(name) = name
                    && name != "auto"
                {
                    // This is a named ViewTransition being mounted or reappearing. Let's add it to
                    // the map so we can match it with deletions later.
                    let state: Rc<ViewTransitionState> =
                        unsafe { js::cast(fiber.state_node.get()) };
                    // Reset the pair in case we didn't end up restoring the instance in previous commits.
                    // This shouldn't really happen anymore but just in case. We could maybe add an invariant.
                    state.paired.replace(js::null());
                    track_appearing_view_transition(name.clone(), state);
                }
            }
            recursively_accumulate_suspensey_commit(fiber, committed_lanes, suspended_state);
        }
        // Fallthrough
        _ => {
            recursively_accumulate_suspensey_commit(fiber, committed_lanes, suspended_state);
        }
    }
}

fn detach_alternate_siblings(parent_fiber: &Fiber) {
    // A fiber was deleted from this parent fiber, but it's still part of the
    // previous (alternate) parent fiber's list of children. Because children
    // are a linked list, an earlier sibling that's still alive will be
    // connected to the deleted fiber via its `alternate`:
    //
    //   live fiber --alternate--> previous live fiber --sibling--> deleted
    //   fiber
    //
    // We can't disconnect `alternate` on nodes that haven't been deleted yet,
    // but we can disconnect the `sibling` and `child` pointers.
    let previous_fiber = parent_fiber.alternate.borrow().clone();
    if let Some(previous_fiber) = previous_fiber {
        let mut detached_child = previous_fiber.child.borrow().clone();
        if detached_child.is_some() {
            previous_fiber.child.replace(js::null());
            while let Some(current) = detached_child {
                let detached_sibling = current.sibling.borrow().clone();
                current.sibling.replace(js::null());
                detached_child = detached_sibling;
            }
        }
    }
}

fn recursively_traverse_passive_unmount_effects(parent_fiber: &Rc<Fiber>) {
    // Deletions effects can be scheduled on any fiber type. They need to happen
    // before the children effects have fired.
    let deletions = parent_fiber.deletions.borrow().clone();
    if (parent_fiber.flags.get() & CHILD_DELETION) != NO_FLAGS {
        if let Some(deletions) = &deletions {
            let mut i = 0;
            while i < deletions.borrow().len() {
                let child_to_delete = Rc::clone(&deletions.borrow()[i]);
                let prev_effect_start = push_component_effect_start();
                // TODO: Convert this to use recursion
                NEXT_EFFECT.set(Some(Rc::clone(&child_to_delete)));
                commit_passive_unmount_effects_inside_of_deleted_tree_begin(
                    &child_to_delete,
                    Some(parent_fiber),
                );
                if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                    && ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
                    && ENABLE_COMPONENT_PERFORMANCE_TRACK
                    && (child_to_delete.mode.get() & PROFILE_MODE) != NO_MODE
                    && COMPONENT_EFFECT_START_TIME.get() >= 0.0
                    && COMPONENT_EFFECT_END_TIME.get() >= 0.0
                    && COMPONENT_EFFECT_END_TIME.get() - COMPONENT_EFFECT_START_TIME.get() > 0.05
                {
                    log_component_unmount(
                        &child_to_delete,
                        COMPONENT_EFFECT_START_TIME.get(),
                        COMPONENT_EFFECT_END_TIME.get(),
                    );
                }
                pop_component_effect_start(prev_effect_start);
                i += 1;
            }
        }
        detach_alternate_siblings(parent_fiber);
    }

    // TODO: Split PassiveMask into separate masks for mount and unmount?
    if (parent_fiber.subtree_flags.get() & PASSIVE_MASK) != 0 {
        let mut child = parent_fiber.child.borrow().clone();
        while let Some(current) = child {
            commit_passive_unmount_on_fiber(&current);
            child = current.sibling.borrow().clone();
        }
    }
}

fn commit_passive_unmount_on_fiber(finished_work: &Rc<Fiber>) {
    let prev_effect_start = push_component_effect_start();
    let prev_effect_duration = push_component_effect_duration();
    let prev_effect_errors = push_component_effect_errors();
    let prev_effect_did_spawn_update = push_component_effect_did_spawn_update();
    match finished_work.tag.get() {
        FUNCTION_COMPONENT | FORWARD_REF | SIMPLE_MEMO_COMPONENT => {
            recursively_traverse_passive_unmount_effects(finished_work);
            if (finished_work.flags.get() & PASSIVE) != 0 {
                let return_fiber = finished_work.r#return.borrow().clone();
                commit_hook_passive_unmount_effects(
                    finished_work,
                    return_fiber.as_ref(),
                    HOOK_PASSIVE | HOOK_HAS_EFFECT,
                );
            }
        }
        HOST_ROOT => {
            let prev_profiler_effect_duration = push_nested_effect_durations();
            recursively_traverse_passive_unmount_effects(finished_work);
            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                && ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
            {
                let finished_root: &FiberRoot = unsafe { js::cast(finished_work.state_node.get()) };
                finished_root.passive_effect_duration.set(Some(
                    finished_root.passive_effect_duration.get().unwrap()
                        + pop_nested_effect_durations(prev_profiler_effect_duration),
                ));
            }
        }
        PROFILER => {
            let prev_profiler_effect_duration = push_nested_effect_durations();
            recursively_traverse_passive_unmount_effects(finished_work);
            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                && ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
            {
                let profiler_instance: &ProfilerStateNode =
                    unsafe { js::cast(finished_work.state_node.get()) };
                // Propagate layout effect durations to the next nearest Profiler ancestor.
                // Do not reset these values until the next render so DevTools has a chance to read them first.
                profiler_instance.passive_effect_duration.set(
                    profiler_instance.passive_effect_duration.get()
                        + bubble_nested_effect_durations(prev_profiler_effect_duration),
                );
            }
        }
        OFFSCREEN_COMPONENT => {
            let instance: &OffscreenInstance = unsafe { js::cast(finished_work.state_node.get()) };
            let next_state: Option<&OffscreenState> =
                unsafe { js::cast(finished_work.memoized_state.get()) };

            let is_hidden = next_state.is_some();

            if is_hidden
                && (instance.visibility.get() & OFFSCREEN_PASSIVE_EFFECTS_CONNECTED) != 0
                // For backwards compatibility, don't unmount when a tree suspends. In
                // the future we may change this to unmount after a delay.
                && finished_work
                    .r#return
                    .borrow()
                    .as_ref()
                    .is_none_or(|return_fiber| return_fiber.tag.get() != SUSPENSE_COMPONENT)
            {
                // The effects are currently connected. Disconnect them.
                // TODO: Add option or heuristic to delay before disconnecting the
                // effects. Then if the tree reappears before the delay has elapsed, we
                // can skip toggling the effects entirely.
                instance
                    .visibility
                    .set(instance.visibility.get() & !OFFSCREEN_PASSIVE_EFFECTS_CONNECTED);
                recursively_traverse_disconnect_passive_effects(finished_work);
                if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                    && ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
                    && ENABLE_COMPONENT_PERFORMANCE_TRACK
                    && (finished_work.mode.get() & PROFILE_MODE) != NO_MODE
                    && COMPONENT_EFFECT_START_TIME.get() >= 0.0
                    && COMPONENT_EFFECT_END_TIME.get() >= 0.0
                    && COMPONENT_EFFECT_END_TIME.get() - COMPONENT_EFFECT_START_TIME.get() > 0.05
                {
                    log_component_disappeared(
                        finished_work,
                        COMPONENT_EFFECT_START_TIME.get(),
                        COMPONENT_EFFECT_END_TIME.get(),
                    );
                }
            } else {
                recursively_traverse_passive_unmount_effects(finished_work);
            }
        }
        _ => {
            recursively_traverse_passive_unmount_effects(finished_work);
        }
    }

    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        && ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
        && ENABLE_COMPONENT_PERFORMANCE_TRACK
        && (finished_work.mode.get() & PROFILE_MODE) != NO_MODE
        && COMPONENT_EFFECT_START_TIME.get() >= 0.0
        && COMPONENT_EFFECT_END_TIME.get() >= 0.0
        && (COMPONENT_EFFECT_SPAWNED_UPDATE.get() || COMPONENT_EFFECT_DURATION.get() > 0.05)
    {
        log_component_effect(
            finished_work,
            COMPONENT_EFFECT_START_TIME.get(),
            COMPONENT_EFFECT_END_TIME.get(),
            COMPONENT_EFFECT_DURATION.get(),
            COMPONENT_EFFECT_ERRORS.with_borrow(Clone::clone).as_deref(),
        );
    }

    pop_component_effect_start(prev_effect_start);
    pop_component_effect_duration(prev_effect_duration);
    pop_component_effect_did_spawn_update(prev_effect_did_spawn_update);
    pop_component_effect_errors(prev_effect_errors);
}

fn recursively_traverse_disconnect_passive_effects(parent_fiber: &Rc<Fiber>) {
    // Deletions effects can be scheduled on any fiber type. They need to happen
    // before the children effects have fired.
    let deletions = parent_fiber.deletions.borrow().clone();
    if (parent_fiber.flags.get() & CHILD_DELETION) != NO_FLAGS {
        if let Some(deletions) = &deletions {
            let mut i = 0;
            while i < deletions.borrow().len() {
                let child_to_delete = Rc::clone(&deletions.borrow()[i]);
                let prev_effect_start = push_component_effect_start();
                // TODO: Convert this to use recursion
                NEXT_EFFECT.set(Some(Rc::clone(&child_to_delete)));
                commit_passive_unmount_effects_inside_of_deleted_tree_begin(
                    &child_to_delete,
                    Some(parent_fiber),
                );
                if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                    && ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
                    && ENABLE_COMPONENT_PERFORMANCE_TRACK
                    && (child_to_delete.mode.get() & PROFILE_MODE) != NO_MODE
                    && COMPONENT_EFFECT_START_TIME.get() >= 0.0
                    && COMPONENT_EFFECT_END_TIME.get() >= 0.0
                    && COMPONENT_EFFECT_END_TIME.get() - COMPONENT_EFFECT_START_TIME.get() > 0.05
                {
                    // While this is inside the disconnect path. This is a deletion within the
                    // disconnected tree. We currently log this for deletions in the mutation
                    // phase since it's shared by the disappear path.
                    log_component_unmount(
                        &child_to_delete,
                        COMPONENT_EFFECT_START_TIME.get(),
                        COMPONENT_EFFECT_END_TIME.get(),
                    );
                }
                pop_component_effect_start(prev_effect_start);
                i += 1;
            }
        }
        detach_alternate_siblings(parent_fiber);
    }

    // TODO: Check PassiveStatic flag
    let mut child = parent_fiber.child.borrow().clone();
    while let Some(current) = child {
        disconnect_passive_effect(&current);
        child = current.sibling.borrow().clone();
    }
}

pub fn disconnect_passive_effect(finished_work: &Rc<Fiber>) {
    let prev_effect_start = push_component_effect_start();
    let prev_effect_duration = push_component_effect_duration();
    let prev_effect_errors = push_component_effect_errors();
    let prev_effect_did_spawn_update = push_component_effect_did_spawn_update();

    match finished_work.tag.get() {
        FUNCTION_COMPONENT | FORWARD_REF | SIMPLE_MEMO_COMPONENT => {
            // TODO: Check PassiveStatic flag
            let return_fiber = finished_work.r#return.borrow().clone();
            commit_hook_passive_unmount_effects(finished_work, return_fiber.as_ref(), HOOK_PASSIVE);
            // When disconnecting passive effects, we fire the effects in the same
            // order as during a deletiong: parent before child
            recursively_traverse_disconnect_passive_effects(finished_work);
        }
        OFFSCREEN_COMPONENT => {
            let instance: &OffscreenInstance = unsafe { js::cast(finished_work.state_node.get()) };
            if (instance.visibility.get() & OFFSCREEN_PASSIVE_EFFECTS_CONNECTED) != 0 {
                instance
                    .visibility
                    .set(instance.visibility.get() & !OFFSCREEN_PASSIVE_EFFECTS_CONNECTED);
                recursively_traverse_disconnect_passive_effects(finished_work);
            } else {
                // The effects are already disconnected.
            }
        }
        _ => {
            recursively_traverse_disconnect_passive_effects(finished_work);
        }
    }

    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        && ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
        && ENABLE_COMPONENT_PERFORMANCE_TRACK
        && (finished_work.mode.get() & PROFILE_MODE) != NO_MODE
        && COMPONENT_EFFECT_START_TIME.get() >= 0.0
        && COMPONENT_EFFECT_END_TIME.get() >= 0.0
        && (COMPONENT_EFFECT_SPAWNED_UPDATE.get() || COMPONENT_EFFECT_DURATION.get() > 0.05)
    {
        log_component_effect(
            finished_work,
            COMPONENT_EFFECT_START_TIME.get(),
            COMPONENT_EFFECT_END_TIME.get(),
            COMPONENT_EFFECT_DURATION.get(),
            COMPONENT_EFFECT_ERRORS.with_borrow(Clone::clone).as_deref(),
        );
    }

    pop_component_effect_start(prev_effect_start);
    pop_component_effect_duration(prev_effect_duration);
    pop_component_effect_did_spawn_update(prev_effect_did_spawn_update);
    pop_component_effect_errors(prev_effect_errors);
}

#[cfg_attr(
    rust_js,
    rust_js::name = "commitPassiveUnmountEffectsInsideOfDeletedTree_begin"
)]
fn commit_passive_unmount_effects_inside_of_deleted_tree_begin(
    deleted_subtree_root: &Fiber,
    nearest_mounted_ancestor: Option<&Rc<Fiber>>,
) {
    while let Some(fiber) = NEXT_EFFECT.with_borrow(Clone::clone) {
        // Deletion effects fire in parent -> child order
        // TODO: Check if fiber has a PassiveStatic flag
        commit_passive_unmount_inside_deleted_tree_on_fiber(&fiber, nearest_mounted_ancestor);

        let child = fiber.child.borrow().clone();
        // TODO: Only traverse subtree if it has a PassiveStatic flag.
        if let Some(child) = child {
            child.r#return.replace(Some(Rc::clone(&fiber)));
            NEXT_EFFECT.set(Some(child));
        } else {
            commit_passive_unmount_effects_inside_of_deleted_tree_complete(deleted_subtree_root);
        }
    }
}

#[cfg_attr(
    rust_js,
    rust_js::name = "commitPassiveUnmountEffectsInsideOfDeletedTree_complete"
)]
fn commit_passive_unmount_effects_inside_of_deleted_tree_complete(deleted_subtree_root: &Fiber) {
    while let Some(fiber) = NEXT_EFFECT.with_borrow(Clone::clone) {
        let sibling = fiber.sibling.borrow().clone();
        let return_fiber = fiber.r#return.borrow().clone();

        // Recursively traverse the entire deleted tree and clean up fiber fields.
        // This is more aggressive than ideal, and the long term goal is to only
        // have to detach the deleted tree at the root.
        detach_fiber_after_effects(&fiber);

        if *fiber == *deleted_subtree_root {
            NEXT_EFFECT.set(js::null());
            return;
        }

        if let Some(sibling) = sibling {
            sibling.r#return.replace(return_fiber);
            NEXT_EFFECT.set(Some(sibling));
            return;
        }

        NEXT_EFFECT.set(return_fiber);
    }
}

fn commit_passive_unmount_inside_deleted_tree_on_fiber(
    current: &Rc<Fiber>,
    nearest_mounted_ancestor: Option<&Rc<Fiber>>,
) {
    let prev_effect_start = push_component_effect_start();
    let prev_effect_duration = push_component_effect_duration();
    let prev_effect_errors = push_component_effect_errors();
    let prev_effect_did_spawn_update = push_component_effect_did_spawn_update();

    match current.tag.get() {
        FUNCTION_COMPONENT | FORWARD_REF | SIMPLE_MEMO_COMPONENT => {
            commit_hook_passive_unmount_effects(current, nearest_mounted_ancestor, HOOK_PASSIVE);
        }
        // TODO: run passive unmount effects when unmounting a root.
        // Because passive unmount effects are not currently run,
        // the cache instance owned by the root will never be freed.
        // When effects are run, the cache should be freed here:
        // case HostRoot: {
        //   const cache = current.memoizedState.cache;
        //   releaseCache(cache);
        //   break;
        // }
        LEGACY_HIDDEN_COMPONENT | OFFSCREEN_COMPONENT => {
            if let Some(memoized_state) =
                unsafe { js::cast::<Option<&OffscreenState>>(current.memoized_state.get()) }
                && let Some(cache_pool) =
                    unsafe { js::cast::<Option<&SpawnedCachePool>>(memoized_state.cache_pool) }
            {
                let cache = &cache_pool.pool;
                // Retain/release the cache used for pending (suspended) nodes.
                // Note that this is only reached in the non-suspended/visible case:
                // when the content is suspended/hidden, the retain/release occurs
                // via the parent Suspense component (see case above).
                retain_cache(cache);
            }
        }
        SUSPENSE_COMPONENT => {
            if ENABLE_TRANSITION_TRACING {
                // We need to mark this fiber's parents as deleted
                let offscreen_fiber = current.child.borrow().clone().unwrap();
                let instance: &'static OffscreenInstance =
                    unsafe { js::cast(offscreen_fiber.state_node.get()) };
                let transitions = instance.transitions.get();
                if let Some(transitions) = transitions {
                    let abort_reason = Rc::new(TransitionAbort {
                        reason: TransitionAbortReason::Suspense,
                        name: js::get(current.memoized_props.get(), "name")
                            .filter(|name| js::truthy(Some(*name)))
                            .map(|name| unsafe { js::cast(Some(name)) })
                            .or(js::null()),
                    });
                    let memoized_state: Option<&SuspenseState> =
                        unsafe { js::cast(current.memoized_state.get()) };
                    if memoized_state
                        .is_none_or(|memoized_state| memoized_state.dehydrated.is_none())
                    {
                        abort_parent_marker_transitions_for_deleted_fiber(
                            &offscreen_fiber,
                            &abort_reason,
                            transitions,
                            Some(instance),
                            true,
                        );
                        if let Some(nearest_mounted_ancestor) = nearest_mounted_ancestor {
                            abort_parent_marker_transitions_for_deleted_fiber(
                                nearest_mounted_ancestor,
                                &abort_reason,
                                transitions,
                                Some(instance),
                                false,
                            );
                        }
                    }
                }
            }
        }
        CACHE_COMPONENT => {
            let cache =
                &unsafe { js::cast::<&CacheComponentState>(current.memoized_state.get()) }.cache;
            release_cache(cache);
        }
        TRACING_MARKER_COMPONENT if ENABLE_TRANSITION_TRACING => {
            // We need to mark this fiber's parents as deleted
            let instance: &TracingMarkerInstance = unsafe { js::cast(current.state_node.get()) };
            let transitions = instance.transitions.get();
            if let Some(transitions) = transitions {
                let abort_reason = Rc::new(TransitionAbort {
                    reason: TransitionAbortReason::Marker,
                    name: unsafe { js::cast(js::get(current.memoized_props.get(), "name")) },
                });
                abort_parent_marker_transitions_for_deleted_fiber(
                    current,
                    &abort_reason,
                    transitions,
                    js::null(),
                    true,
                );
                if let Some(nearest_mounted_ancestor) = nearest_mounted_ancestor {
                    abort_parent_marker_transitions_for_deleted_fiber(
                        nearest_mounted_ancestor,
                        &abort_reason,
                        transitions,
                        js::null(),
                        false,
                    );
                }
            }
        }
        _ => {}
    }

    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        && ENABLE_PROFILER_COMMIT_HOOKS.with(|&enabled| enabled)
        && ENABLE_COMPONENT_PERFORMANCE_TRACK
        && (current.mode.get() & PROFILE_MODE) != NO_MODE
        && COMPONENT_EFFECT_START_TIME.get() >= 0.0
        && COMPONENT_EFFECT_END_TIME.get() >= 0.0
        && (COMPONENT_EFFECT_SPAWNED_UPDATE.get() || COMPONENT_EFFECT_DURATION.get() > 0.05)
    {
        log_component_effect(
            current,
            COMPONENT_EFFECT_START_TIME.get(),
            COMPONENT_EFFECT_END_TIME.get(),
            COMPONENT_EFFECT_DURATION.get(),
            COMPONENT_EFFECT_ERRORS.with_borrow(Clone::clone).as_deref(),
        );
    }

    pop_component_effect_start(prev_effect_start);
    pop_component_effect_duration(prev_effect_duration);
    pop_component_effect_did_spawn_update(prev_effect_did_spawn_update);
    pop_component_effect_errors(prev_effect_errors);
}

#[cfg_attr(rust_js, rust_js::name = "invokeLayoutEffectMountInDEV")]
pub fn invoke_layout_effect_mount_in_dev(fiber: &Rc<Fiber>) {
    if __DEV__ {
        // We don't need to re-check StrictEffectsMode here.
        // This function is only called if that check has already passed.
        match fiber.tag.get() {
            FUNCTION_COMPONENT | FORWARD_REF | SIMPLE_MEMO_COMPONENT => {
                commit_hook_effect_list_mount(HOOK_LAYOUT | HOOK_HAS_EFFECT, fiber);
            }
            CLASS_COMPONENT => {
                commit_class_did_mount(fiber);
            }
            _ => {}
        }
    }
}

#[cfg_attr(rust_js, rust_js::name = "invokePassiveEffectMountInDEV")]
pub fn invoke_passive_effect_mount_in_dev(fiber: &Rc<Fiber>) {
    if __DEV__ {
        // We don't need to re-check StrictEffectsMode here.
        // This function is only called if that check has already passed.
        match fiber.tag.get() {
            FUNCTION_COMPONENT | FORWARD_REF | SIMPLE_MEMO_COMPONENT => {
                commit_hook_effect_list_mount(HOOK_PASSIVE | HOOK_HAS_EFFECT, fiber);
            }
            _ => {}
        }
    }
}

#[cfg_attr(rust_js, rust_js::name = "invokeLayoutEffectUnmountInDEV")]
pub fn invoke_layout_effect_unmount_in_dev(fiber: &Rc<Fiber>) {
    if __DEV__ {
        // We don't need to re-check StrictEffectsMode here.
        // This function is only called if that check has already passed.
        match fiber.tag.get() {
            FUNCTION_COMPONENT | FORWARD_REF | SIMPLE_MEMO_COMPONENT => {
                let return_fiber = fiber.r#return.borrow().clone();
                commit_hook_effect_list_unmount(
                    HOOK_LAYOUT | HOOK_HAS_EFFECT,
                    fiber,
                    return_fiber.as_ref(),
                );
            }
            CLASS_COMPONENT => {
                let instance: &'static ClassInstance = unsafe { js::cast(fiber.state_node.get()) };
                if js::type_of(js::get(instance, "componentWillUnmount")) == "function" {
                    let return_fiber = fiber.r#return.borrow().clone();
                    safely_call_component_will_unmount(fiber, return_fiber.as_ref(), instance);
                }
            }
            _ => {}
        }
    }
}

#[cfg_attr(rust_js, rust_js::name = "invokePassiveEffectUnmountInDEV")]
pub fn invoke_passive_effect_unmount_in_dev(fiber: &Rc<Fiber>) {
    if __DEV__ {
        // We don't need to re-check StrictEffectsMode here.
        // This function is only called if that check has already passed.
        match fiber.tag.get() {
            FUNCTION_COMPONENT | FORWARD_REF | SIMPLE_MEMO_COMPONENT => {
                let return_fiber = fiber.r#return.borrow().clone();
                commit_hook_effect_list_unmount(
                    HOOK_PASSIVE | HOOK_HAS_EFFECT,
                    fiber,
                    return_fiber.as_ref(),
                );
            }
            _ => {}
        }
    }
}
