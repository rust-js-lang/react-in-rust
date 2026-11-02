// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use crate::shared::react_flight_import_metadata::Chunk;

#[cfg_attr(rust_js, rust_js::untagged)]
#[derive(Clone, Copy)]
pub enum CrossOrigin {
    #[cfg_attr(rust_js, rust_js::name = "use-credentials")]
    UseCredentials,
    #[cfg_attr(rust_js, rust_js::name = "")]
    Anonymous,
}

pub struct ModuleLoadingOptions {
    pub prefix: String,
    pub cross_origin: Option<CrossOrigin>,
}

pub type ModuleLoading = Option<&'static ModuleLoadingOptions>;

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-client/src/ReactFlightClientConfig#preinitScriptForSSR"
)]
fn preinit_script_for_ssr(href: &str, nonce: Option<&str>, cross_origin: Option<CrossOrigin>) {
    unreachable!()
}

pub fn prepare_destination_with_chunks(
    module_loading: ModuleLoading,
    chunks: &[Chunk],
    nonce: Option<&str>,
) {
    if let Some(module_loading) = module_loading {
        for chunk in chunks {
            // A merged chunk is `[mergedChunkFilename, ...]`; preinit its own URL.
            preinit_script_for_ssr(
                &(module_loading.prefix.clone()
                    + match chunk {
                        Chunk::File(chunk) => chunk,
                        Chunk::Merged((chunk, ..)) => chunk,
                    }),
                nonce,
                module_loading.cross_origin,
            );
        }
    }
}
