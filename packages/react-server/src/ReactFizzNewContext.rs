// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::RefCell;
use std::rc::Rc;

use js::Unknown;
use shared::environment::__DEV__;
use shared::react_types::ReactContext;
use webapi::console;

unsafe extern "Rust" {
    #[link_name = "./src/ReactFizzConfig.js#isPrimaryRenderer"]
    safe static IS_PRIMARY_RENDERER: bool;
}

thread_local! {
    static RENDERER_SIGIL: Option<&'static Unknown> = if __DEV__ {
        // Use this to detect multiple renderers using the same context
        Some(js::unknown(js::dict::new::<Unknown>()))
    } else {
        None
    };
}

/// A context Fizz renders, of any value.
pub type Context = ReactContext<Option<&'static Unknown>>;

// Used to store the parent path of all context overrides in a shared linked list.
// Forming a reverse tree.
pub struct ContextNode {
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub parent: Option<Rc<ContextNode>>,
    pub depth: f64, // Short hand to compute the depth of the tree at this node.
    pub context: &'static Context,
    pub parent_value: Option<&'static Unknown>,
    pub value: Option<&'static Unknown>,
}

impl PartialEq for ContextNode {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

// The structure of a context snapshot is an implementation of this file.
// Currently, it's implemented as tracking the current active node.
pub type ContextSnapshot = Option<Rc<ContextNode>>;

thread_local! {
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static ROOT_CONTEXT_SNAPSHOT: ContextSnapshot = const { None };

    // We assume that this runtime owns the "current" field on all ReactContext instances.
    // This global (actually thread local) state represents what state all those "current",
    // fields are currently in.
    #[cfg_attr(rust_js, rust_js::nullable)]
    static CURRENT_ACTIVE_SNAPSHOT: RefCell<ContextSnapshot> = const { RefCell::new(None) };
}

fn pop_node(prev: &ContextNode) {
    if IS_PRIMARY_RENDERER {
        *prev.context.current_value.borrow_mut() = prev.parent_value;
    } else {
        *prev.context.current_value2.borrow_mut() = prev.parent_value;
    }
}

fn push_node(next: &ContextNode) {
    if IS_PRIMARY_RENDERER {
        *next.context.current_value.borrow_mut() = next.value;
    } else {
        *next.context.current_value2.borrow_mut() = next.value;
    }
}

fn pop_to_nearest_common_ancestor(prev: &ContextNode, next: &ContextNode) {
    if prev == next {
        // We've found a shared ancestor. We don't need to pop nor reapply this one or anything above.
    } else {
        pop_node(prev);
        let parent_prev = &prev.parent;
        let parent_next = &next.parent;
        match parent_prev {
            None => {
                if parent_next.is_some() {
                    panic!(
                        "The stacks must reach the root at the same time. This is a bug in React."
                    );
                }
            }
            Some(parent_prev) => {
                let Some(parent_next) = parent_next else {
                    panic!(
                        "The stacks must reach the root at the same time. This is a bug in React."
                    );
                };

                pop_to_nearest_common_ancestor(parent_prev, parent_next);
            }
        }

        // On the way back, we push the new ones that weren't common.
        push_node(next);
    }
}

fn pop_all_previous(prev: &ContextNode) {
    pop_node(prev);
    if let Some(parent_prev) = &prev.parent {
        pop_all_previous(parent_prev);
    }
}

fn push_all_next(next: &ContextNode) {
    if let Some(parent_next) = &next.parent {
        push_all_next(parent_next);
    }
    push_node(next);
}

fn pop_previous_to_common_level(prev: &ContextNode, next: &ContextNode) {
    pop_node(prev);
    let Some(parent_prev) = &prev.parent else {
        panic!(
            "The depth must equal at least at zero before reaching the root. This is a bug in React."
        );
    };

    if parent_prev.depth == next.depth {
        // We found the same level. Now we just need to find a shared ancestor.
        pop_to_nearest_common_ancestor(parent_prev, next);
    } else {
        // We must still be deeper.
        pop_previous_to_common_level(parent_prev, next);
    }
}

fn pop_next_to_common_level(prev: &ContextNode, next: &ContextNode) {
    let Some(parent_next) = &next.parent else {
        panic!(
            "The depth must equal at least at zero before reaching the root. This is a bug in React."
        );
    };

    if prev.depth == parent_next.depth {
        // We found the same level. Now we just need to find a shared ancestor.
        pop_to_nearest_common_ancestor(prev, parent_next);
    } else {
        // We must still be deeper.
        pop_next_to_common_level(prev, parent_next);
    }
    push_node(next);
}

// Perform context switching to the new snapshot.
// To make it cheap to read many contexts, while not suspending, we make the switch eagerly by
// updating all the context's current values. That way reads, always just read the current value.
// At the cost of updating contexts even if they're never read by this subtree.
pub fn switch_context(new_snapshot: ContextSnapshot) {
    // The basic algorithm we need to do is to pop back any contexts that are no longer on the stack.
    // We also need to update any new contexts that are now on the stack with the deepest value.
    // The easiest way to update new contexts is to just reapply them in reverse order from the
    // perspective of the backpointers. To avoid allocating a lot when switching, we use the stack
    // for that. Therefore this algorithm is recursive.
    // 1) First we pop which ever snapshot tree was deepest. Popping old contexts as we go.
    // 2) Then we find the nearest common ancestor from there. Popping old contexts as we go.
    // 3) Then we reapply new contexts on the way back up the stack.
    let prev = CURRENT_ACTIVE_SNAPSHOT.with_borrow(Clone::clone);
    let next = new_snapshot;
    if prev != next {
        match (&prev, &next) {
            (None, Some(next)) => push_all_next(next),
            (Some(prev), None) => pop_all_previous(prev),
            (Some(prev), Some(next)) if prev.depth == next.depth => {
                pop_to_nearest_common_ancestor(prev, next);
            }
            (Some(prev), Some(next)) if prev.depth > next.depth => {
                pop_previous_to_common_level(prev, next);
            }
            (Some(prev), Some(next)) => pop_next_to_common_level(prev, next),
            // Not equal, so not both the root.
            (None, None) => {}
        }
        CURRENT_ACTIVE_SNAPSHOT.set(next);
    }
}

pub fn push_provider(
    context: &'static Context,
    next_value: Option<&'static Unknown>,
) -> ContextSnapshot {
    let prev_value;
    if IS_PRIMARY_RENDERER {
        prev_value = *context.current_value.borrow();
        *context.current_value.borrow_mut() = next_value;
        if __DEV__ {
            if context.current_renderer.get().is_some()
                && !js::strict_equals(
                    context.current_renderer.get(),
                    RENDERER_SIGIL.with(|&sigil| sigil),
                )
            {
                console::error((concat!(
                    "Detected multiple renderers concurrently rendering the ",
                    "same context provider. This is currently unsupported."
                ),));
            }
            context
                .current_renderer
                .set(RENDERER_SIGIL.with(|&sigil| sigil));
        }
    } else {
        prev_value = *context.current_value2.borrow();
        *context.current_value2.borrow_mut() = next_value;
        if __DEV__ {
            if context.current_renderer2.get().is_some()
                && !js::strict_equals(
                    context.current_renderer2.get(),
                    RENDERER_SIGIL.with(|&sigil| sigil),
                )
            {
                console::error((concat!(
                    "Detected multiple renderers concurrently rendering the ",
                    "same context provider. This is currently unsupported."
                ),));
            }
            context
                .current_renderer2
                .set(RENDERER_SIGIL.with(|&sigil| sigil));
        }
    }
    let prev_node = CURRENT_ACTIVE_SNAPSHOT.with_borrow(Clone::clone);
    let new_node = Rc::new(ContextNode {
        parent: prev_node.clone(),
        depth: match &prev_node {
            None => 0.0,
            Some(prev_node) => prev_node.depth + 1.0,
        },
        context,
        parent_value: prev_value,
        value: next_value,
    });
    CURRENT_ACTIVE_SNAPSHOT.set(Some(Rc::clone(&new_node)));
    Some(new_node)
}

pub fn pop_provider(context: &'static Context) -> ContextSnapshot {
    let Some(prev_snapshot) = CURRENT_ACTIVE_SNAPSHOT.with_borrow(Clone::clone) else {
        panic!("Tried to pop a Context at the root of the app. This is a bug in React.");
    };

    if __DEV__ && !std::ptr::eq(prev_snapshot.context, context) {
        console::error((
            "The parent context is not the expected context. This is probably a bug in React.",
        ));
    }
    if IS_PRIMARY_RENDERER {
        let value = prev_snapshot.parent_value;
        *prev_snapshot.context.current_value.borrow_mut() = value;
        if __DEV__ {
            if context.current_renderer.get().is_some()
                && !js::strict_equals(
                    context.current_renderer.get(),
                    RENDERER_SIGIL.with(|&sigil| sigil),
                )
            {
                console::error((concat!(
                    "Detected multiple renderers concurrently rendering the ",
                    "same context provider. This is currently unsupported."
                ),));
            }
            context
                .current_renderer
                .set(RENDERER_SIGIL.with(|&sigil| sigil));
        }
    } else {
        let value = prev_snapshot.parent_value;
        *prev_snapshot.context.current_value2.borrow_mut() = value;
        if __DEV__ {
            if context.current_renderer2.get().is_some()
                && !js::strict_equals(
                    context.current_renderer2.get(),
                    RENDERER_SIGIL.with(|&sigil| sigil),
                )
            {
                console::error((concat!(
                    "Detected multiple renderers concurrently rendering the ",
                    "same context provider. This is currently unsupported."
                ),));
            }
            context
                .current_renderer2
                .set(RENDERER_SIGIL.with(|&sigil| sigil));
        }
    }
    let parent = prev_snapshot.parent.clone();
    CURRENT_ACTIVE_SNAPSHOT.set(parent.clone());
    parent
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_active_context() -> ContextSnapshot {
    CURRENT_ACTIVE_SNAPSHOT.with_borrow(Clone::clone)
}

pub fn read_context(context: &Context) -> Option<&'static Unknown> {
    if IS_PRIMARY_RENDERER {
        *context.current_value.borrow()
    } else {
        *context.current_value2.borrow()
    }
}
