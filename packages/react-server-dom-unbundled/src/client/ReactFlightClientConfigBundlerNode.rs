// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::RefCell;
use std::collections::HashMap;

use js::{Dict, Unknown, dict};
use shared::has_own_property::HAS_OWN_PROPERTY;
use shared::react_types::{FulfilledThenable, Thenable};

use crate::shared::react_flight_import_metadata::{ImportMetadata, is_async_import};

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-client/src/ReactFlightClientConfig#prepareDestinationWithChunks",
    rust_js::nullable(module_loading)
)]
fn prepare_destination_with_chunks(
    module_loading: Option<&'static Unknown>,
    chunks: &[String],
    nonce: Option<&str>,
) {
    unreachable!()
}

pub type ServerConsumerModuleMap = Dict<&'static Dict<ClientReference>>;

#[cfg_attr(rust_js, rust_js::untagged)]
pub enum ServerManifest {
    #[cfg_attr(rust_js, rust_js::undefined)]
    Undefined,
}

pub type ServerReferenceId = str;

pub type ClientReferenceMetadata = ImportMetadata;

pub struct ClientReference {
    pub specifier: String,
    pub name: String,
    pub r#async: Option<bool>,
}

// The reason this function needs to defined here in this file instead of just
// being exported directly from the WebpackDestination... file is because the
// ClientReferenceMetadata is opaque and we can't unwrap it there.
// This should get inlined and we could also just implement an unwrapping function
// though that risks it getting used in places it shouldn't be. This is unfortunate
// but currently it seems to be the best option we have.
#[cfg_attr(rust_js, rust_js::nullable(module_loading))]
pub fn prepare_destination_for_module(
    module_loading: Option<&'static Unknown>,
    nonce: Option<&str>,
    metadata: &ClientReferenceMetadata,
) {
    let (ImportMetadata::Async((_, chunks, ..)) | ImportMetadata::Sync((_, chunks, ..))) = metadata;
    prepare_destination_with_chunks(module_loading, chunks, nonce);
}

pub fn resolve_client_reference(
    bundler_config: &ServerConsumerModuleMap,
    metadata: &ClientReferenceMetadata,
) -> ClientReference {
    let (ImportMetadata::Async((id, _, metadata_name, _))
    | ImportMetadata::Sync((id, _, metadata_name))) = metadata;
    let module_exports = dict::get(bundler_config, id);
    let resolved_module_data;
    let name;
    if let Some(module_data) =
        module_exports.and_then(|module_exports| dict::get(module_exports, metadata_name))
    {
        resolved_module_data = module_data;
        // The potentially aliased name.
        name = module_data.name.clone();
    } else {
        // If we don't have this specific name, we might have the full module.
        let Some(module_data) =
            module_exports.and_then(|module_exports| dict::get(module_exports, "*"))
        else {
            panic!(
                "Could not find the module \"{id}\" in the React Server Consumer Manifest. This is probably a bug in the React Server Components bundler."
            );
        };
        resolved_module_data = module_data;
        name = metadata_name.clone();
    }
    ClientReference {
        specifier: resolved_module_data.specifier.clone(),
        name,
        r#async: Some(is_async_import(metadata)),
    }
}

pub fn resolve_server_reference(
    _bundler_config: ServerManifest,
    id: &ServerReferenceId,
) -> ClientReference {
    let idx = js::string::last_index_of(id, "#", None);
    let specifier = js::string::slice(id, 0.0, Some(idx));
    let name = js::string::slice(id, idx + 1.0, None);
    ClientReference {
        specifier,
        name,
        r#async: None,
    }
}

thread_local! {
    static ASYNC_MODULE_CACHE: RefCell<HashMap<String, &'static Thenable>> =
        RefCell::new(HashMap::new());
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn preload_module(metadata: &ClientReference) -> Option<&'static Thenable> {
    let existing_promise = ASYNC_MODULE_CACHE
        .with_borrow(|async_module_cache| async_module_cache.get(&metadata.specifier).copied());
    if let Some(existing_promise) = existing_promise {
        if existing_promise.status().as_deref() == Some("fulfilled") {
            return None;
        }
        Some(existing_promise)
    } else {
        // $FlowFixMe[unsupported-syntax]
        let mut module_promise = js::import_specifier(&metadata.specifier).into_unknown();
        if metadata.r#async == Some(true) {
            // If the module is async, it must have been a CJS module.
            // CJS modules are accessed through the default export in
            // Node.js so we have to get the default export to get the
            // full module exports.
            module_promise = module_promise.then(|value| js::get(value, "default"));
        }
        let module_promise: &'static Thenable = js::upcast(module_promise);
        module_promise.then(
            move |value| {
                let fulfilled_thenable: &FulfilledThenable = module_promise;
                fulfilled_thenable.set_status("fulfilled");
                fulfilled_thenable.set_value(value);
            },
            move |reason| {
                let rejected_thenable: &Thenable = module_promise;
                rejected_thenable.set_status("rejected");
                rejected_thenable.set_reason(reason);
            },
        );
        ASYNC_MODULE_CACHE.with_borrow_mut(|async_module_cache| {
            async_module_cache.insert(metadata.specifier.clone(), module_promise)
        });
        Some(module_promise)
    }
}

pub fn require_module(metadata: &ClientReference) -> Option<&'static Unknown> {
    // We assume that preloadModule has been called before, which
    // should have added something to the module cache.
    let promise = ASYNC_MODULE_CACHE
        .with_borrow(|async_module_cache| async_module_cache.get(&metadata.specifier).copied());
    let module_exports = if js::strict_equals(js::get(promise, "status"), "fulfilled") {
        js::get(promise, "value")
    } else {
        js::throw(js::get(promise, "reason"))
    };
    if metadata.name == "*" {
        // This is a placeholder value that represents that the caller imported this
        // as a CommonJS module as is.
        return module_exports;
    }
    if metadata.name.is_empty() {
        // This is a placeholder value that represents that the caller accessed the
        // default property of this if it was an ESM interop module.
        return js::get(module_exports, "default");
    }
    if HAS_OWN_PROPERTY
        .with(|has_own_property| has_own_property.call(module_exports, &*metadata.name))
    {
        return js::get(module_exports, &*metadata.name);
    }
    None
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_module_debug_info(_metadata: &ClientReference) -> Option<()> {
    // We don't emit any debug info on the server since we assume the loading
    // of the bundle is insignificant on the server.
    None
}
