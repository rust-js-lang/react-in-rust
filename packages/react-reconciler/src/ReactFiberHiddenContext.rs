// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::RefCell;
use std::rc::Rc;

use crate::react_fiber_lane::{Lanes, NO_LANES, merge_lanes};
use crate::react_internal_types::Fiber;

/// ReactFiberStack's `StackCursor`, here until that module is ported.
pub struct StackCursor<T> {
    pub current: RefCell<T>,
}

#[cfg_attr(rust_js, rust_js::link_name = "./src/ReactFiberStack.js#createCursor")]
fn create_cursor<T>(default_value: T) -> StackCursor<T> {
    unreachable!()
}

#[cfg_attr(rust_js, rust_js::link_name = "./src/ReactFiberStack.js#push")]
fn push<T>(cursor: &StackCursor<T>, value: T, fiber: &Rc<Fiber>) {
    unreachable!()
}

#[cfg_attr(rust_js, rust_js::link_name = "./src/ReactFiberStack.js#pop")]
fn pop<T>(cursor: &StackCursor<T>, fiber: &Fiber) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#getEntangledRenderLanes"
)]
fn get_entangled_render_lanes() -> Lanes {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFiberWorkLoop.js#setEntangledRenderLanes"
)]
fn set_entangled_render_lanes(new_entangled_render_lanes: Lanes) {
    unreachable!()
}

// TODO: Remove `renderLanes` context in favor of hidden context
pub struct HiddenContext {
    // Represents the lanes that must be included when processing updates in
    // order to reveal the hidden content.
    // TODO: Remove `subtreeLanes` context from work loop in favor of this one.
    pub base_lanes: Lanes,
}

thread_local! {
    // TODO: This isn't being used yet, but it's intended to replace the
    // InvisibleParentContext that is currently managed by SuspenseContext.
    pub static CURRENT_TREE_HIDDEN_STACK_CURSOR: StackCursor<Option<Rc<HiddenContext>>> =
        create_cursor(None);
    pub static PREV_ENTANGLED_RENDER_LANES_CURSOR: StackCursor<Lanes> = create_cursor(NO_LANES);
}

pub fn push_hidden_context(fiber: &Rc<Fiber>, context: Rc<HiddenContext>) {
    let prev_entangled_render_lanes = get_entangled_render_lanes();
    PREV_ENTANGLED_RENDER_LANES_CURSOR
        .with(|cursor| push(cursor, prev_entangled_render_lanes, fiber));
    CURRENT_TREE_HIDDEN_STACK_CURSOR.with(|cursor| push(cursor, Some(Rc::clone(&context)), fiber));

    // When rendering a subtree that's currently hidden, we must include all
    // lanes that would have rendered if the hidden subtree hadn't been deferred.
    // That is, in order to reveal content from hidden -> visible, we must commit
    // all the updates that we skipped when we originally hid the tree.
    set_entangled_render_lanes(merge_lanes(prev_entangled_render_lanes, context.base_lanes));
}

pub fn reuse_hidden_context_on_stack(fiber: &Rc<Fiber>) {
    // This subtree is not currently hidden, so we don't need to add any lanes
    // to the render lanes. But we still need to push something to avoid a
    // context mismatch. Reuse the existing context on the stack.
    PREV_ENTANGLED_RENDER_LANES_CURSOR
        .with(|cursor| push(cursor, get_entangled_render_lanes(), fiber));
    CURRENT_TREE_HIDDEN_STACK_CURSOR.with(|cursor| {
        let current = cursor.current.borrow().clone();
        push(cursor, current, fiber);
    });
}

pub fn pop_hidden_context(fiber: &Fiber) {
    // Restore the previous render lanes from the stack
    set_entangled_render_lanes(
        PREV_ENTANGLED_RENDER_LANES_CURSOR.with(|cursor| *cursor.current.borrow()),
    );

    CURRENT_TREE_HIDDEN_STACK_CURSOR.with(|cursor| pop(cursor, fiber));
    PREV_ENTANGLED_RENDER_LANES_CURSOR.with(|cursor| pop(cursor, fiber));
}

pub fn is_current_tree_hidden() -> bool {
    CURRENT_TREE_HIDDEN_STACK_CURSOR.with(|cursor| cursor.current.borrow().is_some())
}
