// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// Instruction set for the Fizz external runtime

use std::cell::RefCell;

use js::{Kind, function};
use webapi::performance;

use crate::react_dom_fizz_instruction_set_shared::{
    RevealBoundaries, client_render_boundary, complete_boundary, complete_boundary_with_styles,
    complete_segment, listen_to_form_submissions_for_replaying, reveal_completed_boundaries,
    reveal_completed_boundaries_with_view_transitions, set_rb, set_rc, set_rm, set_rr, set_rs,
    set_rt, set_rv, set_rx,
};

js::on_load! {
    // This is a string so Closure's advanced compilation mode doesn't mangle it.
    // These will be renamed to local references by the external-runtime-plugin.
    set_rm(js::map::new(None));
    set_rb(RefCell::new(Vec::new()));
    set_rx(&client_render_boundary);
    set_rv(function::bind(
        reveal_completed_boundaries_with_view_transitions,
        Some(&Kind::Null),
        (&reveal_completed_boundaries as &RevealBoundaries,),
    ));
    set_rc(&complete_boundary);
    set_rr(&complete_boundary_with_styles);
    set_rs(&complete_segment);

    listen_to_form_submissions_for_replaying();

    // Track the paint time of the shell.
    let entries = if js::truthy(js::get(performance, "getEntriesByType")) {
        performance.get_entries_by_type("paint")
    } else {
        Vec::new()
    };
    if let Some(entry) = entries.first() {
        // We might have already painted before this external runtime loaded. In that case we
        // try to get the first paint from the performance metrics to avoid delaying further
        // than necessary.
        set_rt(entry.start_time());
    } else {
        // Otherwise we wait for the next rAF for it.
        webapi::window_global::request_animation_frame(Box::new(|_| {
            set_rt(performance.now());
        }));
    }
}
