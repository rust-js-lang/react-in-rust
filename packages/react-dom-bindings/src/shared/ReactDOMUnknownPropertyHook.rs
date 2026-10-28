// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::rc::Rc;

use js::{Dict, Kind, RegExp, Unknown, dict, reg_exp};
use shared::environment::__DEV__;
use shared::has_own_property::HAS_OWN_PROPERTY;
use webapi::console;

use crate::dom_event_names::DOMEventName;
use crate::is_attribute_name_safe::ATTRIBUTE_NAME_CHAR;
use crate::is_custom_element::is_custom_element;
use crate::possible_standard_names::POSSIBLE_STANDARD_NAMES;

/// The event system's registry, as the client gives it; the server has none.
pub struct EventRegistry {
    pub registration_name_dependencies: &'static Dict<Rc<Vec<DOMEventName>>>,
    pub possible_registration_names: &'static Dict<String>,
}

thread_local! {
    static WARNED_PROPERTIES: &'static Dict<bool> = dict::new();
    #[cfg_attr(rust_js, rust_js::name = "EVENT_NAME_REGEX")]
    static EVENT_NAME_REGEX: &'static RegExp = reg_exp::new("^on.", "");
    #[cfg_attr(rust_js, rust_js::name = "INVALID_EVENT_NAME_REGEX")]
    static INVALID_EVENT_NAME_REGEX: &'static RegExp = reg_exp::new("^on[^A-Z]", "");
    #[cfg_attr(rust_js, rust_js::name = "rARIA")]
    static R_ARIA: Option<&'static RegExp> = if __DEV__ {
        ATTRIBUTE_NAME_CHAR.with(|attribute_name_char| {
            Some(reg_exp::new(&("^(aria)-[".to_owned() + attribute_name_char + "]*$"), ""))
        })
    } else {
        js::null()
    };
    #[cfg_attr(rust_js, rust_js::name = "rARIACamel")]
    static R_ARIA_CAMEL: Option<&'static RegExp> = if __DEV__ {
        ATTRIBUTE_NAME_CHAR.with(|attribute_name_char| {
            Some(reg_exp::new(&("^(aria)[A-Z][".to_owned() + attribute_name_char + "]*$"), ""))
        })
    } else {
        js::null()
    };
}

fn validate_property(
    tag_name: &str,
    name: &str,
    value: Option<&Unknown>,
    event_registry: Option<&EventRegistry>,
) -> bool {
    if __DEV__ {
        let has_own_property = HAS_OWN_PROPERTY.with(|has_own_property| *has_own_property);
        if WARNED_PROPERTIES.with(|warned_properties| {
            has_own_property.call(js::unknown(*warned_properties), name)
                && dict::get(warned_properties, name).is_some_and(|&warned| warned)
        }) {
            return true;
        }

        let lower_cased_name = name.to_lowercase();
        if lower_cased_name == "onfocusin" || lower_cased_name == "onfocusout" {
            console::error((concat!(
                "React uses onFocus and onBlur instead of onFocusIn and onFocusOut. ",
                "All React events are normalized to bubble, so onFocusIn and onFocusOut ",
                "are not needed/supported by React."
            ),));
            WARNED_PROPERTIES.with(|warned_properties| dict::set(warned_properties, name, true));
            return true;
        }

        // Actions are special because unlike events they can have other value types.
        if matches!(js::classify(value), Kind::Function(_)) {
            if tag_name == "form" && name == "action" {
                return true;
            }
            if tag_name == "input" && name == "formAction" {
                return true;
            }
            if tag_name == "button" && name == "formAction" {
                return true;
            }
        }
        // We can't rely on the event system being injected on the server.
        if let Some(event_registry) = event_registry {
            let EventRegistry {
                registration_name_dependencies,
                possible_registration_names,
            } = event_registry;
            if js::unknown(*registration_name_dependencies).has_own_property(name) {
                return true;
            }
            let registration_name =
                if js::unknown(*possible_registration_names).has_own_property(&lower_cased_name) {
                    dict::get(possible_registration_names, &lower_cased_name)
                } else {
                    js::null()
                };
            if let Some(registration_name) = registration_name {
                console::error((
                    "Invalid event handler property `%s`. Did you mean `%s`?",
                    name,
                    registration_name,
                ));
                WARNED_PROPERTIES
                    .with(|warned_properties| dict::set(warned_properties, name, true));
                return true;
            }
            if EVENT_NAME_REGEX.with(|event_name_regex| event_name_regex.test(name)) {
                console::error((
                    "Unknown event handler property `%s`. It will be ignored.",
                    name,
                ));
                WARNED_PROPERTIES
                    .with(|warned_properties| dict::set(warned_properties, name, true));
                return true;
            }
        } else if EVENT_NAME_REGEX.with(|event_name_regex| event_name_regex.test(name)) {
            // If no event plugins have been injected, we are in a server environment.
            // So we can't tell if the event name is correct for sure, but we can filter
            // out known bad ones like `onclick`. We can't suggest a specific replacement though.
            if INVALID_EVENT_NAME_REGEX
                .with(|invalid_event_name_regex| invalid_event_name_regex.test(name))
            {
                console::error((
                    concat!(
                        "Invalid event handler property `%s`. ",
                        "React events use the camelCase naming convention, for example `onClick`."
                    ),
                    name,
                ));
            }
            WARNED_PROPERTIES.with(|warned_properties| dict::set(warned_properties, name, true));
            return true;
        }

        // Let the ARIA attribute hook validate ARIA attributes
        if R_ARIA.with(|r_aria| r_aria.unwrap().test(name))
            || R_ARIA_CAMEL.with(|r_aria_camel| r_aria_camel.unwrap().test(name))
        {
            return true;
        }

        if lower_cased_name == "innerhtml" {
            console::error((concat!(
                "Directly setting property `innerHTML` is not permitted. ",
                "For more information, lookup documentation on `dangerouslySetInnerHTML`."
            ),));
            WARNED_PROPERTIES.with(|warned_properties| dict::set(warned_properties, name, true));
            return true;
        }

        if lower_cased_name == "aria" {
            console::error((concat!(
                "The `aria` attribute is reserved for future use in React. ",
                "Pass individual `aria-` attributes instead."
            ),));
            WARNED_PROPERTIES.with(|warned_properties| dict::set(warned_properties, name, true));
            return true;
        }

        if lower_cased_name == "is"
            && value.is_some()
            && !matches!(js::classify(value), Kind::String(_))
        {
            console::error((
                concat!(
                    "Received a `%s` for a string attribute `is`. If this is expected, cast ",
                    "the value to a string."
                ),
                js::type_of(value),
            ));
            WARNED_PROPERTIES.with(|warned_properties| dict::set(warned_properties, name, true));
            return true;
        }

        if let Kind::Number(number) = js::classify(value)
            && js::is_nan(number)
        {
            console::error((
                concat!(
                    "Received NaN for the `%s` attribute. If this is expected, cast ",
                    "the value to a string."
                ),
                name,
            ));
            WARNED_PROPERTIES.with(|warned_properties| dict::set(warned_properties, name, true));
            return true;
        }

        // Known attributes should match the casing specified in the property config.
        if POSSIBLE_STANDARD_NAMES.with(|possible_standard_names| {
            js::unknown(*possible_standard_names).has_own_property(&lower_cased_name)
        }) {
            let standard_name = POSSIBLE_STANDARD_NAMES.with(|possible_standard_names| {
                dict::get(possible_standard_names, &lower_cased_name).copied()
            });
            if standard_name != Some(name) {
                console::error((
                    "Invalid DOM property `%s`. Did you mean `%s`?",
                    name,
                    standard_name,
                ));
                WARNED_PROPERTIES
                    .with(|warned_properties| dict::set(warned_properties, name, true));
                return true;
            }
        } else if name != lower_cased_name {
            // Unknown attributes should have lowercase casing since that's how they
            // will be cased anyway with server rendering.
            console::error((
                concat!(
                    "React does not recognize the `%s` prop on a DOM element. If you ",
                    "intentionally want it to appear in the DOM as a custom ",
                    "attribute, spell it as lowercase `%s` instead. ",
                    "If you accidentally passed it from a parent component, remove ",
                    "it from the DOM element."
                ),
                name,
                lower_cased_name,
            ));
            WARNED_PROPERTIES.with(|warned_properties| dict::set(warned_properties, name, true));
            return true;
        }

        // Now that we've validated casing, do not validate
        // data types for reserved props
        match name {
            "dangerouslySetInnerHTML"
            | "children"
            | "style"
            | "suppressContentEditableWarning"
            | "suppressHydrationWarning"
            // Reserved
            | "defaultValue"
            | "defaultChecked"
            | "innerHTML"
            | "ref" => {
                return true;
            }
            // Properties
            "innerText" | "textContent" => return true,
            _ => {}
        }

        match js::classify(value) {
            Kind::Bool(boolean) => match name {
                "autoFocus"
                | "checked"
                | "multiple"
                | "muted"
                | "selected"
                | "contentEditable"
                | "spellCheck"
                | "draggable"
                | "value"
                | "autoReverse"
                | "externalResourcesRequired"
                | "focusable"
                | "preserveAlpha"
                | "allowFullScreen"
                | "async"
                | "autoPlay"
                | "controls"
                | "credentialless"
                | "default"
                | "defer"
                | "disabled"
                | "disablePictureInPicture"
                | "disableRemotePlayback"
                | "formNoValidate"
                | "hidden"
                | "loop"
                | "noModule"
                | "noValidate"
                | "open"
                | "playsInline"
                | "readOnly"
                | "required"
                | "reversed"
                | "scoped"
                | "seamless"
                | "itemScope"
                | "capture"
                | "download"
                | "inert" => {
                    // Boolean properties can accept boolean values
                    return true;
                }
                // fallthrough
                _ => {
                    let prefix = js::string::slice(&name.to_lowercase(), 0.0, Some(5.0));
                    if prefix == "data-" || prefix == "aria-" {
                        return true;
                    }
                    if boolean {
                        console::error((
                            concat!(
                                "Received `%s` for a non-boolean attribute `%s`.\n\n",
                                "If you want to write it to the DOM, pass a string instead: ",
                                "%s=\"%s\" or %s={value.toString()}."
                            ),
                            boolean,
                            name,
                            name,
                            boolean,
                            name,
                        ));
                    } else {
                        console::error((
                            concat!(
                                "Received `%s` for a non-boolean attribute `%s`.\n\n",
                                "If you want to write it to the DOM, pass a string instead: ",
                                "%s=\"%s\" or %s={value.toString()}.\n\n",
                                "If you used to conditionally omit it with %s={condition && value}, ",
                                "pass %s={condition ? value : undefined} instead."
                            ),
                            boolean,
                            name,
                            name,
                            boolean,
                            name,
                            name,
                            name,
                        ));
                    }
                    WARNED_PROPERTIES
                        .with(|warned_properties| dict::set(warned_properties, name, true));
                    return true;
                }
            },
            Kind::Function(_) | Kind::Symbol(_) => {
                // Warn when a known attribute is a bad type
                WARNED_PROPERTIES
                    .with(|warned_properties| dict::set(warned_properties, name, true));
                return false;
            }
            // Warn when passing the strings 'false' or 'true' into a boolean prop
            Kind::String(value) if value == "false" || value == "true" => {
                match name {
                    "checked"
                    | "selected"
                    | "multiple"
                    | "muted"
                    | "allowFullScreen"
                    | "async"
                    | "autoPlay"
                    | "controls"
                    | "credentialless"
                    | "default"
                    | "defer"
                    | "disabled"
                    | "disablePictureInPicture"
                    | "disableRemotePlayback"
                    | "formNoValidate"
                    | "hidden"
                    | "loop"
                    | "noModule"
                    | "noValidate"
                    | "open"
                    | "playsInline"
                    | "readOnly"
                    | "required"
                    | "reversed"
                    | "scoped"
                    | "seamless"
                    | "itemScope"
                    | "inert" => {}
                    _ => {
                        return true;
                    }
                }
                console::error((
                    concat!(
                        "Received the string `%s` for the boolean attribute `%s`. ",
                        "%s ",
                        "Did you mean %s={%s}?"
                    ),
                    value,
                    name,
                    if value == "false" {
                        "The browser will interpret it as a truthy value."
                    } else {
                        "Although this works, it will not work as expected if you pass the string \"false\"."
                    },
                    name,
                    value,
                ));
                WARNED_PROPERTIES
                    .with(|warned_properties| dict::set(warned_properties, name, true));
                return true;
            }
            _ => {}
        }
        return true;
    }
    false
}

fn warn_unknown_properties(r#type: &str, props: &Unknown, event_registry: Option<&EventRegistry>) {
    if __DEV__ {
        let mut unknown_props = Vec::new();
        for key in js::keys_in(props) {
            let is_valid = validate_property(r#type, &key, js::get(props, &key), event_registry);
            if !is_valid {
                unknown_props.push(key);
            }
        }

        let unknown_prop_string = unknown_props
            .iter()
            .map(|prop| "`".to_owned() + prop + "`")
            .collect::<Vec<_>>()
            .join(", ");
        if unknown_props.len() == 1 {
            console::error((
                concat!(
                    "Invalid value for prop %s on <%s> tag. Either remove it from the element, ",
                    "or pass a string or number value to keep it in the DOM. ",
                    "For details, see https://react.dev/link/attribute-behavior "
                ),
                unknown_prop_string,
                r#type,
            ));
        } else if unknown_props.len() > 1 {
            console::error((
                concat!(
                    "Invalid values for props %s on <%s> tag. Either remove them from the element, ",
                    "or pass a string or number value to keep them in the DOM. ",
                    "For details, see https://react.dev/link/attribute-behavior "
                ),
                unknown_prop_string,
                r#type,
            ));
        }
    }
}

pub fn validate_properties(r#type: &str, props: &Unknown, event_registry: Option<&EventRegistry>) {
    if is_custom_element(r#type, props)
        || matches!(js::classify(js::get(props, "is")), Kind::String(_))
    {
        return;
    }
    warn_unknown_properties(r#type, props, event_registry);
}
