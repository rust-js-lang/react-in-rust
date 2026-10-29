// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use webapi::Element;

use crate::html_node_type::TEXT_NODE;

/// Set the textContent property of a node. For text updates, it's faster
/// to set the `nodeValue` of the Text node directly instead of using
/// `.textContent` which will remove the existing node and create a new one.
///
/// @param {DOMElement} node
/// @param {string} text
/// @internal
fn set_text_content(node: &Element, text: &str) {
    if !text.is_empty() {
        let first_child = node.first_child();

        if let Some(first_child) = first_child
            && Some(first_child) == node.last_child()
            && first_child.node_type() == TEXT_NODE
        {
            first_child.set_node_value(text);
            return;
        }
    }
    node.set_text_content(text);
}

js::export_default!(set_text_content);
