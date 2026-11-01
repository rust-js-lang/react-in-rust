// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use js::{JsError, Kind, SymbolFor, Unknown};
use react_isomorphic::react_lazy::LazyComponent;
use shared::environment::__DEV__;
use shared::react_component_stack_frame::{
    describe_built_in_component_frame, describe_class_component_frame, describe_debug_info_frame,
    describe_function_component_frame,
};
use shared::react_feature_flags::ENABLE_VIEW_TRANSITION;
use shared::react_owner_stack_frames::format_owner_stack;
use shared::react_symbols::{
    REACT_FORWARD_REF_TYPE, REACT_LAZY_TYPE, REACT_MEMO_TYPE, REACT_SUSPENSE_LIST_TYPE,
    REACT_SUSPENSE_TYPE, REACT_VIEW_TRANSITION_TYPE,
};

pub struct ComponentStackNode {
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub parent: Option<Rc<ComponentStackNode>>,
    // What it renders, as the element gave it: `null` or `undefined` too.
    pub r#type: Option<&'static Unknown>,
    // DEV only: a ReactComponentInfo or a ComponentStackNode.
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub owner: Option<&'static Unknown>,
    // DEV only: its text, or the Error it's made of.
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub stack: Cell<Option<&'static Unknown>>,
}

unsafe impl js::Defined for ComponentStackNode {}
// Safety: a node is an object, a key of the request's `didWarnForKey`.
unsafe impl js::WeakKey for ComponentStackNode {}

fn should_construct(component: &Unknown) -> bool {
    let prototype = js::get(component, "prototype");
    js::truthy(prototype) && js::truthy(js::get(prototype, "isReactComponent"))
}

fn describe_component_stack_by_type(mut r#type: Option<&'static Unknown>) -> String {
    if let Kind::String(r#type) = js::classify(r#type) {
        return describe_built_in_component_frame(r#type);
    }
    if js::type_of(r#type) == "function" {
        if should_construct(r#type.unwrap()) {
            return describe_class_component_frame(unsafe { js::cast(r#type) });
        } else {
            return describe_function_component_frame(unsafe { js::cast(r#type) });
        }
    }
    if js::type_of(r#type) == "object" {
        match SymbolFor::from_value(js::get(r#type, "$$typeof")) {
            Some(REACT_FORWARD_REF_TYPE) => {
                return describe_function_component_frame(unsafe {
                    js::cast(js::get(r#type, "render"))
                });
            }
            Some(REACT_MEMO_TYPE) => {
                return describe_function_component_frame(unsafe {
                    js::cast(js::get(r#type, "type"))
                });
            }
            Some(REACT_LAZY_TYPE) => {
                let lazy_component: &LazyComponent = unsafe { js::cast(r#type) };
                let payload = &lazy_component.payload;
                let init = lazy_component.init;
                match catch_unwind(AssertUnwindSafe(|| init(payload))) {
                    Ok(resolved) => r#type = unsafe { js::cast(resolved) },
                    Err(_) => {
                        // TODO: When we support Thenables as component types we should rename this.
                        return describe_built_in_component_frame("Lazy");
                    }
                }
                return describe_component_stack_by_type(r#type);
            }
            _ => {}
        }
        if js::type_of(js::get(r#type, "name")) == "string" {
            return describe_debug_info_frame(
                unsafe { js::cast(js::get(r#type, "name")) },
                unsafe { js::cast(js::get(r#type, "env")) },
                unsafe { js::cast(js::get(r#type, "debugLocation")) },
            );
        }
    }
    match SymbolFor::from_value(r#type) {
        Some(REACT_SUSPENSE_LIST_TYPE) => describe_built_in_component_frame("SuspenseList"),
        Some(REACT_SUSPENSE_TYPE) => describe_built_in_component_frame("Suspense"),
        Some(REACT_VIEW_TRANSITION_TYPE) if ENABLE_VIEW_TRANSITION => {
            describe_built_in_component_frame("ViewTransition")
        }
        _ => String::new(),
    }
}

pub fn get_stack_by_component_stack_node(component_stack: &Rc<ComponentStackNode>) -> String {
    match catch_unwind(AssertUnwindSafe(|| {
        let mut info = String::new();
        let mut node = Some(Rc::clone(component_stack));
        while let Some(current) = node {
            info += &describe_component_stack_by_type(current.r#type);
            node = current.parent.clone();
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

pub fn get_owner_stack_by_component_stack_node_in_dev(
    component_stack: &ComponentStackNode,
) -> String {
    if !__DEV__ {
        return String::new();
    }
    match catch_unwind(AssertUnwindSafe(|| {
        let mut info = String::new();

        // The owner stack of the current component will be where it was created, i.e. inside its owner.
        // There's no actual name of the currently executing component. Instead, that is available
        // on the regular stack that's currently executing. However, for built-ins there is no such
        // named stack frame and it would be ignored as being internal anyway. Therefore we add
        // add one extra frame just to describe the "current" built-in component by name.
        // Similarly, if there is no owner at all, then there's no stack frame so we add the name
        // of the root component to the stack to know which component is currently executing.
        if let Kind::String(r#type) = js::classify(component_stack.r#type) {
            info += &describe_built_in_component_frame(r#type);
        } else if js::type_of(component_stack.r#type) == "function" {
            if !js::truthy(component_stack.owner) {
                // Only if we have no other data about the callsite do we add
                // the component name as the single stack frame.
                info +=
                    &describe_function_component_frame_without_line_number(component_stack.r#type);
            }
        } else if !js::truthy(component_stack.owner) {
            info += &describe_component_stack_by_type(component_stack.r#type);
        }

        let mut owner: Option<&Unknown> = Some(js::unknown(component_stack));

        while js::truthy(owner) {
            let mut owner_stack: Option<String> = None;
            if js::get(owner, "debugStack").is_some() {
                // Server Component
                // TODO: Should we stash this somewhere for caching purposes?
                owner_stack = Some(format_owner_stack(unsafe {
                    js::cast(js::get(owner, "debugStack"))
                }));
                owner = js::get(owner, "owner");
            } else {
                // Client Component
                let node = owner;
                if let Some(stack) = js::get(node, "stack") {
                    if let Kind::String(stack) = js::classify(Some(stack)) {
                        owner_stack = Some(stack.to_owned());
                    } else {
                        let formatted = format_owner_stack(unsafe { js::cast(Some(stack)) });
                        js::set(node, "stack", formatted.as_str());
                        owner_stack = Some(formatted);
                    }
                }
                owner = js::get(owner, "owner");
            }
            // If we don't actually print the stack if there is no owner of this JSX element.
            // In a real app it's typically not useful since the root app is always controlled
            // by the framework. These also tend to have noisy stacks because they're not rooted
            // in a React render but in some imperative bootstrapping code. It could be useful
            // if the element was created in module scope. E.g. hoisted. We could add a a single
            // stack frame for context for example but it doesn't say much if that's a wrapper.
            if js::truthy(owner) && js::truthy(owner_stack.as_deref()) {
                info += &("\n".to_owned() + &owner_stack.unwrap());
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
