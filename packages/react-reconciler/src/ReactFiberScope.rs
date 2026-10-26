// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::rc::Rc;

use js::{Dict, Unknown};
use shared::react_feature_flags::ENABLE_SCOPE_API;

use crate::react_fiber_tree_reflection::is_fiber_suspense_and_timed_out;
use crate::react_internal_types::Fiber;
use crate::react_work_tags::{CONTEXT_PROVIDER, HOST_COMPONENT, SCOPE_COMPONENT};

// ReactFiberConfig's `Instance`'s public instance.
#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#getPublicInstance",
    rust_js::nullable
)]
fn get_public_instance(instance: Option<&'static Unknown>) -> Option<&'static Unknown> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#getInstanceFromNode",
    rust_js::nullable
)]
fn get_instance_from_node(node: &Unknown) -> Option<Rc<Fiber>> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#getInstanceFromScope",
    rust_js::nullable
)]
fn get_instance_from_scope(scope_instance: &Unknown) -> Option<Rc<Fiber>> {
    unreachable!()
}

/// shared/ReactTypes' `ReactScopeQuery`: whether a host instance, of its
/// type and props, is one the query wants.
pub type ReactScopeQuery = dyn Fn(
    Option<&'static Unknown>,
    Option<&'static Unknown>,
    &'static Unknown,
) -> Option<&'static Unknown>;

#[cfg_attr(rust_js, rust_js::nullable)]
fn get_suspense_fallback_child(fiber: &Fiber) -> Option<Rc<Fiber>> {
    fiber
        .child
        .borrow()
        .as_ref()
        .unwrap()
        .sibling
        .borrow()
        .as_ref()
        .unwrap()
        .child
        .borrow()
        .clone()
}

thread_local! {
    static EMPTY_OBJECT: &'static Dict<()> = js::dict::new();
}

fn collect_scoped_nodes(
    node: &Fiber,
    r#fn: &ReactScopeQuery,
    scoped_nodes: &mut Vec<&'static Unknown>,
) {
    if ENABLE_SCOPE_API {
        if node.tag.get() == HOST_COMPONENT {
            let r#type = node.r#type.get();
            let memoized_props = node.memoized_props.get();
            let state_node = node.state_node.get();
            let instance = get_public_instance(state_node);
            if let Some(instance) = instance
                && js::strict_equals(
                    r#fn(
                        r#type,
                        memoized_props
                            .filter(|memoized_props| js::truthy(Some(*memoized_props)))
                            .or_else(|| {
                                EMPTY_OBJECT.with(|empty_object| Some(js::unknown(*empty_object)))
                            }),
                        instance,
                    ),
                    true,
                )
            {
                scoped_nodes.push(instance);
            }
        }
        let mut child = node.child.borrow().clone();

        if is_fiber_suspense_and_timed_out(node) {
            child = get_suspense_fallback_child(node);
        }
        if let Some(child) = child {
            collect_scoped_nodes_from_children(&child, r#fn, scoped_nodes);
        }
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn collect_first_scoped_node(node: &Fiber, r#fn: &ReactScopeQuery) -> Option<&'static Unknown> {
    if ENABLE_SCOPE_API {
        if node.tag.get() == HOST_COMPONENT {
            let r#type = node.r#type.get();
            let memoized_props = node.memoized_props.get();
            let state_node = node.state_node.get();
            let instance = get_public_instance(state_node);
            if let Some(instance) = instance
                && js::strict_equals(r#fn(r#type, memoized_props, instance), true)
            {
                return Some(instance);
            }
        }
        let mut child = node.child.borrow().clone();

        if is_fiber_suspense_and_timed_out(node) {
            child = get_suspense_fallback_child(node);
        }
        if let Some(child) = child {
            return collect_first_scoped_node_from_children(&child, r#fn);
        }
    }
    None
}

fn collect_scoped_nodes_from_children(
    starting_child: &Rc<Fiber>,
    r#fn: &ReactScopeQuery,
    scoped_nodes: &mut Vec<&'static Unknown>,
) {
    let mut child = Some(Rc::clone(starting_child));
    while let Some(current) = child {
        collect_scoped_nodes(&current, r#fn, scoped_nodes);
        child = current.sibling.borrow().clone();
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn collect_first_scoped_node_from_children(
    starting_child: &Rc<Fiber>,
    r#fn: &ReactScopeQuery,
) -> Option<&'static Unknown> {
    let mut child = Some(Rc::clone(starting_child));
    while let Some(current) = child {
        let scoped_node = collect_first_scoped_node(&current, r#fn);
        if scoped_node.is_some() {
            return scoped_node;
        }
        child = current.sibling.borrow().clone();
    }
    None
}

// shared/ReactTypes' `ReactContext<T>`, and its values.
fn collect_nearest_context_values(
    node: &Fiber,
    context: &'static Unknown,
    child_context_values: &mut Vec<Option<&'static Unknown>>,
) {
    if node.tag.get() == CONTEXT_PROVIDER && js::strict_equals(node.r#type.get(), context) {
        let context_value = js::get(node.memoized_props.get(), "value");
        child_context_values.push(context_value);
    } else {
        let mut child = node.child.borrow().clone();

        if is_fiber_suspense_and_timed_out(node) {
            child = get_suspense_fallback_child(node);
        }
        if child.is_some() {
            collect_nearest_child_context_values(child, context, child_context_values);
        }
    }
}

fn collect_nearest_child_context_values(
    starting_child: Option<Rc<Fiber>>,
    context: &'static Unknown,
    child_context_values: &mut Vec<Option<&'static Unknown>>,
) {
    let mut child = starting_child;
    while let Some(current) = child {
        collect_nearest_context_values(&current, context, child_context_values);
        child = current.sibling.borrow().clone();
    }
}

/// shared/ReactTypes' `ReactScopeInstance`: a Scope's instance, which
/// queries the host instances and context values inside it.
#[cfg_attr(rust_js, rust_js::object)]
pub struct ReactScopeInstance {}

unsafe impl js::Defined for ReactScopeInstance {}

#[cfg_attr(rust_js, rust_js::object)]
impl ReactScopeInstance {
    #[cfg_attr(rust_js, rust_js::name = "DO_NOT_USE_queryAllNodes")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub fn do_not_use_query_all_nodes(
        &self,
        r#fn: &ReactScopeQuery,
    ) -> Option<Vec<&'static Unknown>> {
        let current_fiber = get_instance_from_scope(js::unknown(self))?;
        let child = current_fiber.child.borrow().clone();
        let mut scoped_nodes = Vec::new();
        if let Some(child) = child {
            collect_scoped_nodes_from_children(&child, r#fn, &mut scoped_nodes);
        }
        if scoped_nodes.is_empty() {
            None
        } else {
            Some(scoped_nodes)
        }
    }

    #[cfg_attr(rust_js, rust_js::name = "DO_NOT_USE_queryFirstNode")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub fn do_not_use_query_first_node(&self, r#fn: &ReactScopeQuery) -> Option<&'static Unknown> {
        let current_fiber = get_instance_from_scope(js::unknown(self))?;
        let child = current_fiber.child.borrow().clone();
        if let Some(child) = child {
            return collect_first_scoped_node_from_children(&child, r#fn);
        }
        None
    }

    pub fn contains_node(&self, node: &Unknown) -> bool {
        let mut fiber = get_instance_from_node(node);
        while let Some(current) = fiber {
            if current.tag.get() == SCOPE_COMPONENT
                && js::strict_equals(current.state_node.get(), js::unknown(self))
            {
                return true;
            }
            fiber = current.r#return.borrow().clone();
        }
        false
    }

    pub fn get_child_context_values(
        &self,
        context: &'static Unknown,
    ) -> Vec<Option<&'static Unknown>> {
        let current_fiber = get_instance_from_scope(js::unknown(self));
        let Some(current_fiber) = current_fiber else {
            return Vec::new();
        };
        let child = current_fiber.child.borrow().clone();
        let mut child_context_values = Vec::new();
        if child.is_some() {
            collect_nearest_child_context_values(child, context, &mut child_context_values);
        }
        child_context_values
    }
}

pub fn create_scope_instance() -> ReactScopeInstance {
    ReactScopeInstance {}
}
