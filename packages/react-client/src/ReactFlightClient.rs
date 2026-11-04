// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::convert::Infallible;
use std::rc::Rc;

use js::{
    FinalizationRegistry, IntoUnknown, Kind, Promise, PropertyDescriptor, Set, SymbolFor, Unknown,
    WeakRef, function, object,
};
use node::js_error;
use shared::environment::__DEV__;
use shared::react_types::{
    JSONValue, ReactAsyncInfo, ReactComponentInfo, ReactDebugInfo, ReactDebugInfoEntry,
    ReactErrorInfoDev, ReactIOInfo, ReactStackTrace, ReactTimeInfo, Thenable,
};
use webapi::{
    ConsoleTask, FormDataEntryValue, ReadableStreamController, UnderlyingSource, console,
    performance,
};

use shared::react_feature_flags::{
    ENABLE_COMPONENT_PERFORMANCE_TRACK, ENABLE_FLIGHT_OBJECT_REFERENCES,
    ENABLE_FLIGHT_WEAK_THENABLES, ENABLE_PROFILER_TIMER,
};

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightClientConfig.js#bindToConsole"
)]
fn bind_to_console(
    method_name: &str,
    args: Vec<Option<&'static Unknown>>,
    bad_name: &str,
) -> &'static dyn Fn() -> Option<&'static Unknown> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightClientConfig.js#checkEvalAvailabilityOnceDev"
)]
fn check_eval_availability_once_dev() {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightClientConfig.js#createStringDecoder"
)]
fn create_string_decoder(preserve_byte_order_mark: bool) -> &'static Unknown {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightClientConfig.js#dispatchHint"
)]
fn dispatch_hint(code: &str, model: Option<&'static Unknown>) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightClientConfig.js#getModuleDebugInfo",
    rust_js::nullable
)]
fn get_module_debug_info(metadata: &'static Unknown) -> Option<ReactDebugInfo> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightClientConfig.js#preloadModule"
)]
fn preload_module(metadata: &'static Unknown) -> Option<Promise<Option<&'static Unknown>>> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightClientConfig.js#prepareDestinationForModule",
    rust_js::nullable(module_loading)
)]
fn prepare_destination_for_module(
    module_loading: Option<&'static Unknown>,
    nonce: Option<&str>,
    metadata: Option<&'static Unknown>,
) {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightClientConfig.js#readFinalStringChunk"
)]
fn read_final_string_chunk(decoder: &'static Unknown, buffer: &js::Uint8Array) -> String {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightClientConfig.js#readPartialStringChunk"
)]
fn read_partial_string_chunk(decoder: &'static Unknown, buffer: &js::Uint8Array) -> String {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightClientConfig.js#requireModule"
)]
fn require_module(metadata: &'static Unknown) -> Option<&'static Unknown> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightClientConfig.js#resolveServerReference"
)]
fn resolve_server_reference(config: &'static Unknown, id: &str) -> &'static Unknown {
    unreachable!()
}

// ReactFlightClientConfig: each renderer's fork (they stay JS, as the
// reconciler's ReactFiberConfig forks do).
#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightClientConfig.js#resolveClientReference"
)]
fn resolve_client_reference(
    bundler_config: Option<&'static Unknown>,
    metadata: Option<&'static Unknown>,
) -> &'static Unknown {
    unreachable!()
}

unsafe extern "Rust" {
    #[link_name = "./src/ReactFlightClientConfig.js#rendererVersion"]
    safe static RENDERER_VERSION: &'static str;
    #[link_name = "./src/ReactFlightClientConfig.js#rendererPackageName"]
    safe static RENDERER_PACKAGE_NAME: &'static str;
}

use crate::react_flight_reply_client::{
    create_bound_server_reference, create_server_object_reference, register_bound_server_reference,
};

use crate::react_flight_temporary_references::{TemporaryReferenceSet, read_temporary_reference};

use crate::react_flight_performance_track::{
    log_component_aborted, log_component_await, log_component_await_aborted,
    log_component_await_errored, log_component_errored, log_component_render,
    log_deduped_component_render, log_io_info, log_io_info_errored, mark_all_tracks_in_order,
};

use shared::react_symbols::{
    ASYNC_ITERATOR, REACT_ELEMENT_TYPE, REACT_FRAGMENT_TYPE, REACT_LAZY_TYPE,
};

use shared::get_component_name_from_type::get_component_name_from_type;

use shared::react_component_info_stack::get_owner_stack_by_component_info_in_dev;

use shared::has_own_property::HAS_OWN_PROPERTY;

use shared::get_prototype_of::GET_PROTOTYPE_OF;

use crate::react_flight_client_dev_tools_hook::inject_internals;

use shared::react_flight_property_access::OMITTED_PROP_ERROR;

#[cfg_attr(rust_js, rust_js::name = "ReactVersion")]
use shared::react_version::REACT_VERSION;

use shared::is_array::is_array;

unsafe extern "Rust" {
    #[link_name = "react#*.__SERVER_INTERNALS_DO_NOT_USE_OR_WARN_USERS_THEY_CANNOT_UPGRADE"]
    safe static SERVER_INTERNALS: Option<&'static Unknown>;
    #[link_name = "react#*.__CLIENT_INTERNALS_DO_NOT_USE_OR_WARN_USERS_THEY_CANNOT_UPGRADE"]
    safe static CLIENT_INTERNALS: Option<&'static Unknown>;
}

thread_local! {
    // TODO: This is an unfortunate hack. We shouldn't feature detect the internals
    // like this. It's just that for now we support the same build of the Flight
    // client both in the RSC environment, in the SSR environments as well as the
    // browser client. We should probably have a separate RSC build. This is DEV
    // only though.
    #[cfg_attr(rust_js, rust_js::name = "ReactSharedInteralsServer")]
    static REACT_SHARED_INTERALS_SERVER: Option<&'static Unknown> = SERVER_INTERNALS;
    #[cfg_attr(rust_js, rust_js::name = "ReactSharedInternals")]
    static REACT_SHARED_INTERNALS: Option<&'static Unknown> =
        js::or(CLIENT_INTERNALS, REACT_SHARED_INTERALS_SERVER.with(|&server| server));
}

pub use crate::react_flight_reply_client::{CallServerCallback, EncodeFormActionCallback};

/// What a stream chunk's `reason` holds once it's fulfilled: the stream's
/// controller, which the rows of its id feed.
pub struct FlightStreamController {
    pub enqueue_value: Box<dyn Fn(Option<&'static Unknown>)>,
    pub enqueue_model: Box<dyn Fn(String)>,
    pub close: Box<dyn Fn(String)>,
    pub error: Box<dyn Fn(Option<&'static Unknown>)>,
}

unsafe impl js::Defined for FlightStreamController {}

type UninitializedModel = String;

pub struct ProfilingResult {
    pub track: Cell<usize>,
    pub end_time: Cell<f64>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub component: Cell<Option<&'static ReactComponentInfo>>,
}

unsafe impl js::Defined for ProfilingResult {}

const ROW_ID: u8 = 0;
const ROW_TAG: u8 = 1;
const ROW_LENGTH: u8 = 2;
const ROW_CHUNK_BY_NEWLINE: u8 = 3;
const ROW_CHUNK_BY_LENGTH: u8 = 4;

type RowParserState = u8;

// The statuses a thenable's `status` is read for; a chunk's are its state's
// tags.
const INITIALIZED: &str = "fulfilled";
const ERRORED: &str = "rejected";

const __PROTO__: &str = "__proto__";

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "ObjectPrototype")]
    static OBJECT_PROTOTYPE: &'static Unknown = js::object::prototype;
    #[cfg_attr(rust_js, rust_js::name = "ArrayPrototype")]
    static ARRAY_PROTOTYPE: &'static Unknown = js::array::prototype;
    // Fake symbol type.
    static RESPONSE_SYMBOL: &'static js::Symbol = js::symbol::new(None);
}

/// A listener on a pending or blocked chunk: a function `then` was given, or
/// a reference of a model waiting on the chunk.
#[cfg_attr(rust_js, rust_js::untagged)]
#[derive(Clone)]
pub enum Listener {
    Callback(Rc<dyn Fn(Option<&'static Unknown>)>),
    Reference(Rc<InitializationReference>),
}

/// Profiling-only: the chunks a chunk's model referenced, until its
/// performance is flushed, then what that gave.
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum Children {
    Chunks(RefCell<Vec<Chunk>>),
    Flushed(Rc<ProfilingResult>),
}

/// A chunk's state, upstream's `SomeChunk<T>`: its `status` and what its
/// `value` and `reason` hold in that status.
#[cfg_attr(rust_js, rust_js::tag = "status")]
pub enum ChunkState {
    #[cfg_attr(rust_js, rust_js::name = "pending")]
    Pending {
        #[cfg_attr(rust_js, rust_js::nullable)]
        value: Option<Vec<Listener>>,
        #[cfg_attr(rust_js, rust_js::nullable)]
        reason: Option<Vec<Listener>>,
    },
    // A weak Promise reference. Behaves like PENDING except that when the stream
    // closes it transitions to HALTED instead of erroring, because the server
    // may intentionally never emit it. Only used when enableFlightWeakThenables
    // is on.
    #[cfg_attr(rust_js, rust_js::name = "pending_weak")]
    PendingWeak {
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
    ResolvedModel {
        value: UninitializedModel,
        reason: Rc<ResponseHolder>,
    },
    #[cfg_attr(rust_js, rust_js::name = "resolved_module")]
    ResolvedModule {
        value: &'static Unknown,
        #[cfg_attr(rust_js, rust_js::nullable)]
        reason: Option<Infallible>,
    },
    #[cfg_attr(rust_js, rust_js::name = "fulfilled")]
    Initialized {
        value: Option<&'static Unknown>,
        // A stream's controller.
        #[cfg_attr(rust_js, rust_js::nullable)]
        reason: Option<Rc<FlightStreamController>>,
    },
    #[cfg_attr(rust_js, rust_js::name = "rejected")]
    Errored { reason: Option<&'static Unknown> },
    // Means it never resolves, even when the connection closes. The shared
    // terminal state of a weak chunk that didn't settle before close, of any
    // pending chunk at close when partial streams are allowed, and of DEV-only
    // debug halts.
    #[cfg_attr(rust_js, rust_js::name = "halted")]
    Halted {
        #[cfg_attr(rust_js, rust_js::nullable)]
        value: Option<Infallible>,
        #[cfg_attr(rust_js, rust_js::nullable)]
        reason: Option<Infallible>,
    },
}

impl js::Classed for ReactPromise {
    type Constructor = js::AnyConstructor;
}

#[cfg_attr(rust_js, rust_js::class)]
pub struct ReactPromise {
    #[cfg_attr(rust_js, rust_js::flatten)]
    pub state: RefCell<ChunkState>,
    // Profiling-only
    #[cfg_attr(rust_js, rust_js::optional)]
    pub _children: RefCell<Option<Children>>,
    // DEV-only
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub _debug_chunk: RefCell<Option<Chunk>>,
    // DEV-only. A live array: a lazy node forwards it.
    #[cfg_attr(rust_js, rust_js::optional)]
    pub _debug_info: RefCell<Option<Rc<RefCell<ReactDebugInfo>>>>,
    // DEV-only
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub _received_debug_info: Cell<Option<&'static Set<ReactDebugInfoEntry>>>,
}

unsafe impl js::Defined for ReactPromise {}

#[cfg_attr(rust_js, rust_js::class)]
impl ReactPromise {
    fn new(state: ChunkState) -> Self {
        let chunk = ReactPromise {
            state: RefCell::new(state),
            _children: RefCell::new(None),
            _debug_chunk: RefCell::new(None),
            _debug_info: RefCell::new(None),
            _received_debug_info: Cell::new(None),
        };
        if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK {
            *chunk._children.borrow_mut() = Some(Children::Chunks(RefCell::new(Vec::new())));
        }
        if __DEV__ {
            *chunk._debug_chunk.borrow_mut() = None;
            *chunk._debug_info.borrow_mut() = Some(Rc::new(RefCell::new(Vec::new())));
            chunk._received_debug_info.set(None);
        }
        chunk
    }
}

/// A chunk: JS holds it, so the program holds it counted (ADR 0761's
/// class is a value Rust owns).
type Chunk = Rc<ReactPromise>;

js::on_load! {
    // We subclass Promise.prototype so that we get other methods like .catch
    js::class::<ReactPromise>().set_prototype(object::create(
        Some(js::class::<Promise<Unknown>>().prototype()),
        None,
    ));
}

#[cfg_attr(rust_js, rust_js::class)]
impl ReactPromise {
    // TODO: This doesn't return a new Promise chain unlike the real .then
    // The shadowing `then` must be defined with `Object.defineProperty` instead of
    // assignment. Assignment would throw when `Promise.prototype` is frozen (e.g.
    // by SES lockdown) because assigning over an inherited non-writable property
    // is rejected.
    pub fn then(
        self: &Rc<Self>,
        resolve: Option<Rc<dyn Fn(Option<&'static Unknown>)>>,
        reject: Option<Rc<dyn Fn(Option<&'static Unknown>)>>,
    ) {
        let chunk = self;
        // If we have resolved content, we try to initialize it first which
        // might put us back into one of the other states.
        if matches!(*chunk.state.borrow(), ChunkState::ResolvedModel { .. }) {
            initialize_model_chunk(chunk);
        } else if matches!(*chunk.state.borrow(), ChunkState::ResolvedModule { .. }) {
            initialize_module_chunk(chunk);
        }
        let mut resolve = resolve;
        let mut reject = reject;
        if __DEV__ {
            // Because only native Promises get picked up when we're awaiting we need to wrap
            // this in a native Promise in DEV. This means that these callbacks are no longer sync
            // but the lazy initialization is still sync and the .value can be inspected after,
            // allowing it to be read synchronously anyway.
            let resolve_callback = resolve.clone();
            let reject_callback = reject.clone();
            let wrapper: Rc<Cell<Option<Promise<Option<&'static Unknown>>>>> = Rc::default();
            let wrapper_promise: Promise<Option<&'static Unknown>> =
                js::promise::new(|res, rej| {
                    let (chunk, resolved) = (Rc::clone(self), Rc::clone(&wrapper));
                    resolve = Some(Rc::new(move |value| {
                        js::set(
                            resolved.get().unwrap(),
                            "_debugInfo",
                            chunk._debug_info.borrow().clone(),
                        );
                        res(value);
                    }));
                    let (chunk, rejected) = (Rc::clone(self), Rc::clone(&wrapper));
                    reject = Some(Rc::new(move |reason| {
                        js::set(
                            rejected.get().unwrap(),
                            "_debugInfo",
                            chunk._debug_info.borrow().clone(),
                        );
                        rej(reason);
                    }));
                });
            wrapper.set(Some(wrapper_promise));
            let on_fulfilled = move |value| {
                if let Some(resolve_callback) = resolve_callback {
                    resolve_callback(value);
                }
            };
            match reject_callback {
                Some(reject_callback) => {
                    wrapper_promise.then_with::<(), (), ()>(on_fulfilled, move |reason| {
                        reject_callback(reason.as_unknown())
                    });
                }
                None => {
                    wrapper_promise.then::<(), ()>(on_fulfilled);
                }
            }
        }
        // The status might have changed after initialization.
        let settled = match &mut *chunk.state.borrow_mut() {
            ChunkState::Initialized { value, .. } => Ok(*value),
            ChunkState::Pending { value, reason }
            | ChunkState::PendingWeak { value, reason }
            | ChunkState::Blocked { value, reason } => {
                if let Some(resolve) = resolve {
                    value
                        .get_or_insert_with(Vec::new)
                        .push(Listener::Callback(resolve));
                }
                if let Some(reject) = reject {
                    reason
                        .get_or_insert_with(Vec::new)
                        .push(Listener::Callback(reject));
                }
                return;
            }
            ChunkState::Halted { .. } => return,
            ChunkState::Errored { reason } => Err(*reason),
            ChunkState::ResolvedModel { reason, .. } => {
                Err(Some(js::unknown_of(Rc::clone(reason))))
            }
            ChunkState::ResolvedModule { .. } => Err(None),
        };
        // A callback may read the chunk again, so it's called once the
        // state is no longer borrowed.
        match settled {
            Ok(value) => {
                if let Some(resolve) = resolve {
                    resolve(value);
                }
            }
            Err(reason) => {
                if let Some(reject) = reject {
                    reject(reason);
                }
            }
        }
    }
}

pub type FindSourceMapURLCallback = dyn Fn(&str, &str) -> Option<String>;

pub type DebugChannelCallback = dyn Fn(&str);

pub struct DebugChannel {
    pub has_readable: bool,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub callback: Option<Rc<DebugChannelCallback>>,
}

unsafe impl js::Defined for DebugChannel {}

pub struct ResponseInstance {
    pub _bundler_config: Cell<Option<&'static Unknown>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub _server_reference_config: Cell<Option<&'static Unknown>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub _module_loading: Cell<Option<&'static Unknown>>,
    pub _call_server: Cell<Option<&'static CallServerCallback>>,
    pub _encode_form_action: Cell<Option<&'static EncodeFormActionCallback>>,
    pub _nonce: RefCell<Option<String>>,
    pub _chunks: Cell<Option<&'static js::Map<f64, Chunk>>>,
    pub _string_decoder: Cell<Option<&'static Unknown>>,
    pub _closed: Cell<bool>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub _closed_reason: Cell<Option<&'static Unknown>>,
    pub _allow_partial_stream: Cell<bool>,
    // the set temporary references can be resolved from
    pub _temp_refs: Cell<Option<&'static TemporaryReferenceSet>>,
    // Profiling-only
    #[cfg_attr(rust_js, rust_js::optional)]
    pub _time_origin: Cell<Option<f64>>,
    // Profiling-only,
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub _pending_initial_render: Cell<Option<&'static js::TimeoutId>>,
    // DEV-only
    #[cfg_attr(rust_js, rust_js::optional)]
    pub _pending_chunks: Cell<Option<f64>>,
    // DEV-only
    #[cfg_attr(rust_js, rust_js::optional)]
    pub _weak_response: RefCell<Option<Rc<WeakResponse>>>,
    // DEV-only
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub _debug_root_owner: Cell<Option<&'static ReactComponentInfo>>,
    // DEV-only
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub _debug_root_stack: Cell<Option<&'static js::Error>>,
    // DEV-only
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub _debug_root_task: Cell<Option<&'static ConsoleTask>>,
    // DEV-only
    #[cfg_attr(rust_js, rust_js::optional)]
    pub _debug_start_time: Cell<Option<f64>>,
    // DEV-only
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub _debug_end_time: Cell<Option<f64>>,
    // DEV-only
    #[cfg_attr(rust_js, rust_js::optional)]
    #[cfg_attr(rust_js, rust_js::name = "_debugIOStarted")]
    pub _debug_io_started: Cell<Option<bool>>,
    // DEV-only
    #[cfg_attr(rust_js, rust_js::optional)]
    #[cfg_attr(rust_js, rust_js::name = "_debugFindSourceMapURL")]
    pub _debug_find_source_map_url: Cell<Option<&'static FindSourceMapURLCallback>>,
    // DEV-only
    #[cfg_attr(rust_js, rust_js::optional)]
    pub _debug_channel: RefCell<Option<Rc<DebugChannel>>>,
    // DEV-only
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub _blocked_console: RefCell<Option<Chunk>>,
    // DEV-only
    #[cfg_attr(rust_js, rust_js::optional)]
    pub _replay_console: Cell<Option<bool>>,
    // DEV-only, the requested environment name.
    #[cfg_attr(rust_js, rust_js::optional)]
    pub _root_environment_name: RefCell<Option<String>>,
}

unsafe impl js::Defined for ResponseInstance {}
unsafe impl js::WeakKey for ResponseInstance {}

type Response = ResponseInstance;

// This indirection exists only to clean up DebugChannel when all Lazy References are GC:ed.
// Therefore we only use the indirection in DEV.
pub struct WeakResponse {
    pub weak: &'static WeakRef<Response>,
    // This is null when there are no pending chunks.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub response: Cell<Option<&'static Response>>,
}

unsafe impl js::Defined for WeakResponse {}

#[cfg_attr(rust_js, rust_js::name = "hasGCedResponse")]
fn has_gced_response(weak_response: &WeakResponse) -> bool {
    __DEV__ && weak_response.weak.deref().is_none()
}

fn unwrap_weak_response(weak_response: &WeakResponse) -> &'static Response {
    if __DEV__ {
        let Some(response) = weak_response.weak.deref() else {
            // eslint-disable-next-line react-internal/prod-error-codes
            panic!("We did not expect to receive new data after GC:ing the response.");
        };
        response
    } else {
        // In prod we just use the real Response directly.
        // SAFETY: in production the weak response is the Response itself.
        unsafe { js::cast(Some(js::unknown(weak_response))) }
    }
}

fn get_weak_response(response: &'static Response) -> Rc<WeakResponse> {
    if __DEV__ {
        Rc::clone(response._weak_response.borrow().as_ref().unwrap())
    } else {
        // In prod we just use the real Response directly.
        // SAFETY: in production the weak response is the Response itself.
        unsafe { js::cast(Some(js::unknown(response))) }
    }
}

fn close_debug_channel(debug_channel: Rc<DebugChannel>) {
    if let Some(callback) = &debug_channel.callback {
        callback("");
    }
}

thread_local! {
    // If FinalizationRegistry doesn't exist, we cannot use the debugChannel.
    #[cfg_attr(rust_js, rust_js::nullable)]
    static DEBUG_CHANNEL_REGISTRY: Option<&'static FinalizationRegistry<Rc<DebugChannel>>> =
        if __DEV__ && js::is_function(js::finalization_registry::new::<Rc<DebugChannel>>) {
            Some(js::finalization_registry::new(Box::new(close_debug_channel)))
        } else {
            None
        };
}

fn read_chunk(chunk: &Chunk) -> Option<&'static Unknown> {
    // If we have resolved content, we try to initialize it first which
    // might put us back into one of the other states.
    if matches!(*chunk.state.borrow(), ChunkState::ResolvedModel { .. }) {
        initialize_model_chunk(chunk);
    } else if matches!(*chunk.state.borrow(), ChunkState::ResolvedModule { .. }) {
        initialize_module_chunk(chunk);
    }
    // The status might have changed after initialization.
    match &*chunk.state.borrow() {
        ChunkState::Initialized { value, .. } => *value,
        ChunkState::Pending { .. }
        | ChunkState::PendingWeak { .. }
        | ChunkState::Blocked { .. }
        | ChunkState::Halted { .. } => {
            // eslint-disable-next-line no-throw-literal
            js::throw(Some(js::unknown(chunk)))
        }
        ChunkState::Errored { reason } => js::throw(*reason),
        ChunkState::ResolvedModel { reason, .. } => {
            js::throw(Some(js::unknown_of(Rc::clone(reason))))
        }
        ChunkState::ResolvedModule { .. } => js::throw(js::null::<&Unknown>()),
    }
}

pub fn get_root(weak_response: &WeakResponse) -> Chunk {
    let response = unwrap_weak_response(weak_response);
    get_chunk(response, 0.0)
}

fn create_pending_chunk(response: &'static Response) -> Chunk {
    if __DEV__ {
        // Retain a strong reference to the Response while we wait for the result.
        let pending_chunks = response._pending_chunks.get().unwrap_or(0.0);
        response._pending_chunks.set(Some(pending_chunks + 1.0));
        if pending_chunks == 0.0 {
            response
                ._weak_response
                .borrow()
                .as_ref()
                .unwrap()
                .response
                .set(Some(response));
            if let Some(pending_initial_render) = response._pending_initial_render.get() {
                js::clear_timeout(Some(pending_initial_render));
                response._pending_initial_render.set(None);
            }
        }
    }
    Rc::new(ReactPromise::new(ChunkState::Pending {
        value: None,
        reason: None,
    }))
}

fn create_pending_weak_chunk(_response: &'static Response) -> Chunk {
    // Unlike a regular pending chunk, a weak chunk may never settle, so it
    // doesn't retain a strong reference to the Response while it waits.
    Rc::new(ReactPromise::new(ChunkState::PendingWeak {
        value: None,
        reason: None,
    }))
}

fn release_pending_chunk(response: &'static Response, chunk: &Chunk) {
    if __DEV__ && matches!(*chunk.state.borrow(), ChunkState::Pending { .. }) {
        let pending_chunks = response._pending_chunks.get().unwrap_or(0.0) - 1.0;
        response._pending_chunks.set(Some(pending_chunks));
        if pending_chunks == 0.0 {
            // We're no longer waiting for any more chunks. We can release the strong reference
            // to the response. We'll regain it if we ask for any more data later on.
            response
                ._weak_response
                .borrow()
                .as_ref()
                .unwrap()
                .response
                .set(None);
            // Wait a short period to see if any more chunks get asked for. E.g. by a React render.
            // These chunks might discover more pending chunks.
            // If we don't ask for more then we assume that those chunks weren't blocking initial
            // render and are excluded from the performance track.
            response._pending_initial_render.set(Some(js::set_timeout(
                function::bind(
                    flush_initial_render_performance,
                    Some(&Kind::Null),
                    (response,),
                ),
                Some(100.0),
            )));
        }
    }
}

fn create_halted_chunk(_response: &'static Response) -> Chunk {
    Rc::new(ReactPromise::new(ChunkState::Halted {
        value: None,
        reason: None,
    }))
}

// Transition a chunk to HALTED: it will never resolve, even when the
// connection closes. Clears any listeners to release their closures. Future
// .then() calls on HALTED chunks are no-ops.
fn halt_chunk(response: &'static Response, chunk: &Chunk) {
    release_pending_chunk(response, chunk);
    let halted_chunk = chunk;
    *halted_chunk.state.borrow_mut() = ChunkState::Halted {
        value: None,
        reason: None,
    };
}

fn create_blocked_chunk(_response: &'static Response) -> Chunk {
    Rc::new(ReactPromise::new(ChunkState::Blocked {
        value: None,
        reason: None,
    }))
}

fn create_error_chunk(_response: &'static Response, error: Option<&'static Unknown>) -> Chunk {
    Rc::new(ReactPromise::new(ChunkState::Errored { reason: error }))
}

fn filter_debug_info(response: &'static Response, value: &Chunk) {
    let Some(debug_end_time) = response._debug_end_time.get() else {
        // No end time was defined, so we keep all debug info entries.
        return;
    };

    // Remove any debug info entries after the defined end time. For async info
    // that means we're including anything that was awaited before the end time,
    // but it doesn't need to be resolved before the end time.
    let relative_end_time = debug_end_time - performance.time_origin();
    let mut debug_info = Vec::new();
    let value_debug_info = Rc::clone(value._debug_info.borrow().as_ref().unwrap());
    for &info in value_debug_info.borrow().iter() {
        if let Some(time) = time_of(info)
            && time > relative_end_time
        {
            break;
        }
        debug_info.push(info);
    }
    *value._debug_info.borrow_mut() = Some(Rc::new(RefCell::new(debug_info)));
}

fn prune_debug_info_after_error(response: &'static Response, chunk: &Chunk) {
    let Some(debug_end_time) = response._debug_end_time.get() else {
        return;
    };

    let relative_end_time = debug_end_time - performance.time_origin();
    let debug_info = Rc::clone(chunk._debug_info.borrow().as_ref().unwrap());
    let mut debug_info = debug_info.borrow_mut();
    for i in 0..debug_info.len() {
        let info = debug_info[i];
        if let Some(time) = time_of(info)
            && time > relative_end_time
        {
            // This array may already be attached to the Lazy suspended in Fizz.
            debug_info.truncate(i);
            return;
        }
    }
}

fn move_debug_info_from_chunk_to_inner_value(chunk: &Chunk, value: Option<&'static Unknown>) {
    // Remove the debug info from the initialized chunk, and add it to the inner
    // value instead. This can be a React element, an array, or an uninitialized
    // Lazy.
    let resolved_value = resolve_lazy(value);
    if js::type_of(resolved_value) == "object"
        && let Some(object) = resolved_value
        && (is_array(resolved_value)
            || js::type_of(
                ASYNC_ITERATOR.with(|&async_iterator| js::get(resolved_value, async_iterator)),
            ) == "function"
            || SymbolFor::from_value(js::get(resolved_value, "$$typeof"))
                == Some(REACT_ELEMENT_TYPE)
            || SymbolFor::from_value(js::get(resolved_value, "$$typeof")) == Some(REACT_LAZY_TYPE))
    {
        let debug_info: ReactDebugInfo = chunk
            ._debug_info
            .borrow()
            .as_ref()
            .unwrap()
            .borrow_mut()
            .drain(..)
            .collect();
        let value_debug_info = js::get(resolved_value, "_debugInfo");
        if is_array(value_debug_info) {
            // SAFETY: an array `_debugInfo` is the value's debug info.
            let value_debug_info: &RefCell<ReactDebugInfo> = unsafe { js::cast(value_debug_info) };
            value_debug_info.borrow_mut().splice(0..0, debug_info);
        } else if !object::is_frozen(object) {
            object::define_property(
                object,
                "_debugInfo",
                &PropertyDescriptor {
                    configurable: Some(false),
                    enumerable: Some(false),
                    writable: Some(true),
                    value: Some(&debug_info),
                    get: None,
                    set: None,
                },
            );
        }
        // TODO: If the resolved value is a frozen element (e.g. a client-created
        // element from a temporary reference, or a JSX element exported as a client
        // reference), server debug info is currently dropped because the element
        // can't be mutated. We should probably clone the element so each rendering
        // context gets its own mutable copy with the correct debug info.
    }
}

fn process_chunk_debug_info(
    response: &'static Response,
    chunk: &Chunk,
    value: Option<&'static Unknown>,
) {
    filter_debug_info(response, chunk);
    move_debug_info_from_chunk_to_inner_value(chunk, value);
}

fn wake_chunk(
    response: &'static Response,
    listeners: Vec<Listener>,
    value: Option<&'static Unknown>,
    chunk: &Chunk,
) {
    for listener in listeners {
        match listener {
            Listener::Callback(listener) => listener(value),
            Listener::Reference(listener) => fulfill_reference(response, &listener, value, chunk),
        }
    }

    if __DEV__ {
        process_chunk_debug_info(response, chunk, value);
    }
}

fn reject_chunk(
    response: &'static Response,
    listeners: Vec<Listener>,
    error: Option<&'static Unknown>,
) {
    for listener in listeners {
        match listener {
            Listener::Callback(listener) => listener(error),
            Listener::Reference(listener) => reject_reference(response, &listener.handler, error),
        }
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn resolve_blocked_cycle(
    resolved_chunk: &Chunk,
    reference: &InitializationReference,
) -> Option<Rc<InitializationHandler>> {
    let referenced_chunk = reference.handler.chunk.borrow().clone()?;
    if Rc::ptr_eq(&referenced_chunk, resolved_chunk) {
        // We found the cycle. We can resolve the blocked cycle now.
        return Some(Rc::clone(&reference.handler));
    }
    let resolve_listeners = match &*referenced_chunk.state.borrow() {
        ChunkState::Pending { value, .. }
        | ChunkState::PendingWeak { value, .. }
        | ChunkState::Blocked { value, .. } => value.clone(),
        _ => None,
    };
    if let Some(resolve_listeners) = resolve_listeners {
        for listener in resolve_listeners {
            if let Listener::Reference(listener) = listener {
                let found_handler = resolve_blocked_cycle(resolved_chunk, &listener);
                if found_handler.is_some() {
                    return found_handler;
                }
            }
        }
    }
    None
}

fn wake_chunk_if_initialized(
    response: &'static Response,
    chunk: &Chunk,
    mut resolve_listeners: Vec<Listener>,
    mut reject_listeners: Option<Vec<Listener>>,
) {
    if matches!(*chunk.state.borrow(), ChunkState::Initialized { .. }) {
        let value = initialized_value(chunk);
        wake_chunk(response, resolve_listeners, value, chunk);
    } else if matches!(
        *chunk.state.borrow(),
        ChunkState::Blocked { .. } | ChunkState::Pending { .. }
    ) {
        if matches!(*chunk.state.borrow(), ChunkState::Blocked { .. }) {
            // It is possible that we're blocked on our own chunk if it's a cycle.
            // Before adding back the listeners to the chunk, let's check if it would
            // result in a cycle.
            let mut i = 0;
            while i < resolve_listeners.len() {
                if let Listener::Reference(reference) = &resolve_listeners[i] {
                    let reference = Rc::clone(reference);
                    let cyclic_handler = resolve_blocked_cycle(chunk, &reference);
                    if let Some(cyclic_handler) = cyclic_handler {
                        // This reference points back to this chunk. We can resolve the cycle by
                        // using the value from that handler.
                        fulfill_reference(response, &reference, cyclic_handler.value.get(), chunk);
                        resolve_listeners.remove(i);
                        if let Some(reject_listeners) = &mut reject_listeners {
                            let rejection_idx = reject_listeners.iter().position(|listener| {
                                matches!(listener, Listener::Reference(r) if Rc::ptr_eq(r, &reference))
                            });
                            if let Some(rejection_idx) = rejection_idx {
                                reject_listeners.remove(rejection_idx);
                            }
                        }
                        // The status might have changed after fulfilling the reference.
                        if matches!(*chunk.state.borrow(), ChunkState::Initialized { .. }) {
                            let initialized_chunk = chunk;
                            let value = initialized_value(initialized_chunk);
                            wake_chunk(response, resolve_listeners, value, initialized_chunk);
                            return;
                        } else if matches!(*chunk.state.borrow(), ChunkState::Errored { .. }) {
                            if let Some(reject_listeners) = reject_listeners {
                                let reason = errored_reason(chunk);
                                reject_chunk(response, reject_listeners, reason);
                            }
                            return;
                        }
                        continue;
                    }
                }
                i += 1;
            }
        }
        if let ChunkState::Blocked { value, reason } | ChunkState::Pending { value, reason } =
            &mut *chunk.state.borrow_mut()
        {
            if let Some(value) = value {
                value.extend(resolve_listeners);
            } else {
                *value = Some(resolve_listeners);
            }

            if let Some(reason) = reason {
                if let Some(reject_listeners) = reject_listeners {
                    reason.extend(reject_listeners);
                }
            } else {
                *reason = reject_listeners;
            }
        }
    } else if matches!(*chunk.state.borrow(), ChunkState::Errored { .. })
        && let Some(reject_listeners) = reject_listeners
    {
        let reason = errored_reason(chunk);
        reject_chunk(response, reject_listeners, reason);
    }
}

/// A stream chunk's controller, its `reason` once it's fulfilled.
fn stream_controller(chunk: &Chunk) -> Rc<FlightStreamController> {
    match &*chunk.state.borrow() {
        ChunkState::Initialized {
            reason: Some(controller),
            ..
        } => Rc::clone(controller),
        _ => panic!("Expected a stream chunk."),
    }
}

/// A fulfilled chunk's value, read for a moment.
fn initialized_value(chunk: &Chunk) -> Option<&'static Unknown> {
    match &*chunk.state.borrow() {
        ChunkState::Initialized { value, .. } => *value,
        _ => None,
    }
}

/// A rejected chunk's reason, read for a moment; a halted one's is `null`.
fn errored_reason(chunk: &Chunk) -> Option<&'static Unknown> {
    match &*chunk.state.borrow() {
        ChunkState::Errored { reason } => *reason,
        _ => js::null(),
    }
}

fn trigger_error_on_chunk(
    response: &'static Response,
    chunk: &Chunk,
    error: Option<&'static Unknown>,
) {
    if !matches!(
        *chunk.state.borrow(),
        ChunkState::Pending { .. } | ChunkState::PendingWeak { .. } | ChunkState::Blocked { .. }
    ) {
        // If we get more data to an already resolved ID, we assume that it's
        // a stream chunk since any other row shouldn't have more than one entry.
        let controller = stream_controller(chunk);
        (controller.error)(error);
        return;
    }
    release_pending_chunk(response, chunk);
    let listeners = match &mut *chunk.state.borrow_mut() {
        ChunkState::Pending { reason, .. }
        | ChunkState::PendingWeak { reason, .. }
        | ChunkState::Blocked { reason, .. } => reason.take(),
        _ => None,
    };

    if __DEV__
        && matches!(
            *chunk.state.borrow(),
            ChunkState::Pending { .. } | ChunkState::PendingWeak { .. }
        )
    {
        // Lazily initialize any debug info and block the initializing chunk on any unresolved entries.
        if chunk._debug_chunk.borrow().is_some() {
            let prev_handler = INITIALIZING_HANDLER.take();
            let prev_chunk = INITIALIZING_CHUNK.with_borrow(Clone::clone);
            let cyclic_chunk = chunk;
            *cyclic_chunk.state.borrow_mut() = ChunkState::Blocked {
                value: None,
                reason: None,
            };
            if (ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                && ENABLE_COMPONENT_PERFORMANCE_TRACK)
                || __DEV__
            {
                INITIALIZING_CHUNK.set(Some(Rc::clone(cyclic_chunk)));
            }
            let _restore = RestoreInitializingDebug {
                prev_handler,
                prev_chunk,
            };
            initialize_debug_chunk(response, chunk);
            if let Some(initializing_handler) = INITIALIZING_HANDLER.with_borrow(Clone::clone) {
                if initializing_handler.errored.get() {
                    // Ignore error parsing debug info, we'll report the original error instead.
                } else if initializing_handler.deps.get() > 0.0 {
                    // TODO: Block the resolution of the error until all the debug info has loaded.
                    // We currently don't have a way to throw an error after all dependencies have
                    // loaded because we currently treat errors as immediately cancelling the handler.
                }
            }
        }
    }

    let errored_chunk = chunk;
    *errored_chunk.state.borrow_mut() = ChunkState::Errored { reason: error };
    if __DEV__ {
        prune_debug_info_after_error(response, errored_chunk);
    }
    if let Some(listeners) = listeners {
        reject_chunk(response, listeners, error);
    }
}

fn create_resolved_model_chunk(response: &'static Response, value: UninitializedModel) -> Chunk {
    Rc::new(ReactPromise::new(ChunkState::ResolvedModel {
        value,
        reason: Rc::new(ResponseHolder { response }),
    }))
}

/// A value of an object literal, `null` for none.
fn or_null(value: Option<&'static Unknown>) -> &'static Unknown {
    // SAFETY: `Unknown` is any value, `null` too.
    value.unwrap_or(unsafe { js::cast(js::null::<&Unknown>()) })
}

/// A value of an object literal, `undefined` for none.
fn or_undefined(value: Option<&'static Unknown>) -> &'static Unknown {
    // SAFETY: `Unknown` is any value, `undefined` too.
    value.unwrap_or(unsafe { js::cast(None) })
}

/// A resolved model chunk's `reason`, `{[RESPONSE_SYMBOL]: response}`.
pub struct ResponseHolder {
    #[cfg_attr(rust_js, rust_js::name(RESPONSE_SYMBOL))]
    response: &'static Response,
}

unsafe impl js::Defined for ResponseHolder {}

fn create_resolved_module_chunk(_response: &'static Response, value: &'static Unknown) -> Chunk {
    Rc::new(ReactPromise::new(ChunkState::ResolvedModule {
        value,
        reason: None,
    }))
}

fn create_initialized_text_chunk(_response: &'static Response, value: String) -> Chunk {
    Rc::new(ReactPromise::new(ChunkState::Initialized {
        value: Some(js::unknown_of(value)),
        reason: None,
    }))
}

fn create_initialized_buffer_chunk(_response: &'static Response, value: &'static Unknown) -> Chunk {
    Rc::new(ReactPromise::new(ChunkState::Initialized {
        value: Some(value),
        reason: None,
    }))
}

/// An iterator result, `{done, value}`.
pub struct IteratorResultValue {
    pub done: bool,
    pub value: Option<&'static Unknown>,
}

unsafe impl js::Defined for IteratorResultValue {}

fn create_initialized_iterator_result_chunk(
    _response: &'static Response,
    value: Option<&'static Unknown>,
    done: bool,
) -> Chunk {
    Rc::new(ReactPromise::new(ChunkState::Initialized {
        value: Some(js::unknown_of(IteratorResultValue { done, value })),
        reason: None,
    }))
}

fn create_initialized_stream_chunk(
    response: &'static Response,
    value: &'static Unknown,
    controller: Rc<FlightStreamController>,
) -> Chunk {
    if __DEV__ {
        // Retain a strong reference to the Response while we wait for chunks.
        let pending_chunks = response._pending_chunks.get().unwrap_or(0.0);
        response._pending_chunks.set(Some(pending_chunks + 1.0));
        if pending_chunks == 0.0 {
            response
                ._weak_response
                .borrow()
                .as_ref()
                .unwrap()
                .response
                .set(Some(response));
        }
    }
    // We use the reason field to stash the controller since we already have that
    // field. It's a bit of a hack but efficient.
    Rc::new(ReactPromise::new(ChunkState::Initialized {
        value: Some(value),
        reason: Some(controller),
    }))
}

fn create_resolved_iterator_result_chunk(
    response: &'static Response,
    value: UninitializedModel,
    done: bool,
) -> Chunk {
    // To reuse code as much code as possible we add the wrapper element as part of the JSON.
    let iterator_result_json = (if done {
        "{\"done\":true,\"value\":"
    } else {
        "{\"done\":false,\"value\":"
    })
    .to_owned()
        + &value
        + "}";
    Rc::new(ReactPromise::new(ChunkState::ResolvedModel {
        value: iterator_result_json,
        reason: Rc::new(ResponseHolder { response }),
    }))
}

fn resolve_iterator_result_chunk(
    response: &'static Response,
    chunk: &Chunk,
    value: UninitializedModel,
    done: bool,
) {
    // To reuse code as much code as possible we add the wrapper element as part of the JSON.
    let iterator_result_json = (if done {
        "{\"done\":true,\"value\":"
    } else {
        "{\"done\":false,\"value\":"
    })
    .to_owned()
        + &value
        + "}";
    resolve_model_chunk(response, chunk, iterator_result_json);
}

fn resolve_model_chunk(response: &'static Response, chunk: &Chunk, value: UninitializedModel) {
    if !matches!(
        *chunk.state.borrow(),
        ChunkState::Pending { .. } | ChunkState::PendingWeak { .. }
    ) {
        // If we get more data to an already resolved ID, we assume that it's
        // a stream chunk since any other row shouldn't have more than one entry.
        let controller = stream_controller(chunk);
        (controller.enqueue_model)(value);
        return;
    }
    release_pending_chunk(response, chunk);
    let (resolve_listeners, reject_listeners) = match &mut *chunk.state.borrow_mut() {
        ChunkState::Pending { value, reason } | ChunkState::PendingWeak { value, reason } => {
            (value.take(), reason.take())
        }
        _ => (None, None),
    };
    let resolved_chunk = chunk;
    *resolved_chunk.state.borrow_mut() = ChunkState::ResolvedModel {
        value,
        reason: Rc::new(ResponseHolder { response }),
    };
    if let Some(resolve_listeners) = resolve_listeners {
        // This is unfortunate that we're reading this eagerly if
        // we already have listeners attached since they might no
        // longer be rendered or might not be the highest pri.
        initialize_model_chunk(resolved_chunk);
        // The status might have changed after initialization.
        wake_chunk_if_initialized(response, chunk, resolve_listeners, reject_listeners);
    }
}

fn resolve_module_chunk(response: &'static Response, chunk: &Chunk, value: &'static Unknown) {
    if !matches!(
        *chunk.state.borrow(),
        ChunkState::Pending { .. } | ChunkState::PendingWeak { .. } | ChunkState::Blocked { .. }
    ) {
        // We already resolved. We didn't expect to see this.
        return;
    }
    release_pending_chunk(response, chunk);
    let (resolve_listeners, reject_listeners) = match &mut *chunk.state.borrow_mut() {
        ChunkState::Pending { value, reason }
        | ChunkState::PendingWeak { value, reason }
        | ChunkState::Blocked { value, reason } => (value.take(), reason.take()),
        _ => (None, None),
    };
    let resolved_chunk = chunk;
    *resolved_chunk.state.borrow_mut() = ChunkState::ResolvedModule {
        value,
        reason: None,
    };
    if __DEV__ {
        let debug_info = get_module_debug_info(value);
        if let Some(debug_info) = debug_info {
            // Add to the live set if it was already initialized.
            resolved_chunk
                ._debug_info
                .borrow()
                .as_ref()
                .unwrap()
                .borrow_mut()
                .extend(debug_info);
        }
    }
    if let Some(resolve_listeners) = resolve_listeners {
        initialize_module_chunk(resolved_chunk);
        wake_chunk_if_initialized(response, chunk, resolve_listeners, reject_listeners);
    }
}

/// What a reference does with the value it waited for:
/// `(response, model, parentObject, key) => any`.
type MapFn = fn(
    &'static Response,
    Option<&'static Unknown>,
    &'static Unknown,
    &str,
) -> Option<&'static Unknown>;

pub struct InitializationReference {
    pub handler: Rc<InitializationHandler>,
    pub parent_object: &'static Unknown,
    pub key: String,
    pub map: MapFn,
    pub path: Rc<RefCell<Vec<String>>>,
    // DEV-only
    #[cfg_attr(rust_js, rust_js::optional)]
    pub is_debug: Cell<Option<bool>>,
}

unsafe impl js::Defined for InitializationReference {}

/// A pending or an errored handler (upstream's `PendingInitializationHandler
/// | ErroredInitializationHandler`, told apart by `errored`).
pub struct InitializationHandler {
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub parent: Option<Rc<InitializationHandler>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub chunk: RefCell<Option<Chunk>>,
    pub value: Cell<Option<&'static Unknown>>,
    // The controller of a stream whose chunk is blocked on its debug info. Null
    // for every other model. The error the handler was rejected with.
    pub reason: Cell<Option<&'static Unknown>>,
    pub deps: Cell<f64>,
    pub errored: Cell<bool>,
}

unsafe impl js::Defined for InitializationHandler {}

thread_local! {
    #[cfg_attr(rust_js, rust_js::nullable)]
    static INITIALIZING_HANDLER: RefCell<Option<Rc<InitializationHandler>>> =
        const { RefCell::new(None) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static INITIALIZING_CHUNK: RefCell<Option<Chunk>> = const { RefCell::new(None) };
    static IS_INITIALIZING_DEBUG_INFO: Cell<bool> = const { Cell::new(false) };
}

/// Upstream's `finally { initializingHandler = prevHandler; initializingChunk = prevChunk; }`.
struct RestoreInitializingDebug {
    prev_handler: Option<Rc<InitializationHandler>>,
    prev_chunk: Option<Chunk>,
}

impl Drop for RestoreInitializingDebug {
    fn drop(&mut self) {
        INITIALIZING_HANDLER.set(self.prev_handler.take());
        INITIALIZING_CHUNK.set(self.prev_chunk.take());
    }
}

/// initializeModelChunk's `finally`, whose chunk is restored where it's tracked.
struct RestoreInitializing {
    prev_handler: Option<Rc<InitializationHandler>>,
    prev_chunk: Option<Chunk>,
}

impl Drop for RestoreInitializing {
    fn drop(&mut self) {
        INITIALIZING_HANDLER.set(self.prev_handler.take());
        if (ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK)
            || __DEV__
        {
            INITIALIZING_CHUNK.set(self.prev_chunk.take());
        }
    }
}

/// resolveModule's `finally { initializingHandler = prevHandler; }`.
struct RestoreInitializingHandler {
    prev_handler: Option<Rc<InitializationHandler>>,
}

impl Drop for RestoreInitializingHandler {
    fn drop(&mut self) {
        INITIALIZING_HANDLER.set(self.prev_handler.take());
    }
}

/// Upstream's `finally { isInitializingDebugInfo = prevIsInitializingDebugInfo; }`.
struct RestoreIsInitializingDebugInfo {
    prev_is_initializing_debug_info: bool,
}

impl Drop for RestoreIsInitializingDebugInfo {
    fn drop(&mut self) {
        IS_INITIALIZING_DEBUG_INFO.set(self.prev_is_initializing_debug_info);
    }
}

/// What JS threw, of a caught unwind.
fn thrown(payload: &(dyn std::any::Any + Send)) -> Option<&'static Unknown> {
    payload
        .downcast_ref::<&'static js::JsError>()
        .and_then(|error| error.as_unknown())
}

fn initialize_debug_chunk(response: &'static Response, chunk: &Chunk) {
    let Some(debug_chunk) = chunk._debug_chunk.borrow().clone() else {
        return;
    };
    let debug_info = Rc::clone(chunk._debug_info.borrow().as_ref().unwrap());
    let _restore = RestoreIsInitializingDebugInfo {
        prev_is_initializing_debug_info: IS_INITIALIZING_DEBUG_INFO.get(),
    };
    IS_INITIALIZING_DEBUG_INFO.set(true);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if matches!(
            *debug_chunk.state.borrow(),
            ChunkState::ResolvedModel { .. }
        ) {
            // Find the index of this debug info by walking the linked list.
            let mut idx = debug_info.borrow().len();
            let mut c = debug_chunk._debug_chunk.borrow().clone();
            while let Some(chunk) = c {
                if !matches!(*chunk.state.borrow(), ChunkState::Initialized { .. }) {
                    idx += 1;
                }
                c = chunk._debug_chunk.borrow().clone();
            }
            // Initializing the model for the first time.
            initialize_model_chunk(&debug_chunk);
            let initialized_chunk = debug_chunk;
            if matches!(
                *initialized_chunk.state.borrow(),
                ChunkState::Initialized { .. }
            ) {
                let entry =
                    initialize_debug_info(response, initialized_value(&initialized_chunk).unwrap());
                let mut debug_info = debug_info.borrow_mut();
                if idx >= debug_info.len() {
                    debug_info.resize(idx + 1, None);
                }
                debug_info[idx] = Some(entry);
            } else if matches!(
                *initialized_chunk.state.borrow(),
                ChunkState::Blocked { .. }
                    | ChunkState::Pending { .. }
                    | ChunkState::PendingWeak { .. }
            ) {
                wait_for_reference(
                    &initialized_chunk,
                    js::unknown_of(Rc::clone(&debug_info)),
                    &js::concat_text(idx as f64),
                    response,
                    map_initialize_debug_info,
                    Rc::new(RefCell::new(vec![String::new()])), // path
                    true,
                );
            } else {
                js::throw(errored_reason(&initialized_chunk))
            }
        } else if matches!(*debug_chunk.state.borrow(), ChunkState::Initialized { .. }) {
            // Already done.
        } else if matches!(
            *debug_chunk.state.borrow(),
            ChunkState::Blocked { .. }
                | ChunkState::Pending { .. }
                | ChunkState::PendingWeak { .. }
        ) {
            // Signal to the caller that we need to wait.
            wait_for_reference(
                &debug_chunk,
                js::unknown(object::spread(None)), // noop, since we'll have already added an entry to debug info
                "debug", // noop, but we need it to not be empty string since that indicates the root object
                response,
                map_initialize_debug_info,
                Rc::new(RefCell::new(vec![String::new()])), // path
                true,
            );
        } else {
            js::throw(errored_reason(&debug_chunk))
        }
    }));
    if let Err(error) = result {
        trigger_error_on_chunk(response, chunk, thrown(&*error));
    }
}

/// `initializeDebugInfo` as a reference's map.
fn map_initialize_debug_info(
    response: &'static Response,
    model: Option<&'static Unknown>,
    _parent_object: &'static Unknown,
    _key: &str,
) -> Option<&'static Unknown> {
    Some(initialize_debug_info(response, model.unwrap()))
}

fn initialize_model_chunk(chunk: &Chunk) {
    let prev_handler = INITIALIZING_HANDLER.take();
    let prev_chunk = INITIALIZING_CHUNK.with_borrow(Clone::clone);

    let (resolved_model, response) = match &*chunk.state.borrow() {
        ChunkState::ResolvedModel { value, reason } => (value.clone(), reason.response),
        _ => unreachable!("a resolved model chunk"),
    };

    // We go to the BLOCKED state until we've fully resolved this.
    // We do this before parsing in case we try to initialize the same chunk
    // while parsing the model. Such as in a cyclic reference.
    let cyclic_chunk = chunk;
    *cyclic_chunk.state.borrow_mut() = ChunkState::Blocked {
        value: None,
        reason: None,
    };

    if (ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK)
        || __DEV__
    {
        INITIALIZING_CHUNK.set(Some(Rc::clone(cyclic_chunk)));
    }

    if __DEV__ {
        // Initialize any debug info and block the initializing chunk on any
        // unresolved entries.
        initialize_debug_chunk(response, chunk);
        // TODO: The chunk might have transitioned to ERRORED now.
        // Should we return early if that happens?
    }

    let _restore = RestoreInitializing {
        prev_handler,
        prev_chunk,
    };

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let value = parse_model(response, &resolved_model);
        let handler = INITIALIZING_HANDLER.with_borrow(Clone::clone);
        if let Some(handler) = &handler {
            if handler.errored.get() {
                // The handler errored during the parse, so `cyclicChunk` errors with
                // the same reason. `triggerErrorOnChunk` also rejects the references
                // that nested parses queued on `cyclicChunk` during the parse. No value
                // will arrive for them. `initializeDebugChunk` can have errored
                // `cyclicChunk` before the parse. That error stays.
                if matches!(*cyclic_chunk.state.borrow(), ChunkState::Blocked { .. }) {
                    trigger_error_on_chunk(response, cyclic_chunk, handler.reason.get());
                }
                return;
            }
            // Record on the handler which chunk it completes, and with what value,
            // before the listeners below are examined. `resolveBlockedCycle` walks
            // from a reference to its handler and on to that handler's chunk. A cycle
            // that closes through `cyclicChunk` is only found once `handler.chunk` is
            // `cyclicChunk`.
            handler.value.set(value);
            handler.chunk.replace(Some(Rc::clone(cyclic_chunk)));
        }

        let (listeners, mut reject_listeners) = match &mut *cyclic_chunk.state.borrow_mut() {
            ChunkState::Blocked { value, reason } => (value.take(), reason.take()),
            _ => (None, None),
        };
        if let Some(listeners) = listeners {
            // These listeners were added to `cyclicChunk` while its model was
            // parsing. They come from models that were parsed nested inside that
            // parse and that pointed back at `cyclicChunk`. They fall in two groups.
            //
            // A reference whose handler is transitively waiting on `cyclicChunk` is a
            // cycle. Neither side can complete before the other, so one of them has
            // to accept the value as it is now. The reference receives the current
            // object, and the outstanding references fill it in place.
            //
            // Every other listener waits. Its handler is not waiting on
            // `cyclicChunk`, so it can receive the value once the model has resolved
            // its own references, like any reference into a blocked chunk does.
            // Handing it the value now would complete that handler with an object
            // that still has references outstanding. Function listeners have no
            // handler and always wait.
            let mut cyclic: Option<Vec<Rc<InitializationReference>>> = None;
            let mut deferred: Option<Vec<Listener>> = None;
            for listener in listeners {
                if let Listener::Reference(reference) = &listener
                    && resolve_blocked_cycle(cyclic_chunk, reference).is_some()
                {
                    cyclic
                        .get_or_insert_with(Vec::new)
                        .push(Rc::clone(reference));
                    // A reference is queued for rejection as the same object that is
                    // queued for resolution. It is fulfilled below, so its rejection
                    // entry goes.
                    if let Some(reject_listeners) = &mut reject_listeners {
                        let rejection_idx = reject_listeners.iter().position(|listener| {
                            matches!(listener, Listener::Reference(r) if Rc::ptr_eq(r, reference))
                        });
                        if let Some(rejection_idx) = rejection_idx {
                            reject_listeners.remove(rejection_idx);
                        }
                    }
                } else {
                    deferred.get_or_insert_with(Vec::new).push(listener);
                }
            }
            // Put the deferred listeners back on `cyclicChunk` before any cyclic
            // reference is fulfilled. Fulfilling one can complete `cyclicChunk`, and
            // completion wakes what is queued on it at that moment.
            if deferred.is_some()
                && let ChunkState::Blocked { value, reason } = &mut *cyclic_chunk.state.borrow_mut()
            {
                *value = deferred;
                *reason = reject_listeners;
            }
            if let Some(cyclic) = cyclic {
                for reference in &cyclic {
                    fulfill_reference(response, reference, value, cyclic_chunk);
                }
            }
        }

        if !matches!(*cyclic_chunk.state.borrow(), ChunkState::Blocked { .. }) {
            // Fulfilling a cyclic reference completed `cyclicChunk`, or failed and
            // errored it. A failure rejects the reference's handler. The rejection
            // reaches `cyclicChunk`, directly or through the cycle, because
            // `handler.chunk` is set.
            return;
        }
        if let Some(handler) = &handler
            && handler.deps.get() > 0.0
        {
            // The model references chunks that have not resolved yet. `cyclicChunk`
            // stays BLOCKED, with the deferred listeners queued on it, until the last
            // of those resolves.
            return;
        }
        initialize_blocked_chunk(response, cyclic_chunk, value, None);
    }));
    if let Err(error) = result {
        let errored_chunk = chunk;
        *errored_chunk.state.borrow_mut() = ChunkState::Errored {
            reason: thrown(&*error),
        };
    }
}

fn initialize_module_chunk(chunk: &Chunk) {
    let module = match &*chunk.state.borrow() {
        ChunkState::ResolvedModule { value, .. } => *value,
        _ => unreachable!("a resolved module chunk"),
    };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| require_module(module))) {
        Ok(value) => {
            let initialized_chunk = chunk;
            *initialized_chunk.state.borrow_mut() = ChunkState::Initialized {
                value,
                reason: None,
            };
        }
        Err(error) => {
            let errored_chunk = chunk;
            *errored_chunk.state.borrow_mut() = ChunkState::Errored {
                reason: thrown(&*error),
            };
        }
    }
}

// Report that any missing chunks in the model is now going to throw this
// error upon read. Also notify any pending promises.
pub fn report_global_error(weak_response: &WeakResponse, error: Option<&'static Unknown>) {
    if has_gced_response(weak_response) {
        // Ignore close signal if we are not awaiting any more pending chunks.
        return;
    }
    let response = unwrap_weak_response(weak_response);
    response._closed.set(true);
    response._closed_reason.set(error);
    response._chunks.get().unwrap().for_each(|chunk, _, _| {
        // If this chunk was already resolved or errored, it won't
        // trigger an error but if it wasn't then we need to
        // because we won't be getting any new data to resolve it.
        if matches!(*chunk.state.borrow(), ChunkState::Pending { .. }) {
            trigger_error_on_chunk(response, &chunk, error);
        } else if ENABLE_FLIGHT_WEAK_THENABLES.with(|&enabled| enabled)
            && matches!(*chunk.state.borrow(), ChunkState::PendingWeak { .. })
        {
            // A weak Promise reference may never be emitted by the server. It
            // stays forever pending instead of erroring.
            halt_chunk(response, &chunk);
        } else if matches!(
            *chunk.state.borrow(),
            ChunkState::Initialized {
                reason: Some(_),
                ..
            }
        ) {
            let controller = stream_controller(&chunk);
            (controller.error)(error);
        }
    });
    if __DEV__ {
        let debug_channel = response._debug_channel.borrow().clone();
        if let Some(debug_channel) = debug_channel {
            // If we don't have any more ways of reading data, we don't have to send
            // any more neither. So we close the writable side.
            close_debug_channel(debug_channel);
            *response._debug_channel.borrow_mut() = None;
            // Make sure the debug channel is not closed a second time when the
            // Response gets GC:ed.
            if let Some(debug_channel_registry) = DEBUG_CHANNEL_REGISTRY.with(|&registry| registry)
            {
                debug_channel_registry.unregister(response);
            }
        }
    }
}

fn null_ref_getter() -> Option<&'static Unknown> {
    if __DEV__ {
        return js::null();
    }
    None
}

#[cfg_attr(rust_js, rust_js::name = "getIOInfoTaskName")]
fn get_io_info_task_name(io_info: &ReactIOInfo) -> String {
    let name = io_info.name.get();
    if name.is_empty() {
        "unknown".to_owned()
    } else {
        name.to_owned()
    }
}

fn get_async_info_task_name(async_info: &ReactAsyncInfo) -> String {
    "await ".to_owned() + &get_io_info_task_name(&async_info.awaited)
}

fn get_server_component_task_name(component_info: &ReactComponentInfo) -> String {
    "<".to_owned()
        + if component_info.name.is_empty() {
            "..."
        } else {
            &component_info.name
        }
        + ">"
}

fn get_task_name(r#type: Option<&'static Unknown>) -> String {
    if SymbolFor::from_value(r#type) == Some(REACT_FRAGMENT_TYPE) {
        return "<>".to_owned();
    }
    if js::type_of(r#type) == "function" {
        // This is a function so it must have been a Client Reference that resolved to
        // a function. We use "use client" to indicate that this is the boundary into
        // the client. There should only be one for any given owner chain.
        return "\"use client\"".to_owned();
    }
    if js::type_of(r#type) == "object"
        && r#type.is_some()
        && SymbolFor::from_value(js::get(r#type, "$$typeof")) == Some(REACT_LAZY_TYPE)
    {
        if js::instance_of(js::get(r#type, "_payload"), js::class::<ReactPromise>()) {
            // This is a lazy node created by Flight, i.e. it wraps a chunk. It is
            // probably a client reference. We use the "use client" string to indicate
            // that this is the boundary into the client. There will only be one for
            // any given owner chain.
            return "\"use client\"".to_owned();
        }
        // We don't want to eagerly initialize the initializer in DEV mode so we can't
        // call it to extract the type so we don't know the type of this component.
        return "<...>".to_owned();
    }
    match std::panic::catch_unwind(|| get_component_name_from_type(r#type)) {
        Ok(name) => match name {
            Some(name) if !name.is_empty() => "<".to_owned() + &name + ">",
            _ => "<...>".to_owned(),
        },
        Err(_) => "<...>".to_owned(),
    }
}

fn initialize_element(
    response: &'static Response,
    element: &'static Unknown,
    lazy_node: Option<&'static Unknown>,
) {
    if !__DEV__ {
        return;
    }
    let stack = js::get(element, "_debugStack");
    let owner: Option<&'static ReactComponentInfo> =
        // SAFETY: an element's owner is a component info or null.
        unsafe { js::cast(js::get(element, "_owner")) };
    if owner.is_none() {
        js::set(element, "_owner", response._debug_root_owner.get());
    }
    let mut env = response._root_environment_name.borrow().clone().unwrap();
    if let Some(owner) = owner
        && let Some(owner_env) = &owner.env
    {
        // Interestingly we don't actually have the environment name of where
        // this JSX was created if it doesn't have an owner but if it does
        // it must be the same environment as the owner. We could send it separately
        // but it seems a bit unnecessary for this edge case.
        env = owner_env.clone();
    }
    let mut normalized_stack_trace: Option<&'static Unknown> = js::null();
    if owner.is_none()
        && let Some(root_stack) = response._debug_root_stack.get()
    {
        // We override the stack if we override the owner since the stack where the root JSX
        // was created on the server isn't very useful but where the request was made is.
        normalized_stack_trace = Some(js::unknown(root_stack));
    } else if stack.is_some() {
        // We create a fake stack and then create an Error object inside of it.
        // This means that the stack trace is now normalized into the native format
        // of the browser and the stack frames will have been registered with
        // source mapping information.
        // This can unfortunately happen within a user space callstack which will
        // remain on the stack.
        // SAFETY: a DEV element's `_debugStack` is the server's stack trace until here.
        let stack: &ReactStackTrace = unsafe { js::cast(stack) };
        normalized_stack_trace = Some(js::unknown(CREATE_FAKE_JSX_CALL_STACK_IN_DEV.with(
            // SAFETY: a DEV element's, and it's set in DEV.
            |&f| unsafe { f.unwrap_unchecked() },
        )(response, stack, &env)));
    }
    js::set(element, "_debugStack", normalized_stack_trace);
    let mut task: Option<&'static ConsoleTask> = js::null();
    if SUPPORTS_CREATE_TASK.with(|&supports| supports) && stack.is_some() {
        let create_task_fn = function::bind(
            console::create_task().unwrap(),
            Some(&console),
            (get_task_name(js::get(element, "type")),),
        );
        // SAFETY: as above.
        let stack: &ReactStackTrace = unsafe { js::cast(stack) };
        let call_stack = build_fake_call_stack(response, stack, &env, false, create_task_fn);
        // This owner should ideally have already been initialized to avoid getting
        // user stack frames on the stack.
        let owner_task = match owner {
            None => None,
            Some(owner) => initialize_fake_task(response, js::unknown(owner)),
        };
        task = Some(match owner_task {
            None => {
                let root_task = response._debug_root_task.get();
                if let Some(root_task) = root_task {
                    root_task.run(call_stack)
                } else {
                    call_stack()
                }
            }
            Some(owner_task) => owner_task.run(call_stack),
        });
    }
    js::set(element, "_debugTask", task);

    // This owner should ideally have already been initialized to avoid getting
    // user stack frames on the stack.
    if let Some(owner) = owner {
        initialize_fake_stack(response, js::unknown(owner));
    }

    if let Some(lazy_node) = lazy_node {
        // If the lazy node is initialized, we move its debug info to the inner
        // value.
        // SAFETY: a Flight lazy node's payload is its chunk.
        let payload: Chunk = unsafe { js::cast(js::get(lazy_node, "_payload")) };
        let lazy_debug_info = js::get(lazy_node, "_debugInfo");
        if matches!(*payload.state.borrow(), ChunkState::Initialized { .. })
            && js::truthy(lazy_debug_info)
        {
            // SAFETY: a lazy node's `_debugInfo` is its chunk's live array.
            let lazy_debug_info: &RefCell<ReactDebugInfo> = unsafe { js::cast(lazy_debug_info) };
            let debug_info: ReactDebugInfo = lazy_debug_info.borrow_mut().drain(..).collect();
            let element_debug_info = js::get(element, "_debugInfo");
            if js::truthy(element_debug_info) {
                // SAFETY: an element's `_debugInfo`, set, is its debug info.
                let element_debug_info: &RefCell<ReactDebugInfo> =
                    unsafe { js::cast(element_debug_info) };
                element_debug_info.borrow_mut().splice(0..0, debug_info);
            } else {
                object::define_property(
                    element,
                    "_debugInfo",
                    &PropertyDescriptor {
                        configurable: Some(false),
                        enumerable: Some(false),
                        writable: Some(true),
                        value: Some(&debug_info),
                        get: None,
                        set: None,
                    },
                );
            }
        }
    }

    // TODO: We should be freezing the element but currently, we might write into
    // _debugInfo later. We could move it into _store which remains mutable.
    object::freeze(js::get(element, "props").unwrap());
}

/// A DEV element's `_store`, whose `validated` is non-enumerable.
pub struct ElementStore {}

/// A DEV element as the Flight client makes it, its `ref` defined after.
pub struct DevElement {
    #[cfg_attr(rust_js, rust_js::name = "$$typeof")]
    pub r#typeof: SymbolFor<'static>,
    pub r#type: Option<&'static Unknown>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub key: Option<&'static Unknown>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub props: Option<&'static Unknown>,
    #[cfg_attr(rust_js, rust_js::name = "_owner", rust_js::nullable)]
    pub owner: Option<&'static Unknown>,
}

unsafe impl js::Defined for DevElement {}

/// A production element as the Flight client makes it.
pub struct ProdElement {
    #[cfg_attr(rust_js, rust_js::name = "$$typeof")]
    pub r#typeof: SymbolFor<'static>,
    pub r#type: Option<&'static Unknown>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub key: Option<&'static Unknown>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub r#ref: Option<&'static Unknown>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub props: Option<&'static Unknown>,
}

unsafe impl js::Defined for ProdElement {}

fn create_element(
    response: &'static Response,
    r#type: Option<&'static Unknown>,
    key: Option<&'static Unknown>,
    props: Option<&'static Unknown>,
    owner: Option<&'static Unknown>,     // DEV-only
    stack: Option<&'static Unknown>,     // DEV-only
    validated: Option<&'static Unknown>, // DEV-only
) -> &'static Unknown {
    let element: &'static Unknown = if __DEV__ {
        // `ref` is non-enumerable in dev
        let element = js::unknown_of(DevElement {
            r#typeof: REACT_ELEMENT_TYPE,
            r#type,
            key,
            props,
            owner,
        });
        object::define_property(
            element,
            "ref",
            &PropertyDescriptor {
                configurable: None,
                enumerable: Some(false),
                writable: None,
                value: None,
                get: Some(&null_ref_getter),
                set: None,
            },
        );
        element
    } else {
        js::unknown_of(ProdElement {
            // This tag allows us to uniquely identify this as a React Element
            r#typeof: REACT_ELEMENT_TYPE,

            r#type,
            key,
            r#ref: None,
            props,
        })
    };

    if __DEV__ {
        // We don't really need to add any of these but keeping them for good measure.
        // Unfortunately, _store is enumerable in jest matchers so for equality to
        // work, I need to keep it or make _store non-enumerable in the other file.
        js::set(element, "_store", object::spread(None));
        object::define_property(
            js::get(element, "_store").unwrap(),
            "validated",
            &PropertyDescriptor {
                configurable: Some(false),
                enumerable: Some(false),
                writable: Some(true),
                value: Some(&validated), // Whether the element has already been validated on the server.
                get: None,
                set: None,
            },
        );
        // debugInfo contains Server Component debug information.
        object::define_property(
            element,
            "_debugInfo",
            &PropertyDescriptor {
                configurable: Some(false),
                enumerable: Some(false),
                writable: Some(true),
                value: Some(&Kind::Null),
                get: None,
                set: None,
            },
        );
        object::define_property(
            element,
            "_debugStack",
            &PropertyDescriptor {
                configurable: Some(false),
                enumerable: Some(false),
                writable: Some(true),
                value: Some(&or_null(stack)),
                get: None,
                set: None,
            },
        );
        object::define_property(
            element,
            "_debugTask",
            &PropertyDescriptor {
                configurable: Some(false),
                enumerable: Some(false),
                writable: Some(true),
                value: Some(&Kind::Null),
                get: None,
                set: None,
            },
        );
    }

    if let Some(handler) = INITIALIZING_HANDLER.with_borrow(Clone::clone) {
        // We pop the stack to the previous outer handler before leaving the Element.
        // This is effectively the complete phase.
        INITIALIZING_HANDLER.set(handler.parent.clone());
        if handler.errored.get() {
            // Something errored inside this Element's props. We can turn this Element
            // into a Lazy so that we can still render up until that Lazy is rendered.
            let errored_chunk = create_error_chunk(response, handler.reason.get());
            if __DEV__ {
                initialize_element(response, element, None);
                // Conceptually the error happened inside this Element but right before
                // it was rendered. We don't have a client side component to render but
                // we can add some DebugInfo to explain that this was conceptually a
                // Server side error that errored inside this element. That way any stack
                // traces will point to the nearest JSX that errored - e.g. during
                // serialization.
                let errored_component = Rc::new(ReactComponentInfo {
                    name: get_component_name_from_type(js::get(element, "type"))
                        .unwrap_or_default(),
                    env: None,
                    key: None,
                    // SAFETY: a DEV element's `_owner` is a component info or null.
                    owner: Cell::new(unsafe { js::cast(js::get(element, "_owner")) }),
                    stack: RefCell::new(None),
                    props: None,
                    debug_stack: Cell::new(None),
                    debug_task: Cell::new(None),
                    debug_location: Cell::new(None),
                });
                // SAFETY: a DEV element's `_debugStack` is an Error or null.
                errored_component
                    .debug_stack
                    .set(unsafe { js::cast(js::get(element, "_debugStack")) });
                if SUPPORTS_CREATE_TASK.with(|&supports| supports) {
                    // SAFETY: a DEV element's `_debugTask` is a task or null.
                    errored_component
                        .debug_task
                        .set(unsafe { js::cast(js::get(element, "_debugTask")) });
                }
                *errored_chunk._debug_info.borrow_mut() = Some(Rc::new(RefCell::new(vec![Some(
                    js::unknown_of(errored_component),
                )])));
            }
            return create_lazy_chunk_wrapper(&errored_chunk, validated);
        }
        if handler.deps.get() > 0.0 {
            // We have blocked references inside this Element but we can turn this into
            // a Lazy node referencing this Element to let everything around it proceed.
            let blocked_chunk = create_blocked_chunk(response);
            handler.value.set(Some(element));
            handler.chunk.replace(Some(Rc::clone(&blocked_chunk)));
            let lazy_node = create_lazy_chunk_wrapper(&blocked_chunk, validated);
            if __DEV__ {
                // After we have initialized any blocked references, initialize stack etc.
                let init: Rc<dyn Fn(Option<&'static Unknown>)> = Rc::new(function::bind(
                    initialize_element_listener,
                    Some(&Kind::Null),
                    (response, element, Some(lazy_node)),
                ));
                blocked_chunk.then(Some(Rc::clone(&init)), Some(init));
            }
            return lazy_node;
        }
    }
    if __DEV__ {
        initialize_element(response, element, None);
    }

    element
}

/// `initializeElement` given a listener's value too, as `bind` calls it.
fn initialize_element_listener(
    response: &'static Response,
    element: &'static Unknown,
    lazy_node: Option<&'static Unknown>,
    _value: Option<&'static Unknown>,
) {
    initialize_element(response, element, lazy_node);
}

fn transfer_validation(store: &'static Unknown, value: Option<&'static Unknown>) {
    if js::truthy(js::get(store, "validated")) && js::type_of(value) == "object" && value.is_some()
    {
        // Only elements and lazy nodes carry key validation. Any other value, e.g.
        // an array of children, needs to have its own items validated instead.
        let r#typeof = SymbolFor::from_value(js::get(value, "$$typeof"));
        if r#typeof == Some(REACT_ELEMENT_TYPE) || r#typeof == Some(REACT_LAZY_TYPE) {
            let value_store = js::get(value, "_store");
            if js::truthy(value_store) && !js::truthy(js::get(value_store, "validated")) {
                js::set(value_store, "validated", js::get(store, "validated"));
            }
        }
    }
}

fn read_chunk_and_transfer_validation(
    store: &'static Unknown,
    payload: &Chunk,
) -> Option<&'static Unknown> {
    let value = read_chunk(payload);
    transfer_validation(store, value);
    value
}

fn create_lazy_chunk_wrapper(
    chunk: &Chunk,
    validated: Option<&'static Unknown>, // DEV-only
) -> &'static Unknown {
    let lazy_type = js::unknown(object::from_entries(vec![
        ("$$typeof".to_owned(), js::unknown(&REACT_LAZY_TYPE)),
        ("_payload".to_owned(), js::unknown(chunk)),
        ("_init".to_owned(), js::unknown(&(read_chunk as fn(_) -> _))),
    ]));
    if __DEV__ {
        // Forward the live array
        js::set(lazy_type, "_debugInfo", chunk._debug_info.borrow().clone());
        // Initialize a store for key validation by the JSX runtime. It can only
        // validate the lazy node itself, because the value it refers to might not
        // exist yet at that point, e.g. if it's an outlined row that hasn't been
        // initialized. So the validation is transferred to the value when the lazy
        // node is unwrapped. If the value is another lazy node, unwrapping that one
        // forwards the validation further.
        let store = js::unknown(object::from_entries(vec![(
            "validated".to_owned(),
            or_null(validated),
        )]));
        js::set(lazy_type, "_store", store);
        js::set(
            lazy_type,
            "_init",
            function::bind(
                read_chunk_and_transfer_validation,
                Some(&Kind::Null),
                (store,),
            ),
        );
    }
    lazy_type
}

fn get_chunk(response: &'static Response, id: f64) -> Chunk {
    let chunks = response._chunks.get().unwrap();
    let mut chunk = chunks.get(&id);
    if chunk.is_none() {
        let new_chunk = if response._closed.get() {
            if response._allow_partial_stream.get() {
                // For partial streams, chunks accessed after close should be HALTED
                // (never resolve).
                create_halted_chunk(response)
            } else {
                // We have already errored the response and we're not going to get
                // anything more streaming in so this will immediately error.
                create_error_chunk(response, response._closed_reason.get())
            }
        } else {
            create_pending_chunk(response)
        };
        chunks.set(&id, Rc::clone(&new_chunk));
        chunk = Some(new_chunk);
    }
    chunk.unwrap()
}

// Like getChunk, but for weak Promise references. The server may never emit
// the row for a weak reference, so an unresolved weak chunk halts (stays
// forever pending) instead of erroring when the stream closes.
fn get_weak_chunk(response: &'static Response, id: f64) -> Chunk {
    let chunks = response._chunks.get().unwrap();
    let mut chunk = chunks.get(&id);
    if chunk.is_none() {
        let new_chunk = if response._closed.get() {
            // The stream already closed without emitting this row, so it will
            // never resolve.
            create_halted_chunk(response)
        } else {
            create_pending_weak_chunk(response)
        };
        chunks.set(&id, Rc::clone(&new_chunk));
        chunk = Some(new_chunk);
    }
    chunk.unwrap()
}

// Moves a blocked chunk to INITIALIZED and wakes whatever is waiting on it. The
// other `initialize*Chunk` functions start from a resolved chunk and parse it.
// This one starts from a chunk whose model is already parsed and whose
// references have all resolved.
fn initialize_blocked_chunk(
    response: &'static Response,
    chunk: &Chunk,
    value: Option<&'static Unknown>,
    reason: Option<Rc<FlightStreamController>>,
) {
    let resolve_listeners = match &mut *chunk.state.borrow_mut() {
        ChunkState::Blocked { value, .. } => value.take(),
        _ => None,
    };
    if __DEV__ {
        // Only a blocked chunk receives debug info, so release the set here.
        chunk._received_debug_info.set(None);
    }
    let initialized_chunk = chunk;
    *initialized_chunk.state.borrow_mut() = ChunkState::Initialized { value, reason };
    if let Some(resolve_listeners) = resolve_listeners {
        wake_chunk(response, resolve_listeners, value, initialized_chunk);
    } else if __DEV__ {
        process_chunk_debug_info(response, initialized_chunk, value);
    }
}

// Initializes the handler's chunk once the handler has no references left to
// wait on. `handler.chunk` is null while the handler's model is still parsing.
// `initializeModelChunk` handles that case when the parse ends.
fn initialize_chunk_if_unblocked(response: &'static Response, handler: &InitializationHandler) {
    if handler.deps.get() != 0.0 || handler.errored.get() {
        return;
    }
    let Some(chunk) = handler.chunk.borrow().clone() else {
        return;
    };
    // `BlockedChunk` is the chunk's type at the time the handler recorded it.
    // The chunk can have errored since, or a cyclic reference can have completed
    // it, so its status is read again here.
    if !matches!(*chunk.state.borrow(), ChunkState::Blocked { .. }) {
        return;
    }
    // `handler.reason` is the controller of a stream whose chunk was blocked on
    // its debug info, and null otherwise. It becomes the initialized chunk's
    // `reason` either way.
    // SAFETY: a pending handler's reason is null or its stream's controller.
    let controller: Option<Rc<FlightStreamController>> = unsafe { js::cast(handler.reason.get()) };
    initialize_blocked_chunk(response, &chunk, handler.value.get(), controller);
}

fn fulfill_reference(
    response: &'static Response,
    reference: &Rc<InitializationReference>,
    value: Option<&'static Unknown>,
    fulfilled_chunk: &Chunk,
) {
    let InitializationReference {
        handler,
        parent_object,
        key,
        map,
        path,
        ..
    } = &**reference;
    let parent_object = *parent_object;
    let mut value = value;

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> bool {
        let mut i = 1;
        while i < path.borrow().len() {
            while js::type_of(value) == "object"
                && value.is_some()
                && SymbolFor::from_value(js::get(value, "$$typeof")) == Some(REACT_LAZY_TYPE)
            {
                // We never expect to see a Lazy node on this path because we encode those as
                // separate models. This must mean that we have inserted an extra lazy node
                // e.g. to replace a blocked element. We must instead look for it inside.
                // SAFETY: a Flight lazy node's payload is its chunk.
                let referenced_chunk: Chunk = unsafe { js::cast(js::get(value, "_payload")) };
                if handler
                    .chunk
                    .borrow()
                    .clone()
                    .is_some_and(|chunk| Rc::ptr_eq(&referenced_chunk, &chunk))
                {
                    // This is a reference to the thing we're currently blocking. We can peak
                    // inside of it to get the value.
                    value = handler.value.get();
                    continue;
                }
                if matches!(
                    *referenced_chunk.state.borrow(),
                    ChunkState::ResolvedModel { .. }
                ) {
                    initialize_model_chunk(&referenced_chunk);
                } else if matches!(
                    *referenced_chunk.state.borrow(),
                    ChunkState::ResolvedModule { .. }
                ) {
                    initialize_module_chunk(&referenced_chunk);
                }
                if matches!(
                    *referenced_chunk.state.borrow(),
                    ChunkState::Initialized { .. }
                ) {
                    value = initialized_value(&referenced_chunk);
                    continue;
                } else if matches!(
                    *referenced_chunk.state.borrow(),
                    ChunkState::Blocked { .. }
                        | ChunkState::Pending { .. }
                        | ChunkState::PendingWeak { .. }
                ) {
                    if matches!(*referenced_chunk.state.borrow(), ChunkState::Blocked { .. }) {
                        // It is possible that we're blocked on our own chunk if it's a cycle.
                        // Before adding the listener to the inner chunk, let's check if it would
                        // result in a cycle.
                        let cyclic_handler = resolve_blocked_cycle(&referenced_chunk, reference);
                        if let Some(cyclic_handler) = cyclic_handler {
                            // This reference points back to this chunk. We can resolve the cycle by
                            // using the value from that handler.
                            value = cyclic_handler.value.get();
                            continue;
                        }
                    }
                    // If we're not yet initialized we need to skip what we've already drilled
                    // through and then wait for the next value to become available.
                    path.borrow_mut().drain(0..i - 1);
                    // Add "listener" to our new chunk dependency.
                    if let ChunkState::Blocked { value, reason }
                    | ChunkState::Pending { value, reason }
                    | ChunkState::PendingWeak { value, reason } =
                        &mut *referenced_chunk.state.borrow_mut()
                    {
                        match value {
                            None => *value = Some(vec![Listener::Reference(Rc::clone(reference))]),
                            Some(listeners) => {
                                listeners.push(Listener::Reference(Rc::clone(reference)))
                            }
                        }
                        match reason {
                            None => *reason = Some(vec![Listener::Reference(Rc::clone(reference))]),
                            Some(listeners) => {
                                listeners.push(Listener::Reference(Rc::clone(reference)))
                            }
                        }
                    }
                    return false;
                } else if matches!(*referenced_chunk.state.borrow(), ChunkState::Halted { .. }) {
                    // Do nothing. We couldn't fulfill.
                    // TODO: Mark downstreams as halted too.
                    return false;
                } else {
                    reject_reference(
                        response,
                        &reference.handler,
                        errored_reason(&referenced_chunk),
                    );
                    return false;
                }
            }
            let name = path.borrow()[i].clone();
            if js::type_of(value) == "object"
                && value.is_some()
                && HAS_OWN_PROPERTY
                    .with(|has_own_property| has_own_property.call(value.unwrap(), &*name))
            {
                value = js::get(value, &*name);
            } else {
                panic!("Invalid reference.");
            }
            i += 1;
        }

        while js::type_of(value) == "object"
            && value.is_some()
            && SymbolFor::from_value(js::get(value, "$$typeof")) == Some(REACT_LAZY_TYPE)
        {
            // If what we're referencing is a Lazy it must be because we inserted one as a virtual node
            // while it was blocked by other data. If it's no longer blocked, we can unwrap it.
            // SAFETY: a Flight lazy node's payload is its chunk.
            let referenced_chunk: Chunk = unsafe { js::cast(js::get(value, "_payload")) };
            if handler
                .chunk
                .borrow()
                .clone()
                .is_some_and(|chunk| Rc::ptr_eq(&referenced_chunk, &chunk))
            {
                // This is a reference to the thing we're currently blocking. We can peak
                // inside of it to get the value.
                value = handler.value.get();
                continue;
            }
            if matches!(
                *referenced_chunk.state.borrow(),
                ChunkState::ResolvedModel { .. }
            ) {
                initialize_model_chunk(&referenced_chunk);
            } else if matches!(
                *referenced_chunk.state.borrow(),
                ChunkState::ResolvedModule { .. }
            ) {
                initialize_module_chunk(&referenced_chunk);
            }
            if matches!(
                *referenced_chunk.state.borrow(),
                ChunkState::Initialized { .. }
            ) {
                value = initialized_value(&referenced_chunk);
                continue;
            }
            break;
        }

        let mapped_value = map(response, value, parent_object, key);
        if key != __PROTO__ {
            js::set(parent_object, &**key, mapped_value);
        }

        // If this is the root object for a model reference, where `handler.value`
        // is a stale `null`, the resolved value can be used directly.
        if key.is_empty() && js::strict_equals(handler.value.get(), js::null::<&Unknown>()) {
            handler.value.set(mapped_value);
        }

        // If the parent object is an unparsed React element tuple, we also need to
        // update the props and owner of the parsed element object (i.e.
        // handler.value).
        let handler_value = handler.value.get();
        if SymbolFor::from_value(js::get(parent_object, &0.0)) == Some(REACT_ELEMENT_TYPE)
            && js::type_of(handler_value) == "object"
            && let Some(element) = handler_value
            && SymbolFor::from_value(js::get(handler_value, "$$typeof")) == Some(REACT_ELEMENT_TYPE)
        {
            match key.as_str() {
                "3" => {
                    if __DEV__ && reference.is_debug.get() != Some(true) {
                        transfer_referenced_debug_info(
                            handler.chunk.borrow().clone(),
                            fulfilled_chunk,
                        );
                    }
                    js::set(element, "props", mapped_value);
                }
                "4" => {
                    // This path doesn't call transferReferencedDebugInfo because this reference is to a debug chunk.
                    if __DEV__ {
                        js::set(element, "_owner", mapped_value);
                    }
                }
                "5" => {
                    // This path doesn't call transferReferencedDebugInfo because this reference is to a debug chunk.
                    if __DEV__ {
                        js::set(element, "_debugStack", mapped_value);
                    }
                }
                _ => {
                    if __DEV__ && reference.is_debug.get() != Some(true) {
                        transfer_referenced_debug_info(
                            handler.chunk.borrow().clone(),
                            fulfilled_chunk,
                        );
                    }
                }
            }
        } else if __DEV__ && reference.is_debug.get() != Some(true) {
            transfer_referenced_debug_info(handler.chunk.borrow().clone(), fulfilled_chunk);
        }
        true
    }));
    match result {
        Ok(true) => {}
        Ok(false) => return,
        Err(error) => {
            reject_reference(response, &reference.handler, thrown(&*error));
            return;
        }
    }

    handler.deps.set(handler.deps.get() - 1.0);
    initialize_chunk_if_unblocked(response, handler);
}

fn reject_reference(
    response: &'static Response,
    handler: &InitializationHandler,
    error: Option<&'static Unknown>,
) {
    if handler.errored.get() {
        // We've already errored. We could instead build up an AggregateError
        // but if there are multiple errors we just take the first one like
        // Promise.all.
        return;
    }
    let blocked_value = handler.value.get();
    let errored_handler = handler;
    errored_handler.errored.set(true);
    errored_handler.value.set(js::null());
    errored_handler.reason.set(error);
    let chunk = handler.chunk.borrow().clone();
    let Some(chunk) =
        chunk.filter(|chunk| matches!(*chunk.state.borrow(), ChunkState::Blocked { .. }))
    else {
        return;
    };

    if __DEV__
        && js::type_of(blocked_value) == "object"
        && let Some(element) = blocked_value
        && SymbolFor::from_value(js::get(blocked_value, "$$typeof")) == Some(REACT_ELEMENT_TYPE)
    {
        // Conceptually the error happened inside this Element but right before
        // it was rendered. We don't have a client side component to render but
        // we can add some DebugInfo to explain that this was conceptually a
        // Server side error that errored inside this element. That way any stack
        // traces will point to the nearest JSX that errored - e.g. during
        // serialization.
        let errored_component = Rc::new(ReactComponentInfo {
            name: get_component_name_from_type(js::get(element, "type")).unwrap_or_default(),
            env: None,
            key: None,
            // SAFETY: a DEV element's `_owner` is a component info or null.
            owner: Cell::new(unsafe { js::cast(js::get(element, "_owner")) }),
            stack: RefCell::new(None),
            props: None,
            debug_stack: Cell::new(None),
            debug_task: Cell::new(None),
            debug_location: Cell::new(None),
        });
        // SAFETY: a DEV element's `_debugStack` is an Error or null.
        errored_component
            .debug_stack
            .set(unsafe { js::cast(js::get(element, "_debugStack")) });
        if SUPPORTS_CREATE_TASK.with(|&supports| supports) {
            // SAFETY: a DEV element's `_debugTask` is a task or null.
            errored_component
                .debug_task
                .set(unsafe { js::cast(js::get(element, "_debugTask")) });
        }
        chunk
            ._debug_info
            .borrow()
            .as_ref()
            .unwrap()
            .borrow_mut()
            .push(Some(js::unknown_of(errored_component)));
    }

    trigger_error_on_chunk(response, &chunk, error);
}

fn wait_for_reference(
    referenced_chunk: &Chunk,
    parent_object: &'static Unknown,
    key: &str,
    response: &'static Response,
    map: MapFn,
    path: Rc<RefCell<Vec<String>>>,
    is_awaiting_debug_info: bool, // DEV-only
) -> Option<&'static Unknown> {
    if __DEV__
        && response
            ._debug_channel
            .borrow()
            .as_ref()
            .is_none_or(|debug_channel| !debug_channel.has_readable)
        && matches!(*referenced_chunk.state.borrow(), ChunkState::Pending { .. })
        && SymbolFor::from_value(js::get(parent_object, &0.0)) == Some(REACT_ELEMENT_TYPE)
        && (key == "4" || key == "5")
    {
        // If the parent object is an unparsed React element tuple, and this is a reference
        // to the owner or debug stack. Then we expect the chunk to have been emitted earlier
        // in the stream. It might be blocked on other things but chunk should no longer be pending.
        // If it's still pending that suggests that it was referencing an object in the debug
        // channel, but no debug channel was wired up so it's missing. In this case we can just
        // drop the debug info instead of halting the whole stream.
        return js::null();
    }

    let handler = if let Some(handler) = INITIALIZING_HANDLER.with_borrow(Clone::clone) {
        handler.deps.set(handler.deps.get() + 1.0);
        handler
    } else {
        let handler = Rc::new(InitializationHandler {
            parent: None,
            chunk: RefCell::new(None),
            value: Cell::new(js::null()),
            reason: Cell::new(js::null()),
            deps: Cell::new(1.0),
            errored: Cell::new(false),
        });
        INITIALIZING_HANDLER.set(Some(Rc::clone(&handler)));
        handler
    };

    let reference = Rc::new(InitializationReference {
        handler,
        parent_object,
        key: key.to_owned(),
        map,
        path,
        is_debug: Cell::new(None),
    });
    if __DEV__ {
        reference.is_debug.set(Some(is_awaiting_debug_info));
    }

    // Add "listener".
    if let ChunkState::Blocked { value, reason }
    | ChunkState::Pending { value, reason }
    | ChunkState::PendingWeak { value, reason } = &mut *referenced_chunk.state.borrow_mut()
    {
        match value {
            None => *value = Some(vec![Listener::Reference(Rc::clone(&reference))]),
            Some(listeners) => listeners.push(Listener::Reference(Rc::clone(&reference))),
        }
        match reason {
            None => *reason = Some(vec![Listener::Reference(Rc::clone(&reference))]),
            Some(listeners) => listeners.push(Listener::Reference(Rc::clone(&reference))),
        }
    }

    // Return a place holder value for now.
    js::null()
}

fn load_server_reference(
    response: &'static Response,
    meta_data: Option<&'static Unknown>,
    parent_object: &'static Unknown,
    key: &str,
) -> Option<&'static Unknown> {
    let meta_data = meta_data.unwrap();
    if key == "then" {
        // This should never happen because we always serialize objects with then-functions
        // as "thenable" which reduces to a chunk with no other fields.
        return js::null();
    }
    let id = js::get(meta_data, "id");
    let Kind::String(id) = js::classify(id) else {
        return js::null();
    };
    let id = id.to_owned();
    let Some(server_reference_config) = response._server_reference_config.get() else {
        // In the normal case, we can't load this Server Reference in the current environment and
        // we just return a proxy to it.
        // SAFETY: a server reference's metadata is a BoundServerReferenceMetaData.
        let meta_data = unsafe { js::cast(Some(meta_data)) };
        return Some(js::unknown(create_bound_server_reference(
            meta_data,
            response._call_server.get().unwrap(),
            response._encode_form_action.get(),
            if __DEV__ {
                response._debug_find_source_map_url.get()
            } else {
                None
            },
        )));
    };
    // If we have a module mapping we can load the real version of this Server Reference.
    let server_reference = resolve_server_reference(server_reference_config, &id);

    let bound = js::get(meta_data, "bound");
    let bound_is_chunk = js::instance_of(bound, js::class::<ReactPromise>());
    let mut promise: Option<Promise<Option<&'static Unknown>>> = preload_module(server_reference);
    match promise {
        None => {
            if bound_is_chunk {
                promise = Some(js::promise::resolve(bound));
            } else {
                // We only await the bound arguments when they are one of our own chunks.
                // Anything else could be a foreign thenable whose `then` we must not call.
                let resolved_value = require_module(server_reference);
                register_bound_server_reference(
                    // SAFETY: a server reference's module is a function.
                    unsafe { js::cast(resolved_value) },
                    // SAFETY: a server reference's metadata has its id.
                    unsafe { js::cast(js::get(meta_data, "id")) },
                    // SAFETY: its bound arguments are a thenable, or null.
                    unsafe { js::cast(bound) },
                    response._encode_form_action.get(),
                );
                return resolved_value;
            }
        }
        Some(preloaded) if bound_is_chunk => {
            // SAFETY: a chunk is a thenable, which Promise.all takes as a promise.
            let all = js::promise::all2((preloaded, unsafe {
                js::cast::<Promise<Option<&'static Unknown>>>(bound)
            }));
            promise = Some(unsafe { js::cast(Some(js::unknown(&all))) });
        }
        Some(_) => {}
    }

    let handler = if let Some(handler) = INITIALIZING_HANDLER.with_borrow(Clone::clone) {
        handler.deps.set(handler.deps.get() + 1.0);
        handler
    } else {
        let handler = Rc::new(InitializationHandler {
            parent: None,
            chunk: RefCell::new(None),
            value: Cell::new(js::null()),
            reason: Cell::new(js::null()),
            deps: Cell::new(1.0),
            errored: Cell::new(false),
        });
        INITIALIZING_HANDLER.set(Some(Rc::clone(&handler)));
        handler
    };
    let key = key.to_owned();

    let fulfill = {
        let handler = Rc::clone(&handler);
        let key = key.clone();
        move |_: Option<&'static Unknown>| {
            let mut resolved_value = require_module(server_reference);

            if js::truthy(bound) {
                // This promise is coming from us and should have initialized by now.
                let promise_value = js::get(bound, "value");
                let bound_args: Vec<Option<&'static Unknown>> = if is_array(promise_value) {
                    // SAFETY: an array is a list of values.
                    unsafe { js::cast::<&Vec<Option<&'static Unknown>>>(promise_value) }.to_vec()
                } else {
                    Vec::new()
                };
                // SAFETY: a server reference's module is a function.
                let function: &'static js::Function = unsafe { js::cast(resolved_value) };
                resolved_value = Some(js::unknown(function::bind(
                    function,
                    Some(&Kind::Null),
                    &bound_args,
                )));
            }

            register_bound_server_reference(
                // SAFETY: a server reference's module is a function.
                unsafe { js::cast(resolved_value) },
                // SAFETY: a server reference's metadata has its id.
                unsafe { js::cast(js::get(meta_data, "id")) },
                // SAFETY: its bound arguments are a thenable, or null.
                unsafe { js::cast(bound) },
                response._encode_form_action.get(),
            );

            if key != __PROTO__ {
                js::set(parent_object, &*key, resolved_value);
            }

            // If this is the root object for a model reference, where `handler.value`
            // is a stale `null`, the resolved value can be used directly.
            if key.is_empty() && js::strict_equals(handler.value.get(), js::null::<&Unknown>()) {
                handler.value.set(resolved_value);
            }

            // If the parent object is an unparsed React element tuple, we also need to
            // update the props and owner of the parsed element object (i.e.
            // handler.value).
            let handler_value = handler.value.get();
            if SymbolFor::from_value(js::get(parent_object, &0.0)) == Some(REACT_ELEMENT_TYPE)
                && js::type_of(handler_value) == "object"
                && let Some(element) = handler_value
                && SymbolFor::from_value(js::get(handler_value, "$$typeof"))
                    == Some(REACT_ELEMENT_TYPE)
            {
                match key.as_str() {
                    "3" => js::set(element, "props", resolved_value),
                    "4" if __DEV__ => js::set(element, "_owner", resolved_value),
                    _ => {}
                }
            }

            handler.deps.set(handler.deps.get() - 1.0);
            initialize_chunk_if_unblocked(response, &handler);
        }
    };

    let reject = move |error: &'static js::JsError| {
        if handler.errored.get() {
            // We've already errored. We could instead build up an AggregateError
            // but if there are multiple errors we just take the first one like
            // Promise.all.
            return;
        }
        let blocked_value = handler.value.get();
        let errored_handler = &handler;
        errored_handler.errored.set(true);
        errored_handler.value.set(js::null());
        errored_handler.reason.set(error.as_unknown());
        let chunk = handler.chunk.borrow().clone();
        let Some(chunk) =
            chunk.filter(|chunk| matches!(*chunk.state.borrow(), ChunkState::Blocked { .. }))
        else {
            return;
        };

        if __DEV__
            && js::type_of(blocked_value) == "object"
            && let Some(element) = blocked_value
            && SymbolFor::from_value(js::get(blocked_value, "$$typeof")) == Some(REACT_ELEMENT_TYPE)
        {
            // Conceptually the error happened inside this Element but right before
            // it was rendered. We don't have a client side component to render but
            // we can add some DebugInfo to explain that this was conceptually a
            // Server side error that errored inside this element. That way any stack
            // traces will point to the nearest JSX that errored - e.g. during
            // serialization.
            let errored_component = Rc::new(ReactComponentInfo {
                name: get_component_name_from_type(js::get(element, "type")).unwrap_or_default(),
                env: None,
                key: None,
                // SAFETY: a DEV element's `_owner` is a component info or null.
                owner: Cell::new(unsafe { js::cast(js::get(element, "_owner")) }),
                stack: RefCell::new(None),
                props: None,
                debug_stack: Cell::new(None),
                debug_task: Cell::new(None),
                debug_location: Cell::new(None),
            });
            // SAFETY: a DEV element's `_debugStack` is an Error or null.
            errored_component
                .debug_stack
                .set(unsafe { js::cast(js::get(element, "_debugStack")) });
            if SUPPORTS_CREATE_TASK.with(|&supports| supports) {
                // SAFETY: a DEV element's `_debugTask` is a task or null.
                errored_component
                    .debug_task
                    .set(unsafe { js::cast(js::get(element, "_debugTask")) });
            }
            chunk
                ._debug_info
                .borrow()
                .as_ref()
                .unwrap()
                .borrow_mut()
                .push(Some(js::unknown_of(errored_component)));
        }

        trigger_error_on_chunk(response, &chunk, error.as_unknown());
    };

    promise.unwrap().then_with::<(), _, _>(fulfill, reject);

    // Return a place holder value for now.
    js::null()
}

fn load_server_object_reference(
    _response: &'static Response,
    meta_data: Option<&'static Unknown>,
    _parent_object: &'static Unknown,
    _key: &str,
) -> Option<&'static Unknown> {
    // A Server Reference to an object is always opaque on the client, even if
    // we have a module mapping for Server References: unlike a function
    // reference, the module that produced it is only meant to be evaluated in a
    // true server environment. The reference can only be passed back to the
    // server, where it resolves to the object.
    // The server reuses this metadata when the same object occurs again.
    // Cache its handle so reference equality is preserved within this response.
    let existing_reference = js::get(meta_data, "$$reference");
    if !matches!(js::classify(existing_reference), Kind::Undefined) {
        return existing_reference;
    }
    // SAFETY: a server object reference's metadata has its id.
    let reference = create_server_object_reference(unsafe { js::cast(js::get(meta_data, "id")) });
    js::set(meta_data, "$$reference", reference);
    Some(reference)
}

fn resolve_lazy(value: Option<&'static Unknown>) -> Option<&'static Unknown> {
    let mut value = value;
    while js::type_of(value) == "object"
        && value.is_some()
        && SymbolFor::from_value(js::get(value, "$$typeof")) == Some(REACT_LAZY_TYPE)
    {
        // SAFETY: a Flight lazy node's payload is its chunk.
        let payload: Chunk = unsafe { js::cast(js::get(value, "_payload")) };
        if matches!(*payload.state.borrow(), ChunkState::Initialized { .. }) {
            value = initialized_value(&payload);
            continue;
        }
        break;
    }

    value
}

fn transfer_referenced_debug_info(receiving_chunk: Option<Chunk>, referenced_chunk: &Chunk) {
    if __DEV__ {
        // We add the debug info to the receiving chunk since the resolution of that
        // promise is also blocked by the referenced debug info. By adding it to
        // both we can track it even if the array/element/lazy is extracted, or if
        // the root is rendered as is.
        if let Some(receiving_chunk) = receiving_chunk {
            let referenced_debug_info =
                Rc::clone(referenced_chunk._debug_info.borrow().as_ref().unwrap());
            let receiving_debug_info =
                Rc::clone(receiving_chunk._debug_info.borrow().as_ref().unwrap());
            // The receiving chunk takes each entry only once. A repeated entry
            // carries no information. An entry repeats in two ways:
            //
            // - the receiving chunk references the same chunk more than once
            // - two referenced chunks carry the same entry
            //
            // Without the set, the entries multiply along a chain of references.
            let received_debug_info = match receiving_chunk._received_debug_info.get() {
                Some(set) => set,
                None => {
                    let set = js::set::new(None);
                    receiving_chunk._received_debug_info.set(Some(set));
                    set
                }
            };
            let mut i = 0;
            while i < referenced_debug_info.borrow().len() {
                let debug_info_entry = referenced_debug_info.borrow()[i];
                if !js::loose_equals(js::get(debug_info_entry, "name"), js::null::<&Unknown>()) {
                    // We're not transferring Component info since we use Component info
                    // in Debug info to fill in gaps between Fibers for the parent stack.
                } else if !received_debug_info.has(
                    // SAFETY: an entry whose `name` was read isn't a hole.
                    unsafe { js::cast(debug_info_entry) },
                ) {
                    received_debug_info.add(unsafe { js::cast(debug_info_entry) });
                    receiving_debug_info.borrow_mut().push(debug_info_entry);
                }
                i += 1;
            }
        }
    }
}

thread_local! {
    // Most references have no path, so they can all share the same empty array.
    // It's never mutated because only paths with entries get spliced in place.
    #[cfg_attr(rust_js, rust_js::name = "EMPTY_REFERENCE_PATH")]
    static EMPTY_REFERENCE_PATH: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
}

fn get_outlined_model(
    response: &'static Response,
    reference: &str,
    parent_object: &'static Unknown,
    key: &str,
    map: MapFn,
) -> Option<&'static Unknown> {
    // parseInt stops at the ':' so we only need to split when there's a path.
    let id = js::parse_int(reference, Some(16));
    let path = if js::string::index_of(reference, ":", None) == -1.0 {
        EMPTY_REFERENCE_PATH.with(Rc::clone)
    } else {
        Rc::new(RefCell::new(js::string::split(reference, ":")))
    };
    let chunk = get_chunk(response, id);
    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        && ENABLE_COMPONENT_PERFORMANCE_TRACK
        && let Some(initializing_chunk) = INITIALIZING_CHUNK.with_borrow(Clone::clone)
        && let Some(Children::Chunks(children)) = &*initializing_chunk._children.borrow()
    {
        children.borrow_mut().push(Rc::clone(&chunk));
    }
    if matches!(*chunk.state.borrow(), ChunkState::ResolvedModel { .. }) {
        initialize_model_chunk(&chunk);
    } else if matches!(*chunk.state.borrow(), ChunkState::ResolvedModule { .. }) {
        initialize_module_chunk(&chunk);
    }
    // The status might have changed after initialization.
    if matches!(*chunk.state.borrow(), ChunkState::Initialized { .. }) {
        let mut value = initialized_value(&chunk);
        let path_len = path.borrow().len();
        for i in 1..path_len {
            while js::type_of(value) == "object"
                && value.is_some()
                && SymbolFor::from_value(js::get(value, "$$typeof")) == Some(REACT_LAZY_TYPE)
            {
                // SAFETY: a Flight lazy node's payload is its chunk.
                let referenced_chunk: Chunk = unsafe { js::cast(js::get(value, "_payload")) };
                if matches!(
                    *referenced_chunk.state.borrow(),
                    ChunkState::ResolvedModel { .. }
                ) {
                    initialize_model_chunk(&referenced_chunk);
                } else if matches!(
                    *referenced_chunk.state.borrow(),
                    ChunkState::ResolvedModule { .. }
                ) {
                    initialize_module_chunk(&referenced_chunk);
                }
                if matches!(
                    *referenced_chunk.state.borrow(),
                    ChunkState::Initialized { .. }
                ) {
                    value = initialized_value(&referenced_chunk);
                } else if matches!(
                    *referenced_chunk.state.borrow(),
                    ChunkState::Blocked { .. }
                        | ChunkState::Pending { .. }
                        | ChunkState::PendingWeak { .. }
                ) {
                    return wait_for_reference(
                        &referenced_chunk,
                        parent_object,
                        key,
                        response,
                        map,
                        Rc::new(RefCell::new(path.borrow()[i - 1..].to_vec())),
                        IS_INITIALIZING_DEBUG_INFO.get(),
                    );
                } else if matches!(*referenced_chunk.state.borrow(), ChunkState::Halted { .. }) {
                    // Add a dependency that will never resolve.
                    // TODO: Mark downstreams as halted too.
                    if let Some(handler) = INITIALIZING_HANDLER.with_borrow(Clone::clone) {
                        handler.deps.set(handler.deps.get() + 1.0);
                    } else {
                        INITIALIZING_HANDLER.set(Some(Rc::new(InitializationHandler {
                            parent: None,
                            chunk: RefCell::new(None),
                            value: Cell::new(js::null()),
                            reason: Cell::new(js::null()),
                            deps: Cell::new(1.0),
                            errored: Cell::new(false),
                        })));
                    }
                    return js::null();
                } else {
                    // This is an error. Instead of erroring directly, we're going to encode this on
                    // an initialization handler so that we can catch it at the nearest Element.
                    if let Some(initializing_handler) =
                        INITIALIZING_HANDLER.with_borrow(Clone::clone)
                    {
                        let errored_handler = initializing_handler;
                        errored_handler.errored.set(true);
                        errored_handler.value.set(js::null());
                        errored_handler
                            .reason
                            .set(errored_reason(&referenced_chunk));
                    } else {
                        INITIALIZING_HANDLER.set(Some(Rc::new(InitializationHandler {
                            parent: None,
                            chunk: RefCell::new(None),
                            value: Cell::new(js::null()),
                            reason: Cell::new(errored_reason(&referenced_chunk)),
                            deps: Cell::new(0.0),
                            errored: Cell::new(true),
                        })));
                    }
                    return js::null();
                }
            }
            let name = path.borrow()[i].clone();
            if js::type_of(value) == "object"
                && value.is_some()
                && (js::strict_equals(
                    GET_PROTOTYPE_OF.with(|get_prototype_of| get_prototype_of(value)),
                    OBJECT_PROTOTYPE.with(|&prototype| prototype),
                ) || js::strict_equals(
                    GET_PROTOTYPE_OF.with(|get_prototype_of| get_prototype_of(value)),
                    ARRAY_PROTOTYPE.with(|&prototype| prototype),
                ))
                && HAS_OWN_PROPERTY
                    .with(|has_own_property| has_own_property.call(value.unwrap(), &*name))
            {
                value = js::get(value, &*name);
            } else {
                panic!("Invalid reference.");
            }
        }

        while js::type_of(value) == "object"
            && value.is_some()
            && SymbolFor::from_value(js::get(value, "$$typeof")) == Some(REACT_LAZY_TYPE)
        {
            // If what we're referencing is a Lazy it must be because we inserted one as a virtual node
            // while it was blocked by other data. If it's no longer blocked, we can unwrap it.
            // SAFETY: a Flight lazy node's payload is its chunk.
            let referenced_chunk: Chunk = unsafe { js::cast(js::get(value, "_payload")) };
            if matches!(
                *referenced_chunk.state.borrow(),
                ChunkState::ResolvedModel { .. }
            ) {
                initialize_model_chunk(&referenced_chunk);
            } else if matches!(
                *referenced_chunk.state.borrow(),
                ChunkState::ResolvedModule { .. }
            ) {
                initialize_module_chunk(&referenced_chunk);
            }
            if matches!(
                *referenced_chunk.state.borrow(),
                ChunkState::Initialized { .. }
            ) {
                value = initialized_value(&referenced_chunk);
                continue;
            }
            break;
        }

        let chunk_value = map(response, value, parent_object, key);
        if __DEV__ {
            if SymbolFor::from_value(js::get(parent_object, &0.0)) == Some(REACT_ELEMENT_TYPE)
                && (key == "4" || key == "5")
            {
                // If we're resolving the "owner" or "stack" slot of an Element array,
                // we don't call transferReferencedDebugInfo because this reference is
                // to a debug chunk.
            } else if IS_INITIALIZING_DEBUG_INFO.get() {
                // If we're resolving references as part of debug info resolution, we
                // don't call transferReferencedDebugInfo because these references are
                // to debug chunks.
            } else {
                transfer_referenced_debug_info(
                    INITIALIZING_CHUNK.with_borrow(Clone::clone),
                    &chunk,
                );
            }
        }
        chunk_value
    } else if matches!(
        *chunk.state.borrow(),
        ChunkState::Pending { .. } | ChunkState::PendingWeak { .. } | ChunkState::Blocked { .. }
    ) {
        wait_for_reference(
            &chunk,
            parent_object,
            key,
            response,
            map,
            path,
            IS_INITIALIZING_DEBUG_INFO.get(),
        )
    } else if matches!(*chunk.state.borrow(), ChunkState::Halted { .. }) {
        // Add a dependency that will never resolve.
        // TODO: Mark downstreams as halted too.
        if let Some(handler) = INITIALIZING_HANDLER.with_borrow(Clone::clone) {
            handler.deps.set(handler.deps.get() + 1.0);
        } else {
            INITIALIZING_HANDLER.set(Some(Rc::new(InitializationHandler {
                parent: None,
                chunk: RefCell::new(None),
                value: Cell::new(js::null()),
                reason: Cell::new(js::null()),
                deps: Cell::new(1.0),
                errored: Cell::new(false),
            })));
        }
        js::null()
    } else {
        // This is an error. Instead of erroring directly, we're going to encode this on
        // an initialization handler so that we can catch it at the nearest Element.
        if let Some(initializing_handler) = INITIALIZING_HANDLER.with_borrow(Clone::clone) {
            let errored_handler = initializing_handler;
            errored_handler.errored.set(true);
            errored_handler.value.set(js::null());
            errored_handler.reason.set(errored_reason(&chunk));
        } else {
            INITIALIZING_HANDLER.set(Some(Rc::new(InitializationHandler {
                parent: None,
                chunk: RefCell::new(None),
                value: Cell::new(js::null()),
                reason: Cell::new(errored_reason(&chunk)),
                deps: Cell::new(0.0),
                errored: Cell::new(true),
            })));
        }
        // Placeholder
        js::null()
    }
}

fn create_map(
    _response: &'static Response,
    model: Option<&'static Unknown>,
    _parent_object: &'static Unknown,
    _key: &str,
) -> Option<&'static Unknown> {
    // SAFETY: a Map's model is its entries.
    let entries: &Vec<(&'static Unknown, Option<&'static Unknown>)> = unsafe { js::cast(model) };
    Some(js::unknown(
        js::map::new::<Unknown, Option<&'static Unknown>>(Some(entries)),
    ))
}

fn create_set(
    _response: &'static Response,
    model: Option<&'static Unknown>,
    _parent_object: &'static Unknown,
    _key: &str,
) -> Option<&'static Unknown> {
    // SAFETY: a Set's model is its values.
    let values: &Vec<&'static Unknown> = unsafe { js::cast(model) };
    Some(js::unknown(js::set::new::<Unknown>(Some(values))))
}

fn create_blob(
    _response: &'static Response,
    model: Option<&'static Unknown>,
    _parent_object: &'static Unknown,
    _key: &str,
) -> Option<&'static Unknown> {
    // SAFETY: a Blob's model is its type and its parts.
    let model: &Vec<Option<&'static Unknown>> = unsafe { js::cast(model) };
    // SAFETY: a Blob's parts are strings, buffers, views or Blobs.
    let parts: &[webapi::BlobPart<'_>] = unsafe { js::cast(Some(js::unknown(&model[1..]))) };
    Some(js::unknown(webapi::blob::new(
        Some(parts),
        // SAFETY: a Blob's type is a string.
        Some(webapi::BlobPropertyBag {
            r#type: Some(unsafe { js::cast(model[0]) }),
            endings: None,
        }),
    )))
}

fn create_form_data(
    _response: &'static Response,
    model: Option<&'static Unknown>,
    _parent_object: &'static Unknown,
    _key: &str,
) -> Option<&'static Unknown> {
    let form_data = webapi::form_data::new(None, None);
    // SAFETY: a FormData's model is its entries.
    let model: &Vec<(String, Option<&'static Unknown>)> = unsafe { js::cast(model) };
    for (name, value) in model {
        form_data.append(name, unsafe { js::cast::<FormDataEntryValue>(*value) });
    }
    Some(js::unknown(form_data))
}

fn apply_constructor(
    _response: &'static Response,
    model: Option<&'static Unknown>,
    parent_object: &'static Unknown,
    _key: &str,
) -> Option<&'static Unknown> {
    // SAFETY: `Unknown` is any value, `undefined` too.
    let prototype: &Unknown = unsafe { js::cast(js::get(model, "prototype")) };
    object::set_prototype_of(parent_object, Some(prototype));
    // Delete the property. It was just a placeholder.
    None
}

fn define_lazy_getter(
    _response: &'static Response,
    chunk: Chunk,
    parent_object: &'static Unknown,
    key: &str,
) -> Option<&'static Unknown> {
    // We don't immediately initialize it even if it's resolved.
    // Instead, we wait for the getter to get accessed.
    if key != __PROTO__ {
        object::define_property(
            parent_object,
            key,
            &PropertyDescriptor {
                get: Some(&move || {
                    if matches!(*chunk.state.borrow(), ChunkState::ResolvedModel { .. }) {
                        // If it was now resolved, then we initialize it. This may then discover
                        // a new set of lazy references that are then asked for eagerly in case
                        // we get that deep.
                        initialize_model_chunk(&chunk);
                    }
                    match &*chunk.state.borrow() {
                        ChunkState::Initialized { value, .. } => return *value,
                        ChunkState::Errored { reason } => js::throw(*reason),
                        _ => {}
                    }
                    // Otherwise, we didn't have enough time to load the object before it was
                    // accessed or the connection closed. So we just log that it was omitted.
                    // TODO: We should ideally throw here to indicate a difference.
                    Some(js::unknown_of(OMITTED_PROP_ERROR))
                }),
                // no-op: the walk function may try to reassign this property after
                // parseModelString returns. With the JSON.parse reviver, the engine's
                // internal CreateDataProperty silently failed. We use a no-op setter
                // to match that behavior in strict mode.
                set: Some(&|_| {}),
                enumerable: Some(true),
                configurable: Some(false),
                value: None,
                writable: None,
            },
        );
    }
    js::null()
}

fn extract_iterator(
    _response: &'static Response,
    model: Option<&'static Unknown>,
    _parent_object: &'static Unknown,
    _key: &str,
) -> Option<&'static Unknown> {
    let iterator = js::get(model, js::symbol::iterator);
    // SAFETY: an array's `[Symbol.iterator]` is a function.
    let iterator: &'static dyn Fn() -> Option<&'static Unknown> = unsafe { js::cast(iterator) };
    function::call(iterator, model.map(|model| model as &dyn IntoUnknown), ())
}

fn create_model(
    _response: &'static Response,
    model: Option<&'static Unknown>,
    _parent_object: &'static Unknown,
    key: &str,
) -> Option<&'static Unknown> {
    if key == "then" && js::type_of(model) == "function" {
        // This should never happen because we always serialize objects with then-functions
        // as "thenable" which reduces to a chunk with no other fields.
        return js::null();
    }
    model
}

thread_local! {
    static MIGHT_HAVE_STATIC_CONSTRUCTOR: &'static js::RegExp =
        js::reg_exp::new(r"\bclass\b.*\bstatic\b", "");
}

fn get_inferred_function_approximate(code: &str) -> Option<&'static Unknown> {
    let sliced_code = if code.starts_with("Object.defineProperty(") {
        js::string::slice(
            code,
            js::string::length("Object.defineProperty(") as f64,
            None,
        )
    } else if code.starts_with('(') {
        js::string::slice(code, 1.0, None)
    } else {
        code.to_owned()
    };
    if sliced_code.starts_with("async function") {
        let idx = js::string::index_of(&sliced_code, "(", Some(14.0));
        if idx != -1.0 {
            let name = js::string::trim(&js::string::slice(&sliced_code, 14.0, Some(idx)));
            // eslint-disable-next-line no-eval
            return js::get(
                js::eval(
                    &("({".to_owned() + &js::json::stringify(&name) + ":async function(){}})"),
                )
                .unwrap(),
                &*name,
            );
        }
    } else if sliced_code.starts_with("function") {
        let idx = js::string::index_of(&sliced_code, "(", Some(8.0));
        if idx != -1.0 {
            let name = js::string::trim(&js::string::slice(&sliced_code, 8.0, Some(idx)));
            // eslint-disable-next-line no-eval
            return js::get(
                js::eval(&("({".to_owned() + &js::json::stringify(&name) + ":function(){}})"))
                    .unwrap(),
                &*name,
            );
        }
    } else if sliced_code.starts_with("class") {
        let idx = js::string::index_of(&sliced_code, "{", Some(5.0));
        if idx != -1.0 {
            let name = js::string::trim(&js::string::slice(&sliced_code, 5.0, Some(idx)));
            // eslint-disable-next-line no-eval
            return js::get(
                js::eval(&("({".to_owned() + &js::json::stringify(&name) + ":class{}})")).unwrap(),
                &*name,
            );
        }
    }
    Some(js::unknown(function::with_this(|_, ()| {})))
}

fn parse_model_string(
    response: &'static Response,
    parent_object: &'static Unknown,
    key: &str,
    value: &str,
) -> Option<&'static Unknown> {
    if js::string::get(value, 0.0) == Some("$") {
        if value == "$" {
            // A very common symbol.
            if let Some(initializing_handler) = INITIALIZING_HANDLER.with_borrow(Clone::clone)
                && key == "0"
            {
                // We we already have an initializing handler and we're abound to enter
                // a new element, we need to shadow it because we're now in a new scope.
                // This is effectively the "begin" or "push" phase of Element parsing.
                // We'll pop later when we parse the array itself.
                INITIALIZING_HANDLER.set(Some(Rc::new(InitializationHandler {
                    parent: Some(initializing_handler),
                    chunk: RefCell::new(None),
                    value: Cell::new(js::null()),
                    reason: Cell::new(js::null()),
                    deps: Cell::new(0.0),
                    errored: Cell::new(false),
                })));
            }
            return Some(js::unknown(&REACT_ELEMENT_TYPE));
        }
        let tag = js::string::get(value, 1.0);
        match tag {
            Some("$") => {
                // This was an escaped string value.
                return Some(js::unknown_of(js::string::slice(value, 1.0, None)));
            }
            Some("L") => {
                // Lazy node
                let id = js::parse_int(js::string::slice(value, 2.0, None), Some(16));
                let chunk = get_chunk(response, id);
                if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                    && ENABLE_COMPONENT_PERFORMANCE_TRACK
                    && let Some(initializing_chunk) = INITIALIZING_CHUNK.with_borrow(Clone::clone)
                    && let Some(Children::Chunks(children)) =
                        &*initializing_chunk._children.borrow()
                {
                    children.borrow_mut().push(Rc::clone(&chunk));
                }
                // We create a React.lazy wrapper around any lazy values.
                // When passed into React, we'll know how to suspend on this.
                return Some(create_lazy_chunk_wrapper(&chunk, Some(js::unknown_of(0.0))));
            }
            Some("@") => {
                // Promise
                let id = js::parse_int(js::string::slice(value, 2.0, None), Some(16));
                let chunk = get_chunk(response, id);
                if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                    && ENABLE_COMPONENT_PERFORMANCE_TRACK
                    && let Some(initializing_chunk) = INITIALIZING_CHUNK.with_borrow(Clone::clone)
                    && let Some(Children::Chunks(children)) =
                        &*initializing_chunk._children.borrow()
                {
                    children.borrow_mut().push(Rc::clone(&chunk));
                }
                return Some(js::unknown_of(chunk));
            }
            Some("w") => {
                if ENABLE_FLIGHT_WEAK_THENABLES.with(|&enabled| enabled) {
                    // Weak Promise
                    let id = js::parse_int(js::string::slice(value, 2.0, None), Some(16));
                    let chunk = get_weak_chunk(response, id);
                    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                        && ENABLE_COMPONENT_PERFORMANCE_TRACK
                        && let Some(initializing_chunk) =
                            INITIALIZING_CHUNK.with_borrow(Clone::clone)
                        && let Some(Children::Chunks(children)) =
                            &*initializing_chunk._children.borrow()
                    {
                        children.borrow_mut().push(Rc::clone(&chunk));
                    }
                    return Some(js::unknown_of(chunk));
                }
                return None;
            }
            Some("S") => {
                // Symbol
                return unsafe {
                    js::cast(Some(js::unknown(&SymbolFor(&js::string::slice(
                        value, 2.0, None,
                    )))))
                };
            }
            Some("h") => {
                // Server Reference
                let r#ref = js::string::slice(value, 2.0, None);
                return get_outlined_model(
                    response,
                    &r#ref,
                    parent_object,
                    key,
                    load_server_reference,
                );
            }
            Some("H") => {
                if ENABLE_FLIGHT_OBJECT_REFERENCES.with(|&enabled| enabled) {
                    // Server Reference to an object
                    let r#ref = js::string::slice(value, 2.0, None);
                    return get_outlined_model(
                        response,
                        &r#ref,
                        parent_object,
                        key,
                        load_server_object_reference,
                    );
                }
                return None;
            }
            Some("T") => {
                // Temporary Reference
                let reference = "$".to_owned() + &js::string::slice(value, 2.0, None);
                let Some(temporary_references) = response._temp_refs.get() else {
                    panic!(
                        "{}",
                        concat!(
                            "Missing a temporary reference set but the RSC response returned a temporary reference. ",
                            "Pass a temporaryReference option with the set that was used with the reply.",
                        )
                    );
                };
                return read_temporary_reference(temporary_references, &reference);
            }
            Some("Q") => {
                // Map
                let r#ref = js::string::slice(value, 2.0, None);
                return get_outlined_model(response, &r#ref, parent_object, key, create_map);
            }
            Some("W") => {
                // Set
                let r#ref = js::string::slice(value, 2.0, None);
                return get_outlined_model(response, &r#ref, parent_object, key, create_set);
            }
            Some("B") => {
                // Blob
                let r#ref = js::string::slice(value, 2.0, None);
                return get_outlined_model(response, &r#ref, parent_object, key, create_blob);
            }
            Some("K") => {
                // FormData
                let r#ref = js::string::slice(value, 2.0, None);
                return get_outlined_model(response, &r#ref, parent_object, key, create_form_data);
            }
            Some("Z") => {
                // Error
                if __DEV__ {
                    let r#ref = js::string::slice(value, 2.0, None);
                    return get_outlined_model(
                        response,
                        &r#ref,
                        parent_object,
                        key,
                        map_resolve_error_dev,
                    );
                } else {
                    return Some(js::unknown(resolve_error_prod(response)));
                }
            }
            Some("i") => {
                // Iterator
                let r#ref = js::string::slice(value, 2.0, None);
                return get_outlined_model(response, &r#ref, parent_object, key, extract_iterator);
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
                return Some(js::unknown(js::big_int(&js::string::slice(
                    value, 2.0, None,
                ))));
            }
            _ => {
                if tag == Some("P") && __DEV__ {
                    // In DEV mode we allow debug objects to specify themselves as instances of
                    // another constructor.
                    let r#ref = js::string::slice(value, 2.0, None);
                    return get_outlined_model(
                        response,
                        &r#ref,
                        parent_object,
                        key,
                        apply_constructor,
                    );
                }
                //Fallthrough
                if (tag == Some("P") || tag == Some("E")) && __DEV__ {
                    // In DEV mode we allow indirect eval to produce functions for logging.
                    // This should not compile to eval() because then it has local scope access.
                    let code = js::string::slice(value, 2.0, None);
                    // If this might be a class constructor with a static initializer or
                    // static constructor then don't eval it. It might cause unexpected
                    // side-effects. Instead, fallback to parsing out the function type
                    // and name.
                    if !MIGHT_HAVE_STATIC_CONSTRUCTOR.with(|regex| regex.test(&code))
                        && let Ok(function) = js::eval(&code)
                    {
                        // eslint-disable-next-line no-eval
                        return function;
                    }
                    // Fallthrough to fallback case.
                    // We currently use this to express functions so we fail parsing it,
                    // let's just return a blank function as a place holder.
                    let r#fn = std::panic::catch_unwind(|| {
                        let r#fn = get_inferred_function_approximate(&code);
                        if code.starts_with("Object.defineProperty(") {
                            const DESCRIPTOR: &str = ",\"name\",{value:\"";
                            let idx = js::string::last_index_of(&code, DESCRIPTOR, None);
                            if idx != -1.0 {
                                let name = js::json::parse(&js::string::slice(
                                    &code,
                                    idx + js::string::length(DESCRIPTOR) as f64 - 1.0,
                                    Some(js::string::length(&code) as f64 - 2.0),
                                ))
                                .unwrap();
                                object::define_property(
                                    r#fn.unwrap(),
                                    "name",
                                    &PropertyDescriptor {
                                        value: Some(&name),
                                        configurable: None,
                                        enumerable: None,
                                        writable: None,
                                        get: None,
                                        set: None,
                                    },
                                );
                            }
                        }
                        r#fn
                    })
                    .unwrap_or_else(|_| Some(js::unknown(function::with_this(|_, ()| {}))));
                    return r#fn;
                }
                // Fallthrough
                if (tag == Some("P") || tag == Some("E") || tag == Some("Y")) && __DEV__ {
                    if js::string::length(value) > 2 {
                        let debug_channel_callback = response
                            ._debug_channel
                            .borrow()
                            .as_ref()
                            .and_then(|debug_channel| debug_channel.callback.clone());
                        if let Some(debug_channel_callback) = debug_channel_callback {
                            if js::string::get(value, 2.0) == Some("@") {
                                // This is a deferred Promise.
                                let r#ref = js::string::slice(value, 3.0, None); // We assume this doesn't have a path just id.
                                let id = js::parse_int(&r#ref, Some(16));
                                if !response._chunks.get().unwrap().has(&id) {
                                    // We haven't seen this id before. Query the server to start sending it.
                                    debug_channel_callback(&("P:".to_owned() + &r#ref));
                                }
                                // Start waiting. This now creates a pending chunk if it doesn't already exist.
                                // This is the actual Promise we're waiting for.
                                return Some(js::unknown_of(get_chunk(response, id)));
                            }
                            let r#ref = js::string::slice(value, 2.0, None); // We assume this doesn't have a path just id.
                            let id = js::parse_int(&r#ref, Some(16));
                            if !response._chunks.get().unwrap().has(&id) {
                                // We haven't seen this id before. Query the server to start sending it.
                                debug_channel_callback(&("Q:".to_owned() + &r#ref));
                            }
                            // Start waiting. This now creates a pending chunk if it doesn't already exist.
                            let chunk = get_chunk(response, id);
                            if matches!(*chunk.state.borrow(), ChunkState::Initialized { .. }) {
                                // We already loaded this before. We can just use the real value.
                                return initialized_value(&chunk);
                            }
                            return define_lazy_getter(response, chunk, parent_object, key);
                        }
                    }

                    // In DEV mode we encode omitted objects in logs as a getter that throws
                    // so that when you try to access it on the client, you know why that
                    // happened.
                    if key != __PROTO__ {
                        object::define_property(
                            parent_object,
                            key,
                            &PropertyDescriptor {
                                get: Some(&|| {
                                    // TODO: We should ideally throw here to indicate a difference.
                                    Some(js::unknown_of(OMITTED_PROP_ERROR))
                                }),
                                // no-op: the walk function may try to reassign this property
                                // after parseModelString returns. With the JSON.parse reviver,
                                // the engine's internal CreateDataProperty silently failed.
                                // We use a no-op setter to match that behavior in strict mode.
                                set: Some(&|_| {}),
                                enumerable: Some(true),
                                configurable: Some(false),
                                value: None,
                                writable: None,
                            },
                        );
                    }
                    return js::null();
                }
                // Fallthrough
                // We assume that anything else is a reference ID.
                let r#ref = js::string::slice(value, 1.0, None);
                return get_outlined_model(response, &r#ref, parent_object, key, create_model);
            }
        }
    }
    Some(js::unknown_of(value.to_owned()))
}

/// `resolveErrorDev` as a reference's map.
fn map_resolve_error_dev(
    response: &'static Response,
    model: Option<&'static Unknown>,
    _parent_object: &'static Unknown,
    _key: &str,
) -> Option<&'static Unknown> {
    // SAFETY: a "$Z" model is a DEV error's info.
    let error_info: &'static ReactErrorInfoDev = unsafe { js::cast(model) };
    Some(js::unknown(resolve_error_dev(response, error_info)))
}

fn parse_model_tuple(
    response: &'static Response,
    value: &'static Unknown,
) -> Option<&'static Unknown> {
    let tuple = value;

    if SymbolFor::from_value(js::get(tuple, &0.0)) == Some(REACT_ELEMENT_TYPE) {
        // TODO: Consider having React just directly accept these arrays as elements.
        // Or even change the ReactElement type to be an array.
        return Some(create_element(
            response,
            js::get(tuple, &1.0),
            js::get(tuple, &2.0),
            js::get(tuple, &3.0),
            if __DEV__ {
                js::get(tuple, &4.0)
            } else {
                js::null()
            },
            if __DEV__ {
                js::get(tuple, &5.0)
            } else {
                js::null()
            },
            if __DEV__ {
                js::get(tuple, &6.0)
            } else {
                Some(js::unknown_of(0.0))
            },
        ));
    }
    Some(value)
}

fn missing_call(
    _id: &'static Unknown,
    _args: &'static Unknown,
) -> Promise<Option<&'static Unknown>> {
    panic!(
        "{}",
        concat!(
            "Trying to call a function from \"use server\" but the callServer option ",
            "was not implemented in your router runtime.",
        )
    );
}

/// ResponseInstance's parameters (DEV-only from findSourceMapURL on).
type ResponseParameters = (
    Option<&'static Unknown>,                  // bundlerConfig
    Option<&'static Unknown>,                  // serverReferenceConfig
    Option<&'static Unknown>,                  // moduleLoading
    Option<&'static CallServerCallback>,       // callServer
    Option<&'static EncodeFormActionCallback>, // encodeFormAction
    Option<String>,                            // nonce
    Option<&'static TemporaryReferenceSet>,    // temporaryReferences
    bool,                                      // allowPartialStream
    Option<&'static FindSourceMapURLCallback>, // findSourceMapURL, DEV-only
    bool,                                      // replayConsole, DEV-only
    Option<String>,                            // environmentName, DEV-only
    Option<f64>,                               // debugStartTime, DEV-only
    Option<f64>,                               // debugEndTime, DEV-only
    Option<Rc<DebugChannel>>,                  // debugChannel, DEV-only
);

/// ResponseInstance's signature, `new ResponseInstance(..)`'s.
type ResponseConstructor = fn(
    Option<&'static Unknown>,                  // bundlerConfig
    Option<&'static Unknown>,                  // serverReferenceConfig
    Option<&'static Unknown>,                  // moduleLoading
    Option<&'static CallServerCallback>,       // callServer
    Option<&'static EncodeFormActionCallback>, // encodeFormAction
    Option<String>,                            // nonce
    Option<&'static TemporaryReferenceSet>,    // temporaryReferences
    bool,                                      // allowPartialStream
    Option<&'static FindSourceMapURLCallback>, // findSourceMapURL, DEV-only
    bool,                                      // replayConsole, DEV-only
    Option<String>,                            // environmentName, DEV-only
    Option<f64>,                               // debugStartTime, DEV-only
    Option<f64>,                               // debugEndTime, DEV-only
    Option<Rc<DebugChannel>>,                  // debugChannel, DEV-only
) -> ResponseInstance;

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "ResponseInstance")]
    static RESPONSE_INSTANCE: &'static js::Class<ResponseConstructor> =
        js::Class::of(function::with_this(
            move |this,
                  (
                bundler_config,
                server_reference_config,
                module_loading,
                call_server,
                encode_form_action,
                nonce,
                temporary_references,
                allow_partial_stream,
                find_source_map_url, // DEV-only
                replay_console,      // DEV-only
                environment_name,    // DEV-only
                debug_start_time,    // DEV-only
                debug_end_time,      // DEV-only
                debug_channel,       // DEV-only
            ): ResponseParameters| {
                // SAFETY: `new` gives the constructor the object it makes.
                let this: &'static Response = unsafe { js::cast(this) };
                let chunks: &'static js::Map<f64, Chunk> = js::map::new(None);
                this._bundler_config.set(bundler_config);
                this._server_reference_config.set(server_reference_config);
                this._module_loading.set(module_loading);
                this._call_server.set(Some(match call_server {
                    Some(call_server) => call_server,
                    None => &missing_call,
                }));
                this._encode_form_action.set(encode_form_action);
                *this._nonce.borrow_mut() = nonce;
                this._chunks.set(Some(chunks));
                // Preserve a leading U+FEFF instead of consuming it as an encoding signature.
                this._string_decoder.set(Some(create_string_decoder(true)));
                this._closed.set(false);
                this._closed_reason.set(None);
                this._allow_partial_stream.set(allow_partial_stream);
                this._temp_refs.set(temporary_references);
                if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && ENABLE_COMPONENT_PERFORMANCE_TRACK {
                    this._time_origin.set(Some(0.0));
                    this._pending_initial_render.set(None);
                }
                if __DEV__ {
                    this._pending_chunks.set(Some(0.0));
                    *this._weak_response.borrow_mut() = Some(Rc::new(WeakResponse {
                        weak: js::weak_ref::new(this),
                        response: Cell::new(Some(this)),
                    }));
                    // TODO: The Flight Client can be used in a Client Environment too and we should really support
                    // getting the owner there as well, but currently the owner of ReactComponentInfo is typed as only
                    // supporting other ReactComponentInfo as owners (and not Fiber or Fizz's ComponentStackNode).
                    // We need to update all the callsites consuming ReactComponentInfo owners to support those.
                    // In the meantime we only check ReactSharedInteralsServer since we know that in an RSC environment
                    // the only owners will be ReactComponentInfo.
                    let root_owner: Option<&'static ReactComponentInfo> = match REACT_SHARED_INTERALS_SERVER.with(|&server| server) {
                        Some(server) if js::get(server, "A").is_some() => {
                            let a = js::get(server, "A");
                            let get_owner: &'static dyn Fn() -> Option<&'static Unknown> =
                                unsafe { js::cast(js::get(a, "getOwner")) };
                            unsafe { js::cast(function::call(get_owner, a.map(|a| a as &dyn IntoUnknown), ())) }
                        }
                        _ => None,
                    };

                    this._debug_root_owner.set(root_owner);
                    this._debug_root_stack.set(if root_owner.is_some() {
                        // TODO: Consider passing the top frame in so we can avoid internals showing up.
                        Some(js::error::new(Some("react-stack-top-frame"), None))
                    } else {
                        None
                    });

                    let root_env = environment_name.unwrap_or_else(|| "Server".to_owned());
                    if SUPPORTS_CREATE_TASK.with(|&supports| supports) {
                        // Any stacks that appear on the server need to be rooted somehow on the client
                        // so we create a root Task for this response which will be the root owner for any
                        // elements created by the server. We use the "use server" string to indicate that
                        // this is where we enter the server from the client.
                        // TODO: Make this string configurable.
                        this._debug_root_task.set(Some(console::create_task().unwrap()(
                            &("\"use ".to_owned() + &root_env.to_lowercase() + "\""),
                        )));
                    }
                    // Track the start of the fetch to the best of our knowledge.
                    // Note: createFromFetch allows this to be marked at the start of the fetch
                    // where as if you use createFromReadableStream from the body of the fetch
                    // then the start time is when the headers resolved.
                    this._debug_start_time.set(Some(debug_start_time.unwrap_or_else(|| performance.now())));
                    this._debug_io_started.set(Some(false));
                    // We consider everything before the first setTimeout task to be cached data
                    // and is not considered I/O required to load the stream.
                    js::set_timeout(
                        function::bind(MARK_IO_STARTED.with(|&f| f), Some(this), ()),
                        Some(0.0),
                    );
                    this._debug_end_time.set(debug_end_time);
                    this._debug_find_source_map_url.set(find_source_map_url);
                    *this._debug_channel.borrow_mut() = debug_channel.clone();
                    *this._blocked_console.borrow_mut() = None;
                    this._replay_console.set(Some(replay_console));
                    *this._root_environment_name.borrow_mut() = Some(root_env);
                    if let Some(debug_channel) = debug_channel {
                        match DEBUG_CHANNEL_REGISTRY.with(|&registry| registry) {
                            None => {
                                // We can't safely clean things up later, so we immediately close the
                                // debug channel.
                                close_debug_channel(debug_channel);
                                *this._debug_channel.borrow_mut() = None;
                            }
                            Some(debug_channel_registry) => {
                                // When a Response gets GC:ed because nobody is referring to any of the
                                // objects that lazily load from the Response anymore, then we can close
                                // the debug channel.
                                debug_channel_registry.register(this, debug_channel, Some(this));
                            }
                        }
                    }
                }
                if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                    && ENABLE_COMPONENT_PERFORMANCE_TRACK
                    // Since we don't know when recording of profiles will start and stop, we have to
                    // mark the order over and over again.
                    && replay_console
                {
                    mark_all_tracks_in_order();
                }
                this
            },
        ));

    #[cfg_attr(rust_js, rust_js::name = "markIOStarted")]
    static MARK_IO_STARTED: &'static dyn Fn() = function::with_this(|this, ()| {
        // SAFETY: JS calls it bound to a Response.
        let this: &Response = unsafe { js::cast(this) };
        this._debug_io_started.set(Some(true));
    });
}

#[allow(clippy::too_many_arguments)] // upstream's 14 positional parameters.
#[cfg_attr(
    rust_js,
    rust_js::nullable(bundler_config, server_reference_config, module_loading)
)]
pub fn create_response(
    bundler_config: Option<&'static Unknown>,
    server_reference_config: Option<&'static Unknown>,
    module_loading: Option<&'static Unknown>,
    call_server: Option<&'static CallServerCallback>,
    encode_form_action: Option<&'static EncodeFormActionCallback>,
    nonce: Option<String>,
    temporary_references: Option<&'static TemporaryReferenceSet>,
    allow_partial_stream: bool,
    find_source_map_url: Option<&'static FindSourceMapURLCallback>, // DEV-only
    replay_console: bool,                                           // DEV-only
    environment_name: Option<String>,                               // DEV-only
    debug_start_time: Option<f64>,                                  // DEV-only
    debug_end_time: Option<f64>,                                    // DEV-only
    debug_channel: Option<Rc<DebugChannel>>,                        // DEV-only
) -> Rc<WeakResponse> {
    if __DEV__ {
        // We use eval to create fake function stacks which includes Component stacks.
        // A warning would be noise if you used Flight without Components and don't encounter
        // errors. We're warning eagerly so that you configure your environment accordingly
        // before you encounter an error.
        check_eval_availability_once_dev();
    }

    get_weak_response(RESPONSE_INSTANCE.with(|&response_instance| {
        js::new(
            response_instance,
            (
                bundler_config,
                server_reference_config,
                module_loading,
                call_server,
                encode_form_action,
                nonce,
                temporary_references,
                allow_partial_stream,
                find_source_map_url,
                replay_console,
                environment_name,
                debug_start_time,
                debug_end_time,
                debug_channel,
            ),
        )
    }))
}

pub struct StreamState {
    pub _row_state: Cell<RowParserState>,
    // parts of a row ID parsed so far
    #[cfg_attr(rust_js, rust_js::name = "_rowID")]
    pub _row_id: Cell<i32>,
    // 0 indicates that we're currently parsing the row ID
    pub _row_tag: Cell<i32>,
    // remaining bytes in the row. 0 indicates that we're looking for a newline.
    pub _row_length: Cell<i32>,
    // chunks received so far as part of this row
    pub _buffer: RefCell<Vec<&'static js::Uint8Array>>,
    // DEV-only
    #[cfg_attr(rust_js, rust_js::optional)]
    pub _debug_info: RefCell<Option<Rc<ReactIOInfo>>>,
    // DEV-only
    #[cfg_attr(rust_js, rust_js::optional)]
    pub _debug_target_chunk_size: Cell<Option<f64>>,
}

unsafe impl js::Defined for StreamState {}

pub fn create_stream_state(
    weak_response: &WeakResponse,                 // DEV-only
    stream_debug_value: Option<&'static Unknown>, // DEV-only
) -> StreamState {
    let stream_state = StreamState {
        _row_state: Cell::new(0),
        _row_id: Cell::new(0),
        _row_tag: Cell::new(0),
        _row_length: Cell::new(0),
        _buffer: RefCell::new(Vec::new()),
        _debug_info: RefCell::new(None),
        _debug_target_chunk_size: Cell::new(None),
    };
    if __DEV__ {
        let response = unwrap_weak_response(weak_response);
        // Create an entry for the I/O to load the stream itself.
        let debug_value_promise = js::promise::resolve(stream_debug_value);
        js::set(debug_value_promise, "status", "fulfilled");
        js::set(debug_value_promise, "value", stream_debug_value);
        *stream_state._debug_info.borrow_mut() = Some(Rc::new(ReactIOInfo {
            name: Cell::new("rsc stream"),
            start: Cell::new(response._debug_start_time.get().unwrap()),
            end: Cell::new(response._debug_start_time.get().unwrap()), // will be updated once we finish a chunk
            byte_size: Cell::new(Some(0.0)), // will be updated as we resolve a data chunk
            value: Cell::new(Some(debug_value_promise)),
            env: None,
            owner: response._debug_root_owner.get(),
            stack: None,
            debug_stack: Cell::new(response._debug_root_stack.get()),
            debug_task: Cell::new(response._debug_root_task.get()),
        }));
        stream_state
            ._debug_target_chunk_size
            .set(Some(MIN_CHUNK_SIZE));
    }
    stream_state
}

// Depending on set up the chunks of a TLS connection can vary in size. However in practice it's often
// at 64kb or even multiples of 64kb. It can also be smaller but in practice it also happens that 64kb
// is around what you can download on fast 4G connection in 300ms which is what we throttle reveals at
// anyway. The net effect is that in practice, you won't really reveal anything in smaller units than
// 64kb if they're revealing at maximum speed in production. Therefore we group smaller chunks into
// these larger chunks since in production that's more realistic.
// TODO: If the stream is compressed, then you could fit much more in a single 300ms so maybe it should
// actually be larger.
const MIN_CHUNK_SIZE: f64 = 65536.0;

fn increment_chunk_debug_info(stream_state: &StreamState, chunk_length: f64) {
    if __DEV__ {
        let debug_info = Rc::clone(stream_state._debug_info.borrow().as_ref().unwrap());
        let end_time = performance.now();
        let previous_end_time = debug_info.end.get();
        let new_byte_length = debug_info.byte_size.get().unwrap() + chunk_length;
        if new_byte_length > stream_state._debug_target_chunk_size.get().unwrap()
            || end_time > previous_end_time + 10.0
        {
            // This new chunk would overshoot the chunk size so therefore we treat it as its own new chunk
            // by cloning the old one. Similarly, if some time has passed we assume that it was actually
            // due to the server being unable to flush chunks faster e.g. due to I/O so it would be a
            // new chunk in production even if the buffer hasn't been reached.
            *stream_state._debug_info.borrow_mut() = Some(Rc::new(ReactIOInfo {
                name: Cell::new(debug_info.name.get()),
                start: Cell::new(debug_info.start.get()),
                end: Cell::new(end_time),
                byte_size: Cell::new(Some(new_byte_length)),
                value: Cell::new(debug_info.value.get()),
                env: None,
                owner: debug_info.owner,
                stack: None,
                debug_stack: Cell::new(debug_info.debug_stack.get()),
                debug_task: Cell::new(debug_info.debug_task.get()),
            }));
            stream_state
                ._debug_target_chunk_size
                .set(Some(new_byte_length + MIN_CHUNK_SIZE));
        } else {
            // Otherwise we reuse the old chunk but update the end time and byteSize to the latest.
            debug_info.end.set(end_time);
            debug_info.byte_size.set(Some(new_byte_length));
        }
    }
}

fn add_async_info(chunk: &Chunk, async_info: &'static Unknown) {
    let value = resolve_lazy(initialized_value(chunk));
    if js::type_of(value) == "object"
        && let Some(object) = value
        && (is_array(value)
            || js::type_of(ASYNC_ITERATOR.with(|&async_iterator| js::get(value, async_iterator)))
                == "function"
            || SymbolFor::from_value(js::get(value, "$$typeof")) == Some(REACT_ELEMENT_TYPE)
            || SymbolFor::from_value(js::get(value, "$$typeof")) == Some(REACT_LAZY_TYPE))
    {
        let value_debug_info = js::get(value, "_debugInfo");
        if is_array(value_debug_info) {
            // SAFETY: an array `_debugInfo` is the value's debug info.
            let value_debug_info: &RefCell<ReactDebugInfo> = unsafe { js::cast(value_debug_info) };
            value_debug_info.borrow_mut().push(Some(async_info));
        } else if !object::is_frozen(object) {
            // TODO: Debug info is dropped for frozen elements. See the TODO in
            // moveDebugInfoFromChunkToInnerValue.
            object::define_property(
                object,
                "_debugInfo",
                &PropertyDescriptor {
                    configurable: Some(false),
                    enumerable: Some(false),
                    writable: Some(true),
                    value: Some(&vec![async_info]),
                    get: None,
                    set: None,
                },
            );
        }
    } else {
        chunk
            ._debug_info
            .borrow()
            .as_ref()
            .unwrap()
            .borrow_mut()
            .push(Some(async_info));
    }
}

fn resolve_chunk_debug_info(
    response: &'static Response,
    stream_state: &StreamState,
    chunk: &Chunk,
) {
    // Only include stream information after a macrotask. Any chunk processed
    // before that is considered cached data.
    if __DEV__ && response._debug_io_started.get() == Some(true) {
        // Add the currently resolving chunk's debug info representing the stream
        // to the Promise that was waiting on the stream, or its underlying value.
        let async_info: &'static Unknown = js::unknown_of(ReactAsyncInfo {
            awaited: Rc::clone(stream_state._debug_info.borrow().as_ref().unwrap()),
            env: None,
            owner: Cell::new(None),
            stack: RefCell::new(None),
            debug_stack: Cell::new(None),
            debug_task: Cell::new(None),
        });
        if matches!(
            *chunk.state.borrow(),
            ChunkState::Pending { .. } | ChunkState::Blocked { .. }
        ) {
            let bound_add_async_info: Rc<dyn Fn(Option<&'static Unknown>)> =
                Rc::new(function::bind(
                    add_async_info_listener,
                    Some(&Kind::Null),
                    (Rc::clone(chunk), async_info),
                ));
            chunk.then(
                Some(Rc::clone(&bound_add_async_info)),
                Some(bound_add_async_info),
            );
        } else {
            add_async_info(chunk, async_info);
        }
    }
}

/// `addAsyncInfo` given a listener's value too, as `bind` calls it.
fn add_async_info_listener(
    chunk: &Chunk,
    async_info: &'static Unknown,
    _value: Option<&'static Unknown>,
) {
    add_async_info(chunk, async_info);
}

fn resolve_debug_halt(response: &'static Response, id: f64) {
    let chunks = response._chunks.get().unwrap();
    let chunk = match chunks.get(&id) {
        Some(chunk) => chunk,
        None => {
            let chunk = create_pending_chunk(response);
            chunks.set(&id, Rc::clone(&chunk));
            chunk
        }
    };
    if !matches!(
        *chunk.state.borrow(),
        ChunkState::Pending { .. } | ChunkState::PendingWeak { .. } | ChunkState::Blocked { .. }
    ) {
        return;
    }
    halt_chunk(response, &chunk);
}

fn resolve_model(
    response: &'static Response,
    id: f64,
    model: UninitializedModel,
    stream_state: &StreamState,
) {
    let chunks = response._chunks.get().unwrap();
    let chunk = chunks.get(&id);
    match chunk {
        None => {
            let new_chunk = create_resolved_model_chunk(response, model);
            if __DEV__ {
                resolve_chunk_debug_info(response, stream_state, &new_chunk);
            }
            chunks.set(&id, new_chunk);
        }
        Some(chunk) => {
            if __DEV__ {
                resolve_chunk_debug_info(response, stream_state, &chunk);
            }
            resolve_model_chunk(response, &chunk, model);
        }
    }
}

fn resolve_text(response: &'static Response, id: f64, text: String, stream_state: &StreamState) {
    let chunks = response._chunks.get().unwrap();
    let chunk = chunks.get(&id);
    if let Some(chunk) = &chunk
        && !matches!(*chunk.state.borrow(), ChunkState::Pending { .. })
    {
        // If we get more data to an already resolved ID, we assume that it's
        // a stream chunk since any other row shouldn't have more than one entry.
        let controller = stream_controller(chunk);
        (controller.enqueue_value)(Some(js::unknown_of(text)));
        return;
    }
    if let Some(chunk) = &chunk {
        release_pending_chunk(response, chunk);
    }
    let new_chunk = create_initialized_text_chunk(response, text);
    if __DEV__ {
        resolve_chunk_debug_info(response, stream_state, &new_chunk);
    }
    chunks.set(&id, new_chunk);
}

fn resolve_buffer(
    response: &'static Response,
    id: f64,
    buffer: &'static Unknown,
    stream_state: &StreamState,
) {
    let chunks = response._chunks.get().unwrap();
    let chunk = chunks.get(&id);
    if let Some(chunk) = &chunk
        && !matches!(*chunk.state.borrow(), ChunkState::Pending { .. })
    {
        // If we get more data to an already resolved ID, we assume that it's
        // a stream chunk since any other row shouldn't have more than one entry.
        let controller = stream_controller(chunk);
        (controller.enqueue_value)(Some(buffer));
        return;
    }
    if let Some(chunk) = &chunk {
        release_pending_chunk(response, chunk);
    }
    let new_chunk = create_initialized_buffer_chunk(response, buffer);
    if __DEV__ {
        resolve_chunk_debug_info(response, stream_state, &new_chunk);
    }
    chunks.set(&id, new_chunk);
}

fn resolve_module(
    response: &'static Response,
    id: f64,
    model: UninitializedModel,
    stream_state: &StreamState,
) {
    let chunks = response._chunks.get().unwrap();
    let chunk = chunks.get(&id);
    let prev_handler = INITIALIZING_HANDLER.take();
    let client_reference_metadata = {
        let _restore = RestoreInitializingHandler { prev_handler };
        let client_reference_metadata = parse_model(response, &model);
        if INITIALIZING_HANDLER.with_borrow(Option::is_some) {
            // We resolve the client reference below and have nothing to wait on,
            // so the metadata can't reference a row that hasn't arrived.
            panic!(
                "{}",
                concat!(
                    "A client reference was blocked on a row that has not been received yet. ",
                    "This is a bug in React.",
                )
            );
        }
        client_reference_metadata
    };
    let client_reference =
        resolve_client_reference(response._bundler_config.get(), client_reference_metadata);

    prepare_destination_for_module(
        response._module_loading.get(),
        response._nonce.borrow().as_deref(),
        client_reference_metadata,
    );

    // TODO: Add an option to encode modules that are lazy loaded.
    // For now we preload all modules as early as possible since it's likely
    // that we'll need them.
    let promise = preload_module(client_reference);
    if let Some(promise) = promise {
        let blocked_chunk = match chunk {
            None => {
                // Technically, we should just treat promise as the chunk in this
                // case. Because it'll just behave as any other promise.
                let blocked_chunk = create_blocked_chunk(response);
                chunks.set(&id, Rc::clone(&blocked_chunk));
                blocked_chunk
            }
            Some(chunk) => {
                release_pending_chunk(response, &chunk);
                // This can't actually happen because we don't have any forward
                // references to modules.
                let blocked_chunk = chunk;
                let (value, reason) = match &mut *blocked_chunk.state.borrow_mut() {
                    ChunkState::Pending { value, reason } => (value.take(), reason.take()),
                    _ => (None, None),
                };
                *blocked_chunk.state.borrow_mut() = ChunkState::Blocked { value, reason };
                blocked_chunk
            }
        };
        if __DEV__ {
            resolve_chunk_debug_info(response, stream_state, &blocked_chunk);
        }
        let rejected_chunk = Rc::clone(&blocked_chunk);
        promise.then_with::<(), _, _>(
            move |_| resolve_module_chunk(response, &blocked_chunk, client_reference),
            move |error| trigger_error_on_chunk(response, &rejected_chunk, error.as_unknown()),
        );
    } else {
        match chunk {
            None => {
                let new_chunk = create_resolved_module_chunk(response, client_reference);
                if __DEV__ {
                    resolve_chunk_debug_info(response, stream_state, &new_chunk);
                }
                chunks.set(&id, new_chunk);
            }
            Some(chunk) => {
                if __DEV__ {
                    resolve_chunk_debug_info(response, stream_state, &chunk);
                }
                // This can't actually happen because we don't have any forward
                // references to modules.
                resolve_module_chunk(response, &chunk, client_reference);
            }
        }
    }
}

fn resolve_stream(
    response: &'static Response,
    id: f64,
    stream: &'static Unknown,
    controller: Rc<FlightStreamController>,
    stream_state: &StreamState,
) {
    let chunks = response._chunks.get().unwrap();
    let Some(chunk) = chunks.get(&id) else {
        let new_chunk = create_initialized_stream_chunk(response, stream, controller);
        if __DEV__ {
            resolve_chunk_debug_info(response, stream_state, &new_chunk);
        }
        chunks.set(&id, new_chunk);
        return;
    };
    if __DEV__ {
        resolve_chunk_debug_info(response, stream_state, &chunk);
    }
    if !matches!(*chunk.state.borrow(), ChunkState::Pending { .. }) {
        // We already resolved. We didn't expect to see this.
        return;
    }

    let resolve_listeners = match &mut *chunk.state.borrow_mut() {
        ChunkState::Pending { value, .. } => value.take(),
        _ => None,
    };

    if __DEV__ {
        // Initialize any debug info and block the initializing chunk on any
        // unresolved entries.
        if chunk._debug_chunk.borrow().clone().is_some() {
            let prev_handler = INITIALIZING_HANDLER.take();
            let prev_chunk = INITIALIZING_CHUNK.with_borrow(Clone::clone);
            let cyclic_chunk = &chunk;
            *cyclic_chunk.state.borrow_mut() = ChunkState::Blocked {
                value: None,
                reason: None,
            };
            if (ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
                && ENABLE_COMPONENT_PERFORMANCE_TRACK)
                || __DEV__
            {
                INITIALIZING_CHUNK.set(Some(Rc::clone(cyclic_chunk)));
            }
            let _restore = RestoreInitializingDebug {
                prev_handler,
                prev_chunk,
            };
            initialize_debug_chunk(response, &chunk);
            if let Some(initializing_handler) = INITIALIZING_HANDLER.with_borrow(Clone::clone) {
                if initializing_handler.errored.get() {
                    // Ignore error parsing debug info, we'll report the original error instead.
                } else if initializing_handler.deps.get() > 0.0 {
                    // Leave blocked until we can resolve all the debug info.
                    initializing_handler.value.set(Some(stream));
                    initializing_handler
                        .reason
                        .set(Some(js::unknown_of(controller)));
                    initializing_handler
                        .chunk
                        .replace(Some(Rc::clone(cyclic_chunk)));
                    return;
                }
            }
        }
    }

    let resolved_chunk = &chunk;
    *resolved_chunk.state.borrow_mut() = ChunkState::Initialized {
        value: Some(stream),
        reason: Some(controller),
    };
    if let Some(resolve_listeners) = resolve_listeners {
        wake_chunk(response, resolve_listeners, Some(stream), &chunk);
    } else if __DEV__ {
        process_chunk_debug_info(response, resolved_chunk, Some(stream));
    }
}

fn start_readable_stream(
    response: &'static Response,
    id: f64,
    r#type: Option<&'static str>,
    stream_state: &StreamState,
) {
    let controller: Rc<Cell<Option<&'static ReadableStreamController>>> = Rc::default();
    let closed = Rc::new(Cell::new(false));
    let stream = webapi::readable_stream::new(
        Some(&UnderlyingSource {
            r#type,
            start: Some(&|c| controller.set(Some(c))),
            ..Default::default()
        }),
        None,
    );
    let previous_blocked_chunk: Rc<RefCell<Option<Chunk>>> = Rc::default();
    let flight_controller = Rc::new(FlightStreamController {
        enqueue_value: Box::new({
            let controller = Rc::clone(&controller);
            let previous_blocked_chunk = Rc::clone(&previous_blocked_chunk);
            move |value| {
                let Some(previous) = previous_blocked_chunk.borrow().clone() else {
                    controller.get().unwrap().enqueue(Some(&value));
                    return;
                };
                // We're still waiting on a previous chunk so we can't enqueue quite yet.
                let controller = Rc::clone(&controller);
                previous_blocked_chunk_ref(&previous).then(
                    Some(Rc::new(move |_| {
                        controller.get().unwrap().enqueue(Some(&value));
                    })),
                    None,
                );
            }
        }),
        enqueue_model: Box::new({
            let controller = Rc::clone(&controller);
            let previous_blocked_chunk = Rc::clone(&previous_blocked_chunk);
            move |json| {
                let previous = previous_blocked_chunk.borrow().clone();
                match previous {
                    None => {
                        // If we're not blocked on any other chunks, we can try to eagerly initialize
                        // this as a fast-path to avoid awaiting them.
                        let chunk = create_resolved_model_chunk(response, json);
                        initialize_model_chunk(&chunk);
                        let initialized_chunk = &chunk;
                        if matches!(
                            *initialized_chunk.state.borrow(),
                            ChunkState::Initialized { .. }
                        ) {
                            controller
                                .get()
                                .unwrap()
                                .enqueue(Some(&initialized_value(initialized_chunk)));
                        } else {
                            let resolve_controller = Rc::clone(&controller);
                            let reject_controller = Rc::clone(&controller);
                            chunk.then(
                                Some(Rc::new(move |v| {
                                    resolve_controller.get().unwrap().enqueue(Some(&v))
                                })),
                                Some(Rc::new(move |e| {
                                    reject_controller.get().unwrap().error(Some(&e))
                                })),
                            );
                            *previous_blocked_chunk.borrow_mut() = Some(chunk);
                        }
                    }
                    Some(blocked_chunk) => {
                        // We're still waiting on a previous chunk so we can't enqueue quite yet.
                        let chunk = create_pending_chunk(response);
                        let resolve_controller = Rc::clone(&controller);
                        let reject_controller = Rc::clone(&controller);
                        chunk.then(
                            Some(Rc::new(move |v| {
                                resolve_controller.get().unwrap().enqueue(Some(&v))
                            })),
                            Some(Rc::new(move |e| {
                                reject_controller.get().unwrap().error(Some(&e))
                            })),
                        );
                        *previous_blocked_chunk.borrow_mut() = Some(Rc::clone(&chunk));
                        let previous_blocked_chunk = Rc::clone(&previous_blocked_chunk);
                        blocked_chunk.then(
                            Some(Rc::new(move |_| {
                                if previous_blocked_chunk
                                    .borrow()
                                    .as_ref()
                                    .is_some_and(|previous| Rc::ptr_eq(previous, &chunk))
                                {
                                    // We were still the last chunk so we can now clear the queue and return
                                    // to synchronous emitting.
                                    *previous_blocked_chunk.borrow_mut() = None;
                                }
                                resolve_model_chunk(response, &chunk, json.clone());
                            })),
                            None,
                        );
                    }
                }
            }
        }),
        close: Box::new({
            let controller = Rc::clone(&controller);
            let previous_blocked_chunk = Rc::clone(&previous_blocked_chunk);
            let closed = Rc::clone(&closed);
            move |_json| {
                if closed.get() {
                    return;
                }
                closed.set(true);
                let previous = previous_blocked_chunk.borrow_mut().take();
                match previous {
                    None => controller.get().unwrap().close(),
                    Some(blocked_chunk) => {
                        // We shouldn't get any more enqueues after this so we can set it back to null.
                        let controller = Rc::clone(&controller);
                        blocked_chunk.then(
                            Some(Rc::new(move |_| controller.get().unwrap().close())),
                            None,
                        );
                    }
                }
            }
        }),
        error: Box::new({
            let controller = Rc::clone(&controller);
            let previous_blocked_chunk = Rc::clone(&previous_blocked_chunk);
            let closed = Rc::clone(&closed);
            move |error| {
                if closed.get() {
                    return;
                }
                closed.set(true);
                let previous = previous_blocked_chunk.borrow_mut().take();
                match previous {
                    None => controller.get().unwrap().error(Some(&error)),
                    Some(blocked_chunk) => {
                        // We shouldn't get any more enqueues after this so we can set it back to null.
                        let controller = Rc::clone(&controller);
                        blocked_chunk.then(
                            Some(Rc::new(move |_| {
                                controller.get().unwrap().error(Some(&error))
                            })),
                            None,
                        );
                    }
                }
            }
        }),
    });
    resolve_stream(
        response,
        id,
        js::unknown(stream),
        flight_controller,
        stream_state,
    );
}

fn previous_blocked_chunk_ref(chunk: &Chunk) -> &Chunk {
    chunk
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "asyncIterator")]
    static ASYNC_ITERATOR_SELF: &'static dyn Fn() -> Option<&'static Unknown> =
        function::with_this(|this, ()| {
            // Self referencing iterator.
            this
        });
}

fn create_iterator(next: &'static dyn Fn(Option<&'static Unknown>) -> Chunk) -> &'static Unknown {
    let iterator = js::unknown(object::from_entries(vec![
        ("next".to_owned(), js::unknown(next)),
        // TODO: Add return/throw as options for aborting.
    ]));
    // TODO: The iterator could inherit the AsyncIterator prototype which is not exposed as
    // a global but exists as a prototype of an AsyncGenerator. However, it's not needed
    // to satisfy the iterable protocol.
    ASYNC_ITERATOR.with(|&async_iterator| {
        js::set(iterator, async_iterator, ASYNC_ITERATOR_SELF.with(|&f| f))
    });
    iterator
}

fn start_async_iterable(
    response: &'static Response,
    id: f64,
    iterator: bool,
    stream_state: &StreamState,
) {
    let buffer: Rc<RefCell<Vec<Chunk>>> = Rc::default();
    let closed = Rc::new(Cell::new(false));
    let next_write_index = Rc::new(Cell::new(0usize));
    let flight_controller = Rc::new(FlightStreamController {
        enqueue_value: Box::new({
            let buffer = Rc::clone(&buffer);
            let next_write_index = Rc::clone(&next_write_index);
            move |value| {
                if next_write_index.get() == buffer.borrow().len() {
                    let chunk = create_initialized_iterator_result_chunk(response, value, false);
                    buffer.borrow_mut().push(chunk);
                } else {
                    let chunk = Rc::clone(&buffer.borrow()[next_write_index.get()]);
                    let (resolve_listeners, reject_listeners) = match &mut *chunk.state.borrow_mut()
                    {
                        ChunkState::Pending { value, reason } => (value.take(), reason.take()),
                        _ => (None, None),
                    };
                    let initialized_chunk = &chunk;
                    *initialized_chunk.state.borrow_mut() = ChunkState::Initialized {
                        value: Some(js::unknown_of(IteratorResultValue { done: false, value })),
                        reason: None,
                    };
                    if let Some(resolve_listeners) = resolve_listeners {
                        wake_chunk_if_initialized(
                            response,
                            &chunk,
                            resolve_listeners,
                            reject_listeners,
                        );
                    }
                }
                next_write_index.set(next_write_index.get() + 1);
            }
        }),
        enqueue_model: Box::new({
            let buffer = Rc::clone(&buffer);
            let next_write_index = Rc::clone(&next_write_index);
            move |value| {
                if next_write_index.get() == buffer.borrow().len() {
                    let chunk = create_resolved_iterator_result_chunk(response, value, false);
                    buffer.borrow_mut().push(chunk);
                } else {
                    let chunk = Rc::clone(&buffer.borrow()[next_write_index.get()]);
                    resolve_iterator_result_chunk(response, &chunk, value, false);
                }
                next_write_index.set(next_write_index.get() + 1);
            }
        }),
        close: Box::new({
            let buffer = Rc::clone(&buffer);
            let next_write_index = Rc::clone(&next_write_index);
            let closed = Rc::clone(&closed);
            move |value| {
                if closed.get() {
                    return;
                }
                closed.set(true);
                if next_write_index.get() == buffer.borrow().len() {
                    let chunk = create_resolved_iterator_result_chunk(response, value, true);
                    buffer.borrow_mut().push(chunk);
                } else {
                    let chunk = Rc::clone(&buffer.borrow()[next_write_index.get()]);
                    resolve_iterator_result_chunk(response, &chunk, value, true);
                }
                next_write_index.set(next_write_index.get() + 1);
                while next_write_index.get() < buffer.borrow().len() {
                    // In generators, any extra reads from the iterator have the value undefined.
                    let chunk = Rc::clone(&buffer.borrow()[next_write_index.get()]);
                    next_write_index.set(next_write_index.get() + 1);
                    resolve_iterator_result_chunk(
                        response,
                        &chunk,
                        "\"$undefined\"".to_owned(),
                        true,
                    );
                }
            }
        }),
        error: Box::new({
            let buffer = Rc::clone(&buffer);
            let next_write_index = Rc::clone(&next_write_index);
            let closed = Rc::clone(&closed);
            move |error| {
                if closed.get() {
                    return;
                }
                closed.set(true);
                if next_write_index.get() == buffer.borrow().len() {
                    let chunk = create_pending_chunk(response);
                    buffer.borrow_mut().push(chunk);
                }
                while next_write_index.get() < buffer.borrow().len() {
                    let chunk = Rc::clone(&buffer.borrow()[next_write_index.get()]);
                    next_write_index.set(next_write_index.get() + 1);
                    trigger_error_on_chunk(response, &chunk, error);
                }
            }
        }),
    });

    let iterable = js::unknown(object::spread(None));
    let make_iterator: &'static dyn Fn() -> &'static Unknown = function::with_this({
        let buffer = Rc::clone(&buffer);
        let closed = Rc::clone(&closed);
        move |_, ()| {
            let next_read_index = Rc::new(Cell::new(0usize));
            let buffer = Rc::clone(&buffer);
            let closed = Rc::clone(&closed);
            create_iterator(function::with_this(
                move |_, (arg,): (Option<&'static Unknown>,)| {
                    if !matches!(js::classify(arg), Kind::Undefined) {
                        panic!(
                            "Values cannot be passed to next() of AsyncIterables passed to Client Components."
                        );
                    }
                    if next_read_index.get() == buffer.borrow().len() {
                        if closed.get() {
                            return Rc::new(ReactPromise::new(ChunkState::Initialized {
                                value: Some(js::unknown_of(IteratorResultValue {
                                    done: true,
                                    value: None,
                                })),
                                reason: None,
                            }));
                        }
                        let chunk = create_pending_chunk(response);
                        buffer.borrow_mut().push(chunk);
                    }
                    let chunk = Rc::clone(&buffer.borrow()[next_read_index.get()]);
                    next_read_index.set(next_read_index.get() + 1);
                    chunk
                },
            ))
        }
    });
    ASYNC_ITERATOR.with(|&async_iterator| js::set(iterable, async_iterator, make_iterator));

    // TODO: If it's a single shot iterator we can optimize memory by cleaning up the buffer after
    // reading through the end, but currently we favor code size over this optimization.
    resolve_stream(
        response,
        id,
        if iterator { make_iterator() } else { iterable },
        flight_controller,
        stream_state,
    );
}

fn stop_stream(response: &'static Response, id: f64, row: UninitializedModel) {
    let chunks = response._chunks.get().unwrap();
    let Some(chunk) = chunks
        .get(&id)
        .filter(|chunk| matches!(*chunk.state.borrow(), ChunkState::Initialized { .. }))
    else {
        // We didn't expect not to have an existing stream;
        return;
    };
    if __DEV__ {
        let pending_chunks = response._pending_chunks.get().unwrap_or(0.0) - 1.0;
        response._pending_chunks.set(Some(pending_chunks));
        if pending_chunks == 0.0 {
            // We're no longer waiting for any more chunks. We can release the strong
            // reference to the response. We'll regain it if we ask for any more data
            // later on.
            response
                ._weak_response
                .borrow()
                .as_ref()
                .unwrap()
                .response
                .set(None);
        }
    }
    let controller = stream_controller(&chunk);
    (controller.close)(if row.is_empty() {
        "\"$undefined\"".to_owned()
    } else {
        row
    });
}

fn resolve_error_prod(_response: &'static Response) -> &'static js::Error {
    if __DEV__ {
        // These errors should never make it into a build so we don't need to encode them in codes.json
        // eslint-disable-next-line react-internal/prod-error-codes
        panic!(
            "resolveErrorProd should never be called in development mode. Use resolveErrorDev instead. This is a bug in React."
        );
    }
    let error = js::error::new(
        Some(concat!(
            "An error occurred in the Server Components render. The specific message is omitted in production",
            " builds to avoid leaking sensitive details. A digest property is included on this error instance which",
            " may provide additional details about the nature of the error.",
        )),
        None,
    );
    js::set(error, "stack", "Error: ".to_owned() + &error.message());
    error
}

fn resolve_error_dev(
    response: &'static Response,
    error_info: &'static ReactErrorInfoDev,
) -> &'static Unknown {
    let name = &error_info.name;
    let message = &error_info.message;
    let stack = &error_info.stack;
    let env = &error_info.env;

    if !__DEV__ {
        // These errors should never make it into a build so we don't need to encode them in codes.json
        // eslint-disable-next-line react-internal/prod-error-codes
        panic!(
            "resolveErrorDev should never be called in production mode. Use resolveErrorProd instead. This is a bug in React."
        );
    }

    // We don't serialize Error.cause in prod so we never need to deserialize
    let error_options = if __DEV__ && js::has(error_info, "cause") {
        Some(js::unknown(object::from_entries(vec![(
            "cause".to_owned(),
            or_undefined(revive_model(
                response,
                error_info.cause,
                js::unknown(error_info),
                "cause",
            )),
        )])))
    } else {
        None
    };
    let is_aggregate_error =
        js::is_defined(js::aggregate_error::new) && js::has(error_info, "errors");
    // We don't serialize AggregateError.errors in prod so we never need to deserialize
    let revived_errors = if __DEV__ && is_aggregate_error {
        revive_model(
            response,
            error_info.errors,
            js::unknown(error_info),
            "errors",
        )
    } else {
        js::null()
    };
    let message = if message.is_empty() {
        "An error occurred in the Server Components render but no message was provided"
    } else {
        message
    };
    // SAFETY: `{cause}` is an Error's options.
    let error_options: Option<&'static js::ErrorOptions<'static>> =
        error_options.map(|options| unsafe { js::cast(Some(options)) });
    let inner_call: &'static dyn Fn() -> &'static js::Error = if is_aggregate_error {
        // SAFETY: revived errors are a list.
        let errors: &'static Vec<Option<&'static Unknown>> = unsafe { js::cast(revived_errors) };
        let bound = function::bind(
            js::aggregate_error::new,
            Some(&Kind::Null),
            (&errors[..], Some(message), error_options),
        );
        // SAFETY: an AggregateError is an Error.
        unsafe { js::cast(Some(js::unknown(bound))) }
    } else {
        function::bind(
            js::error::new,
            Some(&Kind::Null),
            (Some(message), error_options),
        )
    };
    let call_stack = build_fake_call_stack(response, stack, env, false, inner_call);

    let mut owner_task: Option<&'static ConsoleTask> = None;
    if let Some(owner) = &error_info.owner {
        let owner_ref = js::string::slice(owner, 1.0, None);
        // TODO: This is not resilient to the owner loading later in an Error like a debug channel.
        // The whole error serialization should probably go through the regular model at least for DEV.
        let owner = get_outlined_model(
            response,
            &owner_ref,
            js::unknown(object::spread(None)),
            "",
            create_model,
        );
        if let Some(owner) = owner {
            owner_task = initialize_fake_task(response, owner);
        }
    }

    let error = match owner_task {
        None => {
            let root_task = get_root_task(response, env);
            if let Some(root_task) = root_task {
                root_task.run(call_stack)
            } else {
                call_stack()
            }
        }
        Some(owner_task) => owner_task.run(call_stack),
    };

    js::set(error, "name", name.as_str());
    js::set(error, "environmentName", env.as_str());
    js::unknown(error)
}

fn resolve_error_model(
    response: &'static Response,
    id: f64,
    row: UninitializedModel,
    stream_state: &StreamState,
) {
    let chunks = response._chunks.get().unwrap();
    let chunk = chunks.get(&id);
    let error_info = js::json::parse(&row).unwrap();
    let error: &'static Unknown = if __DEV__ {
        // SAFETY: a DEV error row is a DEV error's info.
        resolve_error_dev(response, unsafe { js::cast(error_info) })
    } else {
        js::unknown(resolve_error_prod(response))
    };
    js::set(error, "digest", js::get(error_info, "digest"));
    let error_with_digest = error;
    match chunk {
        None => {
            let new_chunk = create_error_chunk(response, Some(error_with_digest));
            if __DEV__ {
                resolve_chunk_debug_info(response, stream_state, &new_chunk);
            }
            chunks.set(&id, new_chunk);
        }
        Some(chunk) => {
            if __DEV__ {
                resolve_chunk_debug_info(response, stream_state, &chunk);
            }
            trigger_error_on_chunk(response, &chunk, Some(error_with_digest));
        }
    }
}

fn resolve_hint(response: &'static Response, code: &str, model: UninitializedModel) {
    let hint_model = parse_model(response, &model);
    dispatch_hint(code, hint_model);
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "supportsCreateTask")]
    static SUPPORTS_CREATE_TASK: bool = __DEV__ && console::create_task().is_some();
}

/// A fake function of a server frame: called with what's inside it, it
/// calls that, its frame on the stack.
type FakeFunction =
    dyn Fn(&'static dyn Fn() -> Option<&'static Unknown>) -> Option<&'static Unknown>;

thread_local! {
    #[cfg_attr(rust_js, rust_js::nullable)]
    static FAKE_FUNCTION_CACHE: Option<&'static js::Map<str, &'static FakeFunction>> =
        if __DEV__ { Some(js::map::new(None)) } else { None };
    static FAKE_FUNCTION_IDX: Cell<f64> = const { Cell::new(0.0) };
}

#[allow(clippy::too_many_arguments)] // upstream's eight positional parameters.
fn create_fake_function(
    name: &str,
    filename: &str,
    source_map: Option<&str>,
    line: f64,
    col: f64,
    enclosing_line: f64,
    enclosing_col: f64,
    environment_name: &str,
) -> &'static FakeFunction {
    // This creates a fake copy of a Server Module. It represents a module that has already
    // executed on the server but we re-execute a blank copy for its stack frames on the client.

    let comment = "/* This module was rendered by a Server Component. Turn on Source Maps to see the server source. */";

    let mut name = name.to_owned();
    if name.is_empty() {
        // An eval:ed function with no name gets the name "eval". We give it something more descriptive.
        name = "<anonymous>".to_owned();
    }
    let encoded_name = js::json::stringify(&name);
    // We generate code where the call is at the line and column of the server executed code.
    // This allows us to use the original source map as the source map of this fake file to
    // point to the original source.
    let mut code;
    let (mut line, mut col, mut enclosing_line, mut enclosing_col) =
        (line, col, enclosing_line, enclosing_col);
    // Normalize line/col to zero based.
    if enclosing_line < 1.0 {
        enclosing_line = 0.0;
    } else {
        enclosing_line -= 1.0;
    }
    if enclosing_col < 1.0 {
        enclosing_col = 0.0;
    } else {
        enclosing_col -= 1.0;
    }
    if line < 1.0 {
        line = 0.0;
    } else {
        line -= 1.0;
    }
    if col < 1.0 {
        col = 0.0;
    } else {
        col -= 1.0;
    }
    if line < enclosing_line || (line == enclosing_line && col < enclosing_col) {
        // Protection against invalid enclosing information. Should not happen.
        enclosing_line = 0.0;
        enclosing_col = 0.0;
    }
    if line < 1.0 {
        // Fit everything on the first line.
        let min_col = js::string::length(&encoded_name) as f64 + 3.0;
        let mut enclosing_col_distance = enclosing_col - min_col;
        if enclosing_col_distance < 0.0 {
            enclosing_col_distance = 0.0;
        }
        let mut col_distance = col - enclosing_col_distance - min_col - 3.0;
        if col_distance < 0.0 {
            col_distance = 0.0;
        }
        code = "({".to_owned()
            + &encoded_name
            + ":"
            + &js::string::repeat(" ", enclosing_col_distance)
            + "_=>"
            + &js::string::repeat(" ", col_distance)
            + "_()})";
    } else if enclosing_line < 1.0 {
        // Fit just the enclosing function on the first line.
        let min_col = js::string::length(&encoded_name) as f64 + 3.0;
        let mut enclosing_col_distance = enclosing_col - min_col;
        if enclosing_col_distance < 0.0 {
            enclosing_col_distance = 0.0;
        }
        code = "({".to_owned()
            + &encoded_name
            + ":"
            + &js::string::repeat(" ", enclosing_col_distance)
            + "_=>"
            + &js::string::repeat("\n", line - enclosing_line)
            + &js::string::repeat(" ", col)
            + "_()})";
    } else if enclosing_line == line {
        // Fit the enclosing function and callsite on same line.
        let mut col_distance = col - enclosing_col - 3.0;
        if col_distance < 0.0 {
            col_distance = 0.0;
        }
        code = js::string::repeat("\n", enclosing_line - 1.0)
            + "({"
            + &encoded_name
            + ":\n"
            + &js::string::repeat(" ", enclosing_col)
            + "_=>"
            + &js::string::repeat(" ", col_distance)
            + "_()})";
    } else {
        // This is the ideal because we can always encode any position.
        code = js::string::repeat("\n", enclosing_line - 1.0)
            + "({"
            + &encoded_name
            + ":\n"
            + &js::string::repeat(" ", enclosing_col)
            + "_=>"
            + &js::string::repeat("\n", line - enclosing_line)
            + &js::string::repeat(" ", col)
            + "_()})";
    }

    if enclosing_line < 1.0 {
        // If the function starts at the first line, we append the comment after.
        code = code + "\n" + comment;
    } else {
        // Otherwise we prepend the comment on the first line.
        code = comment.to_owned() + &code;
    }

    let mut filename = filename.to_owned();
    if filename.starts_with('/') {
        // If the filename starts with `/` we assume that it is a file system file
        // rather than relative to the current host. Since on the server fully qualified
        // stack traces use the file path.
        // TODO: What does this look like on Windows?
        filename = "file://".to_owned() + &filename;
    }

    if let Some(source_map) = source_map.filter(|source_map| !source_map.is_empty()) {
        // We use the prefix about://React/ to separate these from other files listed in
        // the Chrome DevTools. We need a "host name" and not just a protocol because
        // otherwise the group name becomes the root folder. Ideally we don't want to
        // show these at all but there's two reasons to assign a fake URL.
        // 1) A printed stack trace string needs a unique URL to be able to source map it.
        // 2) If source maps are disabled or fails, you should at least be able to tell
        //    which file it was.
        let idx = FAKE_FUNCTION_IDX.get();
        FAKE_FUNCTION_IDX.set(idx + 1.0);
        code += &("\n//# sourceURL=about://React/".to_owned()
            + &js::encode_uri_component(environment_name)
            + "/"
            + &js::encode_uri(&filename)
            + "?"
            + &js::concat_text(idx));
        code += &("\n//# sourceMappingURL=".to_owned() + source_map);
    } else if !filename.is_empty() {
        code += &("\n//# sourceURL=".to_owned() + &js::encode_uri(&filename));
    } else {
        code += "\n//# sourceURL=<anonymous>";
    }

    // eslint-disable-next-line no-eval
    match js::eval(&code).map(|module| js::get(module, &*name)) {
        // SAFETY: the code evaluates to an object whose `name` is the fake function.
        Ok(r#fn) => unsafe { js::cast(r#fn) },
        Err(_) => {
            // If eval fails, such as if in an environment that doesn't support it,
            // we fallback to creating a function here. It'll still have the right
            // name but it'll lose line/column number and file name.
            let r#fn: &'static FakeFunction = function::with_this(
                |_, (callback,): (&'static dyn Fn() -> Option<&'static Unknown>,)| callback(),
            );
            // Using the usual {[name]: _() => _()}.bind() trick to avoid minifiers
            // doesn't work here since this will produce `Object.*` names.
            object::define_property(
                r#fn,
                "name",
                &PropertyDescriptor {
                    value: Some(&name),
                    configurable: None,
                    enumerable: None,
                    writable: None,
                    get: None,
                    set: None,
                },
            );
            r#fn
        }
    }
}

fn build_fake_call_stack<T: 'static>(
    response: &'static Response,
    stack: &ReactStackTrace,
    environment_name: &str,
    use_enclosing_line: bool,
    inner_call: &'static dyn Fn() -> T,
) -> &'static dyn Fn() -> T {
    let mut call_stack = inner_call;
    let fake_function_cache = FAKE_FUNCTION_CACHE.with(|&cache| cache).unwrap();
    for frame in stack {
        let frame_key = js::array::join(&frame_values(frame), "-")
            + "-"
            + environment_name
            + if use_enclosing_line { "-e" } else { "-n" };
        let mut r#fn = fake_function_cache.get(&*frame_key);
        if r#fn.is_none() {
            let (name, filename, line, col, enclosing_line, enclosing_col, _) = frame;
            let find_source_map_url = response._debug_find_source_map_url.get();
            let source_map = match find_source_map_url {
                Some(find_source_map_url) => find_source_map_url(filename, environment_name),
                None => None,
            };
            let new_fn = create_fake_function(
                name,
                filename,
                source_map.as_deref(),
                *line,
                *col,
                if use_enclosing_line {
                    *line
                } else {
                    *enclosing_line
                },
                if use_enclosing_line {
                    *col
                } else {
                    *enclosing_col
                },
                environment_name,
            );
            // TODO: This cache should technically live on the response since the _debugFindSourceMapURL
            // function is an input and can vary by response.
            fake_function_cache.set(&*frame_key, new_fn);
            r#fn = Some(new_fn);
        }
        // SAFETY: a fake function calls the function it's given and gives
        // what that gives, of any type, as upstream's `<T>` has it.
        let r#fn: &'static dyn Fn(&'static dyn Fn() -> T) -> T =
            unsafe { js::cast(Some(js::unknown(r#fn.unwrap()))) };
        call_stack = function::bind(r#fn, Some(&Kind::Null), (call_stack,));
    }
    call_stack
}

/// A frame's values, as `join` reads them.
fn frame_values(frame: &shared::react_types::ReactCallSite) -> Vec<Option<&'static Unknown>> {
    let (name, filename, line, col, enclosing_line, enclosing_col, is_async) = frame;
    vec![
        Some(js::unknown_of(name.clone())),
        Some(js::unknown_of(filename.clone())),
        Some(js::unknown_of(*line)),
        Some(js::unknown_of(*col)),
        Some(js::unknown_of(*enclosing_line)),
        Some(js::unknown_of(*enclosing_col)),
        Some(js::unknown_of(*is_async)),
    ]
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn get_root_task(
    response: &'static Response,
    child_environment_name: &str,
) -> Option<&'static ConsoleTask> {
    let root_task = response._debug_root_task.get()?;
    if response._root_environment_name.borrow().as_deref() != Some(child_environment_name) {
        // If the root most owner component is itself in a different environment than the requested
        // environment then we create an extra task to indicate that we're transitioning into it.
        // Like if one environment just requests another environment.
        let create_task_fn = function::bind(
            console::create_task().unwrap(),
            Some(&console),
            ("\"use ".to_owned() + &child_environment_name.to_lowercase() + "\"",),
        );
        return Some(root_task.run(create_task_fn));
    }
    Some(root_task)
}

/// A debug info entry's environment, or none.
fn env_of(debug_info: Option<&'static Unknown>) -> Option<String> {
    match js::classify(js::get(debug_info, "env")) {
        Kind::String(env) => Some(env.to_owned()),
        _ => None,
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn initialize_fake_task(
    response: &'static Response,
    debug_info: &'static Unknown,
) -> Option<&'static ConsoleTask> {
    if !SUPPORTS_CREATE_TASK.with(|&supports| supports) {
        return None;
    }
    if js::loose_equals(js::get(debug_info, "stack"), js::null::<&Unknown>()) {
        // If this is an error, we should've really already initialized the task.
        // If it's null, we can't initialize a task.
        return None;
    }
    let cached_entry = js::get(debug_info, "debugTask");
    if !matches!(js::classify(cached_entry), Kind::Undefined) {
        // SAFETY: a set `debugTask` is a task or null.
        return unsafe { js::cast(cached_entry) };
    }

    // Workaround for a bug where Chrome Performance tracking uses the enclosing line/column
    // instead of the callsite. For ReactAsyncInfo/ReactIOInfo, the only thing we're going
    // to use the fake task for is the Performance tracking so we encode the enclosing line/
    // column at the callsite to get a better line number. We could do this for Components too
    // but we're going to use those for other things too like console logs and it's not worth
    // duplicating. If this bug is every fixed in Chrome, this should be set to false.
    let use_enclosing_line = matches!(js::classify(js::get(debug_info, "key")), Kind::Undefined);

    // SAFETY: a non-null `stack` is a stack trace.
    let stack: &ReactStackTrace = unsafe { js::cast(js::get(debug_info, "stack")) };
    let root_environment_name = response._root_environment_name.borrow().clone().unwrap();
    let env = env_of(Some(debug_info)).unwrap_or_else(|| root_environment_name.clone());
    let owner = js::get(debug_info, "owner");
    let owner_env = if js::loose_equals(owner, js::null::<&Unknown>()) {
        root_environment_name
    } else {
        env_of(owner).unwrap_or(root_environment_name)
    };
    let owner_task = if js::loose_equals(owner, js::null::<&Unknown>()) {
        None
    } else {
        initialize_fake_task(response, owner.unwrap())
    };
    let task_name =
        // This is the boundary between two environments so we'll annotate the task name.
        // We assume that the stack frame of the entry into the new environment was done
        // from the old environment. So we use the owner's environment as the current.
        if env != owner_env {
            "\"use ".to_owned() + &env.to_lowercase() + "\""
        } else if !matches!(js::classify(js::get(debug_info, "key")), Kind::Undefined) {
            // Some unfortunate pattern matching to refine the type.
            // SAFETY: an entry with a `key` is a component's info.
            get_server_component_task_name(unsafe { js::cast(Some(debug_info)) })
        } else if !matches!(js::classify(js::get(debug_info, "name")), Kind::Undefined) {
            // SAFETY: an entry with a `name` is an I/O's info.
            get_io_info_task_name(unsafe { js::cast(Some(debug_info)) })
        } else {
            // SAFETY: the rest is an await's info.
            get_async_info_task_name(unsafe { js::cast(Some(debug_info)) })
        };
    let task = build_fake_task(
        response,
        owner_task,
        stack,
        &task_name,
        &owner_env,
        use_enclosing_line,
    );
    js::set(debug_info, "debugTask", task);
    Some(task)
}

fn build_fake_task(
    response: &'static Response,
    owner_task: Option<&'static ConsoleTask>,
    stack: &ReactStackTrace,
    task_name: &str,
    env: &str,
    use_enclosing_line: bool,
) -> &'static ConsoleTask {
    let create_task_fn = function::bind(
        console::create_task().unwrap(),
        Some(&console),
        (task_name.to_owned(),),
    );
    let call_stack =
        build_fake_call_stack(response, stack, env, use_enclosing_line, create_task_fn);
    match owner_task {
        None => {
            let root_task = get_root_task(response, env);
            if let Some(root_task) = root_task {
                root_task.run(call_stack)
            } else {
                call_stack()
            }
        }
        Some(owner_task) => owner_task.run(call_stack),
    }
}

type CreateFakeJSXCallStack = fn(&'static Response, &ReactStackTrace, &str) -> &'static Unknown;

struct CreateFakeJSXCallStackFrame {
    #[cfg_attr(rust_js, rust_js::name = "react_stack_bottom_frame")]
    react_stack_bottom_frame: CreateFakeJSXCallStack,
}

#[cfg_attr(rust_js, rust_js::name = "createFakeJSXCallStack")]
const CREATE_FAKE_JSX_CALL_STACK: CreateFakeJSXCallStackFrame = CreateFakeJSXCallStackFrame {
    react_stack_bottom_frame: |response, stack, environment_name| {
        let call_stack_for_error = build_fake_call_stack(
            response,
            stack,
            environment_name,
            false,
            FAKE_JSX_CALL_SITE.with(|&f| f),
        );
        call_stack_for_error()
    },
};

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "createFakeJSXCallStackInDEV")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    static CREATE_FAKE_JSX_CALL_STACK_IN_DEV: Option<CreateFakeJSXCallStack> = if __DEV__ {
        // We use this technique to trick minifiers to preserve the function name.
        Some(function::bind(
            CREATE_FAKE_JSX_CALL_STACK.react_stack_bottom_frame,
            Some(&CREATE_FAKE_JSX_CALL_STACK),
            (),
        ))
    } else {
        None
    };
}

// v8 (Chromium, Node.js) defaults to 10
// SpiderMonkey (Firefox) does not support Error.stackTraceLimit
// JSC (Safari) defaults to 100
// The lower the limit, the more likely we'll not reach react_stack_bottom_frame
// The higher the limit, the slower Error() is when not inspecting with a debugger.
// When inspecting with a debugger, Error.stackTraceLimit has no impact on Error() performance (in v8).
#[cfg_attr(rust_js, rust_js::name = "ownerStackTraceLimit")]
const OWNER_STACK_TRACE_LIMIT: f64 = 10.0;

thread_local! {
    /* @noinline */
    #[cfg_attr(rust_js, rust_js::name = "fakeJSXCallSite")]
    static FAKE_JSX_CALL_SITE: &'static dyn Fn() -> &'static Unknown =
        function::with_this(|_, ()| {
            // This extra call frame represents the JSX creation function. We always pop this frame
            // off before presenting so it needs to be part of the stack.
            let previous_stack_trace_limit = js_error::stack_trace_limit();
            js_error::set_stack_trace_limit(OWNER_STACK_TRACE_LIMIT);
            let error = js::error::new(Some("react-stack-top-frame"), None); // eslint-disable-line prefer-const
            js_error::set_stack_trace_limit(previous_stack_trace_limit);
            js::unknown(error)
        });
}

fn initialize_fake_stack(response: &'static Response, debug_info: &'static Unknown) {
    let cached_entry = js::get(debug_info, "debugStack");
    if !matches!(js::classify(cached_entry), Kind::Undefined) {
        return;
    }
    let stack = js::get(debug_info, "stack");
    if !js::loose_equals(stack, js::null::<&Unknown>()) {
        // SAFETY: a non-null `stack` is a stack trace.
        let stack: &ReactStackTrace = unsafe { js::cast(stack) };
        let env = env_of(Some(debug_info)).unwrap_or_default();
        js::set(
            debug_info,
            "debugStack",
            CREATE_FAKE_JSX_CALL_STACK_IN_DEV.with(
                // SAFETY: a fake stack is DEV's, and it's set in DEV.
                |&f| unsafe { f.unwrap_unchecked() },
            )(response, stack, &env),
        );
    }
    let owner = js::get(debug_info, "owner");
    if let Some(owner) = owner.filter(|_| !js::loose_equals(owner, js::null::<&Unknown>())) {
        // Initialize any owners not yet initialized.
        initialize_fake_stack(response, owner);
        if matches!(
            js::classify(js::get(owner, "debugLocation")),
            Kind::Undefined
        ) && !js::loose_equals(js::get(debug_info, "debugStack"), js::null::<&Unknown>())
        {
            // If we are the child of this owner, then the owner should be the bottom frame
            // our stack. We can use it as the implied location of the owner.
            js::set(owner, "debugLocation", js::get(debug_info, "debugStack"));
        }
    }
}

fn initialize_debug_info(
    response: &'static Response,
    debug_info: &'static ReactDebugInfoEntry,
) -> &'static ReactDebugInfoEntry {
    if !__DEV__ {
        // These errors should never make it into a build so we don't need to encode them in codes.json
        // eslint-disable-next-line react-internal/prod-error-codes
        panic!(
            "initializeDebugInfo should never be called in production mode. This is a bug in React."
        );
    }
    if !matches!(js::classify(js::get(debug_info, "stack")), Kind::Undefined) {
        let component_info_or_async_info = debug_info;
        // We eagerly initialize the fake task because this resolving happens outside any
        // render phase so we're not inside a user space stack at this point. If we waited
        // to initialize it when we need it, we might be inside user code.
        initialize_fake_task(response, component_info_or_async_info);
    }
    if js::loose_equals(js::get(debug_info, "owner"), js::null::<&Unknown>())
        && let Some(root_owner) = response._debug_root_owner.get()
    {
        let component_info_or_async_info = debug_info;
        js::set(component_info_or_async_info, "owner", root_owner);
        // We clear the parsed stack frames to indicate that it needs to be re-parsed from debugStack.
        js::set(component_info_or_async_info, "stack", Kind::Null);
        // We override the stack if we override the owner since the stack where the root JSX
        // was created on the server isn't very useful but where the request was made is.
        js::set(
            component_info_or_async_info,
            "debugStack",
            response._debug_root_stack.get(),
        );
        js::set(
            component_info_or_async_info,
            "debugTask",
            response._debug_root_task.get(),
        );
    } else if !matches!(js::classify(js::get(debug_info, "stack")), Kind::Undefined) {
        let component_info_or_async_info = debug_info;
        initialize_fake_stack(response, component_info_or_async_info);
    }
    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        && ENABLE_COMPONENT_PERFORMANCE_TRACK
        && let Kind::Number(time) = js::classify(js::get(debug_info, "time"))
    {
        // Adjust the time to the current environment's time space.
        // Since this might be a deduped object, we clone it to avoid
        // applying the adjustment twice.
        return js::unknown_of(ReactTimeInfo {
            time: time + response._time_origin.get().unwrap(),
        });
    }
    debug_info
}

fn resolve_debug_model(response: &'static Response, id: f64, json: UninitializedModel) {
    let parent_chunk = get_chunk(response, id);
    if matches!(
        *parent_chunk.state.borrow(),
        ChunkState::Initialized { .. }
            | ChunkState::Errored { .. }
            | ChunkState::Halted { .. }
            | ChunkState::Blocked { .. }
    ) {
        // We shouldn't really get debug info late. It's too late to add it after we resolved.
        return;
    }
    if matches!(
        *parent_chunk.state.borrow(),
        ChunkState::ResolvedModule { .. }
    ) {
        // We don't expect to get debug info on modules.
        return;
    }
    let previous_chunk = parent_chunk._debug_chunk.take();
    let debug_chunk = create_resolved_model_chunk(response, json.clone());
    debug_chunk._debug_chunk.replace(previous_chunk); // Linked list of the debug chunks
    parent_chunk
        ._debug_chunk
        .replace(Some(Rc::clone(&debug_chunk)));
    initialize_debug_chunk(response, &parent_chunk);
    if __DEV__
        && matches!(*debug_chunk.state.borrow(), ChunkState::Blocked { .. })
        && response
            ._debug_channel
            .borrow()
            .as_ref()
            .is_none_or(|debug_channel| !debug_channel.has_readable)
        && js::string::get(&json, 0.0) == Some("\"")
        && js::string::get(&json, 1.0) == Some("$")
    {
        let path = js::string::split(
            &js::string::slice(&json, 2.0, Some(js::string::length(&json) as f64 - 1.0)),
            ":",
        );
        let outlined_id = js::parse_int(&path[0], Some(16));
        let chunk = get_chunk(response, outlined_id);
        if matches!(*chunk.state.borrow(), ChunkState::Pending { .. }) {
            // We expect the debug chunk to have been emitted earlier in the stream. It might be
            // blocked on other things but chunk should no longer be pending.
            // If it's still pending that suggests that it was referencing an object in the debug
            // channel, but no debug channel was wired up so it's missing. In this case we can just
            // drop the debug info instead of halting the whole stream.
            parent_chunk._debug_chunk.replace(None);
        }
    }
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::nullable)]
    #[cfg_attr(rust_js, rust_js::name = "currentOwnerInDEV")]
    static CURRENT_OWNER_IN_DEV: Cell<Option<&'static ReactComponentInfo>> = const { Cell::new(None) };
}

#[cfg_attr(rust_js, rust_js::name = "getCurrentStackInDEV")]
fn get_current_stack_in_dev() -> String {
    if __DEV__ {
        let owner = CURRENT_OWNER_IN_DEV.get();
        let Some(owner) = owner else {
            return String::new();
        };
        return get_owner_stack_by_component_info_in_dev(owner);
    }
    String::new()
}

/// A console entry, `[methodName, stackTrace, owner, env, ...args]`.
type ConsoleEntry = Unknown;

type ReplayConsoleWithCallStack = fn(&'static Response, &'static ConsoleEntry);

struct ReplayConsoleWithCallStackFrame {
    #[cfg_attr(rust_js, rust_js::name = "react_stack_bottom_frame")]
    react_stack_bottom_frame: ReplayConsoleWithCallStack,
}

/// Upstream's `finally { currentOwnerInDEV = null;
/// ReactSharedInternals.getCurrentStack = prevStack; }`.
struct RestoreCurrentStack(Option<&'static Unknown>);

impl Drop for RestoreCurrentStack {
    fn drop(&mut self) {
        CURRENT_OWNER_IN_DEV.set(None);
        REACT_SHARED_INTERNALS.with(|&internals| js::set(internals, "getCurrentStack", self.0));
    }
}

#[cfg_attr(rust_js, rust_js::name = "replayConsoleWithCallStack")]
const REPLAY_CONSOLE_WITH_CALL_STACK: ReplayConsoleWithCallStackFrame =
    ReplayConsoleWithCallStackFrame {
        react_stack_bottom_frame: |response, payload| {
            // SAFETY: an entry's first item is the console method's name.
            let method_name: String = unsafe { js::cast(js::get(payload, &0.0)) };
            // SAFETY: an entry's second item is its stack trace.
            let stack_trace: &ReactStackTrace = unsafe { js::cast(js::get(payload, &1.0)) };
            // SAFETY: an entry's third item is its owner's info, or null.
            let owner: Option<&'static ReactComponentInfo> =
                unsafe { js::cast(js::get(payload, &2.0)) };
            // SAFETY: an entry's fourth item is its environment's name.
            let env: String = unsafe { js::cast(js::get(payload, &3.0)) };
            // SAFETY: an entry is an array.
            let entry: &Vec<Option<&'static Unknown>> = unsafe { js::cast(Some(payload)) };
            let args: Vec<Option<&'static Unknown>> = entry[4..].to_vec();

            // There really shouldn't be anything else on the stack atm.
            let prev_stack =
                REACT_SHARED_INTERNALS.with(|&internals| js::get(internals, "getCurrentStack"));
            REACT_SHARED_INTERNALS.with(|&internals| {
                js::set(
                    internals,
                    "getCurrentStack",
                    get_current_stack_in_dev as fn() -> String,
                )
            });
            CURRENT_OWNER_IN_DEV.set(match owner {
                None => response._debug_root_owner.get(),
                Some(owner) => Some(owner),
            });

            let _restore = RestoreCurrentStack(prev_stack);
            let call_stack = build_fake_call_stack(
                response,
                stack_trace,
                &env,
                false,
                bind_to_console(&method_name, args, &env),
            );
            if let Some(owner) = owner {
                let task = initialize_fake_task(response, js::unknown(owner));
                initialize_fake_stack(response, js::unknown(owner));
                if let Some(task) = task {
                    task.run(call_stack);
                    return;
                }
            }
            let root_task = get_root_task(response, &env);
            if let Some(root_task) = root_task {
                root_task.run(call_stack);
                return;
            }
            call_stack();
        },
    };

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "replayConsoleWithCallStackInDEV")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    static REPLAY_CONSOLE_WITH_CALL_STACK_IN_DEV: Option<ReplayConsoleWithCallStack> = if __DEV__ {
        // We use this technique to trick minifiers to preserve the function name.
        Some(function::bind(
            REPLAY_CONSOLE_WITH_CALL_STACK.react_stack_bottom_frame,
            Some(&REPLAY_CONSOLE_WITH_CALL_STACK),
            (),
        ))
    } else {
        None
    };
}

fn resolve_console_entry(response: &'static Response, json: UninitializedModel) {
    if !__DEV__ {
        // These errors should never make it into a build so we don't need to encode them in codes.json
        // eslint-disable-next-line react-internal/prod-error-codes
        panic!(
            "resolveConsoleEntry should never be called in production mode. This is a bug in React."
        );
    }

    if response._replay_console.get() != Some(true) {
        return;
    }

    let blocked_chunk = response._blocked_console.borrow().clone();
    match blocked_chunk {
        None => {
            // If we're not blocked on any other chunks, we can try to eagerly initialize
            // this as a fast-path to avoid awaiting them.
            let chunk = create_resolved_model_chunk(response, json);
            initialize_model_chunk(&chunk);
            let initialized_chunk = &chunk;
            if matches!(
                *initialized_chunk.state.borrow(),
                ChunkState::Initialized { .. }
            ) {
                REPLAY_CONSOLE_WITH_CALL_STACK_IN_DEV.with(
                    // SAFETY: resolveConsoleEntry runs in DEV, where it's set.
                    |&f| unsafe { f.unwrap_unchecked() },
                )(response, initialized_value(initialized_chunk).unwrap());
            } else {
                chunk.then(
                    Some(Rc::new(move |v| {
                        REPLAY_CONSOLE_WITH_CALL_STACK_IN_DEV.with(
                            // SAFETY: resolveConsoleEntry runs in DEV, where it's set.
                            |&f| unsafe { f.unwrap_unchecked() },
                        )(response, v.unwrap())
                    })),
                    Some(Rc::new(|_e| {
                        // Ignore console errors for now. Unnecessary noise.
                    })),
                );
                *response._blocked_console.borrow_mut() = Some(chunk);
            }
        }
        Some(blocked_chunk) => {
            // We're still waiting on a previous chunk so we can't enqueue quite yet.
            let chunk = create_pending_chunk(response);
            chunk.then(
                Some(Rc::new(move |v| {
                    REPLAY_CONSOLE_WITH_CALL_STACK_IN_DEV.with(
                        // SAFETY: resolveConsoleEntry runs in DEV, where it's set.
                        |&f| unsafe { f.unwrap_unchecked() },
                    )(response, v.unwrap())
                })),
                Some(Rc::new(|_e| {
                    // Ignore console errors for now. Unnecessary noise.
                })),
            );
            *response._blocked_console.borrow_mut() = Some(Rc::clone(&chunk));
            let unblock: Rc<dyn Fn(Option<&'static Unknown>)> = Rc::new(move |_| {
                if response
                    ._blocked_console
                    .borrow()
                    .as_ref()
                    .is_some_and(|blocked| Rc::ptr_eq(blocked, &chunk))
                {
                    // We were still the last chunk so we can now clear the queue and return
                    // to synchronous emitting.
                    *response._blocked_console.borrow_mut() = None;
                }
                resolve_model_chunk(response, &chunk, json.clone());
            });
            blocked_chunk.then(Some(Rc::clone(&unblock)), Some(unblock));
        }
    }
}

#[cfg_attr(rust_js, rust_js::name = "initializeIOInfo")]
fn initialize_io_info(response: &'static Response, io_info: &'static ReactIOInfo) {
    if !matches!(js::classify(js::get(io_info, "stack")), Kind::Undefined) {
        initialize_fake_task(response, js::unknown(io_info));
        initialize_fake_stack(response, js::unknown(io_info));
    }
    // Adjust the time to the current environment's time space.
    let time_origin = response._time_origin.get().unwrap();
    io_info.start.set(io_info.start.get() + time_origin);
    io_info.end.set(io_info.end.get() + time_origin);

    if ENABLE_COMPONENT_PERFORMANCE_TRACK && response._replay_console.get() == Some(true) {
        let env = response._root_environment_name.borrow().clone().unwrap();
        let promise = io_info.value.get();
        if let Some(promise) = promise {
            let thenable: &Thenable = js::upcast(promise);
            match thenable.status().as_deref() {
                Some(INITIALIZED) => log_io_info(io_info, &env, thenable.value()),
                Some(ERRORED) => log_io_info_errored(io_info, &env, thenable.reason()),
                _ => {
                    // If we haven't resolved the Promise yet, wait to log until have so we can include
                    // its data in the log.
                    let errored_env = env.clone();
                    promise.then_with::<(), _, _>(
                        move |value| log_io_info(io_info, &env, value),
                        move |error| log_io_info_errored(io_info, &errored_env, error.as_unknown()),
                    );
                }
            }
        } else {
            log_io_info(io_info, &env, None);
        }
    }
}

#[cfg_attr(rust_js, rust_js::name = "resolveIOInfo")]
fn resolve_io_info(response: &'static Response, id: f64, model: UninitializedModel) {
    let chunks = response._chunks.get().unwrap();
    let chunk = {
        let _restore = RestoreIsInitializingDebugInfo {
            prev_is_initializing_debug_info: IS_INITIALIZING_DEBUG_INFO.get(),
        };
        IS_INITIALIZING_DEBUG_INFO.set(true);
        match chunks.get(&id) {
            None => {
                let chunk = create_resolved_model_chunk(response, model);
                chunks.set(&id, Rc::clone(&chunk));
                initialize_model_chunk(&chunk);
                chunk
            }
            Some(chunk) => {
                resolve_model_chunk(response, &chunk, model);
                if matches!(*chunk.state.borrow(), ChunkState::ResolvedModel { .. }) {
                    initialize_model_chunk(&chunk);
                }
                chunk
            }
        }
    };
    if matches!(*chunk.state.borrow(), ChunkState::Initialized { .. }) {
        // SAFETY: an I/O row's model is its I/O info.
        initialize_io_info(response, unsafe { js::cast(initialized_value(&chunk)) });
    } else {
        chunk.then(
            Some(Rc::new(move |v| {
                // SAFETY: as above.
                initialize_io_info(response, unsafe { js::cast(v) });
            })),
            Some(Rc::new(|_e| {
                // Ignore debug info errors for now. Unnecessary noise.
            })),
        );
    }
}

fn merge_buffer(
    buffer: &[&'static js::Uint8Array],
    last_chunk: &js::Uint8Array,
) -> &'static js::Uint8Array {
    let l = buffer.len();
    // Count the bytes we'll need
    let mut byte_length = last_chunk.length();
    for chunk in buffer.iter().take(l) {
        byte_length += chunk.byte_length();
    }
    // Allocate enough contiguous space
    let result = js::uint8_array::new(byte_length);
    let mut offset = 0.0;
    // Copy all the buffers into it.
    for chunk in buffer.iter().take(l) {
        result.set(*chunk, Some(offset));
        offset += chunk.byte_length();
    }
    result.set(last_chunk, Some(offset));
    result
}

/// A typed array's or a DataView's constructor, `new constructor(buffer, byteOffset, length)`.
type ViewConstructor = js::Class<fn(&'static js::ArrayBuffer, f64, f64) -> js::ArrayBufferView>;

fn resolve_typed_array(
    response: &'static Response,
    id: f64,
    buffer: &[&'static js::Uint8Array],
    last_chunk: &'static js::Uint8Array,
    constructor: &ViewConstructor,
    bytes_per_element: f64,
    stream_state: &StreamState,
) {
    // If the view fits into one original buffer, we just reuse that buffer instead of
    // copying it out to a separate copy. This means that it's not always possible to
    // transfer these values to other threads without copying first since they may
    // share array buffer. For this to work, it must also have bytes aligned to a
    // multiple of a size of the type.
    let chunk = if buffer.is_empty() && last_chunk.byte_offset() % bytes_per_element == 0.0 {
        last_chunk
    } else {
        merge_buffer(buffer, last_chunk)
    };
    // TODO: The transfer protocol of RSC is little-endian. If the client isn't little-endian
    // we should convert it instead. In practice big endian isn't really Web compatible so it's
    // somewhat safe to assume that browsers aren't going to run it, but maybe there's some SSR
    // server that's affected.
    let view = js::new(
        constructor,
        (
            chunk.buffer(),
            chunk.byte_offset(),
            chunk.byte_length() / bytes_per_element,
        ),
    );
    resolve_buffer(response, id, js::unknown(view), stream_state);
}

#[allow(clippy::too_many_arguments)] // upstream's eight positional parameters.
fn log_component_info(
    response: &'static Response,
    root: &Chunk,
    component_info: &'static ReactComponentInfo,
    track_idx: usize,
    start_time: f64,
    component_end_time: f64,
    children_end_time: f64,
    is_last_component: bool,
) {
    if is_last_component
        && matches!(*root.state.borrow(), ChunkState::Errored { .. })
        && !js::strict_equals(errored_reason(root), response._closed_reason.get())
    {
        // If this is the last component to render before this chunk rejected, then conceptually
        // this component errored. If this was a cancellation then it wasn't this component that
        // errored.
        let root_env = response._root_environment_name.borrow().clone().unwrap();
        log_component_errored(
            component_info,
            track_idx,
            start_time,
            component_end_time,
            children_end_time,
            &root_env,
            errored_reason(root),
        );
    } else {
        let root_env = response._root_environment_name.borrow().clone().unwrap();
        log_component_render(
            component_info,
            track_idx,
            start_time,
            component_end_time,
            children_end_time,
            &root_env,
        );
    }
}

/// A debug info entry's `time`, if it's a number.
fn time_of(info: Option<&'static Unknown>) -> Option<f64> {
    match js::classify(js::get(info, "time")) {
        Kind::Number(time) => Some(time),
        _ => None,
    }
}

fn flush_component_performance(
    response: &'static Response,
    root: &Chunk,
    track_idx: usize, // Next available track
    track_time: f64,  // The time after which it is available,
    parent_end_time: f64,
) -> Rc<ProfilingResult> {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled) || !ENABLE_COMPONENT_PERFORMANCE_TRACK {
        // eslint-disable-next-line react-internal/prod-error-codes
        panic!(
            "flushComponentPerformance should never be called in production mode. This is a bug in React."
        );
    }
    let (mut track_idx, mut track_time, mut parent_end_time) =
        (track_idx, track_time, parent_end_time);
    // Write performance.measure() entries for Server Components in tree order.
    // This must be done at the end to collect the end time from the whole tree.
    let children = match &*root._children.borrow() {
        Some(Children::Flushed(previous_result)) => {
            // We have already written this chunk. If this was a cycle, then this will
            // be -Infinity and it won't contribute to the parent end time.
            // If this was already emitted by another sibling then we reused the same
            // chunk in two places. We should extend the current end time as if it was
            // rendered as part of this tree.
            let previous_end_time = previous_result.end_time.get();
            if parent_end_time > f64::NEG_INFINITY
                && parent_end_time < previous_end_time
                && let Some(component) = previous_result.component.get()
            {
                // Log a placeholder for the deduped value under this child starting
                // from the end of the self time of the parent and spanning until the
                // the deduped end.
                let root_env = response._root_environment_name.borrow().clone().unwrap();
                log_deduped_component_render(
                    component,
                    track_idx,
                    parent_end_time,
                    previous_end_time,
                    &root_env,
                );
            }
            // Since we didn't bump the track this time, we just return the same track.
            previous_result.track.set(track_idx);
            return Rc::clone(previous_result);
        }
        Some(Children::Chunks(children)) => children.borrow().clone(),
        None => Vec::new(),
    };

    // First find the start time of the first component to know if it was running
    // in parallel with the previous.
    let mut debug_info: Option<Rc<RefCell<ReactDebugInfo>>> = None;
    if __DEV__ {
        debug_info = root._debug_info.borrow().clone();
        if debug_info.as_ref().unwrap().borrow().is_empty()
            && matches!(*root.state.borrow(), ChunkState::Initialized { .. })
        {
            let resolved_value = resolve_lazy(initialized_value(root));
            if js::type_of(resolved_value) == "object"
                && resolved_value.is_some()
                && (is_array(resolved_value)
                    || js::type_of(
                        ASYNC_ITERATOR
                            .with(|&async_iterator| js::get(resolved_value, async_iterator)),
                    ) == "function"
                    || SymbolFor::from_value(js::get(resolved_value, "$$typeof"))
                        == Some(REACT_ELEMENT_TYPE)
                    || SymbolFor::from_value(js::get(resolved_value, "$$typeof"))
                        == Some(REACT_LAZY_TYPE))
                && is_array(js::get(resolved_value, "_debugInfo"))
            {
                // It's possible that the value has been given the debug info.
                // In that case we need to look for it on the resolved value.
                // SAFETY: an array `_debugInfo` is the value's debug info.
                debug_info = Some(unsafe { js::cast(js::get(resolved_value, "_debugInfo")) });
            }
        }
    }
    if let Some(debug_info) = &debug_info {
        let mut start_time = 0.0;
        for &info in debug_info.borrow().iter() {
            if let Some(time) = time_of(info) {
                start_time = time;
            }
            if let Kind::String(_) = js::classify(js::get(info, "name")) {
                if start_time < track_time {
                    // The start time of this component is before the end time of the previous
                    // component on this track so we need to bump the next one to a parallel track.
                    track_idx += 1;
                }
                track_time = start_time;
                break;
            }
        }
        for &info in debug_info.borrow().iter().rev() {
            if let Some(time) = time_of(info)
                && time > parent_end_time
            {
                parent_end_time = time;
                break; // We assume the highest number is at the end.
            }
        }
    }

    let result = Rc::new(ProfilingResult {
        track: Cell::new(track_idx),
        end_time: Cell::new(f64::NEG_INFINITY),
        component: Cell::new(None),
    });
    *root._children.borrow_mut() = Some(Children::Flushed(Rc::clone(&result)));
    let mut children_end_time = f64::NEG_INFINITY;
    let mut child_track_idx = track_idx;
    let mut child_track_time = track_time;
    for child in &children {
        let child_result = flush_component_performance(
            response,
            child,
            child_track_idx,
            child_track_time,
            parent_end_time,
        );
        if let Some(component) = child_result.component.get() {
            result.component.set(Some(component));
        }
        child_track_idx = child_result.track.get();
        let child_end_time = child_result.end_time.get();
        if child_end_time > child_track_time {
            child_track_time = child_end_time;
        }
        if child_end_time > children_end_time {
            children_end_time = child_end_time;
        }
    }

    if let Some(debug_info) = &debug_info {
        let debug_info = debug_info.borrow();
        // Write debug info in reverse order (just like stack traces).
        let mut component_end_time = 0.0;
        let mut is_last_component = true;
        let mut end_time = -1.0;
        let mut end_time_idx: isize = -1;
        for i in (0..debug_info.len()).rev() {
            let info = debug_info[i];
            let Some(time) = time_of(info) else {
                continue;
            };
            if component_end_time == 0.0 {
                // Last timestamp is the end of the last component.
                component_end_time = time;
            }
            if end_time_idx > -1 {
                // Now that we know the start and end time, we can emit the entries between.
                for j in (i + 1..end_time_idx as usize).rev() {
                    let candidate_info = debug_info[j];
                    let awaited = js::get(candidate_info, "awaited");
                    if let Kind::String(_) = js::classify(js::get(candidate_info, "name")) {
                        if component_end_time > children_end_time {
                            children_end_time = component_end_time;
                        }
                        // SAFETY: an entry with a string `name` is a component's info.
                        let component_info: &'static ReactComponentInfo =
                            unsafe { js::cast(candidate_info) };
                        log_component_info(
                            response,
                            root,
                            component_info,
                            track_idx,
                            time,
                            component_end_time,
                            children_end_time,
                            is_last_component,
                        );
                        component_end_time = time; // The end time of previous component is the start time of the next.
                        // Track the root most component of the result for deduping logging.
                        result.component.set(Some(component_info));
                        is_last_component = false;
                    } else if js::truthy(awaited)
                        // Skip awaits on client resources since they didn't block the server component.
                        && !js::loose_equals(js::get(awaited, "env"), js::null::<&Unknown>())
                    {
                        if end_time > children_end_time {
                            children_end_time = end_time;
                        }
                        // SAFETY: an entry with an `awaited` is an await's info.
                        let async_info: &'static ReactAsyncInfo =
                            unsafe { js::cast(candidate_info) };
                        let env = response._root_environment_name.borrow().clone().unwrap();
                        let promise = async_info.awaited.value.get();
                        if let Some(promise) = promise {
                            let thenable: &Thenable = js::upcast(promise);
                            match thenable.status().as_deref() {
                                Some(INITIALIZED) => log_component_await(
                                    async_info,
                                    track_idx,
                                    time,
                                    end_time,
                                    &env,
                                    thenable.value(),
                                ),
                                Some(ERRORED) => log_component_await_errored(
                                    async_info,
                                    track_idx,
                                    time,
                                    end_time,
                                    &env,
                                    thenable.reason(),
                                ),
                                // We assume that we should have received the data by now since this is logged at the
                                // end of the response stream. This is more sensitive to ordering so we don't wait
                                // to log it.
                                _ => log_component_await(
                                    async_info, track_idx, time, end_time, &env, None,
                                ),
                            }
                        } else {
                            log_component_await(async_info, track_idx, time, end_time, &env, None);
                        }
                    }
                }
            } else {
                // Anything between the end and now was aborted if it has no end time.
                // Either because the client stream was aborted reading it or the server stream aborted.
                end_time = time; // If we don't find anything else the endTime is the start time.
                for j in (i + 1..debug_info.len()).rev() {
                    let candidate_info = debug_info[j];
                    let awaited = js::get(candidate_info, "awaited");
                    if let Kind::String(_) = js::classify(js::get(candidate_info, "name")) {
                        if component_end_time > children_end_time {
                            children_end_time = component_end_time;
                        }
                        // SAFETY: an entry with a string `name` is a component's info.
                        let component_info: &'static ReactComponentInfo =
                            unsafe { js::cast(candidate_info) };
                        let env = response._root_environment_name.borrow().clone().unwrap();
                        log_component_aborted(
                            component_info,
                            track_idx,
                            time,
                            component_end_time,
                            children_end_time,
                            &env,
                        );
                        component_end_time = time; // The end time of previous component is the start time of the next.
                        // Track the root most component of the result for deduping logging.
                        result.component.set(Some(component_info));
                        is_last_component = false;
                    } else if js::truthy(awaited)
                        // Skip awaits on client resources since they didn't block the server component.
                        && !js::loose_equals(js::get(awaited, "env"), js::null::<&Unknown>())
                    {
                        // If we don't have an end time for an await, that means we aborted.
                        // SAFETY: an entry with an `awaited` is an await's info.
                        let async_info: &'static ReactAsyncInfo =
                            unsafe { js::cast(candidate_info) };
                        let env = response._root_environment_name.borrow().clone().unwrap();
                        if async_info.awaited.end.get() > end_time {
                            end_time = async_info.awaited.end.get(); // Take the end time of the I/O as the await end.
                        }
                        if end_time > children_end_time {
                            children_end_time = end_time;
                        }
                        log_component_await_aborted(async_info, track_idx, time, end_time, &env);
                    }
                }
            }
            end_time = time; // The end time of the next entry is this time.
            end_time_idx = i as isize;
        }
    }
    result.end_time.set(children_end_time);
    result
}

fn flush_initial_render_performance(response: &'static Response) {
    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled)
        && ENABLE_COMPONENT_PERFORMANCE_TRACK
        && response._replay_console.get() == Some(true)
    {
        let root_chunk = get_chunk(response, 0.0);
        if matches!(&*root_chunk._children.borrow(), Some(Children::Chunks(_))) {
            mark_all_tracks_in_order();
            flush_component_performance(
                response,
                &root_chunk,
                0,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            );
        }
    }
}

fn process_full_binary_row(
    response: &'static Response,
    stream_state: &StreamState,
    id: f64,
    tag: i32,
    buffer: &[&'static js::Uint8Array],
    chunk: &'static js::Uint8Array,
) {
    match tag {
        65 /* "A" */ => {
            // We must always clone to extract it into a separate buffer instead of just a view.
            resolve_buffer(response, id, js::unknown(merge_buffer(buffer, chunk).buffer()), stream_state);
            return;
        }
        79 /* "O" */ => {
            resolve_typed_array(response, id, buffer, chunk, js::class::<js::Int8Array>(), 1.0, stream_state);
            return;
        }
        111 /* "o" */ => {
            resolve_buffer(
                response,
                id,
                js::unknown(if buffer.is_empty() { chunk } else { merge_buffer(buffer, chunk) }),
                stream_state,
            );
            return;
        }
        85 /* "U" */ => {
            resolve_typed_array(response, id, buffer, chunk, js::class::<js::Uint8ClampedArray>(), 1.0, stream_state);
            return;
        }
        83 /* "S" */ => {
            resolve_typed_array(response, id, buffer, chunk, js::class::<js::Int16Array>(), 2.0, stream_state);
            return;
        }
        115 /* "s" */ => {
            resolve_typed_array(response, id, buffer, chunk, js::class::<js::Uint16Array>(), 2.0, stream_state);
            return;
        }
        76 /* "L" */ => {
            resolve_typed_array(response, id, buffer, chunk, js::class::<js::Int32Array>(), 4.0, stream_state);
            return;
        }
        108 /* "l" */ => {
            resolve_typed_array(response, id, buffer, chunk, js::class::<js::Uint32Array>(), 4.0, stream_state);
            return;
        }
        71 /* "G" */ => {
            resolve_typed_array(response, id, buffer, chunk, js::class::<js::Float32Array>(), 4.0, stream_state);
            return;
        }
        103 /* "g" */ => {
            resolve_typed_array(response, id, buffer, chunk, js::class::<js::Float64Array>(), 8.0, stream_state);
            return;
        }
        77 /* "M" */ => {
            resolve_typed_array(response, id, buffer, chunk, js::class::<js::BigInt64Array>(), 8.0, stream_state);
            return;
        }
        109 /* "m" */ => {
            resolve_typed_array(response, id, buffer, chunk, js::class::<js::BigUint64Array>(), 8.0, stream_state);
            return;
        }
        86 /* "V" */ => {
            resolve_typed_array(response, id, buffer, chunk, js::class::<js::DataView>(), 1.0, stream_state);
            return;
        }
        _ => {}
    }

    let string_decoder = response._string_decoder.get().unwrap();
    let mut row = String::new();
    for &part in buffer {
        row += &read_partial_string_chunk(string_decoder, part);
    }
    row += &read_final_string_chunk(string_decoder, chunk);
    process_full_string_row(response, stream_state, id, tag, row);
}

fn process_full_string_row(
    response: &'static Response,
    stream_state: &StreamState,
    id: f64,
    tag: i32,
    row: String,
) {
    match tag {
        73 /* "I" */ => {
            resolve_module(response, id, row, stream_state);
        }
        72 /* "H" */ => {
            let code = js::string::get(&row, 0.0).unwrap_or_default();
            resolve_hint(response, code, js::string::slice(&row, 1.0, None));
        }
        69 /* "E" */ => {
            resolve_error_model(response, id, row, stream_state);
        }
        84 /* "T" */ => {
            resolve_text(response, id, row, stream_state);
        }
        78 /* "N" */ | 68 /* "D" */ | 74 /* "J" */ | 87 /* "W" */ => {
            if tag == 78 && ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
                // Track the time origin for future debug info. We track it relative
                // to the current environment's time space.
                let time_origin = js::to_number(&*row);
                response._time_origin.set(Some(time_origin - performance.time_origin()));
                return;
            }
            // Fallthrough to share the error with Debug and Console entries.
            if (tag == 78 || tag == 68) && __DEV__ {
                resolve_debug_model(response, id, row);
                return;
            }
            // Fallthrough to share the error with Console entries.
            if (tag == 78 || tag == 68 || tag == 74) && ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
                resolve_io_info(response, id, row);
                return;
            }
            // Fallthrough to share the error with Console entries.
            if __DEV__ {
                resolve_console_entry(response, row);
                return;
            }
            panic!(
                "{}",
                concat!(
                    "Failed to read a RSC payload created by a development version of React ",
                    "on the server while using a production version on the client. Always use ",
                    "matching versions on the server and the client.",
                )
            );
        }
        82 /* "R" */ => {
            start_readable_stream(response, id, None, stream_state);
        }
        // Fallthrough
        114 /* "r" */ => {
            start_readable_stream(response, id, Some("bytes"), stream_state);
        }
        // Fallthrough
        88 /* "X" */ => {
            start_async_iterable(response, id, false, stream_state);
        }
        // Fallthrough
        120 /* "x" */ => {
            start_async_iterable(response, id, true, stream_state);
        }
        // Fallthrough
        67 /* "C" */ => {
            stop_stream(response, id, row);
        }
        // Fallthrough
        _ /* """ "{" "[" "t" "f" "n" "0" - "9" */ => {
            if __DEV__ && row.is_empty() {
                resolve_debug_halt(response, id);
                return;
            }
            // We assume anything else is JSON.
            resolve_model(response, id, row, stream_state);
        }
    }
}

pub fn process_binary_chunk(
    weak_response: &WeakResponse,
    stream_state: &StreamState,
    chunk: &'static js::Uint8Array,
) {
    if has_gced_response(weak_response) {
        // Ignore more chunks if we've already GC:ed all listeners.
        return;
    }
    let response = unwrap_weak_response(weak_response);
    let mut i = 0.0;
    let mut row_state = stream_state._row_state.get();
    let mut row_id = stream_state._row_id.get();
    let mut row_tag = stream_state._row_tag.get();
    let mut row_length = stream_state._row_length.get();
    let buffer = &stream_state._buffer;
    let chunk_length = chunk.length();
    increment_chunk_debug_info(stream_state, chunk_length);
    while i < chunk_length {
        let mut last_idx = -1.0;
        match row_state {
            ROW_ID => {
                let byte = chunk.get(i).unwrap() as i32;
                i += 1.0;
                if byte == 58
                /* ":" */
                {
                    // Finished the rowID, next we'll parse the tag.
                    row_state = ROW_TAG;
                } else {
                    row_id = (row_id << 4) | (if byte > 96 { byte - 87 } else { byte - 48 });
                }
                continue;
            }
            ROW_TAG => {
                let resolved_row_tag = chunk.get(i).unwrap() as i32;
                if resolved_row_tag == 84 /* "T" */
                    || resolved_row_tag == 65 /* "A" */
                    || resolved_row_tag == 79 /* "O" */
                    || resolved_row_tag == 111 /* "o" */
                    || resolved_row_tag == 98 /* "b" */
                    || resolved_row_tag == 85 /* "U" */
                    || resolved_row_tag == 83 /* "S" */
                    || resolved_row_tag == 115 /* "s" */
                    || resolved_row_tag == 76 /* "L" */
                    || resolved_row_tag == 108 /* "l" */
                    || resolved_row_tag == 71 /* "G" */
                    || resolved_row_tag == 103 /* "g" */
                    || resolved_row_tag == 77 /* "M" */
                    || resolved_row_tag == 109 /* "m" */
                    || resolved_row_tag == 86
                /* "V" */
                {
                    row_tag = resolved_row_tag;
                    row_state = ROW_LENGTH;
                    i += 1.0;
                } else if (resolved_row_tag > 64 && resolved_row_tag < 91) /* "A"-"Z" */
                    || resolved_row_tag == 35 /* "#" */
                    || resolved_row_tag == 114 /* "r" */
                    || resolved_row_tag == 120
                /* "x" */
                {
                    row_tag = resolved_row_tag;
                    row_state = ROW_CHUNK_BY_NEWLINE;
                    i += 1.0;
                } else {
                    row_tag = 0;
                    row_state = ROW_CHUNK_BY_NEWLINE;
                    // This was an unknown tag so it was probably part of the data.
                }
                continue;
            }
            ROW_LENGTH => {
                let byte = chunk.get(i).unwrap() as i32;
                i += 1.0;
                if byte == 44
                /* "," */
                {
                    // Finished the rowLength, next we'll buffer up to that length.
                    row_state = ROW_CHUNK_BY_LENGTH;
                } else {
                    row_length =
                        (row_length << 4) | (if byte > 96 { byte - 87 } else { byte - 48 });
                }
                continue;
            }
            ROW_CHUNK_BY_NEWLINE => {
                // We're looking for a newline
                last_idx = chunk.index_of(10 /* "\n" */, Some(i));
            }
            ROW_CHUNK_BY_LENGTH => {
                // We're looking for the remaining byte length
                last_idx = i + row_length as f64;
                if last_idx > chunk.length() {
                    last_idx = -1.0;
                }
            }
            _ => {}
        }
        let offset = chunk.byte_offset() + i;
        if last_idx > -1.0 {
            // We found the last chunk of the row
            let length = last_idx - i;
            let last_chunk =
                js::uint8_array::new_with_buffer(chunk.buffer(), Some(offset), Some(length));

            // Check if this is a Uint8Array for a byte stream. We enqueue it
            // immediately but need to determine if we can use zero-copy or must copy.
            if row_tag == 98
            /* "b" */
            {
                resolve_buffer(
                    response,
                    row_id as f64,
                    // If we're at the end of the RSC chunk, no more parsing will access
                    // this buffer and we don't need to copy the chunk to allow detaching
                    // the buffer, otherwise we need to copy.
                    js::unknown(if last_idx == chunk_length {
                        last_chunk
                    } else {
                        last_chunk.slice(None, None)
                    }),
                    stream_state,
                );
            } else {
                // Process all other row types.
                let parts = buffer.borrow().clone();
                process_full_binary_row(
                    response,
                    stream_state,
                    row_id as f64,
                    row_tag,
                    &parts,
                    last_chunk,
                );
            }

            // Reset state machine for a new row
            i = last_idx;
            if row_state == ROW_CHUNK_BY_NEWLINE {
                // If we're trailing by a newline we need to skip it.
                i += 1.0;
            }
            row_state = ROW_ID;
            row_tag = 0;
            row_id = 0;
            row_length = 0;
            buffer.borrow_mut().clear();
        } else {
            // The rest of this row is in a future chunk.
            let length = chunk.byte_length() - i;
            let remaining_slice =
                js::uint8_array::new_with_buffer(chunk.buffer(), Some(offset), Some(length));

            // For byte streams, we can enqueue the partial row immediately without
            // copying since we're at the end of the RSC chunk and no more parsing
            // will access this buffer.
            if row_tag == 98
            /* "b" */
            {
                // Update how many bytes we're still waiting for. We need to do this
                // before enqueueing, as enqueue will detach the buffer and byteLength
                // will become 0.
                row_length -= remaining_slice.byte_length() as i32;
                resolve_buffer(
                    response,
                    row_id as f64,
                    js::unknown(remaining_slice),
                    stream_state,
                );
            } else {
                // For other row types, stash the rest of the current chunk until we can
                // process the full row.
                buffer.borrow_mut().push(remaining_slice);
                // Update how many bytes we're still waiting for. If we're looking for
                // a newline, this doesn't hurt since we'll just ignore it.
                row_length -= remaining_slice.byte_length() as i32;
            }
            break;
        }
    }
    stream_state._row_state.set(row_state);
    stream_state._row_id.set(row_id);
    stream_state._row_tag.set(row_tag);
    stream_state._row_length.set(row_length);
}

pub fn process_string_chunk(weak_response: &WeakResponse, stream_state: &StreamState, chunk: &str) {
    if has_gced_response(weak_response) {
        // Ignore more chunks if we've already GC:ed all listeners.
        return;
    }
    let response = unwrap_weak_response(weak_response);
    // This is a fork of processBinaryChunk that takes a string as input.
    // This can't be just any binary chunk coverted to a string. It needs to be
    // in the same offsets given from the Flight Server. E.g. if it's shifted by
    // one byte then it won't line up to the UCS-2 encoding. It also needs to
    // be valid Unicode. Also binary chunks cannot use this even if they're
    // value Unicode. Large strings are encoded as binary and cannot be passed
    // here. Basically, only if Flight Server gave you this string as a chunk,
    // you can use it here.
    let mut i: f64 = 0.0;
    let mut row_state = stream_state._row_state.get();
    let mut row_id = stream_state._row_id.get();
    let mut row_tag = stream_state._row_tag.get();
    let mut row_length = stream_state._row_length.get();
    let buffer = &stream_state._buffer;
    let chunk_length = js::string::length(chunk) as f64;
    increment_chunk_debug_info(stream_state, chunk_length);
    while i < chunk_length {
        let mut last_idx = -1.0;
        match row_state {
            ROW_ID => {
                let byte = js::string::char_code_at(chunk, i) as i32;
                i += 1.0;
                if byte == 58
                /* ":" */
                {
                    // Finished the rowID, next we'll parse the tag.
                    row_state = ROW_TAG;
                } else {
                    row_id = (row_id << 4) | (if byte > 96 { byte - 87 } else { byte - 48 });
                }
                continue;
            }
            ROW_TAG => {
                let resolved_row_tag = js::string::char_code_at(chunk, i) as i32;
                if resolved_row_tag == 84 /* "T" */
                    || resolved_row_tag == 65 /* "A" */
                    || resolved_row_tag == 79 /* "O" */
                    || resolved_row_tag == 111 /* "o" */
                    || resolved_row_tag == 85 /* "U" */
                    || resolved_row_tag == 83 /* "S" */
                    || resolved_row_tag == 115 /* "s" */
                    || resolved_row_tag == 76 /* "L" */
                    || resolved_row_tag == 108 /* "l" */
                    || resolved_row_tag == 71 /* "G" */
                    || resolved_row_tag == 103 /* "g" */
                    || resolved_row_tag == 77 /* "M" */
                    || resolved_row_tag == 109 /* "m" */
                    || resolved_row_tag == 86
                /* "V" */
                {
                    row_tag = resolved_row_tag;
                    row_state = ROW_LENGTH;
                    i += 1.0;
                } else if (resolved_row_tag > 64 && resolved_row_tag < 91) /* "A"-"Z" */
                    || resolved_row_tag == 114 /* "r" */
                    || resolved_row_tag == 120
                /* "x" */
                {
                    row_tag = resolved_row_tag;
                    row_state = ROW_CHUNK_BY_NEWLINE;
                    i += 1.0;
                } else {
                    row_tag = 0;
                    row_state = ROW_CHUNK_BY_NEWLINE;
                    // This was an unknown tag so it was probably part of the data.
                }
                continue;
            }
            ROW_LENGTH => {
                let byte = js::string::char_code_at(chunk, i) as i32;
                i += 1.0;
                if byte == 44
                /* "," */
                {
                    // Finished the rowLength, next we'll buffer up to that length.
                    row_state = ROW_CHUNK_BY_LENGTH;
                } else {
                    row_length =
                        (row_length << 4) | (if byte > 96 { byte - 87 } else { byte - 48 });
                }
                continue;
            }
            ROW_CHUNK_BY_NEWLINE => {
                // We're looking for a newline
                last_idx = js::string::index_of(chunk, "\n", Some(i));
            }
            ROW_CHUNK_BY_LENGTH => {
                if row_tag != 84 {
                    panic!(
                        "{}",
                        concat!(
                            "Binary RSC chunks cannot be encoded as strings. ",
                            "This is a bug in the wiring of the React streams.",
                        )
                    );
                }
                // For a large string by length, we don't know how many unicode characters
                // we are looking for but we can assume that the raw string will be its own
                // chunk. We add extra validation that the length is at least within the
                // possible byte range it could possibly be to catch mistakes.
                if (row_length as f64) < chunk_length || chunk_length > row_length as f64 * 3.0 {
                    panic!(
                        "{}",
                        concat!(
                            "String chunks need to be passed in their original shape. ",
                            "Not split into smaller string chunks. ",
                            "This is a bug in the wiring of the React streams.",
                        )
                    );
                }
                last_idx = chunk_length;
            }
            _ => {}
        }
        if last_idx > -1.0 {
            // We found the last chunk of the row
            if !buffer.borrow().is_empty() {
                // If we had a buffer already, it means that this chunk was split up into
                // binary chunks preceeding it.
                panic!(
                    "{}",
                    concat!(
                        "String chunks need to be passed in their original shape. ",
                        "Not split into smaller string chunks. ",
                        "This is a bug in the wiring of the React streams.",
                    )
                );
            }
            let last_chunk = js::string::slice(chunk, i, Some(last_idx));
            process_full_string_row(response, stream_state, row_id as f64, row_tag, last_chunk);
            // Reset state machine for a new row
            i = last_idx;
            if row_state == ROW_CHUNK_BY_NEWLINE {
                // If we're trailing by a newline we need to skip it.
                i += 1.0;
            }
            row_state = ROW_ID;
            row_tag = 0;
            row_id = 0;
            row_length = 0;
            buffer.borrow_mut().clear();
        } else if chunk_length != i {
            // The rest of this row is in a future chunk. We only support passing the
            // string from chunks in their entirety. Not split up into smaller string chunks.
            // We could support this by buffering them but we shouldn't need to for
            // this use case.
            panic!(
                "{}",
                concat!(
                    "String chunks need to be passed in their original shape. ",
                    "Not split into smaller string chunks. ",
                    "This is a bug in the wiring of the React streams.",
                )
            );
        }
    }
    stream_state._row_state.set(row_state);
    stream_state._row_id.set(row_id);
    stream_state._row_tag.set(row_tag);
    stream_state._row_length.set(row_length);
}

fn parse_model(response: &'static Response, json: &str) -> Option<&'static Unknown> {
    let raw_model = js::json::parse(json).unwrap();
    // Pass a wrapper object as parentObject to match the original JSON.parse
    // reviver behavior, where the root value's reviver receives {"": rootValue}
    // as `this`. This ensures parentObject is never null when accessed downstream.
    let parent_object = js::unknown(object::from_entries(vec![(
        String::new(),
        or_null(raw_model),
    )]));
    revive_model(response, raw_model, parent_object, "")
}

fn revive_model(
    response: &'static Response,
    value: Option<&'static JSONValue>,
    parent_object: &'static Unknown,
    key: &str,
) -> Option<&'static Unknown> {
    match js::classify(value) {
        Kind::String(text) => {
            if js::string::get(text, 0.0) == Some("$") {
                return parse_model_string(response, parent_object, key, text);
            }
            return value;
        }
        Kind::Object(_) | Kind::Array(_) => {}
        _ => return value,
    }
    let value = value.unwrap();
    if is_array(Some(value)) {
        // SAFETY: an array of JSON values.
        let items: &RefCell<Vec<Option<&'static Unknown>>> = unsafe { js::cast(Some(value)) };
        let length = items.borrow().len();
        for i in 0..length {
            let item = items.borrow()[i];
            let revived = revive_model(response, item, value, &js::concat_text(i as f64));
            items.borrow_mut()[i] = revived;
        }
        if SymbolFor::from_value(js::get(value, &0.0)) == Some(REACT_ELEMENT_TYPE) {
            // React element tuple
            return parse_model_tuple(response, value);
        }
        return Some(value);
    }
    // Plain object
    for k in js::keys_in(value) {
        if HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(value, &*k)) {
            if k == __PROTO__ {
                js::delete(value, &*k);
            } else {
                let walked = revive_model(response, js::get(value, &*k), value, &k);
                if !matches!(js::classify(walked), Kind::Undefined) {
                    js::set(value, &*k, walked);
                } else {
                    js::delete(value, &*k);
                }
            }
        }
    }
    Some(value)
}

pub fn close(weak_response: &WeakResponse) {
    // In case there are any remaining unresolved chunks, they won't be resolved
    // now. So we either error or halt them depending on whether partial streams
    // are allowed.
    // TODO: Ideally we should be able to bail out early if we kept a ref count of
    // pending chunks.
    if has_gced_response(weak_response) {
        return;
    }
    let response = unwrap_weak_response(weak_response);
    if response._allow_partial_stream.get() {
        // For partial streams, we halt pending chunks instead of erroring them.
        response._closed.set(true);
        response._chunks.get().unwrap().for_each(|chunk, _, _| {
            if matches!(*chunk.state.borrow(), ChunkState::Pending { .. })
                || (ENABLE_FLIGHT_WEAK_THENABLES.with(|&enabled| enabled)
                    && matches!(*chunk.state.borrow(), ChunkState::PendingWeak { .. }))
            {
                halt_chunk(response, &chunk);
            } else if matches!(
                *chunk.state.borrow(),
                ChunkState::Initialized {
                    reason: Some(_),
                    ..
                }
            ) {
                // Stream chunk - close gracefully instead of erroring.
                let controller = stream_controller(&chunk);
                (controller.close)("\"$undefined\"".to_owned());
            }
        });
        if __DEV__ {
            let debug_channel = response._debug_channel.borrow().clone();
            if let Some(debug_channel) = debug_channel {
                close_debug_channel(debug_channel);
                *response._debug_channel.borrow_mut() = None;
                if let Some(debug_channel_registry) =
                    DEBUG_CHANNEL_REGISTRY.with(|&registry| registry)
                {
                    debug_channel_registry.unregister(response);
                }
            }
        }
    } else {
        report_global_error(
            weak_response,
            Some(js::unknown(js::error::new(
                Some("Connection closed."),
                None,
            ))),
        );
    }
}

#[cfg_attr(rust_js, rust_js::name = "getCurrentOwnerInDEV")]
fn get_current_owner_in_dev() -> Option<&'static ReactComponentInfo> {
    CURRENT_OWNER_IN_DEV.get()
}

/// What `injectIntoDevTools` hands DevTools.
pub struct DevToolsInternals {
    pub bundle_type: f64,
    pub version: &'static str,
    pub renderer_package_name: &'static str,
    pub current_dispatcher_ref: Option<&'static Unknown>,
    pub reconciler_version: &'static str,
    pub get_current_component_info: fn() -> Option<&'static ReactComponentInfo>,
}

unsafe impl js::Defined for DevToolsInternals {}

pub fn inject_into_dev_tools() -> bool {
    let internals = DevToolsInternals {
        bundle_type: if __DEV__ { 1.0 } else { 0.0 }, // Might add PROFILE later.
        version: RENDERER_VERSION,
        renderer_package_name: RENDERER_PACKAGE_NAME,
        current_dispatcher_ref: REACT_SHARED_INTERNALS.with(|&internals| internals),
        // Enables DevTools to detect reconciler version rather than renderer version
        // which may not match for third party renderers.
        reconciler_version: REACT_VERSION,
        get_current_component_info: get_current_owner_in_dev,
    };
    inject_internals(js::unknown_of(internals))
}

/// This module, as JS hands it around: the Flight client a renderer's
/// `ReactFlightClient(config)` makes of its config.
#[cfg_attr(rust_js, rust_js::namespace)]
pub struct ReactFlightClient;

unsafe impl js::Defined for ReactFlightClient {}

#[cfg_attr(rust_js, rust_js::namespace)]
impl ReactFlightClient {
    #[allow(clippy::too_many_arguments)] // upstream's 14 positional parameters.
    #[cfg_attr(
        rust_js,
        rust_js::nullable(bundler_config, server_reference_config, module_loading)
    )]
    pub fn create_response(
        &self,
        bundler_config: Option<&'static Unknown>,
        server_reference_config: Option<&'static Unknown>,
        module_loading: Option<&'static Unknown>,
        call_server: Option<&'static CallServerCallback>,
        encode_form_action: Option<&'static EncodeFormActionCallback>,
        nonce: Option<String>,
        temporary_references: Option<&'static TemporaryReferenceSet>,
        allow_partial_stream: bool,
        find_source_map_url: Option<&'static FindSourceMapURLCallback>,
        replay_console: bool,
        environment_name: Option<String>,
        debug_start_time: Option<f64>,
        debug_end_time: Option<f64>,
        debug_channel: Option<Rc<DebugChannel>>,
    ) -> Rc<WeakResponse> {
        create_response(
            bundler_config,
            server_reference_config,
            module_loading,
            call_server,
            encode_form_action,
            nonce,
            temporary_references,
            allow_partial_stream,
            find_source_map_url,
            replay_console,
            environment_name,
            debug_start_time,
            debug_end_time,
            debug_channel,
        )
    }

    pub fn create_stream_state(
        &self,
        weak_response: &WeakResponse,
        stream_debug_value: Option<&'static Unknown>,
    ) -> StreamState {
        create_stream_state(weak_response, stream_debug_value)
    }

    pub fn process_binary_chunk(
        &self,
        weak_response: &WeakResponse,
        stream_state: &StreamState,
        chunk: &'static js::Uint8Array,
    ) {
        process_binary_chunk(weak_response, stream_state, chunk)
    }

    pub fn get_root(&self, weak_response: &WeakResponse) -> Chunk {
        get_root(weak_response)
    }

    pub fn close(&self, weak_response: &WeakResponse) {
        close(weak_response)
    }
}
