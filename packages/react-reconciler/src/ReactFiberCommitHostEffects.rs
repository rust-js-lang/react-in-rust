// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use js::{JsError, Unknown};
use shared::environment::__DEV__;

use crate::react_current_fiber::run_with_fiber_in_dev;
use crate::react_fiber_flags::{CONTENT_RESET, PLACEMENT};
use crate::react_fiber_fragment_instance::{
    commit_new_child_to_fragment_instances, get_parent_fragment_instances,
};
use crate::react_fiber_mutation_tracking::track_host_mutation;
use crate::react_fiber_work_loop::capture_commit_phase_error;
use crate::react_internal_types::{Fiber, FiberRoot};
use crate::react_work_tags::{
    DEHYDRATED_FRAGMENT, HOST_COMPONENT, HOST_HOISTABLE, HOST_PORTAL, HOST_ROOT, HOST_SINGLETON,
    HOST_TEXT,
};

unsafe extern "Rust" {
    #[link_name = "./src/ReactFiberConfig.js#supportsMutation"]
    safe static SUPPORTS_MUTATION: bool;
    #[link_name = "./src/ReactFiberConfig.js#supportsResources"]
    safe static SUPPORTS_RESOURCES: bool;
    #[link_name = "./src/ReactFiberConfig.js#supportsSingletons"]
    safe static SUPPORTS_SINGLETONS: bool;
}

// ReactFiberConfig's host operations, on its `Instance`, `TextInstance`,
// `Container`, `ChildSet`, `ActivityInstance` and `SuspenseInstance`.
#[cfg_attr(rust_js, rust_js::link_name = "./src/ReactFiberConfig.js#commitMount")]
fn commit_mount(
    instance: Option<&'static Unknown>,
    r#type: Option<&'static Unknown>,
    props: Option<&'static Unknown>,
    internal_instance_handle: &Fiber,
) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#commitHydratedInstance"
)]
fn commit_hydrated_instance(
    instance: Option<&'static Unknown>,
    r#type: Option<&'static Unknown>,
    props: Option<&'static Unknown>,
    internal_instance_handle: &Fiber,
) {
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
fn commit_text_update(text_instance: Option<&'static Unknown>, old_text: &str, new_text: &str) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#resetTextContent"
)]
fn reset_text_content(instance: Option<&'static Unknown>) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#hideDehydratedBoundary"
)]
fn hide_dehydrated_boundary(instance: Option<&'static Unknown>) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#unhideDehydratedBoundary"
)]
fn unhide_dehydrated_boundary(instance: Option<&'static Unknown>) {
    unreachable!()
}

#[cfg_attr(rust_js, rust_js::link_name = "./src/ReactFiberConfig.js#hideInstance")]
fn hide_instance(instance: Option<&'static Unknown>) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#unhideInstance"
)]
fn unhide_instance(instance: Option<&'static Unknown>, props: Option<&'static Unknown>) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#hideTextInstance"
)]
fn hide_text_instance(text_instance: Option<&'static Unknown>) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#unhideTextInstance"
)]
fn unhide_text_instance(text_instance: Option<&'static Unknown>, text: Option<&'static Unknown>) {
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
    rust_js::link_name = "./src/ReactFiberConfig.js#insertInContainerBefore"
)]
fn insert_in_container_before(
    container: Option<&'static Unknown>,
    child: Option<&'static Unknown>,
    before_child: &'static Unknown,
) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#appendChildToContainer"
)]
fn append_child_to_container(container: Option<&'static Unknown>, child: Option<&'static Unknown>) {
    unreachable!()
}

#[cfg_attr(rust_js, rust_js::link_name = "./src/ReactFiberConfig.js#insertBefore")]
fn insert_before(
    parent_instance: Option<&'static Unknown>,
    child: Option<&'static Unknown>,
    before_child: &'static Unknown,
) {
    unreachable!()
}

#[cfg_attr(rust_js, rust_js::link_name = "./src/ReactFiberConfig.js#appendChild")]
fn append_child(parent_instance: Option<&'static Unknown>, child: Option<&'static Unknown>) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#removeChildFromContainer"
)]
fn remove_child_from_container(container: &'static Unknown, child: &'static Unknown) {
    unreachable!()
}

#[cfg_attr(rust_js, rust_js::link_name = "./src/ReactFiberConfig.js#removeChild")]
fn remove_child(parent_instance: &'static Unknown, child: &'static Unknown) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#replaceContainerChildren"
)]
fn replace_container_children(container: &'static Unknown, new_children: Option<&'static Unknown>) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#commitHydratedContainer"
)]
fn commit_hydrated_container(container: &'static Unknown) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#commitHydratedActivityInstance"
)]
fn commit_hydrated_activity_instance(activity_instance: &'static Unknown) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#commitHydratedSuspenseInstance"
)]
fn commit_hydrated_suspense_instance(suspense_instance: &'static Unknown) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#acquireSingletonInstance"
)]
fn acquire_singleton_instance(
    r#type: Option<&'static Unknown>,
    props: Option<&'static Unknown>,
    instance: Option<&'static Unknown>,
    internal_instance_handle: &Fiber,
) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#releaseSingletonInstance"
)]
fn release_singleton_instance(
    instance: Option<&'static Unknown>,
    r#type: Option<&'static Unknown>,
    props: Option<&'static Unknown>,
) {
    unreachable!()
}

pub fn commit_host_mount(finished_work: &Rc<Fiber>) {
    let r#type = finished_work.r#type.get();
    let props = finished_work.memoized_props.get();
    // ReactFiberConfig's `Instance`.
    let instance = finished_work.state_node.get();
    if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
        if __DEV__ {
            run_with_fiber_in_dev(
                Some(Rc::clone(finished_work)),
                |instance: Option<&'static Unknown>,
                 r#type: Option<&'static Unknown>,
                 props: Option<&'static Unknown>,
                 finished_work: &Rc<Fiber>,
                 _: ()| commit_mount(instance, r#type, props, finished_work),
                instance,
                r#type,
                props,
                finished_work,
                (),
            );
        } else {
            commit_mount(instance, r#type, props, finished_work);
        }
    })) && let Some(error) = error.downcast_ref::<&'static JsError>()
    {
        let return_fiber = finished_work.r#return.borrow().clone();
        capture_commit_phase_error(finished_work, return_fiber.as_ref(), error.as_unknown());
    }
}

pub fn commit_host_hydrated_instance(finished_work: &Rc<Fiber>) {
    let r#type = finished_work.r#type.get();
    let props = finished_work.memoized_props.get();
    // ReactFiberConfig's `Instance`.
    let instance = finished_work.state_node.get();
    if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
        if __DEV__ {
            run_with_fiber_in_dev(
                Some(Rc::clone(finished_work)),
                |instance: Option<&'static Unknown>,
                 r#type: Option<&'static Unknown>,
                 props: Option<&'static Unknown>,
                 finished_work: &Rc<Fiber>,
                 _: ()| {
                    commit_hydrated_instance(instance, r#type, props, finished_work)
                },
                instance,
                r#type,
                props,
                finished_work,
                (),
            );
        } else {
            commit_hydrated_instance(instance, r#type, props, finished_work);
        }
    })) && let Some(error) = error.downcast_ref::<&'static JsError>()
    {
        let return_fiber = finished_work.r#return.borrow().clone();
        capture_commit_phase_error(finished_work, return_fiber.as_ref(), error.as_unknown());
    }
}

pub fn commit_host_update(
    finished_work: &Rc<Fiber>,
    new_props: Option<&'static Unknown>,
    old_props: Option<&'static Unknown>,
) {
    if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
        if __DEV__ {
            run_with_fiber_in_dev(
                Some(Rc::clone(finished_work)),
                |instance: Option<&'static Unknown>,
                 r#type: Option<&'static Unknown>,
                 old_props: Option<&'static Unknown>,
                 new_props: Option<&'static Unknown>,
                 finished_work: &Rc<Fiber>| {
                    commit_update(instance, r#type, old_props, new_props, finished_work)
                },
                finished_work.state_node.get(),
                finished_work.r#type.get(),
                old_props,
                new_props,
                finished_work,
            );
        } else {
            commit_update(
                finished_work.state_node.get(),
                finished_work.r#type.get(),
                old_props,
                new_props,
                finished_work,
            );
        }
        // Mutations are tracked manually from within commitUpdate.
    })) && let Some(error) = error.downcast_ref::<&'static JsError>()
    {
        let return_fiber = finished_work.r#return.borrow().clone();
        capture_commit_phase_error(finished_work, return_fiber.as_ref(), error.as_unknown());
    }
}

pub fn commit_host_text_update(finished_work: &Rc<Fiber>, new_text: &str, old_text: &str) {
    // ReactFiberConfig's `TextInstance`.
    let text_instance = finished_work.state_node.get();
    if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
        if __DEV__ {
            run_with_fiber_in_dev(
                Some(Rc::clone(finished_work)),
                |text_instance: Option<&'static Unknown>,
                 old_text: &str,
                 new_text: &str,
                 _: (),
                 _: ()| commit_text_update(text_instance, old_text, new_text),
                text_instance,
                old_text,
                new_text,
                (),
                (),
            );
        } else {
            commit_text_update(text_instance, old_text, new_text);
        }
        track_host_mutation();
    })) && let Some(error) = error.downcast_ref::<&'static JsError>()
    {
        let return_fiber = finished_work.r#return.borrow().clone();
        capture_commit_phase_error(finished_work, return_fiber.as_ref(), error.as_unknown());
    }
}

pub fn commit_host_reset_text_content(finished_work: &Rc<Fiber>) {
    // ReactFiberConfig's `Instance`.
    let instance = finished_work.state_node.get();
    if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
        if __DEV__ {
            run_with_fiber_in_dev(
                Some(Rc::clone(finished_work)),
                |instance: Option<&'static Unknown>, _: (), _: (), _: (), _: ()| {
                    reset_text_content(instance)
                },
                instance,
                (),
                (),
                (),
                (),
            );
        } else {
            reset_text_content(instance);
        }
        track_host_mutation();
    })) && let Some(error) = error.downcast_ref::<&'static JsError>()
    {
        let return_fiber = finished_work.r#return.borrow().clone();
        capture_commit_phase_error(finished_work, return_fiber.as_ref(), error.as_unknown());
    }
}

pub fn commit_show_hide_suspense_boundary(node: &Rc<Fiber>, is_hidden: bool) {
    if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
        let instance = node.state_node.get();
        if is_hidden {
            if __DEV__ {
                run_with_fiber_in_dev(
                    Some(Rc::clone(node)),
                    |instance: Option<&'static Unknown>, _: (), _: (), _: (), _: ()| {
                        hide_dehydrated_boundary(instance)
                    },
                    instance,
                    (),
                    (),
                    (),
                    (),
                );
            } else {
                hide_dehydrated_boundary(instance);
            }
        } else if __DEV__ {
            run_with_fiber_in_dev(
                Some(Rc::clone(node)),
                |instance: Option<&'static Unknown>, _: (), _: (), _: (), _: ()| {
                    unhide_dehydrated_boundary(instance)
                },
                node.state_node.get(),
                (),
                (),
                (),
                (),
            );
        } else {
            unhide_dehydrated_boundary(node.state_node.get());
        }
    })) && let Some(error) = error.downcast_ref::<&'static JsError>()
    {
        let return_fiber = node.r#return.borrow().clone();
        capture_commit_phase_error(node, return_fiber.as_ref(), error.as_unknown());
    }
}

pub fn commit_show_hide_host_instance(node: &Rc<Fiber>, is_hidden: bool) {
    if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
        let instance = node.state_node.get();
        if is_hidden {
            if __DEV__ {
                run_with_fiber_in_dev(
                    Some(Rc::clone(node)),
                    |instance: Option<&'static Unknown>, _: (), _: (), _: (), _: ()| {
                        hide_instance(instance)
                    },
                    instance,
                    (),
                    (),
                    (),
                    (),
                );
            } else {
                hide_instance(instance);
            }
        } else if __DEV__ {
            run_with_fiber_in_dev(
                Some(Rc::clone(node)),
                |instance: Option<&'static Unknown>,
                 props: Option<&'static Unknown>,
                 _: (),
                 _: (),
                 _: ()| unhide_instance(instance, props),
                node.state_node.get(),
                node.memoized_props.get(),
                (),
                (),
                (),
            );
        } else {
            unhide_instance(node.state_node.get(), node.memoized_props.get());
        }
    })) && let Some(error) = error.downcast_ref::<&'static JsError>()
    {
        let return_fiber = node.r#return.borrow().clone();
        capture_commit_phase_error(node, return_fiber.as_ref(), error.as_unknown());
    }
}

pub fn commit_show_hide_host_text_instance(node: &Rc<Fiber>, is_hidden: bool) {
    if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
        let instance = node.state_node.get();
        if is_hidden {
            if __DEV__ {
                run_with_fiber_in_dev(
                    Some(Rc::clone(node)),
                    |instance: Option<&'static Unknown>, _: (), _: (), _: (), _: ()| {
                        hide_text_instance(instance)
                    },
                    instance,
                    (),
                    (),
                    (),
                    (),
                );
            } else {
                hide_text_instance(instance);
            }
        } else if __DEV__ {
            run_with_fiber_in_dev(
                Some(Rc::clone(node)),
                |instance: Option<&'static Unknown>,
                 props: Option<&'static Unknown>,
                 _: (),
                 _: (),
                 _: ()| unhide_text_instance(instance, props),
                instance,
                node.memoized_props.get(),
                (),
                (),
                (),
            );
        } else {
            unhide_text_instance(instance, node.memoized_props.get());
        }
        track_host_mutation();
    })) && let Some(error) = error.downcast_ref::<&'static JsError>()
    {
        let return_fiber = node.r#return.borrow().clone();
        capture_commit_phase_error(node, return_fiber.as_ref(), error.as_unknown());
    }
}

fn is_host_parent(fiber: &Fiber) -> bool {
    fiber.tag.get() == HOST_COMPONENT
        || fiber.tag.get() == HOST_ROOT
        || (SUPPORTS_RESOURCES && fiber.tag.get() == HOST_HOISTABLE)
        || (SUPPORTS_SINGLETONS
            && fiber.tag.get() == HOST_SINGLETON
            && is_singleton_scope(fiber.r#type.get()))
        || fiber.tag.get() == HOST_PORTAL
}

// ReactFiberConfig's `Instance`.
#[cfg_attr(rust_js, rust_js::nullable)]
fn get_host_sibling(fiber: &Rc<Fiber>) -> Option<&'static Unknown> {
    // We're going to search forward into the tree until we find a sibling host
    // node. Unfortunately, if multiple insertions are done in a row we have to
    // search past them. This leads to exponential search for the next sibling.
    // TODO: Find a more efficient way to do this.
    let mut node = Rc::clone(fiber);
    'siblings: loop {
        // If we didn't find anything, let's try the next sibling.
        while node.sibling.borrow().is_none() {
            let return_fiber = node.r#return.borrow().clone();
            let Some(return_fiber) = return_fiber else {
                // If we pop out of the root or hit the parent the fiber we are the
                // last sibling.
                return None;
            };
            if is_host_parent(&return_fiber) {
                return None;
            }
            node = return_fiber;
        }
        let sibling = node.sibling.borrow().clone().unwrap();
        let return_fiber = node.r#return.borrow().clone();
        sibling.r#return.replace(return_fiber);
        node = sibling;
        while node.tag.get() != HOST_COMPONENT
            && node.tag.get() != HOST_TEXT
            && node.tag.get() != DEHYDRATED_FRAGMENT
        {
            // If this is a host singleton we go deeper if it's not a special
            // singleton scope. If it is a singleton scope we skip over it because
            // you only insert against this scope when you are already inside of it
            if SUPPORTS_SINGLETONS
                && node.tag.get() == HOST_SINGLETON
                && is_singleton_scope(node.r#type.get())
            {
                continue 'siblings;
            }

            // If it is not host node and, we might have a host node inside it.
            // Try to search down until we find one.
            if (node.flags.get() & PLACEMENT) != 0 {
                // If we don't have a child, try the siblings instead.
                continue 'siblings;
            }
            // If we don't have a child, try the siblings instead.
            // We also skip portals because they are not part of this host tree.
            let child = node.child.borrow().clone();
            if let Some(child) = child
                && node.tag.get() != HOST_PORTAL
            {
                child.r#return.replace(Some(Rc::clone(&node)));
                node = child;
            } else {
                continue 'siblings;
            }
        }
        // Check if this host node is stable or about to be placed.
        if (node.flags.get() & PLACEMENT) == 0 {
            // Found it!
            return node.state_node.get();
        }
    }
}

// ReactFiberConfig's `Instance`, `Container` and `FragmentInstanceType`.
#[cfg_attr(rust_js, rust_js::nullable(parent_fragment_instances))]
fn insert_or_append_placement_node_into_container(
    node: &Fiber,
    mut before: Option<&'static Unknown>,
    mut parent: Option<&'static Unknown>,
    mut parent_fragment_instances: Option<&[&'static Unknown]>,
) {
    let tag = node.tag.get();
    let is_host = tag == HOST_COMPONENT || tag == HOST_TEXT;
    if is_host {
        let state_node = node.state_node.get();
        if let Some(before) = before {
            insert_in_container_before(parent, state_node, before);
        } else {
            append_child_to_container(parent, state_node);
        }
        commit_new_child_to_fragment_instances(node, parent_fragment_instances);
        track_host_mutation();
        return;
    } else if tag == HOST_PORTAL {
        // If the insertion itself is a portal, then we don't want to traverse
        // down its children. Instead, we'll get insertions from each child in
        // the portal directly.
        return;
    }

    if SUPPORTS_SINGLETONS && tag == HOST_SINGLETON {
        // The singleton is the fragment child. Its own children are not
        // attributed to the fragment instances above it.
        commit_new_child_to_fragment_instances(node, parent_fragment_instances);
        parent_fragment_instances = None;
        if is_singleton_scope(node.r#type.get()) {
            // This singleton is the parent of deeper nodes and needs to become
            // the parent for child insertions and appends
            parent = node.state_node.get();
            before = js::null();
        }
    }

    let child = node.child.borrow().clone();
    if let Some(child) = child {
        insert_or_append_placement_node_into_container(
            &child,
            before,
            parent,
            parent_fragment_instances,
        );
        let mut sibling = child.sibling.borrow().clone();
        while let Some(current) = sibling {
            insert_or_append_placement_node_into_container(
                &current,
                before,
                parent,
                parent_fragment_instances,
            );
            sibling = current.sibling.borrow().clone();
        }
    }
}

// ReactFiberConfig's `Instance` and `FragmentInstanceType`.
#[cfg_attr(rust_js, rust_js::nullable(parent_fragment_instances))]
fn insert_or_append_placement_node(
    node: &Fiber,
    before: Option<&'static Unknown>,
    mut parent: Option<&'static Unknown>,
    mut parent_fragment_instances: Option<&[&'static Unknown]>,
) {
    let tag = node.tag.get();
    let is_host = tag == HOST_COMPONENT || tag == HOST_TEXT;
    if is_host {
        let state_node = node.state_node.get();
        if let Some(before) = before {
            insert_before(parent, state_node, before);
        } else {
            append_child(parent, state_node);
        }
        commit_new_child_to_fragment_instances(node, parent_fragment_instances);
        track_host_mutation();
        return;
    } else if tag == HOST_PORTAL {
        // If the insertion itself is a portal, then we don't want to traverse
        // down its children. Instead, we'll get insertions from each child in
        // the portal directly.
        return;
    }

    if SUPPORTS_SINGLETONS && tag == HOST_SINGLETON {
        // The singleton is the fragment child. Its own children are not
        // attributed to the fragment instances above it.
        commit_new_child_to_fragment_instances(node, parent_fragment_instances);
        parent_fragment_instances = None;
        if is_singleton_scope(node.r#type.get()) {
            // This singleton is the parent of deeper nodes and needs to become
            // the parent for child insertions and appends
            parent = node.state_node.get();
        }
    }

    let child = node.child.borrow().clone();
    if let Some(child) = child {
        insert_or_append_placement_node(&child, before, parent, parent_fragment_instances);
        let mut sibling = child.sibling.borrow().clone();
        while let Some(current) = sibling {
            insert_or_append_placement_node(&current, before, parent, parent_fragment_instances);
            sibling = current.sibling.borrow().clone();
        }
    }
}

fn commit_placement(finished_work: &Rc<Fiber>) {
    // Recursively insert all host nodes into the parent.
    let mut host_parent_fiber = None;
    let mut parent_fiber = finished_work.r#return.borrow().clone();
    while let Some(current) = parent_fiber {
        if is_host_parent(&current) {
            host_parent_fiber = Some(current);
            break;
        }
        parent_fiber = current.r#return.borrow().clone();
    }
    // Fragment ancestry is collected separately so portals remain placement
    // parents while fragment bookkeeping still walks past them to ancestors.
    let parent_fragment_instances = get_parent_fragment_instances(finished_work);

    if !SUPPORTS_MUTATION {
        commit_immutable_placement_node_to_fragment_instances(
            finished_work,
            parent_fragment_instances.as_deref(),
        );
        return;
    }

    let Some(host_parent_fiber) = host_parent_fiber else {
        panic!(concat!(
            "Expected to find a host parent. This error is likely caused by a bug ",
            "in React. Please file an issue."
        ));
    };

    match host_parent_fiber.tag.get() {
        HOST_SINGLETON if SUPPORTS_SINGLETONS => {
            // ReactFiberConfig's `Instance`.
            let parent = host_parent_fiber.state_node.get();
            let before = get_host_sibling(finished_work);
            // We only have the top Fiber that was inserted but we need to recurse down its
            // children to find all the terminal nodes.
            insert_or_append_placement_node(
                finished_work,
                before,
                parent,
                parent_fragment_instances.as_deref(),
            );
        }
        // Fall through
        HOST_SINGLETON | HOST_COMPONENT => {
            // ReactFiberConfig's `Instance`.
            let parent = host_parent_fiber.state_node.get();
            if (host_parent_fiber.flags.get() & CONTENT_RESET) != 0 {
                // Reset the text content of the parent before doing any insertions
                reset_text_content(parent);
                // Clear ContentReset from the effect tag
                host_parent_fiber
                    .flags
                    .set(host_parent_fiber.flags.get() & !CONTENT_RESET);
            }

            let before = get_host_sibling(finished_work);
            // We only have the top Fiber that was inserted but we need to recurse down its
            // children to find all the terminal nodes.
            insert_or_append_placement_node(
                finished_work,
                before,
                parent,
                parent_fragment_instances.as_deref(),
            );
        }
        HOST_ROOT | HOST_PORTAL => {
            // ReactFiberConfig's `Container`.
            let parent = js::get(host_parent_fiber.state_node.get(), "containerInfo");
            let before = get_host_sibling(finished_work);
            insert_or_append_placement_node_into_container(
                finished_work,
                before,
                parent,
                parent_fragment_instances.as_deref(),
            );
        }
        _ => panic!(concat!(
            "Invalid host parent fiber. This error is likely caused by a bug ",
            "in React. Please file an issue."
        )),
    }
}

// ReactFiberConfig's `FragmentInstanceType`.
#[cfg_attr(rust_js, rust_js::nullable(parent_fragment_instances))]
fn commit_immutable_placement_node_to_fragment_instances(
    finished_work: &Fiber,
    parent_fragment_instances: Option<&[&'static Unknown]>,
) {
    let is_host = finished_work.tag.get() == HOST_COMPONENT
        || (SUPPORTS_SINGLETONS && finished_work.tag.get() == HOST_SINGLETON);
    if is_host {
        // A singleton is the fragment child itself, so its own children are
        // not attributed to the fragment instances above it.
        commit_new_child_to_fragment_instances(finished_work, parent_fragment_instances);
        return;
    } else if finished_work.tag.get() == HOST_PORTAL {
        // If the insertion itself is a portal, then we don't want to traverse
        // down its children. Instead, we'll get insertions from each child in
        // the portal directly.
        return;
    }

    let child = finished_work.child.borrow().clone();
    if let Some(child) = child {
        commit_immutable_placement_node_to_fragment_instances(&child, parent_fragment_instances);
        let mut sibling = child.sibling.borrow().clone();
        while let Some(current) = sibling {
            commit_immutable_placement_node_to_fragment_instances(
                &current,
                parent_fragment_instances,
            );
            sibling = current.sibling.borrow().clone();
        }
    }
}

pub fn commit_host_placement(finished_work: &Rc<Fiber>) {
    if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
        if __DEV__ {
            run_with_fiber_in_dev(
                Some(Rc::clone(finished_work)),
                |finished_work: &Rc<Fiber>, _: (), _: (), _: (), _: ()| {
                    commit_placement(finished_work)
                },
                finished_work,
                (),
                (),
                (),
                (),
            );
        } else {
            commit_placement(finished_work);
        }
    })) && let Some(error) = error.downcast_ref::<&'static JsError>()
    {
        let return_fiber = finished_work.r#return.borrow().clone();
        capture_commit_phase_error(finished_work, return_fiber.as_ref(), error.as_unknown());
    }
}

pub fn commit_host_remove_child_from_container(
    deleted_fiber: &Rc<Fiber>,
    nearest_mounted_ancestor: &Rc<Fiber>,
    parent_container: &'static Unknown,
    host_instance: &'static Unknown,
) {
    if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
        if __DEV__ {
            run_with_fiber_in_dev(
                Some(Rc::clone(deleted_fiber)),
                |container: &'static Unknown, child: &'static Unknown, _: (), _: (), _: ()| {
                    remove_child_from_container(container, child)
                },
                parent_container,
                host_instance,
                (),
                (),
                (),
            );
        } else {
            remove_child_from_container(parent_container, host_instance);
        }
        track_host_mutation();
    })) && let Some(error) = error.downcast_ref::<&'static JsError>()
    {
        capture_commit_phase_error(
            deleted_fiber,
            Some(nearest_mounted_ancestor),
            error.as_unknown(),
        );
    }
}

pub fn commit_host_remove_child(
    deleted_fiber: &Rc<Fiber>,
    nearest_mounted_ancestor: &Rc<Fiber>,
    parent_instance: &'static Unknown,
    host_instance: &'static Unknown,
) {
    if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
        if __DEV__ {
            run_with_fiber_in_dev(
                Some(Rc::clone(deleted_fiber)),
                |parent_instance: &'static Unknown,
                 child: &'static Unknown,
                 _: (),
                 _: (),
                 _: ()| remove_child(parent_instance, child),
                parent_instance,
                host_instance,
                (),
                (),
                (),
            );
        } else {
            remove_child(parent_instance, host_instance);
        }
        track_host_mutation();
    })) && let Some(error) = error.downcast_ref::<&'static JsError>()
    {
        capture_commit_phase_error(
            deleted_fiber,
            Some(nearest_mounted_ancestor),
            error.as_unknown(),
        );
    }
}

pub fn commit_host_root_container_children(root: &FiberRoot, finished_work: &Rc<Fiber>) {
    let container_info = root.container_info;
    let pending_children = root.pending_children.get();
    if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
        if __DEV__ {
            run_with_fiber_in_dev(
                Some(Rc::clone(finished_work)),
                |container: &'static Unknown,
                 new_children: Option<&'static Unknown>,
                 _: (),
                 _: (),
                 _: ()| replace_container_children(container, new_children),
                container_info,
                pending_children,
                (),
                (),
                (),
            );
        } else {
            replace_container_children(container_info, pending_children);
        }
        track_host_mutation();
    })) && let Some(error) = error.downcast_ref::<&'static JsError>()
    {
        let return_fiber = finished_work.r#return.borrow().clone();
        capture_commit_phase_error(finished_work, return_fiber.as_ref(), error.as_unknown());
    }
}

pub fn commit_host_portal_container_children(
    portal: &'static Unknown,
    finished_work: &Rc<Fiber>,
    pending_children: Option<&'static Unknown>,
) {
    let container_info: &Unknown = unsafe { js::cast(js::get(portal, "containerInfo")) };
    if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
        if __DEV__ {
            run_with_fiber_in_dev(
                Some(Rc::clone(finished_work)),
                |container: &'static Unknown,
                 new_children: Option<&'static Unknown>,
                 _: (),
                 _: (),
                 _: ()| replace_container_children(container, new_children),
                container_info,
                pending_children,
                (),
                (),
                (),
            );
        } else {
            replace_container_children(container_info, pending_children);
        }
    })) && let Some(error) = error.downcast_ref::<&'static JsError>()
    {
        let return_fiber = finished_work.r#return.borrow().clone();
        capture_commit_phase_error(finished_work, return_fiber.as_ref(), error.as_unknown());
    }
}

pub fn commit_host_hydrated_container(root: &FiberRoot, finished_work: &Rc<Fiber>) {
    if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
        if __DEV__ {
            run_with_fiber_in_dev(
                Some(Rc::clone(finished_work)),
                |container: &'static Unknown, _: (), _: (), _: (), _: ()| {
                    commit_hydrated_container(container)
                },
                root.container_info,
                (),
                (),
                (),
                (),
            );
        } else {
            commit_hydrated_container(root.container_info);
        }
    })) && let Some(error) = error.downcast_ref::<&'static JsError>()
    {
        let return_fiber = finished_work.r#return.borrow().clone();
        capture_commit_phase_error(finished_work, return_fiber.as_ref(), error.as_unknown());
    }
}

pub fn commit_host_hydrated_activity(
    activity_instance: &'static Unknown,
    finished_work: &Rc<Fiber>,
) {
    if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
        if __DEV__ {
            run_with_fiber_in_dev(
                Some(Rc::clone(finished_work)),
                |activity_instance: &'static Unknown, _: (), _: (), _: (), _: ()| {
                    commit_hydrated_activity_instance(activity_instance)
                },
                activity_instance,
                (),
                (),
                (),
                (),
            );
        } else {
            commit_hydrated_activity_instance(activity_instance);
        }
    })) && let Some(error) = error.downcast_ref::<&'static JsError>()
    {
        let return_fiber = finished_work.r#return.borrow().clone();
        capture_commit_phase_error(finished_work, return_fiber.as_ref(), error.as_unknown());
    }
}

pub fn commit_host_hydrated_suspense(
    suspense_instance: &'static Unknown,
    finished_work: &Rc<Fiber>,
) {
    if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
        if __DEV__ {
            run_with_fiber_in_dev(
                Some(Rc::clone(finished_work)),
                |suspense_instance: &'static Unknown, _: (), _: (), _: (), _: ()| {
                    commit_hydrated_suspense_instance(suspense_instance)
                },
                suspense_instance,
                (),
                (),
                (),
                (),
            );
        } else {
            commit_hydrated_suspense_instance(suspense_instance);
        }
    })) && let Some(error) = error.downcast_ref::<&'static JsError>()
    {
        let return_fiber = finished_work.r#return.borrow().clone();
        capture_commit_phase_error(finished_work, return_fiber.as_ref(), error.as_unknown());
    }
}

pub fn commit_host_singleton_acquisition(finished_work: &Rc<Fiber>) {
    let singleton = finished_work.state_node.get();
    let props = finished_work.memoized_props.get();

    if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
        // This was a new mount, acquire the DOM instance and set initial properties
        if __DEV__ {
            run_with_fiber_in_dev(
                Some(Rc::clone(finished_work)),
                |r#type: Option<&'static Unknown>,
                 props: Option<&'static Unknown>,
                 instance: Option<&'static Unknown>,
                 internal_instance_handle: &Rc<Fiber>,
                 _: ()| {
                    acquire_singleton_instance(r#type, props, instance, internal_instance_handle)
                },
                finished_work.r#type.get(),
                props,
                singleton,
                finished_work,
                (),
            );
        } else {
            acquire_singleton_instance(finished_work.r#type.get(), props, singleton, finished_work);
        }
    })) && let Some(error) = error.downcast_ref::<&'static JsError>()
    {
        let return_fiber = finished_work.r#return.borrow().clone();
        capture_commit_phase_error(finished_work, return_fiber.as_ref(), error.as_unknown());
    }
}

pub fn commit_host_singleton_release(releasing_work: &Rc<Fiber>) {
    if __DEV__ {
        run_with_fiber_in_dev(
            Some(Rc::clone(releasing_work)),
            |instance: Option<&'static Unknown>,
             r#type: Option<&'static Unknown>,
             props: Option<&'static Unknown>,
             _: (),
             _: ()| release_singleton_instance(instance, r#type, props),
            releasing_work.state_node.get(),
            releasing_work.r#type.get(),
            releasing_work.memoized_props.get(),
            (),
            (),
        );
    } else {
        release_singleton_instance(
            releasing_work.state_node.get(),
            releasing_work.r#type.get(),
            releasing_work.memoized_props.get(),
        );
    }
}
