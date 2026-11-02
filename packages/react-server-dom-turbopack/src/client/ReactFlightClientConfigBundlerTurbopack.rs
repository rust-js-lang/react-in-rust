// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::rc::Rc;

use js::{Dict, Promise, Unknown, WeakSet, dict};
use shared::environment::{__DEV__, turbopack_require};
use shared::react_types::{FulfilledThenable, ReactDebugInfo, Thenable};

use crate::shared::react_flight_import_metadata::{
    Async, Chunk, ImportManifestEntry, ImportMetadata, is_async_import,
};

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-client/src/ReactFlightClientConfig#prepareDestinationWithChunks",
    rust_js::nullable(module_loading)
)]
fn prepare_destination_with_chunks(
    module_loading: Option<&'static Unknown>,
    chunks: &[Chunk],
    nonce: Option<&str>,
) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-client/src/ReactFlightClientConfig#loadChunk"
)]
fn load_chunk(filename: &Chunk) -> Promise<Option<&'static Unknown>> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-client/src/ReactFlightClientConfig#addChunkDebugInfo"
)]
fn add_chunk_debug_info(target: &mut ReactDebugInfo, filename: &str) {
    unreachable!()
}

use shared::has_own_property::HAS_OWN_PROPERTY;

pub type ServerConsumerModuleMap =
    Option<&'static Dict<&'static Dict<ClientReferenceManifestEntry>>>;

pub type ServerManifest = Dict<ImportManifestEntry>;

pub type ServerReferenceId = str;

pub type ClientReferenceManifestEntry = ImportManifestEntry;
pub type ClientReferenceMetadata = ImportMetadata;

pub type ClientReference = ClientReferenceMetadata;

// The reason this function needs to defined here in this file instead of just
// being exported directly from the TurbopackDestination... file is because the
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
    bundler_config: ServerConsumerModuleMap,
    metadata: Rc<ClientReferenceMetadata>,
) -> Rc<ClientReference> {
    if let Some(bundler_config) = bundler_config {
        let (ImportMetadata::Async((id, _, metadata_name, _))
        | ImportMetadata::Sync((id, _, metadata_name))) = &*metadata;
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
        if is_async_import(&metadata) {
            return Rc::new(ImportMetadata::Async((
                resolved_module_data.id.clone(),
                resolved_module_data.chunks,
                name,
                Async::Async,
            )));
        } else {
            return Rc::new(ImportMetadata::Sync((
                resolved_module_data.id.clone(),
                resolved_module_data.chunks,
                name,
            )));
        }
    }
    metadata
}

pub fn resolve_server_reference(
    bundler_config: &ServerManifest,
    id: &ServerReferenceId,
) -> ClientReference {
    let mut name = String::new();
    let mut resolved_module_data = dict::get(bundler_config, id);
    if let Some(module_data) = resolved_module_data {
        // The potentially aliased name.
        name = module_data.name.clone();
    } else {
        // We didn't find this specific export name but we might have the * export
        // which contains this name as well.
        // TODO: It's unfortunate that we now have to parse this string. We should
        // probably go back to encoding path and name separately on the client reference.
        if let Some(idx) = id.rfind('#') {
            name = id[idx + 1..].to_owned();
            resolved_module_data = dict::get(bundler_config, &id[..idx]);
        }
    }
    let Some(resolved_module_data) = resolved_module_data else {
        panic!(
            "Could not find the module \"{id}\" in the React Server Manifest. This is probably a bug in the React Server Components bundler."
        );
    };
    if resolved_module_data.r#async == Some(true) {
        // If the module is marked as async in a Client Reference, we don't actually care.
        // What matters is whether the consumer wants to unwrap it or not.
        // For Server References, it is different because the consumer is completely internal
        // to the bundler. So instead of passing it to each reference we can mark it in the
        // manifest.
        return ImportMetadata::Async((
            resolved_module_data.id.clone(),
            resolved_module_data.chunks,
            name,
            Async::Async,
        ));
    }
    ImportMetadata::Sync((
        resolved_module_data.id.clone(),
        resolved_module_data.chunks,
        name,
    ))
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn require_async_module(id: &str) -> Option<&'static Thenable> {
    // We've already loaded all the chunks. We can require the module.
    // SAFETY: an async module's exports are a promise, or tested as one below.
    let promise: &'static Thenable = unsafe { js::cast(Some(turbopack_require(id))) };
    if js::type_of(js::get(promise, "then")) != "function" {
        // This wasn't a promise after all.
        None
    } else if promise.status().as_deref() == Some("fulfilled") {
        // This module was already resolved earlier.
        None
    } else {
        // Instrument the Promise to stash the result.
        promise.then(
            move |value| {
                let fulfilled_thenable: &FulfilledThenable = promise;
                fulfilled_thenable.set_status("fulfilled");
                fulfilled_thenable.set_value(value);
            },
            move |reason| {
                let rejected_thenable: &Thenable = promise;
                rejected_thenable.set_status("rejected");
                rejected_thenable.set_reason(reason);
            },
        );
        Some(promise)
    }
}

thread_local! {
    // Turbopack will return cached promises for the same chunk.
    // We still want to keep track of which chunks we have already instrumented
    // and which chunks have already been loaded until Turbopack returns instrumented
    // thenables directly.
    static INSTRUMENTED_CHUNKS: &'static WeakSet<Promise<Option<&'static Unknown>>> = js::weak_set::new();
    static LOADED_CHUNKS: &'static WeakSet<Promise<Option<&'static Unknown>>> = js::weak_set::new();
}

fn ignore_reject(_reason: &'static js::JsError) {
    // We rely on rejected promises to be handled by another listener.
}

// Start preloading the modules since we might need them soon.
// This function doesn't suspend.
#[cfg_attr(rust_js, rust_js::nullable)]
pub fn preload_module(metadata: &ClientReference) -> Option<&'static Thenable> {
    let (ImportMetadata::Async((id, chunks, ..)) | ImportMetadata::Sync((id, chunks, ..))) =
        metadata;
    let mut promises = Vec::new();
    for chunk_filename in *chunks {
        let thenable = load_chunk(chunk_filename);
        if !LOADED_CHUNKS.with(|loaded_chunks| loaded_chunks.has(&thenable)) {
            promises.push(thenable);
        }

        if !INSTRUMENTED_CHUNKS.with(|instrumented_chunks| instrumented_chunks.has(&thenable)) {
            // $FlowFixMe[method-unbinding]
            let resolve = move |_| {
                LOADED_CHUNKS.with(|loaded_chunks| loaded_chunks.add(&thenable));
            };
            let _: Promise<()> = thenable.then_with(resolve, ignore_reject);
            INSTRUMENTED_CHUNKS.with(|instrumented_chunks| instrumented_chunks.add(&thenable));
        }
    }
    if is_async_import(metadata) {
        if promises.is_empty() {
            require_async_module(id)
        } else {
            let id = id.clone();
            Some(js::upcast(
                js::promise::all(promises).then(move |_| require_async_module(&id)),
            ))
        }
    } else if !promises.is_empty() {
        Some(js::upcast(js::promise::all(promises)))
    } else {
        None
    }
}

// Actually require the module or suspend if it's not yet ready.
// Increase priority if necessary.
pub fn require_module(metadata: &ClientReference) -> Option<&'static Unknown> {
    let (ImportMetadata::Async((id, _, name, _)) | ImportMetadata::Sync((id, _, name))) = metadata;
    let mut module_exports = Some(turbopack_require(id));
    if is_async_import(metadata) {
        if js::type_of(js::get(module_exports, "then")) != "function" {
            // This wasn't a promise after all.
        } else if js::strict_equals(js::get(module_exports, "status"), "fulfilled") {
            // This Promise should've been instrumented by preloadModule.
            module_exports = js::get(module_exports, "value");
        } else {
            js::throw(js::get(module_exports, "reason"));
        }
    }
    if name == "*" {
        // This is a placeholder value that represents that the caller imported this
        // as a CommonJS module as is.
        return module_exports;
    }
    if name.is_empty() {
        // This is a placeholder value that represents that the caller accessed the
        // default property of this if it was an ESM interop module.
        return if js::truthy(js::get(module_exports, "__esModule")) {
            js::get(module_exports, "default")
        } else {
            module_exports
        };
    }
    if HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(module_exports, &**name)) {
        return js::get(module_exports, &**name);
    }
    None
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_module_debug_info(metadata: &ClientReference) -> Option<ReactDebugInfo> {
    if !__DEV__ {
        return None;
    }
    let (ImportMetadata::Async((_, chunks, ..)) | ImportMetadata::Sync((_, chunks, ..))) = metadata;
    let mut debug_info: ReactDebugInfo = Vec::new();
    let mut i = 0;
    while i < chunks.len() {
        let chunk = &chunks[i];
        i += 1;
        // A merged chunk is `[mergedChunkFilename, ...]`; use its own filename.
        add_chunk_debug_info(
            &mut debug_info,
            match chunk {
                Chunk::File(chunk) => chunk,
                Chunk::Merged((chunk, ..)) => chunk,
            },
        );
    }
    Some(debug_info)
}
