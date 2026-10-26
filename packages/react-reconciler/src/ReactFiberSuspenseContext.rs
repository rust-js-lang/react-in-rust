// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::RefCell;
use std::rc::Rc;

use shared::react_feature_flags::ENABLE_SUSPENSE_AVOID_THIS_FALLBACK;

use crate::react_fiber_hidden_context::is_current_tree_hidden;
use crate::react_fiber_stack::{StackCursor, create_cursor, pop, push};
use crate::react_internal_types::Fiber;
use crate::react_work_tags::OFFSCREEN_COMPONENT;

thread_local! {
    // The Suspense handler is the boundary that should capture if something
    // suspends, i.e. it's the nearest `catch` block on the stack.
    #[cfg_attr(rust_js, rust_js::nullable)]
    static SUSPENSE_HANDLER_STACK_CURSOR: StackCursor<Option<Rc<Fiber>>> = create_cursor(None);

    // Represents the outermost boundary that is not visible in the current tree.
    // Everything above this is the "shell". When this is null, it means we're
    // rendering in the shell of the app. If it's non-null, it means we're rendering
    // deeper than the shell, inside a new tree that wasn't already visible.
    //
    // The main way we use this concept is to determine whether showing a fallback
    // would result in a desirable or undesirable loading state. Activing a fallback
    // in the shell is considered an undersirable loading state, because it would
    // mean hiding visible (albeit stale) content in the current tree — we prefer to
    // show the stale content, rather than switch to a fallback. But showing a
    // fallback in a new tree is fine, because there's no stale content to
    // prefer instead.
    #[cfg_attr(rust_js, rust_js::nullable)]
    static SHELL_BOUNDARY: RefCell<Option<Rc<Fiber>>> = const { RefCell::new(None) };
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_shell_boundary() -> Option<Rc<Fiber>> {
    SHELL_BOUNDARY.with_borrow(|shell_boundary| shell_boundary.clone())
}

pub fn push_primary_tree_suspense_handler(handler: &Rc<Fiber>) {
    // TODO: Pass as argument
    let current = handler.alternate.borrow().clone();
    // shared/ReactTypes' `SuspenseProps`.
    let props = handler.pending_props.get();

    // Shallow Suspense context fields, like ForceSuspenseFallback, should only be
    // propagated a single level. For example, when ForceSuspenseFallback is set,
    // it should only force the nearest Suspense boundary into fallback mode.
    SUSPENSE_STACK_CURSOR.with(|cursor| {
        let suspense_context = *cursor.current.borrow();
        push(
            cursor,
            set_default_shallow_suspense_list_context(suspense_context),
            handler,
        );
    });

    // Experimental feature: Some Suspense boundaries are marked as having an
    // undesirable fallback state. These have special behavior where we only
    // activate the fallback if there's no other boundary on the stack that we can
    // use instead.
    if ENABLE_SUSPENSE_AVOID_THIS_FALLBACK
        && js::strict_equals(props.and_then(|props| js::get(props, "unstable_avoidThisFallback")), true)
        // If an avoided boundary is already visible, it behaves identically to
        // a regular Suspense boundary.
        && (current.is_none() || is_current_tree_hidden())
    {
        if SHELL_BOUNDARY.with_borrow(Option::is_none) {
            // We're rendering in the shell. There's no parent Suspense boundary that
            // can provide a desirable fallback state. We'll use this boundary.
            SUSPENSE_HANDLER_STACK_CURSOR
                .with(|cursor| push(cursor, Some(Rc::clone(handler)), handler));

            // However, because this is not a desirable fallback, the children are
            // still considered part of the shell. So we intentionally don't assign
            // to `shellBoundary`.
        } else {
            // There's already a parent Suspense boundary that can provide a desirable
            // fallback state. Prefer that one.
            SUSPENSE_HANDLER_STACK_CURSOR.with(|cursor| {
                let handler_on_stack = cursor.current.borrow().clone();
                push(cursor, handler_on_stack, handler);
            });
        }
        return;
    }

    // TODO: If the parent Suspense handler already suspended, there's no reason
    // to push a nested Suspense handler, because it will get replaced by the
    // outer fallback, anyway. Consider this as a future optimization.
    SUSPENSE_HANDLER_STACK_CURSOR.with(|cursor| push(cursor, Some(Rc::clone(handler)), handler));
    if SHELL_BOUNDARY.with_borrow(Option::is_none) {
        if current.is_none() || is_current_tree_hidden() {
            // This boundary is not visible in the current UI.
            SHELL_BOUNDARY.set(Some(Rc::clone(handler)));
        } else if let Some(current) = current {
            // ReactFiberSuspenseComponent's `SuspenseState`.
            let prev_state = current.memoized_state.get();
            // $FlowFixMe[invalid-compare]
            if prev_state.is_some() {
                // This boundary is showing a fallback in the current UI.
                SHELL_BOUNDARY.set(Some(Rc::clone(handler)));
            }
        }
    }
}

pub fn push_fallback_tree_suspense_handler(fiber: &Rc<Fiber>) {
    // We're about to render the fallback. If something in the fallback suspends,
    // it's akin to throwing inside of a `catch` block. This boundary should not
    // capture. Reuse the existing handler on the stack.
    reuse_suspense_handler_on_stack(fiber);
}

pub fn push_dehydrated_activity_suspense_handler(fiber: &Rc<Fiber>) {
    // This is called when hydrating an Activity boundary. We can just leave it
    // dehydrated if it suspends.
    // A SuspenseList context is only pushed here to avoid a push/pop mismatch.
    // Reuse the current value on the stack.
    // TODO: We can avoid needing to push here by by forking popSuspenseHandler
    // into separate functions for Activity, Suspense and Offscreen.
    SUSPENSE_STACK_CURSOR.with(|cursor| {
        let suspense_context = *cursor.current.borrow();
        push(cursor, suspense_context, fiber);
    });
    SUSPENSE_HANDLER_STACK_CURSOR.with(|cursor| push(cursor, Some(Rc::clone(fiber)), fiber));
    if SHELL_BOUNDARY.with_borrow(Option::is_none) {
        // We can contain any suspense inside the Activity boundary.
        SHELL_BOUNDARY.set(Some(Rc::clone(fiber)));
    }
}

pub fn push_offscreen_suspense_handler(fiber: &Rc<Fiber>) {
    if fiber.tag.get() == OFFSCREEN_COMPONENT {
        // A SuspenseList context is only pushed here to avoid a push/pop mismatch.
        // Reuse the current value on the stack.
        // TODO: We can avoid needing to push here by by forking popSuspenseHandler
        // into separate functions for Activity, Suspense and Offscreen.
        SUSPENSE_STACK_CURSOR.with(|cursor| {
            let suspense_context = *cursor.current.borrow();
            push(cursor, suspense_context, fiber);
        });
        SUSPENSE_HANDLER_STACK_CURSOR.with(|cursor| push(cursor, Some(Rc::clone(fiber)), fiber));
        if SHELL_BOUNDARY.with_borrow(Option::is_none) {
            // We're rendering hidden content. If it suspends, we can handle it by
            // just not committing the offscreen boundary.
            SHELL_BOUNDARY.set(Some(Rc::clone(fiber)));
        }
    } else {
        // This is a LegacyHidden component.
        reuse_suspense_handler_on_stack(fiber);
    }
}

pub fn reuse_suspense_handler_on_stack(fiber: &Rc<Fiber>) {
    SUSPENSE_STACK_CURSOR.with(|cursor| {
        let suspense_context = *cursor.current.borrow();
        push(cursor, suspense_context, fiber);
    });
    SUSPENSE_HANDLER_STACK_CURSOR.with(|cursor| push(cursor, get_suspense_handler(), fiber));
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_suspense_handler() -> Option<Rc<Fiber>> {
    SUSPENSE_HANDLER_STACK_CURSOR.with(|cursor| cursor.current.borrow().clone())
}

pub fn pop_suspense_handler(fiber: &Fiber) {
    SUSPENSE_HANDLER_STACK_CURSOR.with(|cursor| pop(cursor, fiber));
    if SHELL_BOUNDARY.with_borrow(|shell_boundary| shell_boundary.as_deref() == Some(fiber)) {
        // Popping back into the shell.
        SHELL_BOUNDARY.set(None);
    }
    SUSPENSE_STACK_CURSOR.with(|cursor| pop(cursor, fiber));
}

// SuspenseList context
// TODO: Move to a separate module? We may change the SuspenseList
// implementation to hide/show in the commit phase, anyway.
pub type SuspenseContext = i32;
pub type SubtreeSuspenseContext = SuspenseContext;
pub type ShallowSuspenseContext = SuspenseContext;

#[cfg_attr(rust_js, rust_js::name = "DefaultSuspenseContext")]
const DEFAULT_SUSPENSE_CONTEXT: SuspenseContext = 0b00;

#[cfg_attr(rust_js, rust_js::name = "SubtreeSuspenseContextMask")]
const SUBTREE_SUSPENSE_CONTEXT_MASK: SuspenseContext = 0b01;

// ForceSuspenseFallback can be used by SuspenseList to force newly added
// items into their fallback state during one of the render passes.
#[cfg_attr(rust_js, rust_js::name = "ForceSuspenseFallback")]
pub const FORCE_SUSPENSE_FALLBACK: ShallowSuspenseContext = 0b10;

thread_local! {
    pub static SUSPENSE_STACK_CURSOR: StackCursor<SuspenseContext> =
        create_cursor(DEFAULT_SUSPENSE_CONTEXT);
}

pub fn has_suspense_list_context(parent_context: SuspenseContext, flag: SuspenseContext) -> bool {
    (parent_context & flag) != 0
}

pub fn set_default_shallow_suspense_list_context(
    parent_context: SuspenseContext,
) -> SuspenseContext {
    parent_context & SUBTREE_SUSPENSE_CONTEXT_MASK
}

pub fn set_shallow_suspense_list_context(
    parent_context: SuspenseContext,
    shallow_context: ShallowSuspenseContext,
) -> SuspenseContext {
    (parent_context & SUBTREE_SUSPENSE_CONTEXT_MASK) | shallow_context
}

pub fn push_suspense_list_context(fiber: &Rc<Fiber>, new_context: SuspenseContext) {
    // Push the current handler in this case since we're not catching at the SuspenseList
    // for typical rows.
    SUSPENSE_HANDLER_STACK_CURSOR.with(|cursor| {
        let handler_on_stack = cursor.current.borrow().clone();
        push(cursor, handler_on_stack, fiber);
    });
    SUSPENSE_STACK_CURSOR.with(|cursor| push(cursor, new_context, fiber));
}

pub fn push_suspense_list_catch(fiber: &Rc<Fiber>, new_context: SuspenseContext) {
    // In this case we do want to handle catching suspending on the actual boundary itself.
    // This is used for rows that are allowed to be hidden anyway.
    SUSPENSE_HANDLER_STACK_CURSOR.with(|cursor| push(cursor, Some(Rc::clone(fiber)), fiber));
    SUSPENSE_STACK_CURSOR.with(|cursor| push(cursor, new_context, fiber));
    if SHELL_BOUNDARY.with_borrow(Option::is_none) {
        // We can contain the effects to hiding the current row.
        SHELL_BOUNDARY.set(Some(Rc::clone(fiber)));
    }
}

pub fn pop_suspense_list_context(fiber: &Fiber) {
    SUSPENSE_STACK_CURSOR.with(|cursor| pop(cursor, fiber));
    SUSPENSE_HANDLER_STACK_CURSOR.with(|cursor| pop(cursor, fiber));
    if SHELL_BOUNDARY.with_borrow(|shell_boundary| shell_boundary.as_deref() == Some(fiber)) {
        // Popping back into the shell.
        SHELL_BOUNDARY.set(None);
    }
}
