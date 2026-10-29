// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;

use js::{Dict, Unknown, dict};
// TODO: direct imports like some-package/src/* are bad. Fix me.
use react_reconciler::react_current_fiber::get_current_fiber_owner_name_in_dev_or_null;
use shared::environment::__DEV__;
use shared::is_array::is_array;
use webapi::{Element, HTMLSelectElement, console, html_select_element};

use crate::react_dom_event_replaying::queue_change_event;
use crate::to_string_value::{get_to_string_value, to_string};

thread_local! {
    static DID_WARN_VALUE_DEFAULT_VALUE: Cell<bool> = const { Cell::new(false) };
}

fn get_declaration_error_addendum() -> String {
    if let Some(owner_name) =
        get_current_fiber_owner_name_in_dev_or_null().filter(|name| !name.is_empty())
    {
        return "\n\nCheck the render method of `".to_owned() + &owner_name + "`.";
    }
    String::new()
}

#[cfg_attr(rust_js, rust_js::name = "valuePropNames")]
static VALUE_PROP_NAMES: [&str; 2] = ["value", "defaultValue"];

/**
 * Validation function for `value` and `defaultValue`.
 */
fn check_select_prop_types(props: &Unknown) {
    if __DEV__ {
        for &prop_name in &VALUE_PROP_NAMES {
            if js::get(props, prop_name).is_none() {
                continue;
            }
            let prop_name_is_array = is_array(js::get(props, prop_name));
            if js::truthy(js::get(props, "multiple")) && !prop_name_is_array {
                console::error((
                    concat!(
                        "The `%s` prop supplied to <select> must be an array if ",
                        "`multiple` is true.%s"
                    ),
                    prop_name,
                    get_declaration_error_addendum(),
                ));
            } else if !js::truthy(js::get(props, "multiple")) && prop_name_is_array {
                console::error((
                    concat!(
                        "The `%s` prop supplied to <select> must be a scalar ",
                        "value if `multiple` is false.%s"
                    ),
                    prop_name,
                    get_declaration_error_addendum(),
                ));
            }
        }
    }
}

fn update_options(
    node: &HTMLSelectElement,
    multiple: bool,
    prop_value: &'static Unknown,
    set_default_selected: bool,
) {
    let options = node.options();

    if multiple {
        let selected_values: &[&str] = unsafe { js::cast(Some(prop_value)) };
        let selected_value: &Dict<bool> = dict::new();
        for selected in selected_values {
            // Prefix to avoid chaos with special keys.
            dict::set(selected_value, &("$".to_owned() + selected), true);
        }
        for i in 0..options.length() as usize {
            let selected = js::unknown(selected_value)
                .has_own_property(&("$".to_owned() + &options[i].value()));
            if options[i].selected() != selected {
                options[i].set_selected(selected);
            }
            if selected && set_default_selected {
                options[i].set_default_selected(true);
            }
        }
    } else {
        // Do not set `select.value` as exact behavior isn't consistent across all
        // browsers for all cases.
        let selected_value = to_string(get_to_string_value(Some(prop_value)));
        let mut default_selected = js::null();
        for i in 0..options.length() as usize {
            if options[i].value() == selected_value {
                options[i].set_selected(true);
                if set_default_selected {
                    options[i].set_default_selected(true);
                }
                return;
            }
            if default_selected.is_none() && !options[i].disabled() {
                default_selected = Some(&options[i]);
            }
        }
        if let Some(default_selected) = default_selected {
            default_selected.set_selected(true);
        }
    }
}

/**
 * Implements a <select> host component that allows optionally setting the
 * props `value` and `defaultValue`. If `multiple` is false, the prop must be a
 * stringable. If `multiple` is true, the prop must be an array of stringables.
 *
 * If `value` is not supplied (or null/undefined), user actions that change the
 * selected option will trigger updates to the rendered options.
 *
 * If it is supplied (and not null/undefined), the rendered options will not
 * update in response to user actions. Instead, the `value` prop must change in
 * order for the rendered options to update.
 *
 * If `defaultValue` is provided, any options with the supplied values will be
 * selected.
 */
pub fn validate_select_props(_element: &Element, props: &Unknown) {
    if __DEV__ {
        check_select_prop_types(props);
        if !js::strict_equals(js::get(props, "value"), ())
            && !js::strict_equals(js::get(props, "defaultValue"), ())
            && !DID_WARN_VALUE_DEFAULT_VALUE.get()
        {
            console::error((concat!(
                "Select elements must be either controlled or uncontrolled ",
                "(specify either the value prop, or the defaultValue prop, but not ",
                "both). Decide between using a controlled or uncontrolled select ",
                "element and remove one of these props. More info: ",
                "https://react.dev/link/controlled-components"
            ),));
            DID_WARN_VALUE_DEFAULT_VALUE.set(true);
        }
    }
}

pub fn init_select(
    element: &Element,
    value: Option<&'static Unknown>,
    default_value: Option<&'static Unknown>,
    multiple: Option<&'static Unknown>,
) {
    let node = html_select_element::unchecked_from(element);
    node.set_multiple(js::truthy(multiple));
    if let Some(value) = value {
        update_options(node, js::truthy(multiple), value, false);
    } else if let Some(default_value) = default_value {
        update_options(node, js::truthy(multiple), default_value, true);
    }
}

pub fn hydrate_select(
    element: &'static Element,
    value: Option<&'static Unknown>,
    default_value: Option<&'static Unknown>,
    multiple: Option<&'static Unknown>,
) {
    let node = html_select_element::unchecked_from(element);
    let options = node.options();

    let prop_value = if value.is_some() {
        value
    } else {
        default_value
    };

    let mut changed = false;

    if js::truthy(multiple) {
        let selected_values: Option<&[&str]> = unsafe { js::cast(prop_value) };
        let selected_value: &Dict<bool> = dict::new();
        if let Some(selected_values) = selected_values {
            for selected in selected_values {
                // Prefix to avoid chaos with special keys.
                dict::set(selected_value, &("$".to_owned() + selected), true);
            }
        }
        for i in 0..options.length() as usize {
            let expected_selected = js::unknown(selected_value)
                .has_own_property(&("$".to_owned() + &options[i].value()));
            if options[i].selected() != expected_selected {
                changed = true;
                break;
            }
        }
    } else {
        let mut selected_value = if prop_value.is_none() {
            None
        } else {
            Some(to_string(get_to_string_value(prop_value)))
        };
        for i in 0..options.length() as usize {
            if selected_value.is_none() && !options[i].disabled() {
                // We expect the first non-disabled option to be selected if the selected is null.
                selected_value = Some(options[i].value());
            }
            let expected_selected = Some(options[i].value()) == selected_value;
            if options[i].selected() != expected_selected {
                changed = true;
                break;
            }
        }
    }
    if changed {
        // If the current selection is different than our initial that suggests that the user
        // changed it before hydration. Queue a replay of the change event.
        queue_change_event(node);
    }
}

pub fn update_select(
    element: &Element,
    value: Option<&'static Unknown>,
    default_value: Option<&'static Unknown>,
    multiple: Option<&'static Unknown>,
    was_multiple: Option<&'static Unknown>,
) {
    let node = html_select_element::unchecked_from(element);

    if let Some(value) = value {
        update_options(node, js::truthy(multiple), value, false);
    } else if js::truthy(was_multiple) != js::truthy(multiple) {
        // For simplicity, reapply `defaultValue` if `multiple` is toggled.
        if let Some(default_value) = default_value {
            update_options(node, js::truthy(multiple), default_value, true);
        } else {
            // Revert the select back to its default unselected state.
            update_options(
                node,
                js::truthy(multiple),
                if js::truthy(multiple) {
                    js::unknown_of(Vec::<&str>::new())
                } else {
                    js::unknown("")
                },
                false,
            );
        }
    }
}

pub fn restore_controlled_select_state(element: &Element, props: &Unknown) {
    let node = html_select_element::unchecked_from(element);
    let value = js::get(props, "value");

    if let Some(value) = value {
        update_options(node, js::truthy(js::get(props, "multiple")), value, false);
    }
}
