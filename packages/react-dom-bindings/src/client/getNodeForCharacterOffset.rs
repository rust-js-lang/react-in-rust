// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use webapi::Node;

use crate::html_node_type::TEXT_NODE;

/// Given any node return the first leaf node without children.
///
/// @param {DOMElement|DOMTextNode} node
/// @return {DOMElement|DOMTextNode}
fn get_leaf_node(mut node: Option<&'static Node>) -> Option<&'static Node> {
    while let Some(first_child) = node.and_then(Node::first_child) {
        node = Some(first_child);
    }
    node
}

/// Get the next sibling within a container. This will walk up the
/// DOM if a node's siblings have been exhausted.
///
/// @param {DOMElement|DOMTextNode} node
/// @return {?DOMElement|DOMTextNode}
fn get_sibling_node(mut node: Option<&'static Node>) -> Option<&'static Node> {
    while let Some(current) = node {
        if let Some(next_sibling) = current.next_sibling() {
            return Some(next_sibling);
        }
        node = current.parent_node();
    }
    None
}

/// Which node holds the character at an offset, and where in it.
pub struct NodeAndOffset {
    pub node: &'static Node,
    pub offset: f64,
}

/// Get object describing the nodes which contain characters at offset.
///
/// @param {DOMElement|DOMTextNode} root
/// @param {number} offset
/// @return {?object}
pub fn get_node_for_character_offset(root: &'static Node, offset: f64) -> Option<NodeAndOffset> {
    let mut node = get_leaf_node(Some(root));
    let mut node_start = 0.0;
    let mut node_end;

    while let Some(current) = node {
        if current.node_type() == TEXT_NODE {
            node_end = node_start
                + f64::from(js::string::length(
                    &current.text_content().unwrap_or_default(),
                ));

            if node_start <= offset && node_end >= offset {
                return Some(NodeAndOffset {
                    node: current,
                    offset: offset - node_start,
                });
            }

            node_start = node_end;
        }

        node = get_leaf_node(get_sibling_node(node));
    }
    None
}

js::export_default!(get_node_for_character_offset);
