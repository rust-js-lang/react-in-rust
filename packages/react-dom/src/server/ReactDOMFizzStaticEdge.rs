// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::OnceCell;
use std::rc::{Rc, Weak};

use js::{Promise, Unknown, promise};
use shared::react_types::ReactNodeList;
use webapi::{
    AbortSignal, Headers, QueuingStrategy, ReadableByteStreamController, ReadableStream,
    UnderlyingByteSource, headers, readable_stream,
};

use react_server::react_fizz_server::{
    OnBrowserBailout, OnError, OnFatal, PostponedState, Request, abort, attach_abort_signal,
    create_prerender_request, get_postponed_state, resume_and_prerender_request, start_flowing,
    start_work, stop_flowing,
};

use react_dom_bindings::react_fizz_config_dom::{
    BootstrapScript, HeadersDescriptor, Nonce, OnHeaders, create_render_state,
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
    pub on_error: Option<Rc<OnError>>,
    pub on_browser_bailout: Option<Rc<OnBrowserBailout>>,
    #[cfg_attr(rust_js, rust_js::name = "unstable_externalRuntimeSrc")]
    pub unstable_external_runtime_src: Option<BootstrapScript<'static>>,
    pub import_map: Option<&'static Unknown>,
    pub on_headers: Option<&'static dyn Fn(&'static Headers)>,
    pub max_headers_length: Option<&'static Unknown>,
}

pub struct StaticResult {
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub postponed: Option<Rc<PostponedState>>,
    pub prelude: &'static ReadableStream,
}

pub fn prerender(
    children: Option<&'static ReactNodeList>,
    options: Option<&'static Options>,
) -> Promise<StaticResult> {
    promise::new(move |resolve, reject| {
        let on_fatal_error: Rc<OnFatal> = Rc::new(reject);

        let request_cell: Rc<OnceCell<Weak<Request>>> = Rc::new(OnceCell::new());
        let on_all_ready = {
            let cell = Rc::clone(&request_cell);
            move || {
                let request = cell.get().and_then(Weak::upgrade).unwrap();
                let stream = readable_stream::new(
                    Some(&UnderlyingByteSource {
                        r#type: "bytes",
                        auto_allocate_chunk_size: None,
                        start: None,
                        pull: Some(&|controller: &'static ReadableByteStreamController| {
                            start_flowing(&request, js::unknown(controller));
                        }),
                        cancel: Some(&|reason| {
                            stop_flowing(&request);
                            abort(&request, reason);
                        }),
                    }),
                    // $FlowFixMe[prop-missing] size() methods are not allowed on byte streams.
                    // $FlowFixMe[incompatible-type]
                    Some(QueuingStrategy {
                        high_water_mark: Some(0.0),
                    }),
                );

                let result = StaticResult {
                    postponed: get_postponed_state(&request),
                    prelude: stream,
                };
                resolve(result);
            }
        };

        let on_headers = options.and_then(|options| options.on_headers);
        let mut on_headers_impl: Option<Rc<OnHeaders>> = None;
        if let Some(on_headers) = on_headers {
            on_headers_impl = Some(Rc::new(move |headers_descriptor: &HeadersDescriptor| {
                on_headers(headers::new(Some(headers_descriptor)));
            }));
        }
        let resources = Rc::new(create_resumable_state(
            options.and_then(|options| options.identifier_prefix),
            options.and_then(|options| options.unstable_external_runtime_src),
            options.and_then(|options| options.bootstrap_script_content),
            options.and_then(|options| options.bootstrap_scripts),
            options.and_then(|options| options.bootstrap_modules),
        ));
        let request = create_prerender_request(
            children,
            js::unknown_of(Rc::clone(&resources)),
            js::unknown_of(create_render_state(
                &resources,
                None, // nonce is not compatible with prerendered bootstrap scripts
                options.and_then(|options| options.unstable_external_runtime_src),
                options.and_then(|options| options.import_map),
                on_headers_impl,
                options.and_then(|options| options.max_headers_length),
            )),
            js::unknown_of(create_root_format_context(
                options.and_then(|options| options.namespace_uri),
            )),
            options.and_then(|options| options.progressive_chunk_size),
            options.and_then(|options| options.on_error.clone()),
            options.and_then(|options| options.on_browser_bailout.clone()),
            Some(Rc::new(on_all_ready)),
            None,
            None,
            Some(on_fatal_error),
        );
        let _ = request_cell.set(Rc::downgrade(&request));
        if let Some(signal) = options.and_then(|options| options.signal) {
            attach_abort_signal(&request, signal);
        }
        start_work(&request);
    })
}

pub struct ResumeOptions {
    pub nonce: Option<Nonce<'static>>,
    pub signal: Option<&'static AbortSignal>,
    pub on_error: Option<Rc<OnError>>,
    pub on_browser_bailout: Option<Rc<OnBrowserBailout>>,
}

pub fn resume_and_prerender(
    children: Option<&'static ReactNodeList>,
    postponed_state: &'static PostponedState,
    options: Option<&'static ResumeOptions>,
) -> Promise<StaticResult> {
    promise::new(move |resolve, reject| {
        let on_fatal_error: Rc<OnFatal> = Rc::new(reject);

        let request_cell: Rc<OnceCell<Weak<Request>>> = Rc::new(OnceCell::new());
        let on_all_ready = {
            let cell = Rc::clone(&request_cell);
            move || {
                let request = cell.get().and_then(Weak::upgrade).unwrap();
                let stream = readable_stream::new(
                    Some(&UnderlyingByteSource {
                        r#type: "bytes",
                        auto_allocate_chunk_size: None,
                        start: None,
                        pull: Some(&|controller: &'static ReadableByteStreamController| {
                            start_flowing(&request, js::unknown(controller));
                        }),
                        cancel: Some(&|reason| {
                            stop_flowing(&request);
                            abort(&request, reason);
                        }),
                    }),
                    // $FlowFixMe[prop-missing] size() methods are not allowed on byte streams.
                    // $FlowFixMe[incompatible-type]
                    Some(QueuingStrategy {
                        high_water_mark: Some(0.0),
                    }),
                );

                let result = StaticResult {
                    postponed: get_postponed_state(&request),
                    prelude: stream,
                };
                resolve(result);
            }
        };

        let request = resume_and_prerender_request(
            children,
            postponed_state,
            js::unknown_of(resume_render_state(
                unsafe { js::cast(Some(postponed_state.resumable_state)) },
                None,
            )),
            options.and_then(|options| options.on_error.clone()),
            options.and_then(|options| options.on_browser_bailout.clone()),
            Some(Rc::new(on_all_ready)),
            None,
            None,
            Some(on_fatal_error),
        );
        let _ = request_cell.set(Rc::downgrade(&request));
        if let Some(signal) = options.and_then(|options| options.signal) {
            attach_abort_signal(&request, signal);
        }
        start_work(&request);
    })
}
