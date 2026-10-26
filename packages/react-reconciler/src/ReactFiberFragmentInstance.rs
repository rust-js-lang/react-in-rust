// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::Unknown;

use crate::react_internal_types::Fiber;
use crate::react_work_tags::{FRAGMENT, HOST_COMPONENT, HOST_ROOT, HOST_SINGLETON, HOST_TEXT};

unsafe extern "Rust" {
    #[link_name = "./src/ReactFiberConfig.js#supportsSingletons"]
    safe static SUPPORTS_SINGLETONS: bool;
}

// ReactFiberConfig's `Instance` and `FragmentInstanceType`.
#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#commitNewChildToFragmentInstance"
)]
fn commit_new_child_to_fragment_instance(
    child_instance: Option<&'static Unknown>,
    fragment_instance: &'static Unknown,
) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#deleteChildFromFragmentInstance"
)]
fn delete_child_from_fragment_instance(
    child_instance: Option<&'static Unknown>,
    fragment_instance: &'static Unknown,
) {
    unreachable!()
}

#[cfg_attr(rust_js, rust_js::nullable(parent_fragment_instances))]
pub fn commit_new_child_to_fragment_instances(
    fiber: &Fiber,
    parent_fragment_instances: Option<&[&'static Unknown]>,
) {
    if (fiber.tag.get() != HOST_COMPONENT
        && fiber.tag.get() != HOST_SINGLETON
        && fiber.tag.get() != HOST_TEXT)
        // Only run fragment insertion effects for initial insertions
        || fiber.alternate.borrow().is_some()
    {
        return;
    }
    let Some(parent_fragment_instances) = parent_fragment_instances else {
        return;
    };
    for &fragment_instance in parent_fragment_instances {
        commit_new_child_to_fragment_instance(fiber.state_node.get(), fragment_instance);
    }
}

pub fn commit_fragment_instance_insertion_effects(fiber: &Fiber) {
    let mut parent = fiber.r#return.borrow().clone();
    while let Some(current) = parent {
        if is_fragment_instance_parent(&current) {
            // ReactFiberConfig's `FragmentInstanceType`.
            let fragment_instance: &Unknown = unsafe { js::cast(current.state_node.get()) };
            commit_new_child_to_fragment_instance(fiber.state_node.get(), fragment_instance);
        }

        if is_fragment_instance_host_boundary(&current) {
            return;
        }

        parent = current.r#return.borrow().clone();
    }
}

pub fn commit_fragment_instance_deletion_effects(fiber: &Fiber) {
    let mut parent = fiber.r#return.borrow().clone();
    while let Some(current) = parent {
        if is_fragment_instance_parent(&current) {
            // ReactFiberConfig's `FragmentInstanceType`.
            let fragment_instance: &Unknown = unsafe { js::cast(current.state_node.get()) };
            delete_child_from_fragment_instance(fiber.state_node.get(), fragment_instance);
        }

        if is_fragment_instance_host_boundary(&current) {
            return;
        }

        parent = current.r#return.borrow().clone();
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_parent_fragment_instances(fiber: &Fiber) -> Option<Vec<&'static Unknown>> {
    let mut parent_fragment_instances = js::null();
    let mut parent = fiber.r#return.borrow().clone();
    while let Some(current) = parent {
        if is_fragment_instance_parent(&current) {
            // ReactFiberConfig's `FragmentInstanceType`.
            let fragment_instance: &Unknown = unsafe { js::cast(current.state_node.get()) };
            match &mut parent_fragment_instances {
                None => parent_fragment_instances = Some(vec![fragment_instance]),
                Some(instances) => instances.push(fragment_instance),
            }
        }
        if is_fragment_instance_host_boundary(&current) {
            break;
        }
        parent = current.r#return.borrow().clone();
    }
    parent_fragment_instances
}

// HostPortal / HostHoistable are host parents for placement, but not for
// fragment instance ancestry — commit bookkeeping walks past them so it
// matches getFragmentParentInstanceOrContainerFiber. HostSingleton is a
// fragment host boundary (and a collected child) even when it is not a
// placement scope.
fn is_fragment_instance_host_boundary(fiber: &Fiber) -> bool {
    fiber.tag.get() == HOST_COMPONENT
        || fiber.tag.get() == HOST_ROOT
        || (SUPPORTS_SINGLETONS && fiber.tag.get() == HOST_SINGLETON)
}

fn is_fragment_instance_parent(fiber: &Fiber) -> bool {
    fiber.tag.get() == FRAGMENT && fiber.state_node.get().is_some()
}
