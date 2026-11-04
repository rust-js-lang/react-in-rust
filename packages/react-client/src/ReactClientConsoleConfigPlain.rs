// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{Kind, Unknown, function};
use webapi::console;

// Keep in sync with ReactServerConsoleConfig
#[cfg_attr(rust_js, rust_js::name = "badgeFormat")]
static BADGE_FORMAT: &str = "[%s]";
#[cfg_attr(rust_js, rust_js::name = "pad")]
static PAD: &str = " ";

pub fn bind_to_console(
    method_name: &str,
    args: &[Option<&'static Unknown>],
    badge_name: &str,
) -> &'static js::Function {
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
                Some(js::unknown_of(PAD.to_owned() + badge_name + PAD)),
            ],
        );
    } else {
        let at = offset.min(new_args.len());
        new_args.splice(
            at..at,
            [
                Some(js::unknown_of(BADGE_FORMAT)),
                Some(js::unknown_of(PAD.to_owned() + badge_name + PAD)),
            ],
        );
    }

    function::bind(method, Some(&console), &new_args)
}
