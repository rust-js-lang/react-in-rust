// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, OnceCell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::{Rc, Weak};

use busboy::{Busboy, FileInfo, FileStream};
use js::{
    AsyncIterable, AsyncIterator, IteratorResult, JsError, Kind, Promise, Uint8Array, Unknown,
    promise,
};
use node::event;
use node::events::EventEmitterExt;
use node::stream::{Readable, ReadableOptions, Writable, readable};
use webapi::{
    AbortSignal, EventTargetExt, FormData, FormDataEntryValue, MessageEvent, QueuingStrategy,
    ReadableByteStreamController, ReadableStream, ReadableStreamDefaultReader,
    ReadableStreamReadResult, UnderlyingByteSource, WebSocket, WritableStream, events, form_data,
    readable_stream,
};

use shared::environment::__DEV__;
use shared::noop::noop;
use shared::react_types::ReactFormState;

use react_server::react_flight_server::{
    EnvironmentName, FilterStackFrame, OnError, OnFatalError, ReactClientValue, Request, abort,
    attach_abort_signal, close_debug_channel, create_prerender_request, create_request,
    resolve_debug_message, start_flowing, start_flowing_debug, start_work, stop_flowing,
};

use react_server::react_flight_reply_server::{
    FileHandle, ReactPromise, Response, ServerManifest, close, create_response, get_root,
    report_global_error, resolve_field, resolve_file, resolve_file_chunk, resolve_file_complete,
    resolve_file_info,
};

use react_server::react_flight_action_server;

pub fn decode_action(
    body: &'static FormData,
    server_manifest: &'static ServerManifest,
) -> Option<Promise<&'static dyn Fn() -> Option<&'static Unknown>>> {
    react_flight_action_server::decode_action(body, server_manifest)
}

pub fn decode_form_state(
    action_result: Option<&'static Unknown>,
    body: &'static FormData,
    server_manifest: &'static ServerManifest,
) -> Promise<Option<ReactFormState<Option<&'static Unknown>, Option<&'static Unknown>>>> {
    react_flight_action_server::decode_form_state(action_result, body, server_manifest)
}

pub use crate::react_flight_unbundled_references::{
    create_client_module_proxy, register_client_reference, register_server_reference,
};

use react_client::react_flight_client_stream_config_node::{
    StringDecoder, create_string_decoder, read_final_string_chunk, read_partial_string_chunk,
};

use react_server::react_server_stream_config_node::{Destination, TEXT_ENCODER};

use super::react_flight_server_config_unbundled_bundler::ClientManifest;

pub use react_server::react_flight_server_temporary_references::create_temporary_reference_set;

// GAP rd587: a re-exported type writes no `export type {..}` in the `.d.ts`.
pub use react_server::react_flight_server_temporary_references::TemporaryReferenceSet;

fn create_drain_handler(destination: &'static Destination, request: Rc<Request>) -> Box<dyn Fn()> {
    Box::new(move || start_flowing(&request, js::unknown(destination)))
}

fn create_cancel_handler(request: Rc<Request>, reason: &'static str) -> Box<dyn Fn()> {
    Box::new(move || {
        stop_flowing(&request);
        abort(
            &request,
            Some(js::unknown(js::error::new(Some(reason), None))),
        );
    })
}

/// startReadingFromDebugChannelReadable's scope: the text read but not yet
/// split into messages, and whether its last bytes were a partial chunk.
struct DebugChannelReadableReading {
    request: Rc<Request>,
    string_decoder: &'static StringDecoder,
    last_was_partial: Cell<bool>,
    string_buffer: RefCell<String>,
}

impl DebugChannelReadableReading {
    fn on_data(&self, chunk: Option<&Unknown>) {
        if let Kind::String(chunk) = js::classify(chunk) {
            if self.last_was_partial.get() {
                self.string_buffer
                    .borrow_mut()
                    .push_str(&read_final_string_chunk(
                        self.string_decoder,
                        js::uint8_array::new(0.0),
                    ));
                self.last_was_partial.set(false);
            }
            self.string_buffer.borrow_mut().push_str(chunk);
        } else {
            // SAFETY: a chunk that isn't text is a `Uint8Array`.
            let buffer: &Uint8Array = unsafe { js::cast(chunk) };
            self.string_buffer
                .borrow_mut()
                .push_str(&read_partial_string_chunk(self.string_decoder, buffer));
            self.last_was_partial.set(true);
        }
        let mut messages: Vec<String> = self
            .string_buffer
            .borrow()
            .split('\n')
            .map(str::to_owned)
            .collect();
        let last = messages.pop().unwrap_or_default();
        for message in &messages {
            resolve_debug_message(&self.request, message);
        }
        *self.string_buffer.borrow_mut() = last;
    }

    fn on_error(&self, error: Option<&'static Unknown>) {
        abort(
            &self.request,
            Some(js::unknown(js::error::new(
                Some("Lost connection to the Debug Channel."),
                Some(&js::ErrorOptions {
                    cause: Some(&error),
                }),
            ))),
        );
    }

    fn on_close(&self) {
        close_debug_channel(&self.request);
    }
}

/// A debug channel, upstream's `Readable | WebSocket` to read or `Readable
/// | Writable | Duplex | WebSocket` given: any value, told apart by what
/// it has, as upstream tells them.
pub type DebugChannel = Unknown;

fn start_reading_from_debug_channel_readable(request: Rc<Request>, stream: &'static DebugChannel) {
    let reading = Rc::new(DebugChannelReadableReading {
        request,
        string_decoder: create_string_decoder(None),
        last_was_partial: Cell::new(false),
        string_buffer: RefCell::new(String::new()),
    });
    if
    // $FlowFixMe[method-unbinding]
    js::type_of(js::get(stream, "addEventListener")) == "function"
        // $FlowFixMe[method-unbinding]
        && js::type_of(js::get(stream, "binaryType")) == "string"
    {
        // SAFETY: what has these is a WebSocket.
        let ws: &WebSocket = unsafe { js::cast(Some(stream)) };
        ws.set_binary_type("arraybuffer");
        let data = Rc::clone(&reading);
        ws.add_event_listener(
            events::Message,
            move |event: &MessageEvent| {
                // $FlowFixMe[incompatible-type]
                data.on_data(event.data());
            },
            None,
        );
        let error = Rc::clone(&reading);
        ws.add_event_listener(
            events::Error,
            move |event| {
                // $FlowFixMe[prop-missing]
                error.on_error(js::get(event, "error"));
            },
            None,
        );
        let close = Rc::clone(&reading);
        ws.add_event_listener(events::Close, move |_| close.on_close(), None);
    } else {
        // SAFETY: what isn't a WebSocket is a Readable.
        let readable: &Readable = unsafe { js::cast(Some(stream)) };
        // GAP rd591: a boxed closure that only forwards to a function is not
        // that function, `readable.on('data', chunk => onData(chunk))` where
        // upstream passes `onData` (and `ws.addEventListener('close', () =>
        // onClose())`).
        let data = Rc::clone(&reading);
        readable.on(event::Data, Box::new(move |chunk| data.on_data(chunk)));
        let error = Rc::clone(&reading);
        readable.on(
            event::Error,
            Box::new(move |e: &JsError| error.on_error(e.as_unknown())),
        );
        let close = Rc::clone(&reading);
        readable.on(event::End, Box::new(move || close.on_close()));
    }
}

pub struct Options {
    pub debug_channel: Option<&'static DebugChannel>,
    pub environment_name: Option<EnvironmentName>,
    pub filter_stack_frame: Option<Rc<FilterStackFrame>>,
    pub on_error: Option<Rc<OnError>>,
    pub identifier_prefix: Option<&'static str>,
    pub temporary_references: Option<&'static TemporaryReferenceSet>,
    pub start_time: Option<f64>,
}

pub struct PipeableStream {
    pub abort: Box<dyn Fn(Option<&'static Unknown>)>,
    pub pipe: Box<dyn Fn(&'static Destination) -> &'static Destination>,
}

pub fn render_to_pipeable_stream(
    model: Option<&'static ReactClientValue>,
    webpack_map: &'static ClientManifest,
    options: Option<&'static Options>,
) -> PipeableStream {
    let debug_channel = if __DEV__ {
        options.and_then(|options| options.debug_channel)
    } else {
        None
    };
    let debug_channel_readable: Option<&'static DebugChannel> = if __DEV__
        && let Some(debug_channel) = debug_channel
        // $FlowFixMe[method-unbinding]
        && (js::type_of(js::get(debug_channel, "read")) == "function"
            || js::type_of(js::get(debug_channel, "readyState")) == "number")
    {
        Some(debug_channel)
    } else {
        None
    };
    let debug_channel_writable: Option<&'static Writable> =
        if __DEV__ && let Some(debug_channel) = debug_channel {
            // $FlowFixMe[method-unbinding]
            if js::type_of(js::get(debug_channel, "write")) == "function" {
                // SAFETY: what has a `write` is a Writable.
                Some(unsafe { js::cast(Some(debug_channel)) })
            // $FlowFixMe[method-unbinding]
            } else if js::type_of(js::get(debug_channel, "send")) == "function" {
                // SAFETY: what has a `send` is a WebSocket.
                Some(create_fake_writable_from_web_socket(unsafe {
                    js::cast(Some(debug_channel))
                }))
            } else {
                None
            }
        } else {
            None
        };
    let request = create_request(
        model,
        Some(js::unknown(webpack_map)),
        options.and_then(|options| options.on_error.clone()),
        options.and_then(|options| options.identifier_prefix),
        options.and_then(|options| options.temporary_references),
        options.and_then(|options| options.start_time),
        if __DEV__ {
            options.and_then(|options| options.environment_name.clone())
        } else {
            None
        },
        if __DEV__ {
            options.and_then(|options| options.filter_stack_frame.clone())
        } else {
            None
        },
        debug_channel_readable.is_some(),
    );
    let has_started_flowing = Cell::new(false);
    start_work(&request);
    if let Some(debug_channel_writable) = debug_channel_writable {
        start_flowing_debug(&request, js::unknown(debug_channel_writable));
    }
    if let Some(debug_channel_readable) = debug_channel_readable {
        start_reading_from_debug_channel_readable(Rc::clone(&request), debug_channel_readable);
    }
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
                // We don't close until the debug channel closes.
                if !__DEV__ || debug_channel_readable.is_none() {
                    destination.on(
                        event::Close,
                        create_cancel_handler(
                            Rc::clone(&request),
                            "The destination stream closed early.",
                        ),
                    );
                }
                destination
            })
        },
        abort: Box::new(move |reason| {
            abort(&request, reason);
        }),
    }
}

/// What the host config writes to where it expects a Writable: a Writable's
/// methods, each of what it's given.
struct FakeWritable {
    write: Box<dyn Fn(&'static Unknown) -> bool>,
    end: Box<dyn Fn()>,
    destroy: Box<dyn Fn(Option<&'static Unknown>)>,
}

unsafe impl js::Defined for FakeWritable {}

fn create_fake_writable_from_web_socket(web_socket: &'static WebSocket) -> &'static Writable {
    // SAFETY: what the host config calls of a Writable, its `write`, `end` and
    // `destroy`, as upstream's `as any`.
    unsafe {
        js::cast(Some(js::unknown_of(FakeWritable {
            write: Box::new(move |chunk| {
                // SAFETY: a chunk is text or bytes, as upstream's `chunk as any`.
                web_socket.send(js::cast::<&Uint8Array>(Some(chunk)));
                true
            }),
            end: Box::new(move || {
                web_socket.close(None, None);
            }),
            destroy: Box::new(move |reason| {
                let reason = match js::classify(reason) {
                    Kind::Object(_) | Kind::Array(_) => js::get(reason, "message"),
                    _ => reason,
                };
                if let Kind::String(reason) = js::classify(reason) {
                    web_socket.close(Some(1011.0), Some(reason));
                } else {
                    web_socket.close(Some(1011.0), None);
                }
            }),
        })))
    }
}

fn create_fake_writable_from_readable_stream_controller(
    controller: &'static ReadableByteStreamController,
) -> &'static Writable {
    // The current host config expects a Writable so we create
    // a fake writable for now to push into the Readable.
    // SAFETY: what the host config calls of a Writable, as upstream's `as any`.
    unsafe {
        js::cast(Some(js::unknown_of(FakeWritable {
            write: Box::new(move |chunk| {
                let chunk: &'static Uint8Array = match js::classify(Some(chunk)) {
                    Kind::String(chunk) => {
                        TEXT_ENCODER.with(|text_encoder| text_encoder.encode(Some(chunk)))
                    }
                    // SAFETY: a chunk that isn't text is bytes.
                    _ => js::cast(Some(chunk)),
                };
                controller.enqueue(chunk);
                // in web streams there is no backpressure so we can always write more
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
        })))
    }
}

/// startReadingFromDebugChannelReadableStream's scope: its reader, and the
/// text read but not yet split into messages.
struct DebugChannelReading {
    request: Rc<Request>,
    reader: &'static ReadableStreamDefaultReader,
    string_decoder: &'static StringDecoder,
    string_buffer: RefCell<String>,
}

impl DebugChannelReading {
    fn progress(
        self: &Rc<Self>,
        ReadableStreamReadResult { done, value }: ReadableStreamReadResult,
    ) -> Option<Promise<()>> {
        // GAP rd582: the conditional's temporary is named after the else
        // branch's callee, whose import is renamed for it.
        self.string_buffer
            .borrow_mut()
            .push_str(&if js::truthy(done.as_ref()) {
                read_final_string_chunk(self.string_decoder, js::uint8_array::new(0.0))
            } else {
                // SAFETY: a byte stream's chunk is a `Uint8Array`.
                read_partial_string_chunk(self.string_decoder, unsafe { js::cast(value.flatten()) })
            });
        let mut messages: Vec<String> = self
            .string_buffer
            .borrow()
            .split('\n')
            .map(str::to_owned)
            .collect();
        let last = messages.pop().unwrap_or_default();
        for message in &messages {
            resolve_debug_message(&self.request, message);
        }
        *self.string_buffer.borrow_mut() = last;
        if js::truthy(done.as_ref()) {
            close_debug_channel(&self.request);
            return None;
        }
        let reading = Rc::clone(self);
        let erroring = Rc::clone(self);
        Some(
            self.reader
                .read()
                .then(move |entry| reading.progress(entry))
                .catch(move |e| erroring.error(e)),
        )
    }

    fn error(&self, e: &'static JsError) {
        abort(
            &self.request,
            Some(js::unknown(js::error::new(
                Some("Lost connection to the Debug Channel."),
                Some(&js::ErrorOptions { cause: Some(e) }),
            ))),
        );
    }
}

fn start_reading_from_debug_channel_readable_stream(request: Rc<Request>, stream: &ReadableStream) {
    let reading = Rc::new(DebugChannelReading {
        request,
        reader: stream.get_reader(),
        string_decoder: create_string_decoder(None),
        string_buffer: RefCell::new(String::new()),
    });
    let progressing = Rc::clone(&reading);
    let erroring = Rc::clone(&reading);
    let _: Promise<()> = reading
        .reader
        .read()
        .then(move |entry| progressing.progress(entry))
        .catch(move |e| erroring.error(e));
}

pub struct WebStreamsDebugChannel {
    pub readable: Option<&'static ReadableStream>,
    pub writable: Option<&'static WritableStream>,
}

pub struct WebStreamsOptions {
    pub debug_channel: Option<&'static WebStreamsDebugChannel>,
    pub environment_name: Option<EnvironmentName>,
    pub filter_stack_frame: Option<Rc<FilterStackFrame>>,
    pub on_error: Option<Rc<OnError>>,
    pub identifier_prefix: Option<&'static str>,
    pub temporary_references: Option<&'static TemporaryReferenceSet>,
    pub start_time: Option<f64>,
    pub signal: Option<&'static AbortSignal>,
}

pub fn render_to_readable_stream(
    model: Option<&'static ReactClientValue>,
    webpack_map: &'static ClientManifest,
    options: Option<&'static WebStreamsOptions>,
) -> &'static ReadableStream {
    let debug_channel_readable = if __DEV__ {
        options
            .and_then(|options| options.debug_channel)
            .and_then(|debug_channel| debug_channel.readable)
    } else {
        None
    };
    let debug_channel_writable = if __DEV__ {
        options
            .and_then(|options| options.debug_channel)
            .and_then(|debug_channel| debug_channel.writable)
    } else {
        None
    };
    let request = create_request(
        model,
        Some(js::unknown(webpack_map)),
        options.and_then(|options| options.on_error.clone()),
        options.and_then(|options| options.identifier_prefix),
        options.and_then(|options| options.temporary_references),
        options.and_then(|options| options.start_time),
        if __DEV__ {
            options.and_then(|options| options.environment_name.clone())
        } else {
            None
        },
        if __DEV__ {
            options.and_then(|options| options.filter_stack_frame.clone())
        } else {
            None
        },
        debug_channel_readable.is_some(),
    );
    if let Some(signal) = options.and_then(|options| options.signal) {
        attach_abort_signal(&request, signal);
    }
    if let Some(debug_channel_writable) = debug_channel_writable {
        let debug_writable: Cell<Option<&'static Writable>> = Cell::new(None);
        let debug_stream = readable_stream::new(
            Some(&UnderlyingByteSource {
                r#type: "bytes",
                auto_allocate_chunk_size: None,
                start: Some(&|controller: &'static ReadableByteStreamController| {
                    debug_writable.set(Some(create_fake_writable_from_readable_stream_controller(
                        controller,
                    )));
                }),
                pull: Some(&|_controller| {
                    start_flowing_debug(&request, js::unknown(debug_writable.get().unwrap()));
                }),
                cancel: None,
            }),
            // $FlowFixMe[prop-missing] size() methods are not allowed on byte streams.
            // $FlowFixMe[incompatible-type]
            Some(QueuingStrategy {
                high_water_mark: Some(0.0),
            }),
        );
        let _: Promise<()> = debug_stream.pipe_to(debug_channel_writable, None);
    }
    if let Some(debug_channel_readable) = debug_channel_readable {
        start_reading_from_debug_channel_readable_stream(
            Rc::clone(&request),
            debug_channel_readable,
        );
    }
    let writable: Cell<Option<&'static Writable>> = Cell::new(None);
    readable_stream::new(
        Some(&UnderlyingByteSource {
            r#type: "bytes",
            auto_allocate_chunk_size: None,
            start: Some(&|controller: &'static ReadableByteStreamController| {
                writable.set(Some(create_fake_writable_from_readable_stream_controller(
                    controller,
                )));
                start_work(&request);
            }),
            pull: Some(&|_controller| {
                start_flowing(&request, js::unknown(writable.get().unwrap()));
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
    )
}

fn create_fake_writable_from_node_readable(readable: &'static Readable) -> &'static Writable {
    // The current host config expects a Writable so we create
    // a fake writable for now to push into the Readable.
    // SAFETY: what the host config calls of a Writable, as upstream's `as any`.
    unsafe {
        js::cast(Some(js::unknown_of(FakeWritable {
            write: Box::new(move |chunk| readable.push(Some(chunk))),
            end: Box::new(move || {
                readable.push(None);
            }),
            destroy: Box::new(move |error| {
                readable.destroy(error.map(|error| error as &dyn js::IntoUnknown));
            }),
        })))
    }
}

pub struct PrerenderOptions {
    pub environment_name: Option<EnvironmentName>,
    pub filter_stack_frame: Option<Rc<FilterStackFrame>>,
    pub on_error: Option<Rc<OnError>>,
    pub identifier_prefix: Option<&'static str>,
    pub temporary_references: Option<&'static TemporaryReferenceSet>,
    pub signal: Option<&'static AbortSignal>,
    pub start_time: Option<f64>,
}

pub struct StaticResult {
    pub prelude: &'static Readable,
}

pub fn prerender_to_node_stream(
    model: Option<&'static ReactClientValue>,
    webpack_map: &'static ClientManifest,
    options: Option<&'static PrerenderOptions>,
) -> Promise<StaticResult> {
    promise::new(move |resolve, reject| {
        let on_fatal_error: Rc<OnFatalError> = Rc::new(reject);
        let request_cell: Rc<OnceCell<Weak<Request>>> = Rc::new(OnceCell::new());
        let on_all_ready = {
            let cell = Rc::clone(&request_cell);
            move || {
                let request = cell.get().and_then(Weak::upgrade).unwrap();
                let writable: Cell<Option<&'static Writable>> = Cell::new(None);
                let readable: &'static Readable = readable::new(Some(&ReadableOptions {
                    read: Some(&|_| {
                        start_flowing(&request, js::unknown(writable.get().unwrap()));
                    }),
                    high_water_mark: None,
                }));
                writable.set(Some(create_fake_writable_from_node_readable(readable)));
                resolve(StaticResult { prelude: readable });
            }
        };

        let request = create_prerender_request(
            model,
            Some(js::unknown(webpack_map)),
            Rc::new(on_all_ready),
            on_fatal_error,
            options.and_then(|options| options.on_error.clone()),
            options.and_then(|options| options.identifier_prefix),
            options.and_then(|options| options.temporary_references),
            options.and_then(|options| options.start_time),
            if __DEV__ {
                options.and_then(|options| options.environment_name.clone())
            } else {
                None
            },
            if __DEV__ {
                options.and_then(|options| options.filter_stack_frame.clone())
            } else {
                None
            },
            false,
        );
        let _ = request_cell.set(Rc::downgrade(&request));
        if let Some(signal) = options.and_then(|options| options.signal) {
            attach_abort_signal(&request, signal);
        }
        start_work(&request);
    })
}

pub struct WebStreamsStaticResult {
    pub prelude: &'static ReadableStream,
}

pub fn prerender(
    model: Option<&'static ReactClientValue>,
    webpack_map: &'static ClientManifest,
    options: Option<&'static PrerenderOptions>,
) -> Promise<WebStreamsStaticResult> {
    promise::new(move |resolve, reject| {
        let on_fatal_error: Rc<OnFatalError> = Rc::new(reject);
        let request_cell: Rc<OnceCell<Weak<Request>>> = Rc::new(OnceCell::new());
        let on_all_ready = {
            let cell = Rc::clone(&request_cell);
            move || {
                let request = cell.get().and_then(Weak::upgrade).unwrap();
                let writable: Cell<Option<&'static Writable>> = Cell::new(None);
                let stream = readable_stream::new(
                    Some(&UnderlyingByteSource {
                        r#type: "bytes",
                        auto_allocate_chunk_size: None,
                        start: Some(&|controller: &'static ReadableByteStreamController| {
                            writable.set(Some(
                                create_fake_writable_from_readable_stream_controller(controller),
                            ));
                        }),
                        pull: Some(&|_controller| {
                            start_flowing(&request, js::unknown(writable.get().unwrap()));
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
                resolve(WebStreamsStaticResult { prelude: stream });
            }
        };
        let request = create_prerender_request(
            model,
            Some(js::unknown(webpack_map)),
            Rc::new(on_all_ready),
            on_fatal_error,
            options.and_then(|options| options.on_error.clone()),
            options.and_then(|options| options.identifier_prefix),
            options.and_then(|options| options.temporary_references),
            options.and_then(|options| options.start_time),
            if __DEV__ {
                options.and_then(|options| options.environment_name.clone())
            } else {
                None
            },
            if __DEV__ {
                options.and_then(|options| options.filter_stack_frame.clone())
            } else {
                None
            },
            false,
        );
        let _ = request_cell.set(Rc::downgrade(&request));
        if let Some(signal) = options.and_then(|options| options.signal) {
            attach_abort_signal(&request, signal);
        }
        start_work(&request);
    })
}

struct PendingFile {
    name: String,
    file: Rc<RefCell<FileHandle>>,
    complete: Cell<bool>,
    // Lazily allocated when a text field arrives after this file's 'file'
    // event but before its (deferred) 'end' event. Stored as flat
    // [name1, value1, name2, value2, ...] pairs.
    queued_fields: RefCell<Option<Vec<String>>>,
    next: RefCell<Option<Rc<PendingFile>>>,
}

pub struct DecodeReplyOptions {
    pub temporary_references: Option<&'static TemporaryReferenceSet>,
    pub array_size_limit: Option<&'static Unknown>,
}

/// decodeReplyFromBusboy's scope: the response, and its pending files.
struct BusboyReading {
    busboy_stream: &'static Busboy,
    response: Rc<Response>,
    // Linked list of pending files in arrival (payload) order. Text fields that
    // arrive while a file is in flight are queued on the tail file's
    // `queuedFields` so they can be resolved together when that file completes.
    // Fields that arrive while the list is empty bypass it and resolve
    // immediately. This makes the backing FormData's insertion order match the
    // payload's entry order.
    head: RefCell<Option<Rc<PendingFile>>>,
    tail: RefCell<Option<Rc<PendingFile>>>,
    body_finished: Cell<bool>,
    closed: Cell<bool>,
}

// GAP rd590: a `RefCell` borrow held across another crate's call, the
// file's handle given to resolveFileComplete and resolveFileChunk, is
// counted, `$borrow(current.file, true)` in a `try`/`finally`, and so is
// each borrow of a cell of a struct that holds its own kind,
// `$fieldCell(current, 'next')`; the queued fields are copied out,
// `current.queuedFields.slice()`, so no borrow of them is held.
impl BusboyReading {
    fn flush(&self) {
        loop {
            let Some(current) = self.head.borrow().clone() else {
                break;
            };
            if !current.complete.get() {
                // This file is still streaming. Hold later files and fields until it
                // completes so the backing FormData reflects payload order.
                return;
            }
            if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
                resolve_file_complete(&self.response, &current.name, &current.file.borrow());
                let queued_fields = current.queued_fields.borrow().clone();
                if let Some(queued_fields) = queued_fields {
                    for i in (0..queued_fields.len()).step_by(2) {
                        resolve_field(&self.response, &queued_fields[i], &queued_fields[i + 1]);
                    }
                }
            })) {
                self.busboy_stream
                    .destroy(error.downcast_ref::<&'static JsError>().copied());
                return;
            }
            *self.head.borrow_mut() = current.next.borrow().clone();
        }
        *self.tail.borrow_mut() = None;
        if self.body_finished.get() && !self.closed.get() {
            self.closed.set(true);
            close(&self.response);
        }
    }
}

pub fn decode_reply_from_busboy(
    busboy_stream: &'static Busboy,
    webpack_map: &'static ServerManifest,
    options: Option<&'static DecodeReplyOptions>,
) -> Rc<ReactPromise> {
    let response = create_response(
        webpack_map,
        "",
        options.and_then(|options| options.temporary_references),
        None,
        options.and_then(|options| options.array_size_limit),
    );

    let reading = Rc::new(BusboyReading {
        busboy_stream,
        response,
        head: RefCell::new(None),
        tail: RefCell::new(None),
        body_finished: Cell::new(false),
        closed: Cell::new(false),
    });

    let field = Rc::clone(&reading);
    busboy_stream.on(
        busboy::event::Field,
        Box::new(move |name: &str, value: &str, _| {
            let tail = field.tail.borrow().clone();
            if let Some(tail) = tail {
                // A file is in flight; queue the field on the tail (most recent) pending
                // file so it resolves after that file, preserving payload order.
                tail.queued_fields
                    .borrow_mut()
                    .get_or_insert_with(Vec::new)
                    .extend([name.to_owned(), value.to_owned()]);
            } else if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
                resolve_field(&field.response, name, value);
            })) {
                busboy_stream.destroy(error.downcast_ref::<&'static JsError>().copied());
            }
        }),
    );
    let file = Rc::clone(&reading);
    busboy_stream.on(
        busboy::event::File,
        Box::new(
            move |name: &str,
                  value: &FileStream,
                  FileInfo {
                      filename,
                      encoding,
                      mime_type,
                  }: &FileInfo| {
                if encoding.to_lowercase() == "base64" {
                    busboy_stream.destroy(Some::<&JsError>(js::error::new(
                        Some(concat!(
                            "React doesn't accept base64 encoded file uploads because we don't expect ",
                            "form data passed from a browser to ever encode data that way. If that's ",
                            "the wrong assumption, we can easily fix it."
                        )),
                        None,
                    )));
                    return;
                }
                let file_handle = Rc::new(RefCell::new(resolve_file_info(
                    &file.response,
                    name,
                    filename.as_deref(),
                    mime_type,
                )));
                let pending_file = Rc::new(PendingFile {
                    name: name.to_owned(),
                    file: Rc::clone(&file_handle),
                    complete: Cell::new(false),
                    queued_fields: RefCell::new(None),
                    next: RefCell::new(None),
                });
                let tail = file.tail.borrow().clone();
                if let Some(tail) = tail {
                    *tail.next.borrow_mut() = Some(Rc::clone(&pending_file));
                } else {
                    *file.head.borrow_mut() = Some(Rc::clone(&pending_file));
                }
                *file.tail.borrow_mut() = Some(Rc::clone(&pending_file));
                let data = Rc::clone(&file);
                value.on(
                    event::Data,
                    Box::new(move |chunk| {
                        if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
                            resolve_file_chunk(
                                &data.response,
                                &mut file_handle.borrow_mut(),
                                // SAFETY: a file's chunk is bytes.
                                unsafe { js::cast(chunk) },
                            );
                        })) {
                            busboy_stream
                                .destroy(error.downcast_ref::<&'static JsError>().copied());
                        }
                    }),
                );
                value.on(
                    event::Error,
                    Box::new(move |error: &JsError| {
                        busboy_stream.destroy(Some(error));
                    }),
                );
                let end = Rc::clone(&file);
                value.on(
                    event::End,
                    Box::new(move || {
                        pending_file.complete.set(true);
                        end.flush();
                    }),
                );
            },
        ),
    );
    let finish = Rc::clone(&reading);
    busboy_stream.on(
        busboy::event::Finish,
        Box::new(move || {
            finish.body_finished.set(true);
            finish.flush();
            if !finish.closed.get() {
                // Invariant: busboy delays 'finish' until every file's 'end' event has
                // fired, so the flush above should always close the response.
                report_global_error(
                    &finish.response,
                    js::error::new(Some("Reply finished with incomplete file part."), None),
                );
            }
        }),
    );
    let error = Rc::clone(&reading);
    busboy_stream.on(
        busboy::event::Error,
        Box::new(move |err| {
            report_global_error(
                &error.response,
                // $FlowFixMe[incompatible-type] types Error and mixed are incompatible
                // SAFETY: what busboy errors with is an error.
                unsafe { js::cast(err) },
            );
        }),
    );
    get_root(&reading.response)
}

/// What a reply is decoded from: its JSON, or its form's data.
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum ReplyBody {
    Str(&'static str),
    FormData(&'static FormData),
}

pub fn decode_reply(
    body: ReplyBody,
    webpack_map: &'static ServerManifest,
    options: Option<&'static DecodeReplyOptions>,
) -> Rc<ReactPromise> {
    let body = match body {
        ReplyBody::Str(body) => {
            let form = form_data::new(None, None);
            form.append("0", body);
            form
        }
        ReplyBody::FormData(body) => body,
    };
    let response = create_response(
        webpack_map,
        "",
        options.and_then(|options| options.temporary_references),
        Some(js::unknown(body)),
        options.and_then(|options| options.array_size_limit),
    );
    let root = get_root(&response);
    close(&response);
    root
}

/// An entry of a reply's form: its name, and its text or its file.
pub type ReplyEntry = (String, FormDataEntryValue<'static>);

/// decodeReplyFromAsyncIterable's scope: the entries' iterator, and the
/// response they're resolved into.
struct AsyncIterableReading {
    iterator: &'static AsyncIterator<ReplyEntry>,
    response: Rc<Response>,
}

impl AsyncIterableReading {
    fn progress(self: &Rc<Self>, entry: IteratorResult<ReplyEntry>) {
        if js::truthy(entry.done.as_ref()) {
            close(&self.response);
        } else {
            let (name, value) = entry.value;
            match value {
                FormDataEntryValue::Str(value) => resolve_field(&self.response, &name, value),
                FormDataEntryValue::File(value) => resolve_file(&self.response, &name, value),
            }
            let progressing = Rc::clone(self);
            let erroring = Rc::clone(self);
            let _: Promise<()> = self.iterator.next().then_with(
                move |entry| progressing.progress(entry),
                move |reason| erroring.error(reason),
            );
        }
    }

    fn error(&self, reason: &'static JsError) {
        // SAFETY: what an iterator rejects with is an error.
        report_global_error(&self.response, unsafe { js::cast(reason.as_unknown()) });
        if js::is_function((self.iterator, AsyncIterator::<ReplyEntry>::throw)) {
            // The iterator protocol doesn't necessarily include this but a generator do.
            // $FlowFixMe[prop-missing] should be able to pass mixed
            let _: Promise<()> = self
                .iterator
                .throw(Some(reason))
                .then_with(|_| noop(), |_| noop());
        }
    }
}

pub fn decode_reply_from_async_iterable(
    iterable: &'static AsyncIterable<ReplyEntry>,
    webpack_map: &'static ServerManifest,
    options: Option<&'static DecodeReplyOptions>,
) -> Rc<ReactPromise> {
    let iterator: &'static AsyncIterator<ReplyEntry> = iterable.async_iterator();

    let response = create_response(
        webpack_map,
        "",
        options.and_then(|options| options.temporary_references),
        None,
        options.and_then(|options| options.array_size_limit),
    );

    let reading = Rc::new(AsyncIterableReading { iterator, response });
    let progressing = Rc::clone(&reading);
    let erroring = Rc::clone(&reading);
    let _: Promise<()> = reading.iterator.next().then_with(
        move |entry| progressing.progress(entry),
        move |reason| erroring.error(reason),
    );

    get_root(&reading.response)
}
