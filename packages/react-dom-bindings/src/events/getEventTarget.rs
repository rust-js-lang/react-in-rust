// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use webapi::{Event, EventTarget, node, window};

use crate::html_node_type::TEXT_NODE;

/// Gets the target node from a native browser event by accounting for
/// inconsistencies in browser DOM APIs.
///
/// @param {object} nativeEvent Native browser event.
/// @return {DOMEventTarget} Target node.
pub fn get_event_target(native_event: &Event) -> Option<&'static EventTarget> {
    // Fallback to nativeEvent.srcElement for IE9
    // https://github.com/react/react/issues/12506
    let mut target = native_event
        .target()
        .or_else(|| native_event.src_element())
        .unwrap_or(window);

    // Normalize SVG <use> element events #4963
    if let Some(corresponding_use_element) = js::get(target, "correspondingUseElement")
        .filter(|corresponding_use_element| js::truthy(Some(*corresponding_use_element)))
    {
        target = unsafe { js::cast(Some(corresponding_use_element)) };
    }

    // Safari may fire events on text nodes (Node.TEXT_NODE is 3).
    // @see http://www.quirksmode.org/js/events_properties.html
    if node::unchecked_from(target).node_type() == TEXT_NODE {
        node::unchecked_from(target)
            .parent_node()
            .map(|parent_node| &**parent_node)
    } else {
        Some(target)
    }
}

js::export_default!(get_event_target);
