// Copyright (c) Meta Platforms, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use shared::environment::__DEV__;
use shared::react_shared_internals::REACT_SHARED_INTERNALS;

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn capture_owner_stack() -> Option<String> {
    if __DEV__ {
        let get_current_stack = REACT_SHARED_INTERNALS.get_current_stack()?;
        // The current stack will be the owner stack which it is always here.
        return Some(get_current_stack());
    }

    None
}
