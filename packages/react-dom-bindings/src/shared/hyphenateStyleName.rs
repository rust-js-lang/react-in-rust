// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{RegExp, reg_exp};

thread_local! {
    static UPPERCASE_PATTERN: &'static RegExp = reg_exp::new("([A-Z])", "g");
    static MS_PATTERN: &'static RegExp = reg_exp::new("^ms-", "");
}

js::export_default! {
    /// Hyphenates a camelcased CSS property name, for example:
    ///
    ///   > hyphenateStyleName('backgroundColor')
    ///   < "background-color"
    ///   > hyphenateStyleName('MozTransition')
    ///   < "-moz-transition"
    ///   > hyphenateStyleName('msTransition')
    ///   < "-ms-transition"
    ///
    /// As Modernizr suggests (http://modernizr.com/docs/#prefixed), an `ms` prefix
    /// is converted to `-ms-`.
    pub fn hyphenate_style_name(name: &str) -> String {
        UPPERCASE_PATTERN.with(|uppercase_pattern| {
            MS_PATTERN.with(|ms_pattern| {
                reg_exp::replace(
                    &reg_exp::replace(name, uppercase_pattern, "-$1").to_lowercase(),
                    ms_pattern,
                    "-ms-",
                )
            })
        })
    }
}
