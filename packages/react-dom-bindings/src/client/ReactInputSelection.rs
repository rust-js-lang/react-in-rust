// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::panic::{AssertUnwindSafe, catch_unwind};

use js::Kind;
use webapi::{
    Document, Element, HTMLIFrameElement, Node, element, html_element, html_i_frame_element,
    html_input_element, window,
};

use crate::get_active_element::get_active_element;
use crate::html_node_type::{ELEMENT_NODE, TEXT_NODE};
use crate::react_dom_selection::{Offsets, get_offsets, set_offsets};

fn is_text_node(node: Option<&Node>) -> bool {
    node.is_some_and(|node| node.node_type() == TEXT_NODE)
}

fn contains_node(outer_node: Option<&Node>, inner_node: Option<&Node>) -> bool {
    let (Some(outer_node), Some(inner_node)) = (outer_node, inner_node) else {
        return false;
    };
    if std::ptr::eq(outer_node, inner_node) {
        true
    } else if is_text_node(Some(outer_node)) {
        false
    } else if is_text_node(Some(inner_node)) {
        contains_node(Some(outer_node), inner_node.parent_node())
    } else if js::has(outer_node, "contains") {
        outer_node.contains(inner_node)
    } else if js::truthy(js::get(outer_node, "compareDocumentPosition")) {
        outer_node.compare_document_position(inner_node) & 16 != 0
    } else {
        false
    }
}

fn is_in_document(node: &Node) -> bool {
    match node.owner_document() {
        Some(owner_document) => contains_node(
            owner_document.document_element().map(|element| &**element),
            Some(node),
        ),
        None => false,
    }
}

fn is_same_origin_frame(iframe: &HTMLIFrameElement) -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        // Accessing the contentDocument of a HTMLIframeElement can cause the browser
        // to throw, e.g. if it has a cross-origin src attribute.
        // Safari will show an error in the console when the access results in "Blocked a frame with origin". e.g:
        // iframe.contentDocument.defaultView;
        // A safety way is to access one of the cross origin properties: Window or Location
        // Which might result in "SecurityError" DOM Exception and it is compatible to Safari.
        // https://html.spec.whatwg.org/multipage/browsers.html#integration-with-idl

        matches!(
            js::classify(js::get(
                js::get(iframe.content_window(), "location"),
                "href"
            )),
            Kind::String(_)
        )
    }))
    .unwrap_or(false)
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn get_active_element_deep(container_info: Option<&Node>) -> Option<&'static Element> {
    let mut win = container_info
        .and_then(Node::owner_document)
        .and_then(Document::default_view)
        .unwrap_or(window);
    let mut element = get_active_element(Some(win.document()));
    while let Some(iframe) = element
        && js::instance_of(iframe, js::get(win, "HTMLIFrameElement"))
    {
        let iframe = html_i_frame_element::unchecked_from(iframe);
        if is_same_origin_frame(iframe) {
            win = iframe.content_window().unwrap();
        } else {
            return element;
        }
        element = get_active_element(Some(win.document()));
    }
    element
}

// @ReactInputSelection: React input selection module. Based on Selection.js,
// but modified to be suitable for react and has a couple of bug fixes (doesn't
// assume buttons have range selections allowed).
// Input selection module for React.

/// @hasSelectionCapabilities: we get the element types that support selection
/// from https://html.spec.whatwg.org/#do-not-apply, looking at `selectionStart`
/// and `selectionEnd` rows.
pub fn has_selection_capabilities(elem: Option<&Element>) -> bool {
    let Some(elem) = elem else {
        return false;
    };
    let node_name = elem.node_name().to_lowercase();
    !node_name.is_empty()
        && ((node_name == "input"
            && matches!(
                html_input_element::unchecked_from(elem).r#type().as_str(),
                "text" | "search" | "tel" | "url" | "password"
            ))
            || node_name == "textarea"
            || html_element::unchecked_from(elem).content_editable() == "true")
}

/// What was focused, and its selection, before a commit.
pub struct SelectionInformation {
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub focused_elem: Option<&'static Element>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub selection_range: Option<Offsets>,
}

pub fn get_selection_information(container_info: Option<&Node>) -> SelectionInformation {
    let focused_elem = get_active_element_deep(container_info);
    SelectionInformation {
        focused_elem,
        selection_range: match focused_elem {
            Some(focused_elem) if has_selection_capabilities(Some(focused_elem)) => {
                Some(get_selection(focused_elem))
            }
            _ => None,
        },
    }
}

/// An element above the focused one, and where it was scrolled to.
struct ScrollPosition {
    element: &'static Element,
    left: f64,
    top: f64,
}

/// @restoreSelection: If any selection information was potentially lost,
/// restore it. This is useful when performing operations that could remove dom
/// nodes and place them back in, resulting in focus being lost.
pub fn restore_selection(
    prior_selection_information: &SelectionInformation,
    container_info: Option<&Node>,
) {
    let cur_focused_elem = get_active_element_deep(container_info);
    let prior_focused_elem = prior_selection_information.focused_elem;
    let prior_selection_range = &prior_selection_information.selection_range;
    if !js::strict_equals(cur_focused_elem, prior_focused_elem)
        && let Some(prior_focused_elem) = prior_focused_elem
        && is_in_document(prior_focused_elem)
    {
        if let Some(prior_selection_range) = prior_selection_range
            && has_selection_capabilities(Some(prior_focused_elem))
        {
            set_selection(prior_focused_elem, prior_selection_range);
        }

        // Focusing a node can change the scroll position, which is undesirable
        let mut ancestors = Vec::new();
        let mut ancestor: &Node = prior_focused_elem;
        while let Some(parent_node) = ancestor.parent_node() {
            ancestor = parent_node;
            if ancestor.node_type() == ELEMENT_NODE {
                let ancestor = element::unchecked_from(ancestor);
                ancestors.push(ScrollPosition {
                    element: ancestor,
                    left: ancestor.scroll_left(),
                    top: ancestor.scroll_top(),
                });
            }
        }

        if matches!(
            js::classify(js::get(prior_focused_elem, "focus")),
            Kind::Function(_)
        ) {
            html_element::unchecked_from(prior_focused_elem).focus(None);
        }

        for info in &ancestors {
            info.element.set_scroll_left(info.left);
            info.element.set_scroll_top(info.top);
        }
    }
}

/// @getSelection: Gets the selection bounds of a focused textarea, input or
/// contentEditable node.
/// -@input: Look up selection bounds of this input
/// -@return {start: selectionStart, end: selectionEnd}
pub fn get_selection(input: &'static Element) -> Offsets {
    let selection = if js::has(input, "selectionStart") {
        // Modern browser with input or textarea.
        let input = html_input_element::unchecked_from(input);
        Some(Offsets {
            start: input.selection_start().map_or(0.0, f64::from),
            end: Some(input.selection_end().map_or(0.0, f64::from)),
        })
    } else {
        // Content editable or old IE textarea.
        get_offsets(input)
    };

    selection.unwrap_or(Offsets {
        start: 0.0,
        end: Some(0.0),
    })
}

/// @setSelection: Sets the selection bounds of a textarea or input and focuses
/// the input.
/// -@input     Set selection bounds of this input or textarea
/// -@offsets   Object of same form that is returned from get*
pub fn set_selection(input: &'static Element, offsets: &Offsets) {
    let start = offsets.start;
    let end = offsets.end.unwrap_or(start);

    if js::has(input, "selectionStart") {
        let input = html_input_element::unchecked_from(input);
        input.set_selection_start(start as u32);
        input.set_selection_end(
            js::math::min((end, f64::from(js::string::length(&input.value())))) as u32,
        );
    } else {
        set_offsets(input, offsets);
    }
}
