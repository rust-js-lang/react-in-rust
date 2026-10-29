// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;

use js::Unknown;
use react_reconciler::react_current_fiber::get_current_fiber_owner_name_in_dev_or_null;
use shared::environment::__DEV__;
use shared::is_array::is_array;
use shared::react_feature_flags::DISABLE_TEXTAREA_CHILDREN;
use webapi::{Element, console, html_input_element, html_text_area_element};

use crate::input_value_tracking::{track, track_hydrated};
use crate::react_dom_event_replaying::queue_change_event;
use crate::to_string_value::{get_to_string_value, to_string};

thread_local! {
    static DID_WARN_VAL_DEFAULT_VAL: Cell<bool> = const { Cell::new(false) };
}

/**
 * Implements a <textarea> host component that allows setting `value`, and
 * `defaultValue`. This differs from the traditional DOM API because value is
 * usually set as PCDATA children.
 *
 * If `value` is not supplied (or null/undefined), user actions that affect the
 * value will trigger updates to the element.
 *
 * If `value` is supplied (and not null/undefined), the rendered element will
 * not trigger updates to the element. Instead, the `value` prop must change in
 * order for the rendered element to be updated.
 *
 * The rendered element will be initialized with an empty value, the prop
 * `defaultValue` if specified, or the children content (deprecated).
 */
pub fn validate_textarea_props(_element: &Element, props: &Unknown) {
    if __DEV__ {
        if !js::strict_equals(js::get(props, "value"), ())
            && !js::strict_equals(js::get(props, "defaultValue"), ())
            && !DID_WARN_VAL_DEFAULT_VAL.get()
        {
            console::error((
                concat!(
                    "%s contains a textarea with both value and defaultValue props. ",
                    "Textarea elements must be either controlled or uncontrolled ",
                    "(specify either the value prop, or the defaultValue prop, but not ",
                    "both). Decide between using a controlled or uncontrolled textarea ",
                    "and remove one of these props. More info: ",
                    "https://react.dev/link/controlled-components"
                ),
                get_current_fiber_owner_name_in_dev_or_null()
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| "A component".to_owned()),
            ));
            DID_WARN_VAL_DEFAULT_VAL.set(true);
        }
        if js::get(props, "children").is_some() && js::get(props, "value").is_none() {
            console::error((concat!(
                "Use the `defaultValue` or `value` props instead of setting ",
                "children on <textarea>."
            ),));
        }
    }
}

pub fn update_textarea(
    element: &Element,
    value: Option<&'static Unknown>,
    default_value: Option<&'static Unknown>,
) {
    let node = html_text_area_element::unchecked_from(element);
    if value.is_some() {
        // Cast `value` to a string to ensure the value is set correctly. While
        // browsers typically do this as necessary, jsdom doesn't.
        let new_value = to_string(get_to_string_value(value));
        // To avoid side effects (such as losing text selection), only set value if changed
        if new_value != node.value() {
            node.set_value(&new_value);
        }
        // TOOO: This should respect disableInputAttributeSyncing flag.
        if default_value.is_none() {
            if node.default_value() != new_value {
                node.set_default_value(&new_value);
            }
            return;
        }
    }
    if default_value.is_some() {
        node.set_default_value(&to_string(get_to_string_value(default_value)));
    } else {
        node.set_default_value("");
    }
}

pub fn init_textarea(
    element: &Element,
    value: Option<&'static Unknown>,
    mut default_value: Option<&'static Unknown>,
    mut children: Option<&'static Unknown>,
) {
    let node = html_text_area_element::unchecked_from(element);

    let mut initial_value = value;

    // Only bother fetching default value if we're going to use it
    if initial_value.is_none() {
        if children.is_some() && !DISABLE_TEXTAREA_CHILDREN {
            if default_value.is_some() {
                panic!("If you supply `defaultValue` on a <textarea>, do not pass children.");
            }

            if is_array(children) {
                if unsafe { js::cast::<&[Option<&Unknown>]>(children) }.len() > 1 {
                    panic!("<textarea> can only have at most one child.");
                }

                children = unsafe { js::cast::<&[Option<&'static Unknown>]>(children) }
                    .first()
                    .copied()
                    .flatten();
            }

            default_value = children;
        }
        if default_value.is_none() {
            default_value = Some(js::unknown(""));
        }
        initial_value = default_value;
    }

    let string_value = get_to_string_value(initial_value);
    js::set(node, "defaultValue", string_value); // This will be toString:ed.

    // This is in postMount because we need access to the DOM node, which is not
    // available until after the component has mounted.
    let text_content = node.text_content();

    // Only set node.value if textContent is equal to the expected
    // initial value. In IE10/IE11 there is a bug where the placeholder attribute
    // will populate textContent as well.
    // https://developer.microsoft.com/microsoft-edge/platform/issues/101525/
    if js::strict_equals(&text_content, string_value)
        && let Some(text_content) = text_content
        && !text_content.is_empty()
    {
        node.set_value(&text_content);
    }

    track(html_input_element::unchecked_from(element));
}

pub fn hydrate_textarea(
    element: &'static Element,
    value: Option<&'static Unknown>,
    mut default_value: Option<&'static Unknown>,
) {
    let node = html_text_area_element::unchecked_from(element);
    let mut initial_value = value;
    if initial_value.is_none() {
        if default_value.is_none() {
            default_value = Some(js::unknown(""));
        }
        initial_value = default_value;
    }
    // Track the value that we last observed which is the hydrated value so
    // that any change event that fires will trigger onChange on the actual
    // current value.
    let string_value = to_string(get_to_string_value(initial_value));
    let changed = track_hydrated(
        html_input_element::unchecked_from(node),
        &string_value,
        false,
    );
    if changed {
        // If the current value is different, that suggests that the user
        // changed it before hydration. Queue a replay of the change event.
        queue_change_event(node);
    }
}

pub fn restore_controlled_textarea_state(element: &Element, props: &Unknown) {
    // DOM component is still mounted; update
    update_textarea(
        element,
        js::get(props, "value"),
        js::get(props, "defaultValue"),
    );
}
