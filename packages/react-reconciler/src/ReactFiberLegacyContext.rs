// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;
use std::rc::Rc;

use js::{Dict, Unknown, object};
use shared::environment::__DEV__;
use shared::react_feature_flags::DISABLE_LEGACY_CONTEXT;
use webapi::console;

use crate::get_component_name_from_fiber::get_component_name_from_fiber;
use crate::react_fiber_stack::{StackCursor, create_cursor, pop, push};
use crate::react_internal_types::Fiber;
use crate::react_work_tags::{CLASS_COMPONENT, HOST_ROOT};

#[cfg_attr(rust_js, rust_js::link_name = "getChildContext")]
fn get_child_context(this: Option<&Unknown>) -> &'static Unknown {
    unreachable!()
}

/// A legacy context: the values a provider's `getChildContext` gives, by
/// their names.
pub type Context = Dict<Option<&'static Unknown>>;

thread_local! {
    static WARNED_ABOUT_MISSING_GET_CHILD_CONTEXT: Option<&'static Dict<bool>> =
        if __DEV__ { Some(js::dict::new()) } else { None };

    pub static EMPTY_CONTEXT_OBJECT: &'static Context = js::dict::new();
}

js::on_load! {
    if __DEV__ {
        EMPTY_CONTEXT_OBJECT.with(|empty_context_object| object::freeze(*empty_context_object));
    }
}

thread_local! {
    // A cursor to the current merged context object on the stack.
    static CONTEXT_STACK_CURSOR: StackCursor<&'static Context> =
        create_cursor(EMPTY_CONTEXT_OBJECT.with(|empty_context_object| *empty_context_object));
    // A cursor to a boolean indicating whether the context has changed.
    static DID_PERFORM_WORK_STACK_CURSOR: StackCursor<bool> = create_cursor(false);
    // Keep track of the previous context object that was on the stack.
    // We use this to get access to the parent context after we have already
    // pushed the next context provider, and now need to merge their contexts.
    static PREVIOUS_CONTEXT: Cell<&'static Context> =
        Cell::new(EMPTY_CONTEXT_OBJECT.with(|empty_context_object| *empty_context_object));
}

pub fn get_unmasked_context(
    _work_in_progress: &Fiber,
    component: &Unknown,
    did_push_own_context_if_provider: bool,
) -> &'static Context {
    if DISABLE_LEGACY_CONTEXT {
        EMPTY_CONTEXT_OBJECT.with(|empty_context_object| *empty_context_object)
    } else {
        if did_push_own_context_if_provider && is_context_provider(Some(component)) {
            // If the fiber is a context provider itself, when we read its context
            // we may have already pushed its own child context on the stack. A context
            // provider should not "see" its own child context. Therefore we read the
            // previous (parent) context instead for a context provider.
            return PREVIOUS_CONTEXT.get();
        }
        CONTEXT_STACK_CURSOR.with(|cursor| *cursor.current.borrow())
    }
}

pub fn cache_context(
    work_in_progress: &Fiber,
    unmasked_context: &'static Context,
    masked_context: &'static Context,
) {
    if DISABLE_LEGACY_CONTEXT {
    } else {
        let instance = work_in_progress.state_node.get();
        js::set(
            instance,
            "__reactInternalMemoizedUnmaskedChildContext",
            unmasked_context,
        );
        js::set(
            instance,
            "__reactInternalMemoizedMaskedChildContext",
            masked_context,
        );
    }
}

pub fn get_masked_context(
    work_in_progress: &Fiber,
    unmasked_context: &'static Context,
) -> &'static Context {
    if DISABLE_LEGACY_CONTEXT {
        EMPTY_CONTEXT_OBJECT.with(|empty_context_object| *empty_context_object)
    } else {
        let r#type = work_in_progress.r#type.get();
        let Some(context_types) =
            js::get(r#type, "contextTypes").filter(|context_types| js::truthy(Some(*context_types)))
        else {
            return EMPTY_CONTEXT_OBJECT.with(|empty_context_object| *empty_context_object);
        };

        // Avoid recreating masked context unless unmasked context has changed.
        // Failing to do this will result in unnecessary calls to componentWillReceiveProps.
        // This may trigger infinite loops if componentWillReceiveProps calls setState.
        let instance = work_in_progress.state_node.get();
        if js::truthy(instance)
            && js::strict_equals(
                js::get(instance, "__reactInternalMemoizedUnmaskedChildContext"),
                unmasked_context,
            )
        {
            return unsafe { js::cast(js::get(instance, "__reactInternalMemoizedMaskedChildContext")) };
        }

        let context: &'static Context = js::dict::new();
        for key in js::keys_in(context_types) {
            js::dict::set(context, &key, js::get(unmasked_context, key.as_str()));
        }

        // Cache unmasked context so we can avoid recreating masked context unless necessary.
        // Context is created before the class component is instantiated so check for instance.
        if js::truthy(instance) {
            cache_context(work_in_progress, unmasked_context, context);
        }

        context
    }
}

pub fn has_context_changed() -> bool {
    if DISABLE_LEGACY_CONTEXT {
        false
    } else {
        DID_PERFORM_WORK_STACK_CURSOR.with(|cursor| *cursor.current.borrow())
    }
}

pub fn is_context_provider(r#type: Option<&Unknown>) -> bool {
    if DISABLE_LEGACY_CONTEXT {
        false
    } else {
        let child_context_types = js::get(r#type, "childContextTypes");
        child_context_types.is_some()
    }
}

pub fn pop_context(fiber: &Fiber) {
    if DISABLE_LEGACY_CONTEXT {
    } else {
        DID_PERFORM_WORK_STACK_CURSOR.with(|cursor| pop(cursor, fiber));
        CONTEXT_STACK_CURSOR.with(|cursor| pop(cursor, fiber));
    }
}

pub fn pop_top_level_context_object(fiber: &Fiber) {
    if DISABLE_LEGACY_CONTEXT {
    } else {
        DID_PERFORM_WORK_STACK_CURSOR.with(|cursor| pop(cursor, fiber));
        CONTEXT_STACK_CURSOR.with(|cursor| pop(cursor, fiber));
    }
}

pub fn push_top_level_context_object(
    fiber: &Rc<Fiber>,
    context: &'static Context,
    did_change: bool,
) {
    if DISABLE_LEGACY_CONTEXT {
    } else {
        if !std::ptr::eq(
            CONTEXT_STACK_CURSOR.with(|cursor| *cursor.current.borrow()),
            EMPTY_CONTEXT_OBJECT.with(|empty_context_object| *empty_context_object),
        ) {
            panic!(concat!(
                "Unexpected context found on stack. ",
                "This error is likely caused by a bug in React. Please file an issue."
            ));
        }

        CONTEXT_STACK_CURSOR.with(|cursor| push(cursor, context, fiber));
        DID_PERFORM_WORK_STACK_CURSOR.with(|cursor| push(cursor, did_change, fiber));
    }
}

pub fn process_child_context(
    fiber: &Fiber,
    r#type: &Unknown,
    parent_context: &'static Context,
) -> &'static Context {
    if DISABLE_LEGACY_CONTEXT {
        parent_context
    } else {
        let instance = fiber.state_node.get();
        let child_context_types = js::get(r#type, "childContextTypes");

        // TODO (bvaughn) Replace this behavior with an invariant() in the future.
        // It has only been added in Fiber to match the (unintentional) behavior in Stack.
        if js::type_of(js::get(instance, "getChildContext")) != "function" {
            if __DEV__ {
                let component_name = get_component_name_from_fiber(fiber)
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| "Unknown".to_string());

                if let Some(warned_about_missing_get_child_context) =
                    WARNED_ABOUT_MISSING_GET_CHILD_CONTEXT.with(|warned| *warned)
                    && !js::dict::get(warned_about_missing_get_child_context, &component_name)
                        .is_some_and(|&warned| warned)
                {
                    js::dict::set(
                        warned_about_missing_get_child_context,
                        &component_name,
                        true,
                    );
                    console::error((
                        concat!(
                            "%s.childContextTypes is specified but there is no getChildContext() method ",
                            "on the instance. You can either define getChildContext() on %s or remove ",
                            "childContextTypes from it."
                        ),
                        component_name.as_str(),
                        component_name.as_str(),
                    ));
                }
            }
            return parent_context;
        }

        let child_context = get_child_context(instance);
        for context_key in js::keys_in(child_context) {
            if !js::has(child_context_types, context_key.as_str()) {
                panic!(
                    "{}.getChildContext(): key \"{context_key}\" is not defined in childContextTypes.",
                    get_component_name_from_fiber(fiber)
                        .filter(|name| !name.is_empty())
                        .unwrap_or_else(|| "Unknown".to_string())
                );
            }
        }

        let merged: &'static Context = js::dict::new();
        object::assign(js::unknown(merged), (parent_context, child_context));
        merged
    }
}

pub fn push_context_provider(work_in_progress: &Rc<Fiber>) -> bool {
    if DISABLE_LEGACY_CONTEXT {
        false
    } else {
        let instance = work_in_progress.state_node.get();
        // We push the context as early as possible to ensure stack integrity.
        // If the instance does not exist yet, we will push null at first,
        // and replace it on the stack later when invalidating the context.
        let memoized_merged_child_context = if js::truthy(instance) {
            js::get(instance, "__reactInternalMemoizedMergedChildContext")
        } else {
            instance
        };
        let memoized_merged_child_context: &'static Context = if js::truthy(memoized_merged_child_context) {
            unsafe { js::cast(memoized_merged_child_context) }
        } else {
            EMPTY_CONTEXT_OBJECT.with(|empty_context_object| *empty_context_object)
        };

        // Remember the parent context so we can merge with it later.
        // Inherit the parent's did-perform-work value to avoid inadvertently blocking updates.
        PREVIOUS_CONTEXT.set(CONTEXT_STACK_CURSOR.with(|cursor| *cursor.current.borrow()));
        CONTEXT_STACK_CURSOR.with(|cursor| push(cursor, memoized_merged_child_context, work_in_progress));
        DID_PERFORM_WORK_STACK_CURSOR.with(|cursor| {
            let did_perform_work = *cursor.current.borrow();
            push(cursor, did_perform_work, work_in_progress);
        });

        true
    }
}

pub fn invalidate_context_provider(work_in_progress: &Rc<Fiber>, r#type: &Unknown, did_change: bool) {
    if DISABLE_LEGACY_CONTEXT {
    } else {
        let instance = work_in_progress.state_node.get();

        if !js::truthy(instance) {
            panic!(concat!(
                "Expected to have an instance by this point. ",
                "This error is likely caused by a bug in React. Please file an issue."
            ));
        }

        if did_change {
            // Merge parent and own context.
            // Skip this if we're not updating due to sCU.
            // This avoids unnecessarily recomputing memoized values.
            let merged_context = process_child_context(work_in_progress, r#type, PREVIOUS_CONTEXT.get());
            js::set(
                instance,
                "__reactInternalMemoizedMergedChildContext",
                merged_context,
            );

            // Replace the old (or empty) context with the new one.
            // It is important to unwind the context in the reverse order.
            DID_PERFORM_WORK_STACK_CURSOR.with(|cursor| pop(cursor, work_in_progress));
            CONTEXT_STACK_CURSOR.with(|cursor| pop(cursor, work_in_progress));
            // Now push the new context and mark that it has changed.
            CONTEXT_STACK_CURSOR.with(|cursor| push(cursor, merged_context, work_in_progress));
            DID_PERFORM_WORK_STACK_CURSOR.with(|cursor| push(cursor, did_change, work_in_progress));
        } else {
            DID_PERFORM_WORK_STACK_CURSOR.with(|cursor| pop(cursor, work_in_progress));
            DID_PERFORM_WORK_STACK_CURSOR.with(|cursor| push(cursor, did_change, work_in_progress));
        }
    }
}

pub fn find_current_unmasked_context(fiber: &Rc<Fiber>) -> &'static Context {
    if DISABLE_LEGACY_CONTEXT {
        EMPTY_CONTEXT_OBJECT.with(|empty_context_object| *empty_context_object)
    } else {
        // Currently this is only used with renderSubtreeIntoContainer; not sure if it
        // makes sense elsewhere
        let mut node = Rc::clone(fiber);
        loop {
            match node.tag.get() {
                HOST_ROOT => {
                    return unsafe { js::cast(js::get(node.state_node.get(), "context")) };
                }
                CLASS_COMPONENT => {
                    let component = node.r#type.get();
                    if is_context_provider(component) {
                        return unsafe {
                            js::cast(js::get(
                                node.state_node.get(),
                                "__reactInternalMemoizedMergedChildContext",
                            ))
                        };
                    }
                }
                _ => {}
            }
            let Some(parent) = node.r#return.borrow().clone() else {
                break;
            };
            node = parent;
        }

        panic!(concat!(
            "Found unexpected detached subtree parent. ",
            "This error is likely caused by a bug in React. Please file an issue."
        ));
    }
}
