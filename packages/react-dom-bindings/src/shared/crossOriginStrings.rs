// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{Kind, Unknown};

/// A `crossOrigin` attribute's value: `""` or `"use-credentials"`.
pub type CrossOriginString = str;

pub fn get_cross_origin_string(input: Option<&Unknown>) -> Option<&CrossOriginString> {
    if let Kind::String(input) = js::classify(input) {
        return Some(if input == "use-credentials" {
            input
        } else {
            ""
        });
    }
    None
}

pub fn get_cross_origin_string_as<'a>(
    r#as: Option<&str>,
    input: Option<&'a Unknown>,
) -> Option<&'a CrossOriginString> {
    if r#as == Some("font") {
        return Some("");
    }
    if let Kind::String(input) = js::classify(input) {
        return Some(if input == "use-credentials" {
            input
        } else {
            ""
        });
    }
    None
}
