// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js::Unknown;
use react_reconciler::react_internal_types::Fiber;
use shared::execution_environment::CAN_USE_DOM;
use webapi::{Event, EventTarget, Node, document, window};

use crate::dom_plugin_event_system::{
    DispatchEntry, DispatchQueue, accumulate_two_phase_listeners, process_dispatch_queue,
};
use crate::event_registry::register_two_phase_event;
use crate::event_system_flags::EventSystemFlags;
use crate::get_event_target::get_event_target;
use crate::input_value_tracking::update_value_if_changed;
use crate::is_custom_element::is_custom_element;
use crate::is_event_supported::is_event_supported;
use crate::is_text_input_element::is_text_input_element;
use crate::plugin_module_type::AnyNativeEvent;
use crate::react_dom_component_tree::get_node_from_instance;
use crate::react_dom_controlled_component::enqueue_state_restore;
use crate::react_dom_update_batching::batched_updates;
use crate::react_synthetic_event_type::ReactSyntheticEvent;
use crate::synthetic_event::SYNTHETIC_EVENT;

/// IE's `element.attachEvent(type, listener)`.
#[cfg_attr(rust_js, rust_js::link_name = "attachEvent")]
fn attach_event(this: &Unknown, r#type: &str, listener: fn(&'static Event)) {
    unreachable!()
}

/// IE's `element.detachEvent(type, listener)`.
#[cfg_attr(rust_js, rust_js::link_name = "detachEvent")]
fn detach_event(this: &Unknown, r#type: &str, listener: fn(&'static Event)) {
    unreachable!()
}

pub fn register_events() {
    register_two_phase_event(
        "onChange",
        Rc::new(vec![
            "change".to_owned(),
            "click".to_owned(),
            "focusin".to_owned(),
            "focusout".to_owned(),
            "input".to_owned(),
            "keydown".to_owned(),
            "keyup".to_owned(),
            "selectionchange".to_owned(),
        ]),
    );
}

fn create_and_accumulate_change_event(
    dispatch_queue: &mut DispatchQueue,
    inst: Option<Rc<Fiber>>,
    native_event: &'static AnyNativeEvent,
    target: Option<&'static EventTarget>,
) {
    // Flag this event loop as needing state restore.
    enqueue_state_restore(unsafe { js::cast(target.map(js::unknown)) });
    let listeners = accumulate_two_phase_listeners(inst, "onChange");
    if !listeners.is_empty() {
        let event: &'static ReactSyntheticEvent = js::new(
            SYNTHETIC_EVENT.with(|ctor| *ctor),
            (
                Some("onChange".to_owned()),
                "change".to_owned(),
                js::null(),
                native_event,
                target,
            ),
        );
        dispatch_queue.push(DispatchEntry { event, listeners });
    }
}

thread_local! {
    /// For IE shims
    #[cfg_attr(rust_js, rust_js::nullable)]
    static ACTIVE_ELEMENT: Cell<Option<&'static Unknown>> = const { Cell::new(None) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static ACTIVE_ELEMENT_INST: RefCell<Option<Rc<Fiber>>> = const { RefCell::new(None) };
}

/// SECTION: handle `change` event
fn should_use_change_event(elem: Option<&Node>) -> bool {
    let node_name = elem
        .map(|elem| elem.node_name())
        .filter(|node_name| !node_name.is_empty())
        .map(|node_name| node_name.to_lowercase());
    node_name.as_deref() == Some("select")
        || (node_name.as_deref() == Some("input")
            && js::strict_equals(js::get(elem, "type"), "file"))
}

fn manual_dispatch_change_event(native_event: &'static AnyNativeEvent) {
    let mut dispatch_queue: DispatchQueue = vec![];
    create_and_accumulate_change_event(
        &mut dispatch_queue,
        ACTIVE_ELEMENT_INST.with_borrow(Clone::clone),
        native_event,
        get_event_target(native_event),
    );

    // If change and propertychange bubbled, we'd just bind to it like all the
    // other events and have it go through ReactBrowserEventEmitter. Since it
    // doesn't, we manually listen for the events and so we have to enqueue and
    // process the abstract event manually.
    //
    // Batching is necessary here in order to ensure that all event handlers run
    // before the next rerender (including event handlers attached to ancestor
    // elements instead of directly on the input). Without this, controlled
    // components don't work properly in conjunction with event bubbling because
    // the component is rerendered and the value reverted before all the event
    // handlers can run. See https://github.com/react/react/issues/708.
    batched_updates(run_event_in_batch, dispatch_queue, None);
}

fn run_event_in_batch(dispatch_queue: DispatchQueue, _: Option<()>) {
    process_dispatch_queue(&dispatch_queue, 0);
}

fn get_inst_if_value_changed(target_inst: Option<&Rc<Fiber>>) -> Option<Rc<Fiber>> {
    let target_node = get_node_from_instance(target_inst.unwrap());
    if update_value_if_changed(unsafe { js::cast(target_node) }) {
        return target_inst.cloned();
    }
    None
}

fn get_target_inst_for_change_event(
    dom_event_name: &str,
    target_inst: Option<&Rc<Fiber>>,
) -> Option<Rc<Fiber>> {
    if dom_event_name == "change" {
        return target_inst.cloned();
    }
    None
}

thread_local! {
    /// SECTION: handle `input` event
    // IE9 claims to support the input event but fails to trigger it when
    // deleting text, so we ignore its input events.
    #[cfg_attr(rust_js, rust_js::name = "isInputEventSupported")]
    static IS_INPUT_EVENT_SUPPORTED: bool = CAN_USE_DOM.with(|can_use_dom| *can_use_dom)
        && is_event_supported("input")
        && (!js::truthy(js::get(document, "documentMode"))
            || unsafe { js::cast::<f64>(js::get(document, "documentMode")) } > 9.0);
}

/// (For IE <=9) Starts tracking propertychange events on the passed-in element
/// and override the value property so that we can distinguish user events from
/// value changes in JS.
#[cfg_attr(rust_js, rust_js::nullable(target_inst))]
fn start_watching_for_value_change(target: &'static Unknown, target_inst: Option<Rc<Fiber>>) {
    ACTIVE_ELEMENT.set(Some(target));
    ACTIVE_ELEMENT_INST.set(target_inst);
    attach_event(target, "onpropertychange", handle_property_change);
}

/// (For IE <=9) Removes the event listeners from the currently-tracked element,
/// if any exists.
fn stop_watching_for_value_change() {
    let Some(active_element) = ACTIVE_ELEMENT.get() else {
        return;
    };
    detach_event(active_element, "onpropertychange", handle_property_change);
    ACTIVE_ELEMENT.set(js::null());
    ACTIVE_ELEMENT_INST.set(js::null());
}

/// (For IE <=9) Handles a propertychange event, sending a `change` event if
/// the value of the active element has changed.
fn handle_property_change(native_event: &'static Event) {
    if !js::strict_equals(js::get(native_event, "propertyName"), "value") {
        return;
    }
    if get_inst_if_value_changed(ACTIVE_ELEMENT_INST.with_borrow(Clone::clone).as_ref()).is_some() {
        manual_dispatch_change_event(native_event);
    }
}

#[cfg_attr(rust_js, rust_js::nullable(target_inst))]
fn handle_events_for_input_event_polyfill(
    dom_event_name: &str,
    target: Option<&'static Unknown>,
    target_inst: Option<Rc<Fiber>>,
) {
    if dom_event_name == "focusin" {
        // In IE9, propertychange fires for most input events but is buggy and
        // doesn't fire when text is deleted, but conveniently, selectionchange
        // appears to fire in all of the remaining cases so we catch those and
        // forward the event if the value has changed
        // In either case, we don't want to call the event handler if the value
        // is changed from JS so we redefine a setter for `.value` that updates
        // our activeElementValue variable, allowing us to ignore those changes
        //
        // stopWatching() should be a noop here but we call it just in case we
        // missed a blur event somehow.
        stop_watching_for_value_change();
        start_watching_for_value_change(unsafe { js::cast(target) }, target_inst);
    } else if dom_event_name == "focusout" {
        stop_watching_for_value_change();
    }
}

// For IE8 and IE9.
fn get_target_inst_for_input_event_polyfill(
    dom_event_name: &str,
    _target_inst: Option<&Rc<Fiber>>,
) -> Option<Rc<Fiber>> {
    if dom_event_name == "selectionchange"
        || dom_event_name == "keyup"
        || dom_event_name == "keydown"
    {
        // On the selectionchange event, the target is just document which isn't
        // helpful for us so just check activeElement instead.
        //
        // 99% of the time, keydown and keyup aren't necessary. IE8 fails to fire
        // propertychange on the first input event after setting `value` from a
        // script and fires only keydown, keypress, keyup. Catching keyup usually
        // gets it and catching keydown lets us fire an event for the first
        // keystroke if user does a key repeat (it'll be a little delayed: right
        // before the second keystroke). Other input methods (e.g., paste) seem to
        // fire selectionchange normally.
        return get_inst_if_value_changed(ACTIVE_ELEMENT_INST.with_borrow(Clone::clone).as_ref());
    }
    None
}

/// SECTION: handle `click` event
fn should_use_click_event(elem: Option<&Node>) -> bool {
    // Use the `click` event to detect changes to checkbox and radio inputs.
    // This approach works across all browsers, whereas `change` does not fire
    // until `blur` in IE8.
    let node_name = elem
        .map(|elem| elem.node_name())
        .filter(|node_name| !node_name.is_empty());
    node_name.is_some_and(|node_name| node_name.to_lowercase() == "input")
        && (js::strict_equals(js::get(elem, "type"), "checkbox")
            || js::strict_equals(js::get(elem, "type"), "radio"))
}

fn get_target_inst_for_click_event(
    dom_event_name: &str,
    target_inst: Option<&Rc<Fiber>>,
) -> Option<Rc<Fiber>> {
    if dom_event_name == "click" {
        return get_inst_if_value_changed(target_inst);
    }
    None
}

fn get_target_inst_for_input_or_change_event(
    dom_event_name: &str,
    target_inst: Option<&Rc<Fiber>>,
) -> Option<Rc<Fiber>> {
    if dom_event_name == "input" || dom_event_name == "change" {
        return get_inst_if_value_changed(target_inst);
    }
    None
}

/// What finds the instance an `onChange` event is dispatched to, of an event name.
type GetTargetInstFunc = fn(&str, Option<&Rc<Fiber>>) -> Option<Rc<Fiber>>;

/// What handles an event of the input event polyfill.
type HandleEventFunc = fn(&str, Option<&'static Unknown>, Option<Rc<Fiber>>);

/// This plugin creates an `onChange` event that normalizes change events
/// across form elements. This event fires at a time when it's possible to
/// change the element's value without seeing a flicker.
///
/// Supported elements are:
/// - input (see `isTextInputElement`)
/// - textarea
/// - select
#[cfg_attr(
    rust_js,
    rust_js::nullable(target_inst, native_event_target, _target_container)
)]
pub fn extract_events(
    dispatch_queue: &mut DispatchQueue,
    dom_event_name: &str,
    target_inst: Option<Rc<Fiber>>,
    native_event: &'static AnyNativeEvent,
    native_event_target: Option<&'static EventTarget>,
    _event_system_flags: EventSystemFlags,
    _target_container: Option<&'static EventTarget>,
) {
    let target_node = match &target_inst {
        Some(target_inst) => get_node_from_instance(target_inst),
        None => Some(js::unknown(window)),
    };

    let mut get_target_inst_func: Option<GetTargetInstFunc> = None;
    let mut handle_event_func: Option<HandleEventFunc> = None;
    if should_use_change_event(unsafe { js::cast(target_node) }) {
        get_target_inst_func = Some(get_target_inst_for_change_event);
    } else if is_text_input_element(unsafe { js::cast(target_node) }) {
        if IS_INPUT_EVENT_SUPPORTED.with(|is_input_event_supported| *is_input_event_supported) {
            get_target_inst_func = Some(get_target_inst_for_input_or_change_event);
        } else {
            get_target_inst_func = Some(get_target_inst_for_input_event_polyfill);
            handle_event_func = Some(handle_events_for_input_event_polyfill);
        }
    } else if should_use_click_event(unsafe { js::cast(target_node) }) {
        get_target_inst_func = Some(get_target_inst_for_click_event);
    } else if let Some(target_inst) = &target_inst
        && is_custom_element(
            unsafe { js::cast(target_inst.element_type.get()) },
            unsafe { js::cast(target_inst.memoized_props.get()) },
        )
    {
        get_target_inst_func = Some(get_target_inst_for_change_event);
    }

    if let Some(get_target_inst_func) = get_target_inst_func {
        let inst = get_target_inst_func(dom_event_name, target_inst.as_ref());
        if inst.is_some() {
            create_and_accumulate_change_event(
                dispatch_queue,
                inst,
                native_event,
                native_event_target,
            );
            return;
        }
    }

    if let Some(handle_event_func) = handle_event_func {
        handle_event_func(dom_event_name, target_node, target_inst);
    }
}
