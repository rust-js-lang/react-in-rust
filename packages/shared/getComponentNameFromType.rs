// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::marker::PhantomData;

use js::{JsError, JsObject, SymbolFor, Unknown};

use crate::environment::__DEV__;
use crate::react_feature_flags::{ENABLE_TRANSITION_TRACING, ENABLE_VIEW_TRANSITION};
use crate::react_symbols::{
    REACT_ACTIVITY_TYPE, REACT_CONSUMER_TYPE, REACT_CONTEXT_TYPE, REACT_FORWARD_REF_TYPE,
    REACT_FRAGMENT_TYPE, REACT_LAZY_TYPE, REACT_MEMO_TYPE, REACT_PORTAL_TYPE, REACT_PROFILER_TYPE,
    REACT_STRICT_MODE_TYPE, REACT_SUSPENSE_LIST_TYPE, REACT_SUSPENSE_TYPE,
    REACT_TRACING_MARKER_TYPE, REACT_VIEW_TRANSITION_TYPE,
};

// Keep in sync with react-reconciler/getComponentNameFromFiber
fn get_wrapped_name(outer_type: &Unknown, inner_type: &Unknown, wrapper_name: &str) -> String {
    let display_name = js::get(outer_type, "displayName");
    if js::truthy(display_name) {
        return unsafe { js::cast(display_name) };
    }
    let function_name = unsafe {
        js::cast::<Option<String>>(
            js::get(inner_type, "displayName")
                .filter(|name| js::truthy(Some(*name)))
                .or_else(|| js::get(inner_type, "name"))
                .filter(|name| js::truthy(Some(*name))),
        )
    }
    .unwrap_or_default();
    if !function_name.is_empty() {
        format!("{wrapper_name}({function_name})")
    } else {
        wrapper_name.to_string()
    }
}

// Keep in sync with react-reconciler/getComponentNameFromFiber
fn get_context_name(r#type: &Unknown) -> String {
    unsafe {
        js::cast::<Option<String>>(
            js::get(r#type, "displayName").filter(|name| js::truthy(Some(*name))),
        )
    }
    .unwrap_or_else(|| "Context".to_string())
}

/// A lazy component's `_init`, which makes what it loads of its `_payload`,
/// or throws.
pub(crate) struct LazyInit(PhantomData<JsObject>);

impl LazyInit {
    #[cfg_attr(rust_js, rust_js::link_name = "this()")]
    pub(crate) fn call(
        &self,
        payload: Option<&Unknown>,
    ) -> Result<Option<&'static Unknown>, &'static JsError> {
        let _ = payload;
        unreachable!()
    }
}

const REACT_CLIENT_REFERENCE: SymbolFor = SymbolFor("react.client.reference");

// Note that the reconciler package should generally prefer to use getComponentNameFromFiber() instead.
js::export_default! {
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub(crate) fn get_component_name_from_type(r#type: Option<&Unknown>) -> Option<String> {
        let Some(r#type) = r#type else {
            // Host root, text node or just invalid type.
            return None;
        };
        if js::type_of(Some(r#type)) == "function" {
            if matches!(SymbolFor::from_value(js::get(r#type, "$$typeof")), Some(REACT_CLIENT_REFERENCE)) {
                // TODO: Create a convention for naming client references with debug info.
                return None;
            }
            return unsafe {
                js::cast(
                    js::get(r#type, "displayName")
                        .filter(|name| js::truthy(Some(*name)))
                        .or_else(|| js::get(r#type, "name"))
                        .filter(|name| js::truthy(Some(*name))),
                )
            };
        }
        if js::type_of(Some(r#type)) == "string" {
            return Some(unsafe { js::cast(Some(r#type)) });
        }
        match SymbolFor::from_value(Some(r#type)) {
            Some(REACT_FRAGMENT_TYPE) => return Some("Fragment".to_string()),
            Some(REACT_PROFILER_TYPE) => return Some("Profiler".to_string()),
            Some(REACT_STRICT_MODE_TYPE) => return Some("StrictMode".to_string()),
            Some(REACT_SUSPENSE_TYPE) => return Some("Suspense".to_string()),
            Some(REACT_SUSPENSE_LIST_TYPE) => return Some("SuspenseList".to_string()),
            Some(REACT_ACTIVITY_TYPE) => return Some("Activity".to_string()),
            Some(REACT_VIEW_TRANSITION_TYPE) => {
                if ENABLE_VIEW_TRANSITION {
                    return Some("ViewTransition".to_string());
                }
                if ENABLE_TRANSITION_TRACING {
                    return Some("TracingMarker".to_string());
                }
            }
            Some(REACT_TRACING_MARKER_TYPE) if ENABLE_TRANSITION_TRACING => {
                return Some("TracingMarker".to_string());
            }
            _ => {}
        }
        if js::type_of(Some(r#type)) == "object" {
            if __DEV__ && js::type_of(js::get(r#type, "tag")) == "number" {
                eprintln!(
                    "Received an unexpected object in getComponentNameFromType(). This is likely a bug in React. Please file an issue."
                );
            }
            match SymbolFor::from_value(js::get(r#type, "$$typeof")) {
                Some(REACT_PORTAL_TYPE) => return Some("Portal".to_string()),
                Some(REACT_CONTEXT_TYPE) => return Some(get_context_name(r#type)),
                Some(REACT_CONSUMER_TYPE) => {
                    return Some(get_context_name(unsafe { js::cast(js::get(r#type, "_context")) }) + ".Consumer");
                }
                Some(REACT_FORWARD_REF_TYPE) => {
                    return Some(get_wrapped_name(r#type, unsafe { js::cast(js::get(r#type, "render")) }, "ForwardRef"));
                }
                Some(REACT_MEMO_TYPE) => {
                    let outer_name = js::get(r#type, "displayName").filter(|name| js::truthy(Some(*name)));
                    if outer_name.is_some() {
                        return unsafe { js::cast(outer_name) };
                    }
                    return get_component_name_from_type(js::get(r#type, "type"))
                        .filter(|name| js::truthy(Some(name.as_str())))
                        .or_else(|| Some("Memo".to_string()));
                }
                Some(REACT_LAZY_TYPE) => {
                    let lazy_component = r#type;
                    let payload = js::get(lazy_component, "_payload");
                    let init: &LazyInit = unsafe { js::cast(js::get(lazy_component, "_init")) };
                    return match init.call(payload) {
                        Ok(resolved) => get_component_name_from_type(resolved),
                        Err(_) => None,
                    };
                }
                _ => {}
            }
        }
        None
    }
}
