// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::Error;

use crate::shared::react_flight_import_metadata::ImportMetadata;
use react_server::react_flight_hooks::ReactClientValue;

// GAP rd587: a re-exported type writes no `export type {..}` in the `.d.ts`.
pub use crate::react_flight_parcel_references::{ClientReference, ServerReference};

#[cfg_attr(rust_js, rust_js::untagged)]
pub enum ClientManifest {
    #[cfg_attr(rust_js, rust_js::null)]
    Null,
}

pub type ServerReferenceId = String;
pub type ClientReferenceMetadata = ImportMetadata;
pub type ClientReferenceKey = String;

pub use crate::react_flight_parcel_references::{is_client_reference, is_server_reference};

pub fn get_client_reference_key(reference: &ClientReference) -> ClientReferenceKey {
    reference.id.clone() + "#" + &reference.name
}

pub fn resolve_client_reference_metadata(
    _config: &ClientManifest,
    client_reference: &ClientReference,
) -> ClientReferenceMetadata {
    if let Some(import_map) = client_reference.import_map {
        return ImportMetadata::WithImportMap((
            client_reference.id.clone(),
            client_reference.name.clone(),
            client_reference.bundles,
            import_map,
        ));
    }

    ImportMetadata::Plain((
        client_reference.id.clone(),
        client_reference.name.clone(),
        client_reference.bundles,
    ))
}

pub fn get_server_reference_id(
    _config: &ClientManifest,
    server_reference: &ServerReference,
) -> ServerReferenceId {
    server_reference.id.clone()
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_server_reference_bound_arguments(
    _config: &ClientManifest,
    server_reference: &ServerReference,
) -> Option<&'static [Option<&'static ReactClientValue>]> {
    server_reference.bound
}

pub fn get_server_reference_location(
    _config: &ClientManifest,
    server_reference: &ServerReference,
) -> Option<&'static Error> {
    server_reference.location
}
