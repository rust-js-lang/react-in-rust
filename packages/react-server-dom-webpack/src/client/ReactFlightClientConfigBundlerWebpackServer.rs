// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{Promise, Unknown};
use shared::environment::webpack_chunk_load;
use shared::react_types::ReactDebugInfo;

pub fn load_chunk(chunk_id: &str, _filename: &str) -> Promise<Option<&'static Unknown>> {
    webpack_chunk_load(chunk_id)
}

pub fn add_chunk_debug_info(_target: &mut ReactDebugInfo, _chunk_id: &str, _filename: &str) {
    // We don't emit any debug info on the server since we assume the loading
    // of the bundle is insignificant on the server.
}
