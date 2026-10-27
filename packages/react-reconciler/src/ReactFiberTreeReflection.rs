// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js::Unknown;

use crate::react_fiber::PortalStateNode;
use crate::react_fiber_activity_component::ActivityState;
use crate::react_fiber_flags::{HYDRATING, NO_FLAGS, PLACEMENT};
use crate::react_fiber_suspense_component::SuspenseState;
use crate::react_internal_types::{Fiber, FiberRoot};
use crate::react_work_tags::{
    ACTIVITY_COMPONENT, FRAGMENT, HOST_COMPONENT, HOST_HOISTABLE, HOST_PORTAL, HOST_ROOT,
    HOST_SINGLETON, HOST_TEXT, OFFSCREEN_COMPONENT, SUSPENSE_COMPONENT,
};

/// What a fragment's traversal calls of each host instance or text it
/// visits, given what it was given: whether it's done.
pub type Visit<A, B, C> = dyn Fn(&Rc<Fiber>, &A, &B, &C) -> bool;

/// What gives an instance's parent, as `getLowestCommonAncestor` climbs.
pub type GetParent = dyn Fn(Option<Rc<Fiber>>) -> Option<Rc<Fiber>>;

/// What `findFragmentInstanceOrTextInstanceSiblings` shares across its
/// recursive calls.
struct SiblingSearch {
    found_self: Cell<bool>,
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_nearest_mounted_fiber(fiber: &Rc<Fiber>) -> Option<Rc<Fiber>> {
    let mut node = Rc::clone(fiber);
    let mut nearest_mounted = Some(Rc::clone(fiber));
    // If there is no alternate, this might be a new tree that isn't inserted
    // yet. If it is, then it will have a pending insertion effect on it.
    let mut next_node = Some(Rc::clone(&node));
    while let Some(next) = next_node
        && next.alternate.borrow().is_none()
    {
        node = next;
        if (node.flags.get() & (PLACEMENT | HYDRATING)) != NO_FLAGS {
            // This is an insertion or in-progress hydration. The nearest possible
            // mounted fiber is the parent but we need to continue to figure out
            // if that one is still mounted.
            nearest_mounted = node.r#return.borrow().clone();
        }
        next_node = node.r#return.borrow().clone();
    }
    // After we've reached an alternate, go the rest of the way to see if the
    // tree is still mounted. If it's not, its return pointer will be disconnected.
    let mut parent = node.r#return.borrow().clone();
    while let Some(next) = parent {
        node = next;
        parent = node.r#return.borrow().clone();
    }
    if node.tag.get() == HOST_ROOT {
        // TODO: Check if this was a nested HostRoot when used with
        // renderContainerIntoSubtree.
        return nearest_mounted;
    }
    // If we didn't hit the root, that means that we're in an disconnected tree
    // that has been unmounted.
    None
}

// ReactFiberConfig's `SuspenseInstance`.
#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_suspense_instance_from_fiber(fiber: &Fiber) -> Option<&'static Unknown> {
    if fiber.tag.get() == SUSPENSE_COMPONENT {
        let mut suspense_state: Option<&SuspenseState> =
            unsafe { js::cast(fiber.memoized_state.get()) };
        if suspense_state.is_none() {
            let current = fiber.alternate.borrow().clone();
            if let Some(current) = current {
                suspense_state = unsafe { js::cast(current.memoized_state.get()) };
            }
        }
        if let Some(suspense_state) = suspense_state {
            return suspense_state.dehydrated;
        }
    }
    None
}

// ReactFiberConfig's `ActivityInstance`.
#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_activity_instance_from_fiber(fiber: &Fiber) -> Option<&'static Unknown> {
    if fiber.tag.get() == ACTIVITY_COMPONENT {
        let mut activity_state: Option<&ActivityState> =
            unsafe { js::cast(fiber.memoized_state.get()) };
        if activity_state.is_none() {
            let current = fiber.alternate.borrow().clone();
            if let Some(current) = current {
                activity_state = unsafe { js::cast(current.memoized_state.get()) };
            }
        }
        if let Some(activity_state) = activity_state {
            return Some(activity_state.dehydrated);
        }
    }
    // TODO: Implement this on ActivityComponent.
    None
}

// ReactFiberConfig's `Container`.
#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_container_from_fiber(fiber: &Fiber) -> Option<&'static Unknown> {
    if fiber.tag.get() == HOST_ROOT {
        Some(unsafe { js::cast::<&FiberRoot>(fiber.state_node.get()) }.container_info)
    } else {
        None
    }
}

fn assert_is_mounted(fiber: &Rc<Fiber>) {
    if get_nearest_mounted_fiber(fiber).as_ref() != Some(fiber) {
        panic!("Unable to find node on an unmounted component.");
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn find_current_fiber_using_slow_path(fiber: &Rc<Fiber>) -> Option<Rc<Fiber>> {
    let alternate = fiber.alternate.borrow().clone();
    let Some(alternate) = alternate else {
        // If there is no alternate, then we only need to check if it is mounted.
        let nearest_mounted = get_nearest_mounted_fiber(fiber);

        let Some(nearest_mounted) = nearest_mounted else {
            panic!("Unable to find node on an unmounted component.");
        };

        if nearest_mounted != *fiber {
            return None;
        }
        return Some(Rc::clone(fiber));
    };
    // If we have two possible branches, we'll walk backwards up to the root
    // to see what path the root points to. On the way we may hit one of the
    // special cases and we'll deal with them.
    let mut a = Rc::clone(fiber);
    let mut b = Rc::clone(&alternate);
    loop {
        let parent_a = a.r#return.borrow().clone();
        let Some(parent_a) = parent_a else {
            // We're at the root.
            break;
        };
        let parent_b = parent_a.alternate.borrow().clone();
        let Some(parent_b) = parent_b else {
            // There is no alternate. This is an unusual case. Currently, it only
            // happens when a Suspense component is hidden. An extra fragment fiber
            // is inserted in between the Suspense fiber and its children. Skip
            // over this extra fragment fiber and proceed to the next parent.
            let next_parent = parent_a.r#return.borrow().clone();
            if let Some(next_parent) = next_parent {
                b = Rc::clone(&next_parent);
                a = next_parent;
                continue;
            }
            // If there's no parent, we're at the root.
            break;
        };

        // If both copies of the parent fiber point to the same child, we can
        // assume that the child is current. This happens when we bailout on low
        // priority: the bailed out fiber's child reuses the current child.
        if *parent_a.child.borrow() == *parent_b.child.borrow() {
            let mut child = parent_a.child.borrow().clone();
            while let Some(current) = child {
                if current == a {
                    // We've determined that A is the current branch.
                    assert_is_mounted(&parent_a);
                    return Some(Rc::clone(fiber));
                }
                if current == b {
                    // We've determined that B is the current branch.
                    assert_is_mounted(&parent_a);
                    return Some(alternate);
                }
                child = current.sibling.borrow().clone();
            }

            // We should never have an alternate for any mounting node. So the only
            // way this could possibly happen is if this was unmounted, if at all.
            panic!("Unable to find node on an unmounted component.");
        }

        if *a.r#return.borrow() != *b.r#return.borrow() {
            // The return pointer of A and the return pointer of B point to different
            // fibers. We assume that return pointers never criss-cross, so A must
            // belong to the child set of A.return, and B must belong to the child
            // set of B.return.
            a = parent_a;
            b = parent_b;
        } else {
            // The return pointers point to the same fiber. We'll have to use the
            // default, slow path: scan the child sets of each parent alternate to see
            // which child belongs to which set.
            //
            // Search parent A's child set
            let mut did_find_child = false;
            let mut child = parent_a.child.borrow().clone();
            while let Some(current) = child {
                if current == a {
                    did_find_child = true;
                    a = Rc::clone(&parent_a);
                    b = Rc::clone(&parent_b);
                    break;
                }
                if current == b {
                    did_find_child = true;
                    b = Rc::clone(&parent_a);
                    a = Rc::clone(&parent_b);
                    break;
                }
                child = current.sibling.borrow().clone();
            }
            if !did_find_child {
                // Search parent B's child set
                child = parent_b.child.borrow().clone();
                while let Some(current) = child {
                    if current == a {
                        did_find_child = true;
                        a = Rc::clone(&parent_b);
                        b = Rc::clone(&parent_a);
                        break;
                    }
                    if current == b {
                        did_find_child = true;
                        b = Rc::clone(&parent_b);
                        a = Rc::clone(&parent_a);
                        break;
                    }
                    child = current.sibling.borrow().clone();
                }

                if !did_find_child {
                    panic!(
                        "{}",
                        concat!(
                            "Child was not found in either parent set. This indicates a bug ",
                            "in React related to the return pointer. Please file an issue."
                        )
                    );
                }
            }
        }

        if a.alternate.borrow().as_ref() != Some(&b) {
            panic!(
                "{}",
                concat!(
                    "Return fibers should always be each others' alternates. ",
                    "This error is likely caused by a bug in React. Please file an issue."
                )
            );
        }
    }

    // If the root is not a host container, we're in a disconnected tree. I.e.
    // unmounted.
    if a.tag.get() != HOST_ROOT {
        panic!("Unable to find node on an unmounted component.");
    }

    let root: &FiberRoot = unsafe { js::cast(a.state_node.get()) };
    if *root.current.borrow() == a {
        // We've determined that A is the current branch.
        return Some(Rc::clone(fiber));
    }
    // Otherwise B has to be current branch.
    Some(alternate)
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn find_current_host_fiber(parent: &Rc<Fiber>) -> Option<Rc<Fiber>> {
    let current_parent = find_current_fiber_using_slow_path(parent);
    current_parent.and_then(|current_parent| find_current_host_fiber_impl(&current_parent))
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn find_current_host_fiber_impl(node: &Rc<Fiber>) -> Option<Rc<Fiber>> {
    // Next we'll drill down this component to find the first HostComponent/Text.
    let tag = node.tag.get();
    if tag == HOST_COMPONENT || tag == HOST_HOISTABLE || tag == HOST_SINGLETON || tag == HOST_TEXT {
        return Some(Rc::clone(node));
    }

    let mut child = node.child.borrow().clone();
    while let Some(current) = child {
        let r#match = find_current_host_fiber_impl(&current);
        if r#match.is_some() {
            return r#match;
        }
        child = current.sibling.borrow().clone();
    }

    None
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn find_current_host_fiber_with_no_portals(parent: &Rc<Fiber>) -> Option<Rc<Fiber>> {
    let current_parent = find_current_fiber_using_slow_path(parent);
    current_parent
        .and_then(|current_parent| find_current_host_fiber_with_no_portals_impl(&current_parent))
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn find_current_host_fiber_with_no_portals_impl(node: &Rc<Fiber>) -> Option<Rc<Fiber>> {
    // Next we'll drill down this component to find the first HostComponent/Text.
    let tag = node.tag.get();
    if tag == HOST_COMPONENT || tag == HOST_HOISTABLE || tag == HOST_SINGLETON || tag == HOST_TEXT {
        return Some(Rc::clone(node));
    }

    let mut child = node.child.borrow().clone();
    while let Some(current) = child {
        if current.tag.get() != HOST_PORTAL {
            let r#match = find_current_host_fiber_with_no_portals_impl(&current);
            if r#match.is_some() {
                return r#match;
            }
        }
        child = current.sibling.borrow().clone();
    }

    None
}

pub fn is_fiber_suspense_and_timed_out(fiber: &Fiber) -> bool {
    let memoized_state: Option<&SuspenseState> = unsafe { js::cast(fiber.memoized_state.get()) };
    fiber.tag.get() == SUSPENSE_COMPONENT
        && memoized_state.is_some_and(|memoized_state| memoized_state.dehydrated.is_none())
}

pub fn does_fiber_contain(parent_fiber: &Rc<Fiber>, child_fiber: &Rc<Fiber>) -> bool {
    let mut node = Some(Rc::clone(child_fiber));
    let parent_fiber_alternate = parent_fiber.alternate.borrow().clone();
    while let Some(current) = node {
        if current == *parent_fiber || Some(&current) == parent_fiber_alternate.as_ref() {
            return true;
        }
        node = current.r#return.borrow().clone();
    }
    false
}

pub fn traverse_fragment_instances_and_text_instances<A, B, C>(
    fragment_fiber: &Fiber,
    r#fn: &Visit<A, B, C>,
    a: &A,
    b: &B,
    c: &C,
) {
    let child = fragment_fiber.child.borrow().clone();
    traverse_visible_instances_and_text_instances(child, false, r#fn, a, b, c);
}

pub fn traverse_fragment_instances_and_text_instances_deeply<A, B, C>(
    fragment_fiber: &Fiber,
    r#fn: &Visit<A, B, C>,
    a: &A,
    b: &B,
    c: &C,
) {
    let child = fragment_fiber.child.borrow().clone();
    traverse_visible_instances_and_text_instances(child, true, r#fn, a, b, c);
}

fn traverse_visible_instances_and_text_instances<A, B, C>(
    mut child: Option<Rc<Fiber>>,
    search_within_hosts: bool,
    r#fn: &Visit<A, B, C>,
    a: &A,
    b: &B,
    c: &C,
) -> bool {
    while let Some(current) = child {
        let is_host_node = current.tag.get() == HOST_COMPONENT
            || current.tag.get() == HOST_SINGLETON
            || current.tag.get() == HOST_TEXT;
        if is_host_node && r#fn(&current, a, b, c) {
            return true;
        } else if current.tag.get() == OFFSCREEN_COMPONENT && current.memoized_state.get().is_some()
        {
            // Skip hidden subtrees
        } else if search_within_hosts
            || (current.tag.get() != HOST_COMPONENT && current.tag.get() != HOST_SINGLETON)
        {
            let grandchild = current.child.borrow().clone();
            if traverse_visible_instances_and_text_instances(
                grandchild,
                search_within_hosts,
                r#fn,
                a,
                b,
                c,
            ) {
                return true;
            }
        }
        child = current.sibling.borrow().clone();
    }
    false
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_fragment_parent_instance_or_container_fiber(fiber: &Fiber) -> Option<Rc<Fiber>> {
    let mut parent = fiber.r#return.borrow().clone();
    while let Some(current) = parent {
        if current.tag.get() == HOST_ROOT
            || current.tag.get() == HOST_COMPONENT
            || current.tag.get() == HOST_SINGLETON
        {
            return Some(current);
        }
        parent = current.r#return.borrow().clone();
    }

    None
}

// ReactFiberConfig's `Container`.
#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_fragment_root_container_info(fiber: &Fiber) -> Option<&'static Unknown> {
    let mut parent = fiber.r#return.borrow().clone();
    while let Some(current) = parent {
        if current.tag.get() == HOST_ROOT {
            return Some(
                unsafe { js::cast::<&FiberRoot>(current.state_node.get()) }.container_info,
            );
        }
        parent = current.r#return.borrow().clone();
    }
    None
}

pub fn fiber_is_portaled_into_host(fiber: &Fiber) -> bool {
    let mut found_portal_parent = false;
    let mut parent = fiber.r#return.borrow().clone();
    while let Some(current) = parent {
        if current.tag.get() == HOST_PORTAL {
            found_portal_parent = true;
        }
        if current.tag.get() == HOST_ROOT
            || current.tag.get() == HOST_COMPONENT
            || current.tag.get() == HOST_SINGLETON
        {
            break;
        }
        parent = current.r#return.borrow().clone();
    }
    found_portal_parent
}

// ReactFiberConfig's `Container`: a portal's `containerInfo`.
#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_fragment_portal_container_info(fiber: &Fiber) -> Option<&'static Unknown> {
    let mut parent = fiber.r#return.borrow().clone();
    while let Some(current) = parent {
        if current.tag.get() == HOST_PORTAL {
            return unsafe { js::cast::<&PortalStateNode>(current.state_node.get()) }
                .container_info;
        }
        if current.tag.get() == HOST_ROOT
            || current.tag.get() == HOST_COMPONENT
            || current.tag.get() == HOST_SINGLETON
        {
            break;
        }
        parent = current.r#return.borrow().clone();
    }
    None
}

pub fn get_fragment_instance_or_text_instance_siblings(
    fiber: &Rc<Fiber>,
) -> [Option<Rc<Fiber>>; 2] {
    let mut result: [Option<Rc<Fiber>>; 2] = [js::null(), js::null()];
    let parent_host_fiber = get_fragment_parent_instance_or_container_fiber(fiber);
    let Some(parent_host_fiber) = parent_host_fiber else {
        return result;
    };

    let child = parent_host_fiber.child.borrow().clone();
    find_fragment_instance_or_text_instance_siblings(
        &mut result,
        fiber,
        child,
        &SiblingSearch {
            found_self: Cell::new(false),
        },
    );
    result
}

/// Collects HostComponent, HostSingleton, and HostText siblings.
/// Returns true once the following host sibling has been found.
fn find_fragment_instance_or_text_instance_siblings(
    result: &mut [Option<Rc<Fiber>>; 2],
    this: &Rc<Fiber>,
    mut child: Option<Rc<Fiber>>,
    state: &SiblingSearch,
) -> bool {
    while let Some(current) = child {
        if current == *this {
            // Shared across recursive calls so ancestors can keep scanning for
            // following host siblings after a nested empty fragment.
            state.found_self.set(true);
            child = current.sibling.borrow().clone();
            continue;
        }
        if current.tag.get() == HOST_COMPONENT
            || current.tag.get() == HOST_SINGLETON
            || current.tag.get() == HOST_TEXT
        {
            if state.found_self.get() {
                result[1] = Some(current);
                return true;
            } else {
                result[0] = Some(Rc::clone(&current));
            }
        } else if current.tag.get() == OFFSCREEN_COMPONENT && current.memoized_state.get().is_some()
        {
            // Skip hidden subtrees
        } else {
            let grandchild = current.child.borrow().clone();
            if find_fragment_instance_or_text_instance_siblings(result, this, grandchild, state) {
                return true;
            }
        }
        child = current.sibling.borrow().clone();
    }
    false
}

// ReactFiberConfig's `Instance | TextInstance | Container`.
pub fn get_instance_from_host_fiber(fiber: &Fiber) -> &'static Unknown {
    match fiber.tag.get() {
        HOST_COMPONENT | HOST_SINGLETON | HOST_TEXT => unsafe { js::cast(fiber.state_node.get()) },
        HOST_ROOT => unsafe { js::cast::<&FiberRoot>(fiber.state_node.get()) }.container_info,
        _ => panic!("Expected to find a host node. This is a bug in React."),
    }
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::nullable)]
    static SEARCH_TARGET: RefCell<Option<Rc<Fiber>>> = const { RefCell::new(None) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static SEARCH_BOUNDARY: RefCell<Option<Rc<Fiber>>> = const { RefCell::new(None) };
}

#[cfg_attr(rust_js, rust_js::nullable(target))]
fn push_search_target(target: Option<Rc<Fiber>>) {
    SEARCH_TARGET.set(target);
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn pop_search_target() -> Option<Rc<Fiber>> {
    SEARCH_TARGET.with_borrow(Clone::clone)
}

#[cfg_attr(rust_js, rust_js::nullable(value))]
fn push_search_boundary(value: Option<Rc<Fiber>>) {
    SEARCH_BOUNDARY.set(value);
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn pop_search_boundary() -> Option<Rc<Fiber>> {
    SEARCH_BOUNDARY.with_borrow(Clone::clone)
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_next_sibling_instance_or_text_instance_fiber(fiber: &Fiber) -> Option<Rc<Fiber>> {
    let sibling = fiber.sibling.borrow().clone();
    traverse_visible_instances_and_text_instances(
        sibling,
        false,
        &find_next_sibling,
        &(),
        &(),
        &(),
    );
    let sibling = pop_search_target();
    push_search_target(None);
    sibling
}

fn find_next_sibling(child: &Rc<Fiber>, _: &(), _: &(), _: &()) -> bool {
    push_search_target(Some(Rc::clone(child)));
    true
}

pub fn is_fiber_contained_by_fragment(fiber: &Rc<Fiber>, fragment_fiber: &Rc<Fiber>) -> bool {
    let mut current = Some(Rc::clone(fiber));
    while let Some(node) = current {
        if node.tag.get() == FRAGMENT
            && (node == *fragment_fiber || node.alternate.borrow().as_ref() == Some(fragment_fiber))
        {
            return true;
        }
        current = node.r#return.borrow().clone();
    }
    false
}

pub fn is_fragment_contained_by_fiber(fragment_fiber: &Rc<Fiber>, _other_fiber: &Fiber) -> bool {
    let mut current = Some(Rc::clone(fragment_fiber));
    let fiber_host_parent = get_fragment_parent_instance_or_container_fiber(fragment_fiber);
    while let Some(node) = current {
        if (node.tag.get() == HOST_COMPONENT
            || node.tag.get() == HOST_ROOT
            || node.tag.get() == HOST_SINGLETON)
            && (Some(&node) == fiber_host_parent.as_ref()
                || *node.alternate.borrow() == fiber_host_parent)
        {
            return true;
        }
        current = node.r#return.borrow().clone();
    }
    false
}

pub fn is_fiber_preceding(fiber: &Rc<Fiber>, other_fiber: &Rc<Fiber>) -> bool {
    let common_ancestor =
        get_lowest_common_ancestor(fiber, other_fiber, &get_parent_for_fragment_ancestors);
    if common_ancestor.is_none() {
        return false;
    }
    traverse_visible_instances_and_text_instances(
        common_ancestor,
        true,
        &is_fiber_preceding_check,
        other_fiber,
        fiber,
        &(),
    );
    let target = pop_search_target();
    push_search_target(None);
    target.is_some()
}

fn is_fiber_preceding_check(
    child: &Rc<Fiber>,
    target: &Rc<Fiber>,
    boundary: &Rc<Fiber>,
    _: &(),
) -> bool {
    if child == boundary {
        return true;
    }
    if child == target {
        push_search_target(Some(Rc::clone(child)));
        return true;
    }
    false
}

pub fn is_fiber_following(fiber: &Rc<Fiber>, other_fiber: &Rc<Fiber>) -> bool {
    let common_ancestor =
        get_lowest_common_ancestor(fiber, other_fiber, &get_parent_for_fragment_ancestors);
    if common_ancestor.is_none() {
        return false;
    }
    traverse_visible_instances_and_text_instances(
        common_ancestor,
        true,
        &is_fiber_following_check,
        other_fiber,
        fiber,
        &(),
    );
    let target = pop_search_target();
    push_search_target(None);
    push_search_boundary(None);
    target.is_some()
}

fn is_fiber_following_check(
    child: &Rc<Fiber>,
    target: &Rc<Fiber>,
    boundary: &Rc<Fiber>,
    _: &(),
) -> bool {
    if child == boundary {
        push_search_boundary(Some(Rc::clone(child)));
        return false;
    }
    if child == target {
        // The target is only following if we already found the boundary.
        if pop_search_boundary().is_some() {
            push_search_target(Some(Rc::clone(child)));
        }
        return true;
    }
    false
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn get_parent_for_fragment_ancestors(inst: Option<Rc<Fiber>>) -> Option<Rc<Fiber>> {
    let mut inst = inst?;
    loop {
        let parent = inst.r#return.borrow().clone()?;
        inst = parent;
        if inst.tag.get() == HOST_COMPONENT
            || inst.tag.get() == HOST_SINGLETON
            || inst.tag.get() == HOST_ROOT
        {
            return Some(inst);
        }
    }
}

/// Return the lowest common ancestor of A and B, or null if they are in
/// different trees.
#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_lowest_common_ancestor(
    inst_a: &Rc<Fiber>,
    inst_b: &Rc<Fiber>,
    get_parent: &GetParent,
) -> Option<Rc<Fiber>> {
    let mut node_a = Some(Rc::clone(inst_a));
    let mut node_b = Some(Rc::clone(inst_b));
    let mut depth_a = 0;
    let mut temp_a = node_a.clone();
    while let Some(temp) = temp_a {
        depth_a += 1;
        temp_a = get_parent(Some(temp));
    }
    let mut depth_b = 0;
    let mut temp_b = node_b.clone();
    while let Some(temp) = temp_b {
        depth_b += 1;
        temp_b = get_parent(Some(temp));
    }

    // If A is deeper, crawl up.
    while depth_a - depth_b > 0 {
        node_a = get_parent(node_a);
        depth_a -= 1;
    }

    // If B is deeper, crawl up.
    while depth_b - depth_a > 0 {
        node_b = get_parent(node_b);
        depth_b -= 1;
    }

    // Walk in lockstep until we find a match.
    let mut depth = depth_a;
    while depth != 0 {
        depth -= 1;
        if node_a == node_b
            || node_b
                .as_ref()
                .is_some_and(|node_b| *node_b.alternate.borrow() == node_a)
        {
            return node_a;
        }
        node_a = get_parent(node_a);
        node_b = get_parent(node_b);
    }
    None
}
