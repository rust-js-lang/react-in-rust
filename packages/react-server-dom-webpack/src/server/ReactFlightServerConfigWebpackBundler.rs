// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{Dict, Error, dict};

use crate::shared::react_flight_import_metadata::{Async, ImportManifestEntry, ImportMetadata};
use react_server::react_flight_hooks::ReactClientValue;

// GAP rd587: a re-exported type writes no `export type {..}` in the `.d.ts`.
pub use crate::react_flight_webpack_references::{ClientReference, ServerReference};

pub type ClientManifest = Dict<ClientReferenceManifestEntry>;

pub type ServerReferenceId = String;

pub type ClientReferenceMetadata = ImportMetadata;
pub type ClientReferenceManifestEntry = ImportManifestEntry;

pub type ClientReferenceKey = String;

pub use crate::react_flight_webpack_references::{is_client_reference, is_server_reference};

pub fn get_client_reference_key(reference: &ClientReference) -> ClientReferenceKey {
    if reference.r#async {
        reference.id.clone() + "#async"
    } else {
        reference.id.clone()
    }
}

pub fn resolve_client_reference_metadata(
    config: &ClientManifest,
    client_reference: &ClientReference,
) -> ClientReferenceMetadata {
    let module_path = client_reference.id.clone();
    let mut name = String::new();
    let mut resolved_module_data = dict::get(config, &module_path);
    if let Some(resolved_module_data) = resolved_module_data {
        // The potentially aliased name.
        name = resolved_module_data.name.clone();
    } else if let Some(idx) = module_path.rfind('#') {
        // We didn't find this specific export name but we might have the * export
        // which contains this name as well.
        // TODO: It's unfortunate that we now have to parse this string. We should
        // probably go back to encoding path and name separately on the client reference.
        name = module_path[idx + 1..].to_owned();
        resolved_module_data = dict::get(config, &module_path[..idx]);
    }
    let Some(resolved_module_data) = resolved_module_data else {
        panic!(
            "Could not find the module \"{module_path}\" in the React Client Manifest. This is probably a bug in the React Server Components bundler."
        );
    };
    if js::strict_equals(resolved_module_data.r#async, true)
        && js::strict_equals(client_reference.r#async, true)
    {
        panic!(
            "The module \"{module_path}\" is marked as an async ESM module but was loaded as a CJS proxy. This is probably a bug in the React Server Components bundler."
        );
    }
    if js::strict_equals(resolved_module_data.r#async, true)
        || js::strict_equals(client_reference.r#async, true)
    {
        ImportMetadata::Async((
            resolved_module_data.id.clone(),
            resolved_module_data.chunks,
            name,
            Async::Async,
        ))
    } else {
        ImportMetadata::Sync((
            resolved_module_data.id.clone(),
            resolved_module_data.chunks,
            name,
        ))
    }
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
