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
    OnBrowserBailout, OnError, PostponedState, Request, abort, attach_abort_signal, create_request,
    resume_request, start_flowing, start_work, stop_flowing,
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
    pub nonce: Option<Nonce<'static>>,
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
    pub form_state: Option<&'static Unknown>,
    pub on_headers: Option<&'static dyn Fn(&'static Headers)>,
    pub max_headers_length: Option<&'static Unknown>,
}

pub struct ResumeOptions {
    pub nonce: Option<Nonce<'static>>,
    pub signal: Option<&'static AbortSignal>,
    pub on_error: Option<&'static OnError>,
    pub on_browser_bailout: Option<&'static OnBrowserBailout>,
    #[cfg_attr(rust_js, rust_js::name = "unstable_externalRuntimeSrc")]
    pub unstable_external_runtime_src: Option<BootstrapScript<'static>>,
}

// TODO: Move to sub-classing ReadableStream.
pub type ReactDOMServerReadableStream = ReadableStream;

pub fn render_to_readable_stream(
    children: Option<&'static Unknown>,
    options: Option<&'static Options>,
) -> Promise<&'static ReactDOMServerReadableStream> {
    promise::new(move |resolve, reject| {
        let mut on_fatal_error = None;
        let mut on_all_ready = None;
        let all_ready: Promise<()> = promise::new(|res, rej| {
            on_all_ready = Some(res);
            on_fatal_error = Some(rej);
        });

        let request: Cell<Option<&'static Request>> = Cell::new(None);
        let on_shell_ready = || {
            let stream: &'static ReactDOMServerReadableStream = readable_stream::new(
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
            // TODO: Move to sub-classing ReadableStream.
            js::set(js::unknown(stream), "allReady", all_ready);
            resolve(stream);
        };
        let on_shell_error = |error: Option<&'static Unknown>| {
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

        let resumable_state: ResumableState = create_resumable_state(
            options.and_then(|options| options.identifier_prefix),
            options.and_then(|options| options.unstable_external_runtime_src),
            options.and_then(|options| options.bootstrap_script_content),
            options.and_then(|options| options.bootstrap_scripts),
            options.and_then(|options| options.bootstrap_modules),
        );
        let on_all_ready = on_all_ready.unwrap();
        request.set(Some(create_request(
            children,
            &resumable_state,
            &create_render_state(
                &resumable_state,
                options.and_then(|options| options.nonce),
                options.and_then(|options| options.unstable_external_runtime_src),
                options.and_then(|options| options.import_map),
                on_headers_impl,
                options.and_then(|options| options.max_headers_length),
            ),
            &create_root_format_context(options.and_then(|options| options.namespace_uri)),
            options.and_then(|options| options.progressive_chunk_size),
            options.and_then(|options| options.on_error),
            options.and_then(|options| options.on_browser_bailout),
            Some(&|| on_all_ready(())),
            Some(&on_shell_ready),
            Some(&on_shell_error),
            on_fatal_error,
            options.and_then(|options| options.form_state),
        )));
        let request = request.get().unwrap();
        if let Some(signal) = options.and_then(|options| options.signal) {
            attach_abort_signal(request, signal);
        }
        start_work(request);
    })
}

pub fn resume(
    children: Option<&'static Unknown>,
    postponed_state: &'static PostponedState<ResumableState>,
    options: Option<&'static ResumeOptions>,
) -> Promise<&'static ReactDOMServerReadableStream> {
    promise::new(move |resolve, reject| {
        let mut on_fatal_error = None;
        let mut on_all_ready = None;
        let all_ready: Promise<()> = promise::new(|res, rej| {
            on_all_ready = Some(res);
            on_fatal_error = Some(rej);
        });

        let request: Cell<Option<&'static Request>> = Cell::new(None);
        let on_shell_ready = || {
            let stream: &'static ReactDOMServerReadableStream = readable_stream::new(
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
            // TODO: Move to sub-classing ReadableStream.
            js::set(js::unknown(stream), "allReady", all_ready);
            resolve(stream);
        };
        let on_shell_error = |error: Option<&'static Unknown>| {
            // If the shell errors the caller of `renderToReadableStream` won't have access to `allReady`.
            // However, `allReady` will be rejected by `onFatalError` as well.
            // So we need to catch the duplicate, uncatchable fatal error in `allReady` to prevent a `UnhandledPromiseRejection`.
            let _: Promise<()> = all_ready.catch(|_| {});
            reject(error);
        };
        let on_all_ready = on_all_ready.unwrap();
        request.set(Some(resume_request(
            children,
            postponed_state,
            &resume_render_state(
                postponed_state.resumable_state(),
                options.and_then(|options| options.nonce),
            ),
            options.and_then(|options| options.on_error),
            options.and_then(|options| options.on_browser_bailout),
            Some(&|| on_all_ready(())),
            Some(&on_shell_ready),
            Some(&on_shell_error),
            on_fatal_error,
        )));
        let request = request.get().unwrap();
        if let Some(signal) = options.and_then(|options| options.signal) {
            attach_abort_signal(request, signal);
        }
        start_work(request);
    })
}
