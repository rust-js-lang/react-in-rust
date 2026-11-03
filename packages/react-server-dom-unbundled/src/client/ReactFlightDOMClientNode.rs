// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;
use std::rc::Rc;

use js::{JsError, Promise, Unknown};
use node::event;
use node::events::EventEmitterExt;
use node::stream::Readable;
use react_client::react_flight_reply_client::EncodeFormActionCallback;
use shared::environment::__DEV__;

use react_client::react_flight_client::{
    DebugChannel, FindSourceMapURLCallback, ReactPromise, WeakResponse, close, create_response,
    create_stream_state, get_root, process_binary_chunk, process_string_chunk, report_global_error,
};

pub use crate::client::react_flight_dom_client_edge::*;

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

pub struct Options {
    pub nonce: Option<&'static Unknown>,
    pub encode_form_action: Option<&'static EncodeFormActionCallback>,
    #[cfg_attr(rust_js, rust_js::name = "unstable_allowPartialStream")]
    pub unstable_allow_partial_stream: Option<bool>,
    #[cfg_attr(rust_js, rust_js::name = "findSourceMapURL")]
    pub find_source_map_url: Option<&'static FindSourceMapURLCallback>,
    pub replay_console_logs: Option<bool>,
    pub environment_name: Option<String>,
    pub start_time: Option<f64>,
    pub end_time: Option<f64>,
    // For the Node.js client we only support a single-direction debug channel.
    pub debug_channel: Option<&'static Readable>,
}

fn start_reading_from_stream(
    response: Rc<WeakResponse>,
    stream: &'static Readable,
    on_end: Rc<dyn Fn()>,
) {
    let stream_state = Rc::new(create_stream_state(&response, Some(js::unknown(stream))));

    {
        let response = Rc::clone(&response);
        stream.on(
            event::Data,
            Box::new(move |chunk: Option<&Unknown>| {
                if let js::Kind::String(chunk) = js::classify(chunk) {
                    process_string_chunk(&response, &stream_state, chunk);
                } else {
                    // SAFETY: a chunk that isn't text is bytes, as upstream's `any`.
                    process_binary_chunk(&response, &stream_state, unsafe { js::cast(chunk) });
                }
            }),
        );
    }

    stream.on(
        event::Error,
        Box::new(move |error: &JsError| {
            report_global_error(&response, error.as_unknown());
        }),
    );

    stream.on(event::End, Box::new(move || on_end()));
}

pub fn create_from_node_stream(
    stream: &'static Readable,
    server_consumer_manifest: &'static ServerConsumerManifest,
    options: Option<&'static Options>,
) -> Rc<ReactPromise> {
    let debug_channel = if __DEV__ && options.is_some_and(|options| options.debug_channel.is_some())
    {
        Some(Rc::new(DebugChannel {
            has_readable: true,
            callback: None,
        }))
    } else {
        None
    };

    let response = create_response(
        Some(js::unknown(server_consumer_manifest.module_map)),
        js::any_of(server_consumer_manifest.server_module_map),
        server_consumer_manifest.module_loading,
        Some(&no_server_call),
        options.and_then(|options| options.encode_form_action),
        if let Some(options) = options
            && let js::Kind::String(nonce) = js::classify(options.nonce)
        {
            Some(nonce.to_owned())
        } else {
            None
        },
        None, // TODO: If encodeReply is supported, this should support temporaryReferences
        options
            .and_then(|options| options.unstable_allow_partial_stream)
            .unwrap_or(false),
        if __DEV__ {
            options.and_then(|options| options.find_source_map_url)
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
    );

    if __DEV__
        && let Some(options) = options
        && let Some(debug_channel) = options.debug_channel
    {
        let stream_ended_count = Rc::new(Cell::new(0.0));
        let handle_end: Rc<dyn Fn()> = {
            let response = Rc::clone(&response);
            Rc::new(move || {
                stream_ended_count.set(stream_ended_count.get() + 1.0);
                if stream_ended_count.get() == 2.0 {
                    close(&response);
                }
            })
        };
        start_reading_from_stream(Rc::clone(&response), debug_channel, Rc::clone(&handle_end));
        start_reading_from_stream(Rc::clone(&response), stream, handle_end);
    } else {
        let closing = Rc::clone(&response);
        start_reading_from_stream(
            Rc::clone(&response),
            stream,
            Rc::new(move || close(&closing)),
        );
    }

    get_root(&response)
}
