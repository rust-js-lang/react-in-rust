// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

//! This is a renderer of React that doesn't have a render target output.
//! It is useful to demonstrate the internals of the reconciler in isolation
//! and for testing semantics of reconciliation separate from the host
//! environment.

use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

use js::{Function, JsObject, SymbolFor, Uint8Array, Unknown};
use shared::environment::__DEV__;
use webapi::{AbortSignal, TextEncoder, text_encoder};

use crate::flight_modules::save_module;

/// A chunk Flight writes, its text.
pub struct Chunk(PhantomData<JsObject>);

unsafe impl js::Defined for Chunk {}

/// What a render writes to: each chunk, in order.
pub type Destination = RefCell<Vec<&'static Chunk>>;

thread_local! {
    static TEXT_ENCODER: &'static TextEncoder = text_encoder::new();
}

/// A client reference: its module's value.
pub struct ClientReference(PhantomData<JsObject>);

unsafe impl js::Defined for ClientReference {}

impl ClientReference {
    #[cfg_attr(rust_js, rust_js::link_name = "get value")]
    fn value(&self) -> &'static Function {
        unreachable!()
    }
}

/// The noop renderer's Flight host config: what Flight asks of it.
struct HostConfig {
    schedule_microtask: fn(&dyn Fn()),
    schedule_work: fn(&dyn Fn()),
    begin_writing: fn(&Destination),
    write_chunk: fn(&Destination, &'static Chunk),
    write_chunk_and_return: fn(&Destination, &'static Chunk) -> bool,
    complete_writing: fn(&Destination),
    close: fn(&Destination),
    close_with_error: fn(&Destination, Option<&'static Unknown>),
    flush_buffered: fn(&Destination),
    string_to_chunk: fn(&str) -> &'static Uint8Array,
    string_to_precomputed_chunk: fn(&str) -> &'static Uint8Array,
    byte_length_of_chunk: fn(&Uint8Array) -> f64,
    is_client_reference: fn(&'static Unknown) -> bool,
    is_server_reference: fn(&'static Unknown) -> bool,
    get_client_reference_key: fn(&'static Unknown) -> &'static Unknown,
    resolve_client_reference_metadata: fn(Option<&'static Unknown>, &ClientReference) -> String,
}

unsafe impl js::Defined for HostConfig {}

/// The Flight server React's noop renderer is: of its host config.
pub struct ReactNoopFlightServer(PhantomData<JsObject>);

unsafe impl js::Defined for ReactNoopFlightServer {}

/// Flight's request, of the noop renderer.
pub struct Request(PhantomData<JsObject>);

unsafe impl js::Defined for Request {}

impl ReactNoopFlightServer {
    #[cfg_attr(rust_js, rust_js::link_name = "createRequest")]
    fn create_request(
        &self,
        model: Option<&'static Unknown>,
        bundler_config: Option<&'static Unknown>,
        on_error: Option<&'static dyn Fn(Option<&'static Unknown>)>,
        identifier_prefix: Option<&'static str>,
        temporary_references: Option<&'static Unknown>,
        start_time: Option<f64>,
        environment_name: Option<&'static Unknown>,
        filter_stack_frame: Option<&'static FilterStackFrame>,
        keep_debug_alive: bool,
    ) -> &'static Request {
        let _ = (
            model,
            bundler_config,
            on_error,
            identifier_prefix,
            temporary_references,
            start_time,
            environment_name,
            filter_stack_frame,
            keep_debug_alive,
        );
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "attachAbortSignal")]
    fn attach_abort_signal(&self, request: &Request, signal: &AbortSignal) {
        let _ = (request, signal);
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "resolveDebugMessage")]
    fn resolve_debug_message(&self, request: &Request, message: &str) {
        let _ = (request, message);
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "startWork")]
    fn start_work(&self, request: &Request) {
        let _ = request;
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "startFlowing")]
    fn start_flowing(&self, request: &Request, destination: &Destination) {
        let _ = (request, destination);
        unreachable!()
    }
}

#[cfg_attr(rust_js, rust_js::link_name = "react-server/flight#default")]
#[cfg_attr(rust_js, rust_js::name = "ReactFlightServer")]
fn react_flight_server(config: &HostConfig) -> &'static ReactNoopFlightServer {
    let _ = config;
    unreachable!()
}

thread_local! {
    // $FlowFixMe[prop-missing]
    // $FlowFixMe[incompatible-type]
    #[cfg_attr(rust_js, rust_js::name = "ReactNoopFlightServer")]
    static REACT_NOOP_FLIGHT_SERVER: &'static ReactNoopFlightServer = react_flight_server(&HostConfig {
        schedule_microtask: |callback| {
            callback();
        },
        schedule_work: |callback| {
            callback();
        },
        begin_writing: |_destination| {},
        write_chunk: |destination, chunk| {
            destination.borrow_mut().push(chunk);
        },
        write_chunk_and_return: |destination, chunk| {
            destination.borrow_mut().push(chunk);
            true
        },
        complete_writing: |_destination| {},
        close: |_destination| {},
        close_with_error: |_destination, _error| {},
        flush_buffered: |_destination| {},
        string_to_chunk: |content| TEXT_ENCODER.with(|text_encoder| text_encoder.encode(Some(content))),
        string_to_precomputed_chunk: |content| {
            TEXT_ENCODER.with(|text_encoder| text_encoder.encode(Some(content)))
        },
        byte_length_of_chunk: |chunk| chunk.byte_length(),
        is_client_reference: |reference| {
            SymbolFor::from_value(js::get(reference, "$$typeof"))
                == Some(SymbolFor("react.client.reference"))
        },
        is_server_reference: |reference| {
            SymbolFor::from_value(js::get(reference, "$$typeof"))
                == Some(SymbolFor("react.server.reference"))
        },
        get_client_reference_key: |reference| reference,
        resolve_client_reference_metadata: |_config, reference| save_module(reference.value()),
    });
}

/// Whether a stack frame is shown, of its URL and its function's name.
pub type FilterStackFrame = dyn Fn(&str, &str) -> bool;

/// What a debug channel is told of each message.
pub type OnMessage = dyn Fn(&str);

/// What a debug channel is told: its `onMessage`, which a render sets.
pub struct DebugChannel {
    #[cfg_attr(rust_js, rust_js::optional)]
    pub on_message: RefCell<Option<Rc<OnMessage>>>,
}

pub struct Options {
    pub environment_name: Option<&'static Unknown>,
    pub filter_stack_frame: Option<&'static FilterStackFrame>,
    pub identifier_prefix: Option<&'static str>,
    pub signal: Option<&'static AbortSignal>,
    pub debug_channel: Option<&'static DebugChannel>,
    pub on_error: Option<&'static dyn Fn(Option<&'static Unknown>)>,
    pub start_time: Option<f64>,
}

pub fn render(model: Option<&'static Unknown>, options: Option<&'static Options>) -> Destination {
    let destination: Destination = RefCell::new(Vec::new());
    let bundler_config = None;
    let request = REACT_NOOP_FLIGHT_SERVER.with(|react_noop_flight_server| {
        react_noop_flight_server.create_request(
            model,
            // $FlowFixMe[incompatible-type]
            bundler_config,
            options.and_then(|options| options.on_error),
            options.and_then(|options| options.identifier_prefix),
            None,
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
            // $FlowFixMe[incompatible-type]
            __DEV__ && options.is_some_and(|options| options.debug_channel.is_some()),
        )
    });
    let signal = options.and_then(|options| options.signal);
    if let Some(signal) = signal {
        REACT_NOOP_FLIGHT_SERVER.with(|react_noop_flight_server| {
            react_noop_flight_server.attach_abort_signal(request, signal);
        });
    }
    if __DEV__ && let Some(debug_channel) = options.and_then(|options| options.debug_channel) {
        debug_channel
            .on_message
            .replace(Some(Rc::new(move |message: &str| {
                REACT_NOOP_FLIGHT_SERVER.with(|react_noop_flight_server| {
                    react_noop_flight_server.resolve_debug_message(request, message);
                });
            })));
    }
    REACT_NOOP_FLIGHT_SERVER.with(|react_noop_flight_server| {
        react_noop_flight_server.start_work(request);
        react_noop_flight_server.start_flowing(
            request,
            // $FlowFixMe[incompatible-type]
            &destination,
        );
    });
    destination
}
