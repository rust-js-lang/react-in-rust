// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::rc::Rc;

use js::{Kind, Unknown};
use react_isomorphic::react_context::ReactContext;
use react_isomorphic::react_start_transition::Transition;
use shared::react_feature_flags::{
    DISABLE_LEGACY_MODE, ENABLE_LEGACY_HIDDEN, ENABLE_PROFILER_TIMER, ENABLE_SCOPE_API,
    ENABLE_SUSPENSE_CALLBACK, ENABLE_SUSPENSEY_IMAGES, ENABLE_TRANSITION_TRACING,
    ENABLE_VIEW_TRANSITION, ENABLE_VIEW_TRANSITION_PARENT_ENTER_EXIT,
    PASS_CHILDREN_WHEN_CLONING_PERSISTED_NODES,
};

use crate::scheduler::NOW as now;

use crate::react_fiber_flags::{
    CHILD_DELETION, CLONED, DID_CAPTURE, FORCE_CLIENT_RENDER, Flags, HYDRATE, MAY_SUSPEND_COMMIT,
    NO_FLAGS, PASSIVE, PLACEMENT, PORTAL_STATIC, SCHEDULE_RETRY, SHOULD_SUSPEND_COMMIT, SNAPSHOT,
    STATIC_MASK, UPDATE, VIEW_TRANSITION_STATIC, VIEW_TRANSITION_STATIC_PARENT, VISIBILITY,
};
use crate::react_type_of_mode::{CONCURRENT_MODE, NO_MODE, PROFILE_MODE, SUSPENSEY_IMAGES_MODE};
use crate::react_work_tags::{
    ACTIVITY_COMPONENT, CACHE_COMPONENT, CLASS_COMPONENT, CONTEXT_CONSUMER, CONTEXT_PROVIDER,
    FORWARD_REF, FRAGMENT, FUNCTION_COMPONENT, HOST_COMPONENT, HOST_HOISTABLE, HOST_PORTAL,
    HOST_ROOT, HOST_SINGLETON, HOST_TEXT, INCOMPLETE_CLASS_COMPONENT,
    INCOMPLETE_FUNCTION_COMPONENT, LAZY_COMPONENT, LEGACY_HIDDEN_COMPONENT, MEMO_COMPONENT, MODE,
    OFFSCREEN_COMPONENT, PROFILER, SCOPE_COMPONENT, SIMPLE_MEMO_COMPONENT, SUSPENSE_COMPONENT,
    SUSPENSE_LIST_COMPONENT, THROW, TRACING_MARKER_COMPONENT, VIEW_TRANSITION_COMPONENT,
};

use crate::react_child_fiber::reset_child_fibers;
use crate::react_fiber::PortalStateNode;
use crate::react_fiber_activity_component::ActivityState;
use crate::react_fiber_cache_component::{
    Cache, CacheComponentState, SpawnedCachePool, pop_cache_provider,
};
use crate::react_fiber_hidden_context::pop_hidden_context;
use crate::react_fiber_host_context::{
    get_host_context, get_root_host_container, pop_host_container, pop_host_context,
};
use crate::react_fiber_hydration_context::{
    emit_pending_hydration_warnings, get_is_hydrating, pop_hydration_state,
    prepare_to_hydrate_host_activity_instance, prepare_to_hydrate_host_instance,
    prepare_to_hydrate_host_suspense_instance, prepare_to_hydrate_host_text_instance,
    reset_hydration_state, upgrade_hydration_errors_to_recoverable,
};
use crate::react_fiber_lane::{
    Lane, Lanes, NO_LANES, OFFSCREEN_LANE, SOME_RETRY_LANE, claim_next_retry_lane,
    includes_only_suspensey_commit_eligible_lanes, includes_some_lane, merge_lanes,
};
use crate::react_fiber_legacy_context::{
    is_context_provider as is_legacy_context_provider, pop_context as pop_legacy_context,
    pop_top_level_context_object as pop_top_level_legacy_context_object,
};
use crate::react_fiber_new_context::pop_provider;
use crate::react_fiber_offscreen_component::{OffscreenQueue, OffscreenState};
use crate::react_fiber_root::RootState;
use crate::react_fiber_scope::create_scope_instance;
use crate::react_fiber_suspense_component::{
    RetryQueue, SuspenseListRenderState, SuspenseListTailMode, SuspenseState, find_first_suspended,
};
use crate::react_fiber_suspense_context::{
    FORCE_SUSPENSE_FALLBACK, SUSPENSE_STACK_CURSOR, pop_suspense_handler,
    pop_suspense_list_context, push_suspense_list_catch, push_suspense_list_context,
    set_default_shallow_suspense_list_context, set_shallow_suspense_list_context,
};
use crate::react_fiber_thenable::suspend_commit;
use crate::react_fiber_tracing_marker_component::{
    TracingMarkerInstance, pop_marker_instance, pop_root_marker_instance,
};
use crate::react_fiber_transition::{pop_root_transition, pop_transition};
use crate::react_fiber_tree_context::{pop_tree_context, push_tree_fork};
use crate::react_internal_types::{Fiber, FiberRoot};
use crate::react_profiler_timer::transfer_actual_duration;

unsafe extern "Rust" {
    #[link_name = "./src/ReactFiberConfig.js#supportsMutation"]
    safe static SUPPORTS_MUTATION: bool;
    #[link_name = "./src/ReactFiberConfig.js#supportsPersistence"]
    safe static SUPPORTS_PERSISTENCE: bool;
    #[link_name = "./src/ReactFiberConfig.js#supportsResources"]
    safe static SUPPORTS_RESOURCES: bool;
    #[link_name = "./src/ReactFiberConfig.js#supportsSingletons"]
    safe static SUPPORTS_SINGLETONS: bool;
}

// ReactFiberConfig's host operations, on its `Instance`, `TextInstance`,
// `Type`, `Props`, `Container`, `HostContext`, `ChildSet` and `Resource`.
#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#createInstance"
)]
fn create_instance(
    r#type: Option<&'static Unknown>,
    props: Option<&'static Unknown>,
    root_container_instance: Option<&'static Unknown>,
    host_context: Option<&'static Unknown>,
    internal_instance_handle: &Fiber,
) -> &'static Unknown {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#createTextInstance"
)]
fn create_text_instance(
    text: Option<&'static Unknown>,
    root_container_instance: Option<&'static Unknown>,
    host_context: Option<&'static Unknown>,
    internal_instance_handle: &Fiber,
) -> &'static Unknown {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#resolveSingletonInstance"
)]
fn resolve_singleton_instance(
    r#type: Option<&'static Unknown>,
    props: Option<&'static Unknown>,
    root_container_instance: Option<&'static Unknown>,
    host_context: Option<&'static Unknown>,
    validate_dom_nesting_dev: bool,
) -> &'static Unknown {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#appendInitialChild"
)]
fn append_initial_child(
    parent_instance: Option<&'static Unknown>,
    child: Option<&'static Unknown>,
) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#finalizeInitialChildren"
)]
fn finalize_initial_children(
    instance: Option<&'static Unknown>,
    r#type: Option<&'static Unknown>,
    props: Option<&'static Unknown>,
    host_context: Option<&'static Unknown>,
) -> bool {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#finalizeHydratedChildren"
)]
fn finalize_hydrated_children(
    instance: Option<&'static Unknown>,
    r#type: Option<&'static Unknown>,
    props: Option<&'static Unknown>,
    host_context: Option<&'static Unknown>,
) -> bool {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#cloneInstance"
)]
fn clone_instance(
    instance: Option<&'static Unknown>,
    r#type: Option<&'static Unknown>,
    old_props: Option<&'static Unknown>,
    new_props: Option<&'static Unknown>,
    keep_children: bool,
    new_child_set: Option<&'static Unknown>,
) -> &'static Unknown {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#cloneHiddenInstance"
)]
fn clone_hidden_instance(
    instance: Option<&'static Unknown>,
    r#type: Option<&'static Unknown>,
    props: Option<&'static Unknown>,
) -> &'static Unknown {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#cloneHiddenTextInstance"
)]
fn clone_hidden_text_instance(
    instance: Option<&'static Unknown>,
    text: Option<&'static Unknown>,
) -> &'static Unknown {
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
    rust_js::link_name = "./src/ReactFiberConfig.js#appendChildToContainerChildSet"
)]
fn append_child_to_container_child_set(
    child_set: &'static Unknown,
    child: Option<&'static Unknown>,
) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#finalizeContainerChildren"
)]
fn finalize_container_children(container: &'static Unknown, new_children: &'static Unknown) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#preparePortalMount"
)]
fn prepare_portal_mount(portal_instance: Option<&'static Unknown>) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#prepareScopeUpdate"
)]
fn prepare_scope_update(scope_instance: &'static Unknown, instance: &Fiber) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#maySuspendCommit"
)]
fn may_suspend_commit(r#type: Option<&'static Unknown>, props: Option<&'static Unknown>) -> bool {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#maySuspendCommitOnUpdate"
)]
fn may_suspend_commit_on_update(
    r#type: Option<&'static Unknown>,
    old_props: &'static Unknown,
    new_props: Option<&'static Unknown>,
) -> bool {
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
    rust_js::link_name = "./src/ReactFiberConfig.js#mayResourceSuspendCommit"
)]
fn may_resource_suspend_commit(resource: &'static Unknown) -> bool {
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
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#renderHasNotSuspendedYet"
)]
fn render_has_not_suspended_yet() -> bool {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#getRenderTargetTime"
)]
fn get_render_target_time() -> f64 {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#getWorkInProgressTransitions"
)]
#[cfg_attr(rust_js, rust_js::nullable)]
fn get_work_in_progress_transitions() -> Option<Rc<Vec<&'static Transition>>> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#shouldRemainOnPreviousScreen"
)]
fn should_remain_on_previous_screen() -> bool {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#markSpawnedRetryLane"
)]
fn mark_spawned_retry_lane(lane: Lane) {
    unreachable!()
}

/// A HostRoot's or a HostPortal's `stateNode`, as one is the other here.
struct PortalOrRoot {
    container_info: &'static Unknown,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pending_children: std::cell::Cell<Option<&'static Unknown>>,
}

/// Tag the fiber with an update effect. This turns a Placement into
/// a PlacementAndUpdate.
fn mark_update(work_in_progress: &Fiber) {
    work_in_progress
        .flags
        .set(work_in_progress.flags.get() | UPDATE);
}

/// Tag the fiber with Cloned in persistent mode to signal that
/// it received an update that requires a clone of the tree above.
fn mark_cloned(work_in_progress: &Fiber) {
    if SUPPORTS_PERSISTENCE {
        work_in_progress
            .flags
            .set(work_in_progress.flags.get() | CLONED);
    }
}

/// In persistent mode, return whether this update needs to clone the subtree.
fn does_require_clone(current: Option<&Fiber>, completed_work: &Fiber) -> bool {
    let did_bailout =
        current.is_some_and(|current| *current.child.borrow() == *completed_work.child.borrow());
    if did_bailout {
        return false;
    }

    if (completed_work.flags.get() & CHILD_DELETION) != NO_FLAGS {
        return true;
    }

    // TODO: If we move the `doesRequireClone` call after `bubbleProperties`
    // then we only have to check the `completedWork.subtreeFlags`.
    let mut child = completed_work.child.borrow().clone();
    while let Some(current) = child {
        let checked_flags = CLONED | VISIBILITY | PLACEMENT | CHILD_DELETION;
        if (current.flags.get() & checked_flags) != NO_FLAGS
            || (current.subtree_flags.get() & checked_flags) != NO_FLAGS
        {
            return true;
        }
        child = current.sibling.borrow().clone();
    }
    false
}

fn append_all_children(
    parent: Option<&'static Unknown>,
    work_in_progress: &Rc<Fiber>,
    needs_visibility_toggle: bool,
    is_hidden: bool,
) {
    if SUPPORTS_MUTATION {
        // We only have the top Fiber that was created but we need recurse down its
        // children to find all the terminal nodes.
        let mut node = work_in_progress.child.borrow().clone();
        while let Some(mut current) = node {
            let child = current.child.borrow().clone();
            if current.tag.get() == HOST_COMPONENT || current.tag.get() == HOST_TEXT {
                append_initial_child(parent, current.state_node.get());
            } else if current.tag.get() == HOST_PORTAL
                || (if SUPPORTS_SINGLETONS {
                    current.tag.get() == HOST_SINGLETON
                } else {
                    false
                })
            {
                // If we have a portal child, then we don't want to traverse
                // down its children. Instead, we'll get insertions from each child in
                // the portal directly.
                // If we have a HostSingleton it will be placed independently
            } else if let Some(child) = child {
                child.r#return.replace(Some(Rc::clone(&current)));
                node = Some(child);
                continue;
            }
            if current == *work_in_progress {
                return;
            }
            while current.sibling.borrow().is_none() {
                let return_fiber = current.r#return.borrow().clone();
                match return_fiber {
                    Some(return_fiber) if return_fiber != *work_in_progress => {
                        current = return_fiber;
                    }
                    _ => return,
                }
            }
            let sibling = current.sibling.borrow().clone().unwrap();
            let return_fiber = current.r#return.borrow().clone();
            sibling.r#return.replace(return_fiber);
            node = Some(sibling);
        }
    } else if SUPPORTS_PERSISTENCE {
        // We only have the top Fiber that was created but we need recurse down its
        // children to find all the terminal nodes.
        let mut node = work_in_progress.child.borrow().clone();
        while let Some(mut current) = node {
            let child = current.child.borrow().clone();
            if current.tag.get() == HOST_COMPONENT {
                let mut instance = current.state_node.get();
                if needs_visibility_toggle && is_hidden {
                    // This child is inside a timed out tree. Hide it.
                    let props = current.memoized_props.get();
                    let r#type = current.r#type.get();
                    instance = Some(clone_hidden_instance(instance, r#type, props));
                }
                append_initial_child(parent, instance);
            } else if current.tag.get() == HOST_TEXT {
                let mut instance = current.state_node.get();
                if needs_visibility_toggle && is_hidden {
                    // This child is inside a timed out tree. Hide it.
                    let text = current.memoized_props.get();
                    instance = Some(clone_hidden_text_instance(instance, text));
                }
                append_initial_child(parent, instance);
            } else if current.tag.get() == HOST_PORTAL {
                // If we have a portal child, then we don't want to traverse
                // down its children. Instead, we'll get insertions from each child in
                // the portal directly.
            } else if current.tag.get() == OFFSCREEN_COMPONENT
                && current.memoized_state.get().is_some()
            {
                // The children in this boundary are hidden. Toggle their visibility
                // before appending.
                if let Some(child) = child {
                    child.r#return.replace(Some(Rc::clone(&current)));
                }
                append_all_children(
                    parent, &current, /* needsVisibilityToggle */ true,
                    /* isHidden */ true,
                );
            } else if let Some(child) = child {
                child.r#return.replace(Some(Rc::clone(&current)));
                node = Some(child);
                continue;
            }
            if current == *work_in_progress {
                return;
            }
            while current.sibling.borrow().is_none() {
                let return_fiber = current.r#return.borrow().clone();
                match return_fiber {
                    Some(return_fiber) if return_fiber != *work_in_progress => {
                        current = return_fiber;
                    }
                    _ => return,
                }
            }
            let sibling = current.sibling.borrow().clone().unwrap();
            let return_fiber = current.r#return.borrow().clone();
            sibling.r#return.replace(return_fiber);
            node = Some(sibling);
        }
    }
}

// An unfortunate fork of appendAllChildren because we have two different parent types.
fn append_all_children_to_container(
    container_child_set: &'static Unknown,
    work_in_progress: &Rc<Fiber>,
    needs_visibility_toggle: bool,
    is_hidden: bool,
) -> bool {
    // Host components that have their visibility toggled by an OffscreenComponent
    // do not support passChildrenWhenCloningPersistedNodes. To inform the callee
    // about their presence, we track and return if they were added to the
    // child set.
    let mut has_offscreen_component_child = false;
    if SUPPORTS_PERSISTENCE {
        // We only have the top Fiber that was created but we need recurse down its
        // children to find all the terminal nodes.
        let mut node = work_in_progress.child.borrow().clone();
        while let Some(mut current) = node {
            let child = current.child.borrow().clone();
            if current.tag.get() == HOST_COMPONENT {
                let mut instance = current.state_node.get();
                if needs_visibility_toggle && is_hidden {
                    // This child is inside a timed out tree. Hide it.
                    let props = current.memoized_props.get();
                    let r#type = current.r#type.get();
                    instance = Some(clone_hidden_instance(instance, r#type, props));
                }
                append_child_to_container_child_set(container_child_set, instance);
            } else if current.tag.get() == HOST_TEXT {
                let mut instance = current.state_node.get();
                if needs_visibility_toggle && is_hidden {
                    // This child is inside a timed out tree. Hide it.
                    let text = current.memoized_props.get();
                    instance = Some(clone_hidden_text_instance(instance, text));
                }
                append_child_to_container_child_set(container_child_set, instance);
            } else if current.tag.get() == HOST_PORTAL {
                // If we have a portal child, then we don't want to traverse
                // down its children. Instead, we'll get insertions from each child in
                // the portal directly.
            } else if current.tag.get() == OFFSCREEN_COMPONENT
                && current.memoized_state.get().is_some()
            {
                // The children in this boundary are hidden. Toggle their visibility
                // before appending.
                if let Some(child) = child {
                    child.r#return.replace(Some(Rc::clone(&current)));
                }
                append_all_children_to_container(
                    container_child_set,
                    &current,
                    /* needsVisibilityToggle */ true,
                    /* isHidden */ true,
                );

                has_offscreen_component_child = true;
            } else if let Some(child) = child {
                child.r#return.replace(Some(Rc::clone(&current)));
                node = Some(child);
                continue;
            }
            if current == *work_in_progress {
                return has_offscreen_component_child;
            }
            while current.sibling.borrow().is_none() {
                let return_fiber = current.r#return.borrow().clone();
                match return_fiber {
                    Some(return_fiber) if return_fiber != *work_in_progress => {
                        current = return_fiber;
                    }
                    _ => return has_offscreen_component_child,
                }
            }
            let sibling = current.sibling.borrow().clone().unwrap();
            let return_fiber = current.r#return.borrow().clone();
            sibling.r#return.replace(return_fiber);
            node = Some(sibling);
        }
    }

    has_offscreen_component_child
}

fn update_host_container(current: Option<&Fiber>, work_in_progress: &Rc<Fiber>) {
    if SUPPORTS_PERSISTENCE && does_require_clone(current, work_in_progress) {
        let portal_or_root: &PortalOrRoot = unsafe { js::cast(work_in_progress.state_node.get()) };
        let container = portal_or_root.container_info;
        let new_child_set = create_container_child_set();
        // If children might have changed, we have to add them all to the set.
        append_all_children_to_container(
            new_child_set,
            work_in_progress,
            /* needsVisibilityToggle */ false,
            /* isHidden */ false,
        );
        portal_or_root.pending_children.set(Some(new_child_set));
        // Schedule an update on the container to swap out the container.
        mark_update(work_in_progress);
        finalize_container_children(container, new_child_set);
    }
}

fn update_host_component(
    current: &Fiber,
    work_in_progress: &Rc<Fiber>,
    r#type: Option<&'static Unknown>,
    new_props: Option<&'static Unknown>,
    _render_lanes: Lanes,
) {
    if SUPPORTS_MUTATION {
        // If we have an alternate, that means this is an update and we need to
        // schedule a side-effect to do the updates.
        let old_props = current.memoized_props.get();
        if js::strict_equals(old_props, new_props) {
            // In mutation mode, this is sufficient for a bailout because
            // we won't touch this node even if children changed.
            return;
        }

        mark_update(work_in_progress);
    } else if SUPPORTS_PERSISTENCE {
        let current_instance = current.state_node.get();
        let old_props = current.memoized_props.get();
        // If there are no effects associated with this node, then none of our children had any updates.
        // This guarantees that we can reuse all of them.
        let requires_clone = does_require_clone(Some(current), work_in_progress);
        if !requires_clone && js::strict_equals(old_props, new_props) {
            // No changes, just reuse the existing instance.
            // Note that this might release a previous clone.
            work_in_progress.state_node.set(current_instance);
            return;
        }
        let current_host_context = get_host_context();

        let mut new_child_set = js::null();
        let mut has_offscreen_component_child = false;
        if requires_clone && PASS_CHILDREN_WHEN_CLONING_PERSISTED_NODES {
            mark_cloned(work_in_progress);
            let child_set = create_container_child_set();
            new_child_set = Some(child_set);
            // If children might have changed, we have to add them all to the set.
            has_offscreen_component_child = append_all_children_to_container(
                child_set,
                work_in_progress,
                /* needsVisibilityToggle */ false,
                /* isHidden */ false,
            );
        }

        let new_instance = clone_instance(
            current_instance,
            r#type,
            old_props,
            new_props,
            !requires_clone,
            if !has_offscreen_component_child {
                new_child_set
            } else {
                None
            },
        );
        if js::strict_equals(new_instance, current_instance) {
            // No changes, just reuse the existing instance.
            // Note that this might release a previous clone.
            work_in_progress.state_node.set(current_instance);
            return;
        } else {
            mark_cloned(work_in_progress);
        }

        // Certain renderers require commit-time effects for initial mount.
        // (eg DOM renderer supports auto-focus for certain elements).
        // Make sure such renderers get scheduled for later work.
        if finalize_initial_children(Some(new_instance), r#type, new_props, current_host_context) {
            mark_update(work_in_progress);
        }
        work_in_progress.state_node.set(Some(new_instance));
        if requires_clone
            && (!PASS_CHILDREN_WHEN_CLONING_PERSISTED_NODES || has_offscreen_component_child)
        {
            // If children have changed, we have to add them all to the set.
            append_all_children(
                Some(new_instance),
                work_in_progress,
                /* needsVisibilityToggle */ false,
                /* isHidden */ false,
            );
        }
    }
}

// This function must be called at the very end of the complete phase, because
// it might throw to suspend, and if the resource immediately loads, the work
// loop will resume rendering as if the work-in-progress completed. So it must
// fully complete.
// TODO: This should ideally move to begin phase, but currently the instance is
// not created until the complete phase. For our existing use cases, host nodes
// that suspend don't have children, so it doesn't matter. But that might not
// always be true in the future.
fn preload_instance_and_suspend_if_needed(
    work_in_progress: &Fiber,
    r#type: Option<&'static Unknown>,
    old_props: Option<&'static Unknown>,
    new_props: Option<&'static Unknown>,
    render_lanes: Lanes,
) {
    let may_suspend = (ENABLE_SUSPENSEY_IMAGES
        || (work_in_progress.mode.get() & SUSPENSEY_IMAGES_MODE) != NO_MODE)
        && match old_props {
            None => may_suspend_commit(r#type, new_props),
            Some(old_props) => may_suspend_commit_on_update(r#type, old_props, new_props),
        };

    if !may_suspend {
        // If this flag was set previously, we can remove it. The flag
        // represents whether this particular set of props might ever need to
        // suspend. The safest thing to do is for maySuspendCommit to always
        // return true, but if the renderer is reasonably confident that the
        // underlying resource won't be evicted, it can return false as a
        // performance optimization.
        work_in_progress
            .flags
            .set(work_in_progress.flags.get() & !MAY_SUSPEND_COMMIT);
        return;
    }

    // Mark this fiber with a flag. This gets set on all host instances
    // that might possibly suspend, even if they don't need to suspend
    // currently. We use this when revealing a prerendered tree, because
    // even though the tree has "mounted", its resources might not have
    // loaded yet.
    work_in_progress
        .flags
        .set(work_in_progress.flags.get() | MAY_SUSPEND_COMMIT);

    if includes_only_suspensey_commit_eligible_lanes(render_lanes)
        || may_suspend_commit_in_sync_render(r#type, new_props)
    {
        // preload the instance if necessary. Even if this is an urgent render there
        // could be benefits to preloading early.
        // @TODO we should probably do the preload in begin work
        let is_ready = preload_instance(work_in_progress.state_node.get(), r#type, new_props);
        if !is_ready {
            if should_remain_on_previous_screen() {
                work_in_progress
                    .flags
                    .set(work_in_progress.flags.get() | SHOULD_SUSPEND_COMMIT);
            } else {
                suspend_commit();
            }
        } else {
            // Even if we're ready we suspend the commit and check again in the pre-commit
            // phase if we need to suspend anyway. Such as if it's delayed on decoding or
            // if it was dropped from the cache while rendering due to pressure.
            work_in_progress
                .flags
                .set(work_in_progress.flags.get() | SHOULD_SUSPEND_COMMIT);
        }
    }
}

fn preload_resource_and_suspend_if_needed(
    work_in_progress: &Fiber,
    resource: &'static Unknown,
    _type: Option<&'static Unknown>,
    _props: Option<&'static Unknown>,
    _render_lanes: Lanes,
) {
    // This is a fork of preloadInstanceAndSuspendIfNeeded, but for resources.
    if !may_resource_suspend_commit(resource) {
        work_in_progress
            .flags
            .set(work_in_progress.flags.get() & !MAY_SUSPEND_COMMIT);
        return;
    }

    work_in_progress
        .flags
        .set(work_in_progress.flags.get() | MAY_SUSPEND_COMMIT);

    let is_ready = preload_resource(resource);
    if !is_ready {
        if should_remain_on_previous_screen() {
            work_in_progress
                .flags
                .set(work_in_progress.flags.get() | SHOULD_SUSPEND_COMMIT);
        } else {
            suspend_commit();
        }
    }
}

fn schedule_retry_effect(work_in_progress: &Fiber, retry_queue: Option<&RetryQueue>) {
    let wakeables = retry_queue;
    if wakeables.is_some() {
        // Schedule an effect to attach a retry listener to the promise.
        // TODO: Move to passive phase
        work_in_progress
            .flags
            .set(work_in_progress.flags.get() | UPDATE);
    }

    // Check if we need to schedule an immediate retry. This should happen
    // whenever we unwind a suspended tree without fully rendering its siblings;
    // we need to begin the retry so we can start prerendering them.
    //
    // We also use this mechanism for Suspensey Resources (e.g. stylesheets),
    // because those don't actually block the render phase, only the commit phase.
    // So we can start rendering even before the resources are ready.
    if (work_in_progress.flags.get() & SCHEDULE_RETRY) != NO_FLAGS {
        let retry_lane =
            // TODO: This check should probably be moved into claimNextRetryLane
            // I also suspect that we need some further consolidation of offscreen
            // and retry lanes.
            if work_in_progress.tag.get() != OFFSCREEN_COMPONENT {
                claim_next_retry_lane()
            } else {
                OFFSCREEN_LANE
            };
        work_in_progress
            .lanes
            .set(merge_lanes(work_in_progress.lanes.get(), retry_lane));

        // Track the lanes that have been scheduled for an immediate retry so that
        // we can mark them as suspended upon committing the root.
        mark_spawned_retry_lane(retry_lane);
    }
}

fn update_host_text(
    current: &Fiber,
    work_in_progress: &Fiber,
    old_text: Option<&'static Unknown>,
    new_text: Option<&'static Unknown>,
) {
    if SUPPORTS_MUTATION {
        // If the text differs, mark it as an update. All the work in done in commitWork.
        if !js::strict_equals(old_text, new_text) {
            mark_update(work_in_progress);
        }
    } else if SUPPORTS_PERSISTENCE {
        if !js::strict_equals(old_text, new_text) {
            // If the text content differs, we'll create a new text instance for it.
            let root_container_instance = get_root_host_container();
            let current_host_context = get_host_context();
            mark_cloned(work_in_progress);
            work_in_progress.state_node.set(Some(create_text_instance(
                new_text,
                root_container_instance,
                current_host_context,
                work_in_progress,
            )));
        } else {
            work_in_progress.state_node.set(current.state_node.get());
        }
    }
}

fn cut_off_tail_if_needed(
    render_state: &SuspenseListRenderState,
    has_rendered_a_tail_fallback: bool,
) {
    if get_is_hydrating() {
        // If we're hydrating, we should consume as many items as we can
        // so we don't leave any behind.
        return;
    }
    match render_state.tail_mode.get() {
        Some(SuspenseListTailMode::Visible) => {
            // Everything should remain as it was.
        }
        Some(SuspenseListTailMode::Collapsed) => {
            // Any insertions at the end of the tail list after this point
            // should be invisible. If there are already mounted boundaries
            // anything before them are not considered for collapsing.
            // Therefore we need to go through the whole tail to find if
            // there are any.
            let mut tail_node = render_state.tail.borrow().clone();
            let mut last_tail_node = js::null();
            while let Some(current) = tail_node {
                if current.alternate.borrow().is_some() {
                    last_tail_node = Some(Rc::clone(&current));
                }
                tail_node = current.sibling.borrow().clone();
            }
            // Next we're simply going to delete all insertions after the
            // last rendered item.
            match last_tail_node {
                None => {
                    // All remaining items in the tail are insertions.
                    let tail = render_state.tail.borrow().clone();
                    match tail {
                        Some(tail) if !has_rendered_a_tail_fallback => {
                            // We suspended during the head. We want to show at least one
                            // row at the tail. So we'll keep on and cut off the rest.
                            tail.sibling.replace(None);
                        }
                        _ => {
                            render_state.tail.replace(None);
                        }
                    }
                }
                Some(last_tail_node) => {
                    // Detach the insertion after the last node that was already
                    // inserted.
                    last_tail_node.sibling.replace(None);
                }
            }
        }
        // Hidden is now the default.
        _ => {
            // Any insertions at the end of the tail list after this point
            // should be invisible. If there are already mounted boundaries
            // anything before them are not considered for collapsing.
            // Therefore we need to go through the whole tail to find if
            // there are any.
            let mut tail_node = render_state.tail.borrow().clone();
            let mut last_tail_node = js::null();
            while let Some(current) = tail_node {
                if current.alternate.borrow().is_some() {
                    last_tail_node = Some(Rc::clone(&current));
                }
                tail_node = current.sibling.borrow().clone();
            }
            // Next we're simply going to delete all insertions after the
            // last rendered item.
            match last_tail_node {
                None => {
                    // All remaining items in the tail are insertions.
                    render_state.tail.replace(None);
                }
                Some(last_tail_node) => {
                    // Detach the insertion after the last node that was already
                    // inserted.
                    last_tail_node.sibling.replace(None);
                }
            }
        }
    }
}

fn is_only_new_mounts(tail: &Rc<Fiber>) -> bool {
    let mut fiber = Some(Rc::clone(tail));
    while let Some(current) = fiber {
        if current.alternate.borrow().is_some() {
            return false;
        }
        fiber = current.sibling.borrow().clone();
    }
    true
}

fn bubble_properties(completed_work: &Rc<Fiber>) -> bool {
    let did_bailout = completed_work
        .alternate
        .borrow()
        .as_ref()
        .is_some_and(|alternate| *alternate.child.borrow() == *completed_work.child.borrow());

    let mut new_child_lanes: Lanes = NO_LANES;
    let mut subtree_flags: Flags = NO_FLAGS;

    if !did_bailout {
        // Bubble up the earliest expiration time.
        if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
            && (completed_work.mode.get() & PROFILE_MODE) != NO_MODE
        {
            // In profiling mode, resetChildExpirationTime is also used to reset
            // profiler durations.
            let mut actual_duration = completed_work.actual_duration.get().unwrap();
            let mut tree_base_duration = completed_work.self_base_duration.get().unwrap();

            let mut child = completed_work.child.borrow().clone();
            while let Some(current) = child {
                new_child_lanes = merge_lanes(
                    new_child_lanes,
                    merge_lanes(current.lanes.get(), current.child_lanes.get()),
                );

                subtree_flags |= current.subtree_flags.get();
                subtree_flags |= current.flags.get();

                // When a fiber is cloned, its actualDuration is reset to 0. This value will
                // only be updated if work is done on the fiber (i.e. it doesn't bailout).
                // When work is done, it should bubble to the parent's actualDuration. If
                // the fiber has not been cloned though, (meaning no work was done), then
                // this value will reflect the amount of time spent working on a previous
                // render. In that case it should not bubble. We determine whether it was
                // cloned by comparing the child pointer.
                // $FlowFixMe[unsafe-addition] addition with possible null/undefined value
                actual_duration += current.actual_duration.get().unwrap();

                // $FlowFixMe[unsafe-addition] addition with possible null/undefined value
                tree_base_duration += current.tree_base_duration.get().unwrap();
                child = current.sibling.borrow().clone();
            }

            completed_work.actual_duration.set(Some(actual_duration));
            completed_work
                .tree_base_duration
                .set(Some(tree_base_duration));
        } else {
            let mut child = completed_work.child.borrow().clone();
            while let Some(current) = child {
                new_child_lanes = merge_lanes(
                    new_child_lanes,
                    merge_lanes(current.lanes.get(), current.child_lanes.get()),
                );

                subtree_flags |= current.subtree_flags.get();
                subtree_flags |= current.flags.get();

                // Update the return pointer so the tree is consistent. This is a code
                // smell because it assumes the commit phase is never concurrent with
                // the render phase. Will address during refactor to alternate model.
                current.r#return.replace(Some(Rc::clone(completed_work)));

                child = current.sibling.borrow().clone();
            }
        }

        completed_work
            .subtree_flags
            .set(completed_work.subtree_flags.get() | subtree_flags);
    } else {
        // Bubble up the earliest expiration time.
        if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
            && (completed_work.mode.get() & PROFILE_MODE) != NO_MODE
        {
            // In profiling mode, resetChildExpirationTime is also used to reset
            // profiler durations.
            let mut tree_base_duration = completed_work.self_base_duration.get().unwrap();

            let mut child = completed_work.child.borrow().clone();
            while let Some(current) = child {
                new_child_lanes = merge_lanes(
                    new_child_lanes,
                    merge_lanes(current.lanes.get(), current.child_lanes.get()),
                );

                // "Static" flags share the lifetime of the fiber/hook they belong to,
                // so we should bubble those up even during a bailout. All the other
                // flags have a lifetime only of a single render + commit, so we should
                // ignore them.
                subtree_flags |= current.subtree_flags.get() & STATIC_MASK;
                subtree_flags |= current.flags.get() & STATIC_MASK;

                // $FlowFixMe[unsafe-addition] addition with possible null/undefined value
                tree_base_duration += current.tree_base_duration.get().unwrap();
                child = current.sibling.borrow().clone();
            }

            completed_work
                .tree_base_duration
                .set(Some(tree_base_duration));
        } else {
            let mut child = completed_work.child.borrow().clone();
            while let Some(current) = child {
                new_child_lanes = merge_lanes(
                    new_child_lanes,
                    merge_lanes(current.lanes.get(), current.child_lanes.get()),
                );

                // "Static" flags share the lifetime of the fiber/hook they belong to,
                // so we should bubble those up even during a bailout. All the other
                // flags have a lifetime only of a single render + commit, so we should
                // ignore them.
                subtree_flags |= current.subtree_flags.get() & STATIC_MASK;
                subtree_flags |= current.flags.get() & STATIC_MASK;

                // Update the return pointer so the tree is consistent. This is a code
                // smell because it assumes the commit phase is never concurrent with
                // the render phase. Will address during refactor to alternate model.
                current.r#return.replace(Some(Rc::clone(completed_work)));

                child = current.sibling.borrow().clone();
            }
        }

        completed_work
            .subtree_flags
            .set(completed_work.subtree_flags.get() | subtree_flags);
    }

    completed_work.child_lanes.set(new_child_lanes);

    did_bailout
}

fn complete_dehydrated_activity_boundary(
    current: Option<&Fiber>,
    work_in_progress: &Rc<Fiber>,
    mut next_state: Option<&ActivityState>,
) -> bool {
    let was_hydrated = pop_hydration_state(work_in_progress);

    if next_state.is_some() {
        // We might be inside a hydration state the first time we're picking up this
        // Activity boundary, and also after we've reentered it for further hydration.
        if current.is_none() {
            if !was_hydrated {
                panic!(
                    "{}",
                    concat!(
                        "A dehydrated suspense component was completed without a hydrated node. ",
                        "This is probably a bug in React."
                    )
                );
            }
            prepare_to_hydrate_host_activity_instance(work_in_progress);
            bubble_properties(work_in_progress);
            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                && (work_in_progress.mode.get() & PROFILE_MODE) != NO_MODE
            {
                let is_timed_out_suspense = next_state.is_some();
                if is_timed_out_suspense {
                    // Don't count time spent in a timed out Suspense subtree as part of the base duration.
                    let primary_child_fragment = work_in_progress.child.borrow().clone();
                    if let Some(primary_child_fragment) = primary_child_fragment {
                        // $FlowFixMe[unsafe-arithmetic] Flow doesn't support type casting in combination with the -= operator
                        work_in_progress.tree_base_duration.set(Some(
                            work_in_progress.tree_base_duration.get().unwrap()
                                - primary_child_fragment.tree_base_duration.get().unwrap(),
                        ));
                    }
                }
            }
            false
        } else {
            emit_pending_hydration_warnings();
            // We might have reentered this boundary to hydrate it. If so, we need to reset the hydration
            // state since we're now exiting out of it. popHydrationState doesn't do that for us.
            reset_hydration_state();
            if (work_in_progress.flags.get() & DID_CAPTURE) == NO_FLAGS {
                // This boundary did not suspend so it's now hydrated and unsuspended.
                next_state = js::null();
                work_in_progress.memoized_state.set(js::null());
            }
            // If nothing suspended, we need to schedule an effect to mark this boundary
            // as having hydrated so events know that they're free to be invoked.
            // It's also a signal to replay events and the suspense callback.
            // If something suspended, schedule an effect to attach retry listeners.
            // So we might as well always mark this.
            work_in_progress
                .flags
                .set(work_in_progress.flags.get() | UPDATE);
            bubble_properties(work_in_progress);
            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                && (work_in_progress.mode.get() & PROFILE_MODE) != NO_MODE
            {
                let is_timed_out_suspense = next_state.is_some();
                if is_timed_out_suspense {
                    // Don't count time spent in a timed out Suspense subtree as part of the base duration.
                    let primary_child_fragment = work_in_progress.child.borrow().clone();
                    if let Some(primary_child_fragment) = primary_child_fragment {
                        // $FlowFixMe[unsafe-arithmetic] Flow doesn't support type casting in combination with the -= operator
                        work_in_progress.tree_base_duration.set(Some(
                            work_in_progress.tree_base_duration.get().unwrap()
                                - primary_child_fragment.tree_base_duration.get().unwrap(),
                        ));
                    }
                }
            }
            false
        }
    } else {
        // Successfully completed this tree. If this was a forced client render,
        // there may have been recoverable errors during first hydration
        // attempt. If so, add them to a queue so we can log them in the
        // commit phase. We also add them to prev state so we can get to them
        // from the Suspense Boundary.
        let hydration_errors = upgrade_hydration_errors_to_recoverable();
        if let Some(current) = current
            && let Some(prev_state) =
                unsafe { js::cast::<Option<&ActivityState>>(current.memoized_state.get()) }
        {
            prev_state.hydration_errors.replace(hydration_errors);
        }
        // Fall through to normal Offscreen path
        true
    }
}

fn complete_dehydrated_suspense_boundary(
    current: Option<&Fiber>,
    work_in_progress: &Rc<Fiber>,
    mut next_state: Option<&SuspenseState>,
) -> bool {
    let was_hydrated = pop_hydration_state(work_in_progress);

    if next_state.is_some_and(|next_state| next_state.dehydrated.is_some()) {
        // We might be inside a hydration state the first time we're picking up this
        // Suspense boundary, and also after we've reentered it for further hydration.
        if current.is_none() {
            if !was_hydrated {
                panic!(
                    "{}",
                    concat!(
                        "A dehydrated suspense component was completed without a hydrated node. ",
                        "This is probably a bug in React."
                    )
                );
            }
            prepare_to_hydrate_host_suspense_instance(work_in_progress);
            bubble_properties(work_in_progress);
            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                && (work_in_progress.mode.get() & PROFILE_MODE) != NO_MODE
            {
                let is_timed_out_suspense = next_state.is_some();
                if is_timed_out_suspense {
                    // Don't count time spent in a timed out Suspense subtree as part of the base duration.
                    let primary_child_fragment = work_in_progress.child.borrow().clone();
                    if let Some(primary_child_fragment) = primary_child_fragment {
                        // $FlowFixMe[unsafe-arithmetic] Flow doesn't support type casting in combination with the -= operator
                        work_in_progress.tree_base_duration.set(Some(
                            work_in_progress.tree_base_duration.get().unwrap()
                                - primary_child_fragment.tree_base_duration.get().unwrap(),
                        ));
                    }
                }
            }
            false
        } else {
            emit_pending_hydration_warnings();
            // We might have reentered this boundary to hydrate it. If so, we need to reset the hydration
            // state since we're now exiting out of it. popHydrationState doesn't do that for us.
            reset_hydration_state();
            if (work_in_progress.flags.get() & DID_CAPTURE) == NO_FLAGS {
                // This boundary did not suspend so it's now hydrated and unsuspended.
                next_state = js::null();
                work_in_progress.memoized_state.set(js::null());
            }
            // If nothing suspended, we need to schedule an effect to mark this boundary
            // as having hydrated so events know that they're free to be invoked.
            // It's also a signal to replay events and the suspense callback.
            // If something suspended, schedule an effect to attach retry listeners.
            // So we might as well always mark this.
            work_in_progress
                .flags
                .set(work_in_progress.flags.get() | UPDATE);
            bubble_properties(work_in_progress);
            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                && (work_in_progress.mode.get() & PROFILE_MODE) != NO_MODE
            {
                let is_timed_out_suspense = next_state.is_some();
                if is_timed_out_suspense {
                    // Don't count time spent in a timed out Suspense subtree as part of the base duration.
                    let primary_child_fragment = work_in_progress.child.borrow().clone();
                    if let Some(primary_child_fragment) = primary_child_fragment {
                        // $FlowFixMe[unsafe-arithmetic] Flow doesn't support type casting in combination with the -= operator
                        work_in_progress.tree_base_duration.set(Some(
                            work_in_progress.tree_base_duration.get().unwrap()
                                - primary_child_fragment.tree_base_duration.get().unwrap(),
                        ));
                    }
                }
            }
            false
        }
    } else {
        // Successfully completed this tree. If this was a forced client render,
        // there may have been recoverable errors during first hydration
        // attempt. If so, add them to a queue so we can log them in the
        // commit phase. We also add them to prev state so we can get to them
        // from the Suspense Boundary.
        let hydration_errors = upgrade_hydration_errors_to_recoverable();
        if let Some(current) = current
            && let Some(prev_state) =
                unsafe { js::cast::<Option<&SuspenseState>>(current.memoized_state.get()) }
        {
            prev_state.hydration_errors.replace(hydration_errors);
        }
        // Fall through to normal Suspense path
        true
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn complete_work(
    current: Option<&Fiber>,
    work_in_progress: &Rc<Fiber>,
    render_lanes: Lanes,
) -> Option<Rc<Fiber>> {
    let new_props = work_in_progress.pending_props.get();
    // Note: This intentionally doesn't check if we're hydrating because comparing
    // to the current tree provider fiber is just as fast and less error-prone.
    // Ideally we would have a special version of the work loop only
    // for hydration.
    pop_tree_context(work_in_progress);
    match work_in_progress.tag.get() {
        INCOMPLETE_FUNCTION_COMPONENT if DISABLE_LEGACY_MODE => {}
        // Fallthrough
        INCOMPLETE_FUNCTION_COMPONENT
        | LAZY_COMPONENT
        | SIMPLE_MEMO_COMPONENT
        | FUNCTION_COMPONENT
        | FORWARD_REF
        | FRAGMENT
        | MODE
        | PROFILER
        | CONTEXT_CONSUMER
        | MEMO_COMPONENT => {
            bubble_properties(work_in_progress);
            return None;
        }
        CLASS_COMPONENT => {
            let component = work_in_progress.r#type.get();
            if is_legacy_context_provider(component) {
                pop_legacy_context(work_in_progress);
            }
            bubble_properties(work_in_progress);
            return None;
        }
        HOST_ROOT => {
            let fiber_root: &FiberRoot = unsafe { js::cast(work_in_progress.state_node.get()) };

            if ENABLE_TRANSITION_TRACING {
                let transitions = get_work_in_progress_transitions();
                // We set the Passive flag here because if there are new transitions,
                // we will need to schedule callbacks and process the transitions,
                // which we do in the passive phase
                if transitions.is_some() {
                    work_in_progress
                        .flags
                        .set(work_in_progress.flags.get() | PASSIVE);
                }
            }

            let mut previous_cache: Option<&Rc<Cache>> = js::null();
            if let Some(current) = current {
                previous_cache =
                    Some(&unsafe { js::cast::<&RootState>(current.memoized_state.get()) }.cache);
            }
            let cache =
                &unsafe { js::cast::<&RootState>(work_in_progress.memoized_state.get()) }.cache;
            if !js::strict_equals(cache, previous_cache) {
                // Run passive effects to retain/release the cache.
                work_in_progress
                    .flags
                    .set(work_in_progress.flags.get() | PASSIVE);
            }
            pop_cache_provider(work_in_progress, cache);

            if ENABLE_TRANSITION_TRACING {
                pop_root_marker_instance(work_in_progress);
            }

            pop_root_transition(work_in_progress, fiber_root, render_lanes);
            pop_host_container(work_in_progress);
            pop_top_level_legacy_context_object(work_in_progress);
            if js::truthy(fiber_root.pending_context.get()) {
                fiber_root.context.set(fiber_root.pending_context.get());
                fiber_root.pending_context.set(None);
            }
            if current.is_none_or(|current| current.child.borrow().is_none()) {
                // If we hydrated, pop so that we can delete any remaining children
                // that weren't hydrated.
                let was_hydrated = pop_hydration_state(work_in_progress);
                if was_hydrated {
                    emit_pending_hydration_warnings();
                    // If we hydrated, then we'll need to schedule an update for
                    // the commit side-effects on the root.
                    mark_update(work_in_progress);
                } else if let Some(current) = current {
                    let prev_state: &RootState = unsafe { js::cast(current.memoized_state.get()) };
                    if
                    // Check if this is a client root
                    !prev_state.is_dehydrated
                        // Check if we reverted to client rendering (e.g. due to an error)
                        || (work_in_progress.flags.get() & FORCE_CLIENT_RENDER) != NO_FLAGS
                    {
                        // Schedule an effect to clear this container at the start of the
                        // next commit. This handles the case of React rendering into a
                        // container with previous children. It's also safe to do for
                        // updates too, because current.child would only be null if the
                        // previous render was null (so the container would already
                        // be empty).
                        work_in_progress
                            .flags
                            .set(work_in_progress.flags.get() | SNAPSHOT);

                        // If this was a forced client render, there may have been
                        // recoverable errors during first hydration attempt. If so, add
                        // them to a queue so we can log them in the commit phase.
                        upgrade_hydration_errors_to_recoverable();
                    }
                }
            }
            update_host_container(current, work_in_progress);
            bubble_properties(work_in_progress);
            if ENABLE_TRANSITION_TRACING
                && (work_in_progress.subtree_flags.get() & VISIBILITY) != NO_FLAGS
            {
                // If any of our suspense children toggle visibility, this means that
                // the pending boundaries array needs to be updated, which we only
                // do in the passive phase.
                work_in_progress
                    .flags
                    .set(work_in_progress.flags.get() | PASSIVE);
            }
            return None;
        }
        HOST_HOISTABLE if SUPPORTS_RESOURCES => {
            // The branching here is more complicated than you might expect because
            // a HostHoistable sometimes corresponds to a Resource and sometimes
            // corresponds to an Instance. It can also switch during an update.

            let r#type = work_in_progress.r#type.get();
            let next_resource = work_in_progress.memoized_state.get();
            match current {
                None => {
                    // We are mounting and must Update this Hoistable in this commit
                    // @TODO refactor this block to create the instance here in complete
                    // phase if we are not hydrating.
                    mark_update(work_in_progress);
                    if let Some(next_resource) = next_resource {
                        // This is a Hoistable Resource

                        // This must come at the very end of the complete phase.
                        bubble_properties(work_in_progress);
                        preload_resource_and_suspend_if_needed(
                            work_in_progress,
                            next_resource,
                            r#type,
                            new_props,
                            render_lanes,
                        );
                        return None;
                    } else {
                        // This is a Hoistable Instance
                        // This must come at the very end of the complete phase.
                        bubble_properties(work_in_progress);
                        preload_instance_and_suspend_if_needed(
                            work_in_progress,
                            r#type,
                            js::null(),
                            new_props,
                            render_lanes,
                        );
                        return None;
                    }
                }
                Some(current) => {
                    // This is an update.
                    if let Some(next_resource) = next_resource {
                        // This is a Resource
                        if !js::strict_equals(next_resource, current.memoized_state.get()) {
                            // we have a new Resource. we need to update
                            mark_update(work_in_progress);
                            // This must come at the very end of the complete phase.
                            bubble_properties(work_in_progress);
                            // This must come at the very end of the complete phase, because it might
                            // throw to suspend, and if the resource immediately loads, the work loop
                            // will resume rendering as if the work-in-progress completed. So it must
                            // fully complete.
                            preload_resource_and_suspend_if_needed(
                                work_in_progress,
                                next_resource,
                                r#type,
                                new_props,
                                render_lanes,
                            );
                            return None;
                        } else {
                            // This must come at the very end of the complete phase.
                            bubble_properties(work_in_progress);
                            work_in_progress
                                .flags
                                .set(work_in_progress.flags.get() & !MAY_SUSPEND_COMMIT);
                            return None;
                        }
                    } else {
                        let old_props = current.memoized_props.get();
                        // This is an Instance
                        // We may have props to update on the Hoistable instance.
                        if SUPPORTS_MUTATION {
                            if !js::strict_equals(old_props, new_props) {
                                mark_update(work_in_progress);
                            }
                        } else {
                            // We use the updateHostComponent path because it produces
                            // the update queue we need for Hoistables.
                            update_host_component(
                                current,
                                work_in_progress,
                                r#type,
                                new_props,
                                render_lanes,
                            );
                        }
                        // This must come at the very end of the complete phase.
                        bubble_properties(work_in_progress);
                        preload_instance_and_suspend_if_needed(
                            work_in_progress,
                            r#type,
                            old_props,
                            new_props,
                            render_lanes,
                        );
                        return None;
                    }
                }
            }
        }
        // Fall through
        HOST_HOISTABLE | HOST_SINGLETON if SUPPORTS_SINGLETONS => {
            pop_host_context(work_in_progress);
            let root_container_instance = get_root_host_container();
            let r#type = work_in_progress.r#type.get();
            if let Some(current) = current
                && work_in_progress.state_node.get().is_some()
            {
                if SUPPORTS_MUTATION {
                    let old_props = current.memoized_props.get();
                    if !js::strict_equals(old_props, new_props) {
                        mark_update(work_in_progress);
                    }
                } else {
                    update_host_component(
                        current,
                        work_in_progress,
                        r#type,
                        new_props,
                        render_lanes,
                    );
                }
            } else {
                if !js::truthy(new_props) {
                    if work_in_progress.state_node.get().is_none() {
                        panic!(
                            "{}",
                            concat!(
                                "We must have new props for new mounts. This error is likely ",
                                "caused by a bug in React. Please file an issue."
                            )
                        );
                    }

                    // This can happen when we abort work.
                    bubble_properties(work_in_progress);
                    if ENABLE_VIEW_TRANSITION {
                        // Host Components act as their own View Transitions which doesn't run enter/exit animations.
                        // We clear any ViewTransitionStatic flag bubbled from inner View Transitions.
                        work_in_progress
                            .subtree_flags
                            .set(work_in_progress.subtree_flags.get() & !VIEW_TRANSITION_STATIC);
                    }
                    return None;
                }

                let current_host_context = get_host_context();
                let was_hydrated = pop_hydration_state(work_in_progress);
                if was_hydrated {
                    // We ignore the boolean indicating there is an updateQueue because
                    // it is used only to set text children and HostSingletons do not
                    // use them.
                    prepare_to_hydrate_host_instance(work_in_progress, current_host_context);
                } else {
                    let instance = resolve_singleton_instance(
                        r#type,
                        new_props,
                        root_container_instance,
                        current_host_context,
                        true,
                    );
                    work_in_progress.state_node.set(Some(instance));
                    mark_update(work_in_progress);
                }
            }
            bubble_properties(work_in_progress);
            if ENABLE_VIEW_TRANSITION {
                // Host Components act as their own View Transitions which doesn't run enter/exit animations.
                // We clear any ViewTransitionStatic flag bubbled from inner View Transitions.
                work_in_progress
                    .subtree_flags
                    .set(work_in_progress.subtree_flags.get() & !VIEW_TRANSITION_STATIC);
            }
            return None;
        }
        // Fall through
        HOST_HOISTABLE | HOST_SINGLETON | HOST_COMPONENT => {
            pop_host_context(work_in_progress);
            let r#type = work_in_progress.r#type.get();
            if let Some(current) = current
                && work_in_progress.state_node.get().is_some()
            {
                update_host_component(current, work_in_progress, r#type, new_props, render_lanes);
            } else {
                if !js::truthy(new_props) {
                    if work_in_progress.state_node.get().is_none() {
                        panic!(
                            "{}",
                            concat!(
                                "We must have new props for new mounts. This error is likely ",
                                "caused by a bug in React. Please file an issue."
                            )
                        );
                    }

                    // This can happen when we abort work.
                    bubble_properties(work_in_progress);
                    if ENABLE_VIEW_TRANSITION {
                        // Host Components act as their own View Transitions which doesn't run enter/exit animations.
                        // We clear any ViewTransitionStatic flag bubbled from inner View Transitions.
                        work_in_progress
                            .subtree_flags
                            .set(work_in_progress.subtree_flags.get() & !VIEW_TRANSITION_STATIC);
                    }
                    return None;
                }

                let current_host_context = get_host_context();
                // TODO: Move createInstance to beginWork and keep it on a context
                // "stack" as the parent. Then append children as we go in beginWork
                // or completeWork depending on whether we want to add them top->down or
                // bottom->up. Top->down is faster in IE11.
                let was_hydrated = pop_hydration_state(work_in_progress);
                if was_hydrated {
                    // TODO: Move this and createInstance step into the beginPhase
                    // to consolidate.
                    prepare_to_hydrate_host_instance(work_in_progress, current_host_context);
                    if finalize_hydrated_children(
                        work_in_progress.state_node.get(),
                        r#type,
                        new_props,
                        current_host_context,
                    ) {
                        work_in_progress
                            .flags
                            .set(work_in_progress.flags.get() | HYDRATE);
                    }
                } else {
                    let root_container_instance = get_root_host_container();
                    let instance = create_instance(
                        r#type,
                        new_props,
                        root_container_instance,
                        current_host_context,
                        work_in_progress,
                    );
                    // TODO: For persistent renderers, we should pass children as part
                    // of the initial instance creation
                    mark_cloned(work_in_progress);
                    append_all_children(Some(instance), work_in_progress, false, false);
                    work_in_progress.state_node.set(Some(instance));

                    // Certain renderers require commit-time effects for initial mount.
                    // (eg DOM renderer supports auto-focus for certain elements).
                    // Make sure such renderers get scheduled for later work.
                    if finalize_initial_children(
                        Some(instance),
                        r#type,
                        new_props,
                        current_host_context,
                    ) {
                        mark_update(work_in_progress);
                    }
                }
            }
            bubble_properties(work_in_progress);
            if ENABLE_VIEW_TRANSITION {
                // Host Components act as their own View Transitions which doesn't run enter/exit animations.
                // We clear any ViewTransitionStatic flag bubbled from inner View Transitions.
                work_in_progress
                    .subtree_flags
                    .set(work_in_progress.subtree_flags.get() & !VIEW_TRANSITION_STATIC);
            }

            // This must come at the very end of the complete phase, because it might
            // throw to suspend, and if the resource immediately loads, the work loop
            // will resume rendering as if the work-in-progress completed. So it must
            // fully complete.
            preload_instance_and_suspend_if_needed(
                work_in_progress,
                work_in_progress.r#type.get(),
                match current {
                    None => js::null(),
                    Some(current) => current.memoized_props.get(),
                },
                work_in_progress.pending_props.get(),
                render_lanes,
            );
            return None;
        }
        HOST_TEXT => {
            let new_text = new_props;
            if let Some(current) = current
                && work_in_progress.state_node.get().is_some()
            {
                let old_text = current.memoized_props.get();
                // If we have an alternate, that means this is an update and we need
                // to schedule a side-effect to do the updates.
                update_host_text(current, work_in_progress, old_text, new_text);
            } else {
                if !matches!(js::classify(new_text), Kind::String(_))
                    && work_in_progress.state_node.get().is_none()
                {
                    panic!(
                        "{}",
                        concat!(
                            "We must have new props for new mounts. This error is likely ",
                            "caused by a bug in React. Please file an issue."
                        )
                    );
                }
                // This can happen when we abort work.
                let root_container_instance = get_root_host_container();
                let current_host_context = get_host_context();
                let was_hydrated = pop_hydration_state(work_in_progress);
                if was_hydrated {
                    prepare_to_hydrate_host_text_instance(work_in_progress);
                } else {
                    mark_cloned(work_in_progress);
                    work_in_progress.state_node.set(Some(create_text_instance(
                        new_text,
                        root_container_instance,
                        current_host_context,
                        work_in_progress,
                    )));
                }
            }
            bubble_properties(work_in_progress);
            return None;
        }
        ACTIVITY_COMPONENT => {
            let next_state: Option<&ActivityState> =
                unsafe { js::cast(work_in_progress.memoized_state.get()) };

            if current.is_none_or(|current| current.memoized_state.get().is_some()) {
                let fallthrough_to_normal_offscreen_path =
                    complete_dehydrated_activity_boundary(current, work_in_progress, next_state);
                if !fallthrough_to_normal_offscreen_path {
                    if (work_in_progress.flags.get() & FORCE_CLIENT_RENDER) != NO_FLAGS {
                        pop_suspense_handler(work_in_progress);
                        // Special case. There were remaining unhydrated nodes. We treat
                        // this as a mismatch. Revert to client rendering.
                        return Some(Rc::clone(work_in_progress));
                    } else {
                        pop_suspense_handler(work_in_progress);
                        // Did not finish hydrating, either because this is the initial
                        // render or because something suspended.
                        return None;
                    }
                }

                if (work_in_progress.flags.get() & DID_CAPTURE) != NO_FLAGS {
                    // We called retryActivityComponentWithoutHydrating and tried client rendering
                    // but now we suspended again. We should never arrive here because we should
                    // not have pushed a suspense handler during that second pass and it should
                    // instead have suspended above.
                    panic!(
                        "Client rendering an Activity suspended it again. This is a bug in React."
                    );
                }

                // Continue with the normal Activity path.
            }

            bubble_properties(work_in_progress);
            return None;
        }
        SUSPENSE_COMPONENT => {
            let next_state: Option<&SuspenseState> =
                unsafe { js::cast(work_in_progress.memoized_state.get()) };

            // Special path for dehydrated boundaries. We may eventually move this
            // to its own fiber type so that we can add other kinds of hydration
            // boundaries that aren't associated with a Suspense tree. In anticipation
            // of such a refactor, all the hydration logic is contained in
            // this branch.
            if current.is_none_or(|current| {
                unsafe { js::cast::<Option<&SuspenseState>>(current.memoized_state.get()) }
                    .is_some_and(|memoized_state| memoized_state.dehydrated.is_some())
            }) {
                let fallthrough_to_normal_suspense_path =
                    complete_dehydrated_suspense_boundary(current, work_in_progress, next_state);
                if !fallthrough_to_normal_suspense_path {
                    if (work_in_progress.flags.get() & FORCE_CLIENT_RENDER) != NO_FLAGS {
                        pop_suspense_handler(work_in_progress);
                        // Special case. There were remaining unhydrated nodes. We treat
                        // this as a mismatch. Revert to client rendering.
                        return Some(Rc::clone(work_in_progress));
                    } else {
                        pop_suspense_handler(work_in_progress);
                        // Did not finish hydrating, either because this is the initial
                        // render or because something suspended.
                        return None;
                    }
                }

                // Continue with the normal Suspense path.
            }

            pop_suspense_handler(work_in_progress);

            if (work_in_progress.flags.get() & DID_CAPTURE) != NO_FLAGS {
                // Something suspended. Re-render with the fallback children.
                work_in_progress.lanes.set(render_lanes);
                if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                    && (work_in_progress.mode.get() & PROFILE_MODE) != NO_MODE
                {
                    transfer_actual_duration(work_in_progress);
                }
                // Don't bubble properties in this case.
                return Some(Rc::clone(work_in_progress));
            }

            let next_did_timeout = next_state.is_some();
            let prev_did_timeout =
                current.is_some_and(|current| current.memoized_state.get().is_some());

            if next_did_timeout {
                let offscreen_fiber = work_in_progress.child.borrow().clone().unwrap();
                let mut previous_cache: Option<&Rc<Cache>> = js::null();
                let alternate = offscreen_fiber.alternate.borrow().clone();
                if let Some(alternate) = alternate
                    && let Some(memoized_state) = unsafe {
                        js::cast::<Option<&OffscreenState>>(alternate.memoized_state.get())
                    }
                    && let Some(cache_pool) =
                        unsafe { js::cast::<Option<&SpawnedCachePool>>(memoized_state.cache_pool) }
                {
                    previous_cache = Some(&cache_pool.pool);
                }
                let mut cache: Option<&Rc<Cache>> = js::null();
                if let Some(memoized_state) = unsafe {
                    js::cast::<Option<&OffscreenState>>(offscreen_fiber.memoized_state.get())
                } && let Some(cache_pool) =
                    unsafe { js::cast::<Option<&SpawnedCachePool>>(memoized_state.cache_pool) }
                {
                    cache = Some(&cache_pool.pool);
                }
                if !js::strict_equals(cache, previous_cache) {
                    // Run passive effects to retain/release the cache.
                    offscreen_fiber
                        .flags
                        .set(offscreen_fiber.flags.get() | PASSIVE);
                }
            }

            // If the suspended state of the boundary changes, we need to schedule
            // a passive effect, which is when we process the transitions
            if next_did_timeout != prev_did_timeout {
                if ENABLE_TRANSITION_TRACING {
                    let offscreen_fiber = work_in_progress.child.borrow().clone().unwrap();
                    offscreen_fiber
                        .flags
                        .set(offscreen_fiber.flags.get() | PASSIVE);
                }

                // If the suspended state of the boundary changes, we need to schedule
                // an effect to toggle the subtree's visibility. When we switch from
                // fallback -> primary, the inner Offscreen fiber schedules this effect
                // as part of its normal complete phase. But when we switch from
                // primary -> fallback, the inner Offscreen fiber does not have a complete
                // phase. So we need to schedule its effect here.
                //
                // We also use this flag to connect/disconnect the effects, but the same
                // logic applies: when re-connecting, the Offscreen fiber's complete
                // phase will handle scheduling the effect. It's only when the fallback
                // is active that we have to do anything special.
                if next_did_timeout {
                    let offscreen_fiber = work_in_progress.child.borrow().clone().unwrap();
                    offscreen_fiber
                        .flags
                        .set(offscreen_fiber.flags.get() | VISIBILITY);
                }
            }

            let retry_queue: Option<&RetryQueue> =
                unsafe { js::cast(work_in_progress.update_queue.get()) };
            schedule_retry_effect(work_in_progress, retry_queue);

            if ENABLE_SUSPENSE_CALLBACK
                && work_in_progress.update_queue.get().is_some()
                && js::get(work_in_progress.memoized_props.get(), "suspenseCallback").is_some()
            {
                // Always notify the callback
                // TODO: Move to passive phase
                work_in_progress
                    .flags
                    .set(work_in_progress.flags.get() | UPDATE);
            }
            bubble_properties(work_in_progress);
            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                && (work_in_progress.mode.get() & PROFILE_MODE) != NO_MODE
                && next_did_timeout
            {
                // Don't count time spent in a timed out Suspense subtree as part of the base duration.
                let primary_child_fragment = work_in_progress.child.borrow().clone();
                if let Some(primary_child_fragment) = primary_child_fragment {
                    // $FlowFixMe[unsafe-arithmetic] Flow doesn't support type casting in combination with the -= operator
                    work_in_progress.tree_base_duration.set(Some(
                        work_in_progress.tree_base_duration.get().unwrap()
                            - primary_child_fragment.tree_base_duration.get().unwrap(),
                    ));
                }
            }
            return None;
        }
        HOST_PORTAL => {
            pop_host_container(work_in_progress);
            update_host_container(current, work_in_progress);
            if current.is_none() {
                let portal: &PortalStateNode =
                    unsafe { js::cast(work_in_progress.state_node.get()) };
                prepare_portal_mount(portal.container_info);
            }
            work_in_progress
                .flags
                .set(work_in_progress.flags.get() | PORTAL_STATIC);
            bubble_properties(work_in_progress);
            return None;
        }
        CONTEXT_PROVIDER => {
            // Pop provider fiber
            let context: &ReactContext<Option<&'static Unknown>> =
                unsafe { js::cast(work_in_progress.r#type.get()) };
            pop_provider(context, work_in_progress);
            bubble_properties(work_in_progress);
            return None;
        }
        INCOMPLETE_CLASS_COMPONENT if DISABLE_LEGACY_MODE => {}
        INCOMPLETE_CLASS_COMPONENT => {
            // Same as class component case. I put it down here so that the tags are
            // sequential to ensure this switch is compiled to a jump table.
            let component = work_in_progress.r#type.get();
            if is_legacy_context_provider(component) {
                pop_legacy_context(work_in_progress);
            }
            bubble_properties(work_in_progress);
            return None;
        }
        SUSPENSE_LIST_COMPONENT => {
            pop_suspense_list_context(work_in_progress);

            let render_state: Option<&SuspenseListRenderState> =
                unsafe { js::cast(work_in_progress.memoized_state.get()) };

            let Some(render_state) = render_state else {
                // We're running in the default, "independent" mode.
                // We don't do anything in this mode.
                bubble_properties(work_in_progress);
                return None;
            };

            let mut did_suspend_already = (work_in_progress.flags.get() & DID_CAPTURE) != NO_FLAGS;

            let rendered_tail = render_state.rendering.borrow().clone();
            match rendered_tail {
                None => {
                    // We just rendered the head.
                    if !did_suspend_already {
                        // This is the first pass. We need to figure out if anything is still
                        // suspended in the rendered set.

                        // If new content unsuspended, but there's still some content that
                        // didn't. Then we need to do a second pass that forces everything
                        // to keep showing their fallbacks.

                        // We might be suspended if something in this render pass suspended, or
                        // something in the previous committed pass suspended. Otherwise,
                        // there's no chance so we can skip the expensive call to
                        // findFirstSuspended.
                        let cannot_be_suspended = render_has_not_suspended_yet()
                            && current.is_none_or(|current| {
                                (current.flags.get() & DID_CAPTURE) == NO_FLAGS
                            });
                        if !cannot_be_suspended {
                            let mut row = work_in_progress.child.borrow().clone();
                            while let Some(current_row) = row {
                                let suspended = find_first_suspended(&current_row);
                                if let Some(suspended) = suspended {
                                    work_in_progress
                                        .flags
                                        .set(work_in_progress.flags.get() | DID_CAPTURE);
                                    cut_off_tail_if_needed(render_state, false);

                                    // If this is a newly suspended tree, it might not get committed as
                                    // part of the second pass. In that case nothing will subscribe to
                                    // its thenables. Instead, we'll transfer its thenables to the
                                    // SuspenseList so that it can retry if they resolve.
                                    // There might be multiple of these in the list but since we're
                                    // going to wait for all of them anyway, it doesn't really matter
                                    // which ones gets to ping. In theory we could get clever and keep
                                    // track of how many dependencies remain but it gets tricky because
                                    // in the meantime, we can add/remove/change items and dependencies.
                                    // We might bail out of the loop before finding any but that
                                    // doesn't matter since that means that the other boundaries that
                                    // we did find already has their listeners attached.
                                    let retry_queue = suspended.update_queue.get();
                                    work_in_progress.update_queue.set(retry_queue);
                                    schedule_retry_effect(work_in_progress, unsafe {
                                        js::cast(retry_queue)
                                    });

                                    // Rerender the whole list, but this time, we'll force fallbacks
                                    // to stay in place.
                                    // Reset the effect flags before doing the second pass since that's now invalid.
                                    // Reset the child fibers to their original state.
                                    work_in_progress.subtree_flags.set(NO_FLAGS);
                                    reset_child_fibers(work_in_progress, render_lanes);

                                    // Set up the Suspense List Context to force suspense and
                                    // immediately rerender the children.
                                    push_suspense_list_context(
                                        work_in_progress,
                                        set_shallow_suspense_list_context(
                                            SUSPENSE_STACK_CURSOR
                                                .with(|cursor| *cursor.current.borrow()),
                                            FORCE_SUSPENSE_FALLBACK,
                                        ),
                                    );
                                    if get_is_hydrating() {
                                        // Re-apply tree fork since we popped the tree fork context in the beginning of this function.
                                        push_tree_fork(
                                            work_in_progress,
                                            render_state.tree_fork_count.get(),
                                        );
                                    }
                                    // Don't bubble properties in this case.
                                    return work_in_progress.child.borrow().clone();
                                }
                                row = current_row.sibling.borrow().clone();
                            }
                        }

                        if render_state.tail.borrow().is_some() && now() > get_render_target_time()
                        {
                            // We have already passed our CPU deadline but we still have rows
                            // left in the tail. We'll just give up further attempts to render
                            // the main content and only render fallbacks.
                            work_in_progress
                                .flags
                                .set(work_in_progress.flags.get() | DID_CAPTURE);
                            did_suspend_already = true;

                            cut_off_tail_if_needed(render_state, false);

                            // Since nothing actually suspended, there will nothing to ping this
                            // to get it started back up to attempt the next item. While in terms
                            // of priority this work has the same priority as this current render,
                            // it's not part of the same transition once the transition has
                            // committed. If it's sync, we still want to yield so that it can be
                            // painted. Conceptually, this is really the same as pinging.
                            // We can use any RetryLane even if it's the one currently rendering
                            // since we're leaving it behind on this node.
                            work_in_progress.lanes.set(SOME_RETRY_LANE);
                        }
                    } else {
                        cut_off_tail_if_needed(render_state, false);
                    }
                    // Next we're going to render the tail.
                }
                Some(rendered_tail) => {
                    // Append the rendered row to the child list.
                    if !did_suspend_already {
                        let suspended = find_first_suspended(&rendered_tail);
                        if let Some(suspended) = suspended {
                            work_in_progress
                                .flags
                                .set(work_in_progress.flags.get() | DID_CAPTURE);
                            did_suspend_already = true;

                            // Ensure we transfer the update queue to the parent so that it doesn't
                            // get lost if this row ends up dropped during a second pass.
                            let retry_queue = suspended.update_queue.get();
                            work_in_progress.update_queue.set(retry_queue);
                            schedule_retry_effect(work_in_progress, unsafe {
                                js::cast(retry_queue)
                            });

                            cut_off_tail_if_needed(render_state, true);
                            // This might have been modified.
                            if render_state.tail.borrow().is_none()
                                && render_state.tail_mode.get()
                                    != Some(SuspenseListTailMode::Collapsed)
                                && render_state.tail_mode.get()
                                    != Some(SuspenseListTailMode::Visible)
                                && rendered_tail.alternate.borrow().is_none()
                                && !get_is_hydrating()
                            // We don't cut it if we're hydrating.
                            {
                                // We're done.
                                bubble_properties(work_in_progress);
                                return None;
                            }
                        } else if
                        // The time it took to render last row is greater than the remaining
                        // time we have to render. So rendering one more row would likely
                        // exceed it.
                        now() * 2.0 - render_state.rendering_start_time.get()
                            > get_render_target_time()
                            && render_lanes != OFFSCREEN_LANE
                        {
                            // We have now passed our CPU deadline and we'll just give up further
                            // attempts to render the main content and only render fallbacks.
                            // The assumption is that this is usually faster.
                            work_in_progress
                                .flags
                                .set(work_in_progress.flags.get() | DID_CAPTURE);
                            did_suspend_already = true;

                            cut_off_tail_if_needed(render_state, false);

                            // Since nothing actually suspended, there will nothing to ping this
                            // to get it started back up to attempt the next item. While in terms
                            // of priority this work has the same priority as this current render,
                            // it's not part of the same transition once the transition has
                            // committed. If it's sync, we still want to yield so that it can be
                            // painted. Conceptually, this is really the same as pinging.
                            // We can use any RetryLane even if it's the one currently rendering
                            // since we're leaving it behind on this node.
                            work_in_progress.lanes.set(SOME_RETRY_LANE);
                        }
                    }
                    if render_state.is_backwards.get() {
                        // Append to the beginning of the list.
                        let child = work_in_progress.child.borrow().clone();
                        rendered_tail.sibling.replace(child);
                        work_in_progress.child.replace(Some(rendered_tail));
                    } else {
                        let previous_sibling = render_state.last.borrow().clone();
                        if let Some(previous_sibling) = previous_sibling {
                            previous_sibling
                                .sibling
                                .replace(Some(Rc::clone(&rendered_tail)));
                        } else {
                            work_in_progress
                                .child
                                .replace(Some(Rc::clone(&rendered_tail)));
                        }
                        render_state.last.replace(Some(rendered_tail));
                    }
                }
            }

            let tail = render_state.tail.borrow().clone();
            if let Some(next) = tail {
                // We still have tail rows to render.
                // Pop a row.
                // TODO: Consider storing the first of the new mount tail in the state so
                // that we don't have to recompute this for every row in the list.
                let only_new_mounts = is_only_new_mounts(&next);
                render_state.rendering.replace(Some(Rc::clone(&next)));
                let sibling = next.sibling.borrow().clone();
                render_state.tail.replace(sibling);
                render_state.rendering_start_time.set(now());
                next.sibling.replace(None);

                // Restore the context.
                // TODO: We can probably just avoid popping it instead and only
                // setting it the first time we go from not suspended to suspended.
                let mut suspense_context =
                    SUSPENSE_STACK_CURSOR.with(|cursor| *cursor.current.borrow());
                if did_suspend_already {
                    suspense_context = set_shallow_suspense_list_context(
                        suspense_context,
                        FORCE_SUSPENSE_FALLBACK,
                    );
                } else {
                    suspense_context = set_default_shallow_suspense_list_context(suspense_context);
                }
                if render_state.tail_mode.get() == Some(SuspenseListTailMode::Visible)
                    || render_state.tail_mode.get() == Some(SuspenseListTailMode::Collapsed)
                    || !only_new_mounts
                    // TODO: While hydrating, we still let it suspend the parent. Tail mode hidden has broken
                    // hydration anyway right now but this preserves the previous semantics out of caution.
                    // Once proper hydration is implemented, this special case should be removed as it should
                    // never be needed.
                    || get_is_hydrating()
                {
                    push_suspense_list_context(work_in_progress, suspense_context);
                } else {
                    // If we are rendering in 'hidden' (default) tail mode, then we if we suspend in the
                    // tail itself, we can delete it rather than suspend the parent. So we act as a catch in that
                    // case. For 'collapsed' we need to render at least one in suspended state, after which we'll
                    // have cut off the rest to never attempt it so it never hits this case.
                    // If this is an updated node, we cannot delete it from the tail so it's effectively visible.
                    // As a consequence, if it resuspends it actually suspends the parent by taking the other path.
                    push_suspense_list_catch(work_in_progress, suspense_context);
                }
                // Do a pass over the next row.
                if get_is_hydrating() {
                    // Re-apply tree fork since we popped the tree fork context in the beginning of this function.
                    push_tree_fork(work_in_progress, render_state.tree_fork_count.get());
                }
                // Don't bubble properties in this case.
                return Some(next);
            }
            bubble_properties(work_in_progress);
            return None;
        }
        SCOPE_COMPONENT if ENABLE_SCOPE_API => {
            if current.is_none() {
                let scope_instance = js::unknown_of(create_scope_instance());
                work_in_progress.state_node.set(Some(scope_instance));
                prepare_scope_update(scope_instance, work_in_progress);
                if work_in_progress.r#ref.get().is_some() {
                    // Scope components always do work in the commit phase if there's a
                    // ref attached.
                    mark_update(work_in_progress);
                }
            } else if work_in_progress.r#ref.get().is_some() {
                // Scope components always do work in the commit phase if there's a
                // ref attached.
                mark_update(work_in_progress);
            }
            bubble_properties(work_in_progress);
            return None;
        }
        OFFSCREEN_COMPONENT | LEGACY_HIDDEN_COMPONENT => {
            pop_suspense_handler(work_in_progress);
            pop_hidden_context(work_in_progress);
            let next_state: Option<&OffscreenState> =
                unsafe { js::cast(work_in_progress.memoized_state.get()) };
            let next_is_hidden = next_state.is_some();

            // Schedule a Visibility effect if the visibility has changed
            if ENABLE_LEGACY_HIDDEN && work_in_progress.tag.get() == LEGACY_HIDDEN_COMPONENT {
                // LegacyHidden doesn't do any hiding — it only pre-renders.
            } else if let Some(current) = current {
                let prev_state: Option<&OffscreenState> =
                    unsafe { js::cast(current.memoized_state.get()) };
                let prev_is_hidden = prev_state.is_some();
                if prev_is_hidden != next_is_hidden {
                    work_in_progress
                        .flags
                        .set(work_in_progress.flags.get() | VISIBILITY);
                }
            } else {
                // On initial mount, we only need a Visibility effect if the tree
                // is hidden.
                if next_is_hidden {
                    work_in_progress
                        .flags
                        .set(work_in_progress.flags.get() | VISIBILITY);
                }
            }

            if !next_is_hidden
                || (!DISABLE_LEGACY_MODE
                    && (work_in_progress.mode.get() & CONCURRENT_MODE) == NO_MODE)
            {
                bubble_properties(work_in_progress);
            } else {
                // Don't bubble properties for hidden children unless we're rendering
                // at offscreen priority.
                if includes_some_lane(render_lanes, OFFSCREEN_LANE)
                    // Also don't bubble if the tree suspended
                    && (work_in_progress.flags.get() & DID_CAPTURE) == NO_LANES
                {
                    bubble_properties(work_in_progress);
                    // Check if there was an insertion or update in the hidden subtree.
                    // If so, we need to hide those nodes in the commit phase, so
                    // schedule a visibility effect.
                    if (!ENABLE_LEGACY_HIDDEN
                        || work_in_progress.tag.get() != LEGACY_HIDDEN_COMPONENT)
                        && (work_in_progress.subtree_flags.get() & (PLACEMENT | UPDATE)) != NO_FLAGS
                    {
                        work_in_progress
                            .flags
                            .set(work_in_progress.flags.get() | VISIBILITY);
                    }
                }
            }

            let offscreen_queue: Option<&OffscreenQueue> =
                unsafe { js::cast(work_in_progress.update_queue.get()) };
            if let Some(offscreen_queue) = offscreen_queue {
                let retry_queue = offscreen_queue.retry_queue.get();
                schedule_retry_effect(work_in_progress, retry_queue);
            }

            let mut previous_cache: Option<&Rc<Cache>> = js::null();
            if let Some(current) = current
                && let Some(memoized_state) =
                    unsafe { js::cast::<Option<&OffscreenState>>(current.memoized_state.get()) }
                && let Some(cache_pool) =
                    unsafe { js::cast::<Option<&SpawnedCachePool>>(memoized_state.cache_pool) }
            {
                previous_cache = Some(&cache_pool.pool);
            }
            let mut cache: Option<&Rc<Cache>> = js::null();
            if let Some(memoized_state) = unsafe {
                js::cast::<Option<&OffscreenState>>(work_in_progress.memoized_state.get())
            } && let Some(cache_pool) =
                unsafe { js::cast::<Option<&SpawnedCachePool>>(memoized_state.cache_pool) }
            {
                cache = Some(&cache_pool.pool);
            }
            if !js::strict_equals(cache, previous_cache) {
                // Run passive effects to retain/release the cache.
                work_in_progress
                    .flags
                    .set(work_in_progress.flags.get() | PASSIVE);
            }

            pop_transition(work_in_progress, current);

            return None;
        }
        CACHE_COMPONENT => {
            let mut previous_cache: Option<&Rc<Cache>> = js::null();
            if let Some(current) = current {
                previous_cache = Some(
                    &unsafe { js::cast::<&CacheComponentState>(current.memoized_state.get()) }
                        .cache,
                );
            }
            let cache =
                &unsafe { js::cast::<&CacheComponentState>(work_in_progress.memoized_state.get()) }
                    .cache;
            if !js::strict_equals(cache, previous_cache) {
                // Run passive effects to retain/release the cache.
                work_in_progress
                    .flags
                    .set(work_in_progress.flags.get() | PASSIVE);
            }
            pop_cache_provider(work_in_progress, cache);
            bubble_properties(work_in_progress);
            return None;
        }
        TRACING_MARKER_COMPONENT => {
            if ENABLE_TRANSITION_TRACING {
                let instance: Option<&TracingMarkerInstance> =
                    unsafe { js::cast(work_in_progress.state_node.get()) };
                if instance.is_some() {
                    pop_marker_instance(work_in_progress);
                }
                bubble_properties(work_in_progress);
            }
            return None;
        }
        VIEW_TRANSITION_COMPONENT => {
            if ENABLE_VIEW_TRANSITION {
                // We're a component that might need an exit transition. This flag will
                // bubble up to the parent tree to indicate that there's a child that
                // might need an exit View Transition upon unmount.
                work_in_progress
                    .flags
                    .set(work_in_progress.flags.get() | VIEW_TRANSITION_STATIC);
                if ENABLE_VIEW_TRANSITION_PARENT_ENTER_EXIT.with(|&enabled| enabled) {
                    let props = work_in_progress.pending_props.get();
                    if !matches!(js::classify(js::get(props, "parentEnter")), Kind::Undefined)
                        || !matches!(js::classify(js::get(props, "parentExit")), Kind::Undefined)
                        || js::get(props, "onParentEnter").is_some()
                        || js::get(props, "onParentExit").is_some()
                        || js::get(props, "onGestureParentEnter").is_some()
                        || js::get(props, "onGestureParentExit").is_some()
                    {
                        work_in_progress
                            .flags
                            .set(work_in_progress.flags.get() | VIEW_TRANSITION_STATIC_PARENT);
                    } else {
                        work_in_progress
                            .flags
                            .set(work_in_progress.flags.get() & !VIEW_TRANSITION_STATIC_PARENT);
                    }
                }
                bubble_properties(work_in_progress);
            }
            return None;
        }
        // Only Legacy Mode completes an errored node.
        THROW if !DISABLE_LEGACY_MODE => return None,
        _ => {}
    }

    panic!(
        "Unknown unit of work tag ({}). This error is likely caused by a bug in React. Please file an issue.",
        work_in_progress.tag.get()
    );
}
