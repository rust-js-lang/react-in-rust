// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{Kind, SymbolFor, Unknown, function};

use crate::get_component_name_from_type::get_component_name_from_type;
use crate::has_own_property::HAS_OWN_PROPERTY;
use crate::react_flight_property_access::OMITTED_PROP_ERROR;
use crate::react_symbols::REACT_ELEMENT_TYPE;

#[derive(Clone, Copy, PartialEq)]
enum ArrayKind {
    Empty,
    Complex,
    // Primitive values only that are accepted by JSON.stringify
    Primitive,
    // Tuple arrays of string and value (like Headers, Map, etc)
    Entries,
}

// Showing wider objects in the devtools is not useful.
const OBJECT_WIDTH_LIMIT: usize = 100;

fn get_array_kind(array: &[Option<&Unknown>]) -> ArrayKind {
    let mut kind = ArrayKind::Empty;
    for &value in array.iter().take(OBJECT_WIDTH_LIMIT) {
        match js::classify(value) {
            Kind::Array(&[first, _]) if matches!(js::classify(first), Kind::String(_)) => {
                // Key value tuple
                if kind != ArrayKind::Empty && kind != ArrayKind::Entries {
                    return ArrayKind::Complex;
                }
                kind = ArrayKind::Entries;
            }
            Kind::Object(_) | Kind::Array(_) => return ArrayKind::Complex,
            Kind::Function(_) => return ArrayKind::Complex,
            Kind::String(value) if value.encode_utf16().count() > 50 => {
                return ArrayKind::Complex;
            }
            _ if kind != ArrayKind::Empty && kind != ArrayKind::Primitive => {
                return ArrayKind::Complex;
            }
            Kind::BigInt(_) => return ArrayKind::Complex,
            _ => kind = ArrayKind::Primitive,
        }
    }
    kind
}

pub fn add_object_to_properties(
    object: &Unknown,
    properties: &mut Vec<(String, String)>,
    indent: usize,
    prefix: &str,
) {
    if js::array_buffer::is_view(Some(object)) {
        // Typed arrays (e.g. Uint8Array, Float32Array) can hold millions of
        // elements. Enumerating them with for...in forces the engine to
        // materialize a key for every index, which can freeze the page. Their
        // contents aren't useful to show here so we skip them. DataView has no
        // enumerable properties to show anyway.
        return;
    }
    let mut added_properties = 0;
    for key in js::keys_in(object) {
        if HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(object, &key))
            && !key.starts_with('_')
        {
            added_properties += 1;
            let value = js::get(object, &key);
            add_value_to_properties(&key, value, properties, indent, prefix);
            if added_properties >= OBJECT_WIDTH_LIMIT {
                properties.push((
                    format!(
                        "{prefix}{}Only {OBJECT_WIDTH_LIMIT} properties are shown. React will not log more properties of this object.",
                        "\u{a0}\u{a0}".repeat(indent)
                    ),
                    String::new(),
                ));
                break;
            }
        }
    }
}

fn read_react_element_typeof(value: &Unknown) -> Option<&'static Unknown> {
    // Prevents dotting into $$typeof in opaque origin windows.
    if js::has(value, "$$typeof")
        && HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(value, "$$typeof"))
    {
        js::get(value, "$$typeof")
    } else {
        None
    }
}

pub fn add_value_to_properties(
    property_name: &str,
    value: Option<&Unknown>,
    properties: &mut Vec<(String, String)>,
    indent: usize,
    prefix: &str,
) {
    let desc = match js::classify(value) {
        Kind::Null => "null".to_string(),
        Kind::Object(_) | Kind::Array(_) if let Some(value) = value => 'desc: {
            if matches!(
                SymbolFor::from_value(read_react_element_typeof(value)),
                Some(REACT_ELEMENT_TYPE)
            ) {
                // JSX
                let type_name = get_component_name_from_type(js::get(value, "type"))
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| "\u{2026}".to_string());
                let key = js::get(value, "key");
                let props: &Unknown = unsafe { js::cast(js::get(value, "props")) };
                let props_keys = js::object::keys(props);
                let props_length = props_keys.len();
                if key.is_none() && props_length == 0 {
                    break 'desc "<".to_string() + &type_name + " />";
                }
                if indent < 3 || (props_length == 1 && props_keys[0] == "children" && key.is_none())
                {
                    break 'desc "<".to_string() + &type_name + " \u{2026} />";
                }
                properties.push((
                    prefix.to_string() + &"\u{a0}\u{a0}".repeat(indent) + property_name,
                    "<".to_string() + &type_name,
                ));
                if !matches!(js::classify(key), Kind::Null) {
                    add_value_to_properties("key", key, properties, indent + 1, prefix);
                }
                let mut has_children = false;
                let mut added_properties = 0;
                for prop_key in js::keys_in(props) {
                    added_properties += 1;
                    if prop_key == "children" {
                        if let Some(children) = js::get(props, "children")
                            && !matches!(js::classify(Some(children)), Kind::Array(children) if children.is_empty())
                        {
                            has_children = true;
                        }
                    } else if HAS_OWN_PROPERTY
                        .with(|has_own_property| has_own_property.call(props, &prop_key))
                        && !prop_key.starts_with('_')
                    {
                        add_value_to_properties(
                            &prop_key,
                            js::get(props, &prop_key),
                            properties,
                            indent + 1,
                            prefix,
                        );
                    }

                    if added_properties >= OBJECT_WIDTH_LIMIT {
                        break;
                    }
                }
                properties.push((
                    String::new(),
                    if has_children {
                        ">\u{2026}</".to_string() + &type_name + ">"
                    } else {
                        "/>".to_string()
                    },
                ));
                return;
            }
            // $FlowFixMe[method-unbinding]
            let object_to_string = js::object::to_string(Some(value)).unwrap();
            let mut object_name = js::string::slice(
                &object_to_string,
                8.0,
                Some(f64::from(js::string::length(&object_to_string)) - 1.0),
            );
            if js::array_buffer::is_view(Some(value)) {
                // Typed arrays can hold millions of elements. Showing the type and
                // length is more useful than enumerating every index (which is also
                // prohibitively slow, see addObjectToProperties). DataView is the
                // only view without a length; show just its type.
                break 'desc match js::classify(js::get(value, "length")) {
                    Kind::Number(length) => format!("{object_name}({})", js::shown(length)),
                    _ => object_name,
                };
            }
            if object_name == "Array"
                && let Kind::Array(array) = js::classify(Some(value))
            {
                let did_truncate = array.len() > OBJECT_WIDTH_LIMIT;
                let kind = get_array_kind(array);
                if kind == ArrayKind::Primitive || kind == ArrayKind::Empty {
                    break 'desc if did_truncate {
                        js::json::stringify_with(
                            js::unknown(
                                &[&array[..OBJECT_WIDTH_LIMIT], &[Some(js::unknown("…"))]].concat(),
                            ),
                            None,
                            0,
                        )
                    } else {
                        js::json::stringify_with(value, None, 0)
                    }
                    .unwrap()
                    .unwrap();
                } else if kind == ArrayKind::Entries {
                    properties.push((
                        prefix.to_string() + &"\u{a0}\u{a0}".repeat(indent) + property_name,
                        String::new(),
                    ));
                    for &entry in array.iter().take(OBJECT_WIDTH_LIMIT) {
                        if let Kind::Array(&[name, value]) = js::classify(entry)
                            && let Kind::String(name) = js::classify(name)
                        {
                            add_value_to_properties(name, value, properties, indent + 1, prefix);
                        }
                    }
                    if did_truncate {
                        add_value_to_properties(
                            &OBJECT_WIDTH_LIMIT.to_string(),
                            Some(js::unknown("…")),
                            properties,
                            indent + 1,
                            prefix,
                        );
                    }
                    return;
                }
            }
            if object_name == "Promise" {
                match js::classify(js::get(value, "status")) {
                    Kind::String("fulfilled") => {
                        // Print the inner value
                        let idx = properties.len();
                        add_value_to_properties(
                            property_name,
                            js::get(value, "value"),
                            properties,
                            indent,
                            prefix,
                        );
                        if properties.len() > idx {
                            // Wrap the value or type in Promise descriptor.
                            let inserted_entry = &mut properties[idx];
                            inserted_entry.1 = "Promise<".to_string()
                                + if inserted_entry.1.is_empty() {
                                    "Object"
                                } else {
                                    &inserted_entry.1
                                }
                                + ">";
                            return;
                        }
                    }
                    Kind::String("rejected") => {
                        // Print the inner error
                        let idx = properties.len();
                        add_value_to_properties(
                            property_name,
                            js::get(value, "reason"),
                            properties,
                            indent,
                            prefix,
                        );
                        if properties.len() > idx {
                            // Wrap the value or type in Promise descriptor.
                            let inserted_entry = &mut properties[idx];
                            inserted_entry.1 =
                                "Rejected Promise<".to_string() + &inserted_entry.1 + ">";
                            return;
                        }
                    }
                    _ => {}
                }
                properties.push((
                    "\u{a0}\u{a0}".repeat(indent) + property_name,
                    "Promise".to_string(),
                ));
                return;
            }
            if object_name == "Object"
                && let Some(proto) = js::object::get_prototype_of(Some(value))
                && let Kind::Function(constructor) = js::classify(js::get(proto, "constructor"))
            {
                object_name = function::name(constructor);
            }
            properties.push((
                prefix.to_string() + &"\u{a0}\u{a0}".repeat(indent) + property_name,
                if object_name == "Object" {
                    if indent < 3 {
                        String::new()
                    } else {
                        "\u{2026}".to_string()
                    }
                } else {
                    object_name
                },
            ));
            if indent < 3 {
                add_object_to_properties(value, properties, indent + 1, prefix);
            }
            return;
        }
        Kind::Function(_) => match js::classify(value.and_then(|value| js::get(value, "name"))) {
            Kind::String(function_name) if !function_name.is_empty() => {
                function_name.to_string() + "() {}"
            }
            // e.g. proxied functions or classes with a static property "name" that's not a string
            _ => "() => {}".to_string(),
        },
        Kind::String(value) => {
            if value == OMITTED_PROP_ERROR {
                "\u{2026}".to_string() // ellipsis
            } else {
                js::json::stringify(&if value.encode_utf16().count() >= 1024 {
                    js::string::slice(value, 0.0, Some(1023.0)) + "\u{2026}" // ellipsis
                } else {
                    value.to_string()
                })
            }
        }
        Kind::Undefined => "undefined".to_string(),
        Kind::Bool(value) => if value { "true" } else { "false" }.to_string(),
        // eslint-disable-next-line react-internal/safe-string-coercion
        _ => js::string(value),
    };
    properties.push((
        prefix.to_string() + &"\u{a0}\u{a0}".repeat(indent) + property_name,
        desc,
    ));
}

const REMOVED: &str = "-\u{a0}";
const ADDED: &str = "+\u{a0}";
const UNCHANGED: &str = "\u{2007}\u{a0}";

pub fn add_object_diff_to_properties(
    prev: &Unknown,
    next: &Unknown,
    properties: &mut Vec<(String, String)>,
    indent: usize,
) -> bool {
    // Note: We diff even non-owned properties here but things that are shared end up just the same.
    // If a property is added or removed, we just emit the property name and omit the value it had.
    // Mainly for performance. We need to minimize to only relevant information.
    let mut is_deeply_equal = true;
    for (prev_properties_checked, key) in js::keys_in(prev).enumerate() {
        if prev_properties_checked > OBJECT_WIDTH_LIMIT {
            properties.push((
                format!(
                    "Previous object has more than {OBJECT_WIDTH_LIMIT} properties. React will not attempt to diff objects with too many properties."
                ),
                String::new(),
            ));
            is_deeply_equal = false;
            break;
        }

        if !js::has(next, &key) {
            properties.push((
                REMOVED.to_string() + &"\u{a0}\u{a0}".repeat(indent) + &key,
                "\u{2026}".to_string(),
            ));
            is_deeply_equal = false;
        }
    }

    let mut next_properties_checked = 0;
    for key in js::keys_in(next) {
        if next_properties_checked > OBJECT_WIDTH_LIMIT {
            properties.push((
                format!(
                    "Next object has more than {OBJECT_WIDTH_LIMIT} properties. React will not attempt to diff objects with too many properties."
                ),
                String::new(),
            ));
            is_deeply_equal = false;
            break;
        }

        if js::has(prev, &key) {
            let prev_value = js::get(prev, &key);
            let next_value = js::get(next, &key);
            if !js::strict_equals(prev_value, next_value) {
                if indent == 0 && key == "children" {
                    // Omit any change inside the top level children prop since it's expected to change
                    // with any change to children of the component and their props will be logged
                    // elsewhere but still mark it as a cause of render.
                    let line = "\u{a0}\u{a0}".repeat(indent) + &key;
                    properties.extend([
                        (REMOVED.to_string() + &line, "\u{2026}".to_string()),
                        (ADDED.to_string() + &line, "\u{2026}".to_string()),
                    ]);
                    is_deeply_equal = false;
                    continue;
                }
                if indent >= 3 {
                    // Just fallthrough to print the two values if we're deep.
                    // This will skip nested properties of the objects.
                } else if let Some(prev_object) = prev_value
                    && let Some(next_object) = next_value
                    && let Kind::Object(_) | Kind::Array(_) = js::classify(prev_value)
                    && let Kind::Object(_) | Kind::Array(_) = js::classify(next_value)
                    && read_react_element_typeof(prev_object)
                        == read_react_element_typeof(next_object)
                {
                    if matches!(
                        SymbolFor::from_value(read_react_element_typeof(next_object)),
                        Some(REACT_ELEMENT_TYPE)
                    ) {
                        if js::get(prev_object, "type") == js::get(next_object, "type")
                            && js::get(prev_object, "key") == js::get(next_object, "key")
                        {
                            // If the only thing that has changed is the props of a nested element, then
                            // we omit the props because it is likely to be represented as a diff elsewhere.
                            let type_name =
                                get_component_name_from_type(js::get(next_object, "type"))
                                    .filter(|name| !name.is_empty())
                                    .unwrap_or_else(|| "\u{2026}".to_string());
                            let line = "\u{a0}\u{a0}".repeat(indent) + &key;
                            let desc = "<".to_string() + &type_name + " \u{2026} />";
                            properties.extend([
                                (REMOVED.to_string() + &line, desc.clone()),
                                (ADDED.to_string() + &line, desc),
                            ]);
                            is_deeply_equal = false;
                            continue;
                        }
                    } else {
                        // $FlowFixMe[method-unbinding]
                        let prev_kind = js::object::to_string(prev_value).unwrap();
                        // $FlowFixMe[method-unbinding]
                        let next_kind = js::object::to_string(next_value).unwrap();
                        if prev_kind == next_kind
                            && (next_kind == "[object Object]" || next_kind == "[object Array]")
                        {
                            // Diff nested object
                            properties.push((
                                UNCHANGED.to_string() + &"\u{a0}\u{a0}".repeat(indent) + &key,
                                if next_kind == "[object Array]" {
                                    "Array".to_string()
                                } else {
                                    String::new()
                                },
                            ));
                            let prev_length = properties.len();
                            let nested_equal = add_object_diff_to_properties(
                                prev_object,
                                next_object,
                                properties,
                                indent + 1,
                            );
                            if !nested_equal {
                                is_deeply_equal = false;
                            } else if prev_length == properties.len() {
                                // Nothing notably changed inside the nested object. So this is only a change in reference
                                // equality. Let's note it.
                                properties[prev_length - 1].1 =
                                    "Referentially unequal but deeply equal objects. Consider memoization."
                                        .to_string();
                            }
                            continue;
                        }
                    }
                } else if let Kind::Function(prev_function) = js::classify(prev_value)
                    && let Kind::Function(next_function) = js::classify(next_value)
                    && function::name(prev_function) == function::name(next_function)
                    && function::length(prev_function) == function::length(next_function)
                {
                    // $FlowFixMe[method-unbinding]
                    let prev_src = function::to_string(prev_function);
                    // $FlowFixMe[method-unbinding]
                    let next_src = function::to_string(next_function);
                    if prev_src == next_src {
                        // This looks like it might be the same function but different closures.
                        let desc = if function::name(next_function).is_empty() {
                            "() => {}".to_string()
                        } else {
                            function::name(next_function) + "() {}"
                        };
                        properties.push((
                            UNCHANGED.to_string() + &"\u{a0}\u{a0}".repeat(indent) + &key,
                            desc + " Referentially unequal function closure. Consider memoization.",
                        ));
                        continue;
                    }
                }

                // Otherwise, emit the change in property and the values.
                add_value_to_properties(&key, prev_value, properties, indent, REMOVED);
                add_value_to_properties(&key, next_value, properties, indent, ADDED);
                is_deeply_equal = false;
            }
        } else {
            properties.push((
                ADDED.to_string() + &"\u{a0}\u{a0}".repeat(indent) + &key,
                "\u{2026}".to_string(),
            ));
            is_deeply_equal = false;
        }

        next_properties_checked += 1;
    }
    is_deeply_equal
}
