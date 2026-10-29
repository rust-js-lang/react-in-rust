// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{Dict, dict};
use webapi::{HTMLElement, html_input_element};

thread_local! {
    /// @see http://www.whatwg.org/specs/web-apps/current-work/multipage/the-input-element.html#input-type-attr-summary
    static SUPPORTED_INPUT_TYPES: &'static Dict<bool> = dict::from_entries(vec![
        ("color", true),
        ("date", true),
        ("datetime", true),
        ("datetime-local", true),
        ("email", true),
        ("month", true),
        ("number", true),
        ("password", true),
        ("range", true),
        ("search", true),
        ("tel", true),
        ("text", true),
        ("time", true),
        ("url", true),
        ("week", true),
    ]);
}

js::export_default! {
    pub fn is_text_input_element(elem: Option<&HTMLElement>) -> bool {
        let node_name = elem
            .map(|elem| elem.node_name())
            .filter(|node_name| !node_name.is_empty())
            .map(|node_name| node_name.to_lowercase());

        if node_name.as_deref() == Some("input")
            && let Some(elem) = elem
        {
            return SUPPORTED_INPUT_TYPES.with(|supported_input_types| {
                dict::get(
                    supported_input_types,
                    &html_input_element::unchecked_from(elem).r#type(),
                )
                .is_some_and(|&supported| supported)
            });
        }

        if node_name.as_deref() == Some("textarea") {
            return true;
        }

        false
    }
}
