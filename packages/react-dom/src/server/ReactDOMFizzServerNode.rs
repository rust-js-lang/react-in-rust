// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, OnceCell};
use std::rc::Rc;

use js::{JsError, Kind, Promise, Uint8Array, Unknown, promise};
use node::event;
use node::events::EventEmitterExt;
use node::stream::Writable;
use react_dom_bindings::react_fizz_config_dom::{
    BootstrapScript, HeadersDescriptor, Nonce, OnHeaders, ResumableState, create_render_state,
    create_resumable_state, create_root_format_context, resume_render_state,
};
use react_server::react_fizz_current_task::{
    OnBrowserBailout, OnError, PostponedState, Request, abort, attach_abort_signal, create_request,
    prepare_for_start_flowing_if_before_all_ready, resume_request, start_flowing, start_work,
    stop_flowing,
};
use react_server::react_server_stream_config_node::TEXT_ENCODER;
use webapi::{
    AbortSignal, Headers, QueuingStrategy, ReadableByteStreamController, ReadableStream,
    UnderlyingByteSource, headers, readable_stream,
};

use crate::ensure_correct_isomorphic_react_version::ensure_correct_isomorphic_react_version;

#[cfg_attr(rust_js, rust_js::name = "version")]
pub use shared::react_version::REACT_VERSION as VERSION;

js::on_load! {
    ensure_correct_isomorphic_react_version();
}

fn create_drain_handler(
    destination: &'static Writable,
    request: &'static Request,
) -> Box<dyn Fn()> {
    Box::new(move || start_flowing(request, destination))
}

fn create_cancel_handler(request: &'static Request, reason: &'static str) -> Box<dyn Fn()> {
    Box::new(move || {
        stop_flowing(request);
        // eslint-disable-next-line react-internal/prod-error-codes
        abort(request, Some(js::unknown(js::error::new(reason, None))));
    })
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
    pub on_shell_ready: Option<&'static dyn Fn()>,
    pub on_shell_error: Option<&'static dyn Fn(Option<&'static Unknown>)>,
    pub on_all_ready: Option<&'static dyn Fn()>,
    pub on_error: Option<&'static OnError>,
    pub on_browser_bailout: Option<&'static OnBrowserBailout>,
    #[cfg_attr(rust_js, rust_js::name = "unstable_externalRuntimeSrc")]
    pub unstable_external_runtime_src: Option<BootstrapScript<'static>>,
    pub import_map: Option<&'static Unknown>,
    pub form_state: Option<&'static Unknown>,
    pub on_headers: Option<Rc<OnHeaders>>,
    pub max_headers_length: Option<&'static Unknown>,
}

pub struct ResumeOptions {
    pub nonce: Option<Nonce<'static>>,
    pub on_shell_ready: Option<&'static dyn Fn()>,
    pub on_shell_error: Option<&'static dyn Fn(Option<&'static Unknown>)>,
    pub on_all_ready: Option<&'static dyn Fn()>,
    pub on_error: Option<&'static OnError>,
    pub on_browser_bailout: Option<&'static OnBrowserBailout>,
}

pub struct PipeableStream {
    pub pipe: Box<dyn Fn(&'static Writable) -> &'static Writable>,
    // Cancel any pending I/O and put anything remaining into
    // client rendered mode.
    pub abort: Box<dyn Fn(Option<&'static Unknown>)>,
}

fn create_request_impl(
    children: Option<&'static Unknown>,
    options: Option<&'static Options>,
) -> &'static Request {
    let resumable_state = create_resumable_state(
        options.and_then(|options| options.identifier_prefix),
        options.and_then(|options| options.unstable_external_runtime_src),
        options.and_then(|options| options.bootstrap_script_content),
        options.and_then(|options| options.bootstrap_scripts),
        options.and_then(|options| options.bootstrap_modules),
    );
    create_request(
        children,
        &resumable_state,
        &create_render_state(
            &resumable_state,
            options.and_then(|options| options.nonce),
            options.and_then(|options| options.unstable_external_runtime_src),
            options.and_then(|options| options.import_map),
            options.and_then(|options| options.on_headers.clone()),
            options.and_then(|options| options.max_headers_length),
        ),
        &create_root_format_context(options.and_then(|options| options.namespace_uri)),
        options.and_then(|options| options.progressive_chunk_size),
        options.and_then(|options| options.on_error),
        options.and_then(|options| options.on_browser_bailout),
        options.and_then(|options| options.on_all_ready),
        options.and_then(|options| options.on_shell_ready),
        options.and_then(|options| options.on_shell_error),
        None,
        options.and_then(|options| options.form_state),
    )
}

pub fn render_to_pipeable_stream(
    children: Option<&'static Unknown>,
    options: Option<&'static Options>,
) -> PipeableStream {
    let request = create_request_impl(children, options);
    let has_started_flowing = Cell::new(false);
    start_work(request);
    PipeableStream {
        pipe: Box::new(move |destination| {
            if has_started_flowing.get() {
                panic!("React currently only supports piping to one writable stream.");
            }
            has_started_flowing.set(true);
            prepare_for_start_flowing_if_before_all_ready(request);
            start_flowing(request, destination);
            destination.on(event::Drain, create_drain_handler(destination, request));
            let on_error = create_cancel_handler(
                request,
                "The destination stream errored while writing data.",
            );
            destination.on(event::Error, Box::new(move |_: &JsError| on_error()));
            destination.on(
                event::Close,
                create_cancel_handler(request, "The destination stream closed early."),
            );
            destination
        }),
        abort: Box::new(move |reason| {
            abort(request, reason);
        }),
    }
}

/// What a web stream's controller is written to as a Writable is: the host
/// config writes to a Writable, so its chunks are pushed into the Readable.
pub struct FakeWritable {
    pub write: Box<dyn Fn(Option<&'static Unknown>) -> bool>,
    pub end: Box<dyn Fn()>,
    pub destroy: Box<dyn Fn(Option<&'static Unknown>)>,
}

fn create_fake_writable_from_readable_stream_controller(
    controller: &'static ReadableByteStreamController,
) -> FakeWritable {
    // The current host config expects a Writable so we create
    // a fake writable for now to push into the Readable.
    FakeWritable {
        write: Box::new(move |chunk| {
            let chunk: &'static Uint8Array = match js::classify(chunk) {
                Kind::String(chunk) => {
                    TEXT_ENCODER.with(|text_encoder| text_encoder.encode(Some(chunk)))
                }
                _ => unsafe { js::cast(chunk) },
            };
            controller.enqueue(chunk);
            // in web streams there is no backpressure so we can alwas write more
            true
        }),
        end: Box::new(move || {
            controller.close();
        }),
        destroy: Box::new(move |error| {
            // $FlowFixMe[method-unbinding]
            if js::is_function((controller, ReadableByteStreamController::error)) {
                // $FlowFixMe[incompatible-call]: This is an Error object or the destination accepts other types.
                controller.error(error.map(|error| error as &dyn js::IntoUnknown));
            } else {
                controller.close();
            }
        }),
    }
}

// TODO: Move to sub-classing ReadableStream.
pub type ReactDOMServerReadableStream = ReadableStream;

pub struct WebStreamsOptions {
    pub identifier_prefix: Option<&'static str>,
    #[cfg_attr(rust_js, rust_js::name = "namespaceURI")]
    pub namespace_uri: Option<&'static str>,
    pub nonce: Option<Nonce<'static>>,
    pub bootstrap_script_content: Option<&'static str>,
    pub bootstrap_scripts: Option<&'static [BootstrapScript<'static>]>,
    pub bootstrap_modules: Option<&'static [BootstrapScript<'static>]>,
    pub progressive_chunk_size: Option<f64>,
    pub on_error: Option<&'static OnError>,
    pub on_browser_bailout: Option<&'static OnBrowserBailout>,
    #[cfg_attr(rust_js, rust_js::name = "unstable_externalRuntimeSrc")]
    pub unstable_external_runtime_src: Option<BootstrapScript<'static>>,
    pub import_map: Option<&'static Unknown>,
    pub form_state: Option<&'static Unknown>,
    pub max_headers_length: Option<&'static Unknown>,
    pub signal: Option<&'static AbortSignal>,
    pub on_headers: Option<&'static dyn Fn(&'static Headers)>,
}

pub fn render_to_readable_stream(
    children: Option<&'static Unknown>,
    options: Option<&'static WebStreamsOptions>,
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
            let writable: OnceCell<FakeWritable> = OnceCell::new();
            let stream: &'static ReactDOMServerReadableStream = readable_stream::new(
                Some(&UnderlyingByteSource {
                    r#type: "bytes",
                    auto_allocate_chunk_size: None,
                    start: Some(&|controller: &'static ReadableByteStreamController| {
                        let _ = writable.set(create_fake_writable_from_readable_stream_controller(
                            controller,
                        ));
                    }),
                    pull: Some(&|_: &'static ReadableByteStreamController| {
                        start_flowing(request.get().unwrap(), writable.get().unwrap());
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

        let resumable_state = create_resumable_state(
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

fn resume_request_impl(
    children: Option<&'static Unknown>,
    postponed_state: &'static PostponedState<ResumableState>,
    options: Option<&'static ResumeOptions>,
) -> &'static Request {
    resume_request(
        children,
        postponed_state,
        &resume_render_state(
            postponed_state.resumable_state(),
            options.and_then(|options| options.nonce),
        ),
        options.and_then(|options| options.on_error),
        options.and_then(|options| options.on_browser_bailout),
        options.and_then(|options| options.on_all_ready),
        options.and_then(|options| options.on_shell_ready),
        options.and_then(|options| options.on_shell_error),
        None,
    )
}

pub fn resume_to_pipeable_stream(
    children: Option<&'static Unknown>,
    postponed_state: &'static PostponedState<ResumableState>,
    options: Option<&'static ResumeOptions>,
) -> PipeableStream {
    let request = resume_request_impl(children, postponed_state, options);
    let has_started_flowing = Cell::new(false);
    start_work(request);
    PipeableStream {
        pipe: Box::new(move |destination| {
            if has_started_flowing.get() {
                panic!("React currently only supports piping to one writable stream.");
            }
            has_started_flowing.set(true);
            start_flowing(request, destination);
            destination.on(event::Drain, create_drain_handler(destination, request));
            let on_error = create_cancel_handler(
                request,
                "The destination stream errored while writing data.",
            );
            destination.on(event::Error, Box::new(move |_: &JsError| on_error()));
            destination.on(
                event::Close,
                create_cancel_handler(request, "The destination stream closed early."),
            );
            destination
        }),
        abort: Box::new(move |reason| {
            abort(request, reason);
        }),
    }
}

pub struct WebStreamsResumeOptions {
    pub nonce: Option<Nonce<'static>>,
    pub on_error: Option<&'static OnError>,
    pub on_browser_bailout: Option<&'static OnBrowserBailout>,
    pub signal: Option<&'static AbortSignal>,
}

pub fn resume(
    children: Option<&'static Unknown>,
    postponed_state: &'static PostponedState<ResumableState>,
    options: Option<&'static WebStreamsResumeOptions>,
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
            let writable: OnceCell<FakeWritable> = OnceCell::new();
            let stream: &'static ReactDOMServerReadableStream = readable_stream::new(
                Some(&UnderlyingByteSource {
                    r#type: "bytes",
                    auto_allocate_chunk_size: None,
                    start: Some(&|controller: &'static ReadableByteStreamController| {
                        let _ = writable.set(create_fake_writable_from_readable_stream_controller(
                            controller,
                        ));
                    }),
                    pull: Some(&|_: &'static ReadableByteStreamController| {
                        start_flowing(request.get().unwrap(), writable.get().unwrap());
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
