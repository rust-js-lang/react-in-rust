// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{Kind, Unknown};
use react_reconciler::react_fiber_mutation_tracking::track_host_mutation;
use shared::check_string_coercion::check_attribute_string_coercion;
use shared::environment::__DEV__;
use webapi::{Element, Event, EventTargetExt};

use crate::is_attribute_name_safe::is_attribute_name_safe;
use crate::react_dom_component_tree::get_fiber_current_props_from_node;

/**
 * Get the value for a attribute on a node. Only used in DEV for SSR validation.
 * The third argument is used as a hint of what the expected value is. Some
 * attributes have multiple equivalent values.
 */
pub fn get_value_for_attribute(
    node: &Element,
    name: &str,
    expected: Option<&'static Unknown>,
) -> Option<&'static Unknown> {
    if __DEV__ {
        if !is_attribute_name_safe(name) {
            return None;
        }
        if !node.has_attribute(name) {
            // shouldRemoveAttribute
            match js::classify(expected) {
                Kind::Function(_) | Kind::Symbol(_) => return expected,
                Kind::Bool(_) => {
                    let prefix = js::string::slice(&name.to_lowercase(), 0.0, Some(5.0));
                    if prefix != "data-" && prefix != "aria-" {
                        return expected;
                    }
                }
                _ => {}
            }
            return if matches!(js::classify(expected), Kind::Undefined) {
                None
            } else {
                js::null()
            };
        }
        // When CSP is enabled, browsers hide the nonce attribute
        // so we need to access the nonce property directly
        // https://html.spec.whatwg.org/multipage/urls-and-fetching.html#cryptographicnonce
        let is_nonce = name.to_lowercase() == "nonce";
        let value = if is_nonce {
            js::get(node, "nonce")
        } else {
            node.get_attribute(name).map(js::unknown_of)
        };
        if __DEV__ {
            check_attribute_string_coercion(expected, name);
        }
        if js::strict_equals(value, js::concat_text(expected).unwrap()) {
            return expected;
        }
        return value;
    }
    None
}

pub fn get_value_for_attribute_on_custom_component(
    node: &Element,
    name: &str,
    expected: Option<&'static Unknown>,
) -> Option<&'static Unknown> {
    if __DEV__ {
        if !is_attribute_name_safe(name) {
            return None;
        }
        if !node.has_attribute(name) {
            // shouldRemoveAttribute
            match js::classify(expected) {
                Kind::Symbol(_) | Kind::Object(_) | Kind::Array(_) | Kind::Null => {
                    // Symbols and objects are ignored when they're emitted so
                    // it would be expected that they end up not having an attribute.
                    return expected;
                }
                Kind::Function(_) => return expected,
                Kind::Bool(_) if js::strict_equals(expected, false) => return expected,
                _ => {}
            }
            return if matches!(js::classify(expected), Kind::Undefined) {
                None
            } else {
                js::null()
            };
        }
        // When CSP is enabled, browsers hide the nonce attribute
        // so we need to access the nonce property directly
        // https://html.spec.whatwg.org/multipage/urls-and-fetching.html#cryptographicnonce
        let is_nonce = name.to_lowercase() == "nonce";
        let value = if is_nonce {
            js::get(node, "nonce")
        } else {
            node.get_attribute(name).map(js::unknown_of)
        };

        if js::strict_equals(value, "") && js::strict_equals(expected, true) {
            return Some(js::unknown_of(true));
        }

        if __DEV__ {
            check_attribute_string_coercion(expected, name);
        }
        if js::strict_equals(value, js::concat_text(expected).unwrap()) {
            return expected;
        }
        return value;
    }
    None
}

pub fn set_value_for_attribute(node: &Element, name: &str, value: Option<&Unknown>) {
    if is_attribute_name_safe(name) {
        // If the prop isn't in the special list, treat it as a simple attribute.
        // shouldRemoveAttribute
        if matches!(js::classify(value), Kind::Null) {
            node.remove_attribute(name);
            return;
        }
        match js::classify(value) {
            Kind::Undefined | Kind::Function(_) | Kind::Symbol(_) => {
                node.remove_attribute(name);
                return;
            }
            Kind::Bool(_) => {
                let prefix = js::string::slice(&name.to_lowercase(), 0.0, Some(5.0));
                if prefix != "data-" && prefix != "aria-" {
                    node.remove_attribute(name);
                    return;
                }
            }
            _ => {}
        }
        if __DEV__ {
            check_attribute_string_coercion(value, name);
        }
        node.set_attribute(name, value);
    }
}

pub fn set_value_for_known_attribute(node: &Element, name: &str, value: Option<&Unknown>) {
    if matches!(js::classify(value), Kind::Null) {
        node.remove_attribute(name);
        return;
    }
    match js::classify(value) {
        Kind::Undefined | Kind::Function(_) | Kind::Symbol(_) | Kind::Bool(_) => {
            node.remove_attribute(name);
            return;
        }
        _ => {}
    }
    if __DEV__ {
        check_attribute_string_coercion(value, name);
    }
    node.set_attribute(name, value);
}

pub fn set_value_for_namespaced_attribute(
    node: &Element,
    namespace: &str,
    name: &str,
    value: Option<&Unknown>,
) {
    if matches!(js::classify(value), Kind::Null) {
        node.remove_attribute(name);
        return;
    }
    match js::classify(value) {
        Kind::Undefined | Kind::Function(_) | Kind::Symbol(_) | Kind::Bool(_) => {
            node.remove_attribute(name);
            return;
        }
        _ => {}
    }
    if __DEV__ {
        check_attribute_string_coercion(value, name);
    }
    node.set_attribute_ns(namespace, name, value);
}

pub fn set_value_for_property_on_custom_component(
    node: &Element,
    name: &str,
    value: Option<&'static Unknown>,
) {
    if js::string::get(name, 0.0) == Some("o") && js::string::get(name, 1.0) == Some("n") {
        let use_capture = name.ends_with("Capture");
        let event_name = js::string::slice(
            name,
            2.0,
            if use_capture {
                Some(f64::from(js::string::length(name)) - 7.0)
            } else {
                None
            },
        );

        let prev_props = get_fiber_current_props_from_node(js::unknown(node));
        // $FlowFixMe[invalid-computed-prop]
        let prev_value = match prev_props {
            Some(prev_props) => js::get(prev_props, name),
            None => js::null(),
        };
        if let Kind::Function(_) = js::classify(prev_value) {
            node.remove_event_listener_named(
                &event_name,
                unsafe { js::cast::<&'static dyn Fn(&Event)>(prev_value) },
                Some(&use_capture),
            );
        }
        if let Kind::Function(_) = js::classify(value) {
            if !matches!(js::classify(prev_value), Kind::Function(_))
                && !matches!(js::classify(prev_value), Kind::Null)
            {
                // If we previously assigned a non-function type into this node, then
                // remove it when switching to event listener mode.
                if js::has(node, name) {
                    js::set(node, name, js::null::<&Unknown>());
                } else if node.has_attribute(name) {
                    node.remove_attribute(name);
                }
            }
            // $FlowFixMe[incompatible-type] value can't be casted to EventListener.
            node.add_event_listener_named(
                &event_name,
                unsafe { js::cast::<&'static dyn Fn(&Event)>(value) },
                Some(&use_capture),
            );
            return;
        }
    }

    track_host_mutation();

    if js::has(node, name) {
        js::set(node, name, value);
        return;
    }

    if js::strict_equals(value, true) {
        node.set_attribute(name, "");
        return;
    }

    // From here, it's the same as any attribute
    set_value_for_attribute(node, name, value);
}
