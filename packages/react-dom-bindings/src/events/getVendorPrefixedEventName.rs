// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;

use js::{Dict, Unknown, dict};
use webapi::{document, tags::Div, window};

unsafe extern "Rust" {
    #[link_name = "shared/ExecutionEnvironment#canUseDOM"]
    safe static CAN_USE_DOM: bool;
}

/// Generate a mapping of standard vendor prefixes using the defined style property and event name.
///
/// @param {string} styleProp
/// @param {string} eventName
/// @returns {object}
fn make_prefix_map(style_prop: &str, event_name: &str) -> &'static Dict<String> {
    let prefixes = dict::new();

    dict::set(
        prefixes,
        &style_prop.to_lowercase(),
        event_name.to_lowercase(),
    );
    dict::set(
        prefixes,
        &("Webkit".to_owned() + style_prop),
        "webkit".to_owned() + event_name,
    );
    dict::set(
        prefixes,
        &("Moz".to_owned() + style_prop),
        "moz".to_owned() + event_name,
    );

    prefixes
}

thread_local! {
    /// A list of event names to a configurable list of vendor prefixes.
    static VENDOR_PREFIXES: &'static Dict<&'static Dict<String>> = dict::from_entries(vec![
        ("animationcancel", make_prefix_map("Animation", "AnimationCancel")),
        ("animationend", make_prefix_map("Animation", "AnimationEnd")),
        ("animationiteration", make_prefix_map("Animation", "AnimationIteration")),
        ("animationstart", make_prefix_map("Animation", "AnimationStart")),
        ("transitionrun", make_prefix_map("Transition", "TransitionRun")),
        ("transitionstart", make_prefix_map("Transition", "TransitionStart")),
        ("transitioncancel", make_prefix_map("Transition", "TransitionCancel")),
        ("transitionend", make_prefix_map("Transition", "TransitionEnd")),
    ]);

    /// Event names that have already been detected and prefixed (if applicable).
    static PREFIXED_EVENT_NAMES: &'static Dict<String> = dict::new();

    /// Element to check for prefixes on.
    static STYLE: Cell<&'static Unknown> = Cell::new(js::unknown(dict::new::<()>()));
}

// Bootstrap if a DOM exists.
js::on_load! {
    if CAN_USE_DOM {
        STYLE.set(js::unknown(document.create_element(Div).style()));

        // On some platforms, in particular some releases of Android 4.x,
        // the un-prefixed "animation" and "transition" properties are defined on the
        // style object but the events that fire will still be prefixed, so we need
        // to check if the un-prefixed events are usable, and if not remove them from the map.
        if !js::has(window, "AnimationEvent") {
            VENDOR_PREFIXES.with(|vendor_prefixes| {
                js::delete(dict::get(vendor_prefixes, "animationcancel"), "animation");
                js::delete(dict::get(vendor_prefixes, "animationend"), "animation");
                js::delete(dict::get(vendor_prefixes, "animationiteration"), "animation");
                js::delete(dict::get(vendor_prefixes, "animationstart"), "animation");
            });
        }

        // Same as above
        if !js::has(window, "TransitionEvent") {
            VENDOR_PREFIXES.with(|vendor_prefixes| {
                js::delete(dict::get(vendor_prefixes, "transitionend"), "transition");
            });
        }
    }
}

js::export_default! {
    /// Attempts to determine the correct vendor prefixed event name.
    ///
    /// @param {string} eventName
    /// @returns {string}
    pub fn get_vendor_prefixed_event_name(event_name: &str) -> String {
        if let Some(prefixed_event_name) =
            PREFIXED_EVENT_NAMES.with(|prefixed_event_names| dict::get(prefixed_event_names, event_name).cloned())
        {
            return prefixed_event_name;
        }
        let Some(prefix_map) = VENDOR_PREFIXES.with(|vendor_prefixes| dict::get(vendor_prefixes, event_name).copied()) else {
            return event_name.to_owned();
        };

        for style_prop in js::keys_in(js::unknown(prefix_map)) {
            if js::unknown(prefix_map).has_own_property(&style_prop) && js::has(STYLE.get(), &style_prop) {
                let prefixed = dict::get(prefix_map, &style_prop).cloned().unwrap_or_default();
                PREFIXED_EVENT_NAMES.with(|prefixed_event_names| dict::set(prefixed_event_names, event_name, prefixed.clone()));
                return prefixed;
            }
        }

        event_name.to_owned()
    }
}
