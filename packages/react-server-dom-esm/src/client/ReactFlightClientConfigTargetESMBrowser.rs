// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::Unknown;

#[cfg_attr(rust_js, rust_js::untagged)]
pub enum ModuleLoading {
    #[cfg_attr(rust_js, rust_js::null)]
    Null,
}

pub fn prepare_destination_for_module_impl(
    _module_loading: ModuleLoading,
    _chunks: &Unknown,
    _nonce: Option<&str>,
) {
    // In the browser we don't need to prepare our destination since the browser is the Destination
}
