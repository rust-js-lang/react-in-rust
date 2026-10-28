// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{Dict, RegExp, dict, reg_exp};
use shared::environment::__DEV__;
use shared::has_own_property::HAS_OWN_PROPERTY;
use webapi::console;

const ATTRIBUTE_NAME_START_CHAR: &str = ":A-Z_a-z\\u00C0-\\u00D6\\u00D8-\\u00F6\\u00F8-\\u02FF\\u0370-\\u037D\\u037F-\\u1FFF\\u200C-\\u200D\\u2070-\\u218F\\u2C00-\\u2FEF\\u3001-\\uD7FF\\uF900-\\uFDCF\\uFDF0-\\uFFFD";

thread_local! {
    pub static ATTRIBUTE_NAME_CHAR: String =
        ATTRIBUTE_NAME_START_CHAR.to_owned() + "\\-.0-9\\u00B7\\u0300-\\u036F\\u203F-\\u2040";

    #[cfg_attr(rust_js, rust_js::name = "VALID_ATTRIBUTE_NAME_REGEX")]
    static VALID_ATTRIBUTE_NAME_REGEX: &'static RegExp = ATTRIBUTE_NAME_CHAR.with(|attribute_name_char| {
        reg_exp::new(
            &("^[".to_owned() + ATTRIBUTE_NAME_START_CHAR + "][" + attribute_name_char + "]*$"),
            "",
        )
    });

    static ILLEGAL_ATTRIBUTE_NAME_CACHE: &'static Dict<bool> = dict::new();
    static VALIDATED_ATTRIBUTE_NAME_CACHE: &'static Dict<bool> = dict::new();
}

js::export_default! {
    pub fn is_attribute_name_safe(attribute_name: &str) -> bool {
        let has_own_property = HAS_OWN_PROPERTY.with(|has_own_property| *has_own_property);
        if VALIDATED_ATTRIBUTE_NAME_CACHE.with(|cache| has_own_property.call(js::unknown(*cache), attribute_name)) {
            return true;
        }
        if ILLEGAL_ATTRIBUTE_NAME_CACHE.with(|cache| has_own_property.call(js::unknown(*cache), attribute_name)) {
            return false;
        }
        if VALID_ATTRIBUTE_NAME_REGEX.with(|regex| regex.test(attribute_name)) {
            VALIDATED_ATTRIBUTE_NAME_CACHE.with(|cache| dict::set(cache, attribute_name, true));
            return true;
        }
        ILLEGAL_ATTRIBUTE_NAME_CACHE.with(|cache| dict::set(cache, attribute_name, true));
        if __DEV__ {
            console::error(("Invalid attribute name: `%s`", attribute_name));
        }
        false
    }
}
