// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js::{AsyncIterator, IteratorResult, JsError, Kind, Promise, Unknown, function, promise};
use react_client::react_flight_client_stream_config_web::{
    StringDecoder, create_string_decoder, read_final_string_chunk, read_partial_string_chunk,
};
use react_server::react_flight_hooks::Request;
use react_server::react_flight_reply_server::{
    ReactPromise, Response, ServerManifest, close, create_response, get_root, report_global_error,
    resolve_field, resolve_file,
};
use shared::environment::__DEV__;
use shared::noop::noop;
use shared::react_symbols::ASYNC_ITERATOR;
use webapi::{
    AbortSignal, FormData, FormDataEntryValue, QueuingStrategy, ReadableByteStreamController,
    ReadableStream, ReadableStreamDefaultReader, ReadableStreamReadResult, UnderlyingByteSource,
    WritableStream, form_data, readable_stream,
};

use super::react_flight_server::{
    EnvironmentName, FilterStackFrame, OnError, ReactClientValue, abort, attach_abort_signal,
    close_debug_channel, create_prerender_request, create_request, resolve_debug_message,
    start_flowing, start_flowing_debug, start_work, stop_flowing,
};
use super::react_flight_server_config_webpack_bundler::ClientManifest;

// GAP rd586: a re-export of a binding is named as its Rust item, `export
// {decodeAction as decode_action}`, but for its `rust_js::name`.
#[cfg_attr(rust_js, rust_js::name = "decodeAction")]
pub use super::react_flight_action_server::decode_action;
#[cfg_attr(rust_js, rust_js::name = "decodeFormState")]
pub use super::react_flight_action_server::decode_form_state;

#[cfg_attr(rust_js, rust_js::name = "createClientModuleProxy")]
pub use crate::react_flight_webpack_references::create_client_module_proxy;
#[cfg_attr(rust_js, rust_js::name = "registerClientReference")]
pub use crate::react_flight_webpack_references::register_client_reference;
#[cfg_attr(rust_js, rust_js::name = "registerServerReference")]
pub use crate::react_flight_webpack_references::register_server_reference;

pub use react_server::react_flight_server_temporary_references::create_temporary_reference_set;

// GAP rd587: a re-exported type writes no `export type {..}` in the `.d.ts`.
pub use react_server::react_flight_server_temporary_references::TemporaryReferenceSet;

pub struct DebugChannel {
    pub readable: Option<&'static ReadableStream>,
    pub writable: Option<&'static WritableStream>,
}

pub struct Options {
    pub debug_channel: Option<&'static DebugChannel>,
    pub environment_name: Option<EnvironmentName>,
    pub filter_stack_frame: Option<&'static FilterStackFrame>,
    pub identifier_prefix: Option<&'static str>,
    pub signal: Option<&'static AbortSignal>,
    pub temporary_references: Option<&'static TemporaryReferenceSet>,
    pub on_error: Option<&'static OnError>,
    pub start_time: Option<f64>,
}

/// startReadingFromDebugChannelReadableStream's scope: its reader, and the
/// text read but not yet split into messages.
struct DebugChannelReading {
    request: &'static Request,
    reader: &'static ReadableStreamDefaultReader,
    string_decoder: &'static StringDecoder,
    string_buffer: RefCell<String>,
}

impl DebugChannelReading {
    fn progress(
        self: &Rc<Self>,
        ReadableStreamReadResult { done, value }: ReadableStreamReadResult,
    ) {
        // GAP rd583: a read's `done` and `value` are `Option`s, `done ??= false`
        // and `$someValue(value)`, where upstream reads them as given.
        let done = done.unwrap_or(false);
        // GAP rd582: the conditional's temporary is named after the else
        // branch's callee, whose import is renamed for it.
        *self.string_buffer.borrow_mut() += &if done {
            read_final_string_chunk(self.string_decoder, js::uint8_array::new(0.0))
        } else {
            // SAFETY: a byte stream's chunk is a `Uint8Array`.
            read_partial_string_chunk(self.string_decoder, unsafe { js::cast(value.flatten()) })
        };
        let mut messages: Vec<String> = self
            .string_buffer
            .borrow()
            .split('\n')
            .map(str::to_owned)
            .collect();
        let last = messages.pop().unwrap_or_default();
        for message in &messages {
            resolve_debug_message(self.request, message);
        }
        *self.string_buffer.borrow_mut() = last;
        if done {
            close_debug_channel(self.request);
            return;
        }
        let reading = Rc::clone(self);
        let erroring = Rc::clone(self);
        // GAP rd581: upstream gives back this read's promise, `void |
        // Promise<void>`, which no `then` callback's type can give.
        let _: Promise<()> = self
            .reader
            .read()
            .then(move |entry| reading.progress(entry))
            .catch(move |e| erroring.error(e));
    }

    fn error(&self, e: &'static JsError) {
        abort(
            self.request,
            Some(js::unknown(js::error::new(
                Some("Lost connection to the Debug Channel."),
                Some(&js::ErrorOptions { cause: Some(e) }),
            ))),
        );
    }
}

fn start_reading_from_debug_channel_readable_stream(
    request: &'static Request,
    stream: &ReadableStream,
) {
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

pub fn render_to_readable_stream(
    model: Option<&'static ReactClientValue>,
    webpack_map: &'static ClientManifest,
    options: Option<&'static Options>,
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
        webpack_map,
        options.and_then(|options| options.on_error),
        options.and_then(|options| options.identifier_prefix),
        options.and_then(|options| options.temporary_references),
        options.and_then(|options| options.start_time),
        if __DEV__ {
            options.and_then(|options| options.environment_name)
        } else {
            None
        },
        if __DEV__ {
            options.and_then(|options| options.filter_stack_frame)
        } else {
            None
        },
        debug_channel_readable.is_some(),
    );
    if let Some(signal) = options.and_then(|options| options.signal) {
        attach_abort_signal(request, signal);
    }
    if let Some(debug_channel_writable) = debug_channel_writable {
        let debug_stream = readable_stream::new(
            Some(&UnderlyingByteSource {
                r#type: "bytes",
                auto_allocate_chunk_size: None,
                start: None,
                pull: Some(&move |controller: &'static ReadableByteStreamController| {
                    start_flowing_debug(request, controller);
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
        start_reading_from_debug_channel_readable_stream(request, debug_channel_readable);
    }
    readable_stream::new(
        Some(&UnderlyingByteSource {
            r#type: "bytes",
            auto_allocate_chunk_size: None,
            start: Some(&move |_controller| {
                start_work(request);
            }),
            pull: Some(&move |controller| {
                start_flowing(request, controller);
            }),
            cancel: Some(&move |reason| {
                stop_flowing(request);
                abort(request, reason);
            }),
        }),
        // $FlowFixMe[prop-missing] size() methods are not allowed on byte streams.
        // $FlowFixMe[incompatible-type]
        Some(QueuingStrategy {
            high_water_mark: Some(0.0),
        }),
    )
}

pub struct StaticResult {
    pub prelude: &'static ReadableStream,
}

pub fn prerender(
    model: Option<&'static ReactClientValue>,
    webpack_map: &'static ClientManifest,
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
            resolve(StaticResult { prelude: stream });
        };
        request.set(Some(create_prerender_request(
            model,
            webpack_map,
            &on_all_ready,
            on_fatal_error,
            options.and_then(|options| options.on_error),
            options.and_then(|options| options.identifier_prefix),
            options.and_then(|options| options.temporary_references),
            options.and_then(|options| options.start_time),
            if __DEV__ {
                options.and_then(|options| options.environment_name)
            } else {
                None
            },
            if __DEV__ {
                options.and_then(|options| options.filter_stack_frame)
            } else {
                None
            },
            false,
        )));
        let request = request.get().unwrap();
        if let Some(signal) = options.and_then(|options| options.signal) {
            attach_abort_signal(request, signal);
        }
        start_work(request);
    })
}

/// What a reply is decoded from: its JSON, or its form's data.
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum ReplyBody {
    Str(&'static str),
    FormData(&'static FormData),
}

pub struct DecodeReplyOptions {
    pub temporary_references: Option<&'static TemporaryReferenceSet>,
    pub array_size_limit: Option<&'static Unknown>,
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
        if entry.done == Some(true) {
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
        // GAP rd584: an async iterator's `throw`, which js's `AsyncIterator`
        // doesn't bind.
        if let Kind::Function(throw) = js::classify(js::get(self.iterator, "throw")) {
            // The iterator protocol doesn't necessarily include this but a generator do.
            // SAFETY: an async iterator's `throw` gives a promise of its next step.
            let thrown: Promise<IteratorResult<ReplyEntry>> =
                unsafe { js::cast(function::call(throw, Some(self.iterator), (reason,))) };
            let _: Promise<()> = thrown.then_with(|_| noop(), |_| noop());
        }
    }
}

pub fn decode_reply_from_async_iterable(
    iterable: &'static Unknown,
    webpack_map: &'static ServerManifest,
    options: Option<&'static DecodeReplyOptions>,
) -> Rc<ReactPromise> {
    // GAP rd585: an async iterable, whose `[Symbol.asyncIterator]()` gives
    // its iterator, which js has no type of.
    // SAFETY: an async iterable's `[Symbol.asyncIterator]` gives its iterator.
    // SAFETY: an async iterable's `[Symbol.asyncIterator]` is a function.
    let get_async_iterator: &dyn Fn() -> Option<&'static Unknown> = unsafe {
        js::cast(ASYNC_ITERATOR.with(|&async_iterator| js::get(iterable, async_iterator)))
    };
    // SAFETY: what it gives is the iterable's iterator.
    let iterator: &'static AsyncIterator<ReplyEntry> =
        unsafe { js::cast(function::call(get_async_iterator, Some(iterable), ())) };

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
