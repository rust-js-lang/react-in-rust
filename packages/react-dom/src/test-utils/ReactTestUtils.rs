// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;

use js::Unknown;
use webapi::console;

// The `react` package's `act`, which may be another copy's.
#[cfg_attr(rust_js, rust_js::link_name = "react#* as IsomorphicReactPackage.act")]
fn react_act(callback: &dyn Fn() -> Option<&'static Unknown>) -> &'static Unknown {
    unreachable!()
}

thread_local! {
    static DID_WARN_ABOUT_USING_ACT: Cell<bool> = const { Cell::new(false) };
}

pub fn act(callback: &dyn Fn() -> Option<&'static Unknown>) -> &'static Unknown {
    if !DID_WARN_ABOUT_USING_ACT.get() {
        DID_WARN_ABOUT_USING_ACT.set(true);
        console::error((concat!(
            "`ReactDOMTestUtils.act` is deprecated in favor of `React.act`. ",
            "Import `act` from `react` instead of `react-dom/test-utils`. ",
            "See https://react.dev/warnings/react-dom-test-utils for more info."
        ),));
    }
    react_act(callback)
}
