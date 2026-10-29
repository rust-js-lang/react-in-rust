// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;
use std::rc::Rc;

use js::{Unknown, string};
use react_reconciler::react_internal_types::Fiber;
use shared::execution_environment::CAN_USE_DOM;
use webapi::{EventTarget, document, keyboard_event, window};

use crate::dom_plugin_event_system::{
    DispatchEntry, DispatchQueue, accumulate_two_phase_listeners,
};
use crate::event_registry::register_two_phase_event;
use crate::event_system_flags::EventSystemFlags;
use crate::fallback_composition_state::{
    get_data as fallback_composition_state_get_data,
    initialize as fallback_composition_state_initialize, reset as fallback_composition_state_reset,
};
use crate::plugin_module_type::AnyNativeEvent;
use crate::react_synthetic_event_type::ReactSyntheticEvent;
use crate::synthetic_event::{SYNTHETIC_COMPOSITION_EVENT, SYNTHETIC_INPUT_EVENT};

const END_KEYCODES: [f64; 4] = [9.0, 13.0, 27.0, 32.0]; // Tab, Return, Esc, Space
const START_KEYCODE: f64 = 229.0;

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "canUseCompositionEvent")]
    static CAN_USE_COMPOSITION_EVENT: bool =
        CAN_USE_DOM.with(|can_use_dom| *can_use_dom) && js::has(window, "CompositionEvent");

    static DOCUMENT_MODE: Option<&'static Unknown> =
        if CAN_USE_DOM.with(|can_use_dom| *can_use_dom) && js::has(document, "documentMode") {
            js::get(document, "documentMode")
        } else {
            js::null()
        };

    // Webkit offers a very useful `textInput` event that can be used to
    // directly represent `beforeInput`. The IE `textinput` event is not as
    // useful, so we don't use it.
    #[cfg_attr(rust_js, rust_js::name = "canUseTextInputEvent")]
    static CAN_USE_TEXT_INPUT_EVENT: bool = CAN_USE_DOM.with(|can_use_dom| *can_use_dom)
        && js::has(window, "TextEvent")
        && !js::truthy(DOCUMENT_MODE.with(|document_mode| *document_mode));

    // In IE9+, we have access to composition events, but the data supplied
    // by the native compositionend event may be incorrect. Japanese ideographic
    // spaces, for instance (　) are not recorded correctly.
    #[cfg_attr(rust_js, rust_js::name = "useFallbackCompositionData")]
    static USE_FALLBACK_COMPOSITION_DATA: bool = CAN_USE_DOM.with(|can_use_dom| *can_use_dom)
        && (!CAN_USE_COMPOSITION_EVENT.with(|can_use_composition_event| *can_use_composition_event)
            || DOCUMENT_MODE.with(|&document_mode| {
                js::truthy(document_mode)
                    && unsafe { js::cast::<f64>(document_mode) } > 8.0
                    && unsafe { js::cast::<f64>(document_mode) } <= 11.0
            }));
}

const SPACEBAR_CODE: f64 = 32.0;

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "SPACEBAR_CHAR")]
    static SPACEBAR_CHAR: &'static Unknown = js::unknown_of(string::from_char_code((SPACEBAR_CODE,)));
}

pub fn register_events() {
    register_two_phase_event(
        "onBeforeInput",
        Rc::new(vec![
            "compositionend".to_owned(),
            "keypress".to_owned(),
            "textInput".to_owned(),
            "paste".to_owned(),
        ]),
    );
    register_two_phase_event(
        "onCompositionEnd",
        Rc::new(vec![
            "compositionend".to_owned(),
            "focusout".to_owned(),
            "keydown".to_owned(),
            "keypress".to_owned(),
            "keyup".to_owned(),
            "mousedown".to_owned(),
        ]),
    );
    register_two_phase_event(
        "onCompositionStart",
        Rc::new(vec![
            "compositionstart".to_owned(),
            "focusout".to_owned(),
            "keydown".to_owned(),
            "keypress".to_owned(),
            "keyup".to_owned(),
            "mousedown".to_owned(),
        ]),
    );
    register_two_phase_event(
        "onCompositionUpdate",
        Rc::new(vec![
            "compositionupdate".to_owned(),
            "focusout".to_owned(),
            "keydown".to_owned(),
            "keypress".to_owned(),
            "keyup".to_owned(),
            "mousedown".to_owned(),
        ]),
    );
}

thread_local! {
    // Track whether we've ever handled a keypress on the space key.
    static HAS_SPACE_KEYPRESS: Cell<bool> = const { Cell::new(false) };
}

/// Return whether a native keypress event is assumed to be a command.
/// This is required because Firefox fires `keypress` events for key commands
/// (cut, copy, select-all, etc.) even though no character is inserted.
fn is_keypress_command(native_event: &AnyNativeEvent) -> bool {
    let native_event = keyboard_event::unchecked_from(native_event);
    (native_event.ctrl_key() || native_event.alt_key() || native_event.meta_key())
        // ctrlKey && altKey is equivalent to AltGr, and is not a command.
        && !(native_event.ctrl_key() && native_event.alt_key())
}

/// Translate native top level events into event types.
fn get_composition_event_type(dom_event_name: &str) -> Option<&'static str> {
    match dom_event_name {
        "compositionstart" => Some("onCompositionStart"),
        "compositionend" => Some("onCompositionEnd"),
        "compositionupdate" => Some("onCompositionUpdate"),
        _ => None,
    }
}

/// Does our fallback best-guess model think this event signifies that
/// composition has begun?
fn is_fallback_composition_start(dom_event_name: &str, native_event: &AnyNativeEvent) -> bool {
    dom_event_name == "keydown"
        && keyboard_event::unchecked_from(native_event).key_code() == START_KEYCODE
}

/// Does our fallback mode think that this event is the end of composition?
fn is_fallback_composition_end(dom_event_name: &str, native_event: &AnyNativeEvent) -> bool {
    match dom_event_name {
        "keyup" => {
            // Command keys insert or clear IME input.
            END_KEYCODES.contains(&keyboard_event::unchecked_from(native_event).key_code())
        }
        "keydown" => {
            // Expect IME keyCode on each keydown. If we get any other
            // code we must have exited earlier.
            keyboard_event::unchecked_from(native_event).key_code() != START_KEYCODE
        }
        "keypress" | "mousedown" | "focusout" => {
            // Events are not possible without cancelling IME.
            true
        }
        _ => false,
    }
}

/// Google Input Tools provides composition data via a CustomEvent,
/// with the `data` property populated in the `detail` object. If this
/// is available on the event object, use it. If not, this is a plain
/// composition event and we have nothing special to extract.
///
/// @param {object} nativeEvent
/// @return {?string}
fn get_data_from_custom_event(native_event: &AnyNativeEvent) -> Option<&'static Unknown> {
    let detail = js::get(native_event, "detail");
    if js::type_of(detail) == "object" && js::has(detail, "data") {
        return js::get(detail, "data");
    }
    js::null()
}

/// Check if a composition event was triggered by Korean IME.
/// Our fallback mode does not work well with IE's Korean IME,
/// so just use native composition events when Korean IME is used.
/// Although CompositionEvent.locale property is deprecated,
/// it is available in IE, where our fallback mode is enabled.
///
/// @param {object} nativeEvent
/// @return {boolean}
#[cfg_attr(rust_js, rust_js::name = "isUsingKoreanIME")]
fn is_using_korean_ime(native_event: &AnyNativeEvent) -> bool {
    js::strict_equals(js::get(native_event, "locale"), "ko")
}

thread_local! {
    // Track the current IME composition status, if any.
    static IS_COMPOSING: Cell<bool> = const { Cell::new(false) };
}

/// @return {?object} A SyntheticCompositionEvent.
fn extract_composition_event(
    dispatch_queue: &mut DispatchQueue,
    dom_event_name: &str,
    target_inst: Option<Rc<Fiber>>,
    native_event: &'static AnyNativeEvent,
    native_event_target: Option<&'static EventTarget>,
) {
    let mut event_type = None;
    let mut fallback_data = None;

    if CAN_USE_COMPOSITION_EVENT.with(|can_use_composition_event| *can_use_composition_event) {
        event_type = get_composition_event_type(dom_event_name);
    } else if !IS_COMPOSING.get() {
        if is_fallback_composition_start(dom_event_name, native_event) {
            event_type = Some("onCompositionStart");
        }
    } else if is_fallback_composition_end(dom_event_name, native_event) {
        event_type = Some("onCompositionEnd");
    }

    let Some(event_type) = event_type else {
        return;
    };

    if USE_FALLBACK_COMPOSITION_DATA
        .with(|use_fallback_composition_data| *use_fallback_composition_data)
        && !is_using_korean_ime(native_event)
    {
        // The current composition is stored statically and must not be
        // overwritten while composition continues.
        if !IS_COMPOSING.get() && event_type == "onCompositionStart" {
            IS_COMPOSING.set(fallback_composition_state_initialize(native_event_target));
        } else if event_type == "onCompositionEnd" && IS_COMPOSING.get() {
            fallback_data = Some(fallback_composition_state_get_data());
        }
    }

    let listeners = accumulate_two_phase_listeners(target_inst, event_type);
    if !listeners.is_empty() {
        let event: &'static ReactSyntheticEvent = js::new(
            SYNTHETIC_COMPOSITION_EVENT.with(|ctor| *ctor),
            (
                Some(event_type.to_owned()),
                dom_event_name.to_owned(),
                js::null(),
                native_event,
                native_event_target,
            ),
        );
        dispatch_queue.push(DispatchEntry { event, listeners });
        match fallback_data {
            Some(fallback_data) if !fallback_data.is_empty() => {
                // Inject data generated from fallback path into the synthetic event.
                // This matches the property of native CompositionEventInterface.
                js::set(event, "data", fallback_data);
            }
            _ => {
                let custom_data = get_data_from_custom_event(native_event);
                if !js::strict_equals(custom_data, js::null::<&Unknown>()) {
                    js::set(event, "data", custom_data);
                }
            }
        }
    }
}

fn get_native_before_input_chars(
    dom_event_name: &str,
    native_event: &AnyNativeEvent,
) -> Option<&'static Unknown> {
    match dom_event_name {
        "compositionend" => get_data_from_custom_event(native_event),
        "keypress" => {
            // If native `textInput` events are available, our goal is to make
            // use of them. However, there is a special case: the spacebar key.
            // In Webkit, preventing default on a spacebar `textInput` event
            // cancels character insertion, but it *also* causes the browser
            // to fall back to its default spacebar behavior of scrolling the
            // page.
            //
            // Tracking at:
            // https://code.google.com/p/chromium/issues/detail?id=355103
            //
            // To avoid this issue, use the keypress event as if no `textInput`
            // event is available.
            let which = js::get(native_event, "which");
            if !js::strict_equals(which, SPACEBAR_CODE) {
                return js::null();
            }

            HAS_SPACE_KEYPRESS.set(true);
            Some(SPACEBAR_CHAR.with(|spacebar_char| *spacebar_char))
        }
        "textInput" => {
            // Record the characters to be added to the DOM.
            let chars = js::get(native_event, "data");

            // If it's a spacebar character, assume that we have already handled
            // it at the keypress level and bail immediately. Android Chrome
            // doesn't give us keycodes, so we need to ignore it.
            if js::strict_equals(chars, SPACEBAR_CHAR.with(|spacebar_char| *spacebar_char))
                && HAS_SPACE_KEYPRESS.get()
            {
                return js::null();
            }

            chars
        }
        _ => {
            // For other native event types, do nothing.
            js::null()
        }
    }
}

/// For browsers that do not provide the `textInput` event, extract the
/// appropriate string to use for SyntheticInputEvent.
fn get_fallback_before_input_chars(
    dom_event_name: &str,
    native_event: &AnyNativeEvent,
) -> Option<&'static Unknown> {
    // If we are currently composing (IME) and using a fallback to do so,
    // try to extract the composed characters from the fallback object.
    // If composition event is available, we extract a string only at
    // compositionevent, otherwise extract it at fallback events.
    if IS_COMPOSING.get() {
        if dom_event_name == "compositionend"
            || (!CAN_USE_COMPOSITION_EVENT
                .with(|can_use_composition_event| *can_use_composition_event)
                && is_fallback_composition_end(dom_event_name, native_event))
        {
            let chars = fallback_composition_state_get_data();
            fallback_composition_state_reset();
            IS_COMPOSING.set(false);
            return Some(js::unknown_of(chars));
        }
        return js::null();
    }

    match dom_event_name {
        "paste" => {
            // If a paste event occurs after a keypress, throw out the input
            // chars. Paste events should not lead to BeforeInput events.
            js::null()
        }
        "keypress" => {
            // As of v27, Firefox may fire keypress events even when no character
            // will be inserted. A few possibilities:
            //
            // - `which` is `0`. Arrow keys, Esc key, etc.
            //
            // - `which` is the pressed key code, but no char is available.
            //   Ex: 'AltGr + d` in Polish. There is no modified character for
            //   this key combination and no character is inserted into the
            //   document, but FF fires the keypress for char code `100` anyway.
            //   No `input` event will occur.
            //
            // - `which` is the pressed key code, but a command combination is
            //   being used. Ex: `Cmd+C`. No character is inserted, and no
            //   `input` event will occur.
            if !is_keypress_command(native_event) {
                // IE fires the `keypress` event when a user types an emoji via
                // Touch keyboard of Windows.  In such a case, the `char` property
                // holds an emoji character like `😊`.  Because its length
                // is 2, the property `which` does not represent an emoji correctly.
                // In such a case, we directly return the `char` property instead of
                // using `which`.
                let char = js::get(native_event, "char");
                if js::truthy(char) && unsafe { js::cast::<f64>(js::get(char, "length")) } > 1.0 {
                    return char;
                } else if js::truthy(js::get(native_event, "which")) {
                    return Some(js::unknown_of(string::from_char_code((unsafe {
                        js::cast::<f64>(js::get(native_event, "which"))
                    },))));
                }
            }
            js::null()
        }
        "compositionend" => {
            if USE_FALLBACK_COMPOSITION_DATA
                .with(|use_fallback_composition_data| *use_fallback_composition_data)
                && !is_using_korean_ime(native_event)
            {
                js::null()
            } else {
                js::get(native_event, "data")
            }
        }
        _ => js::null(),
    }
}

/// Extract a SyntheticInputEvent for `beforeInput`, based on either native
/// `textInput` or fallback behavior.
///
/// @return {?object} A SyntheticInputEvent.
fn extract_before_input_event(
    dispatch_queue: &mut DispatchQueue,
    dom_event_name: &str,
    target_inst: Option<Rc<Fiber>>,
    native_event: &'static AnyNativeEvent,
    native_event_target: Option<&'static EventTarget>,
) {
    let chars =
        if CAN_USE_TEXT_INPUT_EVENT.with(|can_use_text_input_event| *can_use_text_input_event) {
            get_native_before_input_chars(dom_event_name, native_event)
        } else {
            get_fallback_before_input_chars(dom_event_name, native_event)
        };

    // If no characters are being inserted, no BeforeInput event should
    // be fired.
    if !js::truthy(chars) {
        return;
    }

    let listeners = accumulate_two_phase_listeners(target_inst, "onBeforeInput");
    if !listeners.is_empty() {
        let event: &'static ReactSyntheticEvent = js::new(
            SYNTHETIC_INPUT_EVENT.with(|ctor| *ctor),
            (
                Some("onBeforeInput".to_owned()),
                "beforeinput".to_owned(),
                js::null(),
                native_event,
                native_event_target,
            ),
        );
        dispatch_queue.push(DispatchEntry { event, listeners });
        js::set(event, "data", chars);
    }
}

/// Create an `onBeforeInput` event to match
/// http://www.w3.org/TR/2013/WD-DOM-Level-3-Events-20131105/#events-inputevents.
///
/// This event plugin is based on the native `textInput` event
/// available in Chrome, Safari, Opera, and IE. This event fires after
/// `onKeyPress` and `onCompositionEnd`, but before `onInput`.
///
/// `beforeInput` is spec'd but not implemented in any browsers, and
/// the `input` event does not provide any useful information about what has
/// actually been added, contrary to the spec. Thus, `textInput` is the best
/// available event to identify the characters that have actually been inserted
/// into the target node.
///
/// This plugin is also responsible for emitting `composition` events, thus
/// allowing us to share composition fallback code for both `beforeInput` and
/// `composition` event types.
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
    extract_composition_event(
        dispatch_queue,
        dom_event_name,
        target_inst.clone(),
        native_event,
        native_event_target,
    );
    extract_before_input_event(
        dispatch_queue,
        dom_event_name,
        target_inst,
        native_event,
        native_event_target,
    );
}
