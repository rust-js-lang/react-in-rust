// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use js::Unknown;
use shared::react_feature_flags::{
    ENABLE_COMPONENT_PERFORMANCE_TRACK, ENABLE_PROFILER_TIMER,
    ENABLE_VIEW_TRANSITION_FOR_PERSISTENCE_MODE, ENABLE_VIEW_TRANSITION_PARENT_ENTER_EXIT,
};
use shared::react_types::ViewTransitionProps;

use crate::react_fiber::ViewTransitionState;
use crate::react_fiber_flags::{
    AFFECTED_PARENT_LAYOUT, NO_FLAGS, UPDATE, VIEW_TRANSITION_NAMED_STATIC, VIEW_TRANSITION_STATIC,
    VIEW_TRANSITION_STATIC_PARENT,
};
use crate::react_fiber_view_transition_component::{
    get_view_transition_class_name, get_view_transition_name,
};
use crate::react_fiber_work_loop::{
    schedule_gesture_transition_event, schedule_view_transition_event,
};
use crate::react_internal_types::Fiber;
use crate::react_profiler_timer::track_animating_task;
use crate::react_work_tags::{HOST_COMPONENT, OFFSCREEN_COMPONENT, VIEW_TRANSITION_COMPONENT};

unsafe extern "Rust" {
    #[link_name = "./src/ReactFiberConfig.js#supportsMutation"]
    safe static SUPPORTS_MUTATION: bool;
}

// ReactFiberConfig's view transition operations, on its `Instance`, its
// `Props` and its `InstanceMeasurement`.
#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#applyViewTransitionName"
)]
fn apply_view_transition_name(
    instance: Option<&'static Unknown>,
    name: &str,
    class_name: Option<&str>,
) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#restoreViewTransitionName"
)]
fn restore_view_transition_name(
    instance: Option<&'static Unknown>,
    props: Option<&'static Unknown>,
) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#measureInstance"
)]
fn measure_instance(instance: Option<&'static Unknown>) -> &'static Unknown {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#measureClonedInstance"
)]
fn measure_cloned_instance(instance: &'static Unknown) -> &'static Unknown {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#hasInstanceChanged"
)]
fn has_instance_changed(old_measurement: &Unknown, new_measurement: &Unknown) -> bool {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#hasInstanceAffectedParent"
)]
fn has_instance_affected_parent(old_measurement: &Unknown, new_measurement: &Unknown) -> bool {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#wasInstanceInViewport"
)]
fn was_instance_in_viewport(measurement: &Unknown) -> bool {
    unreachable!()
}

thread_local! {
    pub static SHOULD_START_VIEW_TRANSITION: Cell<bool> = const { Cell::new(false) };
}

pub fn reset_should_start_view_transition() {
    SHOULD_START_VIEW_TRANSITION.set(false);
}

/// The named ViewTransitions that appear, by name: a map the apply gesture
/// phase deletes from too.
pub type AppearingViewTransitions = Rc<RefCell<HashMap<String, Rc<ViewTransitionState>>>>;

thread_local! {
    // This tracks named ViewTransition components found in the accumulateSuspenseyCommit
    // phase that might need to find deleted pairs in the beforeMutation phase.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static APPEARING_VIEW_TRANSITIONS: RefCell<Option<AppearingViewTransitions>> =
        const { RefCell::new(js::null()) };
}

pub fn reset_appearing_view_transitions() {
    APPEARING_VIEW_TRANSITIONS.replace(js::null());
}

pub fn track_appearing_view_transition(name: String, state: Rc<ViewTransitionState>) {
    let appearing_view_transitions =
        APPEARING_VIEW_TRANSITIONS.with_borrow_mut(|appearing_view_transitions| {
            Rc::clone(appearing_view_transitions.get_or_insert_with(Default::default))
        });
    appearing_view_transitions.borrow_mut().insert(name, state);
}

pub fn track_enter_view_transitions(placement: &Fiber) {
    if placement.tag.get() == VIEW_TRANSITION_COMPONENT
        || (placement.subtree_flags.get() & VIEW_TRANSITION_STATIC) != NO_FLAGS
    {
        // If an inserted or appearing Fiber is a ViewTransition component or has one as
        // an immediate child, then that will trigger as an "Enter" in future passes.
        // We don't do anything else for that case in the "before mutation" phase but we
        // still have to mark it as needing to call startViewTransition if nothing else
        // updates.
        SHOULD_START_VIEW_TRANSITION.set(true);
    }
}

/// What a ViewTransition's children that may be canceled are, tupled: each
/// entry's ReactFiberConfig `Instance`, its old name, and its `Props`.
pub type CancelableChildren = Rc<RefCell<Vec<Option<&'static Unknown>>>>;

thread_local! {
    // We can't cancel view transition children until we know that their parent also
    // don't need to transition.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static VIEW_TRANSITION_CANCELABLE_CHILDREN: RefCell<Option<CancelableChildren>> =
        const { RefCell::new(js::null()) }; // tupled array where each entry is [instance: Instance, oldName: string, props: Props]
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn push_view_transition_cancelable_scope() -> Option<CancelableChildren> {
    VIEW_TRANSITION_CANCELABLE_CHILDREN.replace(js::null())
}

#[cfg_attr(rust_js, rust_js::nullable(prev_children))]
pub fn pop_view_transition_cancelable_scope(prev_children: Option<CancelableChildren>) {
    VIEW_TRANSITION_CANCELABLE_CHILDREN.replace(prev_children);
}

thread_local! {
    static VIEW_TRANSITION_HOST_INSTANCE_IDX: Cell<usize> = const { Cell::new(0) };
}

#[cfg_attr(rust_js, rust_js::nullable(collect_measurements))]
fn apply_view_transition_to_host_instances(
    fiber: &Fiber,
    name: &str,
    class_name: Option<&str>,
    collect_measurements: Option<&RefCell<Vec<&'static Unknown>>>,
    stop_at_nested_view_transitions: bool,
) -> bool {
    VIEW_TRANSITION_HOST_INSTANCE_IDX.set(0);
    let child = fiber.child.borrow().clone();
    let in_viewport = apply_view_transition_to_host_instances_recursive(
        child,
        name,
        class_name,
        collect_measurements,
        stop_at_nested_view_transitions,
    );
    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        && ENABLE_COMPONENT_PERFORMANCE_TRACK
        && in_viewport
        && let Some(debug_task) = fiber.debug_task.get()
    {
        track_animating_task(debug_task);
    }
    in_viewport
}

#[cfg_attr(rust_js, rust_js::nullable(collect_measurements))]
fn apply_view_transition_to_host_instances_recursive(
    mut child: Option<Rc<Fiber>>,
    name: &str,
    class_name: Option<&str>,
    collect_measurements: Option<&RefCell<Vec<&'static Unknown>>>,
    stop_at_nested_view_transitions: bool,
) -> bool {
    if !SUPPORTS_MUTATION {
        if ENABLE_VIEW_TRANSITION_FOR_PERSISTENCE_MODE {
            while let Some(current) = child {
                if current.tag.get() == HOST_COMPONENT {
                    let instance = current.state_node.get();
                    // TODO: calculate whether component is in viewport
                    SHOULD_START_VIEW_TRANSITION.set(true);
                    apply_view_transition_name(
                        instance,
                        &if VIEW_TRANSITION_HOST_INSTANCE_IDX.get() == 0 {
                            name.to_string()
                        } else {
                            format!("{name}_{}", VIEW_TRANSITION_HOST_INSTANCE_IDX.get())
                        },
                        class_name,
                    );
                    VIEW_TRANSITION_HOST_INSTANCE_IDX
                        .set(VIEW_TRANSITION_HOST_INSTANCE_IDX.get() + 1);
                } else if current.tag.get() == OFFSCREEN_COMPONENT
                    && current.memoized_state.get().is_some()
                {
                    // Skip any hidden subtrees. They were or are effectively not there.
                } else if current.tag.get() == VIEW_TRANSITION_COMPONENT
                    && stop_at_nested_view_transitions
                {
                    // Skip any nested view transitions for updates since in that case the
                    // inner most one is the one that handles the update.
                } else {
                    let grandchild = current.child.borrow().clone();
                    apply_view_transition_to_host_instances_recursive(
                        grandchild,
                        name,
                        class_name,
                        collect_measurements,
                        stop_at_nested_view_transitions,
                    );
                }
                child = current.sibling.borrow().clone();
            }
            return true;
        } else {
            return false;
        }
    }
    let mut in_viewport = false;
    while let Some(current) = child {
        if current.tag.get() == HOST_COMPONENT {
            let instance = current.state_node.get();
            if let Some(collect_measurements) = collect_measurements {
                let measurement = measure_instance(instance);
                collect_measurements.borrow_mut().push(measurement);
                if was_instance_in_viewport(measurement) {
                    in_viewport = true;
                }
            } else if !in_viewport && was_instance_in_viewport(measure_instance(instance)) {
                in_viewport = true;
            }
            SHOULD_START_VIEW_TRANSITION.set(true);
            apply_view_transition_name(
                instance,
                &if VIEW_TRANSITION_HOST_INSTANCE_IDX.get() == 0 {
                    name.to_string()
                } else {
                    // If we have multiple Host Instances below, we add a suffix to the name to give
                    // each one a unique name.
                    format!("{name}_{}", VIEW_TRANSITION_HOST_INSTANCE_IDX.get())
                },
                class_name,
            );
            VIEW_TRANSITION_HOST_INSTANCE_IDX.set(VIEW_TRANSITION_HOST_INSTANCE_IDX.get() + 1);
        } else if current.tag.get() == OFFSCREEN_COMPONENT && current.memoized_state.get().is_some()
        {
            // Skip any hidden subtrees. They were or are effectively not there.
        } else if current.tag.get() == VIEW_TRANSITION_COMPONENT && stop_at_nested_view_transitions
        {
            // Skip any nested view transitions for updates since in that case the
            // inner most one is the one that handles the update.
        } else {
            let grandchild = current.child.borrow().clone();
            if apply_view_transition_to_host_instances_recursive(
                grandchild,
                name,
                class_name,
                collect_measurements,
                stop_at_nested_view_transitions,
            ) {
                in_viewport = true;
            }
        }
        child = current.sibling.borrow().clone();
    }
    in_viewport
}

fn restore_view_transition_on_host_instances(
    mut child: Option<Rc<Fiber>>,
    stop_at_nested_view_transitions: bool,
) {
    if !SUPPORTS_MUTATION {
        return;
    }
    while let Some(current) = child {
        if current.tag.get() == HOST_COMPONENT {
            let instance = current.state_node.get();
            restore_view_transition_name(instance, current.memoized_props.get());
        } else if current.tag.get() == OFFSCREEN_COMPONENT && current.memoized_state.get().is_some()
        {
            // Skip any hidden subtrees. They were or are effectively not there.
        } else if current.tag.get() == VIEW_TRANSITION_COMPONENT && stop_at_nested_view_transitions
        {
            // Skip any nested view transitions for updates since in that case the
            // inner most one is the one that handles the update.
        } else {
            let grandchild = current.child.borrow().clone();
            restore_view_transition_on_host_instances(grandchild, stop_at_nested_view_transitions);
        }
        child = current.sibling.borrow().clone();
    }
}

fn commit_appearing_pair_view_transitions(placement: &Fiber) {
    if (placement.subtree_flags.get() & VIEW_TRANSITION_NAMED_STATIC) == NO_FLAGS {
        // This has no named view transitions in its subtree.
        return;
    }
    let mut child = placement.child.borrow().clone();
    while let Some(current) = child {
        if current.tag.get() == OFFSCREEN_COMPONENT && current.memoized_state.get().is_some() {
            // This tree was already hidden so we skip it.
        } else {
            commit_appearing_pair_view_transitions(&current);
            if current.tag.get() == VIEW_TRANSITION_COMPONENT
                && (current.flags.get() & VIEW_TRANSITION_NAMED_STATIC) != NO_FLAGS
            {
                let instance: &ViewTransitionState = unsafe { js::cast(current.state_node.get()) };
                if instance.paired.borrow().is_some() {
                    let props: &ViewTransitionProps =
                        unsafe { js::cast(current.memoized_props.get()) };
                    let Some(name) = props.name.as_deref().filter(|name| *name != "auto") else {
                        panic!("Found a pair with an auto name. This is a bug in React.");
                    };
                    let class_name = get_view_transition_class_name(props.default, props.share);
                    if class_name.as_deref() != Some("none") {
                        // We found a new appearing view transition with the same name as this deletion.
                        // We'll transition between them.
                        let in_viewport = apply_view_transition_to_host_instances(
                            &current,
                            name,
                            class_name.as_deref(),
                            None,
                            false,
                        );
                        if !in_viewport {
                            // This boundary is exiting within the viewport but is going to leave the viewport.
                            // Instead, we treat this as an exit of the previous entry by reverting the new name.
                            // Ideally we could undo the old transition but it's now too late. It's also on its
                            // on snapshot. We have know was for it to paint onto the original group.
                            // TODO: This will lead to things unexpectedly having exit animations that normally
                            // wouldn't happen. Consider if we should just let this fly off the screen instead.
                            let grandchild = current.child.borrow().clone();
                            restore_view_transition_on_host_instances(grandchild, false);
                        }
                    }
                }
            }
        }
        child = current.sibling.borrow().clone();
    }
}

pub fn commit_parent_enter_view_transitions(parent: &Fiber, gesture: bool) {
    let mut child = parent.child.borrow().clone();
    while let Some(current) = child {
        if current.tag.get() == OFFSCREEN_COMPONENT && current.memoized_state.get().is_some() {
            // Skip hidden subtrees.
        } else if current.tag.get() == VIEW_TRANSITION_COMPONENT {
            let props: &ViewTransitionProps = unsafe { js::cast(current.memoized_props.get()) };
            let has_parent_class = props.parent_enter.is_some();
            let has_parent_handler = if gesture {
                props.on_gesture_parent_enter.is_some()
            } else {
                props.on_parent_enter.is_some()
            };
            if has_parent_class || has_parent_handler {
                let mut relay = true;
                if has_parent_class {
                    let state: &ViewTransitionState = unsafe { js::cast(current.state_node.get()) };
                    let name = get_view_transition_name(props, state);
                    let class_name =
                        get_view_transition_class_name(props.default, props.parent_enter);
                    if class_name.as_deref() == Some("none") {
                        relay = false;
                    } else {
                        apply_view_transition_to_host_instances(
                            &current,
                            &name,
                            class_name.as_deref(),
                            None,
                            false,
                        );
                        if has_parent_handler {
                            if gesture {
                                schedule_gesture_transition_event(
                                    &current,
                                    props.on_gesture_parent_enter,
                                );
                            } else {
                                schedule_view_transition_event(&current, props.on_parent_enter);
                            }
                        }
                    }
                } else if gesture {
                    schedule_gesture_transition_event(&current, props.on_gesture_parent_enter);
                } else {
                    schedule_view_transition_event(&current, props.on_parent_enter);
                }
                if relay {
                    commit_parent_enter_view_transitions(&current, gesture);
                }
            }
        } else if (current.subtree_flags.get() & VIEW_TRANSITION_STATIC_PARENT) != NO_FLAGS {
            commit_parent_enter_view_transitions(&current, gesture);
        }
        child = current.sibling.borrow().clone();
    }
}

pub fn commit_parent_exit_view_transitions(parent: &Fiber, gesture: bool) {
    let mut child = parent.child.borrow().clone();
    while let Some(current) = child {
        if current.tag.get() == OFFSCREEN_COMPONENT && current.memoized_state.get().is_some() {
            // Skip hidden subtrees.
        } else if current.tag.get() == VIEW_TRANSITION_COMPONENT {
            let props: &ViewTransitionProps = unsafe { js::cast(current.memoized_props.get()) };
            let has_parent_class = props.parent_exit.is_some();
            let has_parent_handler = if gesture {
                props.on_gesture_parent_exit.is_some()
            } else {
                props.on_parent_exit.is_some()
            };
            if has_parent_class || has_parent_handler {
                let mut relay = true;
                if has_parent_class {
                    let state: &ViewTransitionState = unsafe { js::cast(current.state_node.get()) };
                    let name = get_view_transition_name(props, state);
                    let class_name =
                        get_view_transition_class_name(props.default, props.parent_exit);
                    if class_name.as_deref() == Some("none") {
                        relay = false;
                    } else {
                        apply_view_transition_to_host_instances(
                            &current,
                            &name,
                            class_name.as_deref(),
                            None,
                            false,
                        );
                        if has_parent_handler {
                            if gesture {
                                schedule_gesture_transition_event(
                                    &current,
                                    props.on_gesture_parent_exit,
                                );
                            } else {
                                schedule_view_transition_event(&current, props.on_parent_exit);
                            }
                        }
                    }
                } else if gesture {
                    schedule_gesture_transition_event(&current, props.on_gesture_parent_exit);
                } else {
                    schedule_view_transition_event(&current, props.on_parent_exit);
                }
                if relay {
                    commit_parent_exit_view_transitions(&current, gesture);
                }
            }
        } else if (current.subtree_flags.get() & VIEW_TRANSITION_STATIC_PARENT) != NO_FLAGS {
            commit_parent_exit_view_transitions(&current, gesture);
        }
        child = current.sibling.borrow().clone();
    }
}

fn restore_parent_enter_or_exit_view_transitions(parent: &Fiber) {
    let mut child = parent.child.borrow().clone();
    while let Some(current) = child {
        if current.tag.get() == OFFSCREEN_COMPONENT && current.memoized_state.get().is_some() {
            // Skip hidden subtrees.
        } else if current.tag.get() == VIEW_TRANSITION_COMPONENT {
            let props: &ViewTransitionProps = unsafe { js::cast(current.memoized_props.get()) };
            let has_parent_class = props.parent_enter.is_some() || props.parent_exit.is_some();
            let has_parent_handler = props.on_parent_enter.is_some()
                || props.on_parent_exit.is_some()
                || props.on_gesture_parent_enter.is_some()
                || props.on_gesture_parent_exit.is_some();
            if has_parent_class {
                let grandchild = current.child.borrow().clone();
                restore_view_transition_on_host_instances(grandchild, false);
            }
            if has_parent_class || has_parent_handler {
                restore_parent_enter_or_exit_view_transitions(&current);
            }
        } else if (current.subtree_flags.get() & VIEW_TRANSITION_STATIC_PARENT) != NO_FLAGS {
            restore_parent_enter_or_exit_view_transitions(&current);
        }
        child = current.sibling.borrow().clone();
    }
}

pub fn commit_enter_view_transitions(placement: &Fiber, gesture: bool) {
    if placement.tag.get() == VIEW_TRANSITION_COMPONENT {
        let state: &ViewTransitionState = unsafe { js::cast(placement.state_node.get()) };
        let props: &ViewTransitionProps = unsafe { js::cast(placement.memoized_props.get()) };
        let name = get_view_transition_name(props, state);
        let class_name = get_view_transition_class_name(
            props.default,
            if state.paired.borrow().is_some() {
                props.share
            } else {
                props.enter
            },
        );
        if class_name.as_deref() != Some("none") {
            let in_viewport = apply_view_transition_to_host_instances(
                placement,
                &name,
                class_name.as_deref(),
                None,
                false,
            );
            if !in_viewport {
                // TODO: If this was part of a pair we will still run the onShare callback.
                // Revert the transition names. This boundary is not in the viewport
                // so we won't bother animating it.
                let child = placement.child.borrow().clone();
                restore_view_transition_on_host_instances(child, false);
                // TODO: Should we still visit the children in case a named one was in the viewport?
            } else {
                commit_appearing_pair_view_transitions(placement);

                if state.paired.borrow().is_none() {
                    if gesture {
                        schedule_gesture_transition_event(placement, props.on_gesture_enter);
                    } else {
                        schedule_view_transition_event(placement, props.on_enter);
                    }
                    if ENABLE_VIEW_TRANSITION_PARENT_ENTER_EXIT.with(|&enabled| enabled) {
                        commit_parent_enter_view_transitions(placement, gesture);
                    }
                }
            }
        } else {
            commit_appearing_pair_view_transitions(placement);
        }
    } else if (placement.subtree_flags.get() & VIEW_TRANSITION_STATIC) != NO_FLAGS {
        let mut child = placement.child.borrow().clone();
        while let Some(current) = child {
            commit_enter_view_transitions(&current, gesture);
            child = current.sibling.borrow().clone();
        }
    } else {
        commit_appearing_pair_view_transitions(placement);
    }
}

fn commit_deleted_pair_view_transitions(deletion: &Fiber) {
    if APPEARING_VIEW_TRANSITIONS.with_borrow(|appearing_view_transitions| {
        appearing_view_transitions
            .as_ref()
            .is_none_or(|appearing_view_transitions| appearing_view_transitions.borrow().is_empty())
    }) {
        // We've found all.
        return;
    }
    if (deletion.subtree_flags.get() & VIEW_TRANSITION_NAMED_STATIC) == NO_FLAGS {
        // This has no named view transitions in its subtree.
        return;
    }
    let mut child = deletion.child.borrow().clone();
    while let Some(current) = child {
        if current.tag.get() == OFFSCREEN_COMPONENT && current.memoized_state.get().is_some() {
            // This tree was already hidden so we skip it.
        } else {
            if current.tag.get() == VIEW_TRANSITION_COMPONENT
                && (current.flags.get() & VIEW_TRANSITION_NAMED_STATIC) != NO_FLAGS
            {
                let props: &ViewTransitionProps = unsafe { js::cast(current.memoized_props.get()) };
                let name = props.name.as_deref();
                if let Some(name) = name
                    && name != "auto"
                {
                    let pair = APPEARING_VIEW_TRANSITIONS.with_borrow(|pairs| {
                        pairs
                            .as_ref()
                            .and_then(|pairs| pairs.borrow().get(name).cloned())
                    });
                    if let Some(pair) = pair {
                        let class_name = get_view_transition_class_name(props.default, props.share);
                        if class_name.as_deref() != Some("none") {
                            // We found a new appearing view transition with the same name as this deletion.
                            let in_viewport = apply_view_transition_to_host_instances(
                                &current,
                                name,
                                class_name.as_deref(),
                                None,
                                false,
                            );
                            if !in_viewport {
                                // This boundary is not in the viewport so we won't treat it as a matched pair.
                                // Revert the transition names. This avoids it flying onto the screen which can
                                // be disruptive and doesn't really preserve any continuity anyway.
                                let grandchild = current.child.borrow().clone();
                                restore_view_transition_on_host_instances(grandchild, false);
                            } else {
                                // We'll transition between them.
                                let old_instance: Rc<ViewTransitionState> =
                                    unsafe { js::cast(current.state_node.get()) };
                                let new_instance = pair;
                                new_instance.paired.replace(Some(Rc::clone(&old_instance)));
                                old_instance.paired.replace(Some(new_instance));
                                // Note: If the other side ends up outside the viewport, we'll still run this.
                                // Therefore it's possible for onShare to be called with only an old snapshot.
                                schedule_view_transition_event(&current, props.on_share);
                            }
                        }
                        // Delete the entry so that we know when we've found all of them
                        // and can stop searching (size reaches zero).
                        let found_all = APPEARING_VIEW_TRANSITIONS.with_borrow(|pairs| {
                            let pairs = pairs.as_ref().unwrap();
                            pairs.borrow_mut().remove(name);
                            pairs.borrow().is_empty()
                        });
                        if found_all {
                            break;
                        }
                    }
                }
            }
            commit_deleted_pair_view_transitions(&current);
        }
        child = current.sibling.borrow().clone();
    }
}

pub fn commit_exit_view_transitions(deletion: &Fiber) {
    if deletion.tag.get() == VIEW_TRANSITION_COMPONENT {
        let props: &ViewTransitionProps = unsafe { js::cast(deletion.memoized_props.get()) };
        let name = get_view_transition_name(props, unsafe { js::cast(deletion.state_node.get()) });
        let pair = APPEARING_VIEW_TRANSITIONS.with_borrow(|appearing_view_transitions| {
            appearing_view_transitions
                .as_ref()
                .and_then(|appearing_view_transitions| {
                    appearing_view_transitions.borrow().get(&name).cloned()
                })
        });
        let class_name = get_view_transition_class_name(
            props.default,
            if pair.is_some() {
                props.share
            } else {
                props.exit
            },
        );
        if class_name.as_deref() != Some("none") {
            let in_viewport = apply_view_transition_to_host_instances(
                deletion,
                &name,
                class_name.as_deref(),
                None,
                false,
            );
            if !in_viewport {
                // Revert the transition names. This boundary is not in the viewport
                // so we won't bother animating it.
                let child = deletion.child.borrow().clone();
                restore_view_transition_on_host_instances(child, false);
                // TODO: Should we still visit the children in case a named one was in the viewport?
            } else if let Some(pair) = pair {
                // We found a new appearing view transition with the same name as this deletion.
                // We'll transition between them instead of running the normal exit.
                let old_instance: Rc<ViewTransitionState> =
                    unsafe { js::cast(deletion.state_node.get()) };
                let new_instance = pair;
                new_instance.paired.replace(Some(Rc::clone(&old_instance)));
                old_instance.paired.replace(Some(new_instance));
                // Delete the entry so that we know when we've found all of them
                // and can stop searching (size reaches zero).
                APPEARING_VIEW_TRANSITIONS.with_borrow(|appearing_view_transitions| {
                    // Refined by the pair.
                    appearing_view_transitions
                        .as_ref()
                        .unwrap()
                        .borrow_mut()
                        .remove(&name);
                });
                // Note: If the other side ends up outside the viewport, we'll still run this.
                // Therefore it's possible for onShare to be called with only an old snapshot.
                schedule_view_transition_event(deletion, props.on_share);
            } else {
                schedule_view_transition_event(deletion, props.on_exit);
                if ENABLE_VIEW_TRANSITION_PARENT_ENTER_EXIT.with(|&enabled| enabled) {
                    commit_parent_exit_view_transitions(deletion, false);
                }
            }
        }
        if APPEARING_VIEW_TRANSITIONS.with_borrow(Option::is_some) {
            // Look for more pairs deeper in the tree.
            commit_deleted_pair_view_transitions(deletion);
        }
    } else if (deletion.subtree_flags.get() & VIEW_TRANSITION_STATIC) != NO_FLAGS {
        let mut child = deletion.child.borrow().clone();
        while let Some(current) = child {
            commit_exit_view_transitions(&current);
            child = current.sibling.borrow().clone();
        }
    } else if APPEARING_VIEW_TRANSITIONS.with_borrow(Option::is_some) {
        commit_deleted_pair_view_transitions(deletion);
    }
}

pub fn commit_before_update_view_transition(current: &Fiber, finished_work: &Fiber) {
    // The way we deal with multiple HostInstances as children of a View Transition in an
    // update can get tricky. The important bit is that if you swap out n HostInstances
    // from n HostInstances then they match up in order. Similarly, if you don't swap
    // any HostInstances each instance just transitions as is.
    //
    // We call this function twice. First we apply the view transition names on the
    // "current" tree in the snapshot phase. Then in the mutation phase we apply view
    // transition names to the "finishedWork" tree.
    //
    // This means that if there were insertions or deletions before an updated Instance
    // that same Instance might get different names in the "old" and the "new" state.
    // For example if you swap two HostInstances inside a ViewTransition they don't
    // animate to swap position but rather cross-fade into the other instance. This might
    // be unexpected but it is in line with the semantics that the ViewTransition is its
    // own layer that cross-fades its content when it updates. If you want to reorder then
    // each child needs its own ViewTransition.
    let old_props: &ViewTransitionProps = unsafe { js::cast(current.memoized_props.get()) };
    let old_name =
        get_view_transition_name(old_props, unsafe { js::cast(current.state_node.get()) });
    let new_props: &ViewTransitionProps = unsafe { js::cast(finished_work.memoized_props.get()) };
    // This className applies only if there are fewer child DOM nodes than
    // before or if this update should've been cancelled but we ended up with
    // a parent animating so we need to animate the child too.
    // For example, if update="foo" layout="none" and it turns out this was
    // a layout only change, then the "foo" class will be applied even though
    // it was not actually an update. Which is a bug.
    let class_name = get_view_transition_class_name(new_props.default, new_props.update);
    if class_name.as_deref() == Some("none") {
        // If update is "none" then we don't have to apply a name. Since we won't animate this boundary.
        return;
    }
    let measurements = Rc::new(RefCell::new(Vec::new()));
    current
        .memoized_state
        .set(Some(js::unknown_of(Rc::clone(&measurements))));
    apply_view_transition_to_host_instances(
        current,
        &old_name,
        class_name.as_deref(),
        Some(&measurements),
        true,
    );
}

pub fn commit_nested_view_transitions(changed_parent: &Fiber) {
    let mut child = changed_parent.child.borrow().clone();
    while let Some(current) = child {
        if current.tag.get() == VIEW_TRANSITION_COMPONENT {
            // In this case the outer ViewTransition component wins but if there
            // was an update through this component then the inner one wins.
            let props: &ViewTransitionProps = unsafe { js::cast(current.memoized_props.get()) };
            let name =
                get_view_transition_name(props, unsafe { js::cast(current.state_node.get()) });
            let class_name = get_view_transition_class_name(props.default, props.update);
            // "Nested" view transitions are in subtrees that didn't update so
            // this is a "current". We normally clear this upon rerendering
            // but we use this flag to track changes from layout in the commit.
            // So we need it to be cleared before we do that.
            // TODO: Use some other temporary state to track this.
            current.flags.set(current.flags.get() & !UPDATE);
            if class_name.as_deref() != Some("none") {
                let measurements = Rc::new(RefCell::new(Vec::new()));
                current
                    .memoized_state
                    .set(Some(js::unknown_of(Rc::clone(&measurements))));
                apply_view_transition_to_host_instances(
                    &current,
                    &name,
                    class_name.as_deref(),
                    Some(&measurements),
                    false,
                );
            }
        } else if (current.subtree_flags.get() & VIEW_TRANSITION_STATIC) != NO_FLAGS {
            commit_nested_view_transitions(&current);
        }
        child = current.sibling.borrow().clone();
    }
}

fn restore_paired_view_transitions(parent: &Fiber) {
    if (parent.subtree_flags.get() & VIEW_TRANSITION_NAMED_STATIC) == NO_FLAGS {
        // This has no named view transitions in its subtree.
        return;
    }
    let mut child = parent.child.borrow().clone();
    while let Some(current) = child {
        if current.tag.get() == OFFSCREEN_COMPONENT && current.memoized_state.get().is_some() {
            // This tree was already hidden so we skip it.
        } else {
            if current.tag.get() == VIEW_TRANSITION_COMPONENT
                && (current.flags.get() & VIEW_TRANSITION_NAMED_STATIC) != NO_FLAGS
            {
                let instance: &ViewTransitionState = unsafe { js::cast(current.state_node.get()) };
                if instance.paired.borrow().is_some() {
                    instance.paired.replace(js::null());
                    let grandchild = current.child.borrow().clone();
                    restore_view_transition_on_host_instances(grandchild, false);
                }
            }
            restore_paired_view_transitions(&current);
        }
        child = current.sibling.borrow().clone();
    }
}

pub fn restore_enter_or_exit_view_transitions(fiber: &Fiber) {
    if fiber.tag.get() == VIEW_TRANSITION_COMPONENT {
        let instance: &ViewTransitionState = unsafe { js::cast(fiber.state_node.get()) };
        instance.paired.replace(js::null());
        let child = fiber.child.borrow().clone();
        restore_view_transition_on_host_instances(child, false);
        if ENABLE_VIEW_TRANSITION_PARENT_ENTER_EXIT.with(|&enabled| enabled) {
            restore_parent_enter_or_exit_view_transitions(fiber);
        }
        restore_paired_view_transitions(fiber);
    } else if (fiber.subtree_flags.get() & VIEW_TRANSITION_STATIC) != NO_FLAGS {
        let mut child = fiber.child.borrow().clone();
        while let Some(current) = child {
            restore_enter_or_exit_view_transitions(&current);
            child = current.sibling.borrow().clone();
        }
    } else {
        restore_paired_view_transitions(fiber);
    }
}

pub fn restore_update_view_transition(current: &Fiber, finished_work: &Fiber) {
    let current_child = current.child.borrow().clone();
    restore_view_transition_on_host_instances(current_child, true);
    let finished_child = finished_work.child.borrow().clone();
    restore_view_transition_on_host_instances(finished_child, true);
}

pub fn restore_update_view_transition_for_gesture(current: &Fiber, _finished_work: &Fiber) {
    // For gestures we don't need to reset "finishedWork" because those would
    // have all been clones that got deleted.
    let child = current.child.borrow().clone();
    restore_view_transition_on_host_instances(child, true);
}

pub fn restore_nested_view_transitions(changed_parent: &Fiber) {
    let mut child = changed_parent.child.borrow().clone();
    while let Some(current) = child {
        if current.tag.get() == VIEW_TRANSITION_COMPONENT {
            let grandchild = current.child.borrow().clone();
            restore_view_transition_on_host_instances(grandchild, false);
        } else if (current.subtree_flags.get() & VIEW_TRANSITION_STATIC) != NO_FLAGS {
            restore_nested_view_transitions(&current);
        }
        child = current.sibling.borrow().clone();
    }
}

#[cfg_attr(rust_js, rust_js::nullable(previous_measurements))]
pub fn measure_view_transition_host_instances(
    parent_view_transition: &Fiber,
    child: Option<Rc<Fiber>>,
    new_name: &str,
    old_name: &str,
    class_name: Option<&str>,
    previous_measurements: Option<&[&'static Unknown]>,
    stop_at_nested_view_transitions: bool,
) -> bool {
    VIEW_TRANSITION_HOST_INSTANCE_IDX.set(0);
    measure_view_transition_host_instances_recursive(
        parent_view_transition,
        child,
        new_name,
        old_name,
        class_name,
        previous_measurements,
        stop_at_nested_view_transitions,
    )
}

#[cfg_attr(rust_js, rust_js::nullable(previous_measurements))]
fn measure_view_transition_host_instances_recursive(
    parent_view_transition: &Fiber,
    mut child: Option<Rc<Fiber>>,
    new_name: &str,
    old_name: &str,
    class_name: Option<&str>,
    previous_measurements: Option<&[&'static Unknown]>,
    stop_at_nested_view_transitions: bool,
) -> bool {
    if !SUPPORTS_MUTATION {
        if ENABLE_VIEW_TRANSITION_FOR_PERSISTENCE_MODE {
            while let Some(current) = child {
                if current.tag.get() == HOST_COMPONENT {
                    let instance = current.state_node.get();
                    if previous_measurements.is_none_or(|previous_measurements| {
                        VIEW_TRANSITION_HOST_INSTANCE_IDX.get() >= previous_measurements.len()
                    }) {
                        // If there was an insertion of extra nodes, we have to assume they affected the parent.
                        // It should have already been marked as an Update due to the mutation.
                        parent_view_transition
                            .flags
                            .set(parent_view_transition.flags.get() | AFFECTED_PARENT_LAYOUT);
                    }
                    // TODO: check if instance is out of viewport
                    if (parent_view_transition.flags.get() & UPDATE) != NO_FLAGS {
                        apply_view_transition_name(
                            instance,
                            &if VIEW_TRANSITION_HOST_INSTANCE_IDX.get() == 0 {
                                new_name.to_string()
                            } else {
                                format!("{new_name}_{}", VIEW_TRANSITION_HOST_INSTANCE_IDX.get())
                            },
                            class_name,
                        );
                    }
                    // TODO: cancel transition by pushing into viewTransitionCancelableChildren
                    VIEW_TRANSITION_HOST_INSTANCE_IDX
                        .set(VIEW_TRANSITION_HOST_INSTANCE_IDX.get() + 1);
                } else if current.tag.get() == OFFSCREEN_COMPONENT
                    && current.memoized_state.get().is_some()
                {
                    // Skip any hidden subtrees. They were or are effectively not there.
                } else if current.tag.get() == VIEW_TRANSITION_COMPONENT
                    && stop_at_nested_view_transitions
                {
                    // Skip any nested view transitions for updates since in that case the
                    // inner most one is the one that handles the update.
                    // If this inner boundary resized we need to bubble that information up.
                    parent_view_transition.flags.set(
                        parent_view_transition.flags.get()
                            | (current.flags.get() & AFFECTED_PARENT_LAYOUT),
                    );
                } else {
                    let grandchild = current.child.borrow().clone();
                    measure_view_transition_host_instances_recursive(
                        parent_view_transition,
                        grandchild,
                        new_name,
                        old_name,
                        class_name,
                        previous_measurements,
                        stop_at_nested_view_transitions,
                    );
                }
                child = current.sibling.borrow().clone();
            }
            return true;
        } else {
            return false;
        }
    }
    let mut in_viewport = false;
    while let Some(current) = child {
        if current.tag.get() == HOST_COMPONENT {
            let instance = current.state_node.get();
            if let Some(previous_measurements) = previous_measurements
                && VIEW_TRANSITION_HOST_INSTANCE_IDX.get() < previous_measurements.len()
            {
                // The previous measurement of the Instance in this location within the ViewTransition.
                // Note that this might not be the same exact Instance if the Instances within the
                // ViewTransition changed.
                let previous_measurement =
                    previous_measurements[VIEW_TRANSITION_HOST_INSTANCE_IDX.get()];
                let next_measurement = measure_instance(instance);
                if was_instance_in_viewport(previous_measurement)
                    || was_instance_in_viewport(next_measurement)
                {
                    // If either the old or new state was within the viewport we have to animate this.
                    // But if it turns out that none of them were we'll be able to skip it.
                    in_viewport = true;
                }
                if (parent_view_transition.flags.get() & UPDATE) == NO_FLAGS
                    && has_instance_changed(previous_measurement, next_measurement)
                {
                    parent_view_transition
                        .flags
                        .set(parent_view_transition.flags.get() | UPDATE);
                }
                if has_instance_affected_parent(previous_measurement, next_measurement) {
                    // If this instance size within its parent has changed it might have caused the
                    // parent to relayout which needs a cross fade.
                    parent_view_transition
                        .flags
                        .set(parent_view_transition.flags.get() | AFFECTED_PARENT_LAYOUT);
                }
            } else {
                // If there was an insertion of extra nodes, we have to assume they affected the parent.
                // It should have already been marked as an Update due to the mutation.
                parent_view_transition
                    .flags
                    .set(parent_view_transition.flags.get() | AFFECTED_PARENT_LAYOUT);
            }
            if (parent_view_transition.flags.get() & UPDATE) != NO_FLAGS {
                // We might update this node so we need to apply its new name for the new state.
                // Additionally in the ApplyGesture case we also need to do this because the clone
                // will have the name but this one won't.
                apply_view_transition_name(
                    instance,
                    &if VIEW_TRANSITION_HOST_INSTANCE_IDX.get() == 0 {
                        new_name.to_string()
                    } else {
                        // If we have multiple Host Instances below, we add a suffix to the name to give
                        // each one a unique name.
                        format!("{new_name}_{}", VIEW_TRANSITION_HOST_INSTANCE_IDX.get())
                    },
                    class_name,
                );
            }
            if !in_viewport || (parent_view_transition.flags.get() & UPDATE) == NO_FLAGS {
                // It turns out that we had no other deeper mutations, the child transitions didn't
                // affect the parent layout and this instance hasn't changed size. So we can skip
                // animating it. However, in the current model this only works if the parent also
                // doesn't animate. So we have to queue these and wait until we complete the parent
                // to cancel them.
                VIEW_TRANSITION_CANCELABLE_CHILDREN.with_borrow_mut(
                    |view_transition_cancelable_children| {
                        view_transition_cancelable_children
                            .get_or_insert_with(|| Rc::new(RefCell::new(Vec::new())))
                            .borrow_mut()
                            .extend([
                                instance,
                                Some(js::unknown_of(
                                    if VIEW_TRANSITION_HOST_INSTANCE_IDX.get() == 0 {
                                        old_name.to_string()
                                    } else {
                                        // If we have multiple Host Instances below, we add a suffix to the name to give
                                        // each one a unique name.
                                        format!(
                                            "{old_name}_{}",
                                            VIEW_TRANSITION_HOST_INSTANCE_IDX.get()
                                        )
                                    },
                                )),
                                current.memoized_props.get(),
                            ]);
                    },
                );
            }
            VIEW_TRANSITION_HOST_INSTANCE_IDX.set(VIEW_TRANSITION_HOST_INSTANCE_IDX.get() + 1);
        } else if current.tag.get() == OFFSCREEN_COMPONENT && current.memoized_state.get().is_some()
        {
            // Skip any hidden subtrees. They were or are effectively not there.
        } else if current.tag.get() == VIEW_TRANSITION_COMPONENT && stop_at_nested_view_transitions
        {
            // Skip any nested view transitions for updates since in that case the
            // inner most one is the one that handles the update.
            // If this inner boundary resized we need to bubble that information up.
            parent_view_transition.flags.set(
                parent_view_transition.flags.get() | (current.flags.get() & AFFECTED_PARENT_LAYOUT),
            );
        } else {
            let grandchild = current.child.borrow().clone();
            if measure_view_transition_host_instances_recursive(
                parent_view_transition,
                grandchild,
                new_name,
                old_name,
                class_name,
                previous_measurements,
                stop_at_nested_view_transitions,
            ) {
                in_viewport = true;
            }
        }
        child = current.sibling.borrow().clone();
    }
    in_viewport
}

pub fn measure_update_view_transition(
    current: &Rc<Fiber>,
    finished_work: &Rc<Fiber>,
    gesture: bool,
) -> bool {
    // If this was a gesture then which Fiber was used for the "old" vs "new" state is reversed.
    // We still need to treat "finishedWork" as the Fiber that contains the flags for this commmit.
    let old_fiber = if gesture { finished_work } else { current };
    let new_fiber = if gesture { current } else { finished_work };
    let props: &ViewTransitionProps = unsafe { js::cast(new_fiber.memoized_props.get()) };
    let state: &ViewTransitionState = unsafe { js::cast(new_fiber.state_node.get()) };
    let new_name = get_view_transition_name(props, state);
    let old_name =
        get_view_transition_name(unsafe { js::cast(old_fiber.memoized_props.get()) }, state);
    // Whether it ends up having been updated or relayout we apply the update class name.
    let class_name = get_view_transition_class_name(props.default, props.update);
    if class_name.as_deref() == Some("none") {
        // If update is "none" then we don't have to apply a name. Since we won't animate this boundary.
        return false;
    }
    // If nothing changed due to a mutation, or children changing size
    // and the measurements end up unchanged, we should restore it to not animate.
    let previous_measurements: Option<Vec<&'static Unknown>> = if gesture {
        let clones = state.clones.borrow();
        clones.as_ref().map(|clones| {
            clones
                .borrow()
                .iter()
                .copied()
                .map(measure_cloned_instance)
                .collect()
        })
    } else {
        let previous_measurements = unsafe { js::cast(old_fiber.memoized_state.get()) };
        old_fiber.memoized_state.set(js::null()); // Clear it. We won't need it anymore.
        previous_measurements
    };
    let child = new_fiber.child.borrow().clone();
    let in_viewport = measure_view_transition_host_instances(
        finished_work, // This is always finishedWork since it's used to assign flags.
        child,         // This either current or finishedWork depending on if was a gesture.
        &new_name,
        &old_name,
        class_name.as_deref(),
        previous_measurements.as_deref(),
        true,
    );
    let previous_count =
        previous_measurements.map_or(0, |previous_measurements| previous_measurements.len());
    if VIEW_TRANSITION_HOST_INSTANCE_IDX.get() != previous_count {
        // If we found a different number of child DOM nodes we need to assume that
        // the parent layout may have changed as a result. This is not necessarily
        // true if those nodes were absolutely positioned.
        finished_work
            .flags
            .set(finished_work.flags.get() | AFFECTED_PARENT_LAYOUT);
    }
    in_viewport
}

pub fn measure_nested_view_transitions(changed_parent: &Fiber, gesture: bool) {
    let mut child = changed_parent.child.borrow().clone();
    while let Some(current) = child {
        if current.tag.get() == VIEW_TRANSITION_COMPONENT {
            let props: &ViewTransitionProps = unsafe { js::cast(current.memoized_props.get()) };
            let state: &ViewTransitionState = unsafe { js::cast(current.state_node.get()) };
            let name = get_view_transition_name(props, state);
            let class_name = get_view_transition_class_name(props.default, props.update);
            let previous_measurements: Option<Vec<&'static Unknown>> = if gesture {
                let clones = state.clones.borrow();
                clones.as_ref().map(|clones| {
                    clones
                        .borrow()
                        .iter()
                        .copied()
                        .map(measure_cloned_instance)
                        .collect()
                })
            } else {
                let previous_measurements = unsafe { js::cast(current.memoized_state.get()) };
                current.memoized_state.set(js::null()); // Clear it. We won't need it anymore.
                previous_measurements
            };
            let grandchild = current.child.borrow().clone();
            let in_viewport = measure_view_transition_host_instances(
                &current,
                grandchild,
                &name,
                &name, // Since this is unchanged, new and old name is the same.
                class_name.as_deref(),
                previous_measurements.as_deref(),
                false,
            );
            if (current.flags.get() & UPDATE) == NO_FLAGS || !in_viewport {
                // Nothing changed.
            } else if gesture {
                schedule_gesture_transition_event(&current, props.on_gesture_update);
            } else {
                schedule_view_transition_event(&current, props.on_update);
            }
        } else if (current.subtree_flags.get() & VIEW_TRANSITION_STATIC) != NO_FLAGS {
            measure_nested_view_transitions(&current, gesture);
        }
        child = current.sibling.borrow().clone();
    }
}
