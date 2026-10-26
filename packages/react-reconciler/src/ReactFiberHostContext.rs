// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::rc::Rc;

use js::Unknown;
use react_isomorphic::react_context::ReactContext;
use shared::environment::__DEV__;
use webapi::console;

use crate::react_fiber_stack::{StackCursor, create_cursor, pop, push};
use crate::react_internal_types::Fiber;

unsafe extern "Rust" {
    #[link_name = "./src/ReactFiberConfig.js#isPrimaryRenderer"]
    safe static IS_PRIMARY_RENDERER: bool;
    // ReactFiberConfig's `ReactContext<TransitionStatus>`.
    #[link_name = "./src/ReactFiberConfig.js#HostTransitionContext"]
    safe static HOST_TRANSITION_CONTEXT: ReactContext<Option<&'static Unknown>>;
    // ReactFiberConfig's `TransitionStatus`.
    #[link_name = "./src/ReactFiberConfig.js#NotPendingTransition"]
    safe static NOT_PENDING_TRANSITION: Option<&'static Unknown>;
}

// ReactFiberConfig's `HostContext`s.
#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#getChildHostContext"
)]
fn get_child_host_context(
    parent_host_context: Option<&'static Unknown>,
    r#type: Option<&'static Unknown>,
) -> Option<&'static Unknown> {
    unreachable!()
}

// ReactFiberConfig's `HostContext` of a `Container`.
#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberConfig.js#getRootHostContext"
)]
fn get_root_host_context(root_container_instance: &'static Unknown) -> Option<&'static Unknown> {
    unreachable!()
}

thread_local! {
    // ReactFiberConfig's `HostContext`.
    #[cfg_attr(rust_js, rust_js::nullable)]
    static CONTEXT_STACK_CURSOR: StackCursor<Option<&'static Unknown>> = create_cursor(None);
    #[cfg_attr(rust_js, rust_js::nullable)]
    static CONTEXT_FIBER_STACK_CURSOR: StackCursor<Option<Rc<Fiber>>> = create_cursor(None);
    // ReactFiberConfig's `Container`.
    #[cfg_attr(rust_js, rust_js::nullable)]
    static ROOT_INSTANCE_STACK_CURSOR: StackCursor<Option<&'static Unknown>> = create_cursor(None);

    // Represents the nearest host transition provider (in React DOM, a <form />)
    // NOTE: Since forms cannot be nested, and this feature is only implemented by
    // React DOM, we don't technically need this to be a stack. It could be a single
    // module variable instead.
    #[cfg_attr(rust_js, rust_js::nullable)]
    static HOST_TRANSITION_PROVIDER_CURSOR: StackCursor<Option<Rc<Fiber>>> = create_cursor(None);
}

fn required_context<Value>(c: Option<Value>) -> Option<Value> {
    if __DEV__ && c.is_none() {
        console::error((concat!(
            "Expected host context to exist. This error is likely caused by a bug ",
            "in React. Please file an issue."
        ),));
    }
    c
}

// ReactFiberConfig's `Container`.
#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_current_root_host_container() -> Option<&'static Unknown> {
    ROOT_INSTANCE_STACK_CURSOR.with(|cursor| *cursor.current.borrow())
}

// ReactFiberConfig's `Container`.
pub fn get_root_host_container() -> Option<&'static Unknown> {
    required_context(ROOT_INSTANCE_STACK_CURSOR.with(|cursor| *cursor.current.borrow()))
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_host_transition_provider() -> Option<Rc<Fiber>> {
    HOST_TRANSITION_PROVIDER_CURSOR.with(|cursor| cursor.current.borrow().clone())
}

pub fn push_host_container(fiber: &Rc<Fiber>, next_root_instance: &'static Unknown) {
    // Push current root instance onto the stack;
    // This allows us to reset root when portals are popped.
    ROOT_INSTANCE_STACK_CURSOR.with(|cursor| push(cursor, Some(next_root_instance), fiber));
    // Track the context and the Fiber that provided it.
    // This enables us to pop only Fibers that provide unique contexts.
    CONTEXT_FIBER_STACK_CURSOR.with(|cursor| push(cursor, Some(Rc::clone(fiber)), fiber));

    // Finally, we need to push the host context to the stack.
    // However, we can't just call getRootHostContext() and push it because
    // we'd have a different number of entries on the stack depending on
    // whether getRootHostContext() throws somewhere in renderer code or not.
    // So we push an empty value first. This lets us safely unwind on errors.
    CONTEXT_STACK_CURSOR.with(|cursor| push(cursor, None, fiber));
    let next_root_context = get_root_host_context(next_root_instance);
    // Now that we know this function doesn't throw, replace it.
    CONTEXT_STACK_CURSOR.with(|cursor| pop(cursor, fiber));
    CONTEXT_STACK_CURSOR.with(|cursor| push(cursor, next_root_context, fiber));
}

pub fn pop_host_container(fiber: &Fiber) {
    CONTEXT_STACK_CURSOR.with(|cursor| pop(cursor, fiber));
    CONTEXT_FIBER_STACK_CURSOR.with(|cursor| pop(cursor, fiber));
    ROOT_INSTANCE_STACK_CURSOR.with(|cursor| pop(cursor, fiber));
}

// ReactFiberConfig's `HostContext`.
pub fn get_host_context() -> Option<&'static Unknown> {
    required_context(CONTEXT_STACK_CURSOR.with(|cursor| *cursor.current.borrow()))
}

pub fn push_host_context(fiber: &Rc<Fiber>) {
    // ReactFiberHooks' `Hook`.
    let state_hook = fiber.memoized_state.get();
    if let Some(state_hook) = state_hook {
        // Propagate the current state to all the descendents.
        // We use Context as an implementation detail for this.
        //
        // NOTE: This assumes that there cannot be nested transition providers,
        // because the only renderer that implements this feature is React DOM,
        // and forms cannot be nested. If we did support nested providers, then
        // we would need to push a context value even for host fibers that
        // haven't been upgraded yet.
        // ReactFiberConfig's `TransitionStatus`.
        let transition_status = js::get(state_hook, "memoizedState");
        // $FlowFixMe[constant-condition]
        if IS_PRIMARY_RENDERER {
            *HOST_TRANSITION_CONTEXT.current_value.borrow_mut() = transition_status;
        } else {
            *HOST_TRANSITION_CONTEXT.current_value2.borrow_mut() = transition_status;
        }

        // Only provide context if this fiber has been upgraded by a host
        // transition. We use the same optimization for regular host context below.
        HOST_TRANSITION_PROVIDER_CURSOR.with(|cursor| push(cursor, Some(Rc::clone(fiber)), fiber));
    }

    // ReactFiberConfig's `HostContext`.
    let context = required_context(CONTEXT_STACK_CURSOR.with(|cursor| *cursor.current.borrow()));
    let next_context = get_child_host_context(context, fiber.r#type.get());

    // Don't push this Fiber's context unless it's unique.
    if !js::strict_equals(context, next_context) {
        // Track the context and the Fiber that provided it.
        // This enables us to pop only Fibers that provide unique contexts.
        CONTEXT_FIBER_STACK_CURSOR.with(|cursor| push(cursor, Some(Rc::clone(fiber)), fiber));
        CONTEXT_STACK_CURSOR.with(|cursor| push(cursor, next_context, fiber));
    }
}

pub fn pop_host_context(fiber: &Fiber) {
    if CONTEXT_FIBER_STACK_CURSOR.with(|cursor| cursor.current.borrow().as_deref() == Some(fiber)) {
        // Do not pop unless this Fiber provided the current context.
        // pushHostContext() only pushes Fibers that provide unique contexts.
        CONTEXT_STACK_CURSOR.with(|cursor| pop(cursor, fiber));
        CONTEXT_FIBER_STACK_CURSOR.with(|cursor| pop(cursor, fiber));
    }

    if HOST_TRANSITION_PROVIDER_CURSOR
        .with(|cursor| cursor.current.borrow().as_deref() == Some(fiber))
    {
        // Do not pop unless this Fiber provided the current context. This is mostly
        // a performance optimization, but conveniently it also prevents a potential
        // data race where a host provider is upgraded (i.e. memoizedState becomes
        // non-null) during a concurrent event. This is a bit of a flaw in the way
        // we upgrade host components, but because we're accounting for it here, it
        // should be fine.
        HOST_TRANSITION_PROVIDER_CURSOR.with(|cursor| pop(cursor, fiber));

        // When popping the transition provider, we reset the context value back
        // to `NotPendingTransition`. We can do this because you're not allowed to nest forms. If
        // we allowed for multiple nested host transition providers, then we'd
        // need to reset this to the parent provider's status.
        // $FlowFixMe[constant-condition]
        if IS_PRIMARY_RENDERER {
            *HOST_TRANSITION_CONTEXT.current_value.borrow_mut() = NOT_PENDING_TRANSITION;
        } else {
            *HOST_TRANSITION_CONTEXT.current_value2.borrow_mut() = NOT_PENDING_TRANSITION;
        }
    }
}
