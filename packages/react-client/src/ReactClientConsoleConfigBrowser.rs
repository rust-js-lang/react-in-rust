// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{Kind, Unknown, function};
use webapi::console;

// Keep in sync with ReactServerConsoleConfig
#[cfg_attr(rust_js, rust_js::name = "badgeFormat")]
static BADGE_FORMAT: &str = "%c%s%c";
// Same badge styling as DevTools.
#[cfg_attr(rust_js, rust_js::name = "badgeStyle")]
static BADGE_STYLE: &str = concat!(
    // We use a fixed background if light-dark is not supported, otherwise
    // we use a transparent background.
    "background: #e6e6e6;",
    "background: light-dark(rgba(0,0,0,0.1), rgba(255,255,255,0.25));",
    "color: #000000;",
    "color: light-dark(#000000, #ffffff);",
    "border-radius: 2px",
);
#[cfg_attr(rust_js, rust_js::name = "resetStyle")]
static RESET_STYLE: &str = "";
#[cfg_attr(rust_js, rust_js::name = "pad")]
static PAD: &str = " ";

pub fn bind_to_console(
    method_name: &str,
    args: &[Option<&'static Unknown>],
    badge_name: &str,
) -> &'static dyn Fn() -> Option<&'static Unknown> {
    let Kind::Function(method) = js::classify(js::get(console, method_name)) else {
        panic!("console.{method_name} is no function");
    };
    let mut offset = 0;
    match method_name {
        "dir" | "dirxml" | "groupEnd" | "table" => {
            // These methods cannot be colorized because they don't take a formatting string.
            return function::bind(method, Some(&console), args);
        }
        "assert" => {
            // assert takes formatting options as the second argument.
            offset = 1;
        }
        _ => {}
    }

    let mut new_args = args.to_vec();
    if let Kind::String(format) = js::classify(new_args.get(offset).copied().flatten()) {
        new_args.splice(
            offset..offset + 1,
            [
                Some(js::unknown_of(BADGE_FORMAT.to_owned() + " " + format)),
                Some(js::unknown_of(BADGE_STYLE)),
                Some(js::unknown_of(PAD.to_owned() + badge_name + PAD)),
                Some(js::unknown_of(RESET_STYLE)),
            ],
        );
    } else {
        let at = offset.min(new_args.len());
        new_args.splice(
            at..at,
            [
                Some(js::unknown_of(BADGE_FORMAT)),
                Some(js::unknown_of(BADGE_STYLE)),
                Some(js::unknown_of(PAD.to_owned() + badge_name + PAD)),
                Some(js::unknown_of(RESET_STYLE)),
            ],
        );
    }

    function::bind(method, Some(&console), &new_args)
}
