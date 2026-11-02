// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::RefCell;
use std::rc::Rc;

use js::{Promise, Unknown, promise};
use react_dom_bindings::react_dom_legacy_server_stream_config::Destination;
use shared::react_types::ReactNodeList;
use webapi::AbortSignal;

use react_server::react_fizz_server::{
    ErrorInfo, OnError, attach_abort_signal as attach_fizz_abort_signal,
    create_request as create_fizz_request, start_flowing as start_fizz_flowing,
    start_work as start_fizz_work,
};

// GAP rd650: these re-exports of ReactFizzConfigDOM import from it, where
// upstream imports them from './ReactFizzConfigMarkup'.
use crate::react_fizz_config_markup::{
    create_render_state, create_resumable_state, create_root_format_context,
};

#[cfg_attr(rust_js, rust_js::name = "version")]
pub use shared::react_version::REACT_VERSION as VERSION;

pub struct MarkupOptions {
    pub identifier_prefix: Option<&'static str>,
    pub signal: Option<&'static AbortSignal>,
    pub on_error: Option<Rc<OnError>>,
}

#[cfg_attr(rust_js, rust_js::name = "experimental_renderToHTML")]
pub fn experimental_render_to_html(
    children: Option<&'static ReactNodeList>,
    options: Option<&'static MarkupOptions>,
) -> Promise<String> {
    promise::new(move |resolve, reject| {
        let buffer = Rc::new(RefCell::new(String::new()));
        let fizz_destination = Destination {
            push: Rc::new(move |chunk: Option<&str>| {
                if let Some(chunk) = chunk {
                    buffer.borrow_mut().push_str(chunk);
                } else {
                    // null indicates that we finished
                    // GAP rd654: `resolve(buffer.borrow().clone())` is refused (ADR 0863),
                    // though `resolve` of a string runs none of this crate's code.
                    resolve(buffer.take());
                }
                true
            }),
            destroy: Rc::new(move |error| {
                reject(error);
            }),
        };
        // GAP rd557: a closure printed as a function declaration prints its
        // body's comments above it.
        let handle_error = move |error: Option<&'static Unknown>, error_info: &ErrorInfo| {
            // Any error rejects the promise, regardless of where it happened.
            // Unlike other React SSR we don't want to put Suspense boundaries into
            // client rendering mode because there's no client rendering here.
            reject(error);

            let on_error = options.and_then(|options| options.on_error.as_ref());
            if let Some(on_error) = on_error {
                on_error(error, error_info);
            }
            None
        };
        let resumable_state = Rc::new(create_resumable_state(
            options.and_then(|options| options.identifier_prefix),
            None,
            None,
            None,
            None,
        ));
        let fizz_request = create_fizz_request(
            children,
            js::unknown_of(Rc::clone(&resumable_state)),
            js::unknown_of(create_render_state(
                &resumable_state,
                None,
                None,
                None,
                None,
                None,
            )),
            js::unknown_of(create_root_format_context(None)),
            Some(f64::INFINITY),
            Some(Rc::new(handle_error)),
            None,
            None,
            None,
            None,
            None,
            None,
        );
        if let Some(signal) = options.and_then(|options| options.signal) {
            attach_fizz_abort_signal(&fizz_request, signal);
        }
        start_fizz_work(&fizz_request);
        start_fizz_flowing(&fizz_request, js::unknown_of(fizz_destination));
    })
}
