// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js::Unknown;
use react_isomorphic::react_context::ReactContext;
use shared::environment::__DEV__;
use shared::object_is::OBJECT_IS;
use webapi::console;

use crate::react_fiber_flags::{DID_PROPAGATE_CONTEXT, NEEDS_PROPAGATION, NO_FLAGS};
use crate::react_fiber_host_context::get_host_transition_provider;
use crate::react_fiber_lane::{Lanes, NO_LANES, is_subset_of_lanes, merge_lanes};
use crate::react_fiber_stack::{StackCursor, create_cursor, pop, push};
use crate::react_fiber_suspense_component::SuspenseState;
use crate::react_internal_types::{ContextDependency, Dependencies, Fiber};
use crate::react_work_tags::{CONTEXT_PROVIDER, DEHYDRATED_FRAGMENT, SUSPENSE_COMPONENT};

unsafe extern "Rust" {
    #[link_name = "./src/ReactFiberConfig.js#isPrimaryRenderer"]
    safe static IS_PRIMARY_RENDERER: bool;
    // ReactFiberConfig's `ReactContext<TransitionStatus>`.
    #[link_name = "./src/ReactFiberConfig.js#HostTransitionContext"]
    safe static HOST_TRANSITION_CONTEXT: ReactContext<Option<&'static Unknown>>;
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::nullable)]
    static VALUE_CURSOR: StackCursor<Option<&'static Unknown>> = create_cursor(None);

    #[cfg_attr(rust_js, rust_js::name = "rendererCursorDEV")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    static RENDERER_CURSOR_DEV: StackCursor<Option<&'static Unknown>> = create_cursor(None);
    #[cfg_attr(rust_js, rust_js::name = "renderer2CursorDEV")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    static RENDERER2_CURSOR_DEV: StackCursor<Option<&'static Unknown>> = create_cursor(None);

    static RENDERER_SIGIL: Option<&'static Unknown> = if __DEV__ {
        // Use this to detect multiple renderers using the same context
        Some(js::unknown(js::dict::new::<Unknown>()))
    } else {
        None
    };

    #[cfg_attr(rust_js, rust_js::nullable)]
    static CURRENTLY_RENDERING_FIBER: RefCell<Option<Rc<Fiber>>> = const { RefCell::new(None) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static LAST_CONTEXT_DEPENDENCY: RefCell<Option<Rc<ContextDependency<Option<&'static Unknown>>>>> =
        const { RefCell::new(None) };

    #[cfg_attr(rust_js, rust_js::name = "isDisallowedContextReadInDEV")]
    static IS_DISALLOWED_CONTEXT_READ_IN_DEV: Cell<bool> = const { Cell::new(false) };
}

pub fn reset_context_dependencies() {
    // This is called right before React yields execution, to ensure `readContext`
    // cannot be called outside the render phase.
    CURRENTLY_RENDERING_FIBER.set(None);
    LAST_CONTEXT_DEPENDENCY.set(None);
    if __DEV__ {
        IS_DISALLOWED_CONTEXT_READ_IN_DEV.set(false);
    }
}

#[cfg_attr(rust_js, rust_js::name = "enterDisallowedContextReadInDEV")]
pub fn enter_disallowed_context_read_in_dev() {
    if __DEV__ {
        IS_DISALLOWED_CONTEXT_READ_IN_DEV.set(true);
    }
}

#[cfg_attr(rust_js, rust_js::name = "exitDisallowedContextReadInDEV")]
pub fn exit_disallowed_context_read_in_dev() {
    if __DEV__ {
        IS_DISALLOWED_CONTEXT_READ_IN_DEV.set(false);
    }
}

pub fn push_provider<T: 'static>(
    provider_fiber: &Rc<Fiber>,
    context: &ReactContext<T>,
    next_value: T,
) {
    if IS_PRIMARY_RENDERER {
        let current_value = context.current_value.replace(next_value);
        VALUE_CURSOR.with(|cursor| push(cursor, js::any_of(current_value), provider_fiber));

        if __DEV__ {
            RENDERER_CURSOR_DEV
                .with(|cursor| push(cursor, context.current_renderer.get(), provider_fiber));

            let renderer_sigil = RENDERER_SIGIL.with(|&sigil| sigil);
            if context.current_renderer.get().is_some()
                && !js::strict_equals(context.current_renderer.get(), renderer_sigil)
            {
                console::error((concat!(
                    "Detected multiple renderers concurrently rendering the ",
                    "same context provider. This is currently unsupported."
                ),));
            }
            context.current_renderer.set(renderer_sigil);
        }
    } else {
        let current_value2 = context.current_value2.replace(next_value);
        VALUE_CURSOR.with(|cursor| push(cursor, js::any_of(current_value2), provider_fiber));

        if __DEV__ {
            RENDERER2_CURSOR_DEV
                .with(|cursor| push(cursor, context.current_renderer2.get(), provider_fiber));

            let renderer_sigil = RENDERER_SIGIL.with(|&sigil| sigil);
            if context.current_renderer2.get().is_some()
                && !js::strict_equals(context.current_renderer2.get(), renderer_sigil)
            {
                console::error((concat!(
                    "Detected multiple renderers concurrently rendering the ",
                    "same context provider. This is currently unsupported."
                ),));
            }
            context.current_renderer2.set(renderer_sigil);
        }
    }
}

pub fn pop_provider<T>(context: &ReactContext<T>, provider_fiber: &Fiber) {
    let current_value = VALUE_CURSOR.with(|cursor| *cursor.current.borrow());

    if IS_PRIMARY_RENDERER {
        *context.current_value.borrow_mut() = unsafe { js::cast(current_value) };
        if __DEV__ {
            let current_renderer = RENDERER_CURSOR_DEV.with(|cursor| *cursor.current.borrow());
            RENDERER_CURSOR_DEV.with(|cursor| pop(cursor, provider_fiber));
            context.current_renderer.set(current_renderer);
        }
    } else {
        *context.current_value2.borrow_mut() = unsafe { js::cast(current_value) };
        if __DEV__ {
            let current_renderer2 = RENDERER2_CURSOR_DEV.with(|cursor| *cursor.current.borrow());
            RENDERER2_CURSOR_DEV.with(|cursor| pop(cursor, provider_fiber));
            context.current_renderer2.set(current_renderer2);
        }
    }

    VALUE_CURSOR.with(|cursor| pop(cursor, provider_fiber));
}

pub fn schedule_context_work_on_parent_path(
    parent: Option<Rc<Fiber>>,
    render_lanes: Lanes,
    propagation_root: &Fiber,
) {
    // Update the child lanes of all the ancestors, including the alternates.
    let mut node = parent;
    while let Some(fiber) = &node {
        let alternate = fiber.alternate.borrow().clone();
        if !is_subset_of_lanes(fiber.child_lanes.get(), render_lanes) {
            fiber
                .child_lanes
                .set(merge_lanes(fiber.child_lanes.get(), render_lanes));
            if let Some(alternate) = &alternate {
                alternate
                    .child_lanes
                    .set(merge_lanes(alternate.child_lanes.get(), render_lanes));
            }
        } else if let Some(alternate) = &alternate
            && !is_subset_of_lanes(alternate.child_lanes.get(), render_lanes)
        {
            alternate
                .child_lanes
                .set(merge_lanes(alternate.child_lanes.get(), render_lanes));
        } else {
            // Neither alternate was updated.
            // Normally, this would mean that the rest of the
            // ancestor path already has sufficient priority.
            // However, this is not necessarily true inside offscreen
            // or fallback trees because childLanes may be inconsistent
            // with the surroundings. This is why we continue the loop.
        }
        if **fiber == *propagation_root {
            break;
        }
        let next = fiber.r#return.borrow().clone();
        node = next;
    }
    if __DEV__ && node.as_deref() != Some(propagation_root) {
        console::error((concat!(
            "Expected to find the propagation root when scheduling context work. ",
            "This error is likely caused by a bug in React. Please file an issue."
        ),));
    }
}

pub fn propagate_context_change<T>(
    work_in_progress: &Rc<Fiber>,
    context: &ReactContext<T>,
    render_lanes: Lanes,
) {
    // TODO: This path is only used by Cache components. Update
    // lazilyPropagateParentContextChanges to look for Cache components so they
    // can take advantage of lazy propagation.
    let force_propagate_entire_tree = true;
    propagate_context_changes(
        work_in_progress,
        &[context],
        render_lanes,
        force_propagate_entire_tree,
    );
}

fn propagate_context_changes<T>(
    work_in_progress: &Rc<Fiber>,
    contexts: &[&ReactContext<T>],
    render_lanes: Lanes,
    force_propagate_entire_tree: bool,
) {
    let mut fiber = work_in_progress.child.borrow().clone();
    if let Some(fiber) = &fiber {
        // Set the return pointer of the child to the work-in-progress fiber.
        fiber.r#return.replace(Some(Rc::clone(work_in_progress)));
    }
    while let Some(current) = fiber {
        let mut next_fiber;

        // Visit this fiber.
        let list = current.dependencies.borrow().clone();
        if let Some(list) = list {
            next_fiber = current.child.borrow().clone();

            let mut dep = list.first_context.borrow().clone();
            'find_changed_dep: while let Some(dependency) = dep {
                // Assigning these to constants to help Flow
                let consumer = &current;
                for context in contexts {
                    // Check if the context matches.
                    if js::strict_equals(&*dependency.context, *context) {
                        // Match! Schedule an update on this fiber.

                        // In the lazy implementation, don't mark a dirty flag on the
                        // dependency itself. Not all changes are propagated, so we can't
                        // rely on the propagation function alone to determine whether
                        // something has changed; the consumer will check. In the future, we
                        // could add back a dirty flag as an optimization to avoid double
                        // checking, but until we have selectors it's not really worth
                        // the trouble.
                        consumer
                            .lanes
                            .set(merge_lanes(consumer.lanes.get(), render_lanes));
                        let alternate = consumer.alternate.borrow().clone();
                        if let Some(alternate) = alternate {
                            alternate
                                .lanes
                                .set(merge_lanes(alternate.lanes.get(), render_lanes));
                        }
                        schedule_context_work_on_parent_path(
                            consumer.r#return.borrow().clone(),
                            render_lanes,
                            work_in_progress,
                        );

                        if !force_propagate_entire_tree {
                            // During lazy propagation, when we find a match, we can defer
                            // propagating changes to the children, because we're going to
                            // visit them during render. We should continue propagating the
                            // siblings, though
                            next_fiber = None;
                        }

                        // Since we already found a match, we can stop traversing the
                        // dependency list.
                        break 'find_changed_dep;
                    }
                }
                dep = dependency.next.borrow().clone();
            }
        } else if current.tag.get() == DEHYDRATED_FRAGMENT {
            // If a dehydrated suspense boundary is in this subtree, we don't know
            // if it will have any context consumers in it. The best we can do is
            // mark it as having updates.
            let Some(parent_suspense) = current.r#return.borrow().clone() else {
                panic!(
                    "We just came from a parent so we must have had a parent. This is a bug in React."
                );
            };

            parent_suspense
                .lanes
                .set(merge_lanes(parent_suspense.lanes.get(), render_lanes));
            let alternate = parent_suspense.alternate.borrow().clone();
            if let Some(alternate) = alternate {
                alternate
                    .lanes
                    .set(merge_lanes(alternate.lanes.get(), render_lanes));
            }
            // This is intentionally passing this fiber as the parent
            // because we want to schedule this fiber as having work
            // on its children. We'll use the childLanes on
            // this fiber to indicate that a context has changed.
            schedule_context_work_on_parent_path(
                Some(parent_suspense),
                render_lanes,
                work_in_progress,
            );
            next_fiber = None;
        } else if current.tag.get() == SUSPENSE_COMPONENT
            && let Some(state) =
                unsafe { js::cast::<Option<&SuspenseState>>(current.memoized_state.get()) }
            && state.dehydrated.is_none()
        {
            // This is a client-rendered Suspense boundary that is currently
            // showing its fallback. The primary children may include context
            // consumers, but their fibers may not exist in the tree — during
            // initial mount, if the primary children suspended, their fibers
            // were discarded since there was no current tree to preserve them.
            // We can't walk into the primary tree to find consumers, so
            // conservatively mark the Suspense boundary itself for retry.
            // When it re-renders, it will re-mount the primary children,
            // which will read the updated context value.
            current
                .lanes
                .set(merge_lanes(current.lanes.get(), render_lanes));
            let alternate = current.alternate.borrow().clone();
            if let Some(alternate) = alternate {
                alternate
                    .lanes
                    .set(merge_lanes(alternate.lanes.get(), render_lanes));
            }
            schedule_context_work_on_parent_path(
                current.r#return.borrow().clone(),
                render_lanes,
                work_in_progress,
            );
            // The primary children's fibers may not exist in the tree (they
            // were discarded on initial mount if they suspended). However, the
            // fallback children ARE in the committed tree and visible to the
            // user. We need to continue propagating into the fallback subtree
            // so that its context consumers are marked for re-render.
            //
            // The fiber structure is:
            //   SuspenseComponent
            //     → child: OffscreenComponent (primary, hidden)
            //       → sibling: FallbackFragment
            //
            // Skip the primary (hidden) subtree and jump to the fallback.
            let primary_child_fragment = current.child.borrow().clone();
            if let Some(primary_child_fragment) = primary_child_fragment {
                next_fiber = primary_child_fragment.sibling.borrow().clone();
            } else {
                next_fiber = None;
            }
        } else {
            // Traverse down.
            next_fiber = current.child.borrow().clone();
        }

        if let Some(next) = &next_fiber {
            // Set the return pointer of the child to the work-in-progress fiber.
            next.r#return.replace(Some(Rc::clone(&current)));
        } else {
            // No child. Traverse to next sibling.
            next_fiber = Some(current);
            while let Some(next) = next_fiber {
                if *next == **work_in_progress {
                    // We're back to the root of this subtree. Exit.
                    next_fiber = None;
                    break;
                }
                let sibling = next.sibling.borrow().clone();
                if let Some(sibling) = sibling {
                    // Set the return pointer of the sibling to the work-in-progress fiber.
                    let parent = next.r#return.borrow().clone();
                    sibling.r#return.replace(parent);
                    next_fiber = Some(sibling);
                    break;
                }
                // No more siblings. Traverse up.
                next_fiber = next.r#return.borrow().clone();
            }
        }
        fiber = next_fiber;
    }
}

pub fn lazily_propagate_parent_context_changes(
    current: &Fiber,
    work_in_progress: &Rc<Fiber>,
    render_lanes: Lanes,
) -> bool {
    let force_propagate_entire_tree = false;
    propagate_parent_context_changes(
        current,
        work_in_progress,
        render_lanes,
        force_propagate_entire_tree,
    )
}

// Used for propagating a deferred tree (Suspense, Offscreen). We must propagate
// to the entire subtree, because we won't revisit it until after the current
// render has completed, at which point we'll have lost track of which providers
// have changed.
pub fn propagate_parent_context_changes_to_deferred_tree(
    current: &Fiber,
    work_in_progress: &Rc<Fiber>,
    render_lanes: Lanes,
) {
    let force_propagate_entire_tree = true;
    propagate_parent_context_changes(
        current,
        work_in_progress,
        render_lanes,
        force_propagate_entire_tree,
    );
}

fn propagate_parent_context_changes(
    _current: &Fiber,
    work_in_progress: &Rc<Fiber>,
    render_lanes: Lanes,
    force_propagate_entire_tree: bool,
) -> bool {
    // Collect all the parent providers that changed. Since this is usually small
    // number, we use an Array instead of Set.
    let mut contexts: Option<Vec<&ReactContext<Option<&'static Unknown>>>> = None;
    let mut parent = Some(Rc::clone(work_in_progress));
    let mut is_inside_propagation_bailout = false;
    while let Some(fiber) = parent {
        if !is_inside_propagation_bailout {
            if (fiber.flags.get() & NEEDS_PROPAGATION) != NO_FLAGS {
                is_inside_propagation_bailout = true;
            } else if (fiber.flags.get() & DID_PROPAGATE_CONTEXT) != NO_FLAGS {
                break;
            }
        }

        if fiber.tag.get() == CONTEXT_PROVIDER {
            let Some(current_parent) = fiber.alternate.borrow().clone() else {
                panic!("Should have a current fiber. This is a bug in React.");
            };

            let old_props = current_parent.memoized_props.get();
            if old_props.is_some() {
                let context: &ReactContext<Option<&'static Unknown>> =
                    unsafe { js::cast(fiber.r#type.get()) };
                let new_props = fiber.pending_props.get();
                let new_value = js::get(new_props, "value");

                let old_value = js::get(old_props, "value");

                if !OBJECT_IS.with(|is| is(&new_value, &old_value)) {
                    if let Some(contexts) = &mut contexts {
                        contexts.push(context);
                    } else {
                        contexts = Some(vec![context]);
                    }
                }
            }
        } else if Some(&fiber) == get_host_transition_provider().as_ref() {
            // During a host transition, a host component can act like a context
            // provider. E.g. in React DOM, this would be a <form />.
            let Some(current_parent) = fiber.alternate.borrow().clone() else {
                panic!("Should have a current fiber. This is a bug in React.");
            };

            // ReactFiberHooks' `Hook`, of ReactFiberConfig's `TransitionStatus`.
            let old_state_hook = current_parent.memoized_state.get();
            let old_state = js::get(old_state_hook, "memoizedState");

            let new_state_hook = fiber.memoized_state.get();
            let new_state = js::get(new_state_hook, "memoizedState");

            // This uses regular equality instead of Object.is because we assume that
            // host transition state doesn't include NaN as a valid type.
            if !js::strict_equals(old_state, new_state) {
                if let Some(contexts) = &mut contexts {
                    contexts.push(&HOST_TRANSITION_CONTEXT);
                } else {
                    contexts = Some(vec![&HOST_TRANSITION_CONTEXT]);
                }
            }
        }
        parent = fiber.r#return.borrow().clone();
    }

    if let Some(contexts) = &contexts {
        // If there were any changed providers, search through the children and
        // propagate their changes.
        propagate_context_changes(
            work_in_progress,
            contexts,
            render_lanes,
            force_propagate_entire_tree,
        );
    }

    // This is an optimization so that we only propagate once per subtree. If a
    // deeply nested child bails out, and it calls this propagation function, it
    // uses this flag to know that the remaining ancestor providers have already
    // been propagated.
    //
    // NOTE: This optimization is only necessary because we sometimes enter the
    // begin phase of nodes that don't have any work scheduled on them —
    // specifically, the siblings of a node that _does_ have scheduled work. The
    // siblings will bail out and call this function again, even though we already
    // propagated content changes to it and its subtree. So we use this flag to
    // mark that the parent providers already propagated.
    //
    // Unfortunately, though, we need to ignore this flag when we're inside a
    // tree whose context propagation was deferred — that's what the
    // `NeedsPropagation` flag is for.
    //
    // If we could instead bail out before entering the siblings' begin phase,
    // then we could remove both `DidPropagateContext` and `NeedsPropagation`.
    // Consider this as part of the next refactor to the fiber tree structure.
    work_in_progress
        .flags
        .set(work_in_progress.flags.get() | DID_PROPAGATE_CONTEXT);
    contexts.is_some()
}

pub fn check_if_context_changed(current_dependencies: &Dependencies) -> bool {
    // Iterate over the current dependencies to see if something changed. This
    // only gets called if props and state has already bailed out, so it's a
    // relatively uncommon path, except at the root of a changed subtree.
    // Alternatively, we could move these comparisons into `readContext`, but
    // that's a much hotter path, so I think this is an appropriate trade off.
    let mut dependency = current_dependencies.first_context.borrow().clone();
    while let Some(current) = dependency {
        let context = &current.context;
        let new_value = if IS_PRIMARY_RENDERER {
            *context.current_value.borrow()
        } else {
            *context.current_value2.borrow()
        };
        let old_value = current.memoized_value.get();
        if !OBJECT_IS.with(|is| is(&new_value, &old_value)) {
            return true;
        }
        dependency = current.next.borrow().clone();
    }
    false
}

pub fn prepare_to_read_context(work_in_progress: &Rc<Fiber>, _render_lanes: Lanes) {
    CURRENTLY_RENDERING_FIBER.set(Some(Rc::clone(work_in_progress)));
    LAST_CONTEXT_DEPENDENCY.set(None);

    let dependencies = work_in_progress.dependencies.borrow().clone();
    if let Some(dependencies) = dependencies {
        // Reset the work-in-progress list
        dependencies.first_context.replace(None);
    }
}

pub fn read_context<T: Clone + 'static>(context: &Rc<ReactContext<T>>) -> T {
    // This warning would fire if you read context inside a Hook like useMemo.
    // Unlike the class check below, it's not enforced in production for perf.
    if __DEV__ && IS_DISALLOWED_CONTEXT_READ_IN_DEV.get() {
        console::error((concat!(
            "Context can only be read while React is rendering. ",
            "In classes, you can read it in the render method or getDerivedStateFromProps. ",
            "In function components, you can read it directly in the function body, but not ",
            "inside Hooks like useReducer() or useMemo()."
        ),));
    }
    read_context_for_consumer(
        CURRENTLY_RENDERING_FIBER.with_borrow(Clone::clone).as_ref(),
        context,
    )
}

pub fn read_context_during_reconciliation<T: Clone + 'static>(
    consumer: &Rc<Fiber>,
    context: &Rc<ReactContext<T>>,
    render_lanes: Lanes,
) -> T {
    if CURRENTLY_RENDERING_FIBER.with_borrow(Option::is_none) {
        prepare_to_read_context(consumer, render_lanes);
    }
    read_context_for_consumer(Some(consumer), context)
}

fn read_context_for_consumer<T: Clone + 'static>(
    consumer: Option<&Rc<Fiber>>,
    context: &Rc<ReactContext<T>>,
) -> T {
    let value = if IS_PRIMARY_RENDERER {
        context.current_value.borrow().clone()
    } else {
        context.current_value2.borrow().clone()
    };

    let context_item = Rc::new(ContextDependency {
        context: unsafe { js::cast(js::any_of(Rc::clone(context))) },
        memoized_value: Cell::new(js::any_of(value.clone())),
        next: RefCell::new(None),
    });

    let last_context_dependency = LAST_CONTEXT_DEPENDENCY.with_borrow(Clone::clone);
    match last_context_dependency {
        None => {
            let Some(consumer) = consumer else {
                panic!(
                    "{}",
                    concat!(
                        "Context can only be read while React is rendering. ",
                        "In classes, you can read it in the render method or getDerivedStateFromProps. ",
                        "In function components, you can read it directly in the function body, but not ",
                        "inside Hooks like useReducer() or useMemo()."
                    )
                );
            };

            // This is the first dependency for this component. Create a new list.
            LAST_CONTEXT_DEPENDENCY.set(Some(Rc::clone(&context_item)));
            consumer.dependencies.replace(Some(Rc::new(Dependencies {
                lanes: Cell::new(NO_LANES),
                first_context: RefCell::new(Some(context_item)),
                debug_thenable_state: Cell::new(None),
            })));
            consumer.flags.set(consumer.flags.get() | NEEDS_PROPAGATION);
        }
        Some(last_context_dependency) => {
            // Append a new context item.
            last_context_dependency
                .next
                .replace(Some(Rc::clone(&context_item)));
            LAST_CONTEXT_DEPENDENCY.set(Some(context_item));
        }
    }
    value
}
