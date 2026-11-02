// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use crate::client::react_flight_client_config_bundler_parcel::ModuleLoading;

pub fn prepare_destination_with_chunks(
    _module_loading: ModuleLoading,
    _bundles: &[String],
    _nonce: Option<&str>,
) {
    // In the browser we don't need to prepare our destination since the browser is the Destination
}
