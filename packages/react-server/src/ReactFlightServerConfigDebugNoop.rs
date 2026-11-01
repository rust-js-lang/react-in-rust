// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::rc::Rc;

use js::Unknown;

use crate::react_flight_async_sequence::AsyncSequence;

// Exported for runtimes that don't support Promise instrumentation for async debugging.
pub fn init_async_debug_info() {}
pub fn mark_async_sequence_root_task() {}
#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_current_async_sequence() -> Option<Rc<AsyncSequence>> {
    None
}
#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_async_sequence_from_promise(
    _promise: Option<&'static Unknown>,
) -> Option<Rc<AsyncSequence>> {
    None
}
