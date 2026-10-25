// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;
use std::panic::catch_unwind;

use js::Kind;
use shared::environment::__DEV__;

thread_local! {
    pub static HAS_BAD_MAP_POLYFILL: Cell<Option<bool>> = const { Cell::new(None) };
}

js::on_load! {
    if __DEV__ {
        HAS_BAD_MAP_POLYFILL.set(Some(false));
        if let Err(_e) = catch_unwind(|| {
            let frozen_object = js::object::freeze(js::dict::new::<Kind>());
            /* eslint-disable no-new */
            js::map::new(Some(&[(frozen_object, Kind::Null)]));
            js::set::new(Some(&[frozen_object]));
            /* eslint-enable no-new */
        }) {
            // TODO: Consider warning about bad polyfills
            HAS_BAD_MAP_POLYFILL.set(Some(true));
        }
    }
}
