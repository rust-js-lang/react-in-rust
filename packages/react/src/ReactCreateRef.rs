// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;

use js::object;
use shared::environment::__DEV__;
use shared::react_types::RefObject;

// an immutable object with a single mutable value
pub fn create_ref() -> RefObject {
    let ref_object = RefObject {
        current: Cell::new(js::null()),
    };
    if __DEV__ {
        object::seal(&ref_object);
    }
    ref_object
}
