// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

pub struct ModuleLoadingOptions {
    pub prefix: String,
    pub cross_origin: Option<String>,
}

#[cfg_attr(rust_js, rust_js::untagged)]
pub enum ModuleLoading {
    #[cfg_attr(rust_js, rust_js::null)]
    Null,
    Prefix(String),
    #[cfg_attr(rust_js, rust_js::otherwise)]
    Options(&'static ModuleLoadingOptions),
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-client/src/ReactFlightClientConfig#preinitModuleForSSR"
)]
fn preinit_module_for_ssr(href: &str, nonce: Option<&str>, cross_origin: Option<&str>) {
    unreachable!()
}

pub fn prepare_destination_for_module_impl(
    module_loading: ModuleLoading,
    // Chunks are double-indexed [..., idx, filenamex, idy, filenamey, ...]
    r#mod: &str,
    nonce: Option<&str>,
) {
    match module_loading {
        ModuleLoading::Prefix(module_loading) => {
            preinit_module_for_ssr(&(module_loading + r#mod), nonce, None);
        }
        ModuleLoading::Options(module_loading) => {
            preinit_module_for_ssr(
                &(module_loading.prefix.clone() + r#mod),
                nonce,
                module_loading.cross_origin.as_deref(),
            );
        }
        ModuleLoading::Null => {}
    }
}
