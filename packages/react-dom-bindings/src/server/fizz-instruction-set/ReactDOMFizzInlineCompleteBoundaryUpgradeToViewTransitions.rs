// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{Kind, function};

use crate::react_dom_fizz_instruction_set_shared::{
    RV, reveal_completed_boundaries_with_view_transitions, set_rv,
};

js::on_load! {
    // Upgrade the revealCompletedBoundaries instruction to support ViewTransitions.
    // This is a string so Closure's advanced compilation mode doesn't mangle it.
    set_rv(function::bind(
        reveal_completed_boundaries_with_view_transitions,
        Some(&Kind::Null),
        (RV,),
    ));
}
