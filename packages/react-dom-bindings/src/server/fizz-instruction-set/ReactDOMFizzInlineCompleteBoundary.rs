// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::RefCell;

use crate::react_dom_fizz_instruction_set_shared::{
    complete_boundary, reveal_completed_boundaries, set_rb, set_rc, set_rv,
};

js::on_load! {
    // This is a string so Closure's advanced compilation mode doesn't mangle it.
    set_rb(RefCell::new(Vec::new()));
    set_rv(&reveal_completed_boundaries);
    set_rc(&complete_boundary);
}
