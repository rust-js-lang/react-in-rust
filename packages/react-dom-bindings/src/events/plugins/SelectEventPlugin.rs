// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js::Unknown;
use react_reconciler::react_internal_types::Fiber;
use shared::execution_environment::CAN_USE_DOM;
use shared::shallow_equal::shallow_equal;
use webapi::{Document, Element, EventTarget, Node, document, window};

use crate::dom_plugin_event_system::{
    DispatchEntry, DispatchQueue, accumulate_two_phase_listeners,
};
use crate::event_registry::register_two_phase_event;
use crate::event_system_flags::EventSystemFlags;
use crate::get_active_element::get_active_element;
use crate::html_node_type::DOCUMENT_NODE;
use crate::is_text_input_element::is_text_input_element;
use crate::plugin_module_type::AnyNativeEvent;
use crate::react_dom_component_tree::get_node_from_instance;
use crate::react_synthetic_event_type::ReactSyntheticEvent;
use crate::synthetic_event::SYNTHETIC_EVENT;

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/client/ReactInputSelection.js#hasSelectionCapabilities"
)]
#[cfg_attr(rust_js, rust_js::nullable(elem))]
fn has_selection_capabilities(elem: Option<&Element>) -> bool {
    let _ = elem;
    unreachable!()
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "skipSelectionChangeEvent")]
    static SKIP_SELECTION_CHANGE_EVENT: bool = CAN_USE_DOM.with(|can_use_dom| *can_use_dom)
        && js::has(document, "documentMode")
        && unsafe { js::cast::<f64>(js::get(document, "documentMode")) } <= 11.0;
}

pub fn register_events() {
    register_two_phase_event(
        "onSelect",
        Rc::new(vec![
            "focusout".to_owned(),
            "contextmenu".to_owned(),
            "dragend".to_owned(),
            "focusin".to_owned(),
            "keydown".to_owned(),
            "keyup".to_owned(),
            "mousedown".to_owned(),
            "mouseup".to_owned(),
            "selectionchange".to_owned(),
        ]),
    );
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::nullable)]
    static ACTIVE_ELEMENT: Cell<Option<&'static Element>> = const { Cell::new(None) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static ACTIVE_ELEMENT_INST: RefCell<Option<Rc<Fiber>>> = const { RefCell::new(None) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static LAST_SELECTION: Cell<Option<&'static Unknown>> = const { Cell::new(None) };
    static MOUSE_DOWN: Cell<bool> = const { Cell::new(false) };
}

/// An input's or a textarea's selection.
struct InputSelection {
    start: Option<&'static Unknown>,
    end: Option<&'static Unknown>,
}

unsafe impl js::Defined for InputSelection {}

/// The document's selection, of any other element.
struct DocumentSelection {
    anchor_node: Option<&'static Node>,
    anchor_offset: f64,
    focus_node: Option<&'static Node>,
    focus_offset: f64,
}

unsafe impl js::Defined for DocumentSelection {}

/// Get an object which is a unique representation of the current selection.
///
/// The return value will not be consistent across nodes or browsers, but
/// two identical selections on the same node will return identical objects.
fn get_selection(node: &'static Element) -> &'static Unknown {
    if js::has(node, "selectionStart") && has_selection_capabilities(Some(node)) {
        js::unknown_of(InputSelection {
            start: js::get(node, "selectionStart"),
            end: js::get(node, "selectionEnd"),
        })
    } else {
        let win = node
            .owner_document()
            .and_then(Document::default_view)
            .unwrap_or(window);
        let selection = win.get_selection().unwrap();
        js::unknown_of(DocumentSelection {
            anchor_node: selection.anchor_node(),
            anchor_offset: selection.anchor_offset(),
            focus_node: selection.focus_node(),
            focus_offset: selection.focus_offset(),
        })
    }
}

/// Get document associated with the event target.
fn get_event_target_document(
    event_target: Option<&'static EventTarget>,
) -> Option<&'static Unknown> {
    if js::strict_equals(js::get(event_target, "window"), event_target) {
        js::get(event_target, "document")
    } else if js::strict_equals(js::get(event_target, "nodeType"), DOCUMENT_NODE) {
        event_target.map(js::unknown)
    } else {
        js::get(event_target, "ownerDocument")
    }
}

/// Poll selection to see whether it's changed.
///
/// @param {object} nativeEvent
/// @param {object} nativeEventTarget
/// @return {?SyntheticEvent}
fn construct_select_event(
    dispatch_queue: &mut DispatchQueue,
    native_event: &'static AnyNativeEvent,
    native_event_target: Option<&'static EventTarget>,
) {
    // Ensure we have the right element, and that the user is not dragging a
    // selection (this matches native `select` event behavior). In HTML5, select
    // fires only on input and textarea thus if there's no focused element we
    // won't dispatch.
    let doc = get_event_target_document(native_event_target);

    if MOUSE_DOWN.get()
        || ACTIVE_ELEMENT.get().is_none()
        || ACTIVE_ELEMENT.get() != get_active_element(unsafe { js::cast(doc) })
    {
        return;
    }

    // Only fire when selection has actually changed.
    let current_selection = get_selection(ACTIVE_ELEMENT.get().unwrap());
    if LAST_SELECTION.get().is_none()
        || !shallow_equal(LAST_SELECTION.get(), Some(current_selection))
    {
        LAST_SELECTION.set(Some(current_selection));

        let listeners = accumulate_two_phase_listeners(
            ACTIVE_ELEMENT_INST.with_borrow(Clone::clone),
            "onSelect",
        );
        if !listeners.is_empty() {
            let event: &'static ReactSyntheticEvent = js::new(
                SYNTHETIC_EVENT.with(|ctor| *ctor),
                (
                    Some("onSelect".to_owned()),
                    "select".to_owned(),
                    js::null(),
                    native_event,
                    native_event_target,
                ),
            );
            dispatch_queue.push(DispatchEntry { event, listeners });
            event.target.set(ACTIVE_ELEMENT.get().map(js::unknown));
        }
    }
}

/// This plugin creates an `onSelect` event that normalizes select events
/// across form elements.
///
/// Supported elements are:
/// - input (see `isTextInputElement`)
/// - textarea
/// - contentEditable
///
/// This differs from native browser implementations in the following ways:
/// - Fires on contentEditable fields as well as inputs.
/// - Fires for collapsed selection.
/// - Fires after user input.
#[cfg_attr(rust_js, rust_js::nullable(target_inst, native_event_target))]
pub fn extract_events(
    dispatch_queue: &mut DispatchQueue,
    dom_event_name: &str,
    target_inst: Option<Rc<Fiber>>,
    native_event: &'static AnyNativeEvent,
    native_event_target: Option<&'static EventTarget>,
    _event_system_flags: EventSystemFlags,
    _target_container: &'static EventTarget,
) {
    let target_node = match &target_inst {
        Some(target_inst) => get_node_from_instance(target_inst),
        None => Some(js::unknown(window)),
    };

    match dom_event_name {
        // Track the input node that has focus.
        "focusin" => {
            if is_text_input_element(unsafe { js::cast(target_node) })
                || js::strict_equals(js::get(target_node, "contentEditable"), "true")
            {
                ACTIVE_ELEMENT.set(Some(unsafe { js::cast(target_node) }));
                ACTIVE_ELEMENT_INST.set(target_inst);
                LAST_SELECTION.set(js::null());
            }
        }
        "focusout" => {
            ACTIVE_ELEMENT.set(js::null());
            ACTIVE_ELEMENT_INST.set(js::null());
            LAST_SELECTION.set(js::null());
        }
        // Don't fire the event while the user is dragging. This matches the
        // semantics of the native select event.
        "mousedown" => {
            MOUSE_DOWN.set(true);
        }
        "contextmenu" | "mouseup" | "dragend" => {
            MOUSE_DOWN.set(false);
            construct_select_event(dispatch_queue, native_event, native_event_target);
        }
        // Chrome and IE fire non-standard event when selection is changed (and
        // sometimes when it hasn't). IE's event fires out of order with respect
        // to key and input events on deletion, so we discard it.
        //
        // Firefox doesn't support selectionchange, so check selection status
        // after each key entry. The selection changes after keydown and before
        // keyup, but we check on keydown as well in the case of holding down a
        // key, when multiple keydown events are fired but only one keyup is.
        // This is also our approach for IE handling, for the reason above.
        "selectionchange"
            if SKIP_SELECTION_CHANGE_EVENT
                .with(|skip_selection_change_event| *skip_selection_change_event) => {}
        "selectionchange" | "keydown" | "keyup" => {
            construct_select_event(dispatch_queue, native_event, native_event_target);
        }
        _ => {}
    }
}
