// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::panic::{AssertUnwindSafe, catch_unwind};

use webapi::{Document, Node, document, window};

use crate::get_node_for_character_offset::get_node_for_character_offset;
use crate::html_node_type::TEXT_NODE;

/// Where a selection starts and ends, as character offsets.
pub struct Offsets {
    pub start: f64,
    pub end: Option<f64>,
}

/// @param {DOMElement} outerNode
/// @return {?object}
#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_offsets(outer_node: &'static Node) -> Option<Offsets> {
    let owner_document = outer_node.owner_document();
    let win = owner_document
        .and_then(Document::default_view)
        .unwrap_or(window);
    let selection = if js::truthy(js::get(win, "getSelection")) {
        win.get_selection()
    } else {
        None
    };

    let selection = match selection {
        Some(selection) if selection.range_count() != 0 => selection,
        _ => return None,
    };

    let anchor_node = selection.anchor_node();
    let anchor_offset = selection.anchor_offset();
    let focus_node = selection.focus_node();
    let focus_offset = selection.focus_offset();

    // In Firefox, anchorNode and focusNode can be "anonymous divs", e.g. the
    // up/down buttons on an <input type="number">. Anonymous divs do not seem to
    // expose properties, triggering a "Permission denied error" if any of its
    // properties are accessed. The only seemingly possible way to avoid erroring
    // is to access a property that typically works for non-anonymous divs and
    // catch any error that may otherwise arise. See
    // https://bugzilla.mozilla.org/show_bug.cgi?id=208427
    if catch_unwind(AssertUnwindSafe(|| {
        /* eslint-disable ft-flow/no-unused-expressions */
        js::get(anchor_node, "nodeType");
        js::get(focus_node, "nodeType");
        /* eslint-enable ft-flow/no-unused-expressions */
    }))
    .is_err()
    {
        return None;
    }

    get_modern_offsets_from_points(
        outer_node,
        anchor_node,
        f64::from(anchor_offset),
        focus_node,
        f64::from(focus_offset),
    )
}

/// Returns {start, end} where `start` is the character/codepoint index of
/// (anchorNode, anchorOffset) within the textContent of `outerNode`, and
/// `end` is the index of (focusNode, focusOffset).
///
/// Returns null if you pass in garbage input but we should probably just crash.
///
/// Exported only for testing.
#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_modern_offsets_from_points(
    outer_node: &'static Node,
    anchor_node: Option<&'static Node>,
    anchor_offset: f64,
    focus_node: Option<&'static Node>,
    focus_offset: f64,
) -> Option<Offsets> {
    let mut length = 0.0;
    let mut start = -1.0;
    let mut end = -1.0;
    let mut index_within_anchor = 0.0;
    let mut index_within_focus = 0.0;
    let mut node = outer_node;
    let mut parent_node: Option<&'static Node> = js::null();

    'outer: loop {
        loop {
            if js::strict_equals(node, anchor_node)
                && (anchor_offset == 0.0 || node.node_type() == TEXT_NODE)
            {
                start = length + anchor_offset;
            }
            if js::strict_equals(node, focus_node)
                && (focus_offset == 0.0 || node.node_type() == TEXT_NODE)
            {
                end = length + focus_offset;
            }

            if node.node_type() == TEXT_NODE {
                length += f64::from(js::string::length(&node.node_value().unwrap_or_default()));
            }

            let Some(next) = node.first_child() else {
                break;
            };
            // Moving from `node` to its first child `next`.
            parent_node = Some(node);
            node = next;
        }

        let next;
        loop {
            if js::strict_equals(node, outer_node) {
                // If `outerNode` has children, this is always the second time visiting
                // it. If it has no children, this is still the first loop, and the only
                // valid selection is anchorNode and focusNode both equal to this node
                // and both offsets 0, in which case we will have handled above.
                break 'outer;
            }
            if js::strict_equals(parent_node, anchor_node) {
                index_within_anchor += 1.0;
                if index_within_anchor == anchor_offset {
                    start = length;
                }
            }
            if js::strict_equals(parent_node, focus_node) {
                index_within_focus += 1.0;
                if index_within_focus == focus_offset {
                    end = length;
                }
            }
            if let Some(next_sibling) = node.next_sibling() {
                next = next_sibling;
                break;
            }
            node = parent_node.unwrap();
            parent_node = node.parent_node();
        }

        // Moving from `node` to its next sibling `next`.
        node = next;
    }

    if start == -1.0 || end == -1.0 {
        // This should never happen. (Would happen if the anchor/focus nodes aren't
        // actually inside the passed-in node.)
        return None;
    }

    Some(Offsets {
        start,
        end: Some(end),
    })
}

/// In modern non-IE browsers, we can support both forward and backward
/// selections.
///
/// Note: IE10+ supports the Selection object, but it does not support
/// the `extend` method, which means that even in modern IE, it's not possible
/// to programmatically create a backward selection. Thus, for all IE
/// versions, we use the old IE API to create our selections.
///
/// @param {DOMElement|DOMTextNode} node
/// @param {object} offsets
pub fn set_offsets(node: &'static Node, offsets: &Offsets) {
    let doc = node.owner_document().unwrap_or(document);
    let win = doc.default_view().unwrap_or(window);

    // Edge fails with "Object expected" in some scenarios.
    // (For instance: TinyMCE editor used in a list component that supports pasting to add more,
    // fails when pasting 100+ items)
    if !js::truthy(js::get(win, "getSelection")) {
        return;
    }

    let selection = win.get_selection().unwrap();
    let length = f64::from(js::string::length(&node.text_content().unwrap_or_default()));
    let mut start = js::math::min((offsets.start, length));
    let mut end = match offsets.end {
        None => start,
        Some(end) => js::math::min((end, length)),
    };

    // IE 11 uses modern selection, but doesn't support the extend method.
    // Flip backward selections, so we can set with a single range.
    if !js::truthy(js::get(selection, "extend")) && start > end {
        std::mem::swap(&mut start, &mut end);
    }

    let start_marker = get_node_for_character_offset(node, start);
    let end_marker = get_node_for_character_offset(node, end);

    if let (Some(start_marker), Some(end_marker)) = (start_marker, end_marker) {
        if selection.range_count() == 1
            && js::strict_equals(selection.anchor_node(), start_marker.node)
            && f64::from(selection.anchor_offset()) == start_marker.offset
            && js::strict_equals(selection.focus_node(), end_marker.node)
            && f64::from(selection.focus_offset()) == end_marker.offset
        {
            return;
        }
        let range = doc.create_range();
        range.set_start(start_marker.node, start_marker.offset as u32);
        selection.remove_all_ranges();

        if start > end {
            selection.add_range(range);
            selection.extend(end_marker.node, Some(end_marker.offset as u32));
        } else {
            range.set_end(end_marker.node, end_marker.offset as u32);
            selection.add_range(range);
        }
    }
}
