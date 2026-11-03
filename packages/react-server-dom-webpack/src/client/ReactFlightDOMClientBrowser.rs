// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js::{ArrayBuffer, JsError, Promise, Uint8Array, Unknown};
use shared::environment::__DEV__;
use webapi::{
    AbortSignal, ReadableStream, ReadableStreamReadResult, Response, WritableStream, console,
    text_encoder,
};

use react_client::react_flight_client::{
    CallServerCallback, DebugChannel, DebugChannelCallback, FindSourceMapURLCallback, ReactPromise,
    WeakResponse, close, create_response, create_stream_state, get_root, inject_into_dev_tools,
    process_binary_chunk, process_string_chunk, report_global_error,
};
use react_client::react_flight_reply_client::{ReactServerValue, ReplyBody, process_reply};

pub use react_client::react_flight_reply_client::{
    create_server_reference, register_server_reference,
};
pub use react_client::react_flight_temporary_references::{
    TemporaryReferenceSet, create_temporary_reference_set,
};

pub struct DebugChannelOptions {
    pub writable: Option<&'static WritableStream>,
    pub readable: Option<&'static ReadableStream>,
}

pub struct Options {
    pub call_server: Option<&'static CallServerCallback>,
    pub debug_channel: Option<&'static DebugChannelOptions>,
    pub temporary_references: Option<TemporaryReferenceSet>,
    #[cfg_attr(rust_js, rust_js::name = "unstable_allowPartialStream")]
    pub unstable_allow_partial_stream: Option<bool>,
    #[cfg_attr(rust_js, rust_js::name = "findSourceMapURL")]
    pub find_source_map_url: Option<&'static FindSourceMapURLCallback>,
    pub replay_console_logs: Option<bool>,
    pub environment_name: Option<String>,
    pub start_time: Option<f64>,
    pub end_time: Option<f64>,
}

fn create_debug_callback_from_writable_stream(
    debug_writable: &'static WritableStream,
) -> Rc<DebugChannelCallback> {
    let text_encoder = text_encoder::new();
    let writer = debug_writable.get_writer();
    Rc::new(move |message: &str| {
        if message.is_empty() {
            let _ = writer.close();
        } else {
            // Note: It's important that this function doesn't close over the Response object or it can't be GC:ed.
            // Therefore, we can't report errors from this write back to the Response object.
            if __DEV__ {
                let _ = writer
                    .write(Some(
                        text_encoder.encode(Some(&(message.to_owned() + "\n"))),
                    ))
                    .catch(|error| console::error((error.as_unknown(),)));
            }
        }
    })
}

fn create_response_from_options(options: Option<&'static Options>) -> Rc<WeakResponse> {
    let debug_channel = if __DEV__
        && let Some(options) = options
        && let Some(debug_channel) = options.debug_channel
    {
        Some(Rc::new(DebugChannel {
            has_readable: debug_channel.readable.is_some(),
            callback: debug_channel
                .writable
                .map(create_debug_callback_from_writable_stream),
        }))
    } else {
        None
    };

    create_response(
        None,
        None,
        None,
        options.and_then(|options| options.call_server),
        None, // encodeFormAction
        None, // nonce
        options.and_then(|options| options.temporary_references.as_ref()),
        options
            .and_then(|options| options.unstable_allow_partial_stream)
            .unwrap_or(false),
        if __DEV__ {
            options.and_then(|options| options.find_source_map_url)
        } else {
            None
        },
        if __DEV__ {
            options.is_none_or(|options| options.replay_console_logs != Some(false))
        } else {
            false
        }, // defaults to true
        if __DEV__ {
            options
                .and_then(|options| options.environment_name.clone())
                .filter(|environment_name| !environment_name.is_empty())
        } else {
            None
        },
        if __DEV__ {
            options.and_then(|options| options.start_time)
        } else {
            None
        },
        if __DEV__ {
            options.and_then(|options| options.end_time)
        } else {
            None
        },
        debug_channel,
    )
}

type Progress = Rc<dyn Fn(ReadableStreamReadResult)>;

fn start_reading_from_universal_stream(
    response: Rc<WeakResponse>,
    stream: &'static ReadableStream,
    on_done: Rc<dyn Fn()>,
) {
    // This is the same as startReadingFromStream except this allows WebSocketStreams which
    // return ArrayBuffer and string chunks instead of Uint8Array chunks. We could potentially
    // always allow streams with variable chunk types.
    let stream_state = Rc::new(create_stream_state(&response, Some(js::unknown(stream))));
    let reader = stream.get_reader();
    let progress: Rc<RefCell<Option<Progress>>> = Rc::default();
    let error: Rc<dyn Fn(&'static JsError)> = {
        let response = Rc::clone(&response);
        Rc::new(move |e: &'static JsError| report_global_error(&response, e.as_unknown()))
    };
    let next = Rc::clone(&progress);
    let rejecting = Rc::clone(&error);
    *progress.borrow_mut() = Some(Rc::new(move |entry: ReadableStreamReadResult| {
        if entry.done.unwrap_or(false) {
            return on_done();
        }
        let value = entry.value.flatten();
        if js::instance_of(value, js::class::<ArrayBuffer>()) {
            // WebSockets can produce ArrayBuffer values in ReadableStreams.
            process_binary_chunk(
                &response,
                &stream_state,
                // SAFETY: an `ArrayBuffer`, as tested.
                js::uint8_array::new_with_buffer(unsafe { js::cast(value) }, None, None),
            );
        } else if let js::Kind::String(value) = js::classify(value) {
            // WebSockets can produce string values in ReadableStreams.
            process_string_chunk(&response, &stream_state, value);
        } else {
            // SAFETY: what else the stream gives is bytes, as upstream's `any`.
            process_binary_chunk(&response, &stream_state, unsafe { js::cast(value) });
        }
        let progress = next.borrow().clone().unwrap();
        let error = Rc::clone(&rejecting);
        let _: Promise<()> = reader
            .read()
            .then(move |entry| progress(entry))
            .catch(move |e| error(e));
    }));
    let progress = progress.borrow().clone().unwrap();
    let _: Promise<()> = reader
        .read()
        .then(move |entry| progress(entry))
        .catch(move |e| error(e));
}

fn start_reading_from_stream(
    response: Rc<WeakResponse>,
    stream: &'static ReadableStream,
    on_done: Rc<dyn Fn()>,
    debug_value: Option<&'static Unknown>,
) {
    let stream_state = Rc::new(create_stream_state(&response, debug_value));
    let reader = stream.get_reader();
    let progress: Rc<RefCell<Option<Progress>>> = Rc::default();
    let error: Rc<dyn Fn(&'static JsError)> = {
        let response = Rc::clone(&response);
        Rc::new(move |e: &'static JsError| report_global_error(&response, e.as_unknown()))
    };
    let next = Rc::clone(&progress);
    let rejecting = Rc::clone(&error);
    *progress.borrow_mut() = Some(Rc::new(move |entry: ReadableStreamReadResult| {
        if entry.done.unwrap_or(false) {
            return on_done();
        }
        // SAFETY: the stream gives bytes, as upstream's `value as any`.
        let buffer: &'static Uint8Array = unsafe { js::cast(entry.value.flatten()) };
        process_binary_chunk(&response, &stream_state, buffer);
        let progress = next.borrow().clone().unwrap();
        let error = Rc::clone(&rejecting);
        let _: Promise<()> = reader
            .read()
            .then(move |entry| progress(entry))
            .catch(move |e| error(e));
    }));
    let progress = progress.borrow().clone().unwrap();
    let _: Promise<()> = reader
        .read()
        .then(move |entry| progress(entry))
        .catch(move |e| error(e));
}

pub fn create_from_readable_stream(
    stream: &'static ReadableStream,
    options: Option<&'static Options>,
) -> Rc<ReactPromise> {
    let response = create_response_from_options(options);

    if __DEV__
        && let Some(readable) = options
            .and_then(|options| options.debug_channel)
            .and_then(|debug_channel| debug_channel.readable)
    {
        let stream_done_count = Rc::new(Cell::new(0.0));
        let handle_done: Rc<dyn Fn()> = {
            let response = Rc::clone(&response);
            Rc::new(move || {
                stream_done_count.set(stream_done_count.get() + 1.0);
                if stream_done_count.get() == 2.0 {
                    close(&response);
                }
            })
        };
        start_reading_from_universal_stream(
            Rc::clone(&response),
            readable,
            Rc::clone(&handle_done),
        );
        start_reading_from_stream(
            Rc::clone(&response),
            stream,
            handle_done,
            Some(js::unknown(stream)),
        );
    } else {
        let closing = Rc::clone(&response);
        start_reading_from_stream(
            Rc::clone(&response),
            stream,
            Rc::new(move || close(&closing)),
            Some(js::unknown(stream)),
        );
    }

    get_root(&response)
}

pub fn create_from_fetch(
    promise_for_response: Promise<&'static Response>,
    options: Option<&'static Options>,
) -> Rc<ReactPromise> {
    let response = create_response_from_options(options);
    let fulfilled = Rc::clone(&response);
    let rejected = Rc::clone(&response);
    let _: Promise<()> = promise_for_response.then_with(
        move |r: &'static Response| {
            // SAFETY: a Flight response has a body, as upstream's `r.body as any`.
            let body: &'static ReadableStream = unsafe { js::cast(r.body().map(js::unknown)) };
            if __DEV__
                && let Some(readable) = options
                    .and_then(|options| options.debug_channel)
                    .and_then(|debug_channel| debug_channel.readable)
            {
                let stream_done_count = Rc::new(Cell::new(0.0));
                let handle_done: Rc<dyn Fn()> = {
                    let response = Rc::clone(&fulfilled);
                    Rc::new(move || {
                        stream_done_count.set(stream_done_count.get() + 1.0);
                        if stream_done_count.get() == 2.0 {
                            close(&response);
                        }
                    })
                };
                start_reading_from_universal_stream(
                    Rc::clone(&fulfilled),
                    readable,
                    Rc::clone(&handle_done),
                );
                start_reading_from_stream(fulfilled, body, handle_done, Some(js::unknown(r)));
            } else {
                let closing = Rc::clone(&fulfilled);
                start_reading_from_stream(
                    fulfilled,
                    body,
                    Rc::new(move || close(&closing)),
                    Some(js::unknown(r)),
                );
            }
        },
        move |e: &'static JsError| {
            report_global_error(&rejected, e.as_unknown());
        },
    );
    get_root(&response)
}

/// encodeReply's options.
pub struct EncodeReplyOptions {
    pub temporary_references: Option<TemporaryReferenceSet>,
    pub signal: Option<&'static AbortSignal>,
}

pub fn encode_reply(
    value: Option<&'static ReactServerValue>,
    options: Option<&'static EncodeReplyOptions>,
) -> Promise<ReplyBody> /* We don't use URLSearchParams yet but maybe */ {
    js::promise::new(move |resolve, reject| {
        process_reply(
            value,
            "",
            options.and_then(|options| options.temporary_references.clone()),
            resolve,
            reject,
            options.and_then(|options| options.signal),
        );
    })
}

js::on_load! {
    if __DEV__ {
        inject_into_dev_tools();
    }
}
