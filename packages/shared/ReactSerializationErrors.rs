// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::panic::catch_unwind;

use js::{JsObject, Kind, SymbolFor, Unknown, WeakMap};

use crate::environment::__DEV__;
use crate::get_component_name_from_type::LazyInit;
use crate::get_prototype_of::GET_PROTOTYPE_OF;
use crate::is_array::is_array;
use crate::react_feature_flags::ENABLE_VIEW_TRANSITION;
use crate::react_symbols::{
    REACT_ELEMENT_TYPE, REACT_FORWARD_REF_TYPE, REACT_LAZY_TYPE, REACT_MEMO_TYPE,
    REACT_SUSPENSE_LIST_TYPE, REACT_SUSPENSE_TYPE, REACT_VIEW_TRANSITION_TYPE,
};

thread_local! {
    // Used for DEV messages to keep track of which parent rendered some props,
    // in case they error.
    pub static JSX_PROPS_PARENTS: &'static WeakMap<JsObject, &'static Unknown> = js::weak_map::new();
    pub static JSX_CHILDREN_PARENTS: &'static WeakMap<JsObject, &'static Unknown> = js::weak_map::new();
}

fn is_object_prototype(object: Option<&Unknown>) -> bool {
    let Some(object) = object else {
        return false;
    };
    let object_prototype = js::object::prototype;
    if object == object_prototype {
        return true;
    }
    // It might be an object from a different Realm which is
    // still just a plain simple object.
    if GET_PROTOTYPE_OF
        .with(|get_prototype_of| get_prototype_of(Some(object)))
        .is_some()
    {
        return false;
    }
    let names = js::object::get_own_property_names(Some(object));
    for name in &names {
        if !js::has(object_prototype, name) {
            return false;
        }
    }
    true
}

pub fn is_getter(object: Option<&Unknown>, name: &str) -> bool {
    let object_prototype = js::object::prototype;
    let object = match object {
        Some(object) if object != object_prototype => object,
        _ => return false,
    };
    let Some(descriptor) = js::object::get_own_property_descriptor(Some(object), name) else {
        return is_getter(
            GET_PROTOTYPE_OF.with(|get_prototype_of| get_prototype_of(Some(object))),
            name,
        );
    };
    descriptor.get.is_some()
}

pub fn is_simple_object(object: &Unknown) -> bool {
    if !is_object_prototype(
        GET_PROTOTYPE_OF.with(|get_prototype_of| get_prototype_of(Some(object))),
    ) {
        return false;
    }
    let names = js::object::get_own_property_names(Some(object));
    for name in &names {
        let Some(descriptor) = js::object::get_own_property_descriptor(Some(object), name) else {
            return false;
        };
        if !descriptor.enumerable.unwrap_or_default() {
            if (name == "key" || name == "ref") && descriptor.get.is_some() {
                // React adds key and ref getters to props objects to issue warnings.
                // Those getters will not be transferred to the client, but that's ok,
                // so we'll special case them.
                continue;
            }
            return false;
        }
    }
    true
}

pub fn object_name(object: Option<&Unknown>) -> String {
    // $FlowFixMe[method-unbinding]
    let name = js::object::to_string(object).unwrap();
    // Extract 'Object' from '[object Object]':
    js::string::slice(&name, 8.0, Some(f64::from(js::string::length(&name)) - 1.0))
}

fn describe_key_for_error_message(key: &str) -> String {
    let encoded_key = js::json::stringify(key);
    if "\"".to_string() + key + "\"" == encoded_key {
        key.to_string()
    } else {
        encoded_key
    }
}

pub fn describe_value_for_error_message(value: Option<&Unknown>) -> String {
    let Some(value) = value else {
        if let Kind::Null = js::classify(value) {
            return object_name(value);
        }
        return js::string(value);
    };
    match js::classify(Some(value)) {
        Kind::String(value) => js::json::stringify(&if js::string::length(value) <= 10 {
            value.to_string()
        } else {
            js::string::slice(value, 0.0, Some(10.0)) + "..."
        }),
        Kind::Object(_) | Kind::Array(_) => {
            if is_array(Some(value)) {
                return "[...]".to_string();
            }
            if matches!(
                SymbolFor::from_value(js::get(value, "$$typeof")),
                Some(CLIENT_REFERENCE_TAG)
            ) {
                return describe_client_reference(value);
            }
            let name = object_name(Some(value));
            if name == "Object" {
                return "{...}".to_string();
            }
            name
        }
        Kind::Function(_) => {
            if matches!(
                SymbolFor::from_value(js::get(value, "$$typeof")),
                Some(CLIENT_REFERENCE_TAG)
            ) {
                return describe_client_reference(value);
            }
            let name = js::get(value, "displayName")
                .filter(|name| js::truthy(Some(*name)))
                .or_else(|| js::get(value, "name"));
            if js::truthy(name) {
                "function ".to_string() + &js::string(name)
            } else {
                "function".to_string()
            }
        }
        // eslint-disable-next-line react-internal/safe-string-coercion
        _ => js::string(Some(value)),
    }
}

fn describe_element_type(r#type: Option<&Unknown>) -> String {
    let Some(r#type) = r#type else {
        return String::new();
    };
    if let Kind::String(r#type) = js::classify(Some(r#type)) {
        return r#type.to_string();
    }
    match SymbolFor::from_value(Some(r#type)) {
        Some(REACT_SUSPENSE_TYPE) => return "Suspense".to_string(),
        Some(REACT_SUSPENSE_LIST_TYPE) => return "SuspenseList".to_string(),
        Some(REACT_VIEW_TRANSITION_TYPE) if ENABLE_VIEW_TRANSITION => {
            return "ViewTransition".to_string();
        }
        _ => {}
    }
    if let Kind::Object(_) | Kind::Array(_) = js::classify(Some(r#type)) {
        match SymbolFor::from_value(js::get(r#type, "$$typeof")) {
            Some(REACT_FORWARD_REF_TYPE) => {
                return describe_element_type(js::get(r#type, "render"));
            }
            Some(REACT_MEMO_TYPE) => return describe_element_type(js::get(r#type, "type")),
            Some(REACT_LAZY_TYPE) => {
                let lazy_component = r#type;
                let payload = js::get(lazy_component, "_payload");
                let init: &LazyInit = unsafe { js::cast(js::get(lazy_component, "_init")) };
                // Lazy may contain any component type so we recursively resolve it.
                return catch_unwind(|| describe_element_type(init.call(payload).unwrap()))
                    .unwrap_or_default();
            }
            _ => {}
        }
    }
    String::new()
}

const CLIENT_REFERENCE_TAG: SymbolFor = SymbolFor("react.client.reference");

fn describe_client_reference(_ref: &Unknown) -> String {
    "client".to_string()
}

pub fn describe_object_for_error_message(
    object_or_array: &Unknown,
    expanded_name: Option<&str>,
) -> String {
    let obj_kind = object_name(Some(object_or_array));
    if obj_kind != "Object" && obj_kind != "Array" {
        return obj_kind;
    }
    let mut str;
    let mut start = None;
    let mut length = 0;
    let key: &JsObject = unsafe { js::cast(Some(object_or_array)) };
    if let Kind::Array(array) = js::classify(Some(object_or_array)) {
        if __DEV__ && JSX_CHILDREN_PARENTS.with(|parents| parents.has(key)) {
            // Print JSX Children
            let r#type = JSX_CHILDREN_PARENTS.with(|parents| parents.get(key));
            str = "<".to_string() + &describe_element_type(r#type) + ">";
            for (i, &value) in array.iter().enumerate() {
                let substr = if let Kind::String(value) = js::classify(value) {
                    value.to_string()
                } else if let Some(object) = value
                    && let Kind::Object(_) | Kind::Array(_) = js::classify(Some(object))
                {
                    "{".to_string() + &describe_object_for_error_message(object, None) + "}"
                } else {
                    "{".to_string() + &describe_value_for_error_message(value) + "}"
                };
                if expanded_name == Some(i.to_string().as_str()) {
                    start = Some(js::string::length(&str));
                    length = js::string::length(&substr);
                    str += &substr;
                } else if js::string::length(&substr) < 15
                    && js::string::length(&str) + js::string::length(&substr) < 40
                {
                    str += &substr;
                } else {
                    str += "{...}";
                }
            }
            str += &("</".to_string() + &describe_element_type(r#type) + ">");
        } else {
            // Print Array
            str = "[".to_string();
            for (i, &value) in array.iter().enumerate() {
                if i > 0 {
                    str += ", ";
                }
                let substr = if let Some(object) = value
                    && let Kind::Object(_) | Kind::Array(_) = js::classify(Some(object))
                {
                    describe_object_for_error_message(object, None)
                } else {
                    describe_value_for_error_message(value)
                };
                if expanded_name == Some(i.to_string().as_str()) {
                    start = Some(js::string::length(&str));
                    length = js::string::length(&substr);
                    str += &substr;
                } else if js::string::length(&substr) < 10
                    && js::string::length(&str) + js::string::length(&substr) < 40
                {
                    str += &substr;
                } else {
                    str += "...";
                }
            }
            str += "]";
        }
    } else {
        if matches!(
            SymbolFor::from_value(js::get(object_or_array, "$$typeof")),
            Some(REACT_ELEMENT_TYPE)
        ) {
            str = "<".to_string() + &describe_element_type(js::get(object_or_array, "type")) + "/>";
        } else if matches!(
            SymbolFor::from_value(js::get(object_or_array, "$$typeof")),
            Some(CLIENT_REFERENCE_TAG)
        ) {
            return describe_client_reference(object_or_array);
        } else if __DEV__ && JSX_PROPS_PARENTS.with(|parents| parents.has(key)) {
            // Print JSX
            let r#type = JSX_PROPS_PARENTS.with(|parents| parents.get(key));
            str = "<".to_string()
                + Some(describe_element_type(r#type))
                    .filter(|name| !name.is_empty())
                    .as_deref()
                    .unwrap_or("...");
            let object = object_or_array;
            let names = js::object::keys(object);
            for name in &names {
                str += " ";
                str += &(describe_key_for_error_message(name) + "=");
                let value = js::get(object, name);
                let mut substr = if Some(name.as_str()) == expanded_name
                    && let Some(object) = value
                    && let Kind::Object(_) | Kind::Array(_) = js::classify(Some(object))
                {
                    describe_object_for_error_message(object, None)
                } else {
                    describe_value_for_error_message(value)
                };
                if !matches!(js::classify(value), Kind::String(_)) {
                    substr = "{".to_string() + &substr + "}";
                }
                if Some(name.as_str()) == expanded_name {
                    start = Some(js::string::length(&str));
                    length = js::string::length(&substr);
                    str += &substr;
                } else if js::string::length(&substr) < 10
                    && js::string::length(&str) + js::string::length(&substr) < 40
                {
                    str += &substr;
                } else {
                    str += "...";
                }
            }
            str += ">";
        } else {
            // Print Object
            str = "{".to_string();
            let object = object_or_array;
            let names = js::object::keys(object);
            for (i, name) in names.iter().enumerate() {
                if i > 0 {
                    str += ", ";
                }
                str += &(describe_key_for_error_message(name) + ": ");
                let value = js::get(object, name);
                let substr = if let Some(object) = value
                    && let Kind::Object(_) | Kind::Array(_) = js::classify(Some(object))
                {
                    describe_object_for_error_message(object, None)
                } else {
                    describe_value_for_error_message(value)
                };
                if Some(name.as_str()) == expanded_name {
                    start = Some(js::string::length(&str));
                    length = js::string::length(&substr);
                    str += &substr;
                } else if js::string::length(&substr) < 10
                    && js::string::length(&str) + js::string::length(&substr) < 40
                {
                    str += &substr;
                } else {
                    str += "...";
                }
            }
            str += "}";
        }
    }
    if expanded_name.is_none() {
        return str;
    }
    if let Some(start) = start
        && length > 0
    {
        let highlight = " ".repeat(start as usize) + &"^".repeat(length as usize);
        return "\n  ".to_string() + &str + "\n  " + &highlight;
    }
    "\n  ".to_string() + &str
}
