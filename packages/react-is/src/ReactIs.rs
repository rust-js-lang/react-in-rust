// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{SymbolFor, Unknown};

use shared::react_feature_flags::{
    ENABLE_LEGACY_HIDDEN, ENABLE_SCOPE_API, ENABLE_TRANSITION_TRACING, ENABLE_VIEW_TRANSITION,
};
use shared::react_symbols::{
    REACT_CONSUMER_TYPE, REACT_CONTEXT_TYPE, REACT_ELEMENT_TYPE, REACT_FORWARD_REF_TYPE,
    REACT_FRAGMENT_TYPE, REACT_LAZY_TYPE, REACT_LEGACY_HIDDEN_TYPE, REACT_MEMO_TYPE,
    REACT_PORTAL_TYPE, REACT_PROFILER_TYPE, REACT_SCOPE_TYPE, REACT_STRICT_MODE_TYPE,
    REACT_SUSPENSE_LIST_TYPE, REACT_SUSPENSE_TYPE, REACT_TRACING_MARKER_TYPE,
    REACT_VIEW_TRANSITION_TYPE,
};

const REACT_CLIENT_REFERENCE: SymbolFor = SymbolFor("react.client.reference");

pub fn type_of(object: Option<&'static Unknown>) -> Option<&'static Unknown> {
    if js::type_of(object) == "object"
        && let Some(object) = object
    {
        #[cfg_attr(rust_js, rust_js::name = "$$typeof")]
        let typeof_ = js::get(object, "$$typeof");
        match SymbolFor::from_value(typeof_) {
            Some(REACT_ELEMENT_TYPE) => {
                let r#type = js::get(object, "type");

                match SymbolFor::from_value(r#type) {
                    Some(
                        REACT_FRAGMENT_TYPE
                        | REACT_PROFILER_TYPE
                        | REACT_STRICT_MODE_TYPE
                        | REACT_SUSPENSE_TYPE
                        | REACT_SUSPENSE_LIST_TYPE
                        | REACT_VIEW_TRANSITION_TYPE,
                    ) => return r#type,
                    _ => {
                        #[cfg_attr(rust_js, rust_js::name = "$$typeofType")]
                        let typeof_type = r#type
                            .filter(|r#type| js::truthy(Some(*r#type)))
                            .and_then(|r#type| js::get(r#type, "$$typeof"));

                        match SymbolFor::from_value(typeof_type) {
                            Some(
                                REACT_CONTEXT_TYPE
                                | REACT_FORWARD_REF_TYPE
                                | REACT_LAZY_TYPE
                                | REACT_MEMO_TYPE,
                            ) => return typeof_type,
                            Some(REACT_CONSUMER_TYPE) => return typeof_type,
                            // Fall through
                            _ => return typeof_,
                        }
                    }
                }
            }
            Some(REACT_PORTAL_TYPE) => return typeof_,
            _ => {}
        }
    }

    None
}

#[cfg_attr(rust_js, rust_js::name = "ContextConsumer")]
pub const CONTEXT_CONSUMER: SymbolFor = REACT_CONSUMER_TYPE;
#[cfg_attr(rust_js, rust_js::name = "ContextProvider")]
pub const CONTEXT_PROVIDER: SymbolFor = REACT_CONTEXT_TYPE;
#[cfg_attr(rust_js, rust_js::name = "Element")]
pub const ELEMENT: SymbolFor = REACT_ELEMENT_TYPE;
#[cfg_attr(rust_js, rust_js::name = "ForwardRef")]
pub const FORWARD_REF: SymbolFor = REACT_FORWARD_REF_TYPE;
#[cfg_attr(rust_js, rust_js::name = "Fragment")]
pub const FRAGMENT: SymbolFor = REACT_FRAGMENT_TYPE;
#[cfg_attr(rust_js, rust_js::name = "Lazy")]
pub const LAZY: SymbolFor = REACT_LAZY_TYPE;
#[cfg_attr(rust_js, rust_js::name = "Memo")]
pub const MEMO: SymbolFor = REACT_MEMO_TYPE;
#[cfg_attr(rust_js, rust_js::name = "Portal")]
pub const PORTAL: SymbolFor = REACT_PORTAL_TYPE;
#[cfg_attr(rust_js, rust_js::name = "Profiler")]
pub const PROFILER: SymbolFor = REACT_PROFILER_TYPE;
#[cfg_attr(rust_js, rust_js::name = "StrictMode")]
pub const STRICT_MODE: SymbolFor = REACT_STRICT_MODE_TYPE;
#[cfg_attr(rust_js, rust_js::name = "Suspense")]
pub const SUSPENSE: SymbolFor = REACT_SUSPENSE_TYPE;
#[cfg_attr(rust_js, rust_js::name = "SuspenseList")]
pub const SUSPENSE_LIST: SymbolFor = REACT_SUSPENSE_LIST_TYPE;

pub fn is_valid_element_type(r#type: Option<&'static Unknown>) -> bool {
    if js::type_of(r#type) == "string" || js::type_of(r#type) == "function" {
        return true;
    }

    // Note: typeof might be other than 'symbol' or 'number' (e.g. if it's a polyfill).
    if SymbolFor::from_value(r#type) == Some(REACT_FRAGMENT_TYPE)
        || SymbolFor::from_value(r#type) == Some(REACT_PROFILER_TYPE)
        || SymbolFor::from_value(r#type) == Some(REACT_STRICT_MODE_TYPE)
        || SymbolFor::from_value(r#type) == Some(REACT_SUSPENSE_TYPE)
        || SymbolFor::from_value(r#type) == Some(REACT_SUSPENSE_LIST_TYPE)
        || (ENABLE_LEGACY_HIDDEN && SymbolFor::from_value(r#type) == Some(REACT_LEGACY_HIDDEN_TYPE))
        || (ENABLE_SCOPE_API && SymbolFor::from_value(r#type) == Some(REACT_SCOPE_TYPE))
        || (ENABLE_TRANSITION_TRACING
            && SymbolFor::from_value(r#type) == Some(REACT_TRACING_MARKER_TYPE))
        || (ENABLE_VIEW_TRANSITION
            && SymbolFor::from_value(r#type) == Some(REACT_VIEW_TRANSITION_TYPE))
    {
        return true;
    }

    if js::type_of(r#type) == "object"
        && let Some(r#type) = r#type
        && (SymbolFor::from_value(js::get(r#type, "$$typeof")) == Some(REACT_LAZY_TYPE)
        || SymbolFor::from_value(js::get(r#type, "$$typeof")) == Some(REACT_MEMO_TYPE)
        || SymbolFor::from_value(js::get(r#type, "$$typeof")) == Some(REACT_CONTEXT_TYPE)
        || SymbolFor::from_value(js::get(r#type, "$$typeof")) == Some(REACT_CONSUMER_TYPE)
        || SymbolFor::from_value(js::get(r#type, "$$typeof")) == Some(REACT_FORWARD_REF_TYPE)
        // This needs to include all possible module reference object
        // types supported by any Flight configuration anywhere since
        // we don't know which Flight build this will end up being used
        // with.
        || SymbolFor::from_value(js::get(r#type, "$$typeof")) == Some(REACT_CLIENT_REFERENCE)
        || !js::strict_equals(js::get(r#type, "getModuleId"), ()))
    {
        return true;
    }

    false
}

pub fn is_context_consumer(object: Option<&'static Unknown>) -> bool {
    SymbolFor::from_value(type_of(object)) == Some(REACT_CONSUMER_TYPE)
}
pub fn is_context_provider(object: Option<&'static Unknown>) -> bool {
    SymbolFor::from_value(type_of(object)) == Some(REACT_CONTEXT_TYPE)
}
pub fn is_element(object: Option<&'static Unknown>) -> bool {
    js::type_of(object) == "object"
        && object.is_some_and(|object| {
            SymbolFor::from_value(js::get(object, "$$typeof")) == Some(REACT_ELEMENT_TYPE)
        })
}
pub fn is_forward_ref(object: Option<&'static Unknown>) -> bool {
    SymbolFor::from_value(type_of(object)) == Some(REACT_FORWARD_REF_TYPE)
}
pub fn is_fragment(object: Option<&'static Unknown>) -> bool {
    SymbolFor::from_value(type_of(object)) == Some(REACT_FRAGMENT_TYPE)
}
pub fn is_lazy(object: Option<&'static Unknown>) -> bool {
    SymbolFor::from_value(type_of(object)) == Some(REACT_LAZY_TYPE)
}
pub fn is_memo(object: Option<&'static Unknown>) -> bool {
    SymbolFor::from_value(type_of(object)) == Some(REACT_MEMO_TYPE)
}
pub fn is_portal(object: Option<&'static Unknown>) -> bool {
    SymbolFor::from_value(type_of(object)) == Some(REACT_PORTAL_TYPE)
}
pub fn is_profiler(object: Option<&'static Unknown>) -> bool {
    SymbolFor::from_value(type_of(object)) == Some(REACT_PROFILER_TYPE)
}
pub fn is_strict_mode(object: Option<&'static Unknown>) -> bool {
    SymbolFor::from_value(type_of(object)) == Some(REACT_STRICT_MODE_TYPE)
}
pub fn is_suspense(object: Option<&'static Unknown>) -> bool {
    SymbolFor::from_value(type_of(object)) == Some(REACT_SUSPENSE_TYPE)
}
pub fn is_suspense_list(object: Option<&'static Unknown>) -> bool {
    SymbolFor::from_value(type_of(object)) == Some(REACT_SUSPENSE_LIST_TYPE)
}
