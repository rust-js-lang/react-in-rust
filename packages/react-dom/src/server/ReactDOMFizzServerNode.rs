// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, OnceCell};
use std::rc::{Rc, Weak};

use js::{JsError, Kind, Promise, Uint8Array, Unknown, promise};
use node::event;
use node::events::EventEmitterExt;
use shared::react_types::ReactNodeList;
use webapi::{
    AbortSignal, Headers, QueuingStrategy, ReadableByteStreamController, ReadableStream,
    UnderlyingByteSource, headers, readable_stream,
};

use react_server::react_fizz_server::{
    FormState, OnBrowserBailout, OnError, OnFatal, PostponedState, Request, abort,
    attach_abort_signal, create_request, prepare_for_start_flowing_if_before_all_ready,
    resume_request, start_flowing, start_work, stop_flowing,
};

use react_dom_bindings::react_fizz_config_dom::{
    BootstrapScript, HeadersDescriptor, Nonce, OnHeaders, create_render_state,
    create_resumable_state, create_root_format_context, resume_render_state,
};

use react_server::react_server_stream_config_node::{Destination, TEXT_ENCODER};

use crate::ensure_correct_isomorphic_react_version::ensure_correct_isomorphic_react_version;

#[cfg_attr(rust_js, rust_js::name = "version")]
pub use shared::react_version::REACT_VERSION as VERSION;

js::on_load! {
    ensure_correct_isomorphic_react_version();
}

fn create_drain_handler(destination: &'static Destination, request: Rc<Request>) -> Box<dyn Fn()> {
    Box::new(move || start_flowing(&request, js::unknown(destination)))
}

fn create_cancel_handler(request: Rc<Request>, reason: &'static str) -> Box<dyn Fn()> {
    Box::new(move || {
        stop_flowing(&request);
        // eslint-disable-next-line react-internal/prod-error-codes
        abort(
            &request,
            Some(js::unknown(js::error::new(Some(reason), None))),
        );
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
    pub on_shell_ready: Option<Rc<dyn Fn()>>,
    pub on_shell_error: Option<Rc<OnFatal>>,
    pub on_all_ready: Option<Rc<dyn Fn()>>,
    pub on_error: Option<Rc<OnError>>,
    pub on_browser_bailout: Option<Rc<OnBrowserBailout>>,
    #[cfg_attr(rust_js, rust_js::name = "unstable_externalRuntimeSrc")]
    pub unstable_external_runtime_src: Option<BootstrapScript<'static>>,
    pub import_map: Option<&'static Unknown>,
    pub form_state: Option<&'static FormState>,
    pub on_headers: Option<Rc<OnHeaders>>,
    pub max_headers_length: Option<&'static Unknown>,
}

pub struct ResumeOptions {
    pub nonce: Option<Nonce<'static>>,
    pub on_shell_ready: Option<Rc<dyn Fn()>>,
    pub on_shell_error: Option<Rc<OnFatal>>,
    pub on_all_ready: Option<Rc<dyn Fn()>>,
    pub on_error: Option<Rc<OnError>>,
    pub on_browser_bailout: Option<Rc<OnBrowserBailout>>,
}

pub struct PipeableStream {
    pub pipe: Box<dyn Fn(&'static Destination) -> &'static Destination>,
    // Cancel any pending I/O and put anything remaining into
    // client rendered mode.
    pub abort: Box<dyn Fn(Option<&'static Unknown>)>,
}

fn create_request_impl(
    children: Option<&'static ReactNodeList>,
    options: Option<&'static Options>,
) -> Rc<Request> {
    let resumable_state = Rc::new(create_resumable_state(
        options.and_then(|options| options.identifier_prefix),
        options.and_then(|options| options.unstable_external_runtime_src),
        options.and_then(|options| options.bootstrap_script_content),
        options.and_then(|options| options.bootstrap_scripts),
        options.and_then(|options| options.bootstrap_modules),
    ));
    create_request(
        children,
        js::unknown_of(Rc::clone(&resumable_state)),
        js::unknown_of(create_render_state(
            &resumable_state,
            options.and_then(|options| options.nonce),
            options.and_then(|options| options.unstable_external_runtime_src),
            options.and_then(|options| options.import_map),
            options.and_then(|options| options.on_headers.clone()),
            options.and_then(|options| options.max_headers_length),
        )),
        js::unknown_of(create_root_format_context(
            options.and_then(|options| options.namespace_uri),
        )),
        options.and_then(|options| options.progressive_chunk_size),
        options.and_then(|options| options.on_error.clone()),
        options.and_then(|options| options.on_browser_bailout.clone()),
        options.and_then(|options| options.on_all_ready.clone()),
        options.and_then(|options| options.on_shell_ready.clone()),
        options.and_then(|options| options.on_shell_error.clone()),
        None,
        options.and_then(|options| options.form_state),
    )
}

pub fn render_to_pipeable_stream(
    children: Option<&'static ReactNodeList>,
    options: Option<&'static Options>,
) -> PipeableStream {
    let request = create_request_impl(children, options);
    let has_started_flowing = Cell::new(false);
    start_work(&request);
    PipeableStream {
        pipe: {
            let request = Rc::clone(&request);
            Box::new(move |destination| {
                if has_started_flowing.get() {
                    panic!("React currently only supports piping to one writable stream.");
                }
                has_started_flowing.set(true);
                prepare_for_start_flowing_if_before_all_ready(&request);
                start_flowing(&request, js::unknown(destination));
                destination.on(
                    event::Drain,
                    create_drain_handler(destination, Rc::clone(&request)),
                );
                let on_error = create_cancel_handler(
                    Rc::clone(&request),
                    "The destination stream errored while writing data.",
                );
                destination.on(event::Error, Box::new(move |_: &JsError| on_error()));
                destination.on(
                    event::Close,
                    create_cancel_handler(
                        Rc::clone(&request),
                        "The destination stream closed early.",
                    ),
                );
                destination
            })
        },
        abort: Box::new(move |reason| {
            abort(&request, reason);
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

// Safety: a fake writable is a JS object of its three functions.
unsafe impl js::Defined for FakeWritable {}

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
    pub on_error: Option<Rc<OnError>>,
    pub on_browser_bailout: Option<Rc<OnBrowserBailout>>,
    #[cfg_attr(rust_js, rust_js::name = "unstable_externalRuntimeSrc")]
    pub unstable_external_runtime_src: Option<BootstrapScript<'static>>,
    pub import_map: Option<&'static Unknown>,
    pub form_state: Option<&'static FormState>,
    pub max_headers_length: Option<&'static Unknown>,
    pub signal: Option<&'static AbortSignal>,
    pub on_headers: Option<&'static dyn Fn(&'static Headers)>,
}

pub fn render_to_readable_stream(
    children: Option<&'static ReactNodeList>,
    options: Option<&'static WebStreamsOptions>,
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
                let writable: Cell<Option<&'static Unknown>> = Cell::new(None);
                let stream: &'static ReactDOMServerReadableStream = readable_stream::new(
                    Some(&UnderlyingByteSource {
                        r#type: "bytes",
                        auto_allocate_chunk_size: None,
                        start: Some(&|controller: &'static ReadableByteStreamController| {
                            writable.set(Some(js::unknown_of(
                                create_fake_writable_from_readable_stream_controller(controller),
                            )));
                        }),
                        pull: Some(&|_: &'static ReadableByteStreamController| {
                            start_flowing(&request, writable.get().unwrap());
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

fn resume_request_impl(
    children: Option<&'static ReactNodeList>,
    postponed_state: &'static PostponedState,
    options: Option<&'static ResumeOptions>,
) -> Rc<Request> {
    resume_request(
        children,
        postponed_state,
        js::unknown_of(resume_render_state(
            unsafe { js::cast(Some(postponed_state.resumable_state)) },
            options.and_then(|options| options.nonce),
        )),
        options.and_then(|options| options.on_error.clone()),
        options.and_then(|options| options.on_browser_bailout.clone()),
        options.and_then(|options| options.on_all_ready.clone()),
        options.and_then(|options| options.on_shell_ready.clone()),
        options.and_then(|options| options.on_shell_error.clone()),
        None,
    )
}

pub fn resume_to_pipeable_stream(
    children: Option<&'static ReactNodeList>,
    postponed_state: &'static PostponedState,
    options: Option<&'static ResumeOptions>,
) -> PipeableStream {
    let request = resume_request_impl(children, postponed_state, options);
    let has_started_flowing = Cell::new(false);
    start_work(&request);
    PipeableStream {
        pipe: {
            let request = Rc::clone(&request);
            Box::new(move |destination| {
                if has_started_flowing.get() {
                    panic!("React currently only supports piping to one writable stream.");
                }
                has_started_flowing.set(true);
                start_flowing(&request, js::unknown(destination));
                destination.on(
                    event::Drain,
                    create_drain_handler(destination, Rc::clone(&request)),
                );
                let on_error = create_cancel_handler(
                    Rc::clone(&request),
                    "The destination stream errored while writing data.",
                );
                destination.on(event::Error, Box::new(move |_: &JsError| on_error()));
                destination.on(
                    event::Close,
                    create_cancel_handler(
                        Rc::clone(&request),
                        "The destination stream closed early.",
                    ),
                );
                destination
            })
        },
        abort: Box::new(move |reason| {
            abort(&request, reason);
        }),
    }
}

pub struct WebStreamsResumeOptions {
    pub nonce: Option<Nonce<'static>>,
    pub on_error: Option<Rc<OnError>>,
    pub on_browser_bailout: Option<Rc<OnBrowserBailout>>,
    pub signal: Option<&'static AbortSignal>,
}

pub fn resume(
    children: Option<&'static ReactNodeList>,
    postponed_state: &'static PostponedState,
    options: Option<&'static WebStreamsResumeOptions>,
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
                let writable: Cell<Option<&'static Unknown>> = Cell::new(None);
                let stream: &'static ReactDOMServerReadableStream = readable_stream::new(
                    Some(&UnderlyingByteSource {
                        r#type: "bytes",
                        auto_allocate_chunk_size: None,
                        start: Some(&|controller: &'static ReadableByteStreamController| {
                            writable.set(Some(js::unknown_of(
                                create_fake_writable_from_readable_stream_controller(controller),
                            )));
                        }),
                        pull: Some(&|_: &'static ReadableByteStreamController| {
                            start_flowing(&request, writable.get().unwrap());
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
