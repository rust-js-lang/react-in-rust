// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{Kind, Unknown};

// Keep in sync with ReactClientConsoleConfig
#[cfg_attr(rust_js, rust_js::name = "badgeFormat")]
const BADGE_FORMAT: &str = "%c%s%c";
// Same badge styling as DevTools.
#[cfg_attr(rust_js, rust_js::name = "badgeStyle")]
const BADGE_STYLE: &str = concat!(
    // We use a fixed background if light-dark is not supported, otherwise
    // we use a transparent background.
    "background: #e6e6e6;",
    "background: light-dark(rgba(0,0,0,0.1), rgba(255,255,255,0.25));",
    "color: #000000;",
    "color: light-dark(#000000, #ffffff);",
    "border-radius: 2px"
);
#[cfg_attr(rust_js, rust_js::name = "padLength")]
const PAD_LENGTH: f64 = 1.0;

// This mutates the args to remove any badges that was added by a FlightClient and
// returns the name in the badge. This is used when a FlightClient replays inside
// a FlightServer and we capture those replays.
#[cfg_attr(rust_js, rust_js::nullable)]
pub fn unbadge_console(method_name: &str, args: &'static Unknown) -> Option<String> {
    let mut offset = 0.0;
    match method_name {
        "dir" | "dirxml" | "groupEnd" | "table" => {
            // These methods cannot be colorized because they don't take a formatting string.
            // So we wouldn't have added any badge in the first place.
            // $FlowFixMe[incompatible-type]
            return None;
        }
        "assert" => {
            // assert takes formatting options as the second argument.
            offset = 1.0;
        }
        _ => {}
    }
    let format = js::get(args, &offset);
    let style = js::get(args, &(offset + 1.0));
    let badge = js::get(args, &(offset + 2.0));
    if let Kind::String(format) = js::classify(format)
        && format.starts_with(BADGE_FORMAT)
        && js::strict_equals(style, Some(js::unknown(BADGE_STYLE)))
        && let Kind::String(badge) = js::classify(badge)
    {
        // Remove our badging from the arguments.
        let mut unbadged_format =
            js::string::slice(format, f64::from(js::string::length(BADGE_FORMAT)), None);
        if js::string::get(&unbadged_format, 0.0) == Some(" ") {
            // Spacing added on the Client if the original argument was a string.
            unbadged_format = js::string::slice(&unbadged_format, 1.0, None);
        }
        js::array::splice(args, offset, 4.0, (unbadged_format.as_str(),));
        return Some(js::string::slice(
            badge,
            PAD_LENGTH,
            Some(f64::from(js::string::length(badge)) - PAD_LENGTH),
        ));
    }
    None
}
