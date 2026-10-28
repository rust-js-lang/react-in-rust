// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{Kind, Unknown};
use shared::environment::__DEV__;
use webapi::console;

pub fn validate_link_props_for_style_resource(props: &Unknown) -> bool {
    if __DEV__ {
        // This should only be called when we know we are opting into Resource semantics (i.e. precedence is not null)
        let href = js::get(props, "href");
        let on_load = js::get(props, "onLoad");
        let on_error = js::get(props, "onError");
        let disabled = js::get(props, "disabled");
        let mut included_props = Vec::new();
        if js::truthy(on_load) {
            included_props.push("`onLoad`");
        }
        if js::truthy(on_error) {
            included_props.push("`onError`");
        }
        if disabled.is_some() {
            included_props.push("`disabled`");
        }

        let mut included_props_phrase = prop_names_list_join(&included_props, "and");
        included_props_phrase += if included_props.len() == 1 {
            " prop"
        } else {
            " props"
        };
        let with_article_phrase = if included_props.len() == 1 {
            "an ".to_owned() + &included_props_phrase
        } else {
            "the ".to_owned() + &included_props_phrase
        };

        if !included_props.is_empty() {
            console::error((
                concat!(
                    "React encountered a <link rel=\"stylesheet\" href=\"%s\" ... /> with a `precedence` prop that",
                    " also included %s. The presence of loading and error handlers indicates an intent to manage",
                    " the stylesheet loading state from your from your Component code and React will not hoist or",
                    " deduplicate this stylesheet. If your intent was to have React hoist and deduplciate this stylesheet",
                    " using the `precedence` prop remove the %s, otherwise remove the `precedence` prop."
                ),
                href,
                with_article_phrase,
                included_props_phrase,
            ));
            return true;
        }
    }
    false
}

fn prop_names_list_join(list: &[&str], combinator: &str) -> String {
    match list {
        [] => String::new(),
        [only] => (*only).to_owned(),
        [first, second] => (*first).to_owned() + " " + combinator + " " + second,
        [init @ .., last] => init.join(", ") + ", " + combinator + " " + last,
    }
}

pub fn get_value_descriptor_expecting_object_for_warning(thing: Option<&Unknown>) -> String {
    if matches!(js::classify(thing), Kind::Null) {
        "`null`".to_owned()
    } else if matches!(js::classify(thing), Kind::Undefined) {
        "`undefined`".to_owned()
    } else if matches!(js::classify(thing), Kind::String("")) {
        "an empty string".to_owned()
    } else {
        format!("something with type \"{}\"", js::type_of(thing))
    }
}

pub fn get_value_descriptor_expecting_enum_for_warning(thing: Option<&Unknown>) -> String {
    if matches!(js::classify(thing), Kind::Null) {
        "`null`".to_owned()
    } else if matches!(js::classify(thing), Kind::Undefined) {
        "`undefined`".to_owned()
    } else if matches!(js::classify(thing), Kind::String("")) {
        "an empty string".to_owned()
    } else if let Kind::String(text) = js::classify(thing) {
        js::json::stringify(text)
    } else {
        format!("something with type \"{}\"", js::type_of(thing))
    }
}
