// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{Promise, Unknown};

#[cfg_attr(rust_js, rust_js::name = "rendererVersion")]
pub use shared::react_version::REACT_VERSION as RENDERER_VERSION;
#[cfg_attr(rust_js, rust_js::name = "rendererPackageName")]
pub const RENDERER_PACKAGE_NAME: &str = "react-markup";

pub use crate::react_markup_legacy_client_stream_config::*;
pub use react_client::react_client_console_config_plain::*;
pub use react_client::react_client_debug_config_plain::*;

/// `null`, each of what markup's Flight client is given of modules: it
/// renders no client or server references.
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum Null {
    #[cfg_attr(rust_js, rust_js::null)]
    Null,
}

pub type ModuleLoading = Null;
pub type ServerConsumerModuleMap = Null;
pub type ServerManifest = Null;
pub type ServerReferenceId = String;
pub type ClientReferenceMetadata = Null;
pub type ClientReference = Null;

pub fn prepare_destination_for_module(
    module_loading: ModuleLoading,
    nonce: Option<&str>,
    metadata: ClientReferenceMetadata,
) {
    let _ = (module_loading, nonce, metadata);
    panic!("renderToHTML should not have emitted Client References. This is a bug in React.");
}

pub fn resolve_client_reference(
    bundler_config: ServerConsumerModuleMap,
    metadata: ClientReferenceMetadata,
) -> ClientReference {
    let _ = (bundler_config, metadata);
    panic!("renderToHTML should not have emitted Client References. This is a bug in React.");
}

pub fn resolve_server_reference(config: ServerManifest, id: &str) -> ClientReference {
    let _ = (config, id);
    panic!("renderToHTML should not have emitted Server References. This is a bug in React.");
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn preload_module(metadata: ClientReference) -> Option<Promise<Option<&'static Unknown>>> {
    let _ = metadata;
    None
}

pub fn require_module(metadata: ClientReference) -> Option<&'static Unknown> {
    let _ = metadata;
    panic!("renderToHTML should not have emitted Client References. This is a bug in React.");
}

pub fn get_module_debug_info(metadata: ClientReference) {
    let _ = metadata;
    panic!("renderToHTML should not have emitted Client References. This is a bug in React.");
}

#[cfg_attr(rust_js, rust_js::name = "usedWithSSR")]
pub const USED_WITH_SSR: bool = true;

type HintCode = str;
type HintModel = Null;

pub fn dispatch_hint(code: &HintCode, model: HintModel) {
    let _ = (code, model);
    // Should never happen.
}

#[cfg_attr(rust_js, rust_js::name = "preinitModuleForSSR")]
pub fn preinit_module_for_ssr(href: &str, nonce: Option<&str>, cross_origin: Option<&str>) {
    let _ = (href, nonce, cross_origin);
    // Should never happen.
}

#[cfg_attr(rust_js, rust_js::name = "preinitScriptForSSR")]
pub fn preinit_script_for_ssr(href: &str, nonce: Option<&str>, cross_origin: Option<&str>) {
    let _ = (href, nonce, cross_origin);
    // Should never happen.
}
