// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;

use js::{Kind, Unknown};
// TODO: direct imports like some-package/src/* are bad. Fix me.
use react_reconciler::react_current_fiber::get_current_fiber_owner_name_in_dev_or_null;
use shared::check_string_coercion::check_attribute_string_coercion;
use shared::environment::__DEV__;
use shared::react_feature_flags::{DISABLE_INPUT_ATTRIBUTE_SYNCING, ENABLE_HYDRATION_CHANGE_EVENT};
use webapi::{Element, HTMLInputElement, console, element, html_input_element};

use crate::escape_selector_attribute_value_inside_double_quotes::escape_selector_attribute_value_inside_double_quotes;
use crate::input_value_tracking::{track, track_hydrated, update_value_if_changed};
use crate::react_dom_component_tree::get_fiber_current_props_from_node;
use crate::react_dom_event_replaying::queue_change_event;
use crate::to_string_value::{ToStringValue, get_to_string_value, to_string};

thread_local! {
    static DID_WARN_VALUE_DEFAULT_VALUE: Cell<bool> = const { Cell::new(false) };
    static DID_WARN_CHECKED_DEFAULT_CHECKED: Cell<bool> = const { Cell::new(false) };
}

/**
 * Implements an <input> host component that allows setting these optional
 * props: `checked`, `value`, `defaultChecked`, and `defaultValue`.
 *
 * If `checked` or `value` are not supplied (or null/undefined), user actions
 * that affect the checked state or value will trigger updates to the element.
 *
 * If they are supplied (and not null/undefined), the rendered element will not
 * trigger updates to the element. Instead, the props must change in order for
 * the rendered element to be updated.
 *
 * The rendered element will be initialized as unchecked (or `defaultChecked`)
 * with an empty value (or `defaultValue`).
 *
 * See http://www.w3.org/TR/2012/WD-html5-20121025/the-input-element.html
 */
pub fn validate_input_props(_element: &Element, props: &Unknown) {
    if __DEV__ {
        // Normally we check for undefined and null the same, but explicitly specifying both
        // properties, at all is probably worth warning for. We could move this either direction
        // and just make it ok to pass null or just check hasOwnProperty.
        if !js::strict_equals(js::get(props, "checked"), ())
            && !js::strict_equals(js::get(props, "defaultChecked"), ())
            && !DID_WARN_CHECKED_DEFAULT_CHECKED.get()
        {
            console::error((
                concat!(
                    "%s contains an input of type %s with both checked and defaultChecked props. ",
                    "Input elements must be either controlled or uncontrolled ",
                    "(specify either the checked prop, or the defaultChecked prop, but not ",
                    "both). Decide between using a controlled or uncontrolled input ",
                    "element and remove one of these props. More info: ",
                    "https://react.dev/link/controlled-components"
                ),
                get_current_fiber_owner_name_in_dev_or_null()
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| "A component".to_owned()),
                js::get(props, "type"),
            ));
            DID_WARN_CHECKED_DEFAULT_CHECKED.set(true);
        }
        if !js::strict_equals(js::get(props, "value"), ())
            && !js::strict_equals(js::get(props, "defaultValue"), ())
            && !DID_WARN_VALUE_DEFAULT_VALUE.get()
        {
            console::error((
                concat!(
                    "%s contains an input of type %s with both value and defaultValue props. ",
                    "Input elements must be either controlled or uncontrolled ",
                    "(specify either the value prop, or the defaultValue prop, but not ",
                    "both). Decide between using a controlled or uncontrolled input ",
                    "element and remove one of these props. More info: ",
                    "https://react.dev/link/controlled-components"
                ),
                get_current_fiber_owner_name_in_dev_or_null()
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| "A component".to_owned()),
                js::get(props, "type"),
            ));
            DID_WARN_VALUE_DEFAULT_VALUE.set(true);
        }
    }
}

pub fn update_input(
    element: &Element,
    value: Option<&'static Unknown>,
    default_value: Option<&'static Unknown>,
    last_default_value: Option<&'static Unknown>,
    checked: Option<&'static Unknown>,
    default_checked: Option<&'static Unknown>,
    r#type: Option<&'static Unknown>,
    name: Option<&'static Unknown>,
) {
    let node = html_input_element::unchecked_from(element);

    // Temporarily disconnect the input from any radio buttons.
    // Changing the type or name as the same time as changing the checked value
    // needs to be atomically applied. We can only ensure that by disconnecting
    // the name while do the mutations and then reapply the name after that's done.
    node.set_name("");

    if r#type.is_some()
        && !matches!(
            js::classify(r#type),
            Kind::Function(_) | Kind::Symbol(_) | Kind::Bool(_)
        )
    {
        if __DEV__ {
            check_attribute_string_coercion(r#type, "type");
        }
        node.set_type(r#type);
    } else {
        node.remove_attribute("type");
    }

    if value.is_some() {
        if js::strict_equals(r#type, "number") {
            if
            // "" == 0, so a cleared field wouldn't otherwise be restored to 0.
            (js::strict_equals(value, 0) && node.value().is_empty())
                // We explicitly want to coerce to number here if possible, so that
                // other spellings of the same number (e.g. "0.0" mid-edit) aren't
                // clobbered while the user types.
                || !js::loose_equals(node.value(), value)
            {
                node.set_value(&to_string(get_to_string_value(value)));
            }
        } else if node.value() != to_string(get_to_string_value(value)) {
            node.set_value(&to_string(get_to_string_value(value)));
        }
    } else if js::strict_equals(r#type, "submit") || js::strict_equals(r#type, "reset") {
        // Submit/reset inputs need the attribute removed completely to avoid
        // blank-text buttons.
        node.remove_attribute("value");
    }

    if DISABLE_INPUT_ATTRIBUTE_SYNCING {
        // When not syncing the value attribute, React only assigns a new value
        // whenever the defaultValue React prop has changed. When not present,
        // React does nothing
        if default_value.is_some() {
            set_default_value(node, get_to_string_value(default_value));
        } else if last_default_value.is_some() {
            node.remove_attribute("value");
        }
    } else {
        // When syncing the value attribute, the value comes from a cascade of
        // properties:
        //  1. The value React property
        //  2. The defaultValue React property
        //  3. Otherwise there should be no change
        if value.is_some() {
            if js::strict_equals(r#type, "number")
                // We explicitly want to coerce to number here if possible.
                && js::loose_equals(node.value(), value)
            {
                // node.value may be a different spelling of the same number (e.g.
                // "0.0" for 0). Mirror what's displayed, like the value setter does.
                // Not redundant with the assignment above: browsers sanitize invalid
                // assigned values to "", in which case we sync the React value below.
                set_default_value(
                    node,
                    get_to_string_value(Some(js::unknown_of(node.value()))),
                );
            } else {
                set_default_value(node, get_to_string_value(value));
            }
        } else if default_value.is_some() {
            set_default_value(node, get_to_string_value(default_value));
        } else if last_default_value.is_some() {
            node.remove_attribute("value");
        }
    }

    if DISABLE_INPUT_ATTRIBUTE_SYNCING {
        // When not syncing the checked attribute, the attribute is directly
        // controllable from the defaultValue React property. It needs to be
        // updated as new props come in.
        if default_checked.is_none() {
            node.remove_attribute("checked");
        } else {
            node.set_default_checked(js::truthy(default_checked));
        }
    } else {
        // When syncing the checked attribute, it only changes when it needs
        // to be removed, such as transitioning from a checkbox into a text input
        if checked.is_none() && default_checked.is_some() {
            node.set_default_checked(js::truthy(default_checked));
        }
    }

    if checked.is_some() {
        // Important to set this even if it's not a change in order to update input
        // value tracking with radio buttons
        // TODO: Should really update input value tracking for the whole radio
        // button group in an effect or something (similar to #27024)
        node.set_checked(
            js::truthy(checked)
                && !matches!(js::classify(checked), Kind::Function(_) | Kind::Symbol(_)),
        );
    }

    if name.is_some()
        && !matches!(
            js::classify(name),
            Kind::Function(_) | Kind::Symbol(_) | Kind::Bool(_)
        )
    {
        if __DEV__ {
            check_attribute_string_coercion(name, "name");
        }
        node.set_name(to_string(get_to_string_value(name)));
    } else {
        node.remove_attribute("name");
    }
}

pub fn init_input(
    element: &Element,
    value: Option<&'static Unknown>,
    default_value: Option<&'static Unknown>,
    checked: Option<&'static Unknown>,
    default_checked: Option<&'static Unknown>,
    r#type: Option<&'static Unknown>,
    name: Option<&'static Unknown>,
    is_hydrating: bool,
) {
    let node = html_input_element::unchecked_from(element);

    if r#type.is_some()
        && !matches!(
            js::classify(r#type),
            Kind::Function(_) | Kind::Symbol(_) | Kind::Bool(_)
        )
    {
        if __DEV__ {
            check_attribute_string_coercion(r#type, "type");
        }
        node.set_type(r#type);
    }

    if value.is_some() || default_value.is_some() {
        let is_button = js::strict_equals(r#type, "submit") || js::strict_equals(r#type, "reset");

        // Avoid setting value attribute on submit/reset inputs as it overrides the
        // default value provided by the browser. See: #12872
        if is_button && value.is_none() {
            // We track the value just in case it changes type later on.
            track(html_input_element::unchecked_from(element));
            return;
        }

        let default_value_str = if default_value.is_some() {
            to_string(get_to_string_value(default_value))
        } else {
            String::new()
        };
        let initial_value = if value.is_some() {
            to_string(get_to_string_value(value))
        } else {
            default_value_str.clone()
        };

        // Do not assign value if it is already set. This prevents user text input
        // from being lost during SSR hydration.
        if !is_hydrating || ENABLE_HYDRATION_CHANGE_EVENT.with(|&on| on) {
            if DISABLE_INPUT_ATTRIBUTE_SYNCING {
                // When not syncing the value attribute, the value property points
                // directly to the React prop. Only assign it if it exists.
                if value.is_some() {
                    // Always assign on buttons so that it is possible to assign an
                    // empty string to clear button text.
                    //
                    // Otherwise, do not re-assign the value property if is empty. This
                    // potentially avoids a DOM write and prevents Firefox (~60.0.1) from
                    // prematurely marking required inputs as invalid. Equality is compared
                    // to the current value in case the browser provided value is not an
                    // empty string.
                    if is_button || to_string(get_to_string_value(value)) != node.value() {
                        node.set_value(&to_string(get_to_string_value(value)));
                    }
                }
            } else {
                // When syncing the value attribute, the value property should use
                // the wrapperState._initialValue property. This uses:
                //
                //   1. The value React property when present
                //   2. The defaultValue React property when present
                //   3. An empty string
                if initial_value != node.value() {
                    node.set_value(&initial_value);
                }
            }
        }

        if DISABLE_INPUT_ATTRIBUTE_SYNCING {
            // When not syncing the value attribute, assign the value attribute
            // directly from the defaultValue React property (when present)
            if default_value.is_some() {
                node.set_default_value(&default_value_str);
            }
        } else {
            // Otherwise, the value attribute is synchronized to the property,
            // so we assign defaultValue to the same thing as the value property
            // assignment step above.
            node.set_default_value(&initial_value);
        }
    }

    // Normally, we'd just do `node.checked = node.checked` upon initial mount, less this bug
    // this is needed to work around a chrome bug where setting defaultChecked
    // will sometimes influence the value of checked (even after detachment).
    // Reference: https://bugs.chromium.org/p/chromium/issues/detail?id=608416
    // We need to temporarily unset name to avoid disrupting radio button groups.

    let checked_or_default = checked.or(default_checked);
    // TODO: This 'function' or 'symbol' check isn't replicated in other places
    // so this semantic is inconsistent.
    let initial_checked = !matches!(
        js::classify(checked_or_default),
        Kind::Function(_) | Kind::Symbol(_)
    ) && js::truthy(checked_or_default);

    if is_hydrating && !ENABLE_HYDRATION_CHANGE_EVENT.with(|&on| on) {
        // Detach .checked from .defaultChecked but leave user input alone
        node.set_checked(node.checked());
    } else {
        node.set_checked(initial_checked);
    }

    if DISABLE_INPUT_ATTRIBUTE_SYNCING {
        // Only assign the checked attribute if it is defined. This saves
        // a DOM write when controlling the checked attribute isn't needed
        // (text inputs, submit/reset)
        if default_checked.is_some() {
            node.set_default_checked(!node.default_checked());
            node.set_default_checked(js::truthy(default_checked));
        }
    } else {
        // When syncing the checked attribute, both the checked property and
        // attribute are assigned at the same time using defaultChecked. This uses:
        //
        //   1. The checked React property when present
        //   2. The defaultChecked React property when present
        //   3. Otherwise, false
        node.set_default_checked(!node.default_checked());
        node.set_default_checked(initial_checked);
    }

    // Name needs to be set at the end so that it applies atomically to connected radio buttons.
    if name.is_some()
        && !matches!(
            js::classify(name),
            Kind::Function(_) | Kind::Symbol(_) | Kind::Bool(_)
        )
    {
        if __DEV__ {
            check_attribute_string_coercion(name, "name");
        }
        node.set_name(name);
    }
    track(html_input_element::unchecked_from(element));
}

pub fn hydrate_input(
    element: &'static Element,
    value: Option<&'static Unknown>,
    default_value: Option<&'static Unknown>,
    checked: Option<&'static Unknown>,
    default_checked: Option<&'static Unknown>,
) {
    let node = html_input_element::unchecked_from(element);

    let default_value_str = if default_value.is_some() {
        to_string(get_to_string_value(default_value))
    } else {
        String::new()
    };
    let initial_value = if value.is_some() {
        to_string(get_to_string_value(value))
    } else {
        default_value_str
    };

    let checked_or_default = checked.or(default_checked);
    // TODO: This 'function' or 'symbol' check isn't replicated in other places
    // so this semantic is inconsistent.
    let initial_checked = !matches!(
        js::classify(checked_or_default),
        Kind::Function(_) | Kind::Symbol(_)
    ) && js::truthy(checked_or_default);

    // Detach .checked from .defaultChecked but leave user input alone
    node.set_checked(node.checked());

    let changed = track_hydrated(node, &initial_value, initial_checked);
    if changed {
        // If the current value is different, that suggests that the user
        // changed it before hydration. Queue a replay of the change event.
        // For radio buttons the change event only fires on the selected one.
        if node.r#type() != "radio" || node.checked() {
            queue_change_event(node);
        }
    }
}

pub fn restore_controlled_input_state(element: &Element, props: &Unknown) {
    let root_node = html_input_element::unchecked_from(element);
    update_input(
        root_node,
        js::get(props, "value"),
        js::get(props, "defaultValue"),
        js::get(props, "defaultValue"),
        js::get(props, "checked"),
        js::get(props, "defaultChecked"),
        js::get(props, "type"),
        js::get(props, "name"),
    );
    let name = js::get(props, "name");
    if js::strict_equals(js::get(props, "type"), "radio") && name.is_some() {
        let mut query_root: &Element = root_node;

        while let Some(parent_node) = query_root.parent_node() {
            query_root = element::unchecked_from(parent_node);
        }

        // If `rootNode.form` was non-null, then we could try `form.elements`,
        // but that sometimes behaves strangely in IE8. We could also try using
        // `form.getElementsByName`, but that will only return direct children
        // and won't include inputs that use the HTML5 `form=` attribute. Since
        // the input might not even be in a form. It might not even be in the
        // document. Let's just use the local `querySelectorAll` to ensure we don't
        // miss anything.
        if __DEV__ {
            check_attribute_string_coercion(name, "name");
        }
        let group = query_root.query_selector_all(
            &("input[name=\"".to_owned()
                + &escape_selector_attribute_value_inside_double_quotes(
                    &js::concat_text(name).unwrap(),
                )
                + "\"][type=\"radio\"]"),
        );

        for i in 0..group.length() as usize {
            let other_node = html_input_element::unchecked_from(&group[i]);
            if other_node == root_node || other_node.form() != root_node.form() {
                continue;
            }
            // This will throw if radio buttons rendered by different copies of React
            // and the same name are rendered into the same form (same as #1939).
            // That's probably okay; we don't support it just as we don't support
            // mixing React radio buttons with non-React ones.
            let other_props = get_fiber_current_props_from_node(js::unknown(other_node));

            let Some(other_props) = other_props else {
                panic!(concat!(
                    "ReactDOMInput: Mixing React and non-React radio inputs with the ",
                    "same `name` is not supported."
                ));
            };

            // If this is a controlled radio button group, forcing the input that
            // was previously checked to update will cause it to be come re-checked
            // as appropriate.
            update_input(
                other_node,
                js::get(other_props, "value"),
                js::get(other_props, "defaultValue"),
                js::get(other_props, "defaultValue"),
                js::get(other_props, "checked"),
                js::get(other_props, "defaultChecked"),
                js::get(other_props, "type"),
                js::get(other_props, "name"),
            );
        }

        // If any updateInput() call set .checked to true, an input in this group
        // (often, `rootNode` itself) may have become unchecked
        for i in 0..group.length() as usize {
            let other_node = html_input_element::unchecked_from(&group[i]);
            if !js::strict_equals(other_node.form(), root_node.form()) {
                continue;
            }
            update_value_if_changed(Some(other_node));
        }
    }
}

fn set_default_value(node: &HTMLInputElement, value: ToStringValue) {
    if node.default_value() != to_string(value) {
        node.set_default_value(&to_string(value));
    }
}
