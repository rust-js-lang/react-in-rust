// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js::{JsError, Promise, Uint8Array, Unknown};
use shared::environment::{__DEV__, PARCEL_REQUIRE_META_DEV_SERVER};
use webapi::{
    AbortSignal, ReadableStream, ReadableStreamReadResult, Response, URLSearchParams,
    url_search_params,
};

use react_client::react_flight_client::{
    DebugChannel, ReactPromise, WeakResponse, close, create_response, create_stream_state,
    get_root, process_binary_chunk, report_global_error,
};

use react_client::react_flight_reply_client::{
    EncodeFormActionCallback, ReactServerValue, ReplyBody, ServerFunction,
    create_server_reference as create_server_reference_impl, process_reply,
};

pub use react_client::react_flight_reply_client::register_server_reference;

pub use react_client::react_flight_temporary_references::{
    TemporaryReferenceSet, create_temporary_reference_set,
};

#[cfg_attr(rust_js, rust_js::nullable, rust_js::name = "findSourceMapURL")]
fn find_source_map_url(filename: &str, environment_name: &str) -> Option<String> {
    let dev_server = PARCEL_REQUIRE_META_DEV_SERVER;
    if let Some(dev_server) = dev_server {
        let qs: &URLSearchParams = url_search_params::new(None);
        qs.set("filename", filename);
        qs.set("env", environment_name);
        return Some(dev_server.to_owned() + "/__parcel_source_map?" + &qs.to_string());
    }
    None
}

fn no_server_call(
    _id: &'static Unknown,
    _args: &'static Unknown,
) -> Promise<Option<&'static Unknown>> {
    panic!(
        "{}",
        concat!(
            "Server Functions cannot be called during initial render. ",
            "This would create a fetch waterfall. Try to use a Server Component ",
            "to pass data to Client Components instead.",
        )
    );
}

pub fn create_server_reference(id: &str, export_name: &str) -> &'static ServerFunction {
    create_server_reference_impl(
        js::unknown_of(id.to_owned() + "#" + export_name),
        &no_server_call,
        None,
        Some(&find_source_map_url),
        Some(export_name.to_owned()),
    )
}

pub struct DebugChannelOptions {
    pub readable: Option<&'static ReadableStream>,
}

pub struct Options {
    pub nonce: Option<&'static Unknown>,
    pub encode_form_action: Option<&'static EncodeFormActionCallback>,
    pub temporary_references: Option<TemporaryReferenceSet>,
    #[cfg_attr(rust_js, rust_js::name = "unstable_allowPartialStream")]
    pub unstable_allow_partial_stream: Option<bool>,
    pub replay_console_logs: Option<bool>,
    pub environment_name: Option<String>,
    pub start_time: Option<f64>,
    pub end_time: Option<f64>,
    // For the Edge client we only support a single-direction debug channel.
    pub debug_channel: Option<&'static DebugChannelOptions>,
}

fn create_response_from_options(options: Option<&'static Options>) -> Rc<WeakResponse> {
    let debug_channel = if __DEV__
        && let Some(options) = options
        && let Some(debug_channel) = options.debug_channel
    {
        Some(Rc::new(DebugChannel {
            has_readable: debug_channel.readable.is_some(),
            callback: None,
        }))
    } else {
        None
    };

    create_response(
        None, // bundlerConfig
        None, // serverReferenceConfig
        None, // moduleLoading
        Some(&no_server_call),
        options.and_then(|options| options.encode_form_action),
        if let Some(options) = options
            && let js::Kind::String(nonce) = js::classify(options.nonce)
        {
            Some(nonce.to_owned())
        } else {
            None
        },
        options.and_then(|options| options.temporary_references.as_ref()),
        options
            .and_then(|options| options.unstable_allow_partial_stream)
            .unwrap_or(false),
        if __DEV__ {
            Some(&find_source_map_url)
        } else {
            None
        },
        __DEV__ && options.is_some_and(|options| options.replay_console_logs == Some(true)), // defaults to false
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
        start_reading_from_stream(
            Rc::clone(&response),
            readable,
            Rc::clone(&handle_done),
            None,
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
                start_reading_from_stream(
                    Rc::clone(&fulfilled),
                    readable,
                    Rc::clone(&handle_done),
                    None,
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
