// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js::Unknown;
use shared::environment::__DEV__;
use shared::react_feature_flags::{
    ENABLE_COMPONENT_PERFORMANCE_TRACK, ENABLE_PROFILER_TIMER,
    ENABLE_VIEW_TRANSITION_PARENT_ENTER_EXIT,
};
use webapi::console;

use crate::react_fiber::ViewTransitionState;
use crate::react_fiber_commit_effects::{
    commit_hook_effect_list_mount, commit_hook_effect_list_unmount,
};
use crate::react_fiber_commit_view_transitions::{
    APPEARING_VIEW_TRANSITIONS, VIEW_TRANSITION_CANCELABLE_CHILDREN, commit_enter_view_transitions,
    commit_parent_exit_view_transitions, measure_nested_view_transitions,
    measure_update_view_transition, pop_view_transition_cancelable_scope,
    push_view_transition_cancelable_scope, restore_enter_or_exit_view_transitions,
    restore_nested_view_transitions, restore_update_view_transition_for_gesture,
};
use crate::react_fiber_flags::{
    AFFECTED_PARENT_LAYOUT, CONTENT_RESET, MUTATION_MASK, NO_FLAGS, PLACEMENT, UPDATE,
    VIEW_TRANSITION_NAMED_STATIC, VIEW_TRANSITION_STATIC, VISIBILITY,
};
use crate::react_fiber_mutation_tracking::{
    VIEW_TRANSITION_MUTATION_CONTEXT, pop_mutation_context, push_mutation_context,
    track_host_mutation,
};
use crate::react_fiber_view_transition_component::{
    ViewTransitionProps, get_view_transition_class_name, get_view_transition_name,
};
use crate::react_fiber_work_loop::schedule_gesture_transition_event;
#[cfg_attr(rust_js, rust_js::name = "HookHasEffect")]
use crate::react_hook_effect_tags::HAS_EFFECT as HOOK_HAS_EFFECT;
#[cfg_attr(rust_js, rust_js::name = "HookInsertion")]
use crate::react_hook_effect_tags::INSERTION as HOOK_INSERTION;
use crate::react_internal_types::{Fiber, FiberRoot};
use crate::react_profiler_timer::track_animating_task;
use crate::react_work_tags::{
    FORWARD_REF, FUNCTION_COMPONENT, HOST_COMPONENT, HOST_HOISTABLE, HOST_PORTAL, HOST_SINGLETON,
    HOST_TEXT, MEMO_COMPONENT, OFFSCREEN_COMPONENT, SIMPLE_MEMO_COMPONENT,
    VIEW_TRANSITION_COMPONENT,
};

unsafe extern "Rust" {
    #[link_name = "./src/ReactFiberConfig.js#supportsResources"]
    safe static SUPPORTS_RESOURCES: bool;
    #[link_name = "./src/ReactFiberConfig.js#supportsSingletons"]
    safe static SUPPORTS_SINGLETONS: bool;
}

// ReactFiberConfig's host operations, on its `Instance`, `TextInstance`,
// `Container` and `Props`.
#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#cloneMutableInstance"
)]
fn clone_mutable_instance(instance: &'static Unknown, keep_children: bool) -> &'static Unknown {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#cloneMutableTextInstance"
)]
fn clone_mutable_text_instance(text_instance: &'static Unknown) -> &'static Unknown {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#cloneRootViewTransitionContainer"
)]
fn clone_root_view_transition_container(root_container: &'static Unknown) -> &'static Unknown {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#removeRootViewTransitionClone"
)]
fn remove_root_view_transition_clone(root_container: &'static Unknown, clone: &'static Unknown) {
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
    rust_js::link_name = "./src/ReactFiberConfig.js#applyViewTransitionName"
)]
fn apply_view_transition_name(instance: &'static Unknown, name: &str, class_name: Option<&str>) {
    unreachable!()
}

#[cfg_attr(rust_js, rust_js::link_name = "./src/ReactFiberConfig.js#appendChild")]
fn append_child(parent_instance: &'static Unknown, child: &'static Unknown) {
    unreachable!()
}

#[cfg_attr(rust_js, rust_js::link_name = "./src/ReactFiberConfig.js#commitUpdate")]
fn commit_update(
    instance: Option<&'static Unknown>,
    r#type: Option<&'static Unknown>,
    old_props: Option<&'static Unknown>,
    new_props: Option<&'static Unknown>,
    internal_instance_handle: &Fiber,
) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#commitTextUpdate"
)]
fn commit_text_update(text_instance: &'static Unknown, old_text: &str, new_text: &str) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#resetTextContent"
)]
fn reset_text_content(instance: &'static Unknown) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#unhideInstance"
)]
fn unhide_instance(instance: &'static Unknown, props: Option<&'static Unknown>) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#unhideTextInstance"
)]
fn unhide_text_instance(text_instance: &'static Unknown, text: Option<&'static Unknown>) {
    unreachable!()
}

thread_local! {
    // Used during the apply phase to track whether a parent ViewTransition component
    // might have been affected by any mutations / relayouts below.
    static VIEW_TRANSITION_CONTEXT_CHANGED: Cell<bool> = const { Cell::new(false) };
}

fn detect_mutation_or_insert_clones(_finished_work: &Fiber) -> bool {
    true
}

type VisitPhase = u8;
const CLONE_UPDATE: VisitPhase = 0; // Mutations in this subtree or potentially affected by layout.
const CLONE_EXIT: VisitPhase = 1; // Inside a reappearing offscreen before the next ViewTransition or HostComponent.
const CLONE_UNHIDE: VisitPhase = 2; // Inside a reappearing offscreen before the next HostComponent.
const CLONE_APPEARING_PAIR: VisitPhase = 3; // Like UNHIDE but we're already inside the first Host Component only finding pairs.
const CLONE_UNCHANGED: VisitPhase = 4; // Nothing in this tree was changed but we're still walking to clone it.
const INSERT_EXIT: VisitPhase = 5; // Inside a newly mounted tree before the next ViewTransition or HostComponent.
const INSERT_APPEND: VisitPhase = 6; // Inside a newly mounted tree before the next HostComponent.
const INSERT_APPEARING_PAIR: VisitPhase = 7; // Inside a newly mounted tree only finding pairs.

fn apply_view_transition_to_clones(
    name: &str,
    class_name: Option<&str>,
    clones: &RefCell<Vec<&'static Unknown>>,
    fiber: &Fiber,
) {
    // This gets called when we have found a pair, but after the clone in created. The clone is
    // created by the insertion side. If the insertion side if found before the deletion side
    // then this is called by the deletion. If the deletion is visited first then this is called
    // later by the insertion when the clone has been created.
    let mut i = 0;
    while i < clones.borrow().len() {
        let clone = clones.borrow()[i];
        apply_view_transition_name(
            clone,
            &if i == 0 {
                name.to_string()
            } else {
                // If we have multiple Host Instances below, we add a suffix to the name to give
                // each one a unique name.
                format!("{name}_{i}")
            },
            class_name,
        );
        i += 1;
    }
    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        && ENABLE_COMPONENT_PERFORMANCE_TRACK
        && let Some(debug_task) = fiber.debug_task.get()
    {
        track_animating_task(debug_task);
    }
}

fn track_deleted_pair_view_transitions(deletion: &Fiber) {
    if APPEARING_VIEW_TRANSITIONS.with_borrow(|appearing_view_transitions| {
        appearing_view_transitions
            .as_ref()
            .is_none_or(|appearing_view_transitions| appearing_view_transitions.borrow().is_empty())
    }) {
        // We've found all.
        return;
    }
    let pairs = APPEARING_VIEW_TRANSITIONS
        .with_borrow(Clone::clone)
        .unwrap();
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
                    let pair = pairs.borrow().get(name).cloned();
                    if let Some(pair) = pair {
                        // Delete the entry so that we know when we've found all of them
                        // and can stop searching (size reaches zero).
                        pairs.borrow_mut().remove(name);
                        let class_name = get_view_transition_class_name(props.default, props.share);
                        if class_name.as_deref() != Some("none") {
                            // TODO: Since the deleted instance already has layout we could
                            // check if it's in the viewport and if not skip the pairing.
                            // It would currently cause layout thrash though so if we did that
                            // we need to avoid inserting the root of the cloned trees until
                            // the end.

                            // The "old" instance is actually the one we're inserting.
                            let old_instance = pair;
                            // The "new" instance is the already mounted one we're deleting.
                            let new_instance: Rc<ViewTransitionState> =
                                unsafe { js::cast(current.state_node.get()) };
                            old_instance.paired.replace(Some(Rc::clone(&new_instance)));
                            new_instance.paired.replace(Some(Rc::clone(&old_instance)));
                            let clones = old_instance.clones.borrow().clone();
                            if let Some(clones) = clones {
                                // If we have clones that means that we've already visited this
                                // ViewTransition boundary before and we can now apply the name
                                // to those clones. Otherwise, we have to wait until we clone it.
                                apply_view_transition_to_clones(
                                    name,
                                    class_name.as_deref(),
                                    &clones,
                                    &current,
                                );
                            }
                        }
                        if pairs.borrow().is_empty() {
                            break;
                        }
                    }
                }
            }
            track_deleted_pair_view_transitions(&current);
        }
        child = current.sibling.borrow().clone();
    }
}

fn track_enter_view_transitions(deletion: &Fiber) {
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
                props.enter
            },
        );
        if class_name.as_deref() != Some("none")
            && let Some(pair) = pair
        {
            // TODO: Since the deleted instance already has layout we could
            // check if it's in the viewport and if not skip the pairing.
            // It would currently cause layout thrash though so if we did that
            // we need to avoid inserting the root of the cloned trees until
            // the end.

            // Delete the entry so that we know when we've found all of them
            // and can stop searching (size reaches zero).
            // Refined by the pair.
            APPEARING_VIEW_TRANSITIONS.with_borrow(|appearing_view_transitions| {
                appearing_view_transitions
                    .as_ref()
                    .unwrap()
                    .borrow_mut()
                    .remove(&name);
            });
            // The "old" instance is actually the one we're inserting.
            let old_instance = pair;
            // The "new" instance is the already mounted one we're deleting.
            let new_instance: Rc<ViewTransitionState> =
                unsafe { js::cast(deletion.state_node.get()) };
            old_instance.paired.replace(Some(Rc::clone(&new_instance)));
            new_instance.paired.replace(Some(Rc::clone(&old_instance)));
            let clones = old_instance.clones.borrow().clone();
            if let Some(clones) = clones {
                // If we have clones that means that we've already visited this
                // ViewTransition boundary before and we can now apply the name
                // to those clones. Otherwise, we have to wait until we clone it.
                apply_view_transition_to_clones(&name, class_name.as_deref(), &clones, deletion);
            }
        }
        // Look for more pairs deeper in the tree.
        track_deleted_pair_view_transitions(deletion);
    } else if (deletion.subtree_flags.get() & VIEW_TRANSITION_STATIC) != NO_FLAGS {
        let mut child = deletion.child.borrow().clone();
        while let Some(current) = child {
            track_enter_view_transitions(&current);
            child = current.sibling.borrow().clone();
        }
    } else {
        track_deleted_pair_view_transitions(deletion);
    }
}

fn apply_appearing_pair_view_transition(child: &Fiber) {
    // Normally these helpers do recursive calls but since insertion/offscreen is forked
    // we call this helper from those loops instead. This must be called only on
    // ViewTransitionComponent that has already had their clones filled.
    if (child.flags.get() & VIEW_TRANSITION_NAMED_STATIC) != NO_FLAGS {
        let state: &ViewTransitionState = unsafe { js::cast(child.state_node.get()) };
        // If this is not yet paired, it doesn't mean that we won't pair it later when
        // we find the deletion side. If that's the case then we'll add the names to
        // the clones then.
        if state.paired.borrow().is_some() {
            let props: &ViewTransitionProps = unsafe { js::cast(child.memoized_props.get()) };
            let Some(name) = props.name.as_deref().filter(|&name| name != "auto") else {
                panic!("Found a pair with an auto name. This is a bug in React.");
            };
            // Note that this class name that doesn't actually really matter because the
            // "new" side will be the one that wins in practice.
            let class_name = get_view_transition_class_name(props.default, props.share);
            if class_name.as_deref() != Some("none") {
                let clones = state.clones.borrow().clone();
                // If there are no clones at this point, that should mean that there are no
                // HostComponent children in this ViewTransition.
                if let Some(clones) = clones {
                    apply_view_transition_to_clones(name, class_name.as_deref(), &clones, child);
                }
                schedule_gesture_transition_event(child, props.on_gesture_share);
            }
        }
    }
}

fn apply_exit_view_transition(placement: &Fiber) {
    // Normally these helpers do recursive calls but since insertion/offscreen is forked
    // we call this helper from those loops instead. This must be called only on
    // ViewTransitionComponent that has already had their clones filled.
    let state: &ViewTransitionState = unsafe { js::cast(placement.state_node.get()) };
    let props: &ViewTransitionProps = unsafe { js::cast(placement.memoized_props.get()) };
    let name = get_view_transition_name(props, state);
    let class_name = get_view_transition_class_name(
        props.default,
        // Note that just because we don't have a pair yet doesn't mean we won't find one
        // later. However, that doesn't matter because if we do the class name that wins
        // is the one applied by the "new" side anyway.
        if state.paired.borrow().is_some() {
            props.share
        } else {
            props.exit
        },
    );
    if class_name.as_deref() != Some("none") {
        // TODO: Ideally we could determine if this exit is in the viewport and
        // exclude it otherwise but that would require waiting until we insert
        // and layout the clones first. Currently wait until the view transition
        // starts before reading the layout.
        let clones = state.clones.borrow().clone();
        // If there are no clones at this point, that should mean that there are no
        // HostComponent children in this ViewTransition.
        if let Some(clones) = clones {
            apply_view_transition_to_clones(&name, class_name.as_deref(), &clones, placement);
        }
        if state.paired.borrow().is_some() {
            schedule_gesture_transition_event(placement, props.on_gesture_share);
        } else {
            schedule_gesture_transition_event(placement, props.on_gesture_exit);
            if ENABLE_VIEW_TRANSITION_PARENT_ENTER_EXIT.with(|&enabled| enabled) {
                commit_parent_exit_view_transitions(placement, true);
            }
        }
    }
}

fn apply_nested_view_transition(child: &Fiber) {
    let state: &ViewTransitionState = unsafe { js::cast(child.state_node.get()) };
    let props: &ViewTransitionProps = unsafe { js::cast(child.memoized_props.get()) };
    let name = get_view_transition_name(props, state);
    let class_name = get_view_transition_class_name(props.default, props.update);
    if class_name.as_deref() != Some("none") {
        let clones = state.clones.borrow().clone();
        // If there are no clones at this point, that should mean that there are no
        // HostComponent children in this ViewTransition.
        if let Some(clones) = clones {
            apply_view_transition_to_clones(&name, class_name.as_deref(), &clones, child);
        }
    }
}

fn apply_update_view_transition(current: &Fiber, finished_work: &Fiber) {
    let state: &ViewTransitionState = unsafe { js::cast(finished_work.state_node.get()) };
    // Updates can have conflicting names and classNames.
    // Since we're doing a reverse animation the "new" state is actually the current
    // and the "old" state is the finishedWork.
    let new_props: &ViewTransitionProps = unsafe { js::cast(current.memoized_props.get()) };
    let old_props: &ViewTransitionProps = unsafe { js::cast(finished_work.memoized_props.get()) };
    let old_name = get_view_transition_name(old_props, state);
    // This className applies only if there are fewer child DOM nodes than
    // before or if this update should've been cancelled but we ended up with
    // a parent animating so we need to animate the child too. Otherwise
    // the "new" state wins. Since "new" normally wins, that's usually what
    // we would use. However, since this animation is going in reverse we actually
    // want the props from "current" since that's the class that would've won if
    // it was the normal direction. To preserve the same effect in either direction.
    let class_name = get_view_transition_class_name(new_props.default, new_props.update);
    if class_name.as_deref() == Some("none") {
        // If update is "none" then we don't have to apply a name. Since we won't animate this boundary.
        return;
    }
    let clones = state.clones.borrow().clone();
    // If there are no clones at this point, that should mean that there are no
    // HostComponent children in this ViewTransition.
    if let Some(clones) = clones {
        apply_view_transition_to_clones(&old_name, class_name.as_deref(), &clones, finished_work);
    }
}

#[cfg_attr(rust_js, rust_js::nullable(parent_view_transition))]
fn recursively_insert_new(
    parent_fiber: &Rc<Fiber>,
    host_parent_clone: &'static Unknown,
    parent_view_transition: Option<&ViewTransitionState>,
    visit_phase: VisitPhase,
) {
    if visit_phase == INSERT_APPEARING_PAIR
        && parent_view_transition.is_none()
        && (parent_fiber.subtree_flags.get() & (VIEW_TRANSITION_NAMED_STATIC | PLACEMENT))
            == NO_FLAGS
    {
        // We're just searching for pairs or insertion effects but we have reached the end.
        return;
    }
    let mut child = parent_fiber.child.borrow().clone();
    while let Some(current) = child {
        recursively_insert_new_fiber(
            &current,
            host_parent_clone,
            parent_view_transition,
            visit_phase,
        );
        child = current.sibling.borrow().clone();
    }
}

#[cfg_attr(rust_js, rust_js::nullable(parent_view_transition))]
fn recursively_insert_new_fiber(
    finished_work: &Rc<Fiber>,
    host_parent_clone: &'static Unknown,
    parent_view_transition: Option<&ViewTransitionState>,
    visit_phase: VisitPhase,
) {
    match finished_work.tag.get() {
        FUNCTION_COMPONENT | FORWARD_REF | MEMO_COMPONENT | SIMPLE_MEMO_COMPONENT => {
            recursively_insert_new(
                finished_work,
                host_parent_clone,
                parent_view_transition,
                visit_phase,
            );
            if (finished_work.flags.get() & UPDATE) != 0 {
                // Insertion Effects are mounted temporarily during the rendering of the snapshot.
                // This does not affect cloned Offscreen content since those would've been mounted
                // while inside the offscreen tree already.
                // Note that because we are mounting a clone of the DOM tree and the previous DOM
                // tree remains mounted during the snapshot, we can't unmount any previous insertion
                // effects. This can lead to conflicts but that is similar to what can happen with
                // conflicts for two mounted Activity boundaries.
                commit_hook_effect_list_mount(HOOK_INSERTION | HOOK_HAS_EFFECT, finished_work);
            }
        }
        HOST_HOISTABLE if SUPPORTS_RESOURCES => {
            // TODO: Hoistables should get optimistically inserted and then removed.
            recursively_insert_new(
                finished_work,
                host_parent_clone,
                parent_view_transition,
                visit_phase,
            );
        }
        // Fall through
        HOST_HOISTABLE | HOST_SINGLETON if SUPPORTS_SINGLETONS => {
            recursively_insert_new(
                finished_work,
                host_parent_clone,
                parent_view_transition,
                visit_phase,
            );

            if __DEV__ {
                // We cannot apply mutations to Host Singletons since by definition
                // they cannot be cloned. Therefore we warn in DEV if this commit
                // had any effect.
                if (finished_work.flags.get() & UPDATE) != 0 {
                    console::error((
                        concat!(
                            "startGestureTransition() caused something to render a new <%s>. ",
                            "This is not possible in the current implementation. ",
                            "Make sure that the swipe doesn't mount any new <%s> elements."
                        ),
                        finished_work.r#type.get(),
                        finished_work.r#type.get(),
                    ));
                }
            }
        }
        // Fall through
        HOST_HOISTABLE | HOST_SINGLETON | HOST_COMPONENT => {
            let instance: &'static Unknown = unsafe { js::cast(finished_work.state_node.get()) };
            // For insertions we don't need to clone. It's already new state node.
            if visit_phase != INSERT_APPEARING_PAIR {
                append_child(host_parent_clone, instance);
                track_host_mutation();
                recursively_insert_new(finished_work, instance, None, INSERT_APPEARING_PAIR);
            } else {
                recursively_insert_new(finished_work, instance, None, visit_phase);
            }
            if let Some(parent_view_transition) = parent_view_transition {
                let clones = parent_view_transition.clones.borrow().clone();
                match clones {
                    None => {
                        parent_view_transition
                            .clones
                            .replace(Some(Rc::new(RefCell::new(vec![instance]))));
                    }
                    Some(clones) => clones.borrow_mut().push(instance),
                }
            }
        }
        HOST_TEXT => {
            let Some(text_instance) = finished_work.state_node.get() else {
                panic!(concat!(
                    "This should have a text node initialized. This error is likely ",
                    "caused by a bug in React. Please file an issue."
                ));
            };
            // For insertions we don't need to clone. It's already new state node.
            if visit_phase != INSERT_APPEARING_PAIR {
                append_child(host_parent_clone, text_instance);
                track_host_mutation();
            }
        }
        HOST_PORTAL => {
            // TODO: Consider what should happen to Portals. For now we exclude them.
        }
        OFFSCREEN_COMPONENT => {
            let new_state = finished_work.memoized_state.get();
            let is_hidden = new_state.is_some();
            if !is_hidden {
                // Only insert nodes if this tree is going to be visible. No need to
                // insert invisible content.
                // Since there was no mutation to this node, it couldn't have changed
                // visibility so we don't need to update visitPhase here.
                recursively_insert_new(
                    finished_work,
                    host_parent_clone,
                    parent_view_transition,
                    visit_phase,
                );
            }
        }
        VIEW_TRANSITION_COMPONENT => {
            let prev_mutation_context = push_mutation_context();
            let view_transition_state: &ViewTransitionState =
                unsafe { js::cast(finished_work.state_node.get()) };
            // TODO: If this was already cloned by a previous pass we can reuse those clones.
            view_transition_state.clones.replace(None);
            let next_phase = if visit_phase == INSERT_EXIT {
                // This was an Enter of a ViewTransition. We now move onto inserting the inner
                // HostComponents and finding inner pairs.
                INSERT_APPEND
            } else {
                visit_phase
            };
            recursively_insert_new(
                finished_work,
                host_parent_clone,
                Some(view_transition_state),
                next_phase,
            );
            // After we've inserted the new nodes into the "clones" set we can apply share
            // or exit transitions to them.
            if visit_phase == INSERT_EXIT {
                apply_exit_view_transition(finished_work);
            } else if visit_phase == INSERT_APPEARING_PAIR || visit_phase == INSERT_APPEND {
                apply_appearing_pair_view_transition(finished_work);
            }
            pop_mutation_context(prev_mutation_context);
        }
        _ => {
            recursively_insert_new(
                finished_work,
                host_parent_clone,
                parent_view_transition,
                visit_phase,
            );
        }
    }
}

#[cfg_attr(rust_js, rust_js::nullable(parent_view_transition))]
fn recursively_insert_clones_from_existing_tree(
    parent_fiber: &Fiber,
    host_parent_clone: &'static Unknown,
    parent_view_transition: Option<&ViewTransitionState>,
    visit_phase: VisitPhase,
) {
    let mut child = parent_fiber.child.borrow().clone();
    while let Some(current) = child {
        match current.tag.get() {
            HOST_COMPONENT => {
                let instance: &'static Unknown = unsafe { js::cast(current.state_node.get()) };
                let next_phase = match visit_phase {
                    CLONE_EXIT | CLONE_UNHIDE | CLONE_APPEARING_PAIR => {
                        // If this was an unhide, we need to keep going if there are any named
                        // pairs in this subtree, since they might need to be marked.
                        if (current.subtree_flags.get() & VIEW_TRANSITION_NAMED_STATIC) != NO_FLAGS
                        {
                            CLONE_APPEARING_PAIR
                        } else {
                            CLONE_UNCHANGED
                        }
                    }
                    _ => {
                        // We've found any "layout" View Transitions at this point so we can bail.
                        CLONE_UNCHANGED
                    }
                };
                let clone: &'static Unknown;
                if next_phase != CLONE_UNCHANGED {
                    // We might need a handle on these clones, so we need to do a shallow clone
                    // and keep going.
                    clone = clone_mutable_instance(instance, false);
                    recursively_insert_clones_from_existing_tree(&current, clone, None, next_phase);
                } else {
                    // If we have no mutations in this subtree, and we don't need a handle on the
                    // clones, then we can do a deep clone instead and bailout.
                    clone = clone_mutable_instance(instance, true);
                    // TODO: We may need to transfer some DOM state such as scroll position
                    // for the deep clones.
                    // TODO: If there's a manual view-transition-name inside the clone we
                    // should ideally remove it from the original and then restore it in mutation
                    // phase. Otherwise it leads to duplicate names.
                }
                append_child(host_parent_clone, clone);
                if let Some(parent_view_transition) = parent_view_transition {
                    let clones = parent_view_transition.clones.borrow().clone();
                    match clones {
                        None => {
                            parent_view_transition
                                .clones
                                .replace(Some(Rc::new(RefCell::new(vec![clone]))));
                        }
                        Some(clones) => clones.borrow_mut().push(clone),
                    }
                }
                if visit_phase == CLONE_EXIT || visit_phase == CLONE_UNHIDE {
                    unhide_instance(clone, current.memoized_props.get());
                    track_host_mutation();
                }
            }
            HOST_TEXT => {
                let Some(text_instance) = current.state_node.get() else {
                    panic!(concat!(
                        "This should have a text node initialized. This error is likely ",
                        "caused by a bug in React. Please file an issue."
                    ));
                };
                let clone = clone_mutable_text_instance(text_instance);
                append_child(host_parent_clone, clone);
                if visit_phase == CLONE_EXIT || visit_phase == CLONE_UNHIDE {
                    unhide_text_instance(clone, current.memoized_props.get());
                    track_host_mutation();
                }
            }
            HOST_PORTAL => {
                // TODO: Consider what should happen to Portals. For now we exclude them.
            }
            OFFSCREEN_COMPONENT => {
                let new_state = current.memoized_state.get();
                let is_hidden = new_state.is_some();
                if !is_hidden {
                    // Only insert clones if this tree is going to be visible. No need to
                    // clone invisible content.
                    // TODO: If this is visible but detached it should still be cloned.
                    // Since there was no mutation to this node, it couldn't have changed
                    // visibility so we don't need to update visitPhase here.
                    recursively_insert_clones_from_existing_tree(
                        &current,
                        host_parent_clone,
                        parent_view_transition,
                        visit_phase,
                    );
                }
            }
            VIEW_TRANSITION_COMPONENT => {
                let prev_mutation_context = push_mutation_context();
                let view_transition_state: &ViewTransitionState =
                    unsafe { js::cast(current.state_node.get()) };
                // TODO: If this was already cloned by a previous pass we can reuse those clones.
                view_transition_state.clones.replace(None);
                // "Existing" view transitions are in subtrees that didn't update so
                // this is a "current". We normally clear this upon rerendering
                // but we use this flag to track changes from layout in the commit.
                // So we need it to be cleared before we do that.
                // TODO: Use some other temporary state to track this.
                current.flags.set(current.flags.get() & !UPDATE);
                let next_phase = if visit_phase == CLONE_EXIT {
                    // This was an Enter of a ViewTransition. We now move onto unhiding the inner
                    // HostComponents and finding inner pairs.
                    CLONE_UNHIDE
                    // TODO: Mark the name and find a pair.
                } else if visit_phase == CLONE_UPDATE {
                    // If the tree had no mutations and we've found the top most ViewTransition
                    // then this is the one we might apply the "layout" state too if it has changed
                    // position. After we've found its HostComponents we can bail out.
                    CLONE_UNCHANGED
                } else {
                    visit_phase
                };
                recursively_insert_clones_from_existing_tree(
                    &current,
                    host_parent_clone,
                    Some(view_transition_state),
                    next_phase,
                );
                // After we've collected the cloned instances, we can apply exit or share transitions
                // to them.
                if visit_phase == CLONE_EXIT {
                    apply_exit_view_transition(&current);
                } else if visit_phase == CLONE_APPEARING_PAIR || visit_phase == CLONE_UNHIDE {
                    apply_appearing_pair_view_transition(&current);
                } else if visit_phase == CLONE_UPDATE {
                    apply_nested_view_transition(&current);
                }
                pop_mutation_context(prev_mutation_context);
            }
            _ => {
                recursively_insert_clones_from_existing_tree(
                    &current,
                    host_parent_clone,
                    parent_view_transition,
                    visit_phase,
                );
            }
        }
        child = current.sibling.borrow().clone();
    }
}

#[cfg_attr(rust_js, rust_js::nullable(parent_view_transition))]
fn recursively_insert_clones(
    parent_fiber: &Rc<Fiber>,
    host_parent_clone: &'static Unknown,
    parent_view_transition: Option<&ViewTransitionState>,
    visit_phase: VisitPhase,
) {
    let deletions = parent_fiber.deletions.borrow().clone();
    if let Some(deletions) = &deletions {
        let mut i = 0;
        while i < deletions.borrow().len() {
            let child_to_delete = Rc::clone(&deletions.borrow()[i]);
            track_enter_view_transitions(&child_to_delete);
            // Normally we would only mark something as triggering a mutation if there was
            // actually a HostInstance below here. If this tree didn't contain a HostInstances
            // we shouldn't trigger a mutation even though a virtual component was deleted.
            track_host_mutation();
            i += 1;
        }
    }

    if parent_fiber.alternate.borrow().is_none()
        || (parent_fiber.subtree_flags.get() & MUTATION_MASK) != NO_FLAGS
    {
        // If we have mutations or if this is a newly inserted tree, clone as we go.
        let mut child = parent_fiber.child.borrow().clone();
        while let Some(current) = child {
            insert_destination_clones_of_fiber(
                &current,
                host_parent_clone,
                parent_view_transition,
                visit_phase,
            );
            child = current.sibling.borrow().clone();
        }
    } else {
        // Once we reach a subtree with no more mutations we can bail out.
        // However, we must still insert deep clones of the HostComponents.
        recursively_insert_clones_from_existing_tree(
            parent_fiber,
            host_parent_clone,
            parent_view_transition,
            visit_phase,
        );
    }
}

/// Pops the mutation context a block pushed, however it's left.
struct PopMutationContext(bool);

impl Drop for PopMutationContext {
    fn drop(&mut self) {
        pop_mutation_context(self.0);
    }
}

#[cfg_attr(rust_js, rust_js::nullable(parent_view_transition))]
fn insert_destination_clones_of_fiber(
    finished_work: &Rc<Fiber>,
    host_parent_clone: &'static Unknown,
    parent_view_transition: Option<&ViewTransitionState>,
    visit_phase: VisitPhase,
) {
    let current = finished_work.alternate.borrow().clone();
    let Some(current) = current else {
        // This is a newly mounted subtree. Insert any HostComponents and trigger
        // Enter transitions.
        recursively_insert_new_fiber(
            finished_work,
            host_parent_clone,
            parent_view_transition,
            INSERT_EXIT,
        );
        return;
    };

    let flags = finished_work.flags.get();
    // The effect flag should be checked *after* we refine the type of fiber,
    // because the fiber tag is more specific. An exception is any flag related
    // to reconciliation, because those can be set on all fiber types.
    match finished_work.tag.get() {
        HOST_HOISTABLE if SUPPORTS_RESOURCES => {
            // TODO: Hoistables should get optimistically inserted and then removed.
            recursively_insert_clones(
                finished_work,
                host_parent_clone,
                parent_view_transition,
                visit_phase,
            );
        }
        // Fall through
        HOST_HOISTABLE | HOST_SINGLETON if SUPPORTS_SINGLETONS => {
            recursively_insert_clones(
                finished_work,
                host_parent_clone,
                parent_view_transition,
                visit_phase,
            );
            if __DEV__ {
                // We cannot apply mutations to Host Singletons since by definition
                // they cannot be cloned. Therefore we warn in DEV if this commit
                // had any effect.
                if (flags & UPDATE) != 0 {
                    let new_props = finished_work.memoized_props.get();
                    let old_props = current.memoized_props.get();
                    let instance = finished_work.state_node.get();
                    let r#type = finished_work.r#type.get();
                    let prev = push_mutation_context();

                    let _pop = PopMutationContext(prev);
                    // Since we currently don't have a separate diffing algorithm for
                    // individual properties, the Update flag can be a false positive.
                    // We have to apply the new props first o detect any mutations and
                    // then revert them.
                    commit_update(instance, r#type, old_props, new_props, finished_work);
                    if VIEW_TRANSITION_MUTATION_CONTEXT.get() {
                        console::error((
                            concat!(
                                "startGestureTransition() caused something to mutate <%s>. ",
                                "This is not possible in the current implementation. ",
                                "Make sure that the swipe doesn't update any state which ",
                                "causes <%s> to change."
                            ),
                            finished_work.r#type.get(),
                            finished_work.r#type.get(),
                        ));
                    }
                    // Revert
                    commit_update(instance, r#type, new_props, old_props, finished_work);
                }
            }
        }
        // Fall through
        HOST_HOISTABLE | HOST_SINGLETON | HOST_COMPONENT => {
            let instance: &'static Unknown = unsafe { js::cast(finished_work.state_node.get()) };
            let clone: &'static Unknown;
            if finished_work.child.borrow().is_none() {
                // This node is terminal. We still do a deep clone in case this has user
                // inserted content, text content or dangerouslySetInnerHTML.
                clone = clone_mutable_instance(instance, true);
                if (finished_work.flags.get() & CONTENT_RESET) != 0 {
                    reset_text_content(clone);
                    track_host_mutation();
                }
            } else {
                // If we have children we'll clone them as we walk the tree so we just
                // do a shallow clone here.
                clone = clone_mutable_instance(instance, false);
            }

            if (flags & UPDATE) != 0 {
                let new_props = finished_work.memoized_props.get();
                let old_props = current.memoized_props.get();
                let r#type = finished_work.r#type.get();
                // Apply the delta to the clone.
                commit_update(Some(clone), r#type, old_props, new_props, finished_work);
            }

            if visit_phase == CLONE_EXIT || visit_phase == CLONE_UNHIDE {
                append_child(host_parent_clone, clone);
                unhide_instance(clone, finished_work.memoized_props.get());
                recursively_insert_clones(finished_work, clone, None, CLONE_APPEARING_PAIR);
                track_host_mutation();
            } else {
                append_child(host_parent_clone, clone);
                recursively_insert_clones(finished_work, clone, None, visit_phase);
            }
            if let Some(parent_view_transition) = parent_view_transition {
                let clones = parent_view_transition.clones.borrow().clone();
                match clones {
                    None => {
                        parent_view_transition
                            .clones
                            .replace(Some(Rc::new(RefCell::new(vec![clone]))));
                    }
                    Some(clones) => clones.borrow_mut().push(clone),
                }
            }
        }
        HOST_TEXT => {
            let Some(text_instance) = finished_work.state_node.get() else {
                panic!(concat!(
                    "This should have a text node initialized. This error is likely ",
                    "caused by a bug in React. Please file an issue."
                ));
            };
            let clone = clone_mutable_text_instance(text_instance);
            if (flags & UPDATE) != 0 {
                let new_text: &str = unsafe { js::cast(finished_work.memoized_props.get()) };
                let old_text: &str = unsafe { js::cast(current.memoized_props.get()) };
                commit_text_update(clone, new_text, old_text);
                track_host_mutation();
            }
            append_child(host_parent_clone, clone);
            if visit_phase == CLONE_EXIT || visit_phase == CLONE_UNHIDE {
                unhide_text_instance(clone, finished_work.memoized_props.get());
                track_host_mutation();
            }
        }
        HOST_PORTAL => {
            // TODO: Consider what should happen to Portals. For now we exclude them.
        }
        OFFSCREEN_COMPONENT => {
            let new_state = finished_work.memoized_state.get();
            let is_hidden = new_state.is_some();
            if !is_hidden {
                // Only insert clones if this tree is going to be visible. No need to
                // clone invisible content.
                // TODO: If this is visible but detached it should still be cloned.
                let next_phase = if visit_phase == CLONE_UPDATE && (flags & VISIBILITY) != NO_FLAGS
                {
                    // This is the root of an appear. We need to trigger Enter transitions.
                    CLONE_EXIT
                } else {
                    visit_phase
                };
                recursively_insert_clones(
                    finished_work,
                    host_parent_clone,
                    parent_view_transition,
                    next_phase,
                );
            } else if current.memoized_state.get().is_none() {
                // Was previously mounted as visible but is now hidden.
                track_enter_view_transitions(&current);
                // Normally we would only mark something as triggering a mutation if there was
                // actually a HostInstance below here. If this tree didn't contain a HostInstances
                // we shouldn't trigger a mutation even though a virtual component was hidden.
                track_host_mutation();
            }
        }
        VIEW_TRANSITION_COMPONENT => {
            let prev_mutation_context = push_mutation_context();
            let view_transition_state: &ViewTransitionState =
                unsafe { js::cast(finished_work.state_node.get()) };
            // TODO: If this was already cloned by a previous pass we can reuse those clones.
            view_transition_state.clones.replace(None);
            let next_phase = if visit_phase == CLONE_EXIT {
                // This was an Enter of a ViewTransition. We now move onto unhiding the inner
                // HostComponents and finding inner pairs.
                CLONE_UNHIDE
                // TODO: Mark the name and find a pair.
            } else {
                visit_phase
            };
            recursively_insert_clones(
                finished_work,
                host_parent_clone,
                Some(view_transition_state),
                next_phase,
            );
            if VIEW_TRANSITION_MUTATION_CONTEXT.get() {
                // Track that this boundary had a mutation and therefore needs to animate
                // whether it resized or not.
                finished_work.flags.set(finished_work.flags.get() | UPDATE);
            }
            // After we've collected the cloned instances, we can apply exit or share transitions
            // to them.
            if visit_phase == CLONE_EXIT {
                apply_exit_view_transition(finished_work);
            } else if visit_phase == CLONE_APPEARING_PAIR || visit_phase == CLONE_UNHIDE {
                apply_appearing_pair_view_transition(finished_work);
            } else if visit_phase == CLONE_UPDATE {
                apply_update_view_transition(&current, finished_work);
            }
            pop_mutation_context(prev_mutation_context);
        }
        _ => {
            recursively_insert_clones(
                finished_work,
                host_parent_clone,
                parent_view_transition,
                visit_phase,
            );
        }
    }
}

// Clone View Transition boundaries that have any mutations or might have had their
// layout affected by child insertions.
pub fn insert_destination_clones(root: &FiberRoot, finished_work: &Rc<Fiber>) {
    // We'll either not transition the root, or we'll transition the clone. Regardless
    // we cancel the root view transition name.
    let needs_clone = detect_mutation_or_insert_clones(finished_work);
    if needs_clone {
        // Clone the whole root
        let root_clone = clone_root_view_transition_container(root.container_info);
        root.gesture_clone.set(Some(root_clone));
        recursively_insert_clones(finished_work, root_clone, None, CLONE_UPDATE);
    } else {
        root.gesture_clone.set(js::null());
        cancel_root_view_transition_name(root.container_info);
    }
}

fn measure_exit_view_transitions(placement: &Fiber) {
    if placement.tag.get() == VIEW_TRANSITION_COMPONENT {
        // const state: ViewTransitionState = placement.stateNode;
        let props: &ViewTransitionProps = unsafe { js::cast(placement.memoized_props.get()) };
        let name = props.name.as_deref();
        if let Some(name) = name
            && name != "auto"
        {
            // TODO: Find a pair
        }
    } else if (placement.subtree_flags.get() & VIEW_TRANSITION_STATIC) != NO_FLAGS {
        // TODO: Check if this is a hidden Offscreen or a Portal.
        let mut child = placement.child.borrow().clone();
        while let Some(current) = child {
            measure_exit_view_transitions(&current);
            child = current.sibling.borrow().clone();
        }
    } else {
        // We don't need to find pairs here because we would've already found and
        // measured the pairs inside the deletion phase.
    }
}

fn recursively_restore_new(finished_work: &Rc<Fiber>, nearest_mounted_ancestor: &Rc<Fiber>) {
    // There has to be move a Placement AND an Update flag somewhere below for this
    // pass to be relevant since we only apply insertion effects for new components here.
    if ((PLACEMENT | UPDATE) & finished_work.subtree_flags.get()) != NO_FLAGS {
        let mut child = finished_work.child.borrow().clone();
        while let Some(current) = child {
            recursively_restore_new(&current, nearest_mounted_ancestor);
            child = current.sibling.borrow().clone();
        }
    }
    match finished_work.tag.get() {
        FUNCTION_COMPONENT | FORWARD_REF | MEMO_COMPONENT | SIMPLE_MEMO_COMPONENT => {
            let current = finished_work.alternate.borrow().clone();
            if current.is_none() && (finished_work.flags.get() & UPDATE) != 0 {
                // Insertion Effects are mounted temporarily during the rendering of the snapshot.
                // We have now already takes a snapshot of the inserted state so we can now unmount
                // them to get back into the original state before starting the animation.
                commit_hook_effect_list_unmount(
                    HOOK_INSERTION | HOOK_HAS_EFFECT,
                    finished_work,
                    Some(nearest_mounted_ancestor),
                );
            }
        }
        _ => {}
    }
}

fn recursively_apply_view_transitions(parent_fiber: &Rc<Fiber>) {
    let deletions = parent_fiber.deletions.borrow().clone();
    if let Some(deletions) = &deletions {
        let mut i = 0;
        while i < deletions.borrow().len() {
            let child_to_delete = Rc::clone(&deletions.borrow()[i]);
            commit_enter_view_transitions(&child_to_delete, true);
            i += 1;
        }
    }

    if parent_fiber.alternate.borrow().is_none()
        || (parent_fiber.subtree_flags.get() & MUTATION_MASK) != NO_FLAGS
    {
        // If we have mutations or if this is a newly inserted tree, clone as we go.
        let mut child = parent_fiber.child.borrow().clone();
        while let Some(child_fiber) = child {
            let current = child_fiber.alternate.borrow().clone();
            match &current {
                None => {
                    measure_exit_view_transitions(&child_fiber);
                    recursively_restore_new(&child_fiber, parent_fiber);
                }
                Some(current) => {
                    apply_view_transitions_on_fiber(&child_fiber, current);
                }
            }
            child = child_fiber.sibling.borrow().clone();
        }
    } else {
        // Nothing has changed in this subtree, but the parent may have still affected
        // its size and position. We need to measure the old and new state to see if
        // we should animate its size and position.
        measure_nested_view_transitions(parent_fiber, true);
    }
}

fn apply_view_transitions_on_fiber(finished_work: &Rc<Fiber>, current: &Rc<Fiber>) {
    // The effect flag should be checked *after* we refine the type of fiber,
    // because the fiber tag is more specific. An exception is any flag related
    // to reconciliation, because those can be set on all fiber types.
    match finished_work.tag.get() {
        HOST_PORTAL => {
            // TODO: Consider what should happen to Portals. For now we exclude them.
        }
        OFFSCREEN_COMPONENT => {
            let new_state = finished_work.memoized_state.get();
            let is_hidden = new_state.is_some();
            let was_hidden = current.memoized_state.get().is_some();
            if !is_hidden {
                if was_hidden {
                    measure_exit_view_transitions(finished_work);
                    recursively_restore_new(finished_work, finished_work);
                } else {
                    recursively_apply_view_transitions(finished_work);
                }
            } else if !was_hidden {
                // Was previously mounted as visible but is now hidden.
                commit_enter_view_transitions(current, true);
            }
        }
        VIEW_TRANSITION_COMPONENT => {
            let prev_context_changed = VIEW_TRANSITION_CONTEXT_CHANGED.get();
            let prev_cancelable_children = push_view_transition_cancelable_scope();
            VIEW_TRANSITION_CONTEXT_CHANGED.set(false);
            recursively_apply_view_transitions(finished_work);

            if VIEW_TRANSITION_CONTEXT_CHANGED.get() {
                finished_work.flags.set(finished_work.flags.get() | UPDATE);
            }

            let in_viewport = measure_update_view_transition(current, finished_work, true);

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
                schedule_gesture_transition_event(finished_work, props.on_gesture_update);
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

            let view_transition_state: &ViewTransitionState =
                unsafe { js::cast(finished_work.state_node.get()) };
            view_transition_state.clones.replace(None); // Reset
        }
        _ => {
            recursively_apply_view_transitions(finished_work);
        }
    }
}

// Revert insertions and apply view transition names to the "new" (current) state.
pub fn apply_departure_transitions(root: &FiberRoot, finished_work: &Rc<Fiber>) {
    // First measure and apply view-transition-names to the "new" states.
    VIEW_TRANSITION_CONTEXT_CHANGED.set(false);
    push_view_transition_cancelable_scope();

    recursively_apply_view_transitions(finished_work);

    // Then remove the clones.
    let root_clone = root.gesture_clone.get();
    if let Some(root_clone) = root_clone {
        root.gesture_clone.set(js::null());
        remove_root_view_transition_clone(root.container_info, root_clone);
    }

    if !VIEW_TRANSITION_CONTEXT_CHANGED.get() {
        // If we didn't leak any resizing out to the root, we don't have to transition
        // the root itself. This means that we can now safely cancel any cancellations
        // that bubbled all the way up.
        let cancelable_children = VIEW_TRANSITION_CANCELABLE_CHILDREN.with_borrow(Clone::clone);
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
        // We also cancel the root itself. First we restore the name to the documentElement
        // and then we cancel it.
        restore_root_view_transition_name(root.container_info);
        cancel_root_view_transition_name(root.container_info);
    }
    pop_view_transition_cancelable_scope(js::null());
}

fn recursively_restore_view_transitions(parent_fiber: &Rc<Fiber>) {
    let deletions = parent_fiber.deletions.borrow().clone();
    if let Some(deletions) = &deletions {
        let mut i = 0;
        while i < deletions.borrow().len() {
            let child_to_delete = Rc::clone(&deletions.borrow()[i]);
            restore_enter_or_exit_view_transitions(&child_to_delete);
            i += 1;
        }
    }

    if parent_fiber.alternate.borrow().is_none()
        || (parent_fiber.subtree_flags.get() & MUTATION_MASK) != NO_FLAGS
    {
        // If we have mutations or if this is a newly inserted tree, clone as we go.
        let mut child = parent_fiber.child.borrow().clone();
        while let Some(current) = child {
            restore_view_transitions_on_fiber(&current);
            child = current.sibling.borrow().clone();
        }
    } else {
        // Nothing has changed in this subtree, but the parent may have still affected
        // its size and position. We need to measure the old and new state to see if
        // we should animate its size and position.
        restore_nested_view_transitions(parent_fiber);
    }
}

fn restore_view_transitions_on_fiber(finished_work: &Rc<Fiber>) {
    let current = finished_work.alternate.borrow().clone();
    let Some(current) = current else {
        restore_enter_or_exit_view_transitions(finished_work);
        return;
    };

    let flags = finished_work.flags.get();
    // The effect flag should be checked *after* we refine the type of fiber,
    // because the fiber tag is more specific. An exception is any flag related
    // to reconciliation, because those can be set on all fiber types.
    match finished_work.tag.get() {
        HOST_PORTAL => {
            // TODO: Consider what should happen to Portals. For now we exclude them.
        }
        OFFSCREEN_COMPONENT => {
            if (flags & VISIBILITY) != 0 {
                let new_state = finished_work.memoized_state.get();
                let is_hidden = new_state.is_some();
                if !is_hidden {
                    restore_enter_or_exit_view_transitions(finished_work);
                } else if current.memoized_state.get().is_none() {
                    // Was previously mounted as visible but is now hidden.
                    restore_enter_or_exit_view_transitions(&current);
                }
            }
        }
        VIEW_TRANSITION_COMPONENT => {
            restore_update_view_transition_for_gesture(&current, finished_work);
            recursively_restore_view_transitions(finished_work);
        }
        _ => {
            recursively_restore_view_transitions(finished_work);
        }
    }
}

// Revert transition names and start/adjust animations on the started View Transition.
pub fn start_gesture_animations(root: &FiberRoot, finished_work: &Rc<Fiber>) {
    restore_view_transitions_on_fiber(finished_work);
    restore_root_view_transition_name(root.container_info);
}
