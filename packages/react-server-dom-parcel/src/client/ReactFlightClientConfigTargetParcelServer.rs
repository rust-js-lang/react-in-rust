// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use shared::environment::PARCEL_REQUIRE_META_PUBLIC_URL;

use crate::client::react_flight_client_config_bundler_parcel::ModuleLoading;

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-client/src/ReactFlightClientConfig#preinitModuleForSSR"
)]
fn preinit_module_for_ssr(href: &str, nonce: Option<&str>) {
    unreachable!()
}

pub fn prepare_destination_with_chunks(
    _module_loading: ModuleLoading,
    bundles: &[String],
    nonce: Option<&str>,
) {
    for bundle in bundles {
        preinit_module_for_ssr(&(PARCEL_REQUIRE_META_PUBLIC_URL.to_owned() + bundle), nonce);
    }
}
