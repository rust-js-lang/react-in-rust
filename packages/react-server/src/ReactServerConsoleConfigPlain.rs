// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{Kind, Unknown};

// Keep in sync with ReactClientConsoleConfig
#[cfg_attr(rust_js, rust_js::name = "badgeFormat")]
const BADGE_FORMAT: &str = "[%s]";
#[cfg_attr(rust_js, rust_js::name = "padLength")]
const PAD_LENGTH: f64 = 1.0;
#[cfg_attr(rust_js, rust_js::name = "pad")]
const PAD: &str = " ";

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
    let badge = js::get(args, &(offset + 1.0));
    if let Kind::String(format) = js::classify(format)
        && format.starts_with(BADGE_FORMAT)
        && let Kind::String(badge) = js::classify(badge)
        && badge.starts_with(PAD)
        && badge.ends_with(PAD)
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
