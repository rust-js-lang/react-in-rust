// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::Unknown;
use js::string::{char_code_at, length, slice};

// Takes a format string (first argument to console) and returns a normalized
// string that has the exact number of arguments as the args. That way it's safe
// to prepend or append to it.
js::export_default! {
    fn normalize_console_format(format_string: &str, args: &[Option<&Unknown>], first_arg: usize) -> String {
        let mut j = first_arg;
        let mut normalized_string = String::new();
        let mut last = 0;
        let mut i = 0;
        while i + 1 < length(format_string) {
            if char_code_at(format_string, i) != 37.0 /* "%" */ {
                i += 1;
                continue;
            }
            i += 1;
            match char_code_at(format_string, i) {
                79.0 /* "O" */ | 99.0 /* "c" */ | 100.0 /* "d" */ | 102.0 /* "f" */ | 105.0 /* "i" */ | 111.0 /* "o" */ | 115.0 /* "s" */ => {
                    if j < args.len() {
                        // We have a matching argument.
                        j += 1;
                    } else {
                        // We have more format specifiers than arguments.
                        // So we need to escape this to print the literal.
                        normalized_string += &(slice(format_string, f64::from(last), Some(f64::from(i))) + "%");
                        last = i;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        normalized_string += &slice(format_string, f64::from(last), Some(f64::from(length(format_string))));
        // Pad with extra format specifiers for the rest.
        while j < args.len() {
            if !normalized_string.is_empty() {
                normalized_string += " ";
            }
            // Not every environment has the same default.
            // This seems to be what Chrome DevTools defaults to.
            normalized_string += if js::type_of(args[j]) == "string" { "%s" } else { "%o" };
            j += 1;
        }
        normalized_string
    }
}
