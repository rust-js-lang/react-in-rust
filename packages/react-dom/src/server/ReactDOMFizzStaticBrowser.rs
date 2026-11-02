// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;
use std::rc::Rc;

use js::{Promise, Unknown, promise};
use webapi::{
    AbortSignal, Headers, QueuingStrategy, ReadableByteStreamController, ReadableStream,
    UnderlyingByteSource, headers, readable_stream,
};

use react_server::react_fizz_current_task::{
    OnBrowserBailout, OnError, PostponedState, Request, abort, attach_abort_signal,
    create_prerender_request, get_postponed_state, resume_and_prerender_request, start_flowing,
    start_work, stop_flowing,
};

use react_dom_bindings::react_fizz_config_dom::{
    BootstrapScript, HeadersDescriptor, Nonce, OnHeaders, ResumableState, create_render_state,
    create_resumable_state, create_root_format_context, resume_render_state,
};

use crate::ensure_correct_isomorphic_react_version::ensure_correct_isomorphic_react_version;

#[cfg_attr(rust_js, rust_js::name = "version")]
pub use shared::react_version::REACT_VERSION as VERSION;

js::on_load! {
    ensure_correct_isomorphic_react_version();
}

pub struct Options {
    pub identifier_prefix: Option<&'static str>,
    #[cfg_attr(rust_js, rust_js::name = "namespaceURI")]
    pub namespace_uri: Option<&'static str>,
    pub bootstrap_script_content: Option<&'static str>,
    pub bootstrap_scripts: Option<&'static [BootstrapScript<'static>]>,
    pub bootstrap_modules: Option<&'static [BootstrapScript<'static>]>,
    pub progressive_chunk_size: Option<f64>,
    pub signal: Option<&'static AbortSignal>,
    pub on_error: Option<&'static OnError>,
    pub on_browser_bailout: Option<&'static OnBrowserBailout>,
    #[cfg_attr(rust_js, rust_js::name = "unstable_externalRuntimeSrc")]
    pub unstable_external_runtime_src: Option<BootstrapScript<'static>>,
    pub import_map: Option<&'static Unknown>,
    pub on_headers: Option<&'static dyn Fn(&'static Headers)>,
    pub max_headers_length: Option<&'static Unknown>,
}

pub struct StaticResult {
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub postponed: Option<&'static PostponedState<ResumableState>>,
    pub prelude: &'static ReadableStream,
}

pub fn prerender(
    children: Option<&'static Unknown>,
    options: Option<&'static Options>,
) -> Promise<StaticResult> {
    promise::new(move |resolve, reject| {
        let on_fatal_error = reject;
        let request: Cell<Option<&'static Request>> = Cell::new(None);
        let on_all_ready = || {
            let stream = readable_stream::new(
                Some(&UnderlyingByteSource {
                    r#type: "bytes",
                    auto_allocate_chunk_size: None,
                    start: None,
                    pull: Some(&|controller: &'static ReadableByteStreamController| {
                        start_flowing(request.get().unwrap(), controller);
                    }),
                    cancel: Some(&|reason| {
                        stop_flowing(request.get().unwrap());
                        abort(request.get().unwrap(), reason);
                    }),
                }),
                // $FlowFixMe[prop-missing] size() methods are not allowed on byte streams.
                // $FlowFixMe[incompatible-type]
                Some(QueuingStrategy {
                    high_water_mark: Some(0.0),
                }),
            );
            let result = StaticResult {
                postponed: get_postponed_state(request.get().unwrap()),
                prelude: stream,
            };
            resolve(result);
        };

        let on_headers = options.and_then(|options| options.on_headers);
        let mut on_headers_impl: Option<Rc<OnHeaders>> = None;
        if let Some(on_headers) = on_headers {
            on_headers_impl = Some(Rc::new(move |headers_descriptor: &HeadersDescriptor| {
                on_headers(headers::new(Some(headers_descriptor)));
            }));
        }
        let resources = create_resumable_state(
            options.and_then(|options| options.identifier_prefix),
            options.and_then(|options| options.unstable_external_runtime_src),
            options.and_then(|options| options.bootstrap_script_content),
            options.and_then(|options| options.bootstrap_scripts),
            options.and_then(|options| options.bootstrap_modules),
        );
        request.set(Some(create_prerender_request(
            children,
            &resources,
            &create_render_state(
                &resources,
                None, // nonce is not compatible with prerendered bootstrap scripts
                options.and_then(|options| options.unstable_external_runtime_src),
                options.and_then(|options| options.import_map),
                on_headers_impl,
                options.and_then(|options| options.max_headers_length),
            ),
            &create_root_format_context(options.and_then(|options| options.namespace_uri)),
            options.and_then(|options| options.progressive_chunk_size),
            options.and_then(|options| options.on_error),
            options.and_then(|options| options.on_browser_bailout),
            Some(&on_all_ready),
            None,
            None,
            Some(on_fatal_error),
        )));
        let request = request.get().unwrap();
        if let Some(signal) = options.and_then(|options| options.signal) {
            attach_abort_signal(request, signal);
        }
        start_work(request);
    })
}

pub struct ResumeOptions {
    pub nonce: Option<Nonce<'static>>,
    pub signal: Option<&'static AbortSignal>,
    pub on_error: Option<&'static OnError>,
    pub on_browser_bailout: Option<&'static OnBrowserBailout>,
    #[cfg_attr(rust_js, rust_js::name = "unstable_externalRuntimeSrc")]
    pub unstable_external_runtime_src: Option<BootstrapScript<'static>>,
}

pub fn resume_and_prerender(
    children: Option<&'static Unknown>,
    postponed_state: &'static PostponedState<ResumableState>,
    options: Option<&'static ResumeOptions>,
) -> Promise<StaticResult> {
    promise::new(move |resolve, reject| {
        let on_fatal_error = reject;
        let request: Cell<Option<&'static Request>> = Cell::new(None);
        let on_all_ready = || {
            let stream = readable_stream::new(
                Some(&UnderlyingByteSource {
                    r#type: "bytes",
                    auto_allocate_chunk_size: None,
                    start: None,
                    pull: Some(&|controller: &'static ReadableByteStreamController| {
                        start_flowing(request.get().unwrap(), controller);
                    }),
                    cancel: Some(&|reason| {
                        stop_flowing(request.get().unwrap());
                        abort(request.get().unwrap(), reason);
                    }),
                }),
                // $FlowFixMe[prop-missing] size() methods are not allowed on byte streams.
                // $FlowFixMe[incompatible-type]
                Some(QueuingStrategy {
                    high_water_mark: Some(0.0),
                }),
            );
            let result = StaticResult {
                postponed: get_postponed_state(request.get().unwrap()),
                prelude: stream,
            };
            resolve(result);
        };

        request.set(Some(resume_and_prerender_request(
            children,
            postponed_state,
            &resume_render_state(postponed_state.resumable_state(), None),
            options.and_then(|options| options.on_error),
            options.and_then(|options| options.on_browser_bailout),
            Some(&on_all_ready),
            None,
            None,
            Some(on_fatal_error),
        )));
        let request = request.get().unwrap();
        if let Some(signal) = options.and_then(|options| options.signal) {
            attach_abort_signal(request, signal);
        }
        start_work(request);
    })
}
