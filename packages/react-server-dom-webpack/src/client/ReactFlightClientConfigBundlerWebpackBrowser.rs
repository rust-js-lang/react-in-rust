// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use js::{Promise, Unknown};
use shared::environment::{
    __DEV__, WEBPACK_REQUIRE_U, set_webpack_require_u, webpack_chunk_load,
    webpack_get_script_filename,
};
use shared::react_types::{ReactAsyncInfo, ReactDebugInfo, ReactIOInfo};
use webapi::{Performance, document, performance, performance_resource_timing, url};

thread_local! {
    static CHUNK_MAP: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
}

js::on_load! {
    /*
     * We patch the chunk filename function in webpack to insert our own resolution
     * of chunks that come from Flight and may not be known to the webpack runtime
     */
    let webpack_get_chunk_filename = WEBPACK_REQUIRE_U;
    set_webpack_require_u(move |chunk_id: &str| {
        let flight_chunk = CHUNK_MAP.with_borrow(|chunk_map| chunk_map.get(chunk_id).cloned());
        if let Some(flight_chunk) = flight_chunk {
            return flight_chunk;
        }
        webpack_get_chunk_filename(chunk_id)
    });
}

pub fn load_chunk(chunk_id: &str, filename: &str) -> Promise<Option<&'static Unknown>> {
    CHUNK_MAP
        .with_borrow_mut(|chunk_map| chunk_map.insert(chunk_id.to_owned(), filename.to_owned()));
    webpack_chunk_load(chunk_id)
}

thread_local! {
    // We cache ReactIOInfo across requests so that inner refreshes can dedupe with outer.
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::name = "chunkIOInfoCache")]
    static CHUNK_IO_INFO_CACHE: Option<RefCell<HashMap<String, Rc<ReactIOInfo>>>> =
        if __DEV__ { Some(RefCell::new(HashMap::new())) } else { None };
}

/// What a chunk's `script` I/O resolves to.
pub struct ChunkValue {
    pub chunk_id: String,
    pub href: String,
}

pub fn add_chunk_debug_info(target: &mut ReactDebugInfo, chunk_id: &str, _filename: &str) {
    if !__DEV__ {
        return;
    }
    CHUNK_IO_INFO_CACHE.with(|chunk_io_info_cache| {
        // SAFETY: the cache is made in DEV, where this runs.
        let chunk_io_info_cache = unsafe { chunk_io_info_cache.as_ref().unwrap_unchecked() };
        let cached = chunk_io_info_cache.borrow().get(chunk_id).cloned();
        let io_info = if let Some(io_info) = cached {
            io_info
        } else {
            let script_filename = webpack_get_script_filename(chunk_id);
            let href = match std::panic::catch_unwind(|| {
                // $FlowFixMe[incompatible-type]
                url::new(&script_filename, Some(&document.base_uri())).href()
            }) {
                Ok(href) => href,
                Err(_) => script_filename,
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
            // $FlowFixMe[prop-missing]
            js::set(
                &value,
                "value",
                ChunkValue {
                    chunk_id: chunk_id.to_owned(),
                    href: href.clone(),
                    // Is there some more useful representation for the chunk?
                },
            );
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
            chunk_io_info_cache
                .borrow_mut()
                .insert(chunk_id.to_owned(), Rc::clone(&io_info));
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
        target.push(Some(js::unknown_of(async_info)));
    });
}
