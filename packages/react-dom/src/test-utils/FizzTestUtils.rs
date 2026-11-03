// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::panic::catch_unwind;

use js::{JsError, JsObject, Unknown, object};
use shared::react_types::{ReactNode, ReactNodeList};
use webapi::{
    Document, Element, ErrorEventInit, HTMLCollection, Node, character_data, document, element,
    html_collection, named_node_map, node_list, window,
};

/// What `insertNodesAndExecuteScripts` moves the nodes of.
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum DocumentOrElement<'a> {
    Document(&'a Document),
    Element(&'a Element),
}

/// What `stripExternalRuntimeInNodes` is given.
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum Nodes<'a> {
    Array(Vec<&'a Element>),
    Collection(&'a HTMLCollection),
}

// `require('react')` where it's called, the React a test loaded last.
mod react {
    use js::Unknown;
    use shared::react_types::ReactNode;

    // GAP rd684: React's `createElement` is Rust's, but a `use` of it is
    // imported where the module loads, never read through `require`.
    #[cfg_attr(rust_js, rust_js::link_name = "react#createElement")]
    #[cfg_attr(rust_js, rust_js::require)]
    pub fn create_element(r#type: &str, props: &Unknown) -> &'static ReactNode {
        unreachable!()
    }
}

pub async fn insert_nodes_and_execute_scripts(
    source: DocumentOrElement<'_>,
    target: &Node,
    #[cfg_attr(rust_js, rust_js::name = "CSPnonce")] csp_nonce: Option<&str>,
) {
    let owner_document = target
        .owner_document()
        .unwrap_or_else(|| document::unchecked_from(target));

    // We need to remove the script content for any scripts that would not run based on CSP
    // We restore the script content after moving the nodes into the target
    let bad_nonce_script_nodes = js::map::new::<Element, Option<String>>(None);
    if let Some(csp_nonce) = csp_nonce.filter(|nonce| !nonce.is_empty()) {
        let scripts = match source {
            DocumentOrElement::Document(source) => source.query_selector_all("script"),
            DocumentOrElement::Element(source) => source.query_selector_all("script"),
        };
        for script in node_list::iter(scripts) {
            if !script.has_attribute("src")
                && script.get_attribute("nonce").as_deref() != Some(csp_nonce)
            {
                bad_nonce_script_nodes.set(script, script.text_content());
                script.set_text_content("");
            }
        }
    }
    let mut last_child = None;
    while let Some(node) = match source {
        DocumentOrElement::Document(source) => source.first_child(),
        DocumentOrElement::Element(source) => source.first_child(),
    } {
        if last_child == Some(node) {
            panic!("Infinite loop.");
        }
        last_child = Some(node);

        if node.node_type() == 1.0 {
            let element = element::unchecked_from(node);
            if let Some(dataset) = js::get(js::unknown(element), "dataset")
                && (js::get(dataset, "rxi").is_some()
                    || js::get(dataset, "rri").is_some()
                    || js::get(dataset, "rci").is_some()
                    || js::get(dataset, "rsi").is_some())
            {
                // Fizz external runtime instructions are expected to be in the body.
                // When we have renderIntoContainer and renderDocument this will be
                // more enforceable. At the moment you can misconfigure your stream and end up
                // with instructions that are deep in the document
                owner_document.body().unwrap().append_child(element);
            } else {
                target.append_child(element);

                if element.node_name() == "SCRIPT" {
                    execute_script(element).await;
                } else {
                    let scripts = element.query_selector_all("script");
                    for script in node_list::iter(scripts) {
                        execute_script(script).await;
                    }
                }
            }
        } else {
            target.append_child(node);
        }
    }

    // restore the textContent now that we have finished attempting to execute scripts
    bad_nonce_script_nodes.for_each(|script_content, script, _| {
        script.set_text_content(script_content.as_deref());
    });
}

async fn execute_script(script: &Element) {
    let owner_document = script.owner_document();
    let Some(parent) = script.parent_node() else {
        panic!(
            "executeScript expects to be called on script nodes that are currently in a document"
        );
    };
    let script_src = script.get_attribute("src");
    if let Some(script_src) = script_src.filter(|src| !src.is_empty()) {
        if document != owner_document {
            panic!(
                "You must set the current document to the global document to use script src in tests"
            );
        }

        if let Err(x) = catch_unwind(|| {
            // $FlowFixMe[unsupported-syntax]
            // GAP rd685: no binding of CommonJS's own `require(id)`; Node's
            // `module.require` is the function Jest gives a module as both.
            node::module.require(&script_src);
        }) {
            let event = window.new_error_event(
                "error",
                Some(ErrorEventInit {
                    error: x
                        .downcast_ref::<&'static JsError>()
                        .map(|error| *error as &dyn js::IntoUnknown),
                    ..Default::default()
                }),
            );
            window.dispatch_event(event);
        }
    } else {
        let new_script = owner_document.create_element_named("script", None);
        new_script.set_text_content(script.text_content().as_deref());
        // make sure to add nonce back to script if it exists
        for attribute in named_node_map::iter(script.attributes()) {
            new_script.set_attribute(&attribute.name(), attribute.value());
        }

        parent.insert_before(new_script, Some(script));
        parent.remove_child(script);
    }
}

pub fn merge_options(
    options: Option<&Unknown>,
    default_options: Option<&Unknown>,
) -> &'static JsObject {
    object::spread((default_options, options))
}

pub fn strip_external_runtime_in_nodes<'a>(
    nodes: Nodes<'a>,
    external_runtime_src: Option<&str>,
) -> Vec<&'a Element> {
    let nodes: Vec<&Element> = match nodes {
        Nodes::Array(nodes) => nodes,
        Nodes::Collection(nodes) => html_collection::iter(nodes).collect(),
    };
    let Some(external_runtime_src) = external_runtime_src else {
        return nodes;
    };
    nodes
        .into_iter()
        .filter(|n| {
            (n.tag_name() != "SCRIPT" && n.tag_name() != "script")
                || n.get_attribute("src").as_deref() != Some(external_runtime_src)
        })
        .collect()
}

pub fn get_visible_children(element: &Element) -> Option<&'static ReactNodeList> {
    let mut children: Vec<&'static ReactNode> = Vec::new();
    let mut node = element.first_child();
    while let Some(current) = node {
        if current.node_type() == 1.0 {
            let current = element::unchecked_from(current);
            if ((current.tag_name() != "SCRIPT" && current.tag_name() != "script")
                || current.has_attribute("data-meaningful"))
                && current.tag_name() != "TEMPLATE"
                && current.tag_name() != "template"
                && !current.has_attribute("hidden")
                && !current.has_attribute("aria-hidden")
                // Ignore the render blocking expect
                && (current.get_attribute("rel").as_deref() != Some("expect")
                    || current.get_attribute("blocking").as_deref() != Some("render"))
            {
                let props = object::spread(None);
                let attributes = current.attributes();
                for attribute in named_node_map::iter(attributes) {
                    if attribute.name() == "id" && attribute.value().contains(':') {
                        // We assume this is a React added ID that's a non-visual implementation detail.
                        continue;
                    }
                    js::set(props, &attribute.name(), attribute.value());
                }
                let nested_children = get_visible_children(current);
                if let Some(nested_children) = nested_children {
                    js::set(props, "children", nested_children);
                }
                children.push(react::create_element(
                    &current.tag_name().to_lowercase(),
                    js::unknown(props),
                ));
            }
        } else if current.node_type() == 3.0 {
            children.push(js::unknown_of(
                character_data::unchecked_from(current).data(),
            ));
        }
        node = current.next_sibling();
    }
    match children.len() {
        0 => None,
        1 => Some(children[0]),
        _ => Some(js::unknown_of(children)),
    }
}
