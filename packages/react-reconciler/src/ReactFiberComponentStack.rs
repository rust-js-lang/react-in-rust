// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use js::{JsError, Unknown};
use shared::environment::__DEV__;
use shared::react_component_stack_frame::{
    describe_built_in_component_frame, describe_class_component_frame, describe_debug_info_frame,
    describe_function_component_frame,
};
use shared::react_feature_flags::ENABLE_VIEW_TRANSITION;
use shared::react_owner_stack_frames::format_owner_stack;

use crate::react_internal_types::Fiber;
use crate::react_work_tags::{
    ACTIVITY_COMPONENT, CLASS_COMPONENT, FORWARD_REF, FUNCTION_COMPONENT, HOST_COMPONENT,
    HOST_HOISTABLE, HOST_SINGLETON, HOST_TEXT, LAZY_COMPONENT, SIMPLE_MEMO_COMPONENT,
    SUSPENSE_COMPONENT, SUSPENSE_LIST_COMPONENT, VIEW_TRANSITION_COMPONENT,
};

// A fiber is an object, which an owner, a fiber or a Server Component's
// info, is read as.
unsafe impl js::Defined for Fiber {}

fn describe_fiber(fiber: &Fiber, child_fiber: Option<&Rc<Fiber>>) -> String {
    match fiber.tag.get() {
        HOST_HOISTABLE | HOST_SINGLETON | HOST_COMPONENT => {
            describe_built_in_component_frame(unsafe { js::cast(fiber.r#type.get()) })
        }
        LAZY_COMPONENT => {
            // TODO: When we support Thenables as component types we should rename this.
            describe_built_in_component_frame("Lazy")
        }
        SUSPENSE_COMPONENT => {
            if fiber.child.borrow().as_ref() != child_fiber && child_fiber.is_some() {
                // If we came from the second Fiber then we're in the Suspense Fallback.
                return describe_built_in_component_frame("Suspense Fallback");
            }
            describe_built_in_component_frame("Suspense")
        }
        SUSPENSE_LIST_COMPONENT => describe_built_in_component_frame("SuspenseList"),
        FUNCTION_COMPONENT | SIMPLE_MEMO_COMPONENT => {
            describe_function_component_frame(unsafe { js::cast(fiber.r#type.get()) })
        }
        FORWARD_REF => describe_function_component_frame(unsafe {
            js::cast(js::get(fiber.r#type.get(), "render"))
        }),
        CLASS_COMPONENT => describe_class_component_frame(unsafe { js::cast(fiber.r#type.get()) }),
        ACTIVITY_COMPONENT => describe_built_in_component_frame("Activity"),
        VIEW_TRANSITION_COMPONENT if ENABLE_VIEW_TRANSITION => {
            describe_built_in_component_frame("ViewTransition")
        }
        // Fallthrough
        _ => String::new(),
    }
}

pub fn get_stack_by_fiber_in_dev_and_prod(work_in_progress: &Rc<Fiber>) -> String {
    match catch_unwind(AssertUnwindSafe(|| {
        let mut info = String::new();
        let mut node = Some(Rc::clone(work_in_progress));
        let mut previous: Option<Rc<Fiber>> = None;
        while let Some(current) = node {
            info += &describe_fiber(&current, previous.as_ref());
            if __DEV__ {
                // Add any Server Component stack frames in reverse order.
                if let Some(debug_info) = current.debug_info.get() {
                    let debug_info: &[&Unknown] = unsafe { js::cast(Some(debug_info)) };
                    for entry in debug_info.iter().rev() {
                        if js::type_of(js::get(*entry, "name")) == "string" {
                            info += &describe_debug_info_frame(
                                unsafe { js::cast(js::get(*entry, "name")) },
                                unsafe { js::cast(js::get(*entry, "env")) },
                                unsafe { js::cast(js::get(*entry, "debugLocation")) },
                            );
                        }
                    }
                }
            }
            node = current.r#return.borrow().clone();
            previous = Some(current);
        }
        info
    })) {
        Ok(info) => info,
        Err(x) => match x.downcast_ref::<&'static JsError>() {
            Some(x) => format!(
                "\nError generating stack: {}\n{}",
                x.message(),
                js::shown(js::get(*x, "stack"))
            ),
            None => format!(
                "\nError generating stack: {}\n",
                x.downcast_ref::<String>().cloned().unwrap_or_default()
            ),
        },
    }
}

fn describe_function_component_frame_without_line_number(r#fn: Option<&Unknown>) -> String {
    // We use this because we don't actually want to describe the line of the component
    // but just the component name.
    let name = if js::truthy(r#fn) {
        js::get(r#fn, "displayName")
            .filter(|name| js::truthy(Some(*name)))
            .or_else(|| js::get(r#fn, "name"))
    } else {
        None
    };
    if js::truthy(name) {
        describe_built_in_component_frame(unsafe { js::cast(name) })
    } else {
        String::new()
    }
}

pub fn get_owner_stack_by_fiber_in_dev(mut work_in_progress: Rc<Fiber>) -> String {
    if !__DEV__ {
        return String::new();
    }
    match catch_unwind(AssertUnwindSafe(|| {
        let mut info = String::new();

        if work_in_progress.tag.get() == HOST_TEXT {
            // Text nodes never have an owner/stack because they're not created through JSX.
            // We use the parent since text nodes are always created through a host parent.
            let parent = work_in_progress.r#return.borrow().clone().unwrap();
            work_in_progress = parent;
        }

        // The owner stack of the current fiber will be where it was created, i.e. inside its owner.
        // There's no actual name of the currently executing component. Instead, that is available
        // on the regular stack that's currently executing. However, for built-ins there is no such
        // named stack frame and it would be ignored as being internal anyway. Therefore we add
        // add one extra frame just to describe the "current" built-in component by name.
        // Similarly, if there is no owner at all, then there's no stack frame so we add the name
        // of the root component to the stack to know which component is currently executing.
        match work_in_progress.tag.get() {
            HOST_HOISTABLE | HOST_SINGLETON | HOST_COMPONENT => {
                info += &describe_built_in_component_frame(unsafe {
                    js::cast(work_in_progress.r#type.get())
                });
            }
            SUSPENSE_COMPONENT => {
                info += &describe_built_in_component_frame("Suspense");
            }
            SUSPENSE_LIST_COMPONENT => {
                info += &describe_built_in_component_frame("SuspenseList");
            }
            ACTIVITY_COMPONENT => {
                info += &describe_built_in_component_frame("Activity");
            }
            VIEW_TRANSITION_COMPONENT if ENABLE_VIEW_TRANSITION => {
                info += &describe_built_in_component_frame("ViewTransition");
            }
            VIEW_TRANSITION_COMPONENT
            | FUNCTION_COMPONENT
            | SIMPLE_MEMO_COMPONENT
            | CLASS_COMPONENT => {
                if work_in_progress.debug_owner.get().is_none() && info.is_empty() {
                    // Only if we have no other data about the callsite do we add
                    // the component name as the single stack frame.
                    info += &describe_function_component_frame_without_line_number(
                        work_in_progress.r#type.get(),
                    );
                }
            }
            FORWARD_REF if work_in_progress.debug_owner.get().is_none() && info.is_empty() => {
                info += &describe_function_component_frame_without_line_number(js::get(
                    work_in_progress.r#type.get(),
                    "render",
                ));
            }
            _ => {}
        }

        let mut owner = Some(js::unknown(&*work_in_progress));

        while let Some(current) = owner {
            if js::type_of(js::get(current, "tag")) == "number" {
                let fiber: &Fiber = unsafe { js::cast(Some(current)) };
                owner = fiber.debug_owner.get();
                let debug_stack = fiber.debug_stack.get();
                // If we don't actually print the stack if there is no owner of this JSX element.
                // In a real app it's typically not useful since the root app is always controlled
                // by the framework. These also tend to have noisy stacks because they're not rooted
                // in a React render but in some imperative bootstrapping code. It could be useful
                // if the element was created in module scope. E.g. hoisted. We could add a a single
                // stack frame for context for example but it doesn't say much if that's a wrapper.
                if owner.is_some()
                    && let Some(debug_stack) = debug_stack
                {
                    let formatted_stack = format_owner_stack(debug_stack);
                    if !formatted_stack.is_empty() {
                        info += "\n";
                        info += &formatted_stack;
                    }
                }
            } else if js::get(current, "debugStack").is_some() {
                // Server Component
                let owner_stack: &JsError = unsafe { js::cast(js::get(current, "debugStack")) };
                owner = js::get(current, "owner");
                if owner.is_some() {
                    // TODO: Should we stash this somewhere for caching purposes?
                    info += "\n";
                    info += &format_owner_stack(owner_stack);
                }
            } else {
                break;
            }
        }
        info
    })) {
        Ok(info) => info,
        Err(x) => match x.downcast_ref::<&'static JsError>() {
            Some(x) => format!(
                "\nError generating stack: {}\n{}",
                x.message(),
                js::shown(js::get(*x, "stack"))
            ),
            None => format!(
                "\nError generating stack: {}\n",
                x.downcast_ref::<String>().cloned().unwrap_or_default()
            ),
        },
    }
}
