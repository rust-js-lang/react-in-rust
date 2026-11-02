// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js::{JsError, Kind, Promise, Unknown, dict, promise};
use shared::environment::__DEV__;
use shared::react_types::ReactFormState;
use webapi::{
    AbortSignal, FormData, QueuingStrategy, ReadableByteStreamController, ReadableStream,
    ReadableStreamDefaultReader, ReadableStreamReadResult, UnderlyingByteSource, WritableStream,
    form_data, readable_stream,
};

use crate::client::react_flight_client_config_bundler_parcel::{
    ServerManifest, preload_module, require_module, resolve_server_reference,
};

use super::react_flight_server_config_parcel_bundler::ClientManifest;

use react_server::react_flight_hooks::{
    EnvironmentName, FilterStackFrame, OnError, ReactClientValue, Request, abort,
    attach_abort_signal, close_debug_channel, create_prerender_request, create_request,
    resolve_debug_message, start_flowing, start_flowing_debug, start_work, stop_flowing,
};

use react_server::react_flight_reply_server::{ReactPromise, close, create_response, get_root};

use react_server::react_flight_action_server::{
    decode_action as decode_action_impl, decode_form_state as decode_form_state_impl,
};

pub use crate::react_flight_parcel_references::{
    create_client_reference, register_server_reference,
};

use react_client::react_flight_client_stream_config_web::{
    StringDecoder, create_string_decoder, read_final_string_chunk, read_partial_string_chunk,
};

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
    ) -> Option<Promise<()>> {
        // GAP rd583: a read's `value` is an `Option` of one, `$someValue(value)`,
        // where upstream reads it as given.
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
            resolve_debug_message(self.request, message);
        }
        *self.string_buffer.borrow_mut() = last;
        if js::truthy(done.as_ref()) {
            close_debug_channel(self.request);
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
        &ClientManifest::Null,
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
            &ClientManifest::Null,
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

thread_local! {
    static SERVER_MANIFEST: Cell<&'static ServerManifest> = Cell::new(dict::new());
}

pub fn register_server_actions(manifest: &'static ServerManifest) {
    // This function is called by the bundler to register the manifest.
    SERVER_MANIFEST.set(manifest);
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
        // SAFETY: react-server's manifest is the bundler's, Parcel's here.
        unsafe { js::cast(Some(js::unknown(SERVER_MANIFEST.get()))) },
        "",
        options.and_then(|options| options.temporary_references),
        Some(js::unknown(body)),
        options.and_then(|options| options.array_size_limit),
    );
    let root = get_root(&response);
    close(&response);
    root
}

pub fn decode_action(
    body: &'static FormData,
) -> Option<Promise<&'static dyn Fn() -> Option<&'static Unknown>>> {
    // SAFETY: react-server's manifest is the bundler's, Parcel's here.
    decode_action_impl(body, unsafe {
        js::cast(Some(js::unknown(SERVER_MANIFEST.get())))
    })
}

pub fn decode_form_state(
    action_result: Option<&'static Unknown>,
    body: &'static FormData,
) -> Promise<Option<ReactFormState<Option<&'static Unknown>, Option<&'static Unknown>>>> {
    // SAFETY: react-server's manifest is the bundler's, Parcel's here.
    decode_form_state_impl(action_result, body, unsafe {
        js::cast(Some(js::unknown(SERVER_MANIFEST.get())))
    })
}

pub fn load_server_action(id: &str) -> Promise<&'static dyn Fn() -> Option<&'static Unknown>> {
    let reference = Rc::new(resolve_server_reference(SERVER_MANIFEST.get(), id));
    js::promise::resolve(Rc::clone(&reference))
        // GAP rd596: what a thenable that isn't a `Promise` fulfils isn't
        // js's `Fulfils`, so this promise is typed of the thenable itself.
        .then({
            let reference = Rc::clone(&reference);
            move |_| preload_module(&reference)
        })
        .then(move |_| {
            let Kind::Function(r#fn) = js::classify(require_module(&reference)) else {
                panic!("Server actions must be functions");
            };
            r#fn
        })
}
