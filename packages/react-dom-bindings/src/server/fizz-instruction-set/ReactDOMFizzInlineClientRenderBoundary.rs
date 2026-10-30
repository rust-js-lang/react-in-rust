// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use crate::react_dom_fizz_instruction_set_shared::{client_render_boundary, set_rx};

js::on_load! {
    // This is a string so Closure's advanced compilation mode doesn't mangle it.
    set_rx(&client_render_boundary);
}
