// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{Dict, RegExp, Unknown, dict, reg_exp};
use shared::environment::__DEV__;
use shared::has_own_property::HAS_OWN_PROPERTY;
use webapi::console;

use crate::is_attribute_name_safe::ATTRIBUTE_NAME_CHAR;
use crate::valid_aria_properties::ARIA_PROPERTIES as VALID_ARIA_PROPERTIES;

thread_local! {
    static WARNED_PROPERTIES: &'static Dict<bool> = dict::new();
    #[cfg_attr(rust_js, rust_js::name = "rARIA")]
    static R_ARIA: &'static RegExp = ATTRIBUTE_NAME_CHAR.with(|attribute_name_char| {
        reg_exp::new(&("^(aria)-[".to_owned() + attribute_name_char + "]*$"), "")
    });
    #[cfg_attr(rust_js, rust_js::name = "rARIACamel")]
    static R_ARIA_CAMEL: &'static RegExp = ATTRIBUTE_NAME_CHAR.with(|attribute_name_char| {
        reg_exp::new(&("^(aria)[A-Z][".to_owned() + attribute_name_char + "]*$"), "")
    });
}

fn validate_property(_tag_name: &str, name: &str) -> bool {
    if __DEV__ {
        let has_own_property = HAS_OWN_PROPERTY.with(|has_own_property| *has_own_property);
        if WARNED_PROPERTIES.with(|warned_properties| {
            has_own_property.call(js::unknown(*warned_properties), name)
                && dict::get(warned_properties, name).is_some_and(|&warned| warned)
        }) {
            return true;
        }

        if R_ARIA_CAMEL.with(|r_aria_camel| r_aria_camel.test(name)) {
            let aria_name = "aria-".to_owned() + &js::string::slice(name, 4.0, None).to_lowercase();
            let correct_name = VALID_ARIA_PROPERTIES.with(|valid_aria_properties| {
                js::unknown(*valid_aria_properties)
                    .has_own_property(&aria_name)
                    .then_some(aria_name)
            });

            // If this is an aria-* attribute, but is not listed in the known DOM
            // DOM properties, then it is an invalid aria-* attribute.
            let Some(correct_name) = correct_name else {
                console::error((
                    "Invalid ARIA attribute `%s`. ARIA attributes follow the pattern aria-* and must be lowercase.",
                    name,
                ));
                WARNED_PROPERTIES
                    .with(|warned_properties| dict::set(warned_properties, name, true));
                return true;
            };
            // aria-* attributes should be lowercase; suggest the lowercase version.
            if name != correct_name {
                console::error((
                    "Invalid ARIA attribute `%s`. Did you mean `%s`?",
                    name,
                    correct_name,
                ));
                WARNED_PROPERTIES
                    .with(|warned_properties| dict::set(warned_properties, name, true));
                return true;
            }
        }

        if R_ARIA.with(|r_aria| r_aria.test(name)) {
            let lower_cased_name = name.to_lowercase();
            let standard_name = VALID_ARIA_PROPERTIES.with(|valid_aria_properties| {
                js::unknown(*valid_aria_properties)
                    .has_own_property(&lower_cased_name)
                    .then_some(lower_cased_name)
            });

            // If this is an aria-* attribute, but is not listed in the known DOM
            // DOM properties, then it is an invalid aria-* attribute.
            let Some(standard_name) = standard_name else {
                WARNED_PROPERTIES
                    .with(|warned_properties| dict::set(warned_properties, name, true));
                return false;
            };
            // aria-* attributes should be lowercase; suggest the lowercase version.
            if name != standard_name {
                console::error((
                    "Unknown ARIA attribute `%s`. Did you mean `%s`?",
                    name,
                    standard_name,
                ));
                WARNED_PROPERTIES
                    .with(|warned_properties| dict::set(warned_properties, name, true));
                return true;
            }
        }
    }

    true
}

pub fn validate_properties(r#type: &str, props: &Unknown) {
    if __DEV__ {
        let mut invalid_props = Vec::new();

        for key in js::keys_in(props) {
            let is_valid = validate_property(r#type, &key);
            if !is_valid {
                invalid_props.push(key);
            }
        }

        let unknown_prop_string = invalid_props
            .iter()
            .map(|prop| "`".to_owned() + prop + "`")
            .collect::<Vec<_>>()
            .join(", ");

        if invalid_props.len() == 1 {
            console::error((
                concat!(
                    "Invalid aria prop %s on <%s> tag. ",
                    "For details, see https://react.dev/link/invalid-aria-props"
                ),
                unknown_prop_string,
                r#type,
            ));
        } else if invalid_props.len() > 1 {
            console::error((
                concat!(
                    "Invalid aria props %s on <%s> tag. ",
                    "For details, see https://react.dev/link/invalid-aria-props"
                ),
                unknown_prop_string,
                r#type,
            ));
        }
    }
}
