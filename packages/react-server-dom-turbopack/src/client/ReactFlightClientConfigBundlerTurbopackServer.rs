// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{Promise, Unknown};
use shared::environment::turbopack_load_by_url;
use shared::react_types::ReactDebugInfo;

use crate::shared::react_flight_import_metadata::Chunk;

pub fn load_chunk(filename: &Chunk) -> Promise<Option<&'static Unknown>> {
    turbopack_load_by_url(filename)
}

pub fn add_chunk_debug_info(_target: &mut ReactDebugInfo, _filename: &str) {
    // We don't emit any debug info on the server since we assume the loading
    // of the bundle is insignificant on the server.
}
