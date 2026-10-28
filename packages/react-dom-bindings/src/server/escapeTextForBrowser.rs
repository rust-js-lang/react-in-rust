// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.
//
// Based on the escape-html library, which is used under the MIT License below:
//
// Copyright (c) 2012-2013 TJ Holowaychuk
// Copyright (c) 2015 Andreas Lubbe
// Copyright (c) 2015 Tiancheng "Timothy" Gu
//
// Permission is hereby granted, free of charge, to any person obtaining
// a copy of this software and associated documentation files (the
// 'Software'), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to
// permit persons to whom the Software is furnished to do so, subject to
// the following conditions:
//
// The above copyright notice and this permission notice shall be
// included in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED 'AS IS', WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
// IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT,
// TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE
// SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
//
// @flow

// code copied and modified from escape-html
/**
 * Module variables.
 * @private
 */
use js::{RegExp, Unknown, reg_exp};
use shared::check_string_coercion::check_html_string_coercion;
use shared::environment::__DEV__;

thread_local! {
    static MATCH_HTML_REG_EXP: &'static RegExp = reg_exp::new("[\"'&<>]", "");
}

/// Escapes special characters and HTML entities in a given html string.
///
/// @param  {string} string HTML string to escape for later insertion
/// @return {string}
/// @public
fn escape_html(string: Option<&Unknown>) -> String {
    if __DEV__ {
        check_html_string_coercion(string);
    }
    let str = js::concat_text(string).unwrap();
    let Some(r#match) = MATCH_HTML_REG_EXP.with(|match_html_reg_exp| match_html_reg_exp.exec(&str))
    else {
        return str;
    };

    let mut escape;
    let mut html = String::new();
    let mut index = r#match.index();
    let mut last_index = 0;

    while index < js::string::length(&str) {
        match js::string::char_code_at(&str, index) {
            // "
            34.0 => escape = "&quot;",
            // &
            38.0 => escape = "&amp;",
            // '
            39.0 => escape = "&#x27;", // modified from escape-html; used to be '&#39'
            // <
            60.0 => escape = "&lt;",
            // >
            62.0 => escape = "&gt;",
            _ => {
                index += 1;
                continue;
            }
        }

        if last_index != index {
            html += &js::string::slice(&str, last_index.into(), Some(index.into()));
        }

        last_index = index + 1;
        html += escape;
        index += 1;
    }

    if last_index != index {
        html + &js::string::slice(&str, last_index.into(), Some(index.into()))
    } else {
        html
    }
}
// end code copied and modified from escape-html

js::export_default! {
    /// Escapes text to prevent scripting attacks.
    ///
    /// @param {*} text Text value to escape.
    /// @return {string} An escaped string.
    pub fn escape_text_for_browser(text: Option<&Unknown>) -> String {
        if js::type_of(text) == "boolean" || js::type_of(text) == "number" || js::type_of(text) == "bigint" {
            // this shortcircuit helps perf for types that we know will never have
            // special characters, especially given that this function is used often
            // for numeric dom ids.
            return js::concat_text(text).unwrap();
        }
        escape_html(text)
    }
}
