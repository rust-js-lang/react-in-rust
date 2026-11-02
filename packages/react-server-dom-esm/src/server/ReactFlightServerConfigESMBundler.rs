// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::Error;

use react_server::react_flight_hooks::ReactClientValue;

// GAP rd587: a re-exported type writes no `export type {..}` in the `.d.ts`.
pub use crate::react_flight_esm_references::{ClientReference, ServerReference};

pub type ClientManifest = str; // base URL on the file system

pub type ServerReferenceId = String;

pub type ClientReferenceMetadata = (
    String, // module path
    String, // export name
);

pub type ClientReferenceKey = String;

pub use crate::react_flight_esm_references::{is_client_reference, is_server_reference};

pub fn get_client_reference_key(reference: &ClientReference) -> ClientReferenceKey {
    reference.id.clone()
}

pub fn resolve_client_reference_metadata(
    config: &ClientManifest,
    client_reference: &ClientReference,
) -> ClientReferenceMetadata {
    let base_url: &str = config;
    let id = &client_reference.id;
    let idx = js::string::last_index_of(id, "#", None);
    let export_name = js::string::slice(id, idx + 1.0, None);
    let full_url = js::string::slice(id, 0.0, Some(idx));
    if !full_url.starts_with(base_url) {
        panic!("Attempted to load a Client Module outside the hosted root.");
    }
    // Relative URL
    let module_path = js::string::slice(&full_url, f64::from(js::string::length(base_url)), None);
    (module_path, export_name)
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
