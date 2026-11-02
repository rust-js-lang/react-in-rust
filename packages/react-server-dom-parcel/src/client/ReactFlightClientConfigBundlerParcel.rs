// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::rc::Rc;

use js::{Dict, Unknown, dict};
use shared::environment::{parcel_require, parcel_require_extend_import_map, parcel_require_load};
use shared::has_own_property::HAS_OWN_PROPERTY;
use shared::react_types::{ReactDebugInfo, Thenable};

use crate::shared::react_flight_import_metadata::ImportMetadata;

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-client/src/ReactFlightClientConfig#prepareDestinationWithChunks"
)]
fn prepare_destination_with_chunks(
    module_loading: ModuleLoading,
    bundles: &[String],
    nonce: Option<&str>,
) {
    unreachable!()
}

pub type ServerManifest = Dict<&'static [String]>;

#[cfg_attr(rust_js, rust_js::untagged)]
pub enum SSRModuleMap {
    #[cfg_attr(rust_js, rust_js::null)]
    Null,
}

#[cfg_attr(rust_js, rust_js::untagged)]
pub enum ModuleLoading {
    #[cfg_attr(rust_js, rust_js::null)]
    Null,
}

#[cfg_attr(rust_js, rust_js::untagged)]
pub enum ServerConsumerModuleMap {
    #[cfg_attr(rust_js, rust_js::null)]
    Null,
}

pub type ServerReferenceId = str;

pub type ClientReferenceMetadata = ImportMetadata;

pub type ClientReference = ImportMetadata;

pub fn prepare_destination_for_module(
    module_loading: ModuleLoading,
    nonce: Option<&str>,
    metadata: &ClientReferenceMetadata,
) {
    let (ImportMetadata::WithImportMap((_, _, bundles, _))
    | ImportMetadata::Plain((_, _, bundles))) = metadata;
    prepare_destination_with_chunks(module_loading, bundles, nonce);
}

pub fn resolve_client_reference(
    _bundler_config: ServerConsumerModuleMap,
    metadata: Rc<ClientReferenceMetadata>,
) -> Rc<ClientReference> {
    // Reference is already resolved during the build.
    metadata
}

pub fn resolve_server_reference(
    bundler_config: &ServerManifest,
    r#ref: &ServerReferenceId,
) -> ClientReference {
    let idx = js::string::last_index_of(r#ref, "#", None);
    let id = js::string::slice(r#ref, 0.0, Some(idx));
    let name = js::string::slice(r#ref, idx + 1.0, None);
    let Some(&bundles) = dict::get(bundler_config, &id) else {
        panic!("{}", "Invalid server action: ".to_owned() + r#ref);
    };
    ImportMetadata::Plain((id, name, bundles))
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn preload_module(metadata: &ClientReference) -> Option<&'static Thenable> {
    let (ImportMetadata::WithImportMap((_, _, bundles, _))
    | ImportMetadata::Plain((_, _, bundles))) = metadata;
    if let ImportMetadata::WithImportMap((.., import_map)) = metadata {
        parcel_require_extend_import_map(import_map);
    }

    if bundles.is_empty() {
        return None;
    }

    // SAFETY: a promise is a thenable.
    Some(unsafe {
        js::cast(Some(js::unknown(&js::promise::all(
            bundles.iter().map(|url| parcel_require_load(url)).collect(),
        ))))
    })
}

pub fn require_module(metadata: &ClientReference) -> Option<&'static Unknown> {
    let (ImportMetadata::WithImportMap((id, name, ..)) | ImportMetadata::Plain((id, name, _))) =
        metadata;
    let module_exports = parcel_require(id);
    if HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(module_exports, &**name)) {
        return js::get(module_exports, &**name);
    }
    None
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_module_debug_info(_metadata: &ClientReference) -> Option<ReactDebugInfo> {
    // TODO
    None
}
