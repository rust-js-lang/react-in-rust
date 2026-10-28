// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;
use std::rc::Rc;

use js::{Class, Dict, Kind, Unknown, dict, function, string};
use react_reconciler::react_internal_types::Fiber;
use shared::assign::assign;
use webapi::{Event, EventTarget, KeyboardEvent, MouseEvent, window};

use crate::get_event_char_code::get_event_char_code;
use crate::react_synthetic_event_type::ReactSyntheticEvent;

/// `0 | ((event) => mixed)`: the native event's property, copied, or what a
/// function makes of the native event.
#[cfg_attr(rust_js, rust_js::untagged)]
#[derive(Clone, Copy)]
pub enum Normalize {
    Copied(u8),
    Made(fn(&'static Unknown) -> Option<&'static Unknown>),
}

type EventInterfaceType = Dict<Normalize>;

fn function_that_returns_true() -> bool {
    true
}

fn function_that_returns_false() -> bool {
    false
}

/// A constructor of synthetic events, of an interface's.
pub type SyntheticEventClass = Class<
    fn(
        Option<&'static str>,
        &'static str,
        Option<Rc<Fiber>>,
        &'static Event,
        Option<&'static EventTarget>,
    ) -> ReactSyntheticEvent,
>;

/// What each constructor's prototype is given.
struct SyntheticEventPrototype {
    prevent_default: Box<dyn Fn()>,
    stop_propagation: Box<dyn Fn()>,
    /// We release all dispatched `SyntheticEvent`s after each event loop, adding
    /// them back into the pool. This allows a way to hold onto a reference that
    /// won't be added back into the pool.
    persist: Box<dyn Fn()>,
    /// Checks if this event should be released back into the pool.
    ///
    /// @return {boolean} True if this should not be released, false otherwise.
    is_persistent: fn() -> bool,
}

unsafe impl js::Defined for SyntheticEventPrototype {}

// This is intentionally a factory so that we have different returned constructors.
// If we had a single constructor, it would be megamorphic and engines would deopt.
fn create_synthetic_event(
    #[cfg_attr(rust_js, rust_js::name = "Interface")] interface: &'static EventInterfaceType,
) -> &'static SyntheticEventClass {
    // Synthetic events are dispatched by event plugins, typically in response to a
    // top-level event delegation handler.
    //
    // These systems should generally use pooling to reduce the frequency of garbage
    // collection. The system should check `isPersistent` to determine whether the
    // event should be released into the pool after being dispatched. Users that
    // need a persisted event should invoke `persist`.
    //
    // Synthetic events (and subclasses) implement the DOM Level 3 Events API by
    // normalizing browser quirks. Subclasses do not necessarily have to implement a
    // DOM interface; custom application-specific events can also subclass this.
    #[cfg_attr(rust_js, rust_js::name = "SyntheticBaseEvent")]
    let synthetic_base_event = Class::of(function::with_this(
        move |this,
              (react_name, react_event_type, target_inst, native_event, native_event_target): (
            Option<&'static str>,
            &'static str,
            Option<Rc<Fiber>>,
            &'static Event,
            Option<&'static EventTarget>,
        )| {
            let this: &'static ReactSyntheticEvent = unsafe { js::cast(this) };
            this.react_name.set(react_name);
            *this.target_inst.borrow_mut() = target_inst;
            this.r#type.set(react_event_type);
            this.native_event.set(Some(native_event));
            this.target.set(native_event_target.map(js::unknown));
            this.current_target.set(None);

            for prop_name in js::keys_in(interface) {
                if !js::unknown(interface).has_own_property(&prop_name) {
                    continue;
                }
                let normalize = dict::get(interface, &prop_name);
                if let Some(Normalize::Made(normalize)) = normalize {
                    js::set(this, &prop_name, normalize(js::unknown(native_event)));
                } else {
                    js::set(this, &prop_name, js::get(native_event, &prop_name));
                }
            }

            let default_prevented = if js::get(native_event, "defaultPrevented").is_some() {
                js::get(native_event, "defaultPrevented")
            } else {
                Some(js::unknown_of(js::strict_equals(
                    js::get(native_event, "returnValue"),
                    false,
                )))
            };
            if js::truthy(default_prevented) {
                this.is_default_prevented.set(function_that_returns_true);
            } else {
                this.is_default_prevented.set(function_that_returns_false);
            }
            this.is_propagation_stopped.set(function_that_returns_false);
            this
        },
    ));

    assign(
        synthetic_base_event.prototype(),
        (&SyntheticEventPrototype {
            prevent_default: function::with_this(|this, ()| {
                let this: &'static ReactSyntheticEvent = unsafe { js::cast(this) };
                js::set(this, "defaultPrevented", true);
                let Some(event) = this.native_event.get() else {
                    return;
                };

                if js::truthy(js::get(event, "preventDefault")) {
                    event.prevent_default();
                } else if js::type_of(js::get(event, "returnValue")) != "unknown" {
                    js::set(event, "returnValue", false);
                }
                this.is_default_prevented.set(function_that_returns_true);
            }),

            stop_propagation: function::with_this(|this, ()| {
                let this: &'static ReactSyntheticEvent = unsafe { js::cast(this) };
                let Some(event) = this.native_event.get() else {
                    return;
                };

                if js::truthy(js::get(event, "stopPropagation")) {
                    event.stop_propagation();
                } else if js::type_of(js::get(event, "cancelBubble")) != "unknown" {
                    // The ChangeEventPlugin registers a "propertychange" event for
                    // IE. This event does not support bubbling or cancelling, and
                    // any references to cancelBubble throw "Member not found".  A
                    // typeof check of "unknown" circumvents this issue (and is also
                    // IE specific).
                    js::set(event, "cancelBubble", true);
                }

                this.is_propagation_stopped.set(function_that_returns_true);
            }),

            persist: function::with_this(|_, ()| {
                // Modern event system doesn't use pooling.
            }),

            is_persistent: function_that_returns_true,
        },),
    );
    synthetic_base_event
}

thread_local! {
    /// @interface Event
    /// @see http://www.w3.org/TR/DOM-Level-3-Events/
    #[cfg_attr(rust_js, rust_js::name = "EventInterface")]
    static EVENT_INTERFACE: &'static EventInterfaceType = dict::from_entries(vec![
        ("eventPhase", Normalize::Copied(0)),
        ("bubbles", Normalize::Copied(0)),
        ("cancelable", Normalize::Copied(0)),
        (
            "timeStamp",
            Normalize::Made(|event| {
                js::get(event, "timeStamp")
                    .filter(|time_stamp| js::truthy(Some(*time_stamp)))
                    .or_else(|| Some(js::unknown_of(js::date::now())))
            }),
        ),
        ("defaultPrevented", Normalize::Copied(0)),
        ("isTrusted", Normalize::Copied(0)),
    ]);

    #[cfg_attr(rust_js, rust_js::name = "SyntheticEvent")]
    pub static SYNTHETIC_EVENT: &'static SyntheticEventClass =
        create_synthetic_event(EVENT_INTERFACE.with(|i| *i));

    #[cfg_attr(rust_js, rust_js::name = "UIEventInterface")]
    static UI_EVENT_INTERFACE: &'static EventInterfaceType = js::object::spread((EVENT_INTERFACE.with(|i| *i), dict::from_entries(vec![("view", Normalize::Copied(0)), ("detail", Normalize::Copied(0))])));

    #[cfg_attr(rust_js, rust_js::name = "SyntheticUIEvent")]
    pub static SYNTHETIC_UI_EVENT: &'static SyntheticEventClass =
        create_synthetic_event(UI_EVENT_INTERFACE.with(|i| *i));

    static LAST_MOVEMENT_X: Cell<Option<f64>> = const { Cell::new(None) };
    static LAST_MOVEMENT_Y: Cell<Option<f64>> = const { Cell::new(None) };
    static LAST_MOUSE_EVENT: Cell<Option<&'static Unknown>> = const { Cell::new(None) };
}

fn update_mouse_movement_polyfill_state(event: &'static Unknown) {
    if Some(event) != LAST_MOUSE_EVENT.get() {
        if LAST_MOUSE_EVENT.get().is_some()
            && js::strict_equals(js::get(event, "type"), "mousemove")
        {
            let event: &MouseEvent = unsafe { js::cast(Some(event)) };
            LAST_MOVEMENT_X.set(Some(
                event.screen_x()
                    - unsafe { js::cast::<&MouseEvent>(LAST_MOUSE_EVENT.get()) }.screen_x(),
            ));
            LAST_MOVEMENT_Y.set(Some(
                event.screen_y()
                    - unsafe { js::cast::<&MouseEvent>(LAST_MOUSE_EVENT.get()) }.screen_y(),
            ));
        } else {
            LAST_MOVEMENT_X.set(Some(0.0));
            LAST_MOVEMENT_Y.set(Some(0.0));
        }
        LAST_MOUSE_EVENT.set(Some(event));
    }
}

thread_local! {
    /// @interface MouseEvent
    /// @see http://www.w3.org/TR/DOM-Level-3-Events/
    #[cfg_attr(rust_js, rust_js::name = "MouseEventInterface")]
    static MOUSE_EVENT_INTERFACE: &'static EventInterfaceType = js::object::spread((UI_EVENT_INTERFACE.with(|i| *i), dict::from_entries(vec![
            ("screenX", Normalize::Copied(0)),
            ("screenY", Normalize::Copied(0)),
            ("clientX", Normalize::Copied(0)),
            ("clientY", Normalize::Copied(0)),
            ("pageX", Normalize::Copied(0)),
            ("pageY", Normalize::Copied(0)),
            ("ctrlKey", Normalize::Copied(0)),
            ("shiftKey", Normalize::Copied(0)),
            ("altKey", Normalize::Copied(0)),
            ("metaKey", Normalize::Copied(0)),
            ("getModifierState", Normalize::Made(get_event_modifier_state)),
            ("button", Normalize::Copied(0)),
            ("buttons", Normalize::Copied(0)),
            (
                "relatedTarget",
                Normalize::Made(|event| {
                    if matches!(js::classify(js::get(event, "relatedTarget")), Kind::Undefined) {
                        return if js::strict_equals(
                            js::get(event, "fromElement"),
                            js::get(event, "srcElement"),
                        ) {
                            js::get(event, "toElement")
                        } else {
                            js::get(event, "fromElement")
                        };
                    }

                    js::get(event, "relatedTarget")
                }),
            ),
            (
                "movementX",
                Normalize::Made(|event| {
                    if js::has(event, "movementX") {
                        return js::get(event, "movementX");
                    }
                    update_mouse_movement_polyfill_state(event);
                    LAST_MOVEMENT_X.get().map(js::unknown_of)
                }),
            ),
            (
                "movementY",
                Normalize::Made(|event| {
                    if js::has(event, "movementY") {
                        return js::get(event, "movementY");
                    }
                    // Don't need to call updateMouseMovementPolyfillState() here
                    // because it's guaranteed to have already run when movementX
                    // was copied.
                    LAST_MOVEMENT_Y.get().map(js::unknown_of)
                }),
            ),
        ])));

    #[cfg_attr(rust_js, rust_js::name = "SyntheticMouseEvent")]
    pub static SYNTHETIC_MOUSE_EVENT: &'static SyntheticEventClass =
        create_synthetic_event(MOUSE_EVENT_INTERFACE.with(|i| *i));

    /// @interface DragEvent
    /// @see http://www.w3.org/TR/DOM-Level-3-Events/
    #[cfg_attr(rust_js, rust_js::name = "DragEventInterface")]
    static DRAG_EVENT_INTERFACE: &'static EventInterfaceType = js::object::spread((MOUSE_EVENT_INTERFACE.with(|i| *i), dict::from_entries(vec![("dataTransfer", Normalize::Copied(0))])));
    #[cfg_attr(rust_js, rust_js::name = "SyntheticDragEvent")]
    pub static SYNTHETIC_DRAG_EVENT: &'static SyntheticEventClass =
        create_synthetic_event(DRAG_EVENT_INTERFACE.with(|i| *i));

    /// @interface FocusEvent
    /// @see http://www.w3.org/TR/DOM-Level-3-Events/
    #[cfg_attr(rust_js, rust_js::name = "FocusEventInterface")]
    static FOCUS_EVENT_INTERFACE: &'static EventInterfaceType = js::object::spread((UI_EVENT_INTERFACE.with(|i| *i), dict::from_entries(vec![("relatedTarget", Normalize::Copied(0))])));
    #[cfg_attr(rust_js, rust_js::name = "SyntheticFocusEvent")]
    pub static SYNTHETIC_FOCUS_EVENT: &'static SyntheticEventClass =
        create_synthetic_event(FOCUS_EVENT_INTERFACE.with(|i| *i));

    /// @interface Event
    /// @see http://www.w3.org/TR/css3-animations/#AnimationEvent-interface
    /// @see https://developer.mozilla.org/en-US/docs/Web/API/AnimationEvent
    #[cfg_attr(rust_js, rust_js::name = "AnimationEventInterface")]
    static ANIMATION_EVENT_INTERFACE: &'static EventInterfaceType = js::object::spread((EVENT_INTERFACE.with(|i| *i), dict::from_entries(vec![
            ("animationName", Normalize::Copied(0)),
            ("elapsedTime", Normalize::Copied(0)),
            ("pseudoElement", Normalize::Copied(0)),
        ])));
    #[cfg_attr(rust_js, rust_js::name = "SyntheticAnimationEvent")]
    pub static SYNTHETIC_ANIMATION_EVENT: &'static SyntheticEventClass =
        create_synthetic_event(ANIMATION_EVENT_INTERFACE.with(|i| *i));

    /// @interface Event
    /// @see http://www.w3.org/TR/clipboard-apis/
    #[cfg_attr(rust_js, rust_js::name = "ClipboardEventInterface")]
    static CLIPBOARD_EVENT_INTERFACE: &'static EventInterfaceType = js::object::spread((EVENT_INTERFACE.with(|i| *i), dict::from_entries(vec![(
            "clipboardData",
            Normalize::Made(|event| {
                if js::has(event, "clipboardData") {
                    js::get(event, "clipboardData")
                } else {
                    js::get(window, "clipboardData")
                }
            }),
        )])));
    #[cfg_attr(rust_js, rust_js::name = "SyntheticClipboardEvent")]
    pub static SYNTHETIC_CLIPBOARD_EVENT: &'static SyntheticEventClass =
        create_synthetic_event(CLIPBOARD_EVENT_INTERFACE.with(|i| *i));

    /// @interface Event
    /// @see http://www.w3.org/TR/DOM-Level-3-Events/#events-compositionevents
    #[cfg_attr(rust_js, rust_js::name = "CompositionEventInterface")]
    static COMPOSITION_EVENT_INTERFACE: &'static EventInterfaceType =
        js::object::spread((EVENT_INTERFACE.with(|i| *i), dict::from_entries(vec![("data", Normalize::Copied(0))])));
    #[cfg_attr(rust_js, rust_js::name = "SyntheticCompositionEvent")]
    pub static SYNTHETIC_COMPOSITION_EVENT: &'static SyntheticEventClass =
        create_synthetic_event(COMPOSITION_EVENT_INTERFACE.with(|i| *i));

    /// @interface Event
    /// @see http://www.w3.org/TR/2013/WD-DOM-Level-3-Events-20131105
    ///      /#events-inputevents
    // Happens to share the same list for now.
    #[cfg_attr(rust_js, rust_js::name = "SyntheticInputEvent")]
    pub static SYNTHETIC_INPUT_EVENT: &'static SyntheticEventClass =
        SYNTHETIC_COMPOSITION_EVENT.with(|c| *c);

    /// Normalization of deprecated HTML5 `key` values
    /// @see https://developer.mozilla.org/en-US/docs/Web/API/KeyboardEvent#Key_names
    static NORMALIZE_KEY: &'static Dict<&'static str> = dict::from_entries(vec![
        ("Esc", "Escape"),
        ("Spacebar", " "),
        ("Left", "ArrowLeft"),
        ("Up", "ArrowUp"),
        ("Right", "ArrowRight"),
        ("Down", "ArrowDown"),
        ("Del", "Delete"),
        ("Win", "OS"),
        ("Menu", "ContextMenu"),
        ("Apps", "ContextMenu"),
        ("Scroll", "ScrollLock"),
        ("MozPrintableKey", "Unidentified"),
    ]);

    /// Translation from legacy `keyCode` to HTML5 `key`
    /// Only special keys supported, all others depend on keyboard layout or browser
    /// @see https://developer.mozilla.org/en-US/docs/Web/API/KeyboardEvent#Key_names
    static TRANSLATE_TO_KEY: &'static Dict<&'static str> = dict::from_entries(vec![
        ("8", "Backspace"),
        ("9", "Tab"),
        ("12", "Clear"),
        ("13", "Enter"),
        ("16", "Shift"),
        ("17", "Control"),
        ("18", "Alt"),
        ("19", "Pause"),
        ("20", "CapsLock"),
        ("27", "Escape"),
        ("32", " "),
        ("33", "PageUp"),
        ("34", "PageDown"),
        ("35", "End"),
        ("36", "Home"),
        ("37", "ArrowLeft"),
        ("38", "ArrowUp"),
        ("39", "ArrowRight"),
        ("40", "ArrowDown"),
        ("45", "Insert"),
        ("46", "Delete"),
        ("112", "F1"),
        ("113", "F2"),
        ("114", "F3"),
        ("115", "F4"),
        ("116", "F5"),
        ("117", "F6"),
        ("118", "F7"),
        ("119", "F8"),
        ("120", "F9"),
        ("121", "F10"),
        ("122", "F11"),
        ("123", "F12"),
        ("144", "NumLock"),
        ("145", "ScrollLock"),
        ("224", "Meta"),
    ]);
}

/// @param {object} nativeEvent Native browser event.
/// @return {string} Normalized `key` property.
fn get_event_key(native_event: &'static Unknown) -> Option<&'static Unknown> {
    if js::truthy(js::get(native_event, "key")) {
        // Normalize inconsistent values reported by browsers due to
        // implementations of a working draft specification.

        // FireFox implements `key` but returns `MozPrintableKey` for all
        // printable characters (normalized to `Unidentified`), ignore it.
        let key = NORMALIZE_KEY
            .with(|keys| js::get(*keys, &js::get(native_event, "key")))
            .filter(|key| js::truthy(Some(*key)))
            .or_else(|| js::get(native_event, "key"));
        if !js::strict_equals(key, "Unidentified") {
            return key;
        }
    }

    // Browser does not implement `key`, polyfill as much of it as we can.
    if js::strict_equals(js::get(native_event, "type"), "keypress") {
        let char_code = get_event_char_code(unsafe { js::cast(Some(native_event)) });

        // The enter-key is technically both printable and non-printable and can
        // thus be captured by `keypress`, no other non-printable key should.
        return Some(js::unknown_of(if char_code == 13.0 {
            "Enter".to_string()
        } else {
            string::from_char_code((char_code,))
        }));
    }
    if js::strict_equals(js::get(native_event, "type"), "keydown")
        || js::strict_equals(js::get(native_event, "type"), "keyup")
    {
        // While user keyboard layout determines the actual meaning of each
        // `keyCode` value, almost all function keys have a universal value.
        return TRANSLATE_TO_KEY
            .with(|keys| js::get(*keys, &js::get(native_event, "keyCode")))
            .filter(|key| js::truthy(Some(*key)))
            .or_else(|| Some(js::unknown_of("Unidentified")));
    }
    Some(js::unknown_of(""))
}

thread_local! {
    /// Translation from modifier key to the associated property in the event.
    /// @see http://www.w3.org/TR/DOM-Level-3-Events/#keys-Modifiers
    static MODIFIER_KEY_TO_PROP: &'static Dict<&'static str> = dict::from_entries(vec![
        ("Alt", "altKey"),
        ("Control", "ctrlKey"),
        ("Meta", "metaKey"),
        ("Shift", "shiftKey"),
    ]);

    // Older browsers (Safari <= 10, iOS Safari <= 10.2) do not support
    // getModifierState. If getModifierState is not supported, we map it to a set of
    // modifier keys exposed by the event. In this case, Lock-keys are not supported.
    static MODIFIER_STATE_GETTER: &'static Unknown =
        js::unknown_of(function::with_this(|this, (key_arg,): (&'static str,)| {
            let synthetic_event: &'static ReactSyntheticEvent = unsafe { js::cast(this) };
            let native_event = synthetic_event.native_event.get().unwrap();
            if js::truthy(js::get(native_event, "getModifierState")) {
                return unsafe { js::cast::<&KeyboardEvent>(Some(js::unknown(native_event))) }
                    .get_modifier_state(key_arg);
            }
            let key_prop = MODIFIER_KEY_TO_PROP.with(|props| dict::get(props, key_arg).copied());
            match key_prop {
                Some(key_prop) => js::truthy(js::get(native_event, key_prop)),
                None => false,
            }
        }));
}

fn get_event_modifier_state(_native_event: &'static Unknown) -> Option<&'static Unknown> {
    MODIFIER_STATE_GETTER.with(|getter| Some(*getter))
}

thread_local! {
    /// @interface KeyboardEvent
    /// @see http://www.w3.org/TR/DOM-Level-3-Events/
    #[cfg_attr(rust_js, rust_js::name = "KeyboardEventInterface")]
    static KEYBOARD_EVENT_INTERFACE: &'static EventInterfaceType = js::object::spread((UI_EVENT_INTERFACE.with(|i| *i), dict::from_entries(vec![
            ("key", Normalize::Made(get_event_key)),
            ("code", Normalize::Copied(0)),
            ("location", Normalize::Copied(0)),
            ("ctrlKey", Normalize::Copied(0)),
            ("shiftKey", Normalize::Copied(0)),
            ("altKey", Normalize::Copied(0)),
            ("metaKey", Normalize::Copied(0)),
            ("repeat", Normalize::Copied(0)),
            ("locale", Normalize::Copied(0)),
            ("getModifierState", Normalize::Made(get_event_modifier_state)),
            // Legacy Interface
            (
                "charCode",
                Normalize::Made(|event| {
                    // `charCode` is the result of a KeyPress event and represents the value of
                    // the actual printable character.

                    // KeyPress is deprecated, but its replacement is not yet final and not
                    // implemented in any major browser. Only KeyPress has charCode.
                    if js::strict_equals(js::get(event, "type"), "keypress") {
                        return Some(js::unknown_of(get_event_char_code(unsafe { js::cast(Some(event)) })));
                    }
                    Some(js::unknown_of(0))
                }),
            ),
            (
                "keyCode",
                Normalize::Made(|event| {
                    // `keyCode` is the result of a KeyDown/Up event and represents the value of
                    // physical keyboard key.

                    // The actual meaning of the value depends on the users' keyboard layout
                    // which cannot be detected. Assuming that it is a US keyboard layout
                    // provides a surprisingly accurate mapping for US and European users.
                    // Due to this, it is left to the user to implement at this time.
                    if js::strict_equals(js::get(event, "type"), "keydown")
                        || js::strict_equals(js::get(event, "type"), "keyup")
                    {
                        return js::get(event, "keyCode");
                    }
                    Some(js::unknown_of(0))
                }),
            ),
            (
                "which",
                Normalize::Made(|event| {
                    // `which` is an alias for either `keyCode` or `charCode` depending on the
                    // type of the event.
                    if js::strict_equals(js::get(event, "type"), "keypress") {
                        return Some(js::unknown_of(get_event_char_code(unsafe { js::cast(Some(event)) })));
                    }
                    if js::strict_equals(js::get(event, "type"), "keydown")
                        || js::strict_equals(js::get(event, "type"), "keyup")
                    {
                        return js::get(event, "keyCode");
                    }
                    Some(js::unknown_of(0))
                }),
            ),
        ])));
    #[cfg_attr(rust_js, rust_js::name = "SyntheticKeyboardEvent")]
    pub static SYNTHETIC_KEYBOARD_EVENT: &'static SyntheticEventClass =
        create_synthetic_event(KEYBOARD_EVENT_INTERFACE.with(|i| *i));

    /// @interface PointerEvent
    /// @see http://www.w3.org/TR/pointerevents/
    #[cfg_attr(rust_js, rust_js::name = "PointerEventInterface")]
    static POINTER_EVENT_INTERFACE: &'static EventInterfaceType = js::object::spread((MOUSE_EVENT_INTERFACE.with(|i| *i), dict::from_entries(vec![
            ("pointerId", Normalize::Copied(0)),
            ("width", Normalize::Copied(0)),
            ("height", Normalize::Copied(0)),
            ("pressure", Normalize::Copied(0)),
            ("tangentialPressure", Normalize::Copied(0)),
            ("tiltX", Normalize::Copied(0)),
            ("tiltY", Normalize::Copied(0)),
            ("twist", Normalize::Copied(0)),
            ("pointerType", Normalize::Copied(0)),
            ("isPrimary", Normalize::Copied(0)),
        ])));
    #[cfg_attr(rust_js, rust_js::name = "SyntheticPointerEvent")]
    pub static SYNTHETIC_POINTER_EVENT: &'static SyntheticEventClass =
        create_synthetic_event(POINTER_EVENT_INTERFACE.with(|i| *i));

    /// @interface SubmitEvent
    /// @see https://html.spec.whatwg.org/multipage/form-control-infrastructure.html#the-submitevent-interface
    #[cfg_attr(rust_js, rust_js::name = "SubmitEventInterface")]
    static SUBMIT_EVENT_INTERFACE: &'static EventInterfaceType =
        js::object::spread((EVENT_INTERFACE.with(|i| *i), dict::from_entries(vec![("submitter", Normalize::Copied(0))])));
    #[cfg_attr(rust_js, rust_js::name = "SyntheticSubmitEvent")]
    pub static SYNTHETIC_SUBMIT_EVENT: &'static SyntheticEventClass =
        create_synthetic_event(SUBMIT_EVENT_INTERFACE.with(|i| *i));

    /// @interface TouchEvent
    /// @see http://www.w3.org/TR/touch-events/
    #[cfg_attr(rust_js, rust_js::name = "TouchEventInterface")]
    static TOUCH_EVENT_INTERFACE: &'static EventInterfaceType = js::object::spread((UI_EVENT_INTERFACE.with(|i| *i), dict::from_entries(vec![
            ("touches", Normalize::Copied(0)),
            ("targetTouches", Normalize::Copied(0)),
            ("changedTouches", Normalize::Copied(0)),
            ("altKey", Normalize::Copied(0)),
            ("metaKey", Normalize::Copied(0)),
            ("ctrlKey", Normalize::Copied(0)),
            ("shiftKey", Normalize::Copied(0)),
            ("getModifierState", Normalize::Made(get_event_modifier_state)),
        ])));
    #[cfg_attr(rust_js, rust_js::name = "SyntheticTouchEvent")]
    pub static SYNTHETIC_TOUCH_EVENT: &'static SyntheticEventClass =
        create_synthetic_event(TOUCH_EVENT_INTERFACE.with(|i| *i));

    /// @interface Event
    /// @see http://www.w3.org/TR/2009/WD-css3-transitions-20090320/#transition-events-
    /// @see https://developer.mozilla.org/en-US/docs/Web/API/TransitionEvent
    #[cfg_attr(rust_js, rust_js::name = "TransitionEventInterface")]
    static TRANSITION_EVENT_INTERFACE: &'static EventInterfaceType = js::object::spread((EVENT_INTERFACE.with(|i| *i), dict::from_entries(vec![
            ("propertyName", Normalize::Copied(0)),
            ("elapsedTime", Normalize::Copied(0)),
            ("pseudoElement", Normalize::Copied(0)),
        ])));
    #[cfg_attr(rust_js, rust_js::name = "SyntheticTransitionEvent")]
    pub static SYNTHETIC_TRANSITION_EVENT: &'static SyntheticEventClass =
        create_synthetic_event(TRANSITION_EVENT_INTERFACE.with(|i| *i));

    /// @interface WheelEvent
    /// @see http://www.w3.org/TR/DOM-Level-3-Events/
    #[cfg_attr(rust_js, rust_js::name = "WheelEventInterface")]
    static WHEEL_EVENT_INTERFACE: &'static EventInterfaceType = js::object::spread((MOUSE_EVENT_INTERFACE.with(|i| *i), dict::from_entries(vec![
            (
                "deltaX",
                Normalize::Made(|event| {
                    if js::has(event, "deltaX") {
                        js::get(event, "deltaX")
                    } else if js::has(event, "wheelDeltaX") {
                        // Fallback to `wheelDeltaX` for Webkit and normalize (right is positive).
                        Some(js::unknown_of(-unsafe { js::cast::<f64>(js::get(event, "wheelDeltaX")) }))
                    } else {
                        Some(js::unknown_of(0))
                    }
                }),
            ),
            (
                "deltaY",
                Normalize::Made(|event| {
                    if js::has(event, "deltaY") {
                        js::get(event, "deltaY")
                    } else if js::has(event, "wheelDeltaY") {
                        // Fallback to `wheelDeltaY` for Webkit and normalize (down is positive).
                        Some(js::unknown_of(-unsafe { js::cast::<f64>(js::get(event, "wheelDeltaY")) }))
                    } else if js::has(event, "wheelDelta") {
                        // Fallback to `wheelDelta` for IE<9 and normalize (down is positive).
                        Some(js::unknown_of(-unsafe { js::cast::<f64>(js::get(event, "wheelDelta")) }))
                    } else {
                        Some(js::unknown_of(0))
                    }
                }),
            ),
            ("deltaZ", Normalize::Copied(0)),
            // Browsers without "deltaMode" is reporting in raw wheel delta where one
            // notch on the scroll is always +/- 120, roughly equivalent to pixels.
            // A good approximation of DOM_DELTA_LINE (1) is 5% of viewport size or
            // ~40 pixels, for DOM_DELTA_SCREEN (2) it is 87.5% of viewport size.
            ("deltaMode", Normalize::Copied(0)),
        ])));
    #[cfg_attr(rust_js, rust_js::name = "SyntheticWheelEvent")]
    pub static SYNTHETIC_WHEEL_EVENT: &'static SyntheticEventClass =
        create_synthetic_event(WHEEL_EVENT_INTERFACE.with(|i| *i));

    #[cfg_attr(rust_js, rust_js::name = "ToggleEventInterface")]
    static TOGGLE_EVENT_INTERFACE: &'static EventInterfaceType = js::object::spread((EVENT_INTERFACE.with(|i| *i), dict::from_entries(vec![
            ("newState", Normalize::Copied(0)),
            ("oldState", Normalize::Copied(0)),
            ("source", Normalize::Copied(0)),
        ])));
    #[cfg_attr(rust_js, rust_js::name = "SyntheticToggleEvent")]
    pub static SYNTHETIC_TOGGLE_EVENT: &'static SyntheticEventClass =
        create_synthetic_event(TOGGLE_EVENT_INTERFACE.with(|i| *i));
}
