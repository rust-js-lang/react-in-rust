// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::Unknown;
use shared::react_feature_flags::DISABLE_COMMENTS_AS_DOM_CONTAINERS;

use crate::html_node_type::{COMMENT_NODE, DOCUMENT_FRAGMENT_NODE, DOCUMENT_NODE, ELEMENT_NODE};

pub fn is_valid_container(node: Option<&Unknown>) -> bool {
    js::truthy(node)
        && (js::strict_equals(js::get(node, "nodeType"), ELEMENT_NODE)
            || js::strict_equals(js::get(node, "nodeType"), DOCUMENT_NODE)
            || js::strict_equals(js::get(node, "nodeType"), DOCUMENT_FRAGMENT_NODE)
            || (!DISABLE_COMMENTS_AS_DOM_CONTAINERS
                && js::strict_equals(js::get(node, "nodeType"), COMMENT_NODE)
                && js::strict_equals(js::get(node, "nodeValue"), " react-mount-point-unstable ")))
}
