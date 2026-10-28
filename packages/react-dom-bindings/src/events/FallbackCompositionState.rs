// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};

use js::string;
use webapi::{EventTarget, html_input_element, node};

// These variables store information about text content of a target node,
// allowing comparison of content before and after a given event.
//
// Identify the node where selection currently begins, then observe
// both its text content and its current position in the DOM. Since the
// browser may natively replace the target node during composition, we can
// use its position to find its replacement.

thread_local! {
    static ROOT: Cell<Option<&'static EventTarget>> = const { Cell::new(js::null()) };
    static START_TEXT: RefCell<Option<String>> = const { RefCell::new(js::null()) };
    static FALLBACK_TEXT: RefCell<Option<String>> = const { RefCell::new(js::null()) };
}

pub fn initialize(native_event_target: &'static EventTarget) -> bool {
    ROOT.set(Some(native_event_target));
    START_TEXT.set(get_text());
    true
}

pub fn reset() {
    ROOT.set(js::null());
    START_TEXT.set(js::null());
    FALLBACK_TEXT.set(js::null());
}

pub fn get_data() -> String {
    if let Some(fallback_text) = FALLBACK_TEXT
        .with_borrow(Clone::clone)
        .filter(|text| !text.is_empty())
    {
        return fallback_text;
    }

    let start_value = START_TEXT.with_borrow(Clone::clone).unwrap();
    let start_length = f64::from(string::length(&start_value));
    let end_value = get_text().unwrap();
    let end_length = f64::from(string::length(&end_value));

    let mut start = 0.0;
    while start < start_length {
        if string::get(&start_value, start) != string::get(&end_value, start) {
            break;
        }
        start += 1.0;
    }

    let min_end = start_length - start;
    let mut end = 1.0;
    while end <= min_end {
        if string::get(&start_value, start_length - end)
            != string::get(&end_value, end_length - end)
        {
            break;
        }
        end += 1.0;
    }

    let slice_tail = if end > 1.0 { Some(1.0 - end) } else { None };
    let fallback_text = string::slice(&end_value, start, slice_tail);
    FALLBACK_TEXT.set(Some(fallback_text.clone()));
    fallback_text
}

pub fn get_text() -> Option<String> {
    let root = ROOT.get().unwrap();
    if js::has(root, "value") {
        return Some(html_input_element::unchecked_from(root).value());
    }
    node::unchecked_from(root).text_content()
}
