// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use js::{Kind, RegExp, SymbolFor, Unknown, json, object, string};
use shared::assign::assign;
use shared::environment::__DEV__;
use shared::get_component_name_from_type::get_component_name_from_type;
use shared::is_array::is_array;
use shared::react_feature_flags::ENABLE_SRC_OBJECT;
use shared::react_symbols::REACT_ELEMENT_TYPE;
use webapi::console;

use crate::react_internal_types::Fiber;
use crate::react_work_tags::{
    ACTIVITY_COMPONENT, CLASS_COMPONENT, FORWARD_REF, FUNCTION_COMPONENT, HOST_COMPONENT,
    HOST_HOISTABLE, HOST_SINGLETON, HOST_TEXT, LAZY_COMPONENT, SIMPLE_MEMO_COMPONENT,
    SUSPENSE_COMPONENT, SUSPENSE_LIST_COMPONENT,
};

/// What the server rendered where a fiber is.
#[derive(Clone, Copy)]
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum ServerProps<'a> {
    /// Nothing known of it: the node is only a reference for context.
    #[cfg_attr(rust_js, rust_js::undefined)]
    Unknown,
    /// No matching server node.
    #[cfg_attr(rust_js, rust_js::null)]
    Unmatched,
    /// A text node's text.
    Text(&'a str),
    /// An element's props.
    #[cfg_attr(rust_js, rust_js::otherwise)]
    Props(&'a Unknown),
}

/// An element the server rendered that the client didn't.
pub struct ServerElement {
    pub r#type: String,
    pub props: &'static Unknown,
}

/// A node the server rendered after the client's last.
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum ServerTailNode {
    Text(String),
    #[cfg_attr(rust_js, rust_js::otherwise)]
    Element(ServerElement),
}

pub struct HydrationDiffNode {
    pub fiber: Rc<Fiber>,
    pub children: Vec<HydrationDiffNode>,
    pub server_props: ServerProps<'static>, // null means no matching server node
    pub server_tail: Vec<ServerTailNode>,
    pub distance_from_leaf: i32,
}

#[cfg_attr(rust_js, rust_js::name = "maxRowLength")]
const MAX_ROW_LENGTH: f64 = 120.0;
#[cfg_attr(rust_js, rust_js::name = "idealDepth")]
const IDEAL_DEPTH: i32 = 15;

fn find_notable_node(node: &HydrationDiffNode, indent: i32) -> &HydrationDiffNode {
    if matches!(node.server_props, ServerProps::Unknown)
        && node.server_tail.is_empty()
        && node.children.len() == 1
        && node.distance_from_leaf > 3
        && node.distance_from_leaf > IDEAL_DEPTH - indent
    {
        // This is not an interesting node for contextual purposes so we can skip it.
        let child = &node.children[0];
        return find_notable_node(child, indent);
    }
    node
}

fn indentation(indent: i32) -> String {
    "  ".to_owned() + &"  ".repeat(indent as usize)
}

fn added(indent: i32) -> String {
    "+ ".to_owned() + &"  ".repeat(indent as usize)
}

fn removed(indent: i32) -> String {
    "- ".to_owned() + &"  ".repeat(indent as usize)
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn describe_fiber_type(fiber: &Fiber) -> Option<String> {
    match fiber.tag.get() {
        HOST_HOISTABLE | HOST_SINGLETON | HOST_COMPONENT => unsafe { js::cast(fiber.r#type.get()) },
        LAZY_COMPONENT => Some("Lazy".to_string()),
        ACTIVITY_COMPONENT => Some("Activity".to_string()),
        SUSPENSE_COMPONENT => Some("Suspense".to_string()),
        SUSPENSE_LIST_COMPONENT => Some("SuspenseList".to_string()),
        FUNCTION_COMPONENT | SIMPLE_MEMO_COMPONENT => {
            let r#fn = fiber.r#type.get();
            unsafe {
                js::cast(
                    js::get(r#fn, "displayName")
                        .filter(|name| js::truthy(Some(*name)))
                        .or_else(|| js::get(r#fn, "name"))
                        .filter(|name| js::truthy(Some(*name))),
                )
            }
        }
        FORWARD_REF => {
            let render = js::get(fiber.r#type.get(), "render");
            unsafe {
                js::cast(
                    js::get(render, "displayName")
                        .filter(|name| js::truthy(Some(*name)))
                        .or_else(|| js::get(render, "name"))
                        .filter(|name| js::truthy(Some(*name))),
                )
            }
        }
        CLASS_COMPONENT => {
            let ctr = fiber.r#type.get();
            unsafe {
                js::cast(
                    js::get(ctr, "displayName")
                        .filter(|name| js::truthy(Some(*name)))
                        .or_else(|| js::get(ctr, "name"))
                        .filter(|name| js::truthy(Some(*name))),
                )
            }
        }
        _ => {
            // Skip
            None
        }
    }
}

thread_local! {
    static NEEDS_ESCAPING: &'static RegExp = js::reg_exp::new(r#"["'&<>\n\t]|^\s|\s$"#, "");
}

fn describe_text_node(content: &str, max_length: f64) -> String {
    if NEEDS_ESCAPING.with(|needs_escaping| needs_escaping.test(content)) {
        let encoded = json::stringify(content);
        if f64::from(string::length(&encoded)) > max_length - 2.0 {
            if max_length < 8.0 {
                return "{\"...\"}".to_string();
            }
            return "{".to_owned() + &string::slice(&encoded, 0.0, Some(max_length - 7.0)) + "...\"}";
        }
        "{".to_owned() + &encoded + "}"
    } else {
        if f64::from(string::length(content)) > max_length {
            if max_length < 5.0 {
                return "{\"...\"}".to_string();
            }
            return string::slice(content, 0.0, Some(max_length - 3.0)) + "...";
        }
        content.to_string()
    }
}

fn describe_text_diff(client_text: &str, server_props: ServerProps<'_>, indent: i32) -> String {
    let max_length = MAX_ROW_LENGTH - f64::from(indent) * 2.0;
    match server_props {
        ServerProps::Unmatched => {
            added(indent) + &describe_text_node(client_text, max_length) + "\n"
        }
        ServerProps::Text(server_text) => {
            let mut server_text = server_text.to_string();
            let mut client_text = client_text.to_string();
            let mut first_diff = 0;
            while first_diff < string::length(&server_text) && first_diff < string::length(&client_text)
            {
                if string::char_code_at(&server_text, first_diff)
                    != string::char_code_at(&client_text, first_diff)
                {
                    break;
                }
                first_diff += 1;
            }
            if f64::from(first_diff) > max_length - 8.0 && first_diff > 10 {
                // The first difference between the two strings would be cut off, so cut off in
                // the beginning instead.
                client_text = "...".to_owned()
                    + &string::slice(&client_text, f64::from(first_diff) - 8.0, None);
                server_text = "...".to_owned()
                    + &string::slice(&server_text, f64::from(first_diff) - 8.0, None);
            }
            added(indent)
                + &describe_text_node(&client_text, max_length)
                + "\n"
                + &removed(indent)
                + &describe_text_node(&server_text, max_length)
                + "\n"
        }
        _ => indentation(indent) + &describe_text_node(client_text, max_length) + "\n",
    }
}

fn object_name(object: &Unknown) -> String {
    // $FlowFixMe[method-unbinding]
    let name = object::to_string(Some(object)).unwrap();
    js::reg_exp::replace(
        &name,
        js::reg_exp::new(r"^\[object (.*)\]$", ""),
        |_m: &str, p0: Option<&str>| p0.unwrap_or_default().to_string(),
    )
}

fn describe_value(value: Option<&Unknown>, mut max_length: f64) -> String {
    match js::type_of(value).as_str() {
        "string" => {
            let encoded = json::stringify(unsafe { js::cast(value) });
            if f64::from(string::length(&encoded)) > max_length {
                if max_length < 5.0 {
                    return "\"...\"".to_string();
                }
                return string::slice(&encoded, 0.0, Some(max_length - 4.0)) + "...\"";
            }
            encoded
        }
        "object" => {
            let Some(value) = value else {
                return "null".to_string();
            };
            if is_array(Some(value)) {
                return "[...]".to_string();
            }
            if SymbolFor::from_value(js::get(value, "$$typeof")) == Some(REACT_ELEMENT_TYPE) {
                let r#type = get_component_name_from_type(js::get(value, "type"));
                return match r#type {
                    Some(r#type) if !r#type.is_empty() => "<".to_owned() + &r#type + ">",
                    _ => "<...>".to_string(),
                };
            }
            let name = object_name(value);
            if name == "Object" {
                let mut properties = String::new();
                max_length -= 2.0;
                for mut prop_name in js::keys_in(value) {
                    if !object::has_own_property.call(value, prop_name.as_str()) {
                        continue;
                    }
                    let json_prop_name = json::stringify(&prop_name);
                    if json_prop_name != "\"".to_owned() + &prop_name + "\"" {
                        prop_name = json_prop_name;
                    }
                    max_length -= f64::from(string::length(&prop_name)) - 2.0;
                    let prop_value = describe_value(
                        js::get(value, prop_name.as_str()),
                        if max_length < 15.0 { max_length } else { 15.0 },
                    );
                    max_length -= f64::from(string::length(&prop_value));
                    if max_length < 0.0 {
                        properties += if properties.is_empty() { "..." } else { ", ..." };
                        break;
                    }
                    properties += &((if properties.is_empty() { "" } else { "," }).to_owned()
                        + &prop_name
                        + ":"
                        + &prop_value);
                }
                return "{".to_owned() + &properties + "}";
            } else if ENABLE_SRC_OBJECT.with(|&enabled| enabled) && (name == "Blob" || name == "File") {
                return name + ":" + &js::concat_text(js::get(value, "type")).unwrap();
            }
            name
        }
        "function" => {
            let name = js::get(value, "displayName")
                .filter(|name| js::truthy(Some(*name)))
                .or_else(|| js::get(value, "name"));
            if js::truthy(name) {
                "function ".to_owned() + &js::concat_text(name).unwrap()
            } else {
                "function".to_string()
            }
        }
        _ => {
            // eslint-disable-next-line react-internal/safe-string-coercion
            js::string(value)
        }
    }
}

fn describe_prop_value(value: Option<&Unknown>, max_length: f64) -> String {
    if let Kind::String(value) = js::classify(value)
        && !NEEDS_ESCAPING.with(|needs_escaping| needs_escaping.test(value))
    {
        if f64::from(string::length(value)) > max_length - 2.0 {
            if max_length < 5.0 {
                return "\"...\"".to_string();
            }
            return "\"".to_owned() + &string::slice(value, 0.0, Some(max_length - 5.0)) + "...\"";
        }
        return "\"".to_owned() + value + "\"";
    }
    "{".to_owned() + &describe_value(value, max_length - 2.0) + "}"
}

fn describe_collapsed_element(r#type: &str, props: &Unknown, indent: i32) -> String {
    // This function tries to fit the props into a single line for non-essential elements.
    // We also ignore children because we're not going deeper.

    let mut max_length =
        MAX_ROW_LENGTH - f64::from(indent) * 2.0 - f64::from(string::length(r#type)) - 2.0;

    let mut content = String::new();

    for prop_name in js::keys_in(props) {
        if !object::has_own_property.call(props, prop_name.as_str()) {
            continue;
        }
        if prop_name == "children" {
            // Ignored.
            continue;
        }
        let prop_value = describe_prop_value(js::get(props, prop_name.as_str()), 15.0);
        max_length -=
            f64::from(string::length(&prop_name)) + f64::from(string::length(&prop_value)) + 2.0;
        if max_length < 0.0 {
            content += " ...";
            break;
        }
        content += &(" ".to_owned() + &prop_name + "=" + &prop_value);
    }

    indentation(indent) + "<" + r#type + &content + ">\n"
}

fn describe_expanded_element(r#type: &str, props: &Unknown, row_prefix: &str) -> String {
    // This function tries to fit the props into a single line for non-essential elements.
    // We also ignore children because we're not going deeper.

    let mut remaining_row_length = MAX_ROW_LENGTH
        - f64::from(string::length(row_prefix))
        - f64::from(string::length(r#type));

    // We add the properties to a set so we can choose later whether we'll put it on one
    // line or multiple lines.

    let mut properties = Vec::new();

    for prop_name in js::keys_in(props) {
        if !object::has_own_property.call(props, prop_name.as_str()) {
            continue;
        }
        if prop_name == "children" {
            // Ignored.
            continue;
        }
        let max_length = MAX_ROW_LENGTH
            - f64::from(string::length(row_prefix))
            - f64::from(string::length(&prop_name))
            - 1.0;
        let prop_value = describe_prop_value(js::get(props, prop_name.as_str()), max_length);
        remaining_row_length -=
            f64::from(string::length(&prop_name)) + f64::from(string::length(&prop_value)) + 2.0;
        properties.push(prop_name + "=" + &prop_value);
    }

    if properties.is_empty() {
        row_prefix.to_owned() + "<" + r#type + ">\n"
    } else if remaining_row_length > 0.0 {
        // We can fit all on one row.
        row_prefix.to_owned() + "<" + r#type + " " + &properties.join(" ") + ">\n"
    } else {
        // Split into one row per property:
        row_prefix.to_owned()
            + "<"
            + r#type
            + "\n"
            + row_prefix
            + "  "
            + &properties.join(&("\n".to_owned() + row_prefix + "  "))
            + "\n"
            + row_prefix
            + ">\n"
    }
}

fn describe_properties_diff(
    client_object: &Unknown,
    server_object: &Unknown,
    indent: i32,
) -> String {
    let mut properties = String::new();
    let remaining_server_properties =
        assign(js::unknown(js::dict::new::<Option<&Unknown>>()), (server_object,));
    for prop_name in js::keys_in(client_object) {
        if !object::has_own_property.call(client_object, prop_name.as_str()) {
            continue;
        }
        js::delete(remaining_server_properties, prop_name.as_str());
        let max_length = MAX_ROW_LENGTH
            - f64::from(indent) * 2.0
            - f64::from(string::length(&prop_name))
            - 2.0;
        let client_value = js::get(client_object, prop_name.as_str());
        let client_prop_value = describe_value(client_value, max_length);
        if object::has_own_property.call(server_object, prop_name.as_str()) {
            let server_value = js::get(server_object, prop_name.as_str());
            let server_prop_value = describe_value(server_value, max_length);
            properties += &(added(indent) + &prop_name + ": " + &client_prop_value + "\n");
            properties += &(removed(indent) + &prop_name + ": " + &server_prop_value + "\n");
        } else {
            properties += &(added(indent) + &prop_name + ": " + &client_prop_value + "\n");
        }
    }
    for prop_name in js::keys_in(remaining_server_properties) {
        if !object::has_own_property.call(remaining_server_properties, prop_name.as_str()) {
            continue;
        }
        let max_length = MAX_ROW_LENGTH
            - f64::from(indent) * 2.0
            - f64::from(string::length(&prop_name))
            - 2.0;
        let server_value = js::get(remaining_server_properties, prop_name.as_str());
        let server_prop_value = describe_value(server_value, max_length);
        properties += &(removed(indent) + &prop_name + ": " + &server_prop_value + "\n");
    }
    properties
}

fn is_text_like(value: Option<&Unknown>) -> bool {
    matches!(
        js::classify(value),
        Kind::String(_) | Kind::Number(_) | Kind::BigInt(_)
    )
}

fn describe_element_diff(
    r#type: &str,
    client_props: &Unknown,
    server_props: &Unknown,
    indent: i32,
) -> String {
    let mut content = String::new();

    // Maps any previously unmatched lower case server prop name to its full prop name
    let mut server_prop_names: HashMap<String, String> = HashMap::new();

    for prop_name in js::keys_in(server_props) {
        if !object::has_own_property.call(server_props, prop_name.as_str()) {
            continue;
        }
        server_prop_names.insert(prop_name.to_lowercase(), prop_name);
    }

    if server_prop_names.len() == 1 && server_prop_names.contains_key("children") {
        content += &describe_expanded_element(r#type, client_props, &indentation(indent));
    } else {
        for prop_name in js::keys_in(client_props) {
            if !object::has_own_property.call(client_props, prop_name.as_str()) {
                continue;
            }
            if prop_name == "children" {
                // Handled below.
                continue;
            }
            let max_length = MAX_ROW_LENGTH
                - f64::from(indent + 1) * 2.0
                - f64::from(string::length(&prop_name))
                - 1.0;
            let server_prop_name = server_prop_names.get(&prop_name.to_lowercase()).cloned();
            if let Some(server_prop_name) = server_prop_name {
                server_prop_names.remove(&prop_name.to_lowercase());
                // There's a diff here.
                let client_value = js::get(client_props, prop_name.as_str());
                let server_value = js::get(server_props, server_prop_name.as_str());
                let client_prop_value = describe_prop_value(client_value, max_length);
                let server_prop_value = describe_prop_value(server_value, max_length);
                if let Some(client_object) = js::classify(client_value).object()
                    && let Some(server_object) = js::classify(server_value).object()
                    && object_name(client_object) == "Object"
                    && object_name(server_object) == "Object"
                    // Only do the diff if the object has a lot of keys or was shortened.
                    && (object::keys(client_object).len() > 2
                        || object::keys(server_object).len() > 2
                        || client_prop_value.contains("...")
                        || server_prop_value.contains("..."))
                {
                    // We're comparing two plain objects. We can diff the nested objects instead.
                    content += &(indentation(indent + 1)
                        + &prop_name
                        + "={{\n"
                        + &describe_properties_diff(client_object, server_object, indent + 2)
                        + &indentation(indent + 1)
                        + "}}\n");
                } else {
                    content += &(added(indent + 1) + &prop_name + "=" + &client_prop_value + "\n");
                    content +=
                        &(removed(indent + 1) + &prop_name + "=" + &server_prop_value + "\n");
                }
            } else {
                // Considered equal.
                content += &(indentation(indent + 1)
                    + &prop_name
                    + "="
                    + &describe_prop_value(js::get(client_props, prop_name.as_str()), max_length)
                    + "\n");
            }
        }

        for prop_name in server_prop_names.values() {
            if prop_name == "children" {
                // Handled below.
                continue;
            }
            let max_length = MAX_ROW_LENGTH
                - f64::from(indent + 1) * 2.0
                - f64::from(string::length(prop_name))
                - 1.0;
            content += &(removed(indent + 1)
                + prop_name
                + "="
                + &describe_prop_value(js::get(server_props, prop_name.as_str()), max_length)
                + "\n");
        }

        if content.is_empty() {
            // No properties
            content = indentation(indent) + "<" + r#type + ">\n";
        } else {
            // Had properties
            content =
                indentation(indent) + "<" + r#type + "\n" + &content + &indentation(indent) + ">\n";
        }
    }

    let server_children = js::get(server_props, "children");
    let client_children = js::get(client_props, "children");
    if is_text_like(server_children) {
        // There's a diff of the children.
        // $FlowFixMe[unsafe-addition]
        let server_text = js::concat_text(server_children).unwrap();
        let mut client_text = String::new();
        if is_text_like(client_children) {
            // $FlowFixMe[unsafe-addition]
            client_text = js::concat_text(client_children).unwrap();
        }
        content += &describe_text_diff(&client_text, ServerProps::Text(&server_text), indent + 1);
    } else if is_text_like(client_children) {
        if server_children.is_none() {
            // This is a new string child.
            // $FlowFixMe[unsafe-addition]
            content += &describe_text_diff(
                &js::concat_text(client_children).unwrap(),
                ServerProps::Unmatched,
                indent + 1,
            );
        } else {
            // The client has children but it's not considered a difference from the server.
            // $FlowFixMe[unsafe-addition]
            content += &describe_text_diff(
                &js::concat_text(client_children).unwrap(),
                ServerProps::Unknown,
                indent + 1,
            );
        }
    }
    content
}

fn describe_sibling_fiber(fiber: &Fiber, indent: i32) -> String {
    let Some(r#type) = describe_fiber_type(fiber) else {
        // Skip this type of fiber. We currently treat this as a fragment
        // so it's just part of the parent's children.
        let mut flat_content = String::new();
        let mut child_fiber = fiber.child.borrow().clone();
        while let Some(child) = child_fiber {
            flat_content += &describe_sibling_fiber(&child, indent);
            child_fiber = child.sibling.borrow().clone();
        }
        return flat_content;
    };
    indentation(indent) + "<" + &r#type + ">" + "\n"
}

fn describe_node(node: &HydrationDiffNode, mut indent: i32) -> String {
    let skip_to_node = find_notable_node(node, indent);
    if !std::ptr::eq(skip_to_node, node)
        && (node.children.len() != 1 || !std::ptr::eq(&node.children[0], skip_to_node))
    {
        return indentation(indent) + "...\n" + &describe_node(skip_to_node, indent + 1);
    }

    // Prefix with any server components for context
    let mut parent_content = String::new();
    if let Some(debug_info) = node.fiber.debug_info.get() {
        let debug_info: &[&Unknown] = unsafe { js::cast(Some(debug_info)) };
        for info in debug_info {
            let server_component_name = js::get(*info, "name");
            if let Kind::String(server_component_name) = js::classify(server_component_name) {
                parent_content +=
                    &(indentation(indent) + "<" + server_component_name + ">" + "\n");
                indent += 1;
            }
        }
    }

    // Self
    let mut self_content = String::new();

    // We use the pending props since we might be generating a diff before the complete phase
    // when something throws.
    let client_props = node.fiber.pending_props.get();

    if node.fiber.tag.get() == HOST_TEXT {
        // Text Node
        self_content =
            describe_text_diff(unsafe { js::cast(client_props) }, node.server_props, indent);
        indent += 1;
    } else if let Some(r#type) = describe_fiber_type(&node.fiber) {
        // Element Node
        match node.server_props {
            ServerProps::Unknown => {
                // Just a reference node for context.
                self_content =
                    describe_collapsed_element(&r#type, unsafe { js::cast(client_props) }, indent);
                indent += 1;
            }
            ServerProps::Unmatched => {
                self_content = describe_expanded_element(
                    &r#type,
                    unsafe { js::cast(client_props) },
                    &added(indent),
                );
                indent += 1;
            }
            ServerProps::Text(_) => {
                if __DEV__ {
                    console::error((
                        "Should not have matched a non HostText fiber to a Text node. This is a bug in React.",
                    ));
                }
            }
            ServerProps::Props(server_props) => {
                self_content = describe_element_diff(
                    &r#type,
                    unsafe { js::cast(client_props) },
                    server_props,
                    indent,
                );
                indent += 1;
            }
        }
    }

    // Compute children
    let mut child_content = String::new();
    let mut child_fiber = node.fiber.child.borrow().clone();
    let mut diff_idx = 0;
    while let Some(current) = child_fiber.clone()
        && diff_idx < node.children.len()
    {
        let child_node = &node.children[diff_idx];
        if Rc::ptr_eq(&child_node.fiber, &current) {
            // This was a match in the diff.
            child_content += &describe_node(child_node, indent);
            diff_idx += 1;
        } else {
            // This is an unrelated previous sibling.
            child_content += &describe_sibling_fiber(&current, indent);
        }
        child_fiber = current.sibling.borrow().clone();
    }

    if child_fiber.is_some() && !node.children.is_empty() {
        // If we had any further siblings after the last mismatch, we can't be sure if it's
        // actually a valid match since it might not have found a match. So we exclude next
        // siblings to avoid confusion.
        child_content += &(indentation(indent) + "..." + "\n");
    }

    // Deleted tail nodes
    let server_tail = &node.server_tail;
    if matches!(node.server_props, ServerProps::Unmatched) {
        indent -= 1;
    }
    for tail_node in server_tail {
        match tail_node {
            ServerTailNode::Text(tail_node) => {
                // Removed text node
                child_content += &(removed(indent)
                    + &describe_text_node(tail_node, MAX_ROW_LENGTH - f64::from(indent) * 2.0)
                    + "\n");
            }
            ServerTailNode::Element(tail_node) => {
                // Removed element
                child_content +=
                    &describe_expanded_element(&tail_node.r#type, tail_node.props, &removed(indent));
            }
        }
    }

    parent_content + &self_content + &child_content
}

pub fn describe_diff(root_node: &HydrationDiffNode) -> String {
    catch_unwind(AssertUnwindSafe(|| "\n\n".to_owned() + &describe_node(root_node, 0)))
        .unwrap_or_default()
}
