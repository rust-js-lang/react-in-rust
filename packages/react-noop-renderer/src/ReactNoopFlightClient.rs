// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// This is a renderer of React that doesn't have a render target output.
// It is useful to demonstrate the internals of the reconciler in isolation
// and for testing semantics of reconciliation separate from the host
// environment.

use std::cell::Cell;
use std::rc::Rc;

use js::{Function, Uint8Array, Unknown, function};
use react_client::react_flight_client::{
    FindSourceMapURLCallback, ReactFlightClient, ReactPromise,
};
use shared::environment::__DEV__;

use crate::flight_modules::read_module;
use webapi::{TextDecodeOptions, TextDecoder, TextDecoderOptions, console, text_decoder};

#[cfg_attr(rust_js, rust_js::link_name = "react-client/flight#default")]
#[cfg_attr(rust_js, rust_js::name = "ReactFlightClient")]
#[cfg_attr(rust_js, rust_js::runs_nothing)]
fn react_flight_client(config: &FlightClientConfig) -> &'static ReactFlightClient {
    let _ = config;
    unreachable!()
}

type Source = Vec<&'static Uint8Array>;

#[cfg_attr(rust_js, rust_js::name = "decoderOptions")]
static DECODER_OPTIONS: TextDecodeOptions = TextDecodeOptions { stream: Some(true) };

struct FlightClientConfig {
    create_string_decoder: fn(Option<bool>) -> &'static TextDecoder,
    read_partial_string_chunk: fn(&TextDecoder, &Uint8Array) -> String,
    read_final_string_chunk: fn(&TextDecoder, &Uint8Array) -> String,
    resolve_client_reference: fn(Option<&Unknown>, &'static str) -> &'static str,
    prepare_destination_for_module: fn(Option<&Unknown>, &str),
    preload_module: fn(&str),
    require_module: fn(&str) -> Option<&'static Function>,
    bind_to_console: fn(&str, &[Option<&'static Unknown>], &str) -> &'static Function,
    check_eval_availability_once_dev: fn(),
}

thread_local! {
    static FLIGHT_CLIENT: &'static ReactFlightClient = react_flight_client(&FlightClientConfig {
        create_string_decoder: |preserve_byte_order_mark| {
            text_decoder::new(
                Some("utf-8"),
                Some(TextDecoderOptions {
                    ignore_bom: Some(preserve_byte_order_mark.unwrap_or(false)),
                    ..Default::default()
                }),
            )
        },
        read_partial_string_chunk: |decoder, buffer| decoder.decode(Some(&buffer), Some(DECODER_OPTIONS)),
        read_final_string_chunk: |decoder, buffer| decoder.decode(Some(&buffer), None),
        resolve_client_reference: |_bundler_config, idx| idx,
        prepare_destination_for_module: |_module_loading, _metadata| {},
        preload_module: |_idx| {},
        require_module: |idx| read_module(idx),
        bind_to_console: |method_name, args, _badge_name| {
            // SAFETY: Flight replays only console's own methods, each a function.
            let method: &'static Function = unsafe { js::cast(js::get(console, method_name)) };
            function::bind(method, Some(&console), args)
        },
        check_eval_availability_once_dev,
    });
}

pub struct ReadOptions {
    #[cfg_attr(rust_js, rust_js::name = "findSourceMapURL")]
    pub find_source_map_url: Option<&'static FindSourceMapURLCallback>,
    pub debug_channel: Option<&'static DebugChannelOptions>,
    pub close: Option<bool>,
}

pub struct DebugChannelOptions {
    pub on_message: &'static dyn Fn(&str),
}

pub fn read(source: &'static Source, options: Option<&ReadOptions>) -> Rc<ReactPromise> {
    FLIGHT_CLIENT.with(|&client| {
        let response = client.create_response(
            Some(js::unknown(source)),
            None,
            None,
            None,
            None,
            None,
            None,
            false,
            options.and_then(|options| options.find_source_map_url),
            true,
            None,
            None,
            None,
            None,
        );
        let stream_state = client.create_stream_state(&response, Some(js::unknown(source)));
        for chunk in source {
            client.process_binary_chunk(&response, &stream_state, chunk);
        }
        if options.is_some_and(|options| options.close == Some(true)) {
            client.close(&response);
        }
        client.get_root(&response)
    })
}

thread_local! {
    static HAS_CONFIRMED_EVAL: Cell<bool> = const { Cell::new(false) };
}
fn check_eval_availability_once_dev() {
    if __DEV__ {
        if !HAS_CONFIRMED_EVAL.get() {
            HAS_CONFIRMED_EVAL.set(true);
            // eslint-disable-next-line no-eval
            if js::eval("null").is_err() {
                console::error((concat!(
                    "eval() is not supported in this environment. ",
                    "React requires eval() in development mode for various debugging features ",
                    "like reconstructing callstacks from a different environment.\n",
                    "React will never use eval() in production mode",
                ),));
            }
        }
    } else {
        panic!(
            "checkEvalAvailabilityOnceDev should never be called in production mode. This is a bug in React."
        );
    }
}
