// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::panic::catch_unwind;

use js::{JsError, Kind, Unknown};

use crate::environment::__DEV__;

#[cfg_attr(rust_js, rust_js::name = "getIODescription")]
pub fn get_io_description(value: Option<&Unknown>) -> String {
    if !__DEV__ {
        return String::new();
    }
    catch_unwind(|| {
        let Some(value) = value else {
            return String::new();
        };
        match js::classify(Some(value)) {
            Kind::Function(_) => match js::classify(js::get(value, "name")) {
                Kind::String(name) => name.to_string(),
                _ => String::new(),
            },
            Kind::Object(_) | Kind::Array(_) => {
                // Test the object for a bunch of common property names that are useful identifiers.
                // While we only have the return value here, it should ideally be a name that
                // describes the arguments requested.
                if unsafe { js::cast::<&JsError>(Some(value)) }.is_error() {
                    return js::string(js::get(value, "message"));
                }
                if let Kind::String(url) = js::classify(js::get(value, "url")) {
                    return url.to_string();
                }
                if let Kind::String(href) = js::classify(js::get(value, "href")) {
                    return href.to_string();
                }
                if let Kind::String(src) = js::classify(js::get(value, "src")) {
                    return src.to_string();
                }
                if let Kind::String(current_src) =
                    js::classify(js::get(value, "currentSrc"))
                {
                    return current_src.to_string();
                }
                if let Kind::String(command) = js::classify(js::get(value, "command")) {
                    return command.to_string();
                }
                if let Some(request) = js::get(value, "request")
                    && let Kind::Object(_) | Kind::Array(_) = js::classify(Some(request))
                    && let Kind::String(url) = js::classify(js::get(request, "url"))
                {
                    return url.to_string();
                }
                if let Some(response) = js::get(value, "response")
                    && let Kind::Object(_) | Kind::Array(_) = js::classify(Some(response))
                    && let Kind::String(url) = js::classify(js::get(response, "url"))
                {
                    return url.to_string();
                }
                if let Some(id) = js::get(value, "id")
                    && let Kind::String(_) | Kind::Number(_) | Kind::BigInt(_) = js::classify(Some(id))
                {
                    return js::string(Some(id));
                }
                if let Kind::String(name) = js::classify(js::get(value, "name")) {
                    return name.to_string();
                }
                let str = js::string(Some(value));
                if str.starts_with("[object ")
                    || str.encode_utf16().count() < 5
                    || str.encode_utf16().count() > 500
                {
                    // This is probably not a useful description.
                    return String::new();
                }
                str
            }
            Kind::String(text) => {
                if text.encode_utf16().count() < 5 || text.encode_utf16().count() > 500 {
                    return String::new();
                }
                text.to_string()
            }
            Kind::Number(_) | Kind::BigInt(_) => js::string(Some(value)),
            // Not useful descriptors.
            _ => String::new(),
        }
    })
    .unwrap_or_default()
}
