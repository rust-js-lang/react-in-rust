// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{SymbolFor, Unknown};
use shared::environment::__DEV__;
use shared::get_component_name_from_type::get_component_name_from_type;
use shared::react_feature_flags::{
    DISABLE_LEGACY_MODE, ENABLE_LEGACY_HIDDEN, ENABLE_VIEW_TRANSITION,
};
use shared::react_symbols::REACT_STRICT_MODE_TYPE;

use crate::react_internal_types::Fiber;
use crate::react_work_tags::{
    ACTIVITY_COMPONENT, CACHE_COMPONENT, CLASS_COMPONENT, CONTEXT_CONSUMER, CONTEXT_PROVIDER,
    DEHYDRATED_FRAGMENT, FORWARD_REF, FRAGMENT, FUNCTION_COMPONENT, HOST_COMPONENT, HOST_HOISTABLE,
    HOST_PORTAL, HOST_ROOT, HOST_SINGLETON, HOST_TEXT, INCOMPLETE_CLASS_COMPONENT,
    INCOMPLETE_FUNCTION_COMPONENT, LAZY_COMPONENT, LEGACY_HIDDEN_COMPONENT, MEMO_COMPONENT, MODE,
    OFFSCREEN_COMPONENT, PROFILER, SCOPE_COMPONENT, SIMPLE_MEMO_COMPONENT, SUSPENSE_COMPONENT,
    SUSPENSE_LIST_COMPONENT, THROW, TRACING_MARKER_COMPONENT, VIEW_TRANSITION_COMPONENT,
};

// Keep in sync with shared/getComponentNameFromType
fn get_wrapped_name(
    outer_type: Option<&Unknown>,
    inner_type: Option<&Unknown>,
    wrapper_name: &str,
) -> String {
    let function_name = unsafe {
        js::cast::<Option<String>>(
            js::get(inner_type, "displayName")
                .filter(|name| js::truthy(Some(*name)))
                .or_else(|| js::get(inner_type, "name"))
                .filter(|name| js::truthy(Some(*name))),
        )
    }
    .unwrap_or_default();
    unsafe {
        js::cast::<Option<String>>(
            js::get(outer_type, "displayName").filter(|name| js::truthy(Some(*name))),
        )
    }
    .unwrap_or_else(|| {
        if !function_name.is_empty() {
            format!("{wrapper_name}({function_name})")
        } else {
            wrapper_name.to_string()
        }
    })
}

// Keep in sync with shared/getComponentNameFromType
fn get_context_name(r#type: Option<&Unknown>) -> String {
    unsafe {
        js::cast::<Option<String>>(
            js::get(r#type, "displayName").filter(|name| js::truthy(Some(*name))),
        )
    }
    .unwrap_or_else(|| "Context".to_string())
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_component_name_from_owner(owner: &Unknown) -> Option<String> {
    if js::type_of(js::get(owner, "tag")) == "number" {
        return get_component_name_from_fiber(unsafe { js::cast(Some(owner)) });
    }
    if js::type_of(js::get(owner, "name")) == "string" {
        return Some(unsafe { js::cast(js::get(owner, "name")) });
    }
    None
}

js::export_default! {
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub fn get_component_name_from_fiber(fiber: &Fiber) -> Option<String> {
        let tag = fiber.tag.get();
        let r#type = fiber.r#type.get();
        match tag {
            ACTIVITY_COMPONENT => return Some("Activity".to_string()),
            CACHE_COMPONENT => return Some("Cache".to_string()),
            CONTEXT_CONSUMER => {
                let consumer = r#type;
                return Some(get_context_name(js::get(consumer, "_context")) + ".Consumer");
            }
            CONTEXT_PROVIDER => {
                let context = r#type;
                return Some(get_context_name(context));
            }
            DEHYDRATED_FRAGMENT => return Some("DehydratedFragment".to_string()),
            FORWARD_REF => {
                return Some(get_wrapped_name(r#type, js::get(r#type, "render"), "ForwardRef"));
            }
            FRAGMENT => return Some("Fragment".to_string()),
            HOST_HOISTABLE | HOST_SINGLETON | HOST_COMPONENT => {
                // Host component type is the display name (e.g. "div", "View")
                return Some(unsafe { js::cast(r#type) });
            }
            HOST_PORTAL => return Some("Portal".to_string()),
            HOST_ROOT => return Some("Root".to_string()),
            HOST_TEXT => return Some("Text".to_string()),
            LAZY_COMPONENT => {
                // Name comes from the type in this case; we don't have a tag.
                return get_component_name_from_type(r#type);
            }
            MODE => {
                if matches!(SymbolFor::from_value(r#type), Some(REACT_STRICT_MODE_TYPE)) {
                    // Don't be less specific than shared/getComponentNameFromType
                    return Some("StrictMode".to_string());
                }
                return Some("Mode".to_string());
            }
            OFFSCREEN_COMPONENT => {
                let return_fiber = fiber.r#return.borrow().clone();
                if let Some(return_fiber) = return_fiber {
                    return get_component_name_from_fiber(&return_fiber);
                }
                return None;
            }
            PROFILER => return Some("Profiler".to_string()),
            SCOPE_COMPONENT => return Some("Scope".to_string()),
            SUSPENSE_COMPONENT => return Some("Suspense".to_string()),
            SUSPENSE_LIST_COMPONENT => return Some("SuspenseList".to_string()),
            TRACING_MARKER_COMPONENT => return Some("TracingMarker".to_string()),
            VIEW_TRANSITION_COMPONENT if ENABLE_VIEW_TRANSITION => {
                return Some("ViewTransition".to_string());
            }
            // The display name for these tags come from the user-provided type:
            VIEW_TRANSITION_COMPONENT | INCOMPLETE_CLASS_COMPONENT | INCOMPLETE_FUNCTION_COMPONENT
                if DISABLE_LEGACY_MODE => {}
            VIEW_TRANSITION_COMPONENT
            | INCOMPLETE_CLASS_COMPONENT
            | INCOMPLETE_FUNCTION_COMPONENT
            | CLASS_COMPONENT
            | FUNCTION_COMPONENT
            | MEMO_COMPONENT
            | SIMPLE_MEMO_COMPONENT => {
                if js::type_of(r#type) == "function" {
                    return unsafe {
                        js::cast(
                            js::get(r#type, "displayName")
                                .filter(|name| js::truthy(Some(*name)))
                                .or_else(|| js::get(r#type, "name"))
                                .filter(|name| js::truthy(Some(*name))),
                        )
                    };
                }
                if js::type_of(r#type) == "string" {
                    return Some(unsafe { js::cast(r#type) });
                }
            }
            LEGACY_HIDDEN_COMPONENT => {
                if ENABLE_LEGACY_HIDDEN {
                    return Some("LegacyHidden".to_string());
                }
            }
            THROW => {
                if __DEV__ {
                    // For an error in child position we use the name of the inner most parent component.
                    // Whether a Server Component or the parent Fiber.
                    if let Some(debug_info) = fiber.debug_info.get() {
                        let debug_info: &[&Unknown] = unsafe { js::cast(Some(debug_info)) };
                        for info in debug_info.iter().rev() {
                            if js::type_of(js::get(*info, "name")) == "string" {
                                return Some(unsafe { js::cast(js::get(*info, "name")) });
                            }
                        }
                    }
                    let return_fiber = fiber.r#return.borrow().clone()?;
                    return get_component_name_from_fiber(&return_fiber);
                }
                return None;
            }
            _ => {}
        }

        None
    }
}
