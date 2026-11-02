// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use js::Unknown;
use shared::environment::__DEV__;
use shared::react_types::{
    FulfilledThenable, ReactAsyncInfo, ReactDebugInfo, ReactIOInfo, Thenable,
};
use webapi::{Performance, document, performance, performance_resource_timing, url};

pub type ServerConsumerModuleMap = str; // Module root path

pub type ServerManifest = str; // Module root path

pub type ServerReferenceId = str;

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-client/src/ReactFlightClientConfig#prepareDestinationForModuleImpl",
    rust_js::nullable(module_loading)
)]
fn prepare_destination_for_module_impl(
    module_loading: Option<&'static Unknown>,
    r#mod: &str,
    nonce: Option<&str>,
) {
    unreachable!()
}

pub type ClientReferenceMetadata = (
    String, // module path
    String, // export name
);

pub struct ClientReference {
    pub specifier: String,
    pub name: String,
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
    prepare_destination_for_module_impl(module_loading, &metadata.0, nonce);
}

pub fn resolve_client_reference(
    bundler_config: &ServerConsumerModuleMap,
    metadata: &ClientReferenceMetadata,
) -> ClientReference {
    let base_url = bundler_config;
    ClientReference {
        specifier: base_url.to_owned() + &metadata.0,
        name: metadata.1.clone(),
    }
}

pub fn resolve_server_reference(
    config: &ServerManifest,
    id: &ServerReferenceId,
) -> ClientReference {
    let base_url: &str = config;
    let idx = js::string::last_index_of(id, "#", None);
    let export_name = js::string::slice(id, idx + 1.0, None);
    let full_url = js::string::slice(id, 0.0, Some(idx));
    if !full_url.starts_with(base_url) {
        panic!("Attempted to load a Server Reference outside the hosted root.");
    }
    ClientReference {
        specifier: full_url,
        name: export_name,
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
        let module_promise: &'static Thenable =
            js::upcast(js::import_specifier(&metadata.specifier));
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
    js::get(module_exports, &*metadata.name)
}

thread_local! {
    // We cache ReactIOInfo across requests so that inner refreshes can dedupe with outer.
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::name = "moduleIOInfoCache")]
    static MODULE_IO_INFO_CACHE: Option<RefCell<HashMap<String, Rc<ReactIOInfo>>>> =
        if __DEV__ { Some(RefCell::new(HashMap::new())) } else { None };
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_module_debug_info(metadata: &ClientReference) -> Option<ReactDebugInfo> {
    if !__DEV__ {
        return None;
    }
    let filename = &*metadata.specifier;
    MODULE_IO_INFO_CACHE.with(|module_io_info_cache| {
        // SAFETY: the cache is made in DEV, where this runs.
        let module_io_info_cache = unsafe { module_io_info_cache.as_ref().unwrap_unchecked() };
        let cached = module_io_info_cache.borrow().get(filename).cloned();
        let io_info = if let Some(io_info) = cached {
            io_info
        } else {
            let href = match std::panic::catch_unwind(|| {
                // $FlowFixMe[incompatible-type]
                url::new(filename, Some(&document.base_uri())).href()
            }) {
                Ok(href) => href,
                Err(_) => filename.to_owned(),
            };
            let mut start = -1.0;
            let mut end = -1.0;
            let mut byte_size = 0.0;
            // $FlowFixMe[method-unbinding]
            if js::is_function(Performance::get_entries_by_type) {
                // We may be able to collect the start and end time of this resource from Performance Observer.
                let resource_entries = performance.get_entries_by_type("resource");
                for resource_entry in resource_entries {
                    if resource_entry.name() == href {
                        start = resource_entry.start_time();
                        end = start + resource_entry.duration();
                        // $FlowFixMe[prop-missing]
                        let transfer_size =
                            performance_resource_timing::unchecked_from(resource_entry)
                                .transfer_size();
                        byte_size = if js::truthy(Some(&transfer_size)) {
                            transfer_size
                        } else {
                            0.0
                        };
                    }
                }
            }
            let value = js::promise::resolve(href.clone());
            // $FlowFixMe[prop-missing]
            js::set(&value, "status", "fulfilled");
            // Is there some more useful representation for the chunk?
            // $FlowFixMe[prop-missing]
            js::set(&value, "value", href.clone());
            // Create a fake stack frame that points to the beginning of the chunk. This is
            // probably not source mapped so will link to the compiled source rather than
            // any individual file that goes into the chunks.
            let fake_stack = js::error::new(Some("react-stack-top-frame"), None);
            if fake_stack
                .stack()
                .unwrap_or_default()
                .starts_with("Error: react-stack-top-frame")
            {
                // Looks like V8
                fake_stack.set_stack(Some(
                    &("Error: react-stack-top-frame\n".to_owned()
                        // Add two frames since we always trim one off the top.
                        + "    at Client Component Bundle ("
                        + &href
                        + ":1:1)\n"
                        + "    at Client Component Bundle ("
                        + &href
                        + ":1:1)"),
                ));
            } else {
                // Looks like Firefox or Safari.
                // Add two frames since we always trim one off the top.
                fake_stack.set_stack(Some(
                    &("Client Component Bundle@".to_owned()
                        + &href
                        + ":1:1\n"
                        + "Client Component Bundle@"
                        + &href
                        + ":1:1"),
                ));
            }
            let io_info = Rc::new(ReactIOInfo {
                name: Cell::new("script"),
                start: Cell::new(start),
                end: Cell::new(end),
                byte_size: Cell::new(None),
                value: Cell::new(Some(value.into_unknown())),
                env: None,
                owner: None,
                stack: None,
                debug_stack: Cell::new(Some(fake_stack)),
                debug_task: Cell::new(None),
            });
            if byte_size > 0.0 {
                // $FlowFixMe[cannot-write]
                io_info.byte_size.set(Some(byte_size));
            }
            module_io_info_cache
                .borrow_mut()
                .insert(filename.to_owned(), Rc::clone(&io_info));
            io_info
        };
        // We could dedupe the async info too but conceptually each request is its own await.
        let async_info = ReactAsyncInfo {
            awaited: io_info,
            env: None,
            owner: Cell::new(None),
            stack: RefCell::new(None),
            debug_stack: Cell::new(None),
            debug_task: Cell::new(None),
        };
        Some(vec![Some(js::unknown_of(async_info))])
    })
}
