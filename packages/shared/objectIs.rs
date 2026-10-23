// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{Kind, Unknown};

type Is = fn(&Option<&'static Unknown>, &Option<&'static Unknown>) -> bool;

/// inlined Object.is polyfill to avoid requiring consumers ship their own
/// https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Object/is
fn is(x: &Option<&'static Unknown>, y: &Option<&'static Unknown>) -> bool {
    match (js::classify(*x), js::classify(*y)) {
        (Kind::Number(x), Kind::Number(y)) => {
            (x == y && (x != 0.0 || 1.0 / x == 1.0 / y)) || (x.is_nan() && y.is_nan())
        }
        _ => x == y,
    }
}

thread_local! {
    pub(crate) static OBJECT_IS: Is = if js::is_defined(js::object::is::<Option<&'static Unknown>, Option<&'static Unknown>>) {
        js::object::is
    } else {
        is
    };
}

js::export_default!(OBJECT_IS);
