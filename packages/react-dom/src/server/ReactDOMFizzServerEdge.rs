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
    FormState, OnBrowserBailout, OnError, OnFatal, PostponedState, Request, abort,
    attach_abort_signal, create_request, resume_request, start_flowing, start_work, stop_flowing,
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
    pub nonce: Option<Nonce<'static>>,
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
    pub form_state: Option<&'static FormState>,
    pub on_headers: Option<&'static dyn Fn(&'static Headers)>,
    pub max_headers_length: Option<&'static Unknown>,
}

pub struct ResumeOptions {
    pub nonce: Option<Nonce<'static>>,
    pub signal: Option<&'static AbortSignal>,
    pub on_error: Option<Rc<OnError>>,
    pub on_browser_bailout: Option<Rc<OnBrowserBailout>>,
    #[cfg_attr(rust_js, rust_js::name = "unstable_externalRuntimeSrc")]
    pub unstable_external_runtime_src: Option<BootstrapScript<'static>>,
}

// TODO: Move to sub-classing ReadableStream.
pub type ReactDOMServerReadableStream = ReadableStream;

pub fn render_to_readable_stream(
    children: Option<&'static ReactNodeList>,
    options: Option<&'static Options>,
) -> Promise<&'static ReactDOMServerReadableStream> {
    promise::new(move |resolve, reject| {
        let mut on_fatal_error: Option<Rc<OnFatal>> = None;
        let mut on_all_ready: Option<Rc<dyn Fn()>> = None;
        let all_ready: Promise<()> = promise::new(|res, rej| {
            on_all_ready = Some(Rc::new(move || res(())));
            on_fatal_error = Some(Rc::new(rej));
        });

        let request_cell: Rc<OnceCell<Weak<Request>>> = Rc::new(OnceCell::new());
        let on_shell_ready = {
            let cell = Rc::clone(&request_cell);
            move || {
                let request = cell.get().and_then(Weak::upgrade).unwrap();
                let stream: &'static ReactDOMServerReadableStream = readable_stream::new(
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
                // TODO: Move to sub-classing ReadableStream.
                js::set(js::unknown(stream), "allReady", all_ready);
                resolve(stream);
            }
        };
        let on_shell_error = move |error: Option<&'static Unknown>| {
            // If the shell errors the caller of `renderToReadableStream` won't have access to `allReady`.
            // However, `allReady` will be rejected by `onFatalError` as well.
            // So we need to catch the duplicate, uncatchable fatal error in `allReady` to prevent a `UnhandledPromiseRejection`.
            let _: Promise<()> = all_ready.catch(|_| {});
            reject(error);
        };

        let on_headers = options.and_then(|options| options.on_headers);
        let mut on_headers_impl: Option<Rc<OnHeaders>> = None;
        if let Some(on_headers) = on_headers {
            on_headers_impl = Some(Rc::new(move |headers_descriptor: &HeadersDescriptor| {
                on_headers(headers::new(Some(headers_descriptor)));
            }));
        }

        let resumable_state = Rc::new(create_resumable_state(
            options.and_then(|options| options.identifier_prefix),
            options.and_then(|options| options.unstable_external_runtime_src),
            options.and_then(|options| options.bootstrap_script_content),
            options.and_then(|options| options.bootstrap_scripts),
            options.and_then(|options| options.bootstrap_modules),
        ));
        let request = create_request(
            children,
            js::unknown_of(Rc::clone(&resumable_state)),
            js::unknown_of(create_render_state(
                &resumable_state,
                options.and_then(|options| options.nonce),
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
            on_all_ready,
            Some(Rc::new(on_shell_ready)),
            Some(Rc::new(on_shell_error)),
            on_fatal_error,
            options.and_then(|options| options.form_state),
        );
        let _ = request_cell.set(Rc::downgrade(&request));
        if let Some(signal) = options.and_then(|options| options.signal) {
            attach_abort_signal(&request, signal);
        }
        start_work(&request);
    })
}

pub fn resume(
    children: Option<&'static ReactNodeList>,
    postponed_state: &'static PostponedState,
    options: Option<&'static ResumeOptions>,
) -> Promise<&'static ReactDOMServerReadableStream> {
    promise::new(move |resolve, reject| {
        let mut on_fatal_error: Option<Rc<OnFatal>> = None;
        let mut on_all_ready: Option<Rc<dyn Fn()>> = None;
        let all_ready: Promise<()> = promise::new(|res, rej| {
            on_all_ready = Some(Rc::new(move || res(())));
            on_fatal_error = Some(Rc::new(rej));
        });

        let request_cell: Rc<OnceCell<Weak<Request>>> = Rc::new(OnceCell::new());
        let on_shell_ready = {
            let cell = Rc::clone(&request_cell);
            move || {
                let request = cell.get().and_then(Weak::upgrade).unwrap();
                let stream: &'static ReactDOMServerReadableStream = readable_stream::new(
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
                // TODO: Move to sub-classing ReadableStream.
                js::set(js::unknown(stream), "allReady", all_ready);
                resolve(stream);
            }
        };
        let on_shell_error = move |error: Option<&'static Unknown>| {
            // If the shell errors the caller of `renderToReadableStream` won't have access to `allReady`.
            // However, `allReady` will be rejected by `onFatalError` as well.
            // So we need to catch the duplicate, uncatchable fatal error in `allReady` to prevent a `UnhandledPromiseRejection`.
            let _: Promise<()> = all_ready.catch(|_| {});
            reject(error);
        };
        let request = resume_request(
            children,
            postponed_state,
            js::unknown_of(resume_render_state(
                unsafe { js::cast(Some(postponed_state.resumable_state)) },
                options.and_then(|options| options.nonce),
            )),
            options.and_then(|options| options.on_error.clone()),
            options.and_then(|options| options.on_browser_bailout.clone()),
            on_all_ready,
            Some(Rc::new(on_shell_ready)),
            Some(Rc::new(on_shell_error)),
            on_fatal_error,
        );
        let _ = request_cell.set(Rc::downgrade(&request));
        if let Some(signal) = options.and_then(|options| options.signal) {
            attach_abort_signal(&request, signal);
        }
        start_work(&request);
    })
}
