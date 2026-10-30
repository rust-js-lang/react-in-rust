// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use webapi::performance;
use webapi::window_global::request_animation_frame;

use crate::react_dom_fizz_instruction_set_shared::set_rt;

js::on_load! {
    // Track the paint time of the shell
    request_animation_frame(Box::new(|_| {
        set_rt(performance.now());
    }));
}
