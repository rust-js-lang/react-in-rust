// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use js::{
    ArrayBuffer, ArrayBufferView, Error, JsError, JsObject, Kind, Map, Symbol, Uint8Array, Unknown,
    WeakMap, WeakSet, function,
};
use shared::get_prototype_of::GET_PROTOTYPE_OF;
use shared::has_own_property::HAS_OWN_PROPERTY;
use shared::is_array::is_array;
use shared::react_feature_flags::ENABLE_FLIGHT_OBJECT_REFERENCES;
use shared::react_symbols::ASYNC_ITERATOR;
use shared::react_types::Thenable;
use webapi::{
    Blob, BlobPart, BlobPropertyBag, File, FilePropertyBag, FormData, FormDataEntryValue,
    ReadableByteStreamController, ReadableStream, ReadableStreamDefaultController,
    UnderlyingByteSource, UnderlyingDefaultSource, blob, file, form_data, readable_stream,
};

use crate::react_flight_server_temporary_references::{
    TemporaryReferenceSet, create_temporary_reference, register_temporary_reference,
};

// The server acts as a Client of itself when resolving Server References.
// That's why we import the Client configuration from the Server.
// Everything is aliased as their Server equivalence for clarity.

/// react-client's ReactFlightClientConfig's `ServerManifest`, its JS's.
pub struct ServerManifest(PhantomData<JsObject>);

unsafe impl js::Defined for ServerManifest {}

/// react-client's ReactFlightClientConfig's `ClientReference`, a Server
/// Reference here, its JS's.
pub struct ServerReference(PhantomData<JsObject>);

unsafe impl js::Defined for ServerReference {}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-client/src/ReactFlightClientConfig#resolveServerReference"
)]
pub(crate) fn resolve_server_reference(
    bundler_config: &ServerManifest,
    id: &str,
) -> &'static ServerReference {
    let _ = (bundler_config, id);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-client/src/ReactFlightClientConfig#preloadModule"
)]
#[cfg_attr(rust_js, rust_js::nullable)]
pub(crate) fn preload_module(metadata: &ServerReference) -> Option<&'static Thenable> {
    let _ = metadata;
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-client/src/ReactFlightClientConfig#requireModule"
)]
pub(crate) fn require_module(metadata: &ServerReference) -> Option<&'static Unknown> {
    let _ = metadata;
    unreachable!()
}

/// ReactFlightReplyBackingFormData's `BackingFormData`, its JS's.
pub struct BackingFormData(PhantomData<JsObject>);

unsafe impl js::Defined for BackingFormData {}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightReplyBackingFormData.js#createBackingFormData"
)]
fn create_backing_form_data(form_data: &FormData, prefix: &str) -> &'static BackingFormData {
    let _ = (form_data, prefix);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightReplyBackingFormData.js#appendBackingEntry"
)]
fn append_backing_entry(backing_store: &BackingFormData, value: FormDataEntryValue<'_>, key: &str) {
    let _ = (backing_store, value, key);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightReplyBackingFormData.js#consumeBackingEntries"
)]
fn consume_backing_entries(
    backing_store: &BackingFormData,
    form_prefix: &str,
) -> &'static FormData {
    let _ = (backing_store, form_prefix);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightReplyBackingFormData.js#getBackingString"
)]
fn get_backing_string(backing_store: &BackingFormData, key: &str) -> Option<String> {
    let _ = (backing_store, key);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightReplyBackingFormData.js#getStreamChunks"
)]
fn get_stream_chunks(backing_store: &BackingFormData, key: &str) -> Option<Vec<String>> {
    let _ = (backing_store, key);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightReplyBackingFormData.js#getBackingBlob"
)]
fn get_backing_blob(backing_store: &BackingFormData, key: &str) -> Option<&'static Blob> {
    let _ = (backing_store, key);
    unreachable!()
}

/// What a listener of a chunk is given: any value.
type Listen = dyn Fn(Option<&'static Unknown>);

/// What a stream's rows are given to.
pub struct FlightStreamController {
    pub enqueue_model: Rc<dyn Fn(&str)>,
    pub close: Rc<dyn Fn(&str)>,
    #[cfg_attr(rust_js, rust_js::test)]
    pub error: Rc<Listen>,
}

pub type JSONValue = Unknown;

const __PROTO__: &str = "__proto__";

thread_local! {
    static RESPONSE_SYMBOL: &'static Symbol = js::symbol::new(None);
}

/// A chunk's listener: a function, or a reference waiting on its value.
#[cfg_attr(rust_js, rust_js::untagged)]
#[derive(Clone)]
pub enum Listener {
    Function(Rc<Listen>),
    #[cfg_attr(rust_js, rust_js::otherwise)]
    Reference(Rc<InitializationReference>),
}

/// A resolved model's chunk's `reason`: its id and its response.
pub struct ModelReason {
    pub id: f64,
    #[cfg_attr(rust_js, rust_js::name(RESPONSE_SYMBOL))]
    pub response: Rc<Response>,
}

/// An initialized chunk's `reason`: a model's array context, or a
/// stream's controller.
#[cfg_attr(rust_js, rust_js::untagged)]
#[derive(Clone)]
pub enum InitializedReason {
    ArrayRoot(Rc<NestedArrayContext>),
    Controller(Rc<FlightStreamController>),
}

#[cfg_attr(rust_js, rust_js::tag = "status")]
pub enum ChunkState {
    #[cfg_attr(rust_js, rust_js::name = "pending")]
    Pending {
        #[cfg_attr(rust_js, rust_js::nullable)]
        value: Option<Vec<Listener>>,
        #[cfg_attr(rust_js, rust_js::nullable)]
        reason: Option<Vec<Listener>>,
    },
    #[cfg_attr(rust_js, rust_js::name = "blocked")]
    Blocked {
        #[cfg_attr(rust_js, rust_js::nullable)]
        value: Option<Vec<Listener>>,
        #[cfg_attr(rust_js, rust_js::nullable)]
        reason: Option<Vec<Listener>>,
    },
    #[cfg_attr(rust_js, rust_js::name = "resolved_model")]
    ResolvedModel { value: String, reason: ModelReason },
    #[cfg_attr(rust_js, rust_js::name = "fulfilled")]
    Initialized {
        value: Option<&'static Unknown>,
        #[cfg_attr(rust_js, rust_js::nullable)]
        reason: Option<InitializedReason>,
    },
    #[cfg_attr(rust_js, rust_js::name = "rejected")]
    Errored { reason: Option<&'static Unknown> },
}

#[cfg_attr(rust_js, rust_js::class)]
pub struct ReactPromise {
    #[cfg_attr(rust_js, rust_js::flatten)]
    pub state: RefCell<ChunkState>,
}

unsafe impl js::Defined for ReactPromise {}

impl js::Classed for ReactPromise {
    type Constructor = js::AnyConstructor;
}

#[cfg_attr(rust_js, rust_js::class)]
impl ReactPromise {
    fn new(state: ChunkState) -> Self {
        ReactPromise {
            state: RefCell::new(state),
        }
    }

    // TODO: This doesn't return a new Promise chain unlike the real .then
    pub fn then(self: &Rc<Self>, resolve: Option<Rc<Listen>>, reject: Option<Rc<Listen>>) {
        let chunk = self;
        // If we have resolved content, we try to initialize it first which
        // might put us back into one of the other states.
        if matches!(*chunk.state.borrow(), ChunkState::ResolvedModel { .. }) {
            initialize_model_chunk(chunk);
        }
        // The status might have changed after initialization.
        // GAP rd482: a flattened state is read for a moment, so upstream's
        // `switch (chunk.status)` of arms that call out is a read of each.
        // GAP rd489: its value is read again where it's used, as
        // `Some(*value)` of an `Option<&Unknown>` gives the inner `None` as
        // `null`, which a listener would be given for `undefined`.
        if matches!(*chunk.state.borrow(), ChunkState::Initialized { .. }) {
            if let Some(resolve) = resolve {
                let value = match &*chunk.state.borrow() {
                    ChunkState::Initialized { value, .. } => *value,
                    _ => None,
                };
                let mut inspected_value = value;
                // Recursively check if the value is itself a ReactPromise and if so if it points
                // back to itself. This helps catch recursive thenables early error.
                let mut cycle_protection = 0;
                let visited = js::set::new::<Unknown>(None);
                while let Some(inspected) = inspected_value
                    && js::instance_of(inspected, js::class::<ReactPromise>())
                {
                    cycle_protection += 1;
                    if js::strict_equals(inspected, js::unknown(&**chunk))
                        || visited.has(inspected)
                        || cycle_protection > 1000
                    {
                        if let Some(reject) = reject {
                            reject(Some(js::unknown(js::error::new(
                                Some("Cannot have cyclic thenables."),
                                None,
                            ))));
                        }
                        return;
                    }
                    visited.add(inspected);
                    // GAP rd483: a value of any shape that's an instance of a
                    // class is read as one by a cast.
                    let inspected: &ReactPromise = unsafe { js::cast(Some(inspected)) };
                    if let ChunkState::Initialized { value, .. } = &*inspected.state.borrow() {
                        inspected_value = *value;
                    } else {
                        // If this is lazily resolved, pending or blocked, it'll eventually become
                        // initialized and break the loop. Rejected also breaks it.
                        break;
                    }
                }
                resolve(value);
            }
        } else if let ChunkState::Pending { value, reason }
        | ChunkState::Blocked { value, reason } = &mut *chunk.state.borrow_mut()
        {
            if let Some(resolve) = resolve {
                value
                    .get_or_insert_with(Vec::new)
                    .push(Listener::Function(resolve));
            }
            if let Some(reject) = reject {
                reason
                    .get_or_insert_with(Vec::new)
                    .push(Listener::Function(reject));
            }
        } else if let Some(reject) = reject {
            let reason = match &*chunk.state.borrow() {
                ChunkState::Errored { reason } => *reason,
                _ => None,
            };
            reject(reason);
        }
    }
}

js::on_load! {
    // We subclass Promise.prototype so that we get other methods like .catch
    js::class::<ReactPromise>().set_prototype(js::object::create(
        Some(js::class::<js::Promise<()>>().prototype()),
        None,
    ));
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "ObjectPrototype")]
    static OBJECT_PROTOTYPE: &'static Unknown = js::object::prototype;
    #[cfg_attr(rust_js, rust_js::name = "ArrayPrototype")]
    static ARRAY_PROTOTYPE: &'static Unknown = js::array::prototype;
}

pub struct Response {
    pub _bundler_config: &'static ServerManifest,
    pub _prefix: String,
    pub _form_data: &'static BackingFormData,
    pub _chunks: &'static Map<f64, Rc<ReactPromise>>,
    pub _closed: Cell<bool>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub _closed_reason: Cell<Option<&'static Unknown>>,
    pub _temporary_references: Option<&'static TemporaryReferenceSet>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub _server_reference_objects: Cell<Option<&'static WeakSet<JsObject>>>,
    pub _server_reference_cache: &'static WeakMap<JsObject, Rc<ReactPromise>>,
    pub _server_object_reference_cache: &'static WeakMap<JsObject, Rc<ReactPromise>>,
    pub _root_array_contexts: &'static WeakMap<JsObject, Rc<NestedArrayContext>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub _array_size_limit: Option<&'static Unknown>,
}

pub fn get_root(response: &Rc<Response>) -> Rc<ReactPromise> {
    get_chunk(response, 0.0)
}

fn create_pending_chunk(_response: &Rc<Response>) -> Rc<ReactPromise> {
    Rc::new(ReactPromise::new(ChunkState::Pending {
        value: None,
        reason: None,
    }))
}

fn wake_chunk(
    response: &Rc<Response>,
    listeners: Vec<Listener>,
    value: Option<&'static Unknown>,
    chunk: &ReactPromise,
) {
    for listener in listeners {
        match listener {
            Listener::Function(listener) => listener(value),
            Listener::Reference(listener) => {
                let array_root = match &*chunk.state.borrow() {
                    ChunkState::Initialized {
                        reason: Some(InitializedReason::ArrayRoot(array_root)),
                        ..
                    } => Some(Rc::clone(array_root)),
                    _ => None,
                };
                fulfill_reference(response, &listener, value, array_root);
            }
        }
    }
}

fn reject_chunk(
    response: &Rc<Response>,
    listeners: Vec<Listener>,
    error: Option<&'static Unknown>,
) {
    for listener in listeners {
        match listener {
            Listener::Function(listener) => listener(error),
            Listener::Reference(listener) => reject_reference(response, &listener.handler, error),
        }
    }
}

fn wake_chunk_if_initialized(
    response: &Rc<Response>,
    chunk: &Rc<ReactPromise>,
    resolve_listeners: Vec<Listener>,
    reject_listeners: Option<Vec<Listener>>,
) {
    // GAP rd482: as ReactPromise's then.
    if matches!(*chunk.state.borrow(), ChunkState::Initialized { .. }) {
        let value = match &*chunk.state.borrow() {
            ChunkState::Initialized { value, .. } => *value,
            _ => None,
        };
        wake_chunk(response, resolve_listeners, value, chunk);
        return;
    }
    let errored = match &mut *chunk.state.borrow_mut() {
        ChunkState::Blocked { value, reason } | ChunkState::Pending { value, reason } => {
            match value {
                Some(value) => value.extend(resolve_listeners),
                None => *value = Some(resolve_listeners),
            }

            match reason {
                Some(reason) => {
                    if let Some(reject_listeners) = reject_listeners {
                        reason.extend(reject_listeners);
                    }
                }
                None => *reason = reject_listeners,
            }
            return;
        }
        ChunkState::Errored { reason } => *reason,
        _ => return,
    };
    if let Some(reject_listeners) = reject_listeners {
        reject_chunk(response, reject_listeners, errored);
    }
}

fn trigger_error_on_chunk(
    response: &Rc<Response>,
    chunk: &ReactPromise,
    error: Option<&'static Unknown>,
) {
    if !matches!(
        *chunk.state.borrow(),
        ChunkState::Pending { .. } | ChunkState::Blocked { .. }
    ) {
        // If we get more data to an already resolved ID, we assume that it's
        // a stream chunk since any other row shouldn't have more than one entry.
        // GAP rd484: upstream's `const streamChunk: InitializedStreamChunk<any>
        // = chunk as any; const controller = streamChunk.reason;`, read as
        // upstream does, so a chunk that's no stream throws JS's TypeError.
        let controller: &FlightStreamController =
            unsafe { js::cast(js::get(js::unknown(chunk), "reason")) };
        (controller.error)(error);
        return;
    }
    let listeners = match &*chunk.state.borrow() {
        ChunkState::Pending { reason, .. } | ChunkState::Blocked { reason, .. } => reason.clone(),
        _ => None,
    };
    *chunk.state.borrow_mut() = ChunkState::Errored { reason: error };
    if let Some(listeners) = listeners {
        reject_chunk(response, listeners, error);
    }
}

fn create_resolved_model_chunk(
    response: &Rc<Response>,
    value: String,
    id: f64,
) -> Rc<ReactPromise> {
    Rc::new(ReactPromise::new(ChunkState::ResolvedModel {
        value,
        reason: ModelReason {
            id,
            response: Rc::clone(response),
        },
    }))
}

fn create_errored_chunk(
    _response: &Rc<Response>,
    reason: Option<&'static Unknown>,
) -> Rc<ReactPromise> {
    // Upstream's `new ReactPromise(ERRORED, null, reason)`: ChunkState's
    // Errored has no `value`, as its writes leave the one before (rd488), so
    // the chunk is made with `value: null` and then errored.
    let chunk = Rc::new(ReactPromise::new(ChunkState::Blocked {
        value: None,
        reason: None,
    }));
    *chunk.state.borrow_mut() = ChunkState::Errored { reason };
    chunk
}

fn resolve_model_chunk(response: &Rc<Response>, chunk: &Rc<ReactPromise>, value: String, id: f64) {
    if !matches!(*chunk.state.borrow(), ChunkState::Pending { .. }) {
        // If we get more data to an already resolved ID, we assume that it's
        // a stream chunk since any other row shouldn't have more than one entry.
        // GAP rd484: as trigger_error_on_chunk's.
        let controller: &FlightStreamController =
            unsafe { js::cast(js::get(js::unknown(&**chunk), "reason")) };
        if js::string::get(&value, 0.0) == Some("C") {
            (controller.close)(&if value == "C" {
                String::from("\"$undefined\"")
            } else {
                js::string::slice(&value, 1.0, None)
            });
        } else {
            (controller.enqueue_model)(&value);
        }
        return;
    }
    let (resolve_listeners, reject_listeners) = match &mut *chunk.state.borrow_mut() {
        ChunkState::Pending { value, reason } => (value.take(), reason.take()),
        _ => (None, None),
    };
    *chunk.state.borrow_mut() = ChunkState::ResolvedModel {
        value,
        reason: ModelReason {
            id,
            response: Rc::clone(response),
        },
    };
    if let Some(resolve_listeners) = resolve_listeners {
        // This is unfortunate that we're reading this eagerly if
        // we already have listeners attached since they might no
        // longer be rendered or might not be the highest pri.
        initialize_model_chunk(chunk);
        // The status might have changed after initialization.
        wake_chunk_if_initialized(response, chunk, resolve_listeners, reject_listeners);
    }
}

fn create_initialized_stream_chunk(
    _response: &Rc<Response>,
    value: &'static Unknown,
    controller: Rc<FlightStreamController>,
) -> Rc<ReactPromise> {
    // We use the reason field to stash the controller since we already have that
    // field. It's a bit of a hack but efficient.
    Rc::new(ReactPromise::new(ChunkState::Initialized {
        value: Some(value),
        reason: Some(InitializedReason::Controller(controller)),
    }))
}

fn create_resolved_iterator_result_chunk(
    response: &Rc<Response>,
    value: &str,
    done: bool,
) -> Rc<ReactPromise> {
    // To reuse code as much code as possible we add the wrapper element as part of the JSON.
    let iterator_result_json = (if done {
        "{\"done\":true,\"value\":"
    } else {
        "{\"done\":false,\"value\":"
    })
    .to_owned()
        + value
        + "}";
    Rc::new(ReactPromise::new(ChunkState::ResolvedModel {
        value: iterator_result_json,
        reason: ModelReason {
            id: -1.0,
            response: Rc::clone(response),
        },
    }))
}

fn resolve_iterator_result_chunk(
    response: &Rc<Response>,
    chunk: &Rc<ReactPromise>,
    value: &str,
    done: bool,
) {
    // To reuse code as much code as possible we add the wrapper element as part of the JSON.
    let iterator_result_json = (if done {
        "{\"done\":true,\"value\":"
    } else {
        "{\"done\":false,\"value\":"
    })
    .to_owned()
        + value
        + "}";
    resolve_model_chunk(response, chunk, iterator_result_json, -1.0);
}

fn load_server_reference(
    response: &Rc<Response>,
    meta_data: Option<&'static Unknown>,
    parent_object: &'static Unknown,
    key: &str,
) -> Option<&'static Unknown> {
    let Kind::String(id) = js::classify(js::get(meta_data, "id")) else {
        return js::null();
    };
    if key == "then" {
        // This should never happen because we always serialize objects with then-functions
        // as "thenable" which reduces to ReactPromise with no other fields.
        return js::null();
    }
    // GAP rd483: the metadata is any value; the cache's key is an object.
    let meta_data_key: &'static JsObject = unsafe { js::cast(meta_data) };

    // Check for a cached promise from a previous call with the same metadata.
    // This handles deduplication when the same server reference appears multiple
    // times in the payload. The cache is a response-scoped map keyed by the
    // metadata object rather than a field on it, so a client-supplied metadata
    // field can't inject a resolved value and bypass the manifest lookup below.
    let cached_promise = response._server_reference_cache.get(meta_data_key);
    if let Some(cached_promise) = cached_promise {
        return read_server_reference(response, &cached_promise, parent_object, key);
    }

    // This is the first call for this server reference metadata. Create a cached
    // promise to be used for subsequent calls.
    let blocked_promise = Rc::new(ReactPromise::new(ChunkState::Blocked {
        value: None,
        reason: None,
    }));
    response
        ._server_reference_cache
        .set(meta_data_key, Rc::clone(&blocked_promise));

    let server_reference = resolve_server_reference(response._bundler_config, id);
    // We expect most servers to not really need this because you'd just have all
    // the relevant modules already loaded but it allows for lazy loading of code
    // if needed.
    let bound = js::get(meta_data, "bound");
    let server_reference_promise: &'static Thenable = match preload_module(server_reference) {
        None => {
            if js::instance_of(bound, js::class::<ReactPromise>()) {
                // GAP rd485: `Promise.resolve(bound)` and `Promise.all([..])`
                // of thenables: js's promise functions take and give
                // `Promise`s, which a `Thenable` is none of.
                unsafe { js::cast(Some(js::unknown(&js::promise::resolve(bound)))) }
            } else {
                // Nothing to preload and no bound arguments to wait for, so we can
                // resolve the reference synchronously.
                let value = require_server_reference(response, server_reference);
                resolve_server_reference_chunk(response, &blocked_promise, value);
                return read_server_reference(response, &blocked_promise, parent_object, key);
            }
        }
        Some(server_reference_promise) => {
            if js::instance_of(bound, js::class::<ReactPromise>()) {
                // GAP rd485: as above.
                unsafe {
                    js::cast(Some(js::unknown(&js::promise::all(vec![
                        js::cast::<js::Promise<Option<&'static Unknown>>>(Some(js::unknown(
                            server_reference_promise,
                        ))),
                        js::cast(bound),
                    ]))))
                }
            } else {
                server_reference_promise
            }
        }
    };

    let fulfill = {
        let response = Rc::clone(response);
        let blocked_promise = Rc::clone(&blocked_promise);
        move |_| {
            let value = match catch_unwind(AssertUnwindSafe(|| {
                let mut value = require_server_reference(&response, server_reference);
                let bound = js::get(meta_data, "bound");
                if js::truthy(bound) {
                    // This promise is coming from us and should have initialized by now.
                    let promise_value = js::get(bound, "value");
                    let mut bound_args: Vec<Option<&'static Unknown>> =
                        match js::classify(promise_value) {
                            Kind::Array(promise_value) => promise_value.to_vec(),
                            _ => Vec::new(),
                        };
                    if bound_args.len() > MAX_BOUND_ARGS {
                        panic!(
                            "Server Function has too many bound arguments. Received {} but the limit is {}.",
                            bound_args.len(),
                            MAX_BOUND_ARGS
                        );
                    }
                    bound_args.insert(0, js::null()); // this
                    // GAP rd486: upstream's `value.bind.apply(value, boundArgs)`,
                    // the value's own `bind`, read and applied as upstream does
                    // (a value with none throws JS's TypeError), its function
                    // of any shape read by a cast.
                    let bind: &dyn Fn() -> Option<&'static Unknown> =
                        unsafe { js::cast(js::get(value, "bind")) };
                    value = function::apply(bind, Some(&value), Some(&bound_args));
                }
                value
            })) {
                Ok(value) => value,
                Err(error) => {
                    let error = error
                        .downcast_ref::<&'static JsError>()
                        .and_then(|error| error.as_unknown());
                    trigger_error_on_chunk(&response, &blocked_promise, error);
                    return;
                }
            };
            resolve_server_reference_chunk(&response, &blocked_promise, value);
        }
    };
    server_reference_promise.then(fulfill, {
        let response = Rc::clone(response);
        let blocked_promise = Rc::clone(&blocked_promise);
        move |error| {
            trigger_error_on_chunk(&response, &blocked_promise, error);
        }
    });
    read_server_reference(response, &blocked_promise, parent_object, key)
}

fn require_server_reference(
    response: &Response,
    reference: &ServerReference,
) -> Option<&'static Unknown> {
    let value = require_module(reference);
    if let Some(object) = js::classify(value).object() {
        // A manifest export can be a fresh, untagged object. Remember its origin
        // so reply property paths cannot read it as if it came from the client.
        let server_reference_objects = match response._server_reference_objects.get() {
            Some(server_reference_objects) => server_reference_objects,
            None => {
                let server_reference_objects = js::weak_set::new();
                response
                    ._server_reference_objects
                    .set(Some(server_reference_objects));
                server_reference_objects
            }
        };
        server_reference_objects.add(object);
    }
    value
}

fn resolve_server_reference_chunk(
    response: &Rc<Response>,
    chunk: &ReactPromise,
    value: Option<&'static Unknown>,
) {
    let resolve_listeners = match &mut *chunk.state.borrow_mut() {
        ChunkState::Blocked { value, .. } => value.take(),
        _ => None,
    };
    *chunk.state.borrow_mut() = ChunkState::Initialized {
        value,
        reason: None,
    };
    if let Some(resolve_listeners) = resolve_listeners {
        wake_chunk(response, resolve_listeners, value, chunk);
    }
}

fn read_server_reference(
    response: &Rc<Response>,
    chunk: &Rc<ReactPromise>,
    parent_object: &'static Unknown,
    key: &str,
) -> Option<&'static Unknown> {
    // GAP rd482: as ReactPromise's then.
    match &*chunk.state.borrow() {
        ChunkState::Initialized { value, .. } => return *value,
        ChunkState::Blocked { .. } => {}
        _ => js::throw(js::get(js::unknown(&**chunk), "reason")),
    }
    wait_for_reference(
        response,
        chunk,
        parent_object,
        key,
        None,
        create_model,
        Vec::new(),
    )
}

fn load_server_object_reference(
    response: &Rc<Response>,
    meta_data: Option<&'static Unknown>,
    parent_object: &'static Unknown,
    key: &str,
) -> Option<&'static Unknown> {
    let Kind::String(id) = js::classify(js::get(meta_data, "id")) else {
        return js::null();
    };
    // GAP rd483: as load_server_reference's.
    let meta_data_key: &'static JsObject = unsafe { js::cast(meta_data) };

    // Object references are cached in a separate response-scoped map from
    // function references. The same metadata object can be referenced both ways,
    // and each path must read only its own cache so it always applies its own
    // validation instead of the other's cached value.
    let cached_promise = response._server_object_reference_cache.get(meta_data_key);
    if let Some(cached_promise) = cached_promise {
        return read_server_reference(response, &cached_promise, parent_object, key);
    }

    let blocked_promise = Rc::new(ReactPromise::new(ChunkState::Blocked {
        value: None,
        reason: None,
    }));
    response
        ._server_object_reference_cache
        .set(meta_data_key, Rc::clone(&blocked_promise));

    let server_reference = resolve_server_reference(response._bundler_config, id);
    let server_reference_promise = preload_module(server_reference);

    let fulfill = {
        let response = Rc::clone(response);
        let blocked_promise = Rc::clone(&blocked_promise);
        move || {
            let value = match catch_unwind(AssertUnwindSafe(|| {
                let value = require_server_reference(&response, server_reference);
                // Objects can appear under "then", but a function here would turn
                // the containing object into a thenable during reply decoding.
                if js::classify(value).object().is_none() {
                    panic!("Expected a Server Reference to an object to resolve to an object.");
                }
                value
            })) {
                Ok(value) => value,
                Err(error) => {
                    let error = error
                        .downcast_ref::<&'static JsError>()
                        .and_then(|error| error.as_unknown());
                    trigger_error_on_chunk(&response, &blocked_promise, error);
                    return;
                }
            };
            resolve_server_reference_chunk(&response, &blocked_promise, value);
        }
    };
    match server_reference_promise {
        None => fulfill(),
        Some(server_reference_promise) => {
            server_reference_promise.then(move |_| fulfill(), {
                let response = Rc::clone(response);
                let blocked_promise = Rc::clone(&blocked_promise);
                move |error| {
                    trigger_error_on_chunk(&response, &blocked_promise, error);
                }
            });
        }
    }
    read_server_reference(response, &blocked_promise, parent_object, key)
}

fn revive_model(
    response: &Rc<Response>,
    parent_obj: &'static Unknown,
    parent_key: &str,
    value: Option<&'static JSONValue>,
    reference: Option<&str>,
    array_root: Option<&Rc<NestedArrayContext>>,
) -> Option<&'static Unknown> {
    if let Kind::String(value) = js::classify(value) {
        // We can't use .bind here because we need the "this" value.
        return parse_model_string(
            response, parent_obj, parent_key, value, reference, array_root,
        );
    }
    if let Some(object) = js::classify(value).object() {
        if let Some(reference) = reference
            && let Some(temporary_references) = response._temporary_references
        {
            // Store this object's reference in case it's returned later.
            // GAP rd487: the draft's TemporaryReference is a function, the
            // one the server makes; a reply's object is any object.
            register_temporary_reference(
                temporary_references,
                unsafe { js::cast(value) },
                reference,
            );
        }
        if let Kind::Array(items) = js::classify(value) {
            let child_context = match array_root {
                None => {
                    let child_context = Rc::new(NestedArrayContext {
                        count: Cell::new(0.0),
                        fork: Cell::new(false),
                    });
                    response
                        ._root_array_contexts
                        .set(object, Rc::clone(&child_context));
                    child_context
                }
                Some(array_root) => Rc::clone(array_root),
            };
            if items.len() > 1 {
                child_context.fork.set(true);
            }
            bump_array_count(
                &child_context,
                // Number of commas + square brackets
                // value.length - 1 + 2
                items.len() as f64 + 1.0,
                response,
            );
            for (i, item) in items.iter().enumerate() {
                let child_ref =
                    reference.map(|reference| reference.to_owned() + ":" + &i.to_string());
                js::set(
                    object,
                    &i,
                    revive_model(
                        response,
                        object,
                        &i.to_string(),
                        *item,
                        child_ref.as_deref(),
                        Some(&child_context),
                    ),
                );
            }
        } else {
            for key in js::keys_in(object) {
                if HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(object, &key)) {
                    if key == __PROTO__ {
                        js::delete(object, &key);
                        continue;
                    }
                    let child_ref = match reference {
                        Some(reference) if !key.contains(':') => {
                            Some(reference.to_owned() + ":" + &key)
                        }
                        _ => None,
                    };
                    let new_value = revive_model(
                        response,
                        object,
                        &key,
                        js::get(object, &key),
                        child_ref.as_deref(),
                        None, // The array context resets when we're entering a non-array
                    );
                    if !matches!(js::classify(new_value), Kind::Undefined) {
                        js::set(object, &key, new_value);
                    } else {
                        js::delete(object, &key);
                    }
                }
            }
        }
    }
    value
}

pub struct NestedArrayContext {
    // Keeps track of how many slots, bytes or characters are in nested arrays/strings/typed arrays.
    pub count: Cell<f64>,
    // A single child is itself not harmful. There needs to be at least one parent array with more
    // than one child.
    pub fork: Cell<bool>,
}

fn bump_array_count(array_context: &NestedArrayContext, slots: f64, response: &Response) {
    let new_count = array_context.count.get() + slots;
    array_context.count.set(new_count);
    // The limit is what the caller gave, compared as JS compares it: of
    // `null`, `newCount > null`, which is `newCount > 0`.
    let exceeded = match response._array_size_limit {
        Some(array_size_limit) => js::unknown(&new_count) > array_size_limit,
        None => new_count > 0.0,
    };
    if exceeded && array_context.fork.get() {
        panic!(
            "Maximum array nesting exceeded. Large nested arrays can be dangerous. Try adding intermediate objects."
        );
    }
}

/// What a reference's value is made into: a model, a Map, a Set, an
/// iterator, or a Server Reference.
type MapModel =
    fn(&Rc<Response>, Option<&'static Unknown>, &'static Unknown, &str) -> Option<&'static Unknown>;

pub struct InitializationReference {
    pub handler: Rc<InitializationHandler>,
    pub parent_object: &'static Unknown,
    pub key: String,
    pub map: MapModel,
    pub path: Vec<String>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub array_root: Option<Rc<NestedArrayContext>>,
}

pub struct InitializationHandler {
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub chunk: RefCell<Option<Rc<ReactPromise>>>,
    pub value: Cell<Option<&'static Unknown>>,
    // TODO: Split type to make it impossible to treat a thrown value as NestedArrayContext.
    // thrown value if errored, otherwise array context
    pub reason: Cell<Option<&'static Unknown>>,
    // GAP rd488: upstream's one `reason` is a thrown value or, where it's
    // not errored, an array context; an untagged enum can't tell a struct
    // from any thrown value, so the array context is a field of its own.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub array_root: RefCell<Option<Rc<NestedArrayContext>>>,
    pub deps: Cell<i32>,
    pub errored: Cell<bool>,
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::nullable)]
    static INITIALIZING_HANDLER: RefCell<Option<Rc<InitializationHandler>>> = const { RefCell::new(None) };
}

/// Puts back the handler that was initializing, however the model's
/// initialization ends.
struct RestoreInitializingHandler(Option<Rc<InitializationHandler>>);

impl Drop for RestoreInitializingHandler {
    fn drop(&mut self) {
        INITIALIZING_HANDLER.set(self.0.take());
    }
}

fn initialize_model_chunk(chunk: &Rc<ReactPromise>) {
    let (response, id, resolved_model) = match &*chunk.state.borrow() {
        ChunkState::ResolvedModel { value, reason } => {
            (Rc::clone(&reason.response), reason.id, value.clone())
        }
        _ => unreachable!(),
    };
    let _prev_handler = RestoreInitializingHandler(INITIALIZING_HANDLER.take());

    let root_reference = if id == -1.0 {
        None
    } else {
        Some(js::number::to_string(id, Some(16)))
    };

    // We go to the BLOCKED state until we've fully resolved this.
    // We do this before parsing in case we try to initialize the same chunk
    // while parsing the model. Such as in a cyclic reference.
    *chunk.state.borrow_mut() = ChunkState::Blocked {
        value: None,
        reason: None,
    };

    if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
        let raw_model = js::json::parse(&resolved_model).unwrap();

        // The root might not be an array but if it is we want to track the count of entries.
        let array_root = Rc::new(NestedArrayContext {
            count: Cell::new(0.0),
            fork: Cell::new(false),
        });

        let value = revive_model(
            &response,
            js::unknown(js::object::from_entries(vec![(String::new(), raw_model)])),
            "",
            raw_model,
            root_reference.as_deref(),
            Some(&array_root),
        );

        // Invoke any listeners added while resolving this model. I.e. cyclic
        // references. This may or may not fully resolve the model depending on
        // if they were blocked.
        let resolve_listeners = match &mut *chunk.state.borrow_mut() {
            ChunkState::Blocked { value, .. } => value.take(),
            _ => None,
        };
        if let Some(resolve_listeners) = resolve_listeners {
            *chunk.state.borrow_mut() = ChunkState::Blocked {
                value: None,
                reason: None,
            };
            for listener in resolve_listeners {
                match listener {
                    Listener::Function(listener) => listener(value),
                    Listener::Reference(listener) => {
                        fulfill_reference(
                            &response,
                            &listener,
                            value,
                            Some(Rc::clone(&array_root)),
                        );
                    }
                }
            }
        }
        if let Some(initializing_handler) = INITIALIZING_HANDLER.with_borrow(Clone::clone) {
            if initializing_handler.errored.get() {
                js::throw(initializing_handler.reason.get());
            }
            if initializing_handler.deps.get() > 0 {
                // We discovered new dependencies on modules that are not yet resolved.
                // We have to keep the BLOCKED state until they're resolved.
                initializing_handler.value.set(value);
                *initializing_handler.array_root.borrow_mut() = Some(array_root);
                *initializing_handler.chunk.borrow_mut() = Some(Rc::clone(chunk));
                return;
            }
        }
        *chunk.state.borrow_mut() = ChunkState::Initialized {
            value,
            reason: Some(InitializedReason::ArrayRoot(array_root)),
        };
    })) {
        let error = error
            .downcast_ref::<&'static JsError>()
            .and_then(|error| error.as_unknown());
        *chunk.state.borrow_mut() = ChunkState::Errored { reason: error };
    }
}

// Report that any missing chunks in the model is now going to throw this
// error upon read. Also notify any pending promises.
pub fn report_global_error(response: &Rc<Response>, error: &'static Error) {
    response._closed.set(true);
    response._closed_reason.set(Some(js::unknown(error)));
    response._chunks.for_each(|chunk, _, _| {
        // If this chunk was already resolved or errored, it won't
        // trigger an error but if it wasn't then we need to
        // because we won't be getting any new data to resolve it.
        if matches!(*chunk.state.borrow(), ChunkState::Pending { .. }) {
            trigger_error_on_chunk(response, &chunk, Some(js::unknown(error)));
        } else {
            let maybe_controller = match &*chunk.state.borrow() {
                ChunkState::Initialized {
                    reason: Some(InitializedReason::Controller(maybe_controller)),
                    ..
                } => Some(Rc::clone(maybe_controller)),
                _ => None,
            };
            if let Some(maybe_controller) = maybe_controller {
                (maybe_controller.error)(Some(js::unknown(error)));
            }
        }
    });
}

fn get_chunk(response: &Rc<Response>, id: f64) -> Rc<ReactPromise> {
    let chunks = response._chunks;
    let chunk = chunks.get(&id);
    match chunk {
        Some(chunk) => chunk,
        None => {
            let prefix = &response._prefix;
            let key = prefix.clone() + &js::number::to_string(id, None);
            // Check if we have this field in the backing store already.
            let backing_entry = get_backing_string(response._form_data, &key);
            let chunk = if let Some(backing_entry) = backing_entry {
                create_resolved_model_chunk(response, backing_entry, id)
            } else if response._closed.get() {
                // We have already errored the response and we're not going to get
                // anything more streaming in so this will immediately error.
                create_errored_chunk(response, response._closed_reason.get())
            } else {
                // We're still waiting on this entry to stream in.
                create_pending_chunk(response)
            };
            chunks.set(&id, Rc::clone(&chunk));
            chunk
        }
    }
}

fn fulfill_reference(
    response: &Rc<Response>,
    reference: &InitializationReference,
    value: Option<&'static Unknown>,
    array_root: Option<Rc<NestedArrayContext>>,
) {
    let InitializationReference {
        handler,
        parent_object,
        key,
        map,
        path,
        ..
    } = reference;

    let resolved_value = match catch_unwind(AssertUnwindSafe(|| {
        let mut value = value;
        let mut array_root = array_root.clone();
        let mut local_length = 0.0;
        let root_array_contexts = response._root_array_contexts;
        let server_reference_objects = response._server_reference_objects.get();
        for name in path.iter().skip(1) {
            // The server doesn't have any lazy references so we don't expect to go through a Promise.
            if let Some(object) = js::classify(value).object()
                && server_reference_objects
                    .is_none_or(|server_reference_objects| !server_reference_objects.has(object))
                && (js::strict_equals(
                    GET_PROTOTYPE_OF.with(|get_prototype_of| get_prototype_of(value)),
                    OBJECT_PROTOTYPE.with(|prototype| *prototype),
                ) || js::strict_equals(
                    GET_PROTOTYPE_OF.with(|get_prototype_of| get_prototype_of(value)),
                    ARRAY_PROTOTYPE.with(|prototype| *prototype),
                ))
                && HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(object, name))
            {
                value = js::get(object, name);
                if is_array(value) {
                    local_length = 0.0;
                    array_root = root_array_contexts
                        .get(
                            // Our `isArray` typing can't narrow `mixed`
                            unsafe { js::cast(value) },
                        )
                        .or(array_root);
                } else {
                    array_root = None;
                    match js::classify(value) {
                        Kind::String(value) => local_length = js::string::length(value) as f64,
                        Kind::BigInt(value) => {
                            // Estimate the length to avoid expensive toString() calls on large
                            // BigInt values. If the value is too large, we get Infinity, which
                            // will trigger the array size limit error.
                            let n = js::number::from_big_int(value).abs();
                            if n == 0.0 {
                                local_length = 1.0;
                            } else {
                                local_length = n.log10().floor() + 1.0;
                            }
                        }
                        _ => {
                            if js::array_buffer::is_view(value) {
                                let view: &ArrayBufferView = unsafe { js::cast(value) };
                                local_length = view.byte_length();
                            } else {
                                local_length = 0.0;
                            }
                        }
                    }
                }
            } else {
                panic!("Invalid reference.");
            }
        }

        let resolved_value = map(response, value, parent_object, key);

        // Add any array counts to the reference's array root. The value that we're
        // resolving might have deep nesting that we need to resolve.
        if let Some(reference_array_root) = &reference.array_root {
            if let Some(array_root) = array_root {
                if array_root.fork.get() {
                    reference_array_root.fork.set(true);
                }
                bump_array_count(reference_array_root, array_root.count.get(), response);
            } else if local_length > 0.0 {
                bump_array_count(reference_array_root, local_length, response);
            }
        }
        resolved_value
    })) {
        Ok(resolved_value) => resolved_value,
        Err(error) => {
            let error = error
                .downcast_ref::<&'static JsError>()
                .and_then(|error| error.as_unknown());
            reject_reference(response, handler, error);
            return;
        }
    };

    // There are no Elements or Debug Info to transfer here.

    resolve_reference(response, handler, parent_object, key, resolved_value);
}

fn resolve_reference(
    response: &Rc<Response>,
    handler: &InitializationHandler,
    parent_object: &'static Unknown,
    key: &str,
    resolved_value: Option<&'static Unknown>,
) {
    if key != __PROTO__ {
        js::set(parent_object, key, resolved_value);
    }

    // If this is the root object for a model reference, where `handler.value`
    // is a stale `null`, the resolved value can be used directly.
    if key.is_empty() && js::strict_equals(handler.value.get(), js::null::<&Unknown>()) {
        handler.value.set(resolved_value);
    }

    handler.deps.set(handler.deps.get() - 1);

    if handler.deps.get() == 0 {
        let chunk = handler.chunk.borrow().clone();
        let Some(chunk) = chunk else {
            return;
        };
        let resolve_listeners = match &mut *chunk.state.borrow_mut() {
            ChunkState::Blocked { value, .. } => value.take(),
            _ => return,
        };
        // Assuming handler.errored is false.
        let array_root = handler.array_root.borrow().clone();
        *chunk.state.borrow_mut() = ChunkState::Initialized {
            value: handler.value.get(),
            reason: array_root.map(InitializedReason::ArrayRoot),
        };
        if let Some(resolve_listeners) = resolve_listeners {
            wake_chunk(response, resolve_listeners, handler.value.get(), &chunk);
        }
    }
}

fn reject_reference(
    response: &Rc<Response>,
    handler: &InitializationHandler,
    error: Option<&'static Unknown>,
) {
    if handler.errored.get() {
        // We've already errored. We could instead build up an AggregateError
        // but if there are multiple errors we just take the first one like
        // Promise.all.
        return;
    }
    handler.errored.set(true);
    handler.value.set(js::null());
    handler.reason.set(error);
    let chunk = handler.chunk.borrow().clone();
    let Some(chunk) = chunk else {
        return;
    };
    if !matches!(*chunk.state.borrow(), ChunkState::Blocked { .. }) {
        return;
    }
    // There's no debug info to forward in this direction.
    trigger_error_on_chunk(response, &chunk, error);
}

fn wait_for_reference(
    _response: &Rc<Response>,
    referenced_chunk: &ReactPromise,
    parent_object: &'static Unknown,
    key: &str,
    array_root: Option<Rc<NestedArrayContext>>,
    map: MapModel,
    path: Vec<String>,
) -> Option<&'static Unknown> {
    let handler = match INITIALIZING_HANDLER.with_borrow(Clone::clone) {
        Some(handler) => {
            handler.deps.set(handler.deps.get() + 1);
            handler
        }
        None => {
            let handler = Rc::new(InitializationHandler {
                chunk: RefCell::new(None),
                value: Cell::new(js::null()),
                reason: Cell::new(js::null()),
                array_root: RefCell::new(None),
                deps: Cell::new(1),
                errored: Cell::new(false),
            });
            INITIALIZING_HANDLER.set(Some(Rc::clone(&handler)));
            handler
        }
    };

    let reference = Rc::new(InitializationReference {
        handler,
        parent_object,
        key: key.to_owned(),
        map,
        path,
        array_root,
    });

    // Add "listener".
    if let ChunkState::Blocked { value, reason } = &mut *referenced_chunk.state.borrow_mut() {
        value
            .get_or_insert_with(Vec::new)
            .push(Listener::Reference(Rc::clone(&reference)));
        reason
            .get_or_insert_with(Vec::new)
            .push(Listener::Reference(reference));
    }

    // Return a place holder value for now.
    js::null()
}

fn get_outlined_model(
    response: &Rc<Response>,
    reference: &str,
    parent_object: &'static Unknown,
    key: &str,
    reference_array_root: Option<&Rc<NestedArrayContext>>,
    map: MapModel,
) -> Option<&'static Unknown> {
    let path = js::string::split(reference, ":");
    let id = js::parse_int(&path[0], Some(16));
    let chunk = get_chunk(response, id);
    if matches!(*chunk.state.borrow(), ChunkState::ResolvedModel { .. }) {
        initialize_model_chunk(&chunk);
    }
    // The status might have changed after initialization.
    // GAP rd482: as ReactPromise's then.
    let initialized = match &*chunk.state.borrow() {
        ChunkState::Initialized { value, reason } => Some((*value, reason.clone())),
        _ => None,
    };
    if let Some((value, array_root_or_controller)) = initialized {
        let mut value = value;
        // GAP rd484: upstream tests `'error' in arrayRootOrController`; the
        // controller is told by its `error`'s kind.
        let mut array_root = match array_root_or_controller {
            Some(InitializedReason::Controller(_)) => {
                panic!(
                    "{}",
                    concat!(
                        "Expected an initialized chunk but got an initialized stream chunk instead. ",
                        "This payload may have been submitted by an older version of React."
                    )
                );
            }
            Some(InitializedReason::ArrayRoot(array_root)) => Some(array_root),
            None => None,
        };

        let mut local_length = 0.0;
        let root_array_contexts = response._root_array_contexts;
        let server_reference_objects = response._server_reference_objects.get();
        for name in path.iter().skip(1) {
            if let Some(object) = js::classify(value).object()
                && server_reference_objects
                    .is_none_or(|server_reference_objects| !server_reference_objects.has(object))
                && (js::strict_equals(
                    GET_PROTOTYPE_OF.with(|get_prototype_of| get_prototype_of(value)),
                    OBJECT_PROTOTYPE.with(|prototype| *prototype),
                ) || js::strict_equals(
                    GET_PROTOTYPE_OF.with(|get_prototype_of| get_prototype_of(value)),
                    ARRAY_PROTOTYPE.with(|prototype| *prototype),
                ))
                && HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(object, name))
            {
                value = js::get(object, name);
                if is_array(value) {
                    local_length = 0.0;
                    array_root = root_array_contexts
                        .get(
                            // Our `isArray` typing can't narrow `mixed`
                            unsafe { js::cast(value) },
                        )
                        .or(array_root);
                } else {
                    array_root = None;
                    match js::classify(value) {
                        Kind::String(value) => local_length = js::string::length(value) as f64,
                        Kind::BigInt(value) => {
                            // Estimate the length to avoid expensive toString() calls on large
                            // BigInt values. If the value is too large, we get Infinity, which
                            // will trigger the array size limit error.
                            let n = js::number::from_big_int(value).abs();
                            if n == 0.0 {
                                local_length = 1.0;
                            } else {
                                local_length = n.log10().floor() + 1.0;
                            }
                        }
                        _ => {
                            if js::array_buffer::is_view(value) {
                                let view: &ArrayBufferView = unsafe { js::cast(value) };
                                local_length = view.byte_length();
                            } else {
                                local_length = 0.0;
                            }
                        }
                    }
                }
            } else {
                panic!("Invalid reference.");
            }
        }
        let chunk_value = map(response, value, parent_object, key);

        // Add any array counts to the reference's array root. The value that we're
        // resolving might have deep nesting that we need to resolve.
        if let Some(reference_array_root) = reference_array_root {
            if let Some(array_root) = array_root {
                if array_root.fork.get() {
                    reference_array_root.fork.set(true);
                }
                bump_array_count(reference_array_root, array_root.count.get(), response);
            } else if local_length > 0.0 {
                bump_array_count(reference_array_root, local_length, response);
            }
        }
        // There's no Element nor Debug Info in the ReplyServer so we don't have to check those here.
        return chunk_value;
    }
    if matches!(*chunk.state.borrow(), ChunkState::Blocked { .. }) {
        return wait_for_reference(
            response,
            &chunk,
            parent_object,
            key,
            reference_array_root.cloned(),
            map,
            path,
        );
    }
    if matches!(*chunk.state.borrow(), ChunkState::Pending { .. }) {
        // If we don't have the referenced chunk yet, then this must be a forward reference,
        // which is not allowed.
        panic!("Invalid forward reference.");
    }
    // This is an error. Instead of erroring directly, we're going to encode this on
    // an initialization handler.
    let reason = match &*chunk.state.borrow() {
        ChunkState::Errored { reason } => *reason,
        _ => None,
    };
    match INITIALIZING_HANDLER.with_borrow(Clone::clone) {
        Some(initializing_handler) => {
            initializing_handler.errored.set(true);
            initializing_handler.value.set(js::null());
            initializing_handler.reason.set(reason);
        }
        None => {
            INITIALIZING_HANDLER.set(Some(Rc::new(InitializationHandler {
                chunk: RefCell::new(None),
                value: Cell::new(js::null()),
                reason: Cell::new(reason),
                array_root: RefCell::new(None),
                deps: Cell::new(0),
                errored: Cell::new(true),
            })));
        }
    }
    // Placeholder
    js::null()
}

fn is_server_reference_object(response: &Response, value: Option<&'static Unknown>) -> bool {
    // A Server Reference to an object is opaque: the client may only pass it
    // back to the server, never have its contents read out. The consuming
    // decoders below (Map, Set, iterator) read their initializer array directly,
    // without traversing a property path, so the opacity guard in the path
    // walkers doesn't cover them. They reject a server-owned array up front,
    // before consuming or mutating it, by checking this alongside `isArray`.
    let server_reference_objects = response._server_reference_objects.get();
    if let Some(server_reference_objects) = server_reference_objects
        && let Some(value) = js::classify(value).object()
    {
        return server_reference_objects.has(value);
    }
    false
}

fn create_map(
    response: &Rc<Response>,
    model: Option<&'static Unknown>,
    _parent_object: &'static Unknown,
    _key: &str,
) -> Option<&'static Unknown> {
    if !is_array(model) || is_server_reference_object(response, model) {
        panic!("Invalid Map initializer.");
    }
    if js::strict_equals(js::get(model, "$$consumed"), true) {
        panic!("Already initialized Map.");
    }
    // This needs to come first to prevent the model from being consumed again in case of a cyclic reference.
    js::set(model, "$$consumed", true);
    let entries: &[(&'static Unknown, Option<&'static Unknown>)] = unsafe { js::cast(model) };
    let map = js::map::new(Some(&entries));
    Some(js::unknown(map))
}

fn create_set(
    response: &Rc<Response>,
    model: Option<&'static Unknown>,
    _parent_object: &'static Unknown,
    _key: &str,
) -> Option<&'static Unknown> {
    if !is_array(model) || is_server_reference_object(response, model) {
        panic!("Invalid Set initializer.");
    }
    if js::strict_equals(js::get(model, "$$consumed"), true) {
        panic!("Already initialized Set.");
    }
    // This needs to come first to prevent the model from being consumed again in case of a cyclic reference.
    js::set(model, "$$consumed", true);
    let values: &[&'static Unknown] = unsafe { js::cast(model) };
    let set = js::set::new(Some(&values));
    Some(js::unknown(set))
}

fn extract_iterator(
    response: &Rc<Response>,
    model: Option<&'static Unknown>,
    _parent_object: &'static Unknown,
    _key: &str,
) -> Option<&'static Unknown> {
    if !is_array(model) || is_server_reference_object(response, model) {
        panic!("Invalid Iterator initializer.");
    }
    if js::strict_equals(js::get(model, "$$consumed"), true) {
        panic!("Already initialized Iterator.");
    }
    // This needs to come first to prevent the model from being consumed again in case of a cyclic reference.
    js::set(model, "$$consumed", true);
    // This uses raw Symbols because we're extracting from a native array.
    // GAP rd483: the array's own iterator, a function of any shape, by a
    // cast, so one that isn't throws JS's TypeError, as upstream's call does.
    let iterator: &dyn Fn() -> Option<&'static Unknown> =
        unsafe { js::cast(js::get(model, js::symbol::iterator)) };
    function::call(iterator, Some(&model), ())
}

fn create_model(
    _response: &Rc<Response>,
    model: Option<&'static Unknown>,
    _parent_object: &'static Unknown,
    key: &str,
) -> Option<&'static Unknown> {
    if key == "then" && matches!(js::classify(model), Kind::Function(_)) {
        // This should never happen because we always serialize objects with then-functions
        // as "thenable" which reduces to ReactPromise with no other fields.
        return js::null();
    }
    model
}

fn parse_typed_array(
    response: &Rc<Response>,
    reference: &str,
    // GAP rd486: upstream gives the typed array's class, `new constructor(buffer)`
    // where it's not `ArrayBuffer`; `js::new` of a class held is refused (ADR
    // 1009), so each case gives its constructor's binding.
    constructor: fn(&'static ArrayBuffer) -> &'static Unknown,
    _bytes_per_element: f64,
    parent_object: &'static Unknown,
    parent_key: &str,
    reference_array_root: Option<&Rc<NestedArrayContext>>,
) -> Option<&'static Unknown> {
    let id = js::parse_int(js::string::slice(reference, 2.0, None), Some(16));
    let prefix = &response._prefix;
    let key = prefix.clone() + &js::number::to_string(id, None);
    let chunks = response._chunks;
    if chunks.has(&id) {
        panic!("Already initialized typed array.");
    }
    chunks.set(
        &id,
        // We don't need to put the actual Blob in the chunk,
        // because it shouldn't be accessed by anything else.
        create_errored_chunk(
            response,
            Some(js::unknown(js::error::new(
                Some("Already initialized typed array."),
                None,
            ))),
        ),
    );

    // We should have this backingEntry in the store already because we emitted
    // it before referencing it. It should be a Blob.
    // GAP rd483: upstream's `getBackingBlob(..) as any`: a reference to no
    // Blob reads `arrayBuffer` of `undefined`, JS's TypeError.
    let backing_entry: &Blob =
        unsafe { js::cast(get_backing_blob(response._form_data, &key).map(js::unknown)) };

    let promise = backing_entry.array_buffer();

    // Since loading the buffer is an async operation we'll be blocking the parent
    // chunk.

    let handler = match INITIALIZING_HANDLER.with_borrow(Clone::clone) {
        Some(handler) => {
            handler.deps.set(handler.deps.get() + 1);
            handler
        }
        None => {
            let handler = Rc::new(InitializationHandler {
                chunk: RefCell::new(None),
                value: Cell::new(js::null()),
                reason: Cell::new(js::null()),
                array_root: RefCell::new(None),
                deps: Cell::new(1),
                errored: Cell::new(false),
            });
            INITIALIZING_HANDLER.set(Some(Rc::clone(&handler)));
            handler
        }
    };

    let reject = {
        let response = Rc::clone(response);
        let handler = Rc::clone(&handler);
        move |error: Option<&'static Unknown>| {
            if handler.errored.get() {
                // We've already errored. We could instead build up an AggregateError
                // but if there are multiple errors we just take the first one like
                // Promise.all.
                return;
            }
            handler.errored.set(true);
            handler.value.set(js::null());
            handler.reason.set(error);
            let chunk = handler.chunk.borrow().clone();
            let Some(chunk) = chunk else {
                return;
            };
            if !matches!(*chunk.state.borrow(), ChunkState::Blocked { .. }) {
                return;
            }
            trigger_error_on_chunk(&response, &chunk, error);
        }
    };

    let fulfill = {
        let response = Rc::clone(response);
        let reference_array_root = reference_array_root.cloned();
        let parent_key = parent_key.to_owned();
        let reject = reject.clone();
        move |buffer: &'static ArrayBuffer| {
            if let Err(x) = catch_unwind(AssertUnwindSafe(|| {
                if let Some(reference_array_root) = &reference_array_root {
                    bump_array_count(reference_array_root, buffer.byte_length(), &response);
                }

                let resolved_value = constructor(buffer);

                if key != __PROTO__ {
                    js::set(parent_object, &parent_key, resolved_value);
                }

                // If this is the root object for a model reference, where `handler.value`
                // is a stale `null`, the resolved value can be used directly.
                if parent_key.is_empty()
                    && js::strict_equals(handler.value.get(), js::null::<&Unknown>())
                {
                    handler.value.set(Some(resolved_value));
                }
            })) {
                reject(
                    x.downcast_ref::<&'static JsError>()
                        .and_then(|x| x.as_unknown()),
                );
                return;
            }

            handler.deps.set(handler.deps.get() - 1);

            if handler.deps.get() == 0 {
                let chunk = handler.chunk.borrow().clone();
                let Some(chunk) = chunk else {
                    return;
                };
                let resolve_listeners = match &mut *chunk.state.borrow_mut() {
                    ChunkState::Blocked { value, .. } => value.take(),
                    _ => return,
                };
                *chunk.state.borrow_mut() = ChunkState::Initialized {
                    value: handler.value.get(),
                    // We don't keep an array count for this since it won't be referenced again.
                    // In fact, we don't really need to store this chunk at all.
                    reason: None,
                };
                if let Some(resolve_listeners) = resolve_listeners {
                    wake_chunk(&response, resolve_listeners, handler.value.get(), &chunk);
                }
            }
        }
    };

    let _: js::Promise<()> = promise.then_with(fulfill, move |error| reject(error.as_unknown()));

    js::null()
}

fn resolve_stream(
    response: &Rc<Response>,
    id: f64,
    stream: &'static Unknown,
    controller: Rc<FlightStreamController>,
) {
    let chunks = response._chunks;
    let chunk = create_initialized_stream_chunk(response, stream, Rc::clone(&controller));
    chunks.set(&id, chunk);

    let prefix = &response._prefix;
    // Stream chunks are written under a dedicated namespace so they can be
    // told apart from one-shot chunks at append time.
    let key = prefix.clone() + "S" + &js::number::to_string(id, None);
    let existing_entries = get_stream_chunks(response._form_data, &key);
    let Some(existing_entries) = existing_entries else {
        return;
    };
    for value in existing_entries {
        if js::string::get(&value, 0.0) == Some("C") {
            (controller.close)(&if value == "C" {
                String::from("\"$undefined\"")
            } else {
                js::string::slice(&value, 1.0, None)
            });
        } else {
            (controller.enqueue_model)(&value);
        }
    }
}

fn parse_readable_stream(
    response: &Rc<Response>,
    reference: &str,
    r#type: Option<&'static str>,
    _parent_object: &'static Unknown,
    _parent_key: &str,
) -> &'static ReadableStream {
    let id = js::parse_int(js::string::slice(reference, 2.0, None), Some(16));
    let chunks = response._chunks;
    if chunks.has(&id) {
        panic!("Already initialized stream.");
    }

    let controller: Rc<Cell<Option<&'static ReadableStreamDefaultController>>> =
        Rc::new(Cell::new(None));
    // GAP rd488: a byte stream's controller is another type, which nothing
    // tells from a default one but `instanceof`, of a global a host may lack
    // (jest's has neither), so it's a variable of its own.
    let byte_controller: Rc<Cell<Option<&'static ReadableByteStreamController>>> =
        Rc::new(Cell::new(None));
    let closed = Rc::new(Cell::new(false));
    // GAP rd488: upstream's `new ReadableStream({type: type, start(c) {
    // controller = c; }})` gives the stream's type as a value; webapi's
    // sources are a byte source, its `type` "bytes", or a default one of
    // none, each of its own controller.
    let stream = match r#type {
        Some(r#type) => readable_stream::new(
            Some(&UnderlyingByteSource {
                r#type,
                auto_allocate_chunk_size: None,
                start: Some(&|c| byte_controller.set(Some(c))),
                pull: None,
                cancel: None,
            }),
            None,
        ),
        None => readable_stream::new(
            Some(&UnderlyingDefaultSource {
                start: Some(&|c| controller.set(Some(c))),
                ..Default::default()
            }),
            None,
        ),
    };
    let previous_blocked_chunk: Rc<RefCell<Option<Rc<ReactPromise>>>> = Rc::new(RefCell::new(None));
    // GAP rd488: upstream's flightController is an object literal of three
    // methods, `chunk.then(enqueue, flightController.error)` reading its own
    // `error`; it's a struct of closures, its `error` made first.
    let error: Rc<Listen> = Rc::new({
        let controller = Rc::clone(&controller);
        let byte_controller = Rc::clone(&byte_controller);
        let closed = Rc::clone(&closed);
        let previous_blocked_chunk = Rc::clone(&previous_blocked_chunk);
        move |error| {
            if closed.get() {
                return;
            }
            closed.set(true);
            let blocked_chunk = previous_blocked_chunk.borrow().clone();
            let error = error.map(|error| error as &dyn js::IntoUnknown);
            match blocked_chunk {
                None => match byte_controller.get() {
                    Some(byte_controller) => byte_controller.error(error),
                    None => controller.get().unwrap().error(error),
                },
                Some(blocked_chunk) => {
                    // We shouldn't get any more enqueues after this so we can set it back to null.
                    *previous_blocked_chunk.borrow_mut() = None;
                    let controller = Rc::clone(&controller);
                    let byte_controller = Rc::clone(&byte_controller);
                    blocked_chunk.then(
                        Some(Rc::new(move |_| match byte_controller.get() {
                            Some(byte_controller) => byte_controller.error(error),
                            None => controller.get().unwrap().error(error),
                        })),
                        None,
                    );
                }
            }
        }
    });
    let enqueue: Rc<Listen> = Rc::new({
        let controller = Rc::clone(&controller);
        let byte_controller = Rc::clone(&byte_controller);
        let error = Rc::clone(&error);
        move |value| {
            if r#type == Some("bytes") && !js::array_buffer::is_view(value) {
                error(Some(js::unknown(js::error::new(
                    Some("Invalid data for bytes stream."),
                    None,
                ))));
                return;
            }
            match byte_controller.get() {
                Some(byte_controller) => {
                    let view: webapi::ArrayBufferView = unsafe { js::cast(value) };
                    byte_controller.enqueue(view);
                }
                None => controller
                    .get()
                    .unwrap()
                    .enqueue(value.map(|value| value as &dyn js::IntoUnknown)),
            }
        }
    });
    let flight_controller = Rc::new(FlightStreamController {
        enqueue_model: Rc::new({
            let response = Rc::clone(response);
            let previous_blocked_chunk = Rc::clone(&previous_blocked_chunk);
            let enqueue = Rc::clone(&enqueue);
            let error = Rc::clone(&error);
            move |json| {
                let blocked_chunk = previous_blocked_chunk.borrow().clone();
                match blocked_chunk {
                    None => {
                        // If we're not blocked on any other chunks, we can try to eagerly initialize
                        // this as a fast-path to avoid awaiting them.
                        let chunk = create_resolved_model_chunk(&response, json.to_owned(), -1.0);
                        initialize_model_chunk(&chunk);
                        let initialized_chunk = &chunk;
                        if matches!(
                            *initialized_chunk.state.borrow(),
                            ChunkState::Initialized { .. }
                        ) {
                            let value = match &*initialized_chunk.state.borrow() {
                                ChunkState::Initialized { value, .. } => *value,
                                _ => None,
                            };
                            enqueue(value);
                        } else {
                            chunk.then(Some(Rc::clone(&enqueue)), Some(Rc::clone(&error)));
                            *previous_blocked_chunk.borrow_mut() = Some(chunk);
                        }
                    }
                    Some(blocked_chunk) => {
                        // We're still waiting on a previous chunk so we can't enqueue quite yet.
                        let chunk = create_pending_chunk(&response);
                        chunk.then(Some(Rc::clone(&enqueue)), Some(Rc::clone(&error)));
                        *previous_blocked_chunk.borrow_mut() = Some(Rc::clone(&chunk));
                        let json = json.to_owned();
                        let response = Rc::clone(&response);
                        let previous_blocked_chunk = Rc::clone(&previous_blocked_chunk);
                        blocked_chunk.then(
                            Some(Rc::new(move |_| {
                                if previous_blocked_chunk.borrow().as_ref().is_some_and(
                                    |previous_blocked_chunk| {
                                        Rc::ptr_eq(previous_blocked_chunk, &chunk)
                                    },
                                ) {
                                    // We were still the last chunk so we can now clear the queue and return
                                    // to synchronous emitting.
                                    *previous_blocked_chunk.borrow_mut() = None;
                                }
                                resolve_model_chunk(&response, &chunk, json.clone(), -1.0);
                            })),
                            None,
                        );
                    }
                }
            }
        }),
        close: Rc::new({
            let controller = Rc::clone(&controller);
            let closed = Rc::clone(&closed);
            let previous_blocked_chunk = Rc::clone(&previous_blocked_chunk);
            move |_json| {
                if closed.get() {
                    return;
                }
                closed.set(true);
                let blocked_chunk = previous_blocked_chunk.borrow().clone();
                match blocked_chunk {
                    None => match byte_controller.get() {
                        Some(byte_controller) => byte_controller.close(),
                        None => controller.get().unwrap().close(),
                    },
                    Some(blocked_chunk) => {
                        // We shouldn't get any more enqueues after this so we can set it back to null.
                        *previous_blocked_chunk.borrow_mut() = None;
                        let controller = Rc::clone(&controller);
                        let byte_controller = Rc::clone(&byte_controller);
                        blocked_chunk.then(
                            Some(Rc::new(move |_| match byte_controller.get() {
                                Some(byte_controller) => byte_controller.close(),
                                None => controller.get().unwrap().close(),
                            })),
                            None,
                        );
                    }
                }
            }
        }),
        error,
    });
    resolve_stream(response, id, js::unknown(stream), flight_controller);
    stream
}

#[cfg_attr(rust_js, rust_js::class)]
pub struct FlightIterator {
    pub next: Box<dyn Fn(Option<&'static Unknown>) -> Rc<ReactPromise>>,
}

unsafe impl js::Defined for FlightIterator {}

impl js::Classed for FlightIterator {
    type Constructor = js::AnyConstructor;
}

#[cfg_attr(rust_js, rust_js::class)]
impl FlightIterator {
    fn new(next: Box<dyn Fn(Option<&'static Unknown>) -> Rc<ReactPromise>>) -> Self {
        FlightIterator {
            next,
            // TODO: Add return/throw as options for aborting.
        }
    }
}

js::on_load! {
    // TODO: The iterator could inherit the AsyncIterator prototype which is not exposed as
    // a global but exists as a prototype of an AsyncGenerator. However, it's not needed
    // to satisfy the iterable protocol.
    js::class::<FlightIterator>().set_prototype(js::unknown(js::object::spread(None)));
    // GAP rd480: upstream's `function asyncIterator() { return this; }`: a
    // function `with_this` makes is unnamed, `function () { .. }`.
    js::set(
        js::class::<FlightIterator>().prototype(),
        ASYNC_ITERATOR.with(|async_iterator| *async_iterator),
        function::with_this(|this, ()| {
            // Self referencing iterator.
            this
        }),
    );
}

/// An async iterable of a stream's values, its one method its iterator's
/// maker, keyed by the symbol (rust-js ADR 1051).
struct AsyncIterable {
    #[cfg_attr(rust_js, rust_js::name(ASYNC_ITERATOR))]
    async_iterator: Box<dyn Fn() -> Rc<FlightIterator>>,
}

unsafe impl js::Defined for AsyncIterable {}

fn parse_async_iterable(
    response: &Rc<Response>,
    reference: &str,
    iterator: bool,
    _parent_object: &'static Unknown,
    _parent_key: &str,
) -> &'static Unknown {
    let id = js::parse_int(js::string::slice(reference, 2.0, None), Some(16));
    let chunks = response._chunks;
    if chunks.has(&id) {
        panic!("Already initialized stream.");
    }

    let buffer: Rc<RefCell<Vec<Rc<ReactPromise>>>> = Rc::new(RefCell::new(Vec::new()));
    let closed = Rc::new(Cell::new(false));
    let next_write_index = Rc::new(Cell::new(0));
    let flight_controller = Rc::new(FlightStreamController {
        enqueue_model: Rc::new({
            let response = Rc::clone(response);
            let buffer = Rc::clone(&buffer);
            let next_write_index = Rc::clone(&next_write_index);
            move |value| {
                if next_write_index.get() == buffer.borrow().len() {
                    let chunk = create_resolved_iterator_result_chunk(&response, value, false);
                    buffer.borrow_mut().push(chunk);
                } else {
                    let chunk = Rc::clone(&buffer.borrow()[next_write_index.get()]);
                    resolve_iterator_result_chunk(&response, &chunk, value, false);
                }
                next_write_index.set(next_write_index.get() + 1);
            }
        }),
        close: Rc::new({
            let response = Rc::clone(response);
            let buffer = Rc::clone(&buffer);
            let closed = Rc::clone(&closed);
            let next_write_index = Rc::clone(&next_write_index);
            move |value| {
                if closed.get() {
                    return;
                }
                closed.set(true);
                if next_write_index.get() == buffer.borrow().len() {
                    let chunk = create_resolved_iterator_result_chunk(&response, value, true);
                    buffer.borrow_mut().push(chunk);
                } else {
                    let chunk = Rc::clone(&buffer.borrow()[next_write_index.get()]);
                    resolve_iterator_result_chunk(&response, &chunk, value, true);
                }
                next_write_index.set(next_write_index.get() + 1);
                while next_write_index.get() < buffer.borrow().len() {
                    // In generators, any extra reads from the iterator have the value undefined.
                    let chunk = Rc::clone(&buffer.borrow()[next_write_index.get()]);
                    next_write_index.set(next_write_index.get() + 1);
                    resolve_iterator_result_chunk(&response, &chunk, "\"$undefined\"", true);
                }
            }
        }),
        error: Rc::new({
            let response = Rc::clone(response);
            let buffer = Rc::clone(&buffer);
            let closed = Rc::clone(&closed);
            let next_write_index = Rc::clone(&next_write_index);
            move |error| {
                if closed.get() {
                    return;
                }
                closed.set(true);
                if next_write_index.get() == buffer.borrow().len() {
                    let chunk = create_pending_chunk(&response);
                    buffer.borrow_mut().push(chunk);
                }
                while next_write_index.get() < buffer.borrow().len() {
                    let chunk = Rc::clone(&buffer.borrow()[next_write_index.get()]);
                    next_write_index.set(next_write_index.get() + 1);
                    trigger_error_on_chunk(&response, &chunk, error);
                }
            }
        }),
    });
    let iterable = AsyncIterable {
        async_iterator: Box::new({
            let response = Rc::clone(response);
            let buffer = Rc::clone(&buffer);
            let closed = Rc::clone(&closed);
            move || -> Rc<FlightIterator> {
                let next_read_index = Rc::new(Cell::new(0));
                let response = Rc::clone(&response);
                let buffer = Rc::clone(&buffer);
                let closed = Rc::clone(&closed);
                Rc::new(FlightIterator::new(Box::new(move |arg| {
                    if !matches!(js::classify(arg), Kind::Undefined) {
                        panic!(
                            "Values cannot be passed to next() of AsyncIterables passed to Client Components."
                        );
                    }
                    if next_read_index.get() == buffer.borrow().len() {
                        if closed.get() {
                            return Rc::new(ReactPromise::new(ChunkState::Initialized {
                                // GAP rd488: `{done: true, value: undefined}`: js's
                                // `IteratorResult` is no value of any shape, and a
                                // struct's `None` field is left out (ADR 0280), so
                                // its `value` key, which `'value' in step` sees, is
                                // an entry's.
                                value: Some(js::unknown(js::object::from_entries(vec![
                                    ("done".to_owned(), Some(js::unknown_of(true))),
                                    ("value".to_owned(), None),
                                ]))),
                                reason: None,
                            }));
                        }
                        let chunk = create_pending_chunk(&response);
                        buffer.borrow_mut().push(chunk);
                    }
                    let chunk = Rc::clone(&buffer.borrow()[next_read_index.get()]);
                    next_read_index.set(next_read_index.get() + 1);
                    chunk
                })))
            }
        }),
    };
    // TODO: If it's a single shot iterator we can optimize memory by cleaning up the buffer after
    // reading through the end, but currently we favor code size over this optimization.
    let stream = if iterator {
        js::unknown_of((iterable.async_iterator)())
    } else {
        js::unknown_of(iterable)
    };
    resolve_stream(response, id, stream, flight_controller);
    stream
}

fn parse_model_string(
    response: &Rc<Response>,
    obj: &'static Unknown,
    key: &str,
    value: &'static str,
    reference: Option<&str>,
    array_root: Option<&Rc<NestedArrayContext>>,
) -> Option<&'static Unknown> {
    if js::string::get(value, 0.0) == Some("$") {
        match js::string::get(value, 1.0) {
            Some("$") => {
                // This was an escaped string value.
                if let Some(array_root) = array_root {
                    bump_array_count(array_root, js::string::length(value) as f64 - 1.0, response);
                }
                return Some(js::unknown_of(js::string::slice(value, 1.0, None)));
            }
            Some("@") => {
                // Promise
                let id = js::parse_int(js::string::slice(value, 2.0, None), Some(16));
                let chunk = get_chunk(response, id);
                return Some(js::unknown_of(chunk));
            }
            Some("h") => {
                // Server Reference
                let r#ref = js::string::slice(value, 2.0, None);
                return get_outlined_model(response, &r#ref, obj, key, None, load_server_reference);
            }
            Some("H") => {
                if ENABLE_FLIGHT_OBJECT_REFERENCES.with(|&enabled| enabled) {
                    // Server Reference to an object. Its metadata carries only an id —
                    // no bound arguments — and it resolves through the same manifest
                    // lookup as a function reference, which returns the module export
                    // without calling it.
                    let r#ref = js::string::slice(value, 2.0, None);
                    return get_outlined_model(
                        response,
                        &r#ref,
                        obj,
                        key,
                        None,
                        load_server_object_reference,
                    );
                }
                return None;
            }
            Some("T") => {
                // Temporary Reference
                let (Some(reference), Some(temporary_references)) =
                    (reference, response._temporary_references)
                else {
                    panic!(
                        "{}",
                        concat!(
                            "Could not reference an opaque temporary reference. ",
                            "This is likely due to misconfiguring the temporaryReferences options ",
                            "on the server."
                        )
                    );
                };
                return Some(js::unknown(create_temporary_reference(
                    temporary_references,
                    reference,
                )));
            }
            Some("Q") => {
                // Map
                let r#ref = js::string::slice(value, 2.0, None);
                return get_outlined_model(response, &r#ref, obj, key, None, create_map);
            }
            Some("W") => {
                // Set
                let r#ref = js::string::slice(value, 2.0, None);
                return get_outlined_model(response, &r#ref, obj, key, None, create_set);
            }
            Some("K") => {
                // FormData
                let string_id = js::string::slice(value, 2.0, None);

                // Use the special marker from the Client to distinguish keys that should
                // be consumed by referenced FormData.
                let form_prefix = response._prefix.clone() + "_" + &string_id + "_";
                return Some(js::unknown(consume_backing_entries(
                    response._form_data,
                    &form_prefix,
                )));
            }
            Some("i") => {
                // Iterator
                let r#ref = js::string::slice(value, 2.0, None);
                return get_outlined_model(response, &r#ref, obj, key, None, extract_iterator);
            }
            Some("I") => {
                // $Infinity
                return Some(js::unknown_of(f64::INFINITY));
            }
            Some("-") => {
                // $-0 or $-Infinity
                if value == "$-0" {
                    return Some(js::unknown_of(-0.0));
                } else {
                    return Some(js::unknown_of(f64::NEG_INFINITY));
                }
            }
            Some("N") => {
                // $NaN
                return Some(js::unknown_of(f64::NAN));
            }
            Some("u") => {
                // matches "$undefined"
                // Special encoding for `undefined` which can't be serialized as JSON otherwise.
                return None;
            }
            Some("D") => {
                // Date
                return Some(js::unknown(js::date::new(Some(&js::date::parse(
                    &js::string::slice(value, 2.0, None),
                )))));
            }
            Some("n") => {
                // BigInt
                let big_int_str = js::string::slice(value, 2.0, None);
                if js::string::length(&big_int_str) > MAX_BIGINT_DIGITS {
                    panic!(
                        "BigInt is too large. Received {} digits but the limit is {}.",
                        js::string::length(&big_int_str),
                        MAX_BIGINT_DIGITS
                    );
                }
                if let Some(array_root) = array_root {
                    bump_array_count(
                        array_root,
                        js::string::length(&big_int_str) as f64,
                        response,
                    );
                }
                return Some(js::unknown(js::big_int(big_int_str.as_str())));
            }
            Some("A") => {
                return parse_typed_array(
                    response,
                    value,
                    |buffer| js::unknown(buffer),
                    1.0,
                    obj,
                    key,
                    array_root,
                );
            }
            Some("O") => {
                return parse_typed_array(
                    response,
                    value,
                    |buffer| js::unknown(js::int8_array::new_with_buffer(buffer, None, None)),
                    1.0,
                    obj,
                    key,
                    array_root,
                );
            }
            Some("o") => {
                return parse_typed_array(
                    response,
                    value,
                    |buffer| js::unknown(js::uint8_array::new_with_buffer(buffer, None, None)),
                    1.0,
                    obj,
                    key,
                    array_root,
                );
            }
            Some("U") => {
                return parse_typed_array(
                    response,
                    value,
                    |buffer| {
                        js::unknown(js::uint8_clamped_array::new_with_buffer(buffer, None, None))
                    },
                    1.0,
                    obj,
                    key,
                    array_root,
                );
            }
            Some("S") => {
                return parse_typed_array(
                    response,
                    value,
                    |buffer| js::unknown(js::int16_array::new_with_buffer(buffer, None, None)),
                    2.0,
                    obj,
                    key,
                    array_root,
                );
            }
            Some("s") => {
                return parse_typed_array(
                    response,
                    value,
                    |buffer| js::unknown(js::uint16_array::new_with_buffer(buffer, None, None)),
                    2.0,
                    obj,
                    key,
                    array_root,
                );
            }
            Some("L") => {
                return parse_typed_array(
                    response,
                    value,
                    |buffer| js::unknown(js::int32_array::new_with_buffer(buffer, None, None)),
                    4.0,
                    obj,
                    key,
                    array_root,
                );
            }
            Some("l") => {
                return parse_typed_array(
                    response,
                    value,
                    |buffer| js::unknown(js::uint32_array::new_with_buffer(buffer, None, None)),
                    4.0,
                    obj,
                    key,
                    array_root,
                );
            }
            Some("G") => {
                return parse_typed_array(
                    response,
                    value,
                    |buffer| js::unknown(js::float32_array::new_with_buffer(buffer, None, None)),
                    4.0,
                    obj,
                    key,
                    array_root,
                );
            }
            Some("g") => {
                return parse_typed_array(
                    response,
                    value,
                    |buffer| js::unknown(js::float64_array::new_with_buffer(buffer, None, None)),
                    8.0,
                    obj,
                    key,
                    array_root,
                );
            }
            Some("M") => {
                return parse_typed_array(
                    response,
                    value,
                    |buffer| js::unknown(js::big_int64_array::new_with_buffer(buffer, None, None)),
                    8.0,
                    obj,
                    key,
                    array_root,
                );
            }
            Some("m") => {
                return parse_typed_array(
                    response,
                    value,
                    |buffer| js::unknown(js::big_uint64_array::new_with_buffer(buffer, None, None)),
                    8.0,
                    obj,
                    key,
                    array_root,
                );
            }
            Some("V") => {
                return parse_typed_array(
                    response,
                    value,
                    |buffer| js::unknown(js::data_view::new(buffer, None, None)),
                    1.0,
                    obj,
                    key,
                    array_root,
                );
            }
            Some("B") => {
                // Blob
                let id = js::parse_int(js::string::slice(value, 2.0, None), Some(16));
                let prefix = &response._prefix;
                let blob_key = prefix.clone() + &js::number::to_string(id, None);
                // We should have this backingEntry in the store already because we emitted
                // it before referencing it. It should be a Blob.
                let backing_entry = get_backing_blob(response._form_data, &blob_key);
                if !js::instance_of(backing_entry, js::class::<Blob>()) {
                    panic!("Referenced Blob is not a Blob.");
                }
                return backing_entry.map(js::unknown);
            }
            Some("R") => {
                return Some(js::unknown(parse_readable_stream(
                    response, value, None, obj, key,
                )));
            }
            Some("r") => {
                return Some(js::unknown(parse_readable_stream(
                    response,
                    value,
                    Some("bytes"),
                    obj,
                    key,
                )));
            }
            Some("X") => {
                return Some(parse_async_iterable(response, value, false, obj, key));
            }
            Some("x") => {
                return Some(parse_async_iterable(response, value, true, obj, key));
            }
            _ => {}
        }
        // We assume that anything else is a reference ID.
        let r#ref = js::string::slice(value, 1.0, None);
        return get_outlined_model(response, &r#ref, obj, key, array_root, create_model);
    }
    if let Some(array_root) = array_root {
        bump_array_count(array_root, js::string::length(value) as f64, response);
    }
    Some(js::unknown(value))
}

const DEFAULT_MAX_ARRAY_NESTING: f64 = 1000000.0;

// Limit BigInt size to prevent CPU exhaustion from parsing very large values.
// 300 digits covers most practical use cases (even 512-bit integers need only
// ~154 digits) and aligns with the implicit limit from the Number approximation
// checks in fulfillReference and getOutlinedModel.
const MAX_BIGINT_DIGITS: u32 = 300;

pub const MAX_BOUND_ARGS: usize = 1000;

pub fn create_response(
    bundler_config: &'static ServerManifest,
    form_field_prefix: &str,
    temporary_references: Option<&'static TemporaryReferenceSet>,
    backing_form_data: Option<&'static Unknown>,
    array_size_limit: Option<&'static Unknown>,
) -> Rc<Response> {
    // Upstream's default parameters, `backingFormData?: FormData = new
    // FormData()` and `arraySizeLimit?: number = DEFAULT_MAX_ARRAY_NESTING`,
    // default `undefined` alone: a `null` is kept.
    let backing_form_data = match js::classify(backing_form_data) {
        Kind::Undefined => Some(js::unknown(form_data::new(None, None))),
        _ => backing_form_data,
    };
    let array_size_limit = match js::classify(array_size_limit) {
        Kind::Undefined => Some(js::unknown(&DEFAULT_MAX_ARRAY_NESTING)),
        _ => array_size_limit,
    };
    let chunks: &'static Map<f64, Rc<ReactPromise>> = js::map::new(None);

    Rc::new(Response {
        _bundler_config: bundler_config,
        _prefix: form_field_prefix.to_owned(),
        // GAP rd483: the form data as given, any value, as upstream passes it on.
        _form_data: create_backing_form_data(
            unsafe { js::cast(backing_form_data) },
            form_field_prefix,
        ),
        _chunks: chunks,
        _closed: Cell::new(false),
        _closed_reason: Cell::new(None),
        _temporary_references: temporary_references,
        _server_reference_objects: Cell::new(None),
        _server_reference_cache: js::weak_map::new(),
        _server_object_reference_cache: js::weak_map::new(),
        _root_array_contexts: js::weak_map::new(),
        _array_size_limit: array_size_limit,
    })
}

pub fn resolve_field(response: &Rc<Response>, key: &str, value: &str) {
    let prefix = &response._prefix;
    if key.starts_with(prefix.as_str()) {
        let chunks = response._chunks;
        if js::string::get(key, js::string::length(prefix) as f64) == Some("S") {
            // Stream chunks live in a dedicated namespace so that they can be told
            // apart from one-shot chunks.
            if js::string::length(key) > js::string::length(prefix) + js::string::length("S") {
                let id = js::to_number(js::string::slice(
                    key,
                    (js::string::length(prefix) + js::string::length("S")) as f64,
                    None,
                ));
                let chunk = chunks.get(&id);
                if let Some(chunk) = chunk {
                    // The stream was already referenced so we can enqueue right away.
                    resolve_model_chunk(response, &chunk, value.to_owned(), id);
                    return;
                }
            }
        } else {
            let id = js::to_number(js::string::slice(
                key,
                js::string::length(prefix) as f64,
                None,
            ));
            let chunk = chunks.get(&id);
            if let Some(chunk) = chunk {
                // We were waiting on this key so now we can resolve it.
                resolve_model_chunk(response, &chunk, value.to_owned(), id);
                return;
            }
        }
    }
    // Add this field to the backing store.
    append_backing_entry(response._form_data, FormDataEntryValue::Str(value), key);
}

pub fn resolve_file(response: &Rc<Response>, key: &str, file: &File) {
    // Add this field to the backing store.
    append_backing_entry(response._form_data, FormDataEntryValue::File(file), key);
}

pub struct FileHandle {
    pub chunks: Vec<BlobPart<'static>>,
    pub filename: String,
    pub mime: String,
}

pub fn resolve_file_info(
    _response: &Rc<Response>,
    _key: &str,
    filename: &str,
    mime: &str,
) -> FileHandle {
    FileHandle {
        chunks: Vec::new(),
        filename: filename.to_owned(),
        mime: mime.to_owned(),
    }
}

pub fn resolve_file_chunk(
    _response: &Rc<Response>,
    handle: &mut FileHandle,
    chunk: &'static Uint8Array,
) {
    handle.chunks.push(BlobPart::Uint8Array(chunk));
}

// Node.js before v20 doesn't expose a global File constructor, so fall back
// to a scratch single-entry FormData, whose append/get is O(1).
fn create_file_from_handle(handle: &FileHandle) -> &'static File {
    if js::is_function(file::new) {
        return file::new(
            &handle.chunks,
            &handle.filename,
            Some(FilePropertyBag {
                r#type: Some(&handle.mime),
                ..Default::default()
            }),
        );
    }
    let scratch = form_data::new(None, None);
    scratch.append_with_blob(
        "file",
        blob::new(
            Some(&handle.chunks),
            Some(BlobPropertyBag {
                r#type: Some(&handle.mime),
                endings: None,
            }),
        ),
        &handle.filename,
    );
    // GAP rd483: upstream's `scratch.get('file') as any`.
    match scratch.get("file") {
        Some(FormDataEntryValue::File(file)) => file,
        None | Some(FormDataEntryValue::Str(_)) => unreachable!(),
    }
}

pub fn resolve_file_complete(response: &Rc<Response>, key: &str, handle: &FileHandle) {
    // Add this file to the backing store.
    append_backing_entry(
        response._form_data,
        FormDataEntryValue::File(create_file_from_handle(handle)),
        key,
    );
}

pub fn close(response: &Rc<Response>) {
    // In case there are any remaining unresolved chunks, they won't
    // be resolved now. So we need to issue an error to those.
    // Ideally we should be able to early bail out if we kept a
    // ref count of pending chunks.
    report_global_error(response, js::error::new(Some("Connection closed."), None));
}
