// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use js::{
    ArrayBuffer, AsyncIterator, BigInt64Array, BigUint64Array, DataView, Error, Float32Array,
    Float64Array, Int8Array, Int16Array, Int32Array, JsObject, Kind, Map, Promise,
    PropertyDescriptor, Uint8Array, Uint8ClampedArray, Uint16Array, Uint32Array, Unknown, WeakMap,
    WeakRef, WeakSet, function,
};
use node::async_hooks::AsyncLocalStorage;
use react_isomorphic::react_taint_registry::{EntryValue, RequestCleanupQueue, TaintEntry};
use shared::environment::__DEV__;
use shared::react_types::{
    ReactComponentInfo, ReactDebugInfo, ReactDebugInfoEntry, ReactIOInfo, ReactKey,
    ReactStackTrace, Thenable,
};
use webapi::{
    AbortController, AbortSignal, Blob, ConsoleTask, FormData, ReadableStream, console, performance,
};

use shared::react_feature_flags::{ENABLE_PROFILER_TIMER, ENABLE_TAINT};

use crate::react_flight_async_sequence::{AsyncSequence, PromiseWithDebugInfo};

use crate::react_flight_server_temporary_references::TemporaryReferenceSet;

use crate::react_flight_thenable::{SUSPENSE_EXCEPTION, ThenableState};

use crate::react_flight_async_dispatcher::DEFAULT_ASYNC_DISPATCHER;

use crate::react_flight_current_owner::resolve_owner;

use shared::react_component_info_stack::get_owner_stack_by_component_info_in_dev;
use shared::react_owner_stack_reset::reset_owner_stack_limit;

use std::cell::OnceCell;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

use js::{
    AggregateError, ArrayBufferView, Date, ErrorOptions, IntoUnknown, IteratorResult, Set, Symbol,
    SymbolFor,
};
use react_isomorphic::react_lazy::LazyComponent;
use shared::binary_to_comparable_string::binary_to_comparable_string;
use shared::get_prototype_of::GET_PROTOTYPE_OF;
use shared::has_own_property::HAS_OWN_PROPERTY;
use shared::is_array::is_array;

use shared::noop::noop;
use shared::react_element_type::ReactElement;
use shared::react_feature_flags::{
    ENABLE_COMPONENT_PERFORMANCE_TRACK, ENABLE_FLIGHT_OBJECT_REFERENCES,
    ENABLE_FLIGHT_WEAK_THENABLES,
};

use shared::react_serialization_errors::{
    JSX_CHILDREN_PARENTS, JSX_PROPS_PARENTS, describe_object_for_error_message, is_getter,
    is_simple_object, object_name,
};

use crate::react_shared_internals_server::REACT_SHARED_INTERNALS_SERVER as REACT_SHARED_INTERNALS;

use shared::react_symbols::{
    ASYNC_ITERATOR, REACT_ELEMENT_TYPE, REACT_FORWARD_REF_TYPE, REACT_FRAGMENT_TYPE,
    REACT_LAZY_TYPE, REACT_LEGACY_ELEMENT_TYPE, REACT_MEMO_TYPE, REACT_OPTIMISTIC_KEY,
    get_iterator_fn,
};
use shared::react_types::{
    FulfilledThenable, PendingThenable, ReactAsyncInfo, ReactEnvironmentInfo, ReactErrorInfo,
    ReactErrorInfoDev, ReactErrorInfoProd, ReactFunctionLocation,
};
use webapi::{
    AddEventListenerOptions, Byob, Event, EventTargetExt, ReadableStreamDefaultReader,
    ReadableStreamGetByobReaderOptions, ReadableStreamReadResult, events,
};

use crate::react_fizz_call_user_space::Component;
use crate::react_flight_async_sequence::{
    AWAIT_NODE, IO_NODE, PROMISE_NODE, UNRESOLVED_AWAIT_NODE, UNRESOLVED_PROMISE_NODE,
};
use crate::react_flight_call_user_space::{
    CALL_COMPONENT_IN_DEV, CALL_ITERATOR_IN_DEV, CALL_LAZY_INIT_IN_DEV, IteratorError,
    Progress as IteratorProgress,
};
use crate::react_flight_current_owner::set_current_owner;

use crate::react_flight_hooks::{
    HOOKS_DISPATCHER, get_thenable_state_after_suspending, get_tracked_thenables_after_rendering,
    prepare_to_use_hooks_for_component, prepare_to_use_hooks_for_request, reset_hooks_for_request,
};
use crate::react_flight_server_temporary_references::{
    is_opaque_temporary_reference, resolve_temporary_reference,
};
use crate::react_flight_thenable::get_suspended_thenable;

// ReactServerStreamConfig's types, of its fork for the host.

/// Where a request writes, the host's stream.
pub type Destination = Unknown;
/// What's written: text, or its encoded bytes.
pub type Chunk = Unknown;
/// What's written of binary data.
pub type BinaryChunk = Unknown;

// ReactFlightServerConfig's types, of each bundler's and host's config.

/// A bundler's map of its client modules, which a request is made with.
pub type ClientManifest = Unknown;
/// What the client is sent of a client reference to load its module.
pub type ClientReferenceMetadata = Unknown;
/// A reference to a client module's export, the bundler's.
pub type ClientReference = Unknown;
/// What a client reference is deduplicated by.
pub type ClientReferenceKey = Unknown;
/// A function a client may call on the server, the bundler's.
pub type ServerReference = Unknown;
/// What a server reference is sent as.
pub type ServerReferenceId = Unknown;
/// The hints a request has emitted, the host's.
pub type Hints = Unknown;
/// A hint's model, its code's.
pub type HintModel = Unknown;
/// The host's context of what a model is rendered in.
pub type FormatContext = Unknown;

// Serializable values

/// Any value a server component may render or pass to the client:
/// elements, lazy components, references, text, numbers, symbols, `null`,
/// `undefined`, bigints, streams, iterables, arrays, maps, sets, form data,
/// binary data, dates, plain objects and promises.
pub type ReactClientValue = Unknown;

/// What a model is serialized as, `JSON.stringify`'s: text, a boolean, a
/// number, `null`, or an array or object of them.
pub type ReactJsonValue = Unknown;

// ReactServerStreamConfig's functions, of its fork for the host.

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactServerStreamConfig.js#scheduleWork"
)]
fn schedule_work(callback: impl FnOnce() + 'static) {
    let _ = callback;
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactServerStreamConfig.js#scheduleMicrotask"
)]
fn schedule_microtask(callback: impl FnOnce() + 'static) {
    let _ = callback;
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactServerStreamConfig.js#flushBuffered"
)]
fn flush_buffered(destination: &Destination) {
    let _ = destination;
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactServerStreamConfig.js#beginWriting"
)]
fn begin_writing(destination: &Destination) {
    let _ = destination;
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactServerStreamConfig.js#writeChunk"
)]
fn write_chunk(destination: &Destination, chunk: &Chunk) {
    let _ = (destination, chunk);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactServerStreamConfig.js#writeChunkAndReturn"
)]
fn write_chunk_and_return(destination: &Destination, chunk: &Chunk) -> bool {
    let _ = (destination, chunk);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactServerStreamConfig.js#stringToChunk"
)]
fn string_to_chunk(content: &str) -> &'static Chunk {
    let _ = content;
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactServerStreamConfig.js#typedArrayToBinaryChunk"
)]
fn typed_array_to_binary_chunk(content: &Unknown) -> &'static BinaryChunk {
    let _ = content;
    unreachable!()
}

/// A host's count of a chunk's bytes, `null` where the host doesn't count
/// them.
pub type ByteLengthOfChunk = dyn Fn(&Chunk) -> f64;

unsafe extern "Rust" {
    #[link_name = "./src/ReactServerStreamConfig.js#byteLengthOfChunk"]
    #[cfg_attr(rust_js, rust_js::nullable)]
    safe static BYTE_LENGTH_OF_CHUNK: Option<&'static ByteLengthOfChunk>;
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactServerStreamConfig.js#byteLengthOfBinaryChunk"
)]
fn byte_length_of_binary_chunk(chunk: &BinaryChunk) -> f64 {
    let _ = chunk;
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactServerStreamConfig.js#completeWriting"
)]
fn complete_writing(destination: &Destination) {
    let _ = destination;
    unreachable!()
}

#[cfg_attr(rust_js, rust_js::link_name = "./src/ReactServerStreamConfig.js#close")]
fn close(destination: &Destination) {
    let _ = destination;
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactServerStreamConfig.js#closeWithError"
)]
fn close_with_error(destination: &Destination, error: Option<&Unknown>) {
    let _ = (destination, error);
    unreachable!()
}

// ReactFlightServerConfig's functions and values, of the bundler's and the
// host's config.

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightServerConfig.js#resolveClientReferenceMetadata"
)]
fn resolve_client_reference_metadata(
    config: Option<&ClientManifest>,
    client_reference: &ClientReference,
) -> &'static ClientReferenceMetadata {
    let _ = (config, client_reference);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightServerConfig.js#getServerReferenceId"
)]
fn get_server_reference_id(
    config: Option<&ClientManifest>,
    server_reference: &ServerReference,
) -> &'static ServerReferenceId {
    let _ = (config, server_reference);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightServerConfig.js#getServerReferenceBoundArguments"
)]
#[cfg_attr(rust_js, rust_js::nullable)]
fn get_server_reference_bound_arguments(
    config: Option<&ClientManifest>,
    server_reference: &ServerReference,
) -> Option<&'static [Option<&'static ReactClientValue>]> {
    let _ = (config, server_reference);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightServerConfig.js#getServerReferenceLocation"
)]
fn get_server_reference_location(
    config: Option<&ClientManifest>,
    server_reference: &ServerReference,
) -> Option<&'static Error> {
    let _ = (config, server_reference);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightServerConfig.js#getClientReferenceKey"
)]
fn get_client_reference_key(reference: &ClientReference) -> &'static ClientReferenceKey {
    let _ = reference;
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightServerConfig.js#isClientReference"
)]
fn is_client_reference(reference: &Unknown) -> bool {
    let _ = reference;
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightServerConfig.js#isServerReference"
)]
fn is_server_reference(reference: &Unknown) -> bool {
    let _ = reference;
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightServerConfig.js#createHints"
)]
fn create_hints() -> &'static Hints {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightServerConfig.js#createRootFormatContext"
)]
fn create_root_format_context() -> &'static FormatContext {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightServerConfig.js#getChildFormatContext"
)]
fn get_child_format_context(
    parent_context: &FormatContext,
    r#type: &str,
    props: &Unknown,
) -> &'static FormatContext {
    let _ = (parent_context, r#type, props);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightServerConfig.js#initAsyncDebugInfo"
)]
fn init_async_debug_info() {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightServerConfig.js#markAsyncSequenceRootTask"
)]
fn mark_async_sequence_root_task() {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightServerConfig.js#getCurrentAsyncSequence"
)]
#[cfg_attr(rust_js, rust_js::nullable)]
fn get_current_async_sequence() -> Option<Rc<AsyncSequence>> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightServerConfig.js#getAsyncSequenceFromPromise"
)]
#[cfg_attr(rust_js, rust_js::nullable)]
fn get_async_sequence_from_promise(promise: &'static Unknown) -> Option<Rc<AsyncSequence>> {
    let _ = promise;
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightServerConfig.js#parseStackTrace"
)]
fn parse_stack_trace(error: &'static js::JsError, skip_frames: usize) -> Rc<ReactStackTrace> {
    let _ = (error, skip_frames);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightServerConfig.js#parseStackTracePrivate"
)]
#[cfg_attr(rust_js, rust_js::nullable)]
fn parse_stack_trace_private(
    error: &js::JsError,
    skip_frames: usize,
) -> Option<Rc<ReactStackTrace>> {
    let _ = (error, skip_frames);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightServerConfig.js#unbadgeConsole"
)]
#[cfg_attr(rust_js, rust_js::nullable)]
fn unbadge_console(method_name: &str, args: &'static Unknown) -> Option<String> {
    let _ = (method_name, args);
    unreachable!()
}

/// Where a host keeps the component a server component renders in, its
/// Flight config's: an `AsyncLocalStorage`, or none.
pub type ComponentStorage = AsyncLocalStorage<&'static ReactComponentInfo>;

unsafe extern "Rust" {
    #[link_name = "./src/ReactFlightServerConfig.js#supportsRequestStorage"]
    safe static SUPPORTS_REQUEST_STORAGE: bool;
    #[link_name = "./src/ReactFlightServerConfig.js#requestStorage"]
    safe static REQUEST_STORAGE: &'static AsyncLocalStorage<Option<Rc<Request>>>;
    #[link_name = "./src/ReactFlightServerConfig.js#supportsComponentStorage"]
    safe static SUPPORTS_COMPONENT_STORAGE: bool;
    #[link_name = "./src/ReactFlightServerConfig.js#componentStorage"]
    safe static COMPONENT_STORAGE: &'static ComponentStorage;
}

thread_local! {
    // DEV-only set containing internal objects that should not be limited and turned into getters.
    #[cfg_attr(rust_js, rust_js::name = "doNotLimit")]
    static DO_NOT_LIMIT: Option<&'static WeakSet<JsObject>> =
        if __DEV__ { Some(js::weak_set::new()) } else { None };
}

/// A request's `filterStackFrame`: whether a frame is shown, of its URL, its
/// function's name and its line and column.
pub type FilterStackFrame = dyn Fn(&str, &str, f64, f64) -> bool;

fn default_filter_stack_frame(
    filename: &str,
    _function_name: &str,
    _line_number: f64,
    _column_number: f64,
) -> bool {
    !filename.is_empty() && !filename.starts_with("node:") && !filename.contains("node_modules")
}

#[cfg_attr(rust_js, rust_js::name = "devirtualizeURL")]
fn devirtualize_url(url: &str) -> String {
    if url.starts_with("about://React/") {
        // This callsite is a virtual fake callsite that came from another Flight client.
        // We need to reverse it back into the original location by stripping its prefix
        // and suffix. We don't need the environment name because it's available on the
        // parent object that will contain the stack.
        let env_idx =
            js::string::index_of(url, "/", Some(js::string::length("about://React/") as f64));
        let suffix_idx = js::string::last_index_of(url, "?", None);
        if env_idx > -1.0 && suffix_idx > -1.0 {
            return js::decode_uri(&js::string::slice(url, env_idx + 1.0, Some(suffix_idx)))
                .unwrap_or_else(|error| js::throw(error.as_unknown()));
        }
    }
    url.to_owned()
}

fn is_promise_creation_internal(url: &str, function_name: &str) -> bool {
    // Various internals of the JS VM can create Promises but the call frame of the
    // internals are not very interesting for our purposes so we need to skip those.
    if url == "node:internal/async_hooks" {
        // Ignore the stack frames from the async hooks themselves.
        return true;
    }
    if !url.is_empty() {
        return false;
    }
    // V8 used to name the frames of static methods on the Promise constructor
    // "Function.x" but newer versions name them "Promise.x". We match both.
    matches!(
        function_name,
        "new Promise"
            | "Function.withResolvers"
            | "Promise.withResolvers"
            | "Function.reject"
            | "Promise.reject"
            | "Function.resolve"
            | "Promise.resolve"
            | "Function.all"
            | "Promise.all"
            | "Function.allSettled"
            | "Promise.allSettled"
            | "Function.race"
            | "Promise.race"
            | "Function.try"
            | "Promise.try"
    )
}

fn strip_leading_promise_creation_frames(stack: &Rc<ReactStackTrace>) -> Rc<ReactStackTrace> {
    for (i, callsite) in stack.iter().enumerate() {
        let function_name = &callsite.0;
        let url = &callsite.1;
        if !is_promise_creation_internal(url, function_name) {
            if i > 0 {
                return Rc::new(stack[i..].to_vec());
            } else {
                return Rc::clone(stack);
            }
        }
    }
    Rc::new(vec![])
}

fn find_called_function_name_from_stack_trace(
    request: &Request,
    stack: &ReactStackTrace,
) -> String {
    // Gets the name of the first function called from first party code.
    let mut best_match = String::new();
    let filter_stack_frame = &request.filter_stack_frame;
    for callsite in stack {
        let function_name = &callsite.0;
        let url = devirtualize_url(&callsite.1);
        let line_number = callsite.2;
        let column_number = callsite.3;
        if filter_stack_frame(&url, function_name, line_number, column_number)
            // Don't consider anonymous code first party even if the filter wants to include them in the stack.
            && !url.is_empty()
        {
            if best_match.is_empty() {
                // If we had no good stack frames for internal calls, just use the last
                // first party function name.
                return function_name.clone();
            }
            return best_match;
        } else {
            best_match = function_name.clone();
        }
    }
    String::new()
}

fn filter_stack_trace(request: &Request, stack: &ReactStackTrace) -> ReactStackTrace {
    // Since stacks can be quite large and we pass a lot of them, we filter them out eagerly
    // to save bandwidth even in DEV. We'll also replay these stacks on the client so by
    // stripping them early we avoid that overhead. Otherwise we'd normally just rely on
    // the DevTools or framework's ignore lists to filter them out.
    let filter_stack_frame = &request.filter_stack_frame;
    let mut filtered_stack: ReactStackTrace = vec![];
    for callsite in stack {
        let function_name = &callsite.0;
        let url = devirtualize_url(&callsite.1);
        let line_number = callsite.2;
        let column_number = callsite.3;
        if filter_stack_frame(&url, function_name, line_number, column_number) {
            // Use a clone because the Flight protocol isn't yet resilient to deduping
            // objects in the debug info. TODO: Support deduping stacks.
            let mut clone = callsite.clone();
            clone.1 = url;
            filtered_stack.push(clone);
        }
    }
    filtered_stack
}

fn has_unfiltered_frame(request: &Request, stack: &ReactStackTrace) -> bool {
    let filter_stack_frame = &request.filter_stack_frame;
    for callsite in stack {
        let function_name = &callsite.0;
        let url = devirtualize_url(&callsite.1);
        let line_number = callsite.2;
        let column_number = callsite.3;
        // Ignore async stack frames because they're not "real". We'd expect to have at least
        // one non-async frame if we're actually executing inside a first party function.
        // Otherwise we might just be in the resume of a third party function that resumed
        // inside a first party stack.
        let is_async = callsite.6;
        if !is_async
            && filter_stack_frame(&url, function_name, line_number, column_number)
            // Ignore anonymous stack frames like internals. They are also not in first party
            // code even though it might be useful to include them in the final stack.
            && !url.is_empty()
        {
            return true;
        }
    }
    false
}

fn is_promise_await_internal(url: &str, function_name: &str) -> bool {
    // Various internals of the JS VM can await internally on a Promise. If those are at
    // the top of the stack then we don't want to consider them as internal frames. The
    // true "await" conceptually is the thing that called the helper.
    // Ideally we'd also include common third party helpers for this.
    if url == "node:internal/async_hooks" {
        // Ignore the stack frames from the async hooks themselves.
        return true;
    }
    if !url.is_empty() {
        return false;
    }
    // V8 used to name the frames of static methods on the Promise constructor
    // "Function.x" but newer versions name them "Promise.x". We match both.
    matches!(
        function_name,
        "Promise.then"
            | "Promise.catch"
            | "Promise.finally"
            | "Function.reject"
            | "Promise.reject"
            | "Function.resolve"
            | "Promise.resolve"
            | "Function.all"
            | "Promise.all"
            | "Function.allSettled"
            | "Promise.allSettled"
            | "Function.any"
            | "Promise.any"
            | "Function.race"
            | "Promise.race"
            | "Function.try"
            | "Promise.try"
            | "Function.withResolvers"
            | "Promise.withResolvers"
    )
}

pub fn is_await_in_userspace(request: &Request, stack: &ReactStackTrace) -> bool {
    let mut first_frame = 0;
    while stack.len() > first_frame
        && is_promise_await_internal(&stack[first_frame].1, &stack[first_frame].0)
    {
        // Skip the internal frame that awaits itself.
        first_frame += 1;
    }
    if stack.len() > first_frame {
        // Check if the very first stack frame that awaited this Promise was in user space.
        // TODO: This doesn't take into account wrapper functions such as our fake .then()
        // in FlightClient which will always be considered third party awaits if you call
        // .then directly.
        let filter_stack_frame = &request.filter_stack_frame;
        let callsite = &stack[first_frame];
        let function_name = &callsite.0;
        let url = devirtualize_url(&callsite.1);
        let line_number = callsite.2;
        let column_number = callsite.3;
        return filter_stack_frame(&url, function_name, line_number, column_number)
            && !url.is_empty();
    }
    false
}

js::on_load! {
    init_async_debug_info();
}

fn patch_console(console_inst: &'static Unknown, method_name: &'static str) {
    let descriptor = js::object::get_own_property_descriptor(Some(console_inst), method_name);
    if let Some(descriptor) = descriptor
        && (descriptor.configurable.unwrap_or(false) || descriptor.writable.unwrap_or(false))
        && let Kind::Function(original_method) = js::classify(descriptor.value)
    {
        let original_name =
            js::object::get_own_property_descriptor(Some(js::unknown(original_method)), "name");
        let wrapper_method = function::with_this(move |this: Option<&'static Unknown>, ()| {
            let request = resolve_request();
            if method_name == "assert" && js::truthy(js::get(js::arguments(), &0u32)) {
                // assert doesn't emit anything unless first argument is falsy so we can skip it.
            } else if let Some(request) = request {
                // Extract the stack. Not all console logs print the full stack but they have at
                // least the line it was called from. We could optimize transfer by keeping just
                // one stack frame but keeping it simple for now and include all frames.
                let stack = filter_stack_trace(
                    &request,
                    &parse_stack_trace_private(
                        js::error::new(Some("react-stack-top-frame"), None),
                        1,
                    )
                    .unwrap_or_default(),
                );
                request
                    .pending_debug_chunks
                    .set(request.pending_debug_chunks.get() + 1.0);
                let owner = resolve_owner();
                let args: &'static Unknown =
                    js::unknown_of(js::array::from::<Option<&'static Unknown>>(js::unknown(
                        js::arguments(),
                    )));
                // Extract the env if this is a console log that was replayed from another env.
                let mut env = unbadge_console(method_name, args);
                if env.is_none() {
                    // Otherwise add the current environment.
                    env = Some((request.environment_name)());
                }

                emit_console_chunk(
                    &request,
                    method_name,
                    owner,
                    &env.unwrap_or_default(),
                    &stack,
                    args,
                );
            }
            // $FlowFixMe[incompatible-call]
            // $FlowFixMe[incompatible-type]
            function::apply(
                original_method,
                this.map(|this| this as &dyn js::IntoUnknown),
                Some(js::arguments()),
            )
        });
        if let Some(original_name) = original_name {
            js::object::define_property(
                js::unknown(wrapper_method),
                // $FlowFixMe[cannot-write] yes it is
                "name",
                &original_name,
            );
        }
        js::object::define_property(
            console_inst,
            method_name,
            &PropertyDescriptor {
                value: Some(&wrapper_method),
                ..Default::default()
            },
        );
    }
}

js::on_load! {
    // $FlowFixMe[invalid-compare]
    if __DEV__ && js::is_object(webapi::console) {
        // Instrument console to capture logs for replaying on the client.
        patch_console(js::unknown(webapi::console), "assert");
        patch_console(js::unknown(webapi::console), "debug");
        patch_console(js::unknown(webapi::console), "dir");
        patch_console(js::unknown(webapi::console), "dirxml");
        patch_console(js::unknown(webapi::console), "error");
        patch_console(js::unknown(webapi::console), "group");
        patch_console(js::unknown(webapi::console), "groupCollapsed");
        patch_console(js::unknown(webapi::console), "groupEnd");
        patch_console(js::unknown(webapi::console), "info");
        patch_console(js::unknown(webapi::console), "log");
        patch_console(js::unknown(webapi::console), "table");
        patch_console(js::unknown(webapi::console), "trace");
        patch_console(js::unknown(webapi::console), "warn");
    }
}

#[cfg_attr(rust_js, rust_js::name = "getCurrentStackInDEV")]
fn get_current_stack_in_dev() -> String {
    if __DEV__ {
        let owner = resolve_owner();
        let Some(owner) = owner else {
            return String::new();
        };
        return get_owner_stack_by_component_info_in_dev(owner);
    }
    String::new()
}

/// `stringify(value)`, upstream's `JSON.stringify`, of a model that's never
/// `undefined`: what it throws, it throws.
fn stringify(value: Option<&Unknown>) -> String {
    match js::json::stringify(&value) {
        Ok(json) => json.unwrap_or_default(),
        Err(error) => js::throw(error.as_unknown()),
    }
}

// task status
pub(crate) const PENDING: i32 = 0;
pub(crate) const COMPLETED: i32 = 1;
pub(crate) const ABORTED: i32 = 3;
pub(crate) const ERRORED: i32 = 4;
pub(crate) const RENDERING: i32 = 5;

/// The set of tasks a request may abort, shared by the tasks made in it.
pub type AbortSet = Rc<RefCell<HashSet<Rc<Task>>>>;

/// A unit of work: a model to render and emit as one row, `id`.
pub struct Task {
    pub id: f64,
    pub status: Cell<i32>,
    pub model: Cell<Option<&'static ReactClientValue>>,
    // parent server component keys
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub key_path: RefCell<Option<ReactKey>>,
    // true if the root server component of this sequence had a null key
    pub implicit_slot: Cell<bool>,
    // an approximate parent context from host components
    pub format_context: Cell<&'static FormatContext>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub thenable_state: RefCell<Option<Rc<ThenableState>>>,
    // GAP rd493: upstream sets the Profiling-only and DEV-only fields only
    // under their flags; a struct's fields are always set.
    // Profiling-only. Whether we need to track the completion time of this task.
    pub timed: Cell<bool>,
    // Profiling-only. The last time stamp emitted for this task.
    pub time: Cell<f64>,
    // DEV-only. Used to track if the environment for this task changed.
    pub environment_name: RefCell<String>,
    // DEV-only. Each of the three is what it was given, `null` or the
    // element's, `undefined` of a production element's clone, which upstream's
    // `!== null` tests let through.
    pub debug_owner: Cell<Option<&'static Unknown>>,
    // DEV-only
    pub debug_stack: Cell<Option<&'static Unknown>>,
    // DEV-only
    pub debug_task: Cell<Option<&'static Unknown>>,
}

// A task is itself: two are equal only where they're one, as a JS `Set` of
// them holds each.
impl PartialEq for Task {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Eq for Task {}

impl Hash for Task {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::ptr::hash(self, state);
    }
}

/// `task.debugStack === null ? null : filterStackTrace(request,
/// parseStackTrace(task.debugStack, 1))`: the task's stack, parsed. Upstream's
/// `!== null` lets through the `undefined` a production element's clone
/// gives, whose `stack` parseStackTrace reads, a TypeError.
fn task_debug_stack_trace(request: &Request, task: &Task) -> Option<ReactStackTrace> {
    let debug_stack = task.debug_stack.get();
    if matches!(js::classify(debug_stack), Kind::Null) {
        return None;
    }
    // SAFETY: what isn't `null` is an `Error`, or `undefined`.
    let error: Option<&'static Error> = unsafe { js::cast(debug_stack) };
    let Some(debug_stack) = error else {
        // `undefined.stack` throws, as upstream's parseStackTrace of it does.
        js::get(debug_stack, "stack");
        unreachable!("reading an undefined value's property throws");
    };
    Some(filter_stack_trace(
        request,
        &parse_stack_trace(debug_stack, 1),
    ))
}

/// What isn't `null` of a task's `debugOwner`, a component's info, which
/// upstream's `debugOwner !== null` lets through and outlines. Of the
/// `undefined` a production element's clone gives, outlineComponentInfo's
/// read of its `owner` throws a TypeError, as upstream's does.
fn task_debug_owner_info(debug_owner: Option<&'static Unknown>) -> &'static ReactComponentInfo {
    // SAFETY: what isn't `null` is a component's info, or `undefined`.
    let info: Option<&'static ReactComponentInfo> = unsafe { js::cast(debug_owner) };
    let Some(info) = info else {
        // `undefined.owner` throws, as upstream's outlineComponentInfo of it does.
        js::get(debug_owner, "owner");
        unreachable!("reading an undefined value's property throws");
    };
    info
}

/// What a debug model's serialization may still outline, `{objectLimit}`,
/// counted down as it goes.
pub struct Counter {
    pub object_limit: Cell<f64>,
}

/// The debug objects a request keeps alive for a debug channel to ask for
/// later, by id and by the object.
pub struct DeferredDebugStore {
    pub retained: &'static Map<f64, &'static Unknown>,
    pub existing: &'static Map<Unknown, f64>,
}

const __PROTO__: &str = "__proto__";

const OPENING: i32 = 10;
const OPEN: i32 = 11;
const ABORTING: i32 = 12;
const CLOSING: i32 = 13;
const CLOSED: i32 = 14;

const RENDER: i32 = 20;
const PRERENDER: i32 = 21;

thread_local! {
    // Marker pushed before a [headerChunk, contentChunk] pair in
    // completedRegularChunks / completedDebugChunks to signal that the next two
    // entries must be written atomically — see emitTextChunk and
    // emitTypedArrayChunk for why, and flushCompletedChunks for how it's read.
    #[cfg_attr(rust_js, rust_js::name = "NEXT_TWO_CHUNKS_ARE_ATOMIC")]
    static NEXT_TWO_CHUNKS_ARE_ATOMIC: &'static Chunk = js::unknown(js::symbol::new(None));
}

/// What a resource type of `cache()` makes: any value, made once per cache.
pub type ResourceType = dyn Fn() -> Option<&'static Unknown>;

/// A request's cache, of each resource type.
pub type Cache = Map<ResourceType, Option<&'static Unknown>>;

/// A request's `onError`, told of each error: the digest the client is
/// sent of it, if any.
pub type OnError = dyn Fn(Option<&'static Unknown>) -> Option<String>;

/// A request's `onAllReady`.
pub type OnAllReady = dyn Fn();

/// A request's `onFatalError`, of what was thrown.
pub type OnFatalError = dyn Fn(Option<&'static Unknown>);

/// A request's `environmentName`: the name itself, or what gives it.
#[cfg_attr(rust_js, rust_js::untagged)]
#[derive(Clone)]
pub enum EnvironmentName {
    Str(String),
    Fn(Rc<dyn Fn() -> String>),
}

// Safety: a request is a JS object of its fields.
unsafe impl js::Defined for Request {}

#[cfg_attr(rust_js, rust_js::class, rust_js::name = "RequestInstance")]
pub struct Request {
    pub status: Cell<i32>,
    pub r#type: i32,
    pub flush_scheduled: Cell<bool>,
    // What was thrown, `undefined` too: `null` until then.
    pub fatal_error: Cell<Option<&'static Unknown>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub destination: Cell<Option<&'static Destination>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub bundler_config: Option<&'static ClientManifest>,
    pub cache: &'static Cache,
    pub cache_controller: &'static AbortController,
    pub next_chunk_id: Cell<f64>,
    pub pending_chunks: Cell<f64>,
    pub hints: &'static Hints,
    pub abortable_tasks: AbortSet,
    pub pinged_tasks: RefCell<Vec<Rc<Task>>>,
    pub completed_import_chunks: RefCell<Vec<&'static Chunk>>,
    pub completed_hint_chunks: RefCell<Vec<&'static Chunk>>,
    // Text and TypedArray rows are pushed as a NEXT_TWO_CHUNKS_ARE_ATOMIC
    // sentinel followed by their [headerChunk, contentChunk] pair, so that
    // flushCompletedChunks can write the pair atomically and never strand the
    // content chunk on a backpressure break.
    pub completed_regular_chunks: RefCell<Vec<&'static Chunk>>,
    pub completed_error_chunks: RefCell<Vec<&'static Chunk>>,
    pub written_symbols: &'static Map<js::Symbol, f64>,
    pub written_client_references: &'static Map<ClientReferenceKey, f64>,
    pub written_server_references: &'static Map<ServerReference, f64>,
    pub written_objects: &'static WeakMap<JsObject, String>,
    pub written_import_strings: &'static Map<str, String>,
    // The combined length of the keys in writtenImportStrings.
    pub written_import_strings_size: Cell<f64>,
    pub temporary_references: Option<&'static TemporaryReferenceSet>,
    pub identifier_prefix: String,
    pub identifier_count: Cell<f64>,
    pub taint_cleanup_queue: &'static RequestCleanupQueue,
    pub on_error: Rc<OnError>,
    on_all_ready: Rc<OnAllReady>,
    pub on_fatal_error: Rc<OnFatalError>,
    // GAP rd493: upstream sets the Profiling-only and DEV-only fields only
    // under their flags; a struct's fields are always set.
    // Profiling-only
    pub time_origin: Cell<f64>,
    pub abort_time: Cell<f64>,
    // DEV-only
    pub pending_debug_chunks: Cell<f64>,
    // See completedRegularChunks for why some entries are preceded by the
    // NEXT_TWO_CHUNKS_ARE_ATOMIC sentinel.
    pub completed_debug_chunks: RefCell<Vec<&'static Chunk>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub debug_destination: Cell<Option<&'static Destination>>,
    pub environment_name: Rc<dyn Fn() -> String>,
    pub filter_stack_frame: Rc<FilterStackFrame>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub did_warn_for_key: Cell<Option<&'static WeakSet<JsObject>>>,
    pub written_debug_objects: &'static WeakMap<JsObject, String>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub deferred_debug_objects: Option<DeferredDebugStore>,
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "TaintRegistryObjects")]
    static TAINT_REGISTRY_OBJECTS: Option<&'static WeakMap<react_isomorphic::react_taint_registry::Reference, String>> =
        REACT_SHARED_INTERNALS.with(|internals| internals.taint_registry_objects.get());
    #[cfg_attr(rust_js, rust_js::name = "TaintRegistryValues")]
    static TAINT_REGISTRY_VALUES: Option<&'static Map<EntryValue, Rc<TaintEntry>>> =
        REACT_SHARED_INTERNALS.with(|internals| internals.taint_registry_values.get());
    #[cfg_attr(rust_js, rust_js::name = "TaintRegistryByteLengths")]
    static TAINT_REGISTRY_BYTE_LENGTHS: Option<&'static js::Set<f64>> =
        REACT_SHARED_INTERNALS.with(|internals| internals.taint_registry_byte_lengths.get());
    #[cfg_attr(rust_js, rust_js::name = "TaintRegistryPendingRequests")]
    static TAINT_REGISTRY_PENDING_REQUESTS: Option<Rc<RefCell<HashSet<&'static RequestCleanupQueue>>>> =
        REACT_SHARED_INTERNALS.with(|internals| internals.taint_registry_pending_requests.borrow().clone());
}

fn throw_taint_violation(message: &str) -> ! {
    // eslint-disable-next-line react-internal/prod-error-codes
    panic!("{message}");
}

fn cleanup_taint_queue(request: &Request) {
    let cleanup_queue = request.taint_cleanup_queue;
    TAINT_REGISTRY_PENDING_REQUESTS.with(|pending_requests| {
        if let Some(pending_requests) = pending_requests {
            pending_requests.borrow_mut().remove(cleanup_queue);
        }
    });
    let taint_registry_values = TAINT_REGISTRY_VALUES.with(|&values| values.unwrap());
    for entry_value in cleanup_queue.values.borrow().iter() {
        let entry = taint_registry_values.get(entry_value);
        if let Some(entry) = entry {
            if entry.count.get() == 1 {
                taint_registry_values.delete(entry_value);
            } else {
                entry.count.set(entry.count.get() - 1);
            }
        }
    }
    cleanup_queue.values.borrow_mut().clear();
}

fn default_error_handler(error: Option<&'static Unknown>) -> Option<String> {
    console::error((error,));
    // Don't transform to our wrapper
    None
}

#[cfg_attr(rust_js, rust_js::class)]
impl Request {
    #[cfg_attr(rust_js, rust_js::nullable(bundler_config))]
    fn new(
        r#type: i32,
        model: Option<&'static ReactClientValue>,
        bundler_config: Option<&'static ClientManifest>,
        on_error: Option<Rc<OnError>>,
        on_all_ready: Rc<OnAllReady>,
        on_fatal_error: Rc<OnFatalError>,
        identifier_prefix: Option<&str>,
        temporary_references: Option<&'static TemporaryReferenceSet>,
        debug_start_time: Option<f64>,             // Profiling-only
        environment_name: Option<EnvironmentName>, // DEV-only
        filter_stack_frame: Option<Rc<FilterStackFrame>>, // DEV-only
        keep_debug_alive: bool,                    // DEV-only
    ) -> Rc<Self> {
        DEFAULT_ASYNC_DISPATCHER.with(|default_async_dispatcher| {
            REACT_SHARED_INTERNALS.with(|internals| {
                if let Some(a) = internals.a.get()
                    && !js::strict_equals(a, default_async_dispatcher)
                {
                    panic!("Currently React only supports one RSC renderer at a time.");
                }
                internals.a.set(Some(unsafe {
                    js::cast(Some(js::unknown(default_async_dispatcher)))
                }));
            });
        });
        if __DEV__ {
            // Unlike Fizz or Fiber, we don't reset this and just keep it on permanently.
            // This lets it act more like the AsyncDispatcher so that we can get the
            // stack asynchronously too.
            REACT_SHARED_INTERNALS.with(|internals| {
                internals
                    .get_current_stack
                    .set(Some(&get_current_stack_in_dev));
            });
        }

        let abort_set: AbortSet = Rc::new(RefCell::new(HashSet::new()));
        let pinged_tasks: Vec<Rc<Task>> = vec![];
        // Safety: a `RequestCleanupQueue` is its array, which the taint registry
        // holds as long as the request runs.
        let cleanup_queue: &'static RequestCleanupQueue =
            unsafe { js::cast(Some(js::unknown_of(Vec::<EntryValue>::new()))) };
        if ENABLE_TAINT.with(|&enabled| enabled) {
            TAINT_REGISTRY_PENDING_REQUESTS.with(|pending_requests| {
                if let Some(pending_requests) = pending_requests {
                    pending_requests.borrow_mut().insert(cleanup_queue);
                }
            });
        }
        let hints = create_hints();
        let request = Rc::new(Request {
            r#type,
            status: Cell::new(OPENING),
            flush_scheduled: Cell::new(false),
            fatal_error: Cell::new(js::null()),
            destination: Cell::new(None),
            bundler_config,
            cache: js::map::new(None),
            cache_controller: webapi::abort_controller::new(),
            next_chunk_id: Cell::new(0.0),
            pending_chunks: Cell::new(0.0),
            hints,
            abortable_tasks: Rc::clone(&abort_set),
            pinged_tasks: RefCell::new(pinged_tasks),
            completed_import_chunks: RefCell::new(vec![]),
            completed_hint_chunks: RefCell::new(vec![]),
            completed_regular_chunks: RefCell::new(vec![]),
            completed_error_chunks: RefCell::new(vec![]),
            written_symbols: js::map::new(None),
            written_client_references: js::map::new(None),
            written_server_references: js::map::new(None),
            written_objects: js::weak_map::new(),
            written_import_strings: js::map::new(None),
            written_import_strings_size: Cell::new(0.0),
            temporary_references,
            identifier_prefix: identifier_prefix.unwrap_or_default().to_owned(),
            identifier_count: Cell::new(1.0),
            taint_cleanup_queue: cleanup_queue,
            on_error: on_error.unwrap_or_else(|| Rc::new(default_error_handler)),
            on_all_ready,
            on_fatal_error,

            time_origin: Cell::new(0.0),
            abort_time: Cell::new(0.0),

            pending_debug_chunks: Cell::new(0.0),
            completed_debug_chunks: RefCell::new(vec![]),
            debug_destination: Cell::new(None),
            environment_name: match environment_name {
                None => Rc::new(|| "Server".to_owned()),
                Some(EnvironmentName::Str(environment_name)) => {
                    Rc::new(move || environment_name.clone())
                }
                Some(EnvironmentName::Fn(environment_name)) => environment_name,
            },
            filter_stack_frame: filter_stack_frame
                .unwrap_or_else(|| Rc::new(default_filter_stack_frame)),
            did_warn_for_key: Cell::new(None),
            written_debug_objects: js::weak_map::new(),
            deferred_debug_objects: if keep_debug_alive {
                Some(DeferredDebugStore {
                    retained: js::map::new(None),
                    existing: js::map::new(None),
                })
            } else {
                None
            },
        });

        let time_origin;
        if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
            // We start by serializing the time origin. Any future timestamps will be
            // emitted relatively to this origin. Instead of using performance.timeOrigin
            // as this origin, we use the timestamp at the start of the request.
            // This avoids leaking unnecessary information like how long the server has
            // been running and allows for more compact representation of each timestamp.
            // The time origin is stored as an offset in the time space of this environment.
            if let Some(debug_start_time) = debug_start_time {
                // We expect `startTime` to be an absolute timestamp, so relativize it to match the other case.
                time_origin = debug_start_time -
                    // $FlowFixMe[prop-missing]
                    performance.time_origin();
            } else {
                time_origin = performance.now();
            }
            request.time_origin.set(time_origin);
            emit_time_origin_chunk(
                &request,
                time_origin +
                    // $FlowFixMe[prop-missing]
                    performance.time_origin(),
            );
            request.abort_time.set(-0.0);
        } else {
            time_origin = 0.0;
        }

        let root_task = create_task(
            &request,
            model,
            None,
            false,
            create_root_format_context(),
            &abort_set,
            time_origin,
            None,
            None,
            None,
        );
        request.pinged_tasks.borrow_mut().push(root_task);
        request
    }
}

#[cfg_attr(rust_js, rust_js::nullable(bundler_config))]
pub fn create_request(
    model: Option<&'static ReactClientValue>,
    bundler_config: Option<&'static ClientManifest>,
    on_error: Option<Rc<OnError>>,
    identifier_prefix: Option<&str>,
    temporary_references: Option<&'static TemporaryReferenceSet>,
    debug_start_time: Option<f64>,                    // Profiling-only
    environment_name: Option<EnvironmentName>,        // DEV-only
    filter_stack_frame: Option<Rc<FilterStackFrame>>, // DEV-only
    keep_debug_alive: bool,                           // DEV-only
) -> Rc<Request> {
    if __DEV__ {
        reset_owner_stack_limit();
    }

    // $FlowFixMe[invalid-constructor]: the shapes are exact here but Flow doesn't like constructors
    Request::new(
        RENDER,
        model,
        bundler_config,
        on_error,
        Rc::new(shared::noop::noop),
        Rc::new(|_| {}),
        identifier_prefix,
        temporary_references,
        debug_start_time,
        environment_name,
        filter_stack_frame,
        keep_debug_alive,
    )
}

#[cfg_attr(rust_js, rust_js::nullable(bundler_config))]
pub fn create_prerender_request(
    model: Option<&'static ReactClientValue>,
    bundler_config: Option<&'static ClientManifest>,
    on_all_ready: Rc<OnAllReady>,
    on_fatal_error: Rc<OnFatalError>,
    on_error: Option<Rc<OnError>>,
    identifier_prefix: Option<&str>,
    temporary_references: Option<&'static TemporaryReferenceSet>,
    debug_start_time: Option<f64>,                    // Profiling-only
    environment_name: Option<EnvironmentName>,        // DEV-only
    filter_stack_frame: Option<Rc<FilterStackFrame>>, // DEV-only
    keep_debug_alive: bool,                           // DEV-only
) -> Rc<Request> {
    if __DEV__ {
        reset_owner_stack_limit();
    }

    // $FlowFixMe[invalid-constructor]: the shapes are exact here but Flow doesn't like constructors
    Request::new(
        PRERENDER,
        model,
        bundler_config,
        on_error,
        on_all_ready,
        on_fatal_error,
        identifier_prefix,
        temporary_references,
        debug_start_time,
        environment_name,
        filter_stack_frame,
        keep_debug_alive,
    )
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::nullable)]
    static CURRENT_REQUEST: RefCell<Option<Rc<Request>>> = const { RefCell::new(None) };
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn resolve_request() -> Option<Rc<Request>> {
    if let Some(current_request) = CURRENT_REQUEST.with_borrow(Clone::clone) {
        return Some(current_request);
    }
    // $FlowFixMe[constant-condition]
    if SUPPORTS_REQUEST_STORAGE {
        let store = REQUEST_STORAGE.get_store().cloned().flatten();
        if let Some(store) = store {
            return Some(store);
        }
    }
    None
}

fn is_typed_array(value: &Unknown) -> bool {
    if js::instance_of(value, js::class::<ArrayBuffer>()) {
        return true;
    }
    if js::instance_of(value, js::class::<Int8Array>()) {
        return true;
    }
    if js::instance_of(value, js::class::<Uint8Array>()) {
        return true;
    }
    if js::instance_of(value, js::class::<Uint8ClampedArray>()) {
        return true;
    }
    if js::instance_of(value, js::class::<Int16Array>()) {
        return true;
    }
    if js::instance_of(value, js::class::<Uint16Array>()) {
        return true;
    }
    if js::instance_of(value, js::class::<Int32Array>()) {
        return true;
    }
    if js::instance_of(value, js::class::<Uint32Array>()) {
        return true;
    }
    if js::instance_of(value, js::class::<Float32Array>()) {
        return true;
    }
    if js::instance_of(value, js::class::<Float64Array>()) {
        return true;
    }
    if js::instance_of(value, js::class::<BigInt64Array>()) {
        return true;
    }
    if js::instance_of(value, js::class::<BigUint64Array>()) {
        return true;
    }
    if js::instance_of(value, js::class::<DataView>()) {
        return true;
    }
    false
}

/// What a `catch` is given: the JS value thrown, or `None` of a panic.
fn thrown_value_of(payload: &(dyn std::any::Any + Send)) -> Option<&'static Unknown> {
    payload
        .downcast_ref::<&'static js::JsError>()
        .and_then(|error| error.as_unknown())
}

fn is_suspense_exception(thrown_value: Option<&'static Unknown>) -> bool {
    SUSPENSE_EXCEPTION.with(|&suspense_exception| {
        js::strict_equals(thrown_value, Some(js::unknown(suspense_exception)))
    })
}

fn serialize_debug_thenable(
    request: &Rc<Request>,
    counter: &Rc<Counter>,
    thenable: &'static Thenable,
) -> String {
    // Like serializeThenable but for renderDebugModel
    request
        .pending_debug_chunks
        .set(request.pending_debug_chunks.get() + 1.0);
    let id = request.next_chunk_id.get();
    request.next_chunk_id.set(id + 1.0);
    let r#ref = serialize_promise_id(id);
    // SAFETY: a thenable is an object.
    request.written_debug_objects.set(
        unsafe { js::cast(Some(js::unknown(thenable))) },
        r#ref.clone(),
    );

    match thenable.status().as_deref() {
        Some("fulfilled") => {
            emit_outlined_debug_model_chunk(request, id, counter, thenable.value());
            return r#ref;
        }
        Some("rejected") => {
            let x = thenable.reason();
            // We don't log these errors since they didn't actually throw into Flight.
            let digest = "";
            emit_error_chunk(request, id, digest, x, true, None);
            return r#ref;
        }
        _ => {}
    }

    if request.status.get() == ABORTING {
        // Ensure that we have time to emit the halt chunk if we're sync aborting.
        emit_debug_halt_chunk(request, id);
        return r#ref;
    }

    if let Some(deferred_debug_objects) = &request.deferred_debug_objects {
        // For Promises that are not yet resolved, we always defer them. They are async anyway so it's
        // safe to defer them. This also ensures that we don't eagerly call .then() on a Promise that
        // otherwise wouldn't have initialized. It also ensures that we don't "handle" a rejection
        // that otherwise would have triggered unhandled rejection.
        deferred_debug_objects
            .retained
            .set(&id, js::unknown(thenable));
        let deferred_ref = "$Y@".to_owned() + &js::number::to_string(id, Some(16));
        // We can now refer to the deferred object in the future.
        // SAFETY: a thenable is an object.
        request.written_debug_objects.set(
            unsafe { js::cast(Some(js::unknown(thenable))) },
            deferred_ref.clone(),
        );
        return deferred_ref;
    }

    let cancelled = Rc::new(Cell::new(false));

    thenable.then(
        {
            let (request, counter, cancelled) = (
                Rc::clone(request),
                Rc::clone(counter),
                Rc::clone(&cancelled),
            );
            move |value| {
                if cancelled.get() {
                    return;
                }
                cancelled.set(true);
                if request.status.get() == ABORTING {
                    emit_debug_halt_chunk(&request, id);
                    enqueue_flush(&request);
                    return;
                }
                if matches!(js::classify(value), Kind::Array(items) if items.len() > 200)
                    || value.is_some_and(|value| {
                        is_typed_array(value)
                            // SAFETY: an ArrayBuffer's or a view's byteLength is a number.
                            && unsafe { js::cast::<f64>(js::get(value, "byteLength")) } > 1000.0
                    })
                {
                    // If this should be deferred, but we don't have a debug channel installed
                    // it would get omitted. We can't omit outlined models but we can avoid
                    // resolving the Promise at all by halting it.
                    emit_debug_halt_chunk(&request, id);
                    enqueue_flush(&request);
                    return;
                }
                emit_outlined_debug_model_chunk(&request, id, &counter, value);
                enqueue_flush(&request);
            }
        },
        {
            let (request, cancelled) = (Rc::clone(request), Rc::clone(&cancelled));
            move |reason| {
                if cancelled.get() {
                    return;
                }
                cancelled.set(true);
                if request.status.get() == ABORTING {
                    emit_debug_halt_chunk(&request, id);
                    enqueue_flush(&request);
                    return;
                }
                // We don't log these errors since they didn't actually throw into Flight.
                let digest = "";
                emit_error_chunk(&request, id, digest, reason, true, None);
                enqueue_flush(&request);
            }
        },
    );

    // We don't use scheduleMicrotask here because it doesn't actually schedule a microtask
    // in all our configs which is annoying.
    let request = Rc::clone(request);
    let _: Promise<()> = js::promise::resolve(()).then(move |()| {
        // If we don't resolve the Promise within a microtask. Leave it as hanging since we
        // don't want to block the render forever on a Promise that might never resolve.
        if cancelled.get() {
            return;
        }
        cancelled.set(true);
        emit_debug_halt_chunk(&request, id);
        enqueue_flush(&request);
        // Clean up the request so we don't leak this forever.
        // (This closure's own request is dropped as it returns.)
    });

    r#ref
}

fn emit_requested_debug_thenable(
    request: &Rc<Request>,
    id: f64,
    counter: &Rc<Counter>,
    thenable: &'static Thenable,
) {
    thenable.then(
        {
            let (request, counter) = (Rc::clone(request), Rc::clone(counter));
            move |value| {
                if request.status.get() == ABORTING {
                    emit_debug_halt_chunk(&request, id);
                    enqueue_flush(&request);
                    return;
                }
                emit_outlined_debug_model_chunk(&request, id, &counter, value);
                enqueue_flush(&request);
            }
        },
        {
            let request = Rc::clone(request);
            move |reason| {
                if request.status.get() == ABORTING {
                    emit_debug_halt_chunk(&request, id);
                    enqueue_flush(&request);
                    return;
                }
                // We don't log these errors since they didn't actually throw into Flight.
                let digest = "";
                emit_error_chunk(&request, id, digest, reason, true, None);
                enqueue_flush(&request);
            }
        },
    );
}

fn create_thenable_task(
    request: &Rc<Request>,
    task: &Rc<Task>,
    thenable: &'static Thenable,
) -> Rc<Task> {
    let key_path = task.key_path.borrow().clone();
    create_task(
        request,
        Some(js::unknown(thenable)), // will be replaced by the value before we retry. used for debug info.
        key_path, // the server component sequence continues through Promise-as-a-child.
        task.implicit_slot.get(),
        task.format_context.get(),
        &request.abortable_tasks,
        if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
            task.time.get()
        } else {
            0.0
        },
        if __DEV__ {
            task.debug_owner.get()
        } else {
            js::null()
        },
        if __DEV__ {
            task.debug_stack.get()
        } else {
            js::null()
        },
        if __DEV__ {
            task.debug_task.get()
        } else {
            js::null()
        },
    )
}

fn serialize_thenable(request: &Rc<Request>, task: &Rc<Task>, thenable: &'static Thenable) -> f64 {
    match thenable.status().as_deref() {
        Some("fulfilled") => {
            let new_task = create_thenable_task(request, task, thenable);
            forward_debug_info_from_thenable(request, &new_task, thenable, None, None);
            // We have the resolved value, we can go ahead and schedule it for serialization.
            new_task.model.set(thenable.value());
            ping_task(request, &new_task);
            new_task.id
        }
        Some("rejected") => {
            let new_task = create_thenable_task(request, task, thenable);
            forward_debug_info_from_thenable(request, &new_task, thenable, None, None);
            let x = thenable.reason();
            errored_task(request, &new_task, x);
            new_task.id
        }
        Some("pending_weak") if ENABLE_FLIGHT_WEAK_THENABLES.with(|&enabled| enabled) => {
            // A weak-pending thenable doesn't block the stream from closing, so
            // we don't create a task for it yet. We only reserve an id for its
            // reference. If it settles while the stream is still open, we
            // create the task at that point, the same as if we had serialized
            // an already settled thenable.
            //
            // Delivery is driven by the thenable's notification. If the stream
            // closes before the listeners are notified, the value is dropped
            // and the reference is left unfulfilled. Since the stream may close
            // synchronously when the last task completes, a thenable that
            // notifies its listeners synchronously (unlike a native Promise,
            // which notifies in a microtask) is guaranteed delivery of any
            // value it settles with before the stream closes.
            let id = request.next_chunk_id.get();
            request.next_chunk_id.set(id + 1.0);
            // The parent task is mutated as serialization continues, so we
            // snapshot the context that the new task needs if it's created
            // later.
            let key_path = task.key_path.borrow().clone();
            let implicit_slot = task.implicit_slot.get();
            let format_context = task.format_context.get();
            let last_timestamp = if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
                task.time.get()
            } else {
                0.0
            };
            let debug_owner = if __DEV__ {
                task.debug_owner.get()
            } else {
                js::null()
            };
            let debug_stack = if __DEV__ {
                task.debug_stack.get()
            } else {
                js::null()
            };
            let debug_task = if __DEV__ {
                task.debug_task.get()
            } else {
                js::null()
            };
            let settled = Rc::new(Cell::new(false));
            thenable.then(
                {
                    let (request, settled, key_path) =
                        (Rc::clone(request), Rc::clone(&settled), key_path.clone());
                    move |value| {
                        if settled.get() || request.status.get() > OPEN {
                            // Too late. The stream already closed (or the request was
                            // aborted), so the reference stays unfulfilled.
                            return;
                        }
                        settled.set(true);
                        let new_task = create_task_with_id(
                            &request,
                            id,
                            value,
                            key_path,
                            implicit_slot,
                            format_context,
                            &request.abortable_tasks,
                            last_timestamp,
                            debug_owner,
                            debug_stack,
                            debug_task,
                        );
                        forward_debug_info_from_current_context(&request, &new_task, thenable);
                        ping_task(&request, &new_task);
                    }
                },
                {
                    let request = Rc::clone(request);
                    move |reason| {
                        if settled.get() || request.status.get() > OPEN {
                            return;
                        }
                        settled.set(true);
                        let new_task = create_task_with_id(
                            &request,
                            id,
                            Some(js::unknown(thenable)), // never rendered. used for debug info.
                            key_path,
                            implicit_slot,
                            format_context,
                            &request.abortable_tasks,
                            last_timestamp,
                            debug_owner,
                            debug_stack,
                            debug_task,
                        );
                        if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
                            // If this is async we need to time when this task finishes.
                            new_task.timed.set(true);
                        }
                        errored_task(&request, &new_task, reason);
                        enqueue_flush(&request);
                    }
                },
            );
            id
        }
        // Fallthrough
        _ => {
            let new_task = create_thenable_task(request, task, thenable);
            if request.status.get() == ABORTING {
                // We can no longer accept any resolved values
                request.abortable_tasks.borrow_mut().remove(&new_task);
                if request.r#type == PRERENDER {
                    halt_task(&new_task, request);
                    finish_halted_task(&new_task, request);
                } else {
                    // SAFETY: an aborting request's fatalError is the id of its abort's error.
                    let error_id: f64 = unsafe { js::cast(request.fatal_error.get()) };
                    abort_task(&new_task, request, error_id);
                    finish_aborted_task(&new_task, request, error_id);
                }
                return new_task.id;
            }
            if !matches!(js::classify(js::get(thenable, "status")), Kind::String(_)) {
                // Only instrument the thenable if the status if not defined. If
                // it's defined, but an unknown value, assume it's been instrumented by
                // some custom userspace implementation. We treat it as "pending".
                let pending_thenable: &PendingThenable = thenable;
                pending_thenable.set_status("pending");
                pending_thenable.then(
                    move |fulfilled_value| {
                        if thenable.status().as_deref() == Some("pending") {
                            let fulfilled_thenable: &FulfilledThenable = thenable;
                            fulfilled_thenable.set_status("fulfilled");
                            fulfilled_thenable.set_value(fulfilled_value);
                        }
                    },
                    move |error| {
                        if thenable.status().as_deref() == Some("pending") {
                            let rejected_thenable: &Thenable = thenable;
                            rejected_thenable.set_status("rejected");
                            rejected_thenable.set_reason(error);
                        }
                    },
                );
            }
            thenable.then(
                {
                    let (request, new_task) = (Rc::clone(request), Rc::clone(&new_task));
                    move |value| {
                        forward_debug_info_from_current_context(&request, &new_task, thenable);
                        new_task.model.set(value);
                        ping_task(&request, &new_task);
                    }
                },
                {
                    let (request, new_task) = (Rc::clone(request), Rc::clone(&new_task));
                    move |reason| {
                        if new_task.status.get() == PENDING {
                            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
                                // If this is async we need to time when this task finishes.
                                new_task.timed.set(true);
                            }
                            // We expect that the only status it might be otherwise is ABORTED.
                            // When we abort we emit chunks in each pending task slot and don't need
                            // to do so again here.
                            errored_task(&request, &new_task, reason);
                            enqueue_flush(&request);
                        }
                    }
                },
            );
            new_task.id
        }
    }
}

/// What a stream's or an async iterable's row is told of its source's
/// error, any value.
type StreamError = Rc<dyn Fn(Option<&'static Unknown>)>;

/// A stream's or an async iterable's listener of its request's abort.
type AbortListener = &'static dyn Fn(&'static Event);

fn serialize_readable_stream(
    request: &Rc<Request>,
    task: &Rc<Task>,
    stream: &'static ReadableStream,
) -> String {
    // Detect if this is a BYOB stream. BYOB streams should be able to be read as bytes on the
    // receiving side. It also implies that different chunks can be split up or merged as opposed
    // to a readable stream that happens to have Uint8Array as the type which might expect it to be
    // received in the same slices.
    // $FlowFixMe[prop-missing]: This is a Node.js extension.
    let supports_byob = js::get(stream, "supportsBYOB");
    // At this point supportsBYOB is guaranteed to be a boolean.
    let is_byte_stream: bool = if matches!(js::classify(supports_byob), Kind::Undefined) {
        // $FlowFixMe[extra-arg]: This argument is accepted.
        stream
            .get_reader_with_options(ReadableStreamGetByobReaderOptions { mode: Byob })
            .map(|reader| reader.release_lock())
            .is_ok()
    } else {
        js::truthy(supports_byob)
    };

    let reader = stream.get_reader();

    // This task won't actually be retried. We just use it to attempt synchronous renders.
    let key_path = task.key_path.borrow().clone();
    let stream_task = create_task(
        request,
        task.model.get(),
        key_path,
        task.implicit_slot.get(),
        task.format_context.get(),
        &request.abortable_tasks,
        if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
            task.time.get()
        } else {
            0.0
        },
        if __DEV__ {
            task.debug_owner.get()
        } else {
            js::null()
        },
        if __DEV__ {
            task.debug_stack.get()
        } else {
            js::null()
        },
        if __DEV__ {
            task.debug_task.get()
        } else {
            js::null()
        },
    );

    // The task represents the Stop row. This adds a Start row.
    request
        .pending_chunks
        .set(request.pending_chunks.get() + 1.0);
    let start_stream_row = js::number::to_string(stream_task.id, Some(16))
        + ":"
        + if is_byte_stream { "r" } else { "R" }
        + "\n";
    request
        .completed_regular_chunks
        .borrow_mut()
        .push(string_to_chunk(&start_stream_row));

    // GAP rd425: upstream's nested `function progress`, `error` and
    // `abortStream` name themselves and each other; each is a closure held in
    // a cell it, or another, reads back.
    type Progress = Rc<dyn Fn(ReadableStreamReadResult)>;
    let progress_cell: Rc<RefCell<Option<Progress>>> = Rc::default();
    let error_cell: Rc<RefCell<Option<StreamError>>> = Rc::default();
    let abort_stream_cell: Rc<Cell<Option<AbortListener>>> = Rc::default();

    let error: StreamError = {
        let (request, stream_task) = (Rc::clone(request), Rc::clone(&stream_task));
        let (error_cell, abort_stream_cell) =
            (Rc::clone(&error_cell), Rc::clone(&abort_stream_cell));
        Rc::new(move |reason| {
            if stream_task.status.get() != PENDING {
                return;
            }
            request.cache_controller.signal().remove_event_listener(
                events::Abort,
                abort_stream_cell.get().unwrap(),
                None,
            );
            errored_task(&request, &stream_task, reason);
            enqueue_flush(&request);

            // $FlowFixMe[incompatible-type] should be able to pass mixed
            // $FlowFixMe[incompatible-use]
            let (on_fulfilled, on_rejected) = (
                error_cell.borrow().clone().unwrap(),
                error_cell.borrow().clone().unwrap(),
            );
            let _: Promise<()> = reader
                .cancel(reason.map(|reason| reason as &dyn js::IntoUnknown))
                .then_with(
                    move |()| on_fulfilled(None),
                    move |error| on_rejected(error.as_unknown()),
                );
        })
    };
    *error_cell.borrow_mut() = Some(Rc::clone(&error));

    let abort_stream: AbortListener = {
        let (request, stream_task) = (Rc::clone(request), Rc::clone(&stream_task));
        let (error, abort_stream_cell) = (Rc::clone(&error), Rc::clone(&abort_stream_cell));
        webapi::listener(move |_: &Event| {
            if stream_task.status.get() != PENDING {
                return;
            }
            let signal = request.cache_controller.signal();
            signal.remove_event_listener(events::Abort, abort_stream_cell.get().unwrap(), None);
            let reason = signal.reason();
            if request.r#type == PRERENDER {
                request.abortable_tasks.borrow_mut().remove(&stream_task);
                halt_task(&stream_task, &request);
                finish_halted_task(&stream_task, &request);
            } else {
                // TODO: Make this use abortTask() instead.
                errored_task(&request, &stream_task, reason);
                enqueue_flush(&request);
            }
            // $FlowFixMe[incompatible-use] should be able to pass mixed
            let (on_fulfilled, on_rejected) = (Rc::clone(&error), Rc::clone(&error));
            let _: Promise<()> = reader
                .cancel(reason.map(|reason| reason as &dyn js::IntoUnknown))
                .then_with(
                    move |()| on_fulfilled(None),
                    move |error| on_rejected(error.as_unknown()),
                );
        })
    };
    abort_stream_cell.set(Some(abort_stream));

    let progress: Progress = {
        let (request, stream_task) = (Rc::clone(request), Rc::clone(&stream_task));
        let (progress_cell, error) = (Rc::clone(&progress_cell), Rc::clone(&error));
        Rc::new(move |entry: ReadableStreamReadResult| {
            if stream_task.status.get() != PENDING {
                return;
            }

            if entry.done.unwrap_or(false) {
                stream_task.status.set(COMPLETED);
                let end_stream_row = js::number::to_string(stream_task.id, Some(16)) + ":C\n";
                request
                    .completed_regular_chunks
                    .borrow_mut()
                    .push(string_to_chunk(&end_stream_row));
                request.abortable_tasks.borrow_mut().remove(&stream_task);
                request.cache_controller.signal().remove_event_listener(
                    events::Abort,
                    abort_stream,
                    None,
                );
                enqueue_flush(&request);
                call_on_all_ready_if_ready(&request);
            } else {
                // GAP (new): a `catch_unwind` closure's capture of an `Option` of
                // one, `entry.value`, is `$some` of it, `$someValue(captured)`
                // where it's read, so the value is read before the `try`.
                let value = entry.value.flatten();
                if let Err(x) = catch_unwind(AssertUnwindSafe(|| {
                    request
                        .pending_chunks
                        .set(request.pending_chunks.get() + 1.0);
                    stream_task.model.set(value);
                    if is_byte_stream {
                        // Chunks of byte streams are always Uint8Array instances.
                        let chunk: &'static Unknown = stream_task.model.get().unwrap();
                        emit_typed_array_chunk(&request, stream_task.id, "b", chunk, false);
                    } else {
                        try_stream_task(&request, &stream_task);
                    }
                    enqueue_flush(&request);
                    let (progress, error) =
                        (progress_cell.borrow().clone().unwrap(), Rc::clone(&error));
                    let _: Promise<()> = reader.read().then_with(
                        move |entry| progress(entry),
                        move |reason| error(reason.as_unknown()),
                    );
                })) {
                    error(thrown_value_of(&*x));
                }
            }
        })
    };
    *progress_cell.borrow_mut() = Some(Rc::clone(&progress));

    request
        .cache_controller
        .signal()
        .add_event_listener(events::Abort, abort_stream, None);
    let _: Promise<()> = reader.read().then_with(
        move |entry| progress(entry),
        move |reason| error(reason.as_unknown()),
    );
    serialize_by_value_id(stream_task.id)
}

fn serialize_async_iterable(
    request: &Rc<Request>,
    task: &Rc<Task>,
    iterable: &'static Unknown,
    iterator: &'static AsyncIterator<Option<&'static Unknown>>,
) -> String {
    // Generators/Iterators are Iterables but they're also their own iterator
    // functions. If that's the case, we treat them as single-shot. Otherwise,
    // we assume that this iterable might be a multi-shot and allow it to be
    // iterated more than once on the client.
    let is_iterator = js::strict_equals(iterable, iterator);

    // This task won't actually be retried. We just use it to attempt synchronous renders.
    let key_path = task.key_path.borrow().clone();
    let stream_task = create_task(
        request,
        task.model.get(),
        key_path,
        task.implicit_slot.get(),
        task.format_context.get(),
        &request.abortable_tasks,
        if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
            task.time.get()
        } else {
            0.0
        },
        if __DEV__ {
            task.debug_owner.get()
        } else {
            js::null()
        },
        if __DEV__ {
            task.debug_stack.get()
        } else {
            js::null()
        },
        if __DEV__ {
            task.debug_task.get()
        } else {
            js::null()
        },
    );

    if __DEV__ {
        // SAFETY: Flight's `_debugInfo` is its debug info, if any.
        let debug_info: Option<&'static ReactDebugInfo> =
            unsafe { js::cast(js::get(iterable, "_debugInfo")) };
        if let Some(debug_info) = debug_info {
            forward_debug_info(request, &stream_task, debug_info);
        }
    }

    // The task represents the Stop row. This adds a Start row.
    request
        .pending_chunks
        .set(request.pending_chunks.get() + 1.0);
    let start_stream_row = js::number::to_string(stream_task.id, Some(16))
        + ":"
        + if is_iterator { "x" } else { "X" }
        + "\n";
    request
        .completed_regular_chunks
        .borrow_mut()
        .push(string_to_chunk(&start_stream_row));

    // GAP rd425: upstream's nested `function progress`, `error` and
    // `abortIterable` name themselves and each other; each is a closure held
    // in a cell it, or another, reads back. `callIteratorInDEV` is given
    // `function`s of them, as it takes a `&'static dyn Fn`.
    type Step = Rc<dyn Fn(IteratorResult<Option<&'static Unknown>>)>;
    let progress_cell: Rc<RefCell<Option<Step>>> = Rc::default();
    let abort_iterable_cell: Rc<Cell<Option<AbortListener>>> = Rc::default();

    let abort_iterable: AbortListener = {
        let (request, stream_task) = (Rc::clone(request), Rc::clone(&stream_task));
        let abort_iterable_cell = Rc::clone(&abort_iterable_cell);
        webapi::listener(move |_: &Event| {
            if stream_task.status.get() != PENDING {
                return;
            }
            let signal = request.cache_controller.signal();
            signal.remove_event_listener(events::Abort, abort_iterable_cell.get().unwrap(), None);
            let reason = signal.reason();
            if request.r#type == PRERENDER {
                request.abortable_tasks.borrow_mut().remove(&stream_task);
                halt_task(&stream_task, &request);
                finish_halted_task(&stream_task, &request);
            } else {
                // TODO: Make this use abortTask() instead.
                errored_task(&request, &stream_task, signal.reason());
                enqueue_flush(&request);
            }
            if js::is_function((iterator, AsyncIterator::<Option<&'static Unknown>>::throw)) {
                // TODO: Premature exits should call return() on the iterator if it exists
                // to allow cleanup. See https://tc39.es/ecma262/multipage/control-abstraction-objects.html#table-async-iterator-optional
                // The iterator protocol doesn't necessarily include this but a generator do.
                // $FlowFixMe[prop-missing] should be able to pass mixed
                let _: Promise<()> = iterator
                    .throw(reason.map(|reason| reason as &dyn js::IntoUnknown))
                    .then_with(|_| noop(), |_| noop());
            }
        })
    };
    abort_iterable_cell.set(Some(abort_iterable));

    let error: StreamError = {
        let (request, stream_task) = (Rc::clone(request), Rc::clone(&stream_task));
        Rc::new(move |reason| {
            if stream_task.status.get() != PENDING {
                return;
            }
            request.cache_controller.signal().remove_event_listener(
                events::Abort,
                abort_iterable,
                None,
            );
            errored_task(&request, &stream_task, reason);
            enqueue_flush(&request);
            if js::is_function((iterator, AsyncIterator::<Option<&'static Unknown>>::throw)) {
                // The iterator protocol doesn't necessarily include this but a generator do.
                // $FlowFixMe[prop-missing] should be able to pass mixed
                let _: Promise<()> = iterator
                    .throw(reason.map(|reason| reason as &dyn js::IntoUnknown))
                    .then_with(|_| noop(), |_| noop());
            }
        })
    };

    // What `callIteratorInDEV` is given of `progress` and `error`.
    let dev_progress: &'static IteratorProgress = {
        let progress_cell = Rc::clone(&progress_cell);
        function::with_this(move |_, (entry,): (&'static Unknown,)| {
            let progress = progress_cell.borrow().clone().unwrap();
            // SAFETY: what an async iterator's step fulfils is its result.
            progress(unsafe { js::cast(Some(entry)) })
        })
    };
    let dev_error: &'static IteratorError = {
        let error = Rc::clone(&error);
        function::with_this(move |_, (reason,): (&'static js::JsError,)| error(reason.as_unknown()))
    };
    let next_step = {
        let (progress_cell, error) = (Rc::clone(&progress_cell), Rc::clone(&error));
        move || {
            if __DEV__ {
                CALL_ITERATOR_IN_DEV.with(|call_iterator_in_dev| call_iterator_in_dev.unwrap())(
                    // SAFETY: an async iterator is one.
                    unsafe { js::cast(Some(js::unknown(iterator))) },
                    dev_progress,
                    dev_error,
                );
            } else {
                let (progress, error) =
                    (progress_cell.borrow().clone().unwrap(), Rc::clone(&error));
                let _: Promise<()> = iterator.next().then_with(
                    move |entry| progress(entry),
                    move |reason| error(reason.as_unknown()),
                );
            }
        }
    };
    let next_step = Rc::new(next_step);

    let progress: Step = {
        let (request, stream_task) = (Rc::clone(request), Rc::clone(&stream_task));
        let (error, next_step) = (Rc::clone(&error), Rc::clone(&next_step));
        Rc::new(move |entry: IteratorResult<Option<&'static Unknown>>| {
            if stream_task.status.get() != PENDING {
                return;
            }

            if entry.done.unwrap_or(false) {
                stream_task.status.set(COMPLETED);
                let end_stream_row = if matches!(js::classify(entry.value), Kind::Undefined) {
                    js::number::to_string(stream_task.id, Some(16)) + ":C\n"
                } else {
                    // Unlike streams, the last value may not be undefined. If it's not
                    // we outline it and encode a reference to it in the closing instruction.
                    match catch_unwind(AssertUnwindSafe(|| outline_model(&request, entry.value))) {
                        Ok(chunk_id) => {
                            js::number::to_string(stream_task.id, Some(16))
                                + ":C"
                                + &js::json::stringify(&serialize_by_value_id(chunk_id))
                                + "\n"
                        }
                        Err(x) => {
                            error(thrown_value_of(&*x));
                            return;
                        }
                    }
                };
                request
                    .completed_regular_chunks
                    .borrow_mut()
                    .push(string_to_chunk(&end_stream_row));
                request.abortable_tasks.borrow_mut().remove(&stream_task);
                request.cache_controller.signal().remove_event_listener(
                    events::Abort,
                    abort_iterable,
                    None,
                );
                enqueue_flush(&request);
                call_on_all_ready_if_ready(&request);
            } else if let Err(x) = catch_unwind(AssertUnwindSafe(|| {
                stream_task.model.set(entry.value);
                request
                    .pending_chunks
                    .set(request.pending_chunks.get() + 1.0);
                try_stream_task(&request, &stream_task);
                enqueue_flush(&request);
                next_step();
            })) {
                error(thrown_value_of(&*x));
            }
        })
    };
    *progress_cell.borrow_mut() = Some(progress);

    request
        .cache_controller
        .signal()
        .add_event_listener(events::Abort, abort_iterable, None);
    next_step();
    serialize_by_value_id(stream_task.id)
}

pub fn emit_hint(request: &Rc<Request>, code: &str, model: Option<&'static HintModel>) {
    emit_hint_chunk(request, code, model);
    enqueue_flush(request);
}

pub fn get_hints(request: &Request) -> &'static Hints {
    request.hints
}

pub fn get_cache(request: &Request) -> &'static Cache {
    request.cache
}

fn read_thenable(thenable: &'static Thenable) -> Option<&'static Unknown> {
    match thenable.status().as_deref() {
        Some("fulfilled") => thenable.value(),
        Some("rejected") => js::throw(thenable.reason()),
        _ => js::throw(Some(js::unknown(thenable))),
    }
}

/// A lazy component of a thenable, `{$$typeof, _payload, _init}`, which
/// reads the thenable's value.
struct LazyThenable {
    #[cfg_attr(rust_js, rust_js::name = "$$typeof")]
    r#typeof: SymbolFor<'static>,
    #[cfg_attr(rust_js, rust_js::name = "_payload")]
    payload: &'static Thenable,
    #[cfg_attr(rust_js, rust_js::name = "_init")]
    init: fn(&'static Thenable) -> Option<&'static Unknown>,
}

unsafe impl js::Defined for LazyThenable {}

fn create_lazy_wrapper_around_wakeable(
    request: &Rc<Request>,
    task: &Rc<Task>,
    wakeable: &'static Thenable,
) -> Option<&'static Unknown> {
    // This is a temporary fork of the `use` implementation until we accept
    // promises everywhere.
    let thenable: &'static Thenable = wakeable;
    match thenable.status().as_deref() {
        Some("fulfilled") => {
            forward_debug_info_from_thenable(request, task, thenable, None, None);
            return thenable.value();
        }
        Some("rejected") => {
            forward_debug_info_from_thenable(request, task, thenable, None, None);
        }
        _ => {
            if !matches!(js::classify(js::get(thenable, "status")), Kind::String(_)) {
                let pending_thenable: &PendingThenable = thenable;
                pending_thenable.set_status("pending");
                pending_thenable.then(
                    {
                        let (request, task) = (Rc::clone(request), Rc::clone(task));
                        move |fulfilled_value| {
                            forward_debug_info_from_current_context(&request, &task, thenable);
                            if thenable.status().as_deref() == Some("pending") {
                                let fulfilled_thenable: &FulfilledThenable = thenable;
                                fulfilled_thenable.set_status("fulfilled");
                                fulfilled_thenable.set_value(fulfilled_value);
                            }
                        }
                    },
                    {
                        let (request, task) = (Rc::clone(request), Rc::clone(task));
                        move |error| {
                            forward_debug_info_from_current_context(&request, &task, thenable);
                            if thenable.status().as_deref() == Some("pending") {
                                let rejected_thenable: &Thenable = thenable;
                                rejected_thenable.set_status("rejected");
                                rejected_thenable.set_reason(error);
                            }
                        }
                    },
                );
            }
            // Only instrument the thenable if the status if not defined. If
            // it's defined, but an unknown value, assume it's been instrumented by
            // some custom userspace implementation. We treat it as "pending".
        }
    }
    let lazy_type = LazyThenable {
        r#typeof: REACT_LAZY_TYPE,
        payload: thenable,
        init: read_thenable,
    };
    Some(js::unknown_of(lazy_type))
}

#[cfg_attr(rust_js, rust_js::name = "callWithDebugContextInDEV")]
fn call_with_debug_context_in_dev<A, T>(
    request: &Rc<Request>,
    task: &Rc<Task>,
    callback: impl FnOnce(A) -> T,
    arg: A,
) -> T {
    // We don't have a Server Component instance associated with this callback and
    // the nearest context is likely a Client Component being serialized. We create
    // a fake owner during this callback so we can get the stack trace from it.
    // This also gets sent to the client as the owner for the replaying log.
    let env = task.environment_name.borrow().clone();
    let component_debug_info = ReactComponentInfo {
        name: String::new(),
        env: Some(env),
        key: None,
        owner: Cell::new(unsafe {
            js::cast::<Option<&'static ReactComponentInfo>>(task.debug_owner.get())
        }),
        // $FlowFixMe[cannot-write]
        stack: RefCell::new(task_debug_stack_trace(request, task)),
        props: None,
        // $FlowFixMe[cannot-write]
        debug_stack: Cell::new(
            unsafe { js::cast::<Option<&'static Error>>(task.debug_stack.get()) }
                .map(|debug_stack| -> &'static js::JsError { debug_stack }),
        ),
        // $FlowFixMe[cannot-write]
        debug_task: Cell::new(unsafe {
            js::cast::<Option<&'static ConsoleTask>>(task.debug_task.get())
        }),
        debug_location: Cell::new(None),
    };
    // SAFETY: a component info is an object of its fields.
    let component_debug_info: &'static ReactComponentInfo =
        unsafe { js::cast(Some(js::unknown_of(component_debug_info))) };
    let debug_task = unsafe { js::cast::<Option<&'static ConsoleTask>>(task.debug_task.get()) };
    // We don't need the async component storage context here so we only set the
    // synchronous tracking of owner.
    set_current_owner(Some(component_debug_info));
    let result = catch_unwind(AssertUnwindSafe(|| {
        if let Some(debug_task) = debug_task {
            return debug_task.run(|| callback(arg));
        }
        callback(arg)
    }));
    set_current_owner(None);
    result.unwrap_or_else(|x| resume_unwind(x))
}

fn void_handler() {}

/// A server component's iterable child, made multi-shot: each
/// `[Symbol.iterator]()` iterates the child anew.
struct MultiShotIterable {
    #[cfg_attr(rust_js, rust_js::name(js::symbol::iterator))]
    iterator: Box<dyn Fn() -> Option<&'static Unknown>>,
}

unsafe impl js::Defined for MultiShotIterable {}

/// A server component's async iterable child, made multi-shot: each
/// `[ASYNC_ITERATOR]()` iterates the child anew.
struct MultiShotAsyncIterable {
    #[cfg_attr(rust_js, rust_js::name(ASYNC_ITERATOR))]
    async_iterator: Box<dyn Fn() -> Option<&'static Unknown>>,
}

unsafe impl js::Defined for MultiShotAsyncIterable {}

fn process_server_component_return_value(
    request: &Rc<Request>,
    task: &Rc<Task>,
    component: &'static Unknown,
    result: Option<&'static Unknown>,
) -> Option<&'static Unknown> {
    // A Server Component's return value has a few special properties due to being
    // in the return position of a Component. We convert them here.
    let value = match result {
        Some(value) if js::type_of(result) == "object" && !is_client_reference(value) => value,
        _ => return result,
    };

    if js::type_of(js::get(value, "then")) == "function" {
        // When the return value is in children position we can resolve it immediately,
        // to its value without a wrapper if it's synchronously available.
        // SAFETY: an object with a `then` method is a thenable.
        let thenable: &'static Thenable = unsafe { js::cast(result) };
        if __DEV__ {
            // If the thenable resolves to an element, then it was in a static position,
            // the return value of a Server Component. That doesn't need further validation
            // of keys. The Server Component itself would have had a key.
            thenable.then(
                |resolved_value| {
                    if js::type_of(resolved_value) == "object"
                        && let Some(resolved_value) = resolved_value
                        && SymbolFor::from_value(js::get(resolved_value, "$$typeof"))
                            == Some(REACT_ELEMENT_TYPE)
                    {
                        js::set(js::get(resolved_value, "_store"), "validated", 1.0);
                    }
                },
                |_| void_handler(),
            );
        }
        // TODO: Once we accept Promises as children on the client, we can just return
        // the thenable here.
        return create_lazy_wrapper_around_wakeable(request, task, thenable);
    }

    if __DEV__ && SymbolFor::from_value(js::get(value, "$$typeof")) == Some(REACT_ELEMENT_TYPE) {
        // If the server component renders to an element, then it was in a static position.
        // That doesn't need further validation of keys. The Server Component itself would
        // have had a key.
        js::set(js::get(value, "_store"), "validated", 1.0);
    }

    // Normally we'd serialize an Iterator/AsyncIterator as a single-shot which is not compatible
    // to be rendered as a React Child. However, because we have the function to recreate
    // an iterable from rendering the element again, we can effectively treat it as multi-
    // shot. Therefore we treat this as an Iterable/AsyncIterable, whether it was one or not, by
    // adding a wrapper so that this component effectively renders down to an AsyncIterable.
    let iterator_fn = get_iterator_fn(result);
    if let Some(iterator_fn) = iterator_fn {
        // SAFETY: an iterable's `[Symbol.iterator]` is a function.
        let iterator_fn: &'static dyn Fn() -> Option<&'static Unknown> =
            unsafe { js::cast(Some(iterator_fn)) };
        let iterable_child = value;
        let (request, task) = (Rc::clone(request), Rc::clone(task));
        let multi_shot = MultiShotIterable {
            iterator: Box::new(move || {
                let iterator = function::call(iterator_fn, Some(&iterable_child), ());
                if __DEV__ {
                    // If this was an Iterator but not a GeneratorFunction we warn because
                    // it might have been a mistake. Technically you can make this mistake with
                    // GeneratorFunctions and even single-shot Iterables too but it's extra
                    // tempting to try to return the value from a generator.
                    if js::strict_equals(iterator, iterable_child) {
                        let is_generator_component =
                            // $FlowFixMe[method-unbinding]
                            js::object::to_string(Some(component)).unwrap()
                                == "[object GeneratorFunction]"
                                // $FlowFixMe[method-unbinding]
                                && js::object::to_string(Some(iterable_child)).unwrap()
                                    == "[object Generator]";
                        if !is_generator_component {
                            call_with_debug_context_in_dev(
                                &request,
                                &task,
                                |()| {
                                    console::error((concat!(
                                        "Returning an Iterator from a Server Component is not supported ",
                                        "since it cannot be looped over more than once. "
                                    ),));
                                },
                                (),
                            );
                        }
                    }
                }
                iterator
            }),
        };
        let multi_shot = js::unknown_of(multi_shot);
        if __DEV__ {
            js::set(
                multi_shot,
                "_debugInfo",
                js::get(iterable_child, "_debugInfo"),
            );
        }
        return Some(multi_shot);
    }
    if ASYNC_ITERATOR.with(|&async_iterator| js::type_of(js::get(value, async_iterator)))
        == "function"
        && (!js::is_function(webapi::readable_stream::new)
            || !js::instance_of(value, js::class::<ReadableStream>()))
    {
        let iterable_child = value;
        let (request, task) = (Rc::clone(request), Rc::clone(task));
        let multishot = MultiShotAsyncIterable {
            async_iterator: Box::new(move || {
                // SAFETY: an async iterable's `[ASYNC_ITERATOR]` is a function.
                let get_async_iterator: &'static dyn Fn() -> Option<&'static Unknown> = unsafe {
                    js::cast(
                        ASYNC_ITERATOR
                            .with(|&async_iterator| js::get(iterable_child, async_iterator)),
                    )
                };
                let iterator = function::call(get_async_iterator, Some(&iterable_child), ());
                if __DEV__ {
                    // If this was an AsyncIterator but not an AsyncGeneratorFunction we warn because
                    // it might have been a mistake. Technically you can make this mistake with
                    // AsyncGeneratorFunctions and even single-shot AsyncIterables too but it's extra
                    // tempting to try to return the value from a generator.
                    if js::strict_equals(iterator, iterable_child) {
                        let is_generator_component =
                            // $FlowFixMe[method-unbinding]
                            js::object::to_string(Some(component)).unwrap()
                                == "[object AsyncGeneratorFunction]"
                                // $FlowFixMe[method-unbinding]
                                && js::object::to_string(Some(iterable_child)).unwrap()
                                    == "[object AsyncGenerator]";
                        if !is_generator_component {
                            call_with_debug_context_in_dev(
                                &request,
                                &task,
                                |()| {
                                    console::error((concat!(
                                        "Returning an AsyncIterator from a Server Component is not supported ",
                                        "since it cannot be looped over more than once. "
                                    ),));
                                },
                                (),
                            );
                        }
                    }
                }
                iterator
            }),
        };
        let multishot = js::unknown_of(multishot);
        if __DEV__ {
            js::set(
                multishot,
                "_debugInfo",
                js::get(iterable_child, "_debugInfo"),
            );
        }
        return Some(multishot);
    }
    result
}

fn render_function_component(
    request: &Rc<Request>,
    task: &Rc<Task>,
    key: Option<ReactKey>,
    component: &'static Unknown,
    props: &'static Unknown,
    validated: f64, // DEV-only
) -> Option<&'static ReactJsonValue> {
    // Reset the task's thenable state before continuing, so that if a later
    // component suspends we can reuse the same task object. If the same
    // component suspends again, the thenable state will be restored.
    let prev_thenable_state = task.thenable_state.take();

    // SAFETY: a function component is a function of its props.
    let component_fn: &'static Component = unsafe { js::cast(Some(component)) };

    let result;

    let component_debug_info: Option<&'static ReactComponentInfo>;
    if __DEV__ {
        let debug_info: &'static ReactComponentInfo;
        if !CAN_EMIT_DEBUG_INFO.get() {
            // We don't have a chunk to assign debug info. We need to outline this
            // component to assign it an ID.
            return outline_task(request, task);
        } else if let Some(prev_thenable_state) = &prev_thenable_state {
            // This is a replay and we've already emitted the debug info of this component
            // in the first pass. We skip emitting a duplicate line.
            // As a hack we stashed the previous component debug info on this object in DEV.
            // SAFETY: what ReactFlightHooks stashed, a component info.
            debug_info = unsafe {
                js::cast(js::get(
                    js::unknown(&**prev_thenable_state),
                    "_componentDebugInfo",
                ))
            };
        } else {
            // This is a new component in the same task so we can emit more debug info.
            let component_debug_id = task.id;
            // SAFETY: a name is text, as `|| ''` makes a missing one.
            let component_name: String = unsafe {
                js::cast(js::or(
                    js::or(
                        js::get(component, "displayName"),
                        js::get(component, "name"),
                    ),
                    Some(js::unknown("")),
                ))
            };
            let component_env = (request.environment_name)();
            request
                .pending_chunks
                .set(request.pending_chunks.get() + 1.0);
            let new_debug_info = ReactComponentInfo {
                name: component_name,
                env: Some(component_env.clone()),
                key: key.clone(),
                owner: Cell::new(unsafe {
                    js::cast::<Option<&'static ReactComponentInfo>>(task.debug_owner.get())
                }),
                // $FlowFixMe[cannot-write]
                stack: RefCell::new(task_debug_stack_trace(request, task)),
                // $FlowFixMe[cannot-write]
                // SAFETY: a component's props are an object of them.
                props: Some(unsafe { js::cast(Some(props)) }),
                // $FlowFixMe[cannot-write]
                debug_stack: Cell::new(
                    unsafe { js::cast::<Option<&'static Error>>(task.debug_stack.get()) }
                        .map(|debug_stack| -> &'static js::JsError { debug_stack }),
                ),
                // $FlowFixMe[cannot-write]
                debug_task: Cell::new(unsafe {
                    js::cast::<Option<&'static ConsoleTask>>(task.debug_task.get())
                }),
                debug_location: Cell::new(None),
            };
            // SAFETY: a component info is an object of its fields.
            debug_info = unsafe { js::cast(Some(js::unknown_of(new_debug_info))) };

            // We outline this model eagerly so that we can refer to by reference as an owner.
            // If we had a smarter way to dedupe we might not have to do this if there ends up
            // being no references to this as an owner.

            outline_component_info(request, debug_info);

            // Track when we started rendering this component.
            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
                advance_task_time(request, task, performance.now());
            }

            emit_debug_chunk(request, component_debug_id, js::unknown(debug_info));

            // We've emitted the latest environment for this task so we track that.
            *task.environment_name.borrow_mut() = component_env;

            if validated == 2.0 {
                warn_for_missing_key(request, key.as_ref(), debug_info, unsafe {
                    js::cast::<Option<&'static ConsoleTask>>(task.debug_task.get())
                });
            }
        }
        component_debug_info = Some(debug_info);
        prepare_to_use_hooks_for_component(prev_thenable_state, Some(debug_info));
        let call_component_in_dev =
            CALL_COMPONENT_IN_DEV.with(|call_component_in_dev| call_component_in_dev.unwrap());
        // $FlowFixMe[constant-condition]
        if SUPPORTS_COMPONENT_STORAGE {
            // Run the component in an Async Context that tracks the current owner.
            if let Some(debug_task) =
                unsafe { js::cast::<Option<&'static ConsoleTask>>(task.debug_task.get()) }
            {
                result = debug_task.run(|| {
                    // $FlowFixMe[method-unbinding]
                    COMPONENT_STORAGE.run(debug_info, || {
                        call_component_in_dev(component_fn, Some(props), debug_info)
                    })
                });
            } else {
                result = COMPONENT_STORAGE.run(debug_info, || {
                    call_component_in_dev(component_fn, Some(props), debug_info)
                });
            }
        } else if let Some(debug_task) =
            unsafe { js::cast::<Option<&'static ConsoleTask>>(task.debug_task.get()) }
        {
            result =
                debug_task.run(|| call_component_in_dev(component_fn, Some(props), debug_info));
        } else {
            result = call_component_in_dev(component_fn, Some(props), debug_info);
        }
    } else {
        component_debug_info = None;
        prepare_to_use_hooks_for_component(prev_thenable_state, None);
        // The secondArg is always undefined in Server Components since refs error early.
        let second_arg = None;
        result = component_fn(Some(props), second_arg);
    }

    if request.status.get() == ABORTING {
        if js::type_of(result) == "object"
            // $FlowFixMe[invalid-compare]
            && let Some(value) = result
            && js::type_of(js::get(value, "then")) == "function"
            && !is_client_reference(value)
        {
            // SAFETY: an object with a `then` method is a thenable.
            let thenable: &'static Thenable = unsafe { js::cast(result) };
            thenable.then(|_| void_handler(), |_| void_handler());
        }
        // If we aborted during rendering we should interrupt the render but
        // we don't need to provide an error because the renderer will encode
        // the abort error as the reason.
        // eslint-disable-next-line no-throw-literal
        js::throw(js::null());
    }

    if __DEV__ || ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
        // Forward any debug information for any Promises that we use():ed during the render.
        // We do this at the end so that we don't keep doing this for each retry.
        let tracked_thenables = get_tracked_thenables_after_rendering();
        if let Some(tracked_thenables) = tracked_thenables {
            let stacks: Option<&'static Unknown> = if __DEV__ {
                match js::get(js::unknown(&*tracked_thenables), "_stacks") {
                    Some(stacks) if js::truthy(Some(stacks)) => Some(stacks),
                    _ => {
                        let stacks = js::unknown_of(Vec::<&'static Error>::new());
                        js::set(js::unknown(&*tracked_thenables), "_stacks", stacks);
                        Some(stacks)
                    }
                }
            } else {
                None
            };
            let tracked = tracked_thenables.borrow().clone();
            for (i, &thenable) in tracked.iter().enumerate() {
                // SAFETY: each thenable's stack is an Error, if it has one.
                let stack: Option<&'static Error> = if __DEV__ {
                    unsafe { js::cast(js::get(stacks, &i)) }
                } else {
                    None
                };
                forward_debug_info_from_thenable(
                    request,
                    task,
                    thenable,
                    if __DEV__ { component_debug_info } else { None },
                    stack,
                );
            }
        }
    }

    // Apply special cases.
    let result = process_server_component_return_value(request, task, component, result);

    if __DEV__ {
        // From this point on, the parent is the component we just rendered until we
        // hit another JSX element.
        task.debug_owner.set(match component_debug_info {
            Some(component_debug_info) => Some(js::unknown(component_debug_info)),
            None => js::null(),
        });
        // Unfortunately, we don't have a stack frame for this position. Conceptually
        // it would be the location of the `return` inside component that just rendered.
        task.debug_stack.set(js::null());
        task.debug_task.set(js::null());
    }

    // Track this element's key on the Server Component on the keyPath context..
    let prev_key_path = task.key_path.borrow().clone();
    let prev_implicit_slot = task.implicit_slot.get();
    if let Some(key) = &key {
        // Append the key to the path. Technically a null key should really add the child
        // index. We don't do that to hold the payload small and implementation simple.
        let optimistic_key = ReactKey::Optimistic(REACT_OPTIMISTIC_KEY);
        if *key == optimistic_key || prev_key_path.as_ref() == Some(&optimistic_key) {
            // The optimistic key is viral. It turns the whole key into optimistic if any part is.
            *task.key_path.borrow_mut() = Some(optimistic_key);
        } else {
            // Neither is the optimistic key, the only key of a symbol: each is text.
            *task.key_path.borrow_mut() = Some(match (&prev_key_path, key) {
                (Some(ReactKey::Text(prev_key_path)), ReactKey::Text(key)) => {
                    ReactKey::Text(prev_key_path.clone() + "," + key)
                }
                _ => key.clone(),
            });
        }
    } else if prev_key_path.is_none() {
        // This sequence of Server Components has no keys. This means that it was rendered
        // in a slot that needs to assign an implicit key. Even if children below have
        // explicit keys, they should not be used for the outer most key since it might
        // collide with other slots in that set.
        task.implicit_slot.set(true);
    }
    let json = render_model_destructive(request, task, EMPTY_ROOT.with(|&root| root), "", result);
    *task.key_path.borrow_mut() = prev_key_path;
    task.implicit_slot.set(prev_implicit_slot);
    json
}

fn warn_for_missing_key(
    request: &Rc<Request>,
    _key: Option<&ReactKey>,
    component_debug_info: &'static ReactComponentInfo,
    debug_task: Option<&'static ConsoleTask>,
) {
    if __DEV__ {
        let did_warn_for_key = match request.did_warn_for_key.get() {
            Some(did_warn_for_key) => did_warn_for_key,
            None => {
                let did_warn_for_key = js::weak_set::new();
                request.did_warn_for_key.set(Some(did_warn_for_key));
                did_warn_for_key
            }
        };
        let parent_owner = component_debug_info.owner.get();
        if let Some(parent_owner) = parent_owner {
            // SAFETY: a component info is an object.
            let parent_owner: &'static JsObject =
                unsafe { js::cast(Some(js::unknown(parent_owner))) };
            if did_warn_for_key.has(parent_owner) {
                // We already warned for other children in this parent.
                return;
            }
            did_warn_for_key.add(parent_owner);
        }

        // Call with the server component as the currently rendering component
        // for context.
        let log_key_error = |_: Option<&'static Unknown>, _: Option<&'static Unknown>| {
            console::error((
                concat!(
                    "Each child in a list should have a unique \"key\" prop.",
                    "%s%s See https://react.dev/link/warning-keys for more information."
                ),
                "",
                "",
            ));
            None
        };

        let call_component_in_dev =
            CALL_COMPONENT_IN_DEV.with(|call_component_in_dev| call_component_in_dev.unwrap());
        // $FlowFixMe[constant-condition]
        if SUPPORTS_COMPONENT_STORAGE {
            // Run the component in an Async Context that tracks the current owner.
            if let Some(debug_task) = debug_task {
                debug_task.run(|| {
                    // $FlowFixMe[method-unbinding]
                    COMPONENT_STORAGE.run(component_debug_info, || {
                        call_component_in_dev(&log_key_error, js::null(), component_debug_info)
                    })
                });
            } else {
                COMPONENT_STORAGE.run(component_debug_info, || {
                    call_component_in_dev(&log_key_error, js::null(), component_debug_info)
                });
            }
        } else if let Some(debug_task) = debug_task {
            debug_task
                .run(|| call_component_in_dev(&log_key_error, js::null(), component_debug_info));
        } else {
            call_component_in_dev(&log_key_error, js::null(), component_debug_info);
        }
    }
}

/// A fragment's props, `{children}`.
struct FragmentProps {
    children: &'static Unknown,
}

unsafe impl js::Defined for FragmentProps {}

/// What a keyed fragment of `children` is sent as: `[REACT_ELEMENT_TYPE,
/// REACT_FRAGMENT_TYPE, keyPath, {children}]`, and in DEV its owner, stack
/// and validation too.
fn keyed_fragment(key_path: &ReactKey, children: &'static Unknown) -> &'static Unknown {
    let mut fragment = vec![
        Some(js::unknown(&REACT_ELEMENT_TYPE)),
        Some(js::unknown(&REACT_FRAGMENT_TYPE)),
        Some(js::unknown_of(key_path.clone())),
        Some(js::unknown_of(FragmentProps { children })),
    ];
    if __DEV__ {
        fragment.extend([js::null(), js::null(), Some(js::unknown(&0.0))]);
    }
    js::unknown_of(fragment)
}

fn render_fragment(
    request: &Rc<Request>,
    task: &Rc<Task>,
    children: &'static Unknown,
) -> Option<&'static ReactJsonValue> {
    let mut children = children;
    if __DEV__ && let Kind::Array(items) = js::classify(Some(children)) {
        for &child in items {
            if let Some(child) = child
                && js::type_of(Some(child)) == "object"
                && SymbolFor::from_value(js::get(child, "$$typeof")) == Some(REACT_ELEMENT_TYPE)
            {
                let store = js::get(child, "_store");
                if matches!(js::classify(js::get(child, "key")), Kind::Null)
                    && !js::truthy(js::get(store, "validated"))
                {
                    js::set(store, "validated", 2.0);
                }
            }
        }
    }

    if let Some(key_path) = task.key_path.borrow().as_ref() {
        // We have a Server Component that specifies a key but we're now splitting
        // the tree using a fragment.
        let fragment = keyed_fragment(key_path, children);
        if !task.implicit_slot.get() {
            // If this was keyed inside a set. I.e. the outer Server Component was keyed
            // then we need to handle reorders of the whole set. To do this we need to wrap
            // this array in a keyed Fragment.
            return Some(fragment);
        }
        // If the outer Server Component was implicit but then an inner one had a key
        // we don't actually need to be able to move the whole set around. It'll always be
        // in an implicit slot. The key only exists to be able to reset the state of the
        // children. We could achieve the same effect by passing on the keyPath to the next
        // set of components inside the fragment. This would also allow a keyless fragment
        // reconcile against a single child.
        // Unfortunately because of JSON.stringify, we can't call the recursive loop for
        // each child within this context because we can't return a set with already resolved
        // values. E.g. a string would get double encoded. Returning would pop the context.
        // So instead, we wrap it with an unkeyed fragment and inner keyed fragment.
        return Some(js::unknown_of(vec![Some(fragment)]));
    }
    // Since we're yielding here, that implicitly resets the keyPath context on the
    // way up. Which is what we want since we've consumed it. If this changes to
    // be recursive serialization, we need to reset the keyPath and implicitSlot,
    // before recursing here.
    if __DEV__ {
        // SAFETY: Flight's `_debugInfo` is its debug info, if any.
        let debug_info: Option<&'static ReactDebugInfo> =
            unsafe { js::cast(js::get(children, "_debugInfo")) };
        if let Some(debug_info) = debug_info {
            // If this came from Flight, forward any debug info into this new row.
            if !CAN_EMIT_DEBUG_INFO.get() {
                // We don't have a chunk to assign debug info. We need to outline this
                // component to assign it an ID.
                return outline_task(request, task);
            } else {
                // Forward any debug info we have the first time we see it.
                // We do this after init so that we have received all the debug info
                // from the server by the time we emit it.
                forward_debug_info(request, task, debug_info);
            }
            // Since we're rendering this array again, create a copy that doesn't
            // have the debug info so we avoid outlining or emitting debug info again.
            children = js::unknown_of(js::array::from::<Option<&'static Unknown>>(children));
        }
    }
    Some(children)
}

fn render_async_fragment(
    request: &Rc<Request>,
    task: &Rc<Task>,
    children: &'static Unknown,
    get_async_iterator: &'static Unknown,
) -> Option<&'static ReactJsonValue> {
    if let Some(key_path) = task.key_path.borrow().as_ref() {
        // We have a Server Component that specifies a key but we're now splitting
        // the tree using a fragment.
        let fragment = keyed_fragment(key_path, children);
        if !task.implicit_slot.get() {
            // If this was keyed inside a set. I.e. the outer Server Component was keyed
            // then we need to handle reorders of the whole set. To do this we need to wrap
            // this array in a keyed Fragment.
            return Some(fragment);
        }
        // If the outer Server Component was implicit but then an inner one had a key
        // we don't actually need to be able to move the whole set around. It'll always be
        // in an implicit slot. The key only exists to be able to reset the state of the
        // children. We could achieve the same effect by passing on the keyPath to the next
        // set of components inside the fragment. This would also allow a keyless fragment
        // reconcile against a single child.
        // Unfortunately because of JSON.stringify, we can't call the recursive loop for
        // each child within this context because we can't return a set with already resolved
        // values. E.g. a string would get double encoded. Returning would pop the context.
        // So instead, we wrap it with an unkeyed fragment and inner keyed fragment.
        return Some(js::unknown_of(vec![Some(fragment)]));
    }

    // Since we're yielding here, that implicitly resets the keyPath context on the
    // way up. Which is what we want since we've consumed it. If this changes to
    // be recursive serialization, we need to reset the keyPath and implicitSlot,
    // before recursing here.
    // SAFETY: an async iterable's `[ASYNC_ITERATOR]` is a function, which gives
    // its iterator.
    let get_async_iterator: &'static dyn Fn() -> Option<&'static Unknown> =
        unsafe { js::cast(Some(get_async_iterator)) };
    let async_iterator: &'static AsyncIterator<Option<&'static Unknown>> =
        unsafe { js::cast(function::call(get_async_iterator, Some(&children), ())) };
    Some(js::unknown_of(serialize_async_iterable(
        request,
        task,
        children,
        async_iterator,
    )))
}

fn render_client_element(
    request: &Rc<Request>,
    task: &Rc<Task>,
    r#type: &'static Unknown,
    key: Option<ReactKey>,
    props: &'static Unknown,
    validated: f64, // DEV-only
) -> Option<&'static ReactJsonValue> {
    // We prepend the terminal client element that actually gets serialized with
    // the keys of any Server Components which are not serialized.
    let key_path = task.key_path.borrow().clone();
    let optimistic_key = ReactKey::Optimistic(REACT_OPTIMISTIC_KEY);
    let key = match (key, key_path) {
        (None, key_path) => key_path,
        (Some(key), None) => Some(key),
        (Some(key), Some(key_path)) => {
            if key_path == optimistic_key || key == optimistic_key {
                // Optimistic key is viral and turns the whole key optimistic.
                Some(optimistic_key)
            } else {
                // Neither is the optimistic key, the only key of a symbol: each is text.
                match (key_path, key) {
                    (ReactKey::Text(key_path), ReactKey::Text(key)) => {
                        Some(ReactKey::Text(key_path + "," + &key))
                    }
                    (_, key) => Some(key),
                }
            }
        }
    };
    let mut debug_owner = js::null();
    let mut debug_stack = js::null();
    if __DEV__ {
        debug_owner = task.debug_owner.get();
        if !matches!(js::classify(debug_owner), Kind::Null) {
            // Ensure we outline this owner if it is the first time we see it.
            // So that we can refer to it directly.
            outline_component_info(request, task_debug_owner_info(debug_owner));
        }
        if let Some(filtered) = task_debug_stack_trace(request, task) {
            // Outline the debug stack so that we write to the completedDebugChunks instead.
            let object_limit = filtered.len() as f64 * 2.0 + 1.0;
            let filtered = js::unknown_of(filtered);
            let id = outline_debug_model(
                request,
                &Rc::new(Counter {
                    object_limit: Cell::new(object_limit),
                }),
                Some(filtered),
            );
            // We also store this in the main dedupe set so that it can be referenced by inline React Elements.
            // SAFETY: a stack is an array, an object.
            request.written_objects.set(
                unsafe { js::cast(Some(filtered)) },
                serialize_by_value_id(id),
            );
            debug_stack = Some(filtered);
        }
    }
    let key_value = key
        .clone()
        .map_or(js::null(), |key| Some(js::unknown_of(key)));
    let mut element = vec![
        Some(js::unknown(&REACT_ELEMENT_TYPE)),
        Some(r#type),
        key_value,
        Some(props),
    ];
    if __DEV__ {
        element.extend([debug_owner, debug_stack, Some(js::unknown_of(validated))]);
    }
    let element = js::unknown_of(element);
    if task.implicit_slot.get() && key.is_some() {
        // The root Server Component had no key so it was in an implicit slot.
        // If we had a key lower, it would end up in that slot with an explicit key.
        // We wrap the element in a fragment to give it an implicit key slot with
        // an inner explicit key.
        return Some(js::unknown_of(vec![Some(element)]));
    }
    // Since we're yielding here, that implicitly resets the keyPath context on the
    // way up. Which is what we want since we've consumed it. If this changes to
    // be recursive serialization, we need to reset the keyPath and implicitSlot,
    // before recursing here. We also need to reset it once we render into an array
    // or anything else too which we also get implicitly.
    Some(element)
}

thread_local! {
    // Determines if we're currently rendering at the top level of a task and therefore
    // is safe to emit debug info associated with that task. Otherwise, if we're in
    // a nested context, we need to first outline.
    static CAN_EMIT_DEBUG_INFO: Cell<bool> = const { Cell::new(false) };

    // Approximate string length of the currently serializing row.
    // Used to power outlining heuristics.
    static SERIALIZED_SIZE: Cell<f64> = const { Cell::new(0.0) };
}
const MAX_ROW_SIZE: f64 = 3200.0;

// Bundler metadata repeats the same chunk URLs across every client reference of
// a route, so strings in it at least this long get outlined and deduplicated
// when they repeat. The threshold is bounded away from zero because outlining
// something as short as an export name costs more than copying it.
const MIN_DEDUPLICATED_IMPORT_STRING_LENGTH: f64 = 16.0;

// Tracked strings are retained for the rest of the request, so their combined
// length is capped.
const MAX_DEDUPLICATED_IMPORT_STRINGS_SIZE: f64 = 32768.0;

fn defer_task(request: &Rc<Request>, task: &Rc<Task>) -> Option<&'static ReactJsonValue> {
    // Like outlineTask but instead the item is scheduled to be serialized
    // after its parent in the stream.
    let key_path = task.key_path.borrow().clone();
    let new_task = create_task(
        request,
        task.model.get(), // the currently rendering element
        key_path,         // unlike outlineModel this one carries along context
        task.implicit_slot.get(),
        task.format_context.get(),
        &request.abortable_tasks,
        if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
            task.time.get()
        } else {
            0.0
        },
        if __DEV__ {
            task.debug_owner.get()
        } else {
            js::null()
        },
        if __DEV__ {
            task.debug_stack.get()
        } else {
            js::null()
        },
        if __DEV__ {
            task.debug_task.get()
        } else {
            js::null()
        },
    );

    ping_task(request, &new_task);
    Some(js::unknown_of(serialize_lazy_id(new_task.id)))
}

fn outline_task(request: &Rc<Request>, task: &Rc<Task>) -> Option<&'static ReactJsonValue> {
    let key_path = task.key_path.borrow().clone();
    let new_task = create_task(
        request,
        task.model.get(), // the currently rendering element
        key_path,         // unlike outlineModel this one carries along context
        task.implicit_slot.get(),
        task.format_context.get(),
        &request.abortable_tasks,
        if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
            task.time.get()
        } else {
            0.0
        },
        if __DEV__ {
            task.debug_owner.get()
        } else {
            js::null()
        },
        if __DEV__ {
            task.debug_stack.get()
        } else {
            js::null()
        },
        if __DEV__ {
            task.debug_task.get()
        } else {
            js::null()
        },
    );

    retry_task(request, &new_task);
    if new_task.status.get() == COMPLETED {
        // We completed synchronously so we can refer to this by reference. This
        // makes it behaves the same as prod during deserialization.
        return Some(js::unknown_of(serialize_by_value_id(new_task.id)));
    }
    // This didn't complete synchronously so it wouldn't have even if we didn't
    // outline it, so this would reduce to a lazy reference even in prod.
    Some(js::unknown_of(serialize_lazy_id(new_task.id)))
}

fn outline_halted_task(
    request: &Rc<Request>,
    _task: &Rc<Task>,
    allow_lazy: bool,
) -> Option<&'static ReactJsonValue> {
    // In the future if we track task state for resuming we'll maybe need to
    // construnct an actual task here but since we're never going to retry it
    // we just claim the id and serialize it according to the proper convention
    let task_id = request.next_chunk_id.get();
    request.next_chunk_id.set(task_id + 1.0);
    if allow_lazy {
        // We're halting in a position that can handle a lazy reference
        Some(js::unknown_of(serialize_lazy_id(task_id)))
    } else {
        // We're halting in a position that needs a value reference
        Some(js::unknown_of(serialize_by_value_id(task_id)))
    }
}

fn render_element(
    request: &Rc<Request>,
    task: &Rc<Task>,
    r#type: Option<&'static Unknown>,
    key: Option<ReactKey>,
    r#ref: Option<&'static Unknown>,
    props: &'static Unknown,
    validated: f64, // DEV only
) -> Option<&'static ReactJsonValue> {
    if r#ref.is_some() {
        // When the ref moves to the regular props object this will implicitly
        // throw for functions. We could probably relax it to a DEV warning for other
        // cases.
        // TODO: `ref` is now just a prop when `enableRefAsProp` is on. Should we
        // do what the above comment says?
        panic!("Refs cannot be used in Server Components, nor passed to Client Components.");
    }
    if __DEV__ {
        // SAFETY: an element's props are an object.
        let props_key: &JsObject = unsafe { js::cast(Some(props)) };
        JSX_PROPS_PARENTS.with(|parents| parents.set(props_key, r#type));
        if let Some(children) = js::classify(js::get(props, "children")).object() {
            JSX_CHILDREN_PARENTS.with(|parents| parents.set(children, r#type));
        }
    }
    if let Kind::Function(component) = js::classify(r#type)
        && !is_client_reference(js::unknown(component))
        && !is_opaque_temporary_reference(js::unknown(component))
    {
        // This is a Server Component.
        return render_function_component(
            request,
            task,
            key,
            js::unknown(component),
            props,
            validated,
        );
    } else if SymbolFor::from_value(r#type) == Some(REACT_FRAGMENT_TYPE) && key.is_none() {
        // For key-less fragments, we add a small optimization to avoid serializing
        // it as a wrapper.
        if __DEV__ && validated == 2.0 {
            // Create a fake owner node for the error stack.
            let component_debug_info: &'static ReactComponentInfo = unsafe {
                js::cast(Some(js::unknown_of(ReactComponentInfo {
                    name: "Fragment".to_owned(),
                    env: Some((request.environment_name)()),
                    key: key.clone(),
                    owner: Cell::new(js::cast::<Option<&'static ReactComponentInfo>>(
                        task.debug_owner.get(),
                    )),
                    stack: RefCell::new(task_debug_stack_trace(request, task)),
                    props: Some(js::cast(Some(props))),
                    debug_stack: Cell::new(
                        js::cast::<Option<&'static Error>>(task.debug_stack.get())
                            .map(|debug_stack| &**debug_stack),
                    ),
                    debug_task: Cell::new(js::cast::<Option<&'static ConsoleTask>>(
                        task.debug_task.get(),
                    )),
                    debug_location: Cell::new(None),
                })))
            };
            warn_for_missing_key(request, key.as_ref(), component_debug_info, unsafe {
                js::cast::<Option<&'static ConsoleTask>>(task.debug_task.get())
            });
        }
        let prev_implicit_slot = task.implicit_slot.get();
        if task.key_path.borrow().is_none() {
            task.implicit_slot.set(true);
        }
        let json = render_model_destructive(
            request,
            task,
            EMPTY_ROOT.with(|&empty_root| empty_root),
            "",
            js::get(props, "children"),
        );
        task.implicit_slot.set(prev_implicit_slot);
        return json;
    } else if let Some(object) = js::classify(r#type).object()
        && !is_client_reference(js::unknown(object))
    {
        let r#type = js::unknown(object);
        match SymbolFor::from_value(js::get(r#type, "$$typeof")) {
            Some(REACT_LAZY_TYPE) => {
                let lazy: &LazyComponent = unsafe { js::cast(Some(r#type)) };
                let wrapped_type = if __DEV__ {
                    CALL_LAZY_INIT_IN_DEV
                        .with(|call_lazy_init_in_dev| call_lazy_init_in_dev.unwrap())(
                        lazy
                    )
                } else {
                    let payload = &lazy.payload;
                    let init = lazy.init;
                    init(payload)
                };
                if request.status.get() == ABORTING {
                    // lazy initializers are user code and could abort during render
                    // we don't wan to return any value resolved from the lazy initializer
                    // if it aborts so we interrupt rendering here
                    // eslint-disable-next-line no-throw-literal
                    js::throw(js::null());
                }
                return render_element(request, task, wrapped_type, key, r#ref, props, validated);
            }
            Some(REACT_FORWARD_REF_TYPE) => {
                return render_function_component(
                    request,
                    task,
                    key,
                    js::get(r#type, "render").unwrap(),
                    props,
                    validated,
                );
            }
            Some(REACT_MEMO_TYPE) => {
                return render_element(
                    request,
                    task,
                    js::get(r#type, "type"),
                    key,
                    r#ref,
                    props,
                    validated,
                );
            }
            // This is invalid but we'll let the client determine that it is.
            Some(REACT_ELEMENT_TYPE) if __DEV__ => {
                // Disable the key warning that would happen otherwise because this
                // element gets serialized inside an array. We'll error later anyway.
                let element: &ReactElement = unsafe { js::cast(Some(r#type)) };
                js::dict::set(element.store.get().unwrap(), "validated", 1.0);
            }
            _ => {}
        }
    } else if let Kind::String(type_name) = js::classify(r#type) {
        let parent_format_context = task.format_context.get();
        let new_format_context = get_child_format_context(parent_format_context, type_name, props);
        if !js::strict_equals(parent_format_context, new_format_context)
            && js::get(props, "children").is_some()
        {
            // We've entered a new context. We need to create another Task which has
            // the new context set up since it's not safe to push/pop in the middle of
            // a tree. Additionally this means that any deduping within this tree now
            // assumes the new context even if it's reused outside in a different context.
            // We'll rely on this to dedupe the value later as we discover it again
            // inside the returned element's tree.
            outline_model_with_format_context(
                request,
                js::get(props, "children"),
                new_format_context,
            );
        }
    }
    // For anything else, try it on the client instead.
    // We don't know if the client will support it or not. This might error on the
    // client or error during serialization but the stack will point back to the
    // server.
    render_client_element(
        request,
        task,
        // CONTRACT: renderClientElement's `type` is `any`, `undefined` too, so
        // its parameter wants to be an `Option`.
        unsafe { js::cast(r#type) },
        key,
        props,
        validated,
    )
}

/// What a visit of an async node found: `undefined`, the signal that a
/// suitable aborted node was found and no further aborted nodes need to be;
/// `null`, nothing; or the promise or I/O node to log.
#[derive(Clone)]
#[cfg_attr(rust_js, rust_js::untagged)]
enum Found {
    #[cfg_attr(rust_js, rust_js::undefined)]
    Aborted,
    #[cfg_attr(rust_js, rust_js::null)]
    Nothing,
    #[cfg_attr(rust_js, rust_js::otherwise)]
    Node(Rc<AsyncSequence>),
}

/// What a sequence's visit has seen: each node, with what it found, and
/// each promise's debug info it forwarded.
struct Visited {
    nodes: &'static Map<AsyncSequence, Found>,
    debug_infos: &'static Set<ReactDebugInfo>,
}

fn visit_async_node(
    request: &Rc<Request>,
    task: &Rc<Task>,
    node: &Rc<AsyncSequence>,
    visited: &Visited,
    cut_off: f64,
) -> Found {
    // Collect the previous chain iteratively instead of recursively to avoid
    // stack overflow on deep chains. We process from deepest to shallowest so
    // each node has its previousIONode available.
    let mut chain: Vec<Rc<AsyncSequence>> = vec![];
    let mut current = Some(Rc::clone(node));

    while let Some(node) = &current {
        if visited.nodes.has(node) {
            break;
        }
        chain.push(Rc::clone(node));
        let previous = node.previous.borrow().clone();
        current = previous;
    }

    let mut previous_io_node = match &current {
        Some(current) => visited.nodes.get(current).unwrap_or(Found::Aborted),
        None => Found::Nothing,
    };

    // Process from deepest to shallowest (reverse order).
    for n in chain.iter().rev() {
        // Set it as visited early in case we see the node again before returning.
        visited.nodes.set(n, Found::Nothing);

        let result = visit_async_node_impl(request, task, n, visited, cut_off, previous_io_node);

        if !matches!(result, Found::Nothing) {
            // If we ended up with a value, let's use that value for future visits.
            visited.nodes.set(n, result.clone());
        }

        if matches!(result, Found::Aborted) {
            // Undefined is used as a signal that we found a suitable aborted node
            // and we don't have to find further aborted nodes.
            return Found::Aborted;
        }

        previous_io_node = result;
    }

    previous_io_node
}

/// Whether a promise was named, `typeof promise.displayName === 'string'`.
fn has_display_name(promise: &PromiseWithDebugInfo) -> bool {
    // $FlowFixMe[prop-missing]
    matches!(
        js::classify(js::get(promise, "displayName")),
        Kind::String(_)
    )
}

/// The debug info Flight put on a promise, `promise._debugInfo`, forwarded
/// once for each visit.
fn forward_promise_debug_info(
    request: &Rc<Request>,
    task: &Rc<Task>,
    promise: &PromiseWithDebugInfo,
    visited: &Visited,
) {
    let debug_info = js::get(promise, "_debugInfo");
    if debug_info.is_some() {
        // SAFETY: what Flight puts on a promise is its debug info.
        let debug_info: &'static ReactDebugInfo = unsafe { js::cast(debug_info) };
        if !visited.debug_infos.has(debug_info) {
            visited.debug_infos.add(debug_info);
            forward_debug_info(request, task, debug_info);
        }
    }
}

/// What a task is told it awaited: `{awaited, env, owner, stack}`, its
/// `awaited` the I/O node, deduped by this reference, and its `owner` what
/// it's given, a component's info, `null` or the `undefined` of a task whose
/// element was a production element's clone.
struct AsyncInfo {
    awaited: Rc<AsyncSequence>,
    env: String,
    #[cfg_attr(rust_js, rust_js::optional)]
    owner: Cell<Option<&'static Unknown>>,
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    stack: RefCell<Option<ReactStackTrace>>,
}

unsafe impl js::Defined for AsyncInfo {}

fn visit_async_node_impl(
    request: &Rc<Request>,
    task: &Rc<Task>,
    node: &Rc<AsyncSequence>,
    visited: &Visited,
    cut_off: f64,
    previous_io_node: Found,
) -> Found {
    if node.end.get() >= 0.0 && node.end.get() <= request.time_origin.get() {
        // This was already resolved when we started this render. It must have been either something
        // that's part of a start up sequence or externally cached data. We exclude that information.
        // The technique for debugging the effects of uncached data on the render is to simply uncache it.
        return Found::Nothing;
    }

    // `found` represents the return value of the following switch statement.
    // We can't use multiple `return` statements in the switch statement
    // since that prevents Closure compiler from inlining `visitAsyncImpl`
    // thus doubling the call stack size.
    match node.tag.get() {
        IO_NODE => Found::Node(Rc::clone(node)),
        UNRESOLVED_PROMISE_NODE => previous_io_node,
        PROMISE_NODE => 'promise_node: {
            let awaited = node.awaited.borrow().clone();
            let mut r#match = previous_io_node;
            let promise = node.promise.unwrap().deref();
            if let Some(awaited) = awaited {
                let io_node = visit_async_node(request, task, &awaited, visited, cut_off);
                match io_node {
                    Found::Aborted => {
                        // Undefined is used as a signal that we found a suitable aborted node and we don't have to find
                        // further aborted nodes.
                        break 'promise_node Found::Aborted;
                    }
                    Found::Node(io_node) => {
                        // This Promise was blocked on I/O. That's a signal that this Promise is interesting to log.
                        // We don't log it yet though. We return it to be logged by the point where it's awaited.
                        // The ioNode might be another PromiseNode in the case where none of the AwaitNode had
                        // unfiltered stacks.
                        if io_node.tag.get() == PROMISE_NODE {
                            // If the ioNode was a Promise, then that means we found one in user space since otherwise
                            // we would've returned an IO node. We assume this has the best stack.
                            // Note: This might also be a Promise with a displayName but potentially a worse stack.
                            // We could potentially favor the outer Promise if it has a stack but not the inner.
                            r#match = Found::Node(io_node);
                        } else if node
                            .stack
                            .as_ref()
                            .is_some_and(|stack| has_unfiltered_frame(request, stack))
                            || (promise.is_some_and(has_display_name)
                                && io_node
                                    .stack
                                    .as_ref()
                                    .is_none_or(|stack| !has_unfiltered_frame(request, stack)))
                        {
                            // If this Promise has a stack trace then we favor that over the I/O node since we're
                            // mainly dealing with Promises as the abstraction.
                            // If it has no stack but at least has a displayName and the io doesn't have a better
                            // stack anyway, then also use this Promise instead since at least it has a name.
                            r#match = Found::Node(Rc::clone(node));
                        } else {
                            // If this Promise was created inside only third party code, then try to use
                            // the inner I/O node instead. This could happen if third party calls into first
                            // party to perform some I/O.
                            r#match = Found::Node(io_node);
                        }
                    }
                    Found::Nothing
                        if request.status.get() == ABORTING
                            && node.start.get() < request.abort_time.get()
                            && node.end.get() > request.abort_time.get() =>
                    {
                        // We aborted this render. If this Promise spanned the abort time it was probably the
                        // Promise that was aborted. This won't necessarily have I/O associated with it but
                        // it's a point of interest.
                        if node
                            .stack
                            .as_ref()
                            .is_some_and(|stack| has_unfiltered_frame(request, stack))
                            || promise.is_some_and(has_display_name)
                        {
                            r#match = Found::Node(Rc::clone(node));
                        }
                    }
                    Found::Nothing => {}
                }
            }
            // We need to forward after we visit awaited nodes because what ever I/O we requested that's
            // the thing that generated this node and its virtual children.
            if let Some(promise) = promise {
                forward_promise_debug_info(request, task, promise, visited);
            }
            r#match
        }
        UNRESOLVED_AWAIT_NODE => previous_io_node,
        AWAIT_NODE => 'await_node: {
            let awaited = node.awaited.borrow().clone();
            let mut r#match = previous_io_node;
            if let Some(awaited) = awaited {
                let io_node = visit_async_node(request, task, &awaited, visited, cut_off);
                match io_node {
                    Found::Aborted => {
                        // Undefined is used as a signal that we found a suitable aborted node and we don't have to find
                        // further aborted nodes.
                        break 'await_node Found::Aborted;
                    }
                    Found::Node(io_node) => {
                        let start_time = node.start.get();
                        let end_time = node.end.get();
                        if start_time < cut_off {
                            // We started awaiting this node before we started rendering this sequence.
                            // This means that this particular await was never part of the current sequence.
                            // If we have another await higher up in the chain it might have a more actionable stack
                            // from the perspective of this component. If we end up here from the "previous" path,
                            // then this gets I/O ignored, which is what we want because it means it was likely
                            // just part of a previous component's rendering.
                            r#match = Found::Node(Rc::clone(&io_node));
                            if node
                                .stack
                                .as_ref()
                                .is_some_and(|stack| is_await_in_userspace(request, stack))
                            {
                                // This await happened earlier but it was done in user space. This is the first time
                                // that user space saw the value of the I/O. We know we'll emit the I/O eventually
                                // but if we do it now we can override the promise value of the I/O entry to the
                                // one observed by this await which will be a better value than the internals of
                                // the I/O entry. If it's still alive that is.
                                let promise = awaited.promise.and_then(|promise| promise.deref());
                                if promise.is_some() {
                                    serialize_io_node(request, &io_node, awaited.promise);
                                }
                            }
                        } else if node
                            .stack
                            .as_ref()
                            .is_none_or(|stack| !is_await_in_userspace(request, stack))
                        {
                            // If this await was fully filtered out, then it was inside third party code
                            // such as in an external library. We return the I/O node and try another await.
                            r#match = Found::Node(io_node);
                        } else if request.status.get() == ABORTING
                            && start_time > request.abort_time.get()
                        {
                            // This was awaited after aborting so we skip it.
                        } else {
                            // We found a user space await.

                            // Outline the IO node.
                            // The ioNode is where the I/O was initiated, but after that it could have been
                            // processed through various awaits in the internals of the third party code.
                            // Therefore we don't use the inner most Promise as the conceptual value but the
                            // Promise that was ultimately awaited by the user space await.
                            serialize_io_node(request, &io_node, awaited.promise);

                            // If we ever visit this I/O node again, skip it because we already emitted this
                            // exact entry and we don't need two awaits on the same thing.
                            visited.nodes.set(&io_node, Found::Nothing);

                            // Ensure the owner is already outlined.
                            if let Some(owner) = node.owner {
                                outline_component_info(request, owner);
                            }

                            // We log the environment at the time when the last promise pigned ping which may
                            // be later than what the environment was when we actually started awaiting.
                            let env = (request.environment_name)();
                            advance_task_time(request, task, start_time);
                            // Then emit a reference to us awaiting it in the current task.
                            request
                                .pending_chunks
                                .set(request.pending_chunks.get() + 1.0);
                            emit_debug_chunk(
                                request,
                                task.id,
                                js::unknown_of(AsyncInfo {
                                    awaited: io_node, // This is deduped by this reference.
                                    env,
                                    owner: Cell::new(node.owner.map(js::unknown)),
                                    stack: RefCell::new(
                                        node.stack
                                            .as_ref()
                                            .map(|stack| filter_stack_trace(request, stack)),
                                    ),
                                }),
                            );
                            // Mark the end time of the await. If we're aborting then we don't emit this
                            // to signal that this never resolved inside this render.
                            mark_operation_end_time(request, task, end_time);
                            if request.status.get() == ABORTING {
                                // Undefined is used as a signal that we found a suitable aborted node and we don't have to find
                                // further aborted nodes.
                                r#match = Found::Aborted;
                            }
                        }
                    }
                    Found::Nothing => {}
                }
            }
            // We need to forward after we visit awaited nodes because what ever I/O we requested that's
            // the thing that generated this node and its virtual children.
            let promise = node.promise.unwrap().deref();
            if let Some(promise) = promise {
                forward_promise_debug_info(request, task, promise, visited);
            }
            r#match
        }
        _ => {
            // eslint-disable-next-line react-internal/prod-error-codes
            panic!("Unknown AsyncSequence tag. This is a bug in React.");
        }
    }
}

fn emit_async_sequence(
    request: &Rc<Request>,
    task: &Rc<Task>,
    node: &Rc<AsyncSequence>,
    already_forwarded_debug_info: Option<&'static ReactDebugInfo>,
    owner: Option<&'static ReactComponentInfo>,
    stack: Option<&'static Error>,
) {
    let visited = Visited {
        nodes: js::map::new(None),
        debug_infos: js::set::new(None),
    };
    if __DEV__ && let Some(already_forwarded_debug_info) = already_forwarded_debug_info {
        visited.debug_infos.add(already_forwarded_debug_info);
    }
    let awaited_node = visit_async_node(request, task, node, &visited, task.time.get());
    match awaited_node {
        Found::Aborted => {
            // Undefined is used as a signal that we found an aborted await and that's good enough
            // anything derived from that aborted node might be irrelevant.
        }
        Found::Node(awaited_node) => {
            // Nothing in user space (unfiltered stack) awaited this.
            serialize_io_node(request, &awaited_node, awaited_node.promise);
            request
                .pending_chunks
                .set(request.pending_chunks.get() + 1.0);
            // We log the environment at the time when we ping which may be later than what the
            // environment was when we actually started awaiting.
            let env = (request.environment_name)();
            // If we don't have any thing awaited, the time we started awaiting was internal
            // when we yielded after rendering. The current task time is basically that.
            let debug_info = AsyncInfo {
                awaited: Rc::clone(&awaited_node), // This is deduped by this reference.
                env,
                owner: Cell::new(None),
                stack: RefCell::new(None),
            };
            if __DEV__ {
                if owner.is_none() && stack.is_none() {
                    // We have no location for the await. We can use the JSX callsite of the parent
                    // as the await if this was just passed as a prop.
                    if !matches!(js::classify(task.debug_owner.get()), Kind::Null) {
                        // $FlowFixMe[cannot-write]
                        debug_info.owner.set(task.debug_owner.get());
                    }
                    if let Some(stack) = task_debug_stack_trace(request, task) {
                        // $FlowFixMe[cannot-write]
                        *debug_info.stack.borrow_mut() = Some(stack);
                    }
                } else {
                    if let Some(owner) = owner {
                        // $FlowFixMe[cannot-write]
                        debug_info.owner.set(Some(js::unknown(owner)));
                    }
                    if let Some(stack) = stack {
                        // $FlowFixMe[cannot-write]
                        *debug_info.stack.borrow_mut() =
                            Some(filter_stack_trace(request, &parse_stack_trace(stack, 1)));
                    }
                }
            }
            // We don't have a start time for this await but in case there was no start time emitted
            // we need to include something. TODO: We should maybe ideally track the time when we
            // called .then() but without updating the task.time field since that's used for the cutoff.
            advance_task_time(request, task, task.time.get());
            emit_debug_chunk(request, task.id, js::unknown_of(debug_info));
            // Mark the end time of the await. If we're aborting then we don't emit this
            // to signal that this never resolved inside this render.
            // If we're currently aborting, then this never resolved into user space.
            mark_operation_end_time(request, task, awaited_node.end.get());
        }
        Found::Nothing => {}
    }
}

fn ping_task(request: &Rc<Request>, task: &Rc<Task>) {
    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
        // If this was async we need to emit the time when it completes.
        task.timed.set(true);
    }
    let pinged_tasks = &request.pinged_tasks;
    pinged_tasks.borrow_mut().push(Rc::clone(task));
    if pinged_tasks.borrow().len() == 1 {
        request
            .flush_scheduled
            .set(request.destination.get().is_some());
        let request = Rc::clone(request);
        if request.r#type == PRERENDER || request.status.get() == OPENING {
            schedule_microtask(move || perform_work(&request));
        } else {
            schedule_work(move || perform_work(&request));
        }
    }
}

// The user's decision (rd495, Fizz's): a task's ping is made where it's given
// to `then`, its two closures holding the request and the task as upstream's
// `ping` arrow does; the task object has no `ping` property.
fn ping_of(
    request: &Rc<Request>,
    task: &Rc<Task>,
) -> (
    impl FnOnce(Option<&'static Unknown>) + 'static,
    impl FnOnce(Option<&'static Unknown>) + 'static,
) {
    let (resolve_request, resolve_task) = (Rc::clone(request), Rc::clone(task));
    let (reject_request, reject_task) = (Rc::clone(request), Rc::clone(task));
    (
        move |_| ping_task(&resolve_request, &resolve_task),
        move |_| ping_task(&reject_request, &reject_task),
    )
}

#[cfg_attr(rust_js, rust_js::nullable(debug_owner, debug_stack, debug_task))]
fn create_task(
    request: &Rc<Request>,
    model: Option<&'static ReactClientValue>,
    key_path: Option<ReactKey>,
    implicit_slot: bool,
    format_context: &'static FormatContext,
    abort_set: &AbortSet,
    last_timestamp: f64,                   // Profiling-only
    debug_owner: Option<&'static Unknown>, // DEV-only
    debug_stack: Option<&'static Unknown>, // DEV-only
    debug_task: Option<&'static Unknown>,  // DEV-only
) -> Rc<Task> {
    let id = request.next_chunk_id.get();
    request.next_chunk_id.set(id + 1.0);
    create_task_with_id(
        request,
        id,
        model,
        key_path,
        implicit_slot,
        format_context,
        abort_set,
        last_timestamp,
        debug_owner,
        debug_stack,
        debug_task,
    )
}

#[cfg_attr(rust_js, rust_js::name = "createTaskWithID")]
#[cfg_attr(rust_js, rust_js::nullable(debug_owner, debug_stack, debug_task))]
fn create_task_with_id(
    request: &Rc<Request>,
    id: f64,
    model: Option<&'static ReactClientValue>,
    key_path: Option<ReactKey>,
    implicit_slot: bool,
    format_context: &'static FormatContext,
    abort_set: &AbortSet,
    last_timestamp: f64,                   // Profiling-only
    debug_owner: Option<&'static Unknown>, // DEV-only
    debug_stack: Option<&'static Unknown>, // DEV-only
    debug_task: Option<&'static Unknown>,  // DEV-only
) -> Rc<Task> {
    request
        .pending_chunks
        .set(request.pending_chunks.get() + 1.0);
    if let Some(model) = js::classify(model).object() {
        // If we're about to write this into a new task we can assign it an ID early so that
        // any other references can refer to the value we're about to write.
        if key_path.is_some() || implicit_slot {
            // If we're in some kind of context we can't necessarily reuse this object depending
            // what parent components are used.
        } else {
            request
                .written_objects
                .set(model, serialize_by_value_id(id));
        }
    }
    // GAP rd493: upstream sets the Profiling-only and DEV-only fields only
    // under their flags; a struct's fields are always set.
    let task = Rc::new(Task {
        id,
        status: Cell::new(PENDING),
        model: Cell::new(model),
        key_path: RefCell::new(key_path),
        implicit_slot: Cell::new(implicit_slot),
        format_context: Cell::new(format_context),
        thenable_state: RefCell::new(None),
        timed: Cell::new(false),
        time: Cell::new(last_timestamp),
        environment_name: RefCell::new(if __DEV__ {
            (request.environment_name)()
        } else {
            String::new()
        }),
        debug_owner: Cell::new(debug_owner),
        debug_stack: Cell::new(debug_stack),
        debug_task: Cell::new(debug_task),
    });
    abort_set.borrow_mut().insert(Rc::clone(&task));
    task
}

fn resolve_model(
    request: &Rc<Request>,
    task: &Rc<Task>,
    parent: &'static Unknown,
    parent_property_name: &str,
    value: Option<&'static ReactClientValue>,
) -> Option<&'static ReactJsonValue> {
    // Replicate JSON.stringify's toJSON semantics: if the value has a toJSON
    // method, call it first. In practice this only matters for Date objects whose
    // toJSON calls toISOString. Custom toJSON objects are not supported and will
    // trigger a DEV warning below.
    let mut json_value = value;
    if let Some(object) = js::classify(value).object()
        // Registered objects are opaque, including objects with a toJSON method.
        && !(ENABLE_FLIGHT_OBJECT_REFERENCES.with(|&enabled| enabled)
            && is_server_reference(js::unknown(object)))
        // $FlowFixMe[method-unbinding]
        && let Kind::Function(to_json) = js::classify(js::get(object, "toJSON"))
    {
        // $FlowFixMe[incompatible-use]
        json_value = function::call(to_json, Some(object), (parent_property_name,));
    }

    if __DEV__ {
        // $FlowFixMe[incompatible-use]
        let original_value = js::get(parent, parent_property_name);
        if js::type_of(original_value) == "object"
            && !js::strict_equals(original_value, json_value)
            && !js::instance_of(original_value, js::class::<Date>())
        {
            // Call with the server component as the currently rendering component
            // for context.
            call_with_debug_context_in_dev(
                request,
                task,
                |()| {
                    if js::array_buffer::is_view(original_value) {
                        // Binary data such as a Node.js Buffer carries a toJSON method, so it
                        // is serialized through that method rather than as binary. A plain
                        // Uint8Array or ArrayBuffer has no toJSON and is serialized as
                        // binary.
                        console::error((
                            concat!(
                                "Binary data with a toJSON method, such as a Node.js Buffer, is ",
                                "serialized through toJSON instead of as binary. Pass a ",
                                "Uint8Array or ArrayBuffer to send binary data.%s"
                            ),
                            describe_object_for_error_message(parent, Some(parent_property_name)),
                        ));
                    } else if object_name(original_value) != "Object" {
                        // SAFETY: a model's parent is an object or an array.
                        let parent_key: &JsObject = unsafe { js::cast(Some(parent)) };
                        let jsx_parent_type = JSX_CHILDREN_PARENTS
                            .with(|parents| parents.get(parent_key))
                            .flatten();
                        if let Kind::String(_) = js::classify(jsx_parent_type) {
                            console::error((
                                "%s objects cannot be rendered as text children. Try formatting it using toString().%s",
                                object_name(original_value),
                                describe_object_for_error_message(
                                    parent,
                                    Some(parent_property_name),
                                ),
                            ));
                        } else {
                            console::error((
                                concat!(
                                    "Only plain objects can be passed to Client Components from Server Components. ",
                                    "%s objects are not supported.%s"
                                ),
                                object_name(original_value),
                                describe_object_for_error_message(
                                    parent,
                                    Some(parent_property_name),
                                ),
                            ));
                        }
                    } else {
                        console::error((
                            concat!(
                                "Only plain objects can be passed to Client Components from Server Components. ",
                                "Objects with toJSON methods are not supported. Convert it manually ",
                                "to a simple value before passing it to props.%s"
                            ),
                            describe_object_for_error_message(parent, Some(parent_property_name)),
                        ));
                    }
                },
                (),
            );
        }
    }

    let rendered = render_model(request, task, parent, parent_property_name, json_value);

    match js::classify(rendered) {
        Kind::Array(items) => {
            let rendered = rendered.unwrap();
            let mut resolved: Vec<Option<&'static ReactJsonValue>> = vec![];
            for (i, item) in items.iter().enumerate() {
                resolved.push(resolve_model(
                    request,
                    task,
                    rendered,
                    &js::concat_text(i as f64),
                    *item,
                ));
            }
            Some(js::unknown_of(resolved))
        }
        Kind::Object(object) => {
            // Use `{}` for fast properties; `__proto__` is handled below because simple
            // assignment would hit Object.prototype's setter instead of creating a key.
            let resolved = js::object::spread(None::<&Unknown>);
            for key in js::keys_in(object) {
                if HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(object, &key)) {
                    let resolved_value = resolve_model(
                        request,
                        task,
                        js::unknown(object),
                        &key,
                        js::get(object, &key),
                    );
                    if key == __PROTO__ {
                        // Match JSON's ordinary data-property semantics for this legacy key.
                        js::object::define_property(
                            resolved,
                            &key,
                            &PropertyDescriptor {
                                value: resolved_value.map(|value| value as &dyn js::IntoUnknown),
                                enumerable: Some(true),
                                writable: Some(true),
                                configurable: Some(true),
                                ..Default::default()
                            },
                        );
                    } else {
                        js::set(resolved, &key, resolved_value);
                    }
                }
            }
            Some(js::unknown(resolved))
        }
        _ => rendered,
    }
}

#[cfg_attr(rust_js, rust_js::name = "serializeByValueID")]
fn serialize_by_value_id(id: f64) -> String {
    "$".to_owned() + &js::number::to_string(id, Some(16))
}

#[cfg_attr(rust_js, rust_js::name = "serializeLazyID")]
fn serialize_lazy_id(id: f64) -> String {
    "$L".to_owned() + &js::number::to_string(id, Some(16))
}

#[cfg_attr(rust_js, rust_js::name = "serializePromiseID")]
fn serialize_promise_id(id: f64) -> String {
    "$@".to_owned() + &js::number::to_string(id, Some(16))
}

#[cfg_attr(rust_js, rust_js::name = "serializeWeakPromiseID")]
fn serialize_weak_promise_id(id: f64) -> String {
    "$w".to_owned() + &js::number::to_string(id, Some(16))
}

#[cfg_attr(rust_js, rust_js::name = "serializeServerReferenceID")]
fn serialize_server_reference_id(id: f64) -> String {
    "$h".to_owned() + &js::number::to_string(id, Some(16))
}

#[cfg_attr(rust_js, rust_js::name = "serializeServerObjectReferenceID")]
fn serialize_server_object_reference_id(id: f64) -> String {
    "$H".to_owned() + &js::number::to_string(id, Some(16))
}

fn serialize_symbol_reference(name: Option<&str>) -> String {
    // A symbol of no description is `$Sundefined`, upstream's `'$S' + name` of
    // `undefined`.
    "$S".to_owned() + name.unwrap_or("undefined")
}

fn serialize_deferred_object(request: &Rc<Request>, value: &'static Unknown) -> String {
    let deferred_debug_objects = &request.deferred_debug_objects;
    if let Some(deferred_debug_objects) = deferred_debug_objects {
        // This client supports a long lived connection. We can assign this object
        // an ID to be lazy loaded later.
        // This keeps the connection alive until we ask for it or release it.
        request
            .pending_debug_chunks
            .set(request.pending_debug_chunks.get() + 1.0);
        let id = request.next_chunk_id.get();
        request.next_chunk_id.set(id + 1.0);
        deferred_debug_objects.existing.set(value, id);
        deferred_debug_objects.retained.set(&id, value);
        return "$Y".to_owned() + &js::number::to_string(id, Some(16));
    }
    "$Y".to_owned()
}

fn serialize_number(number: f64) -> &'static ReactJsonValue {
    if number.is_finite() {
        if number == 0.0 && 1.0 / number == f64::NEG_INFINITY {
            js::unknown("$-0")
        } else {
            js::unknown_of(number)
        }
    } else if number == f64::INFINITY {
        js::unknown("$Infinity")
    } else if number == f64::NEG_INFINITY {
        js::unknown("$-Infinity")
    } else {
        js::unknown("$NaN")
    }
}

fn serialize_undefined() -> String {
    "$undefined".to_owned()
}

fn serialize_date(date: &'static Unknown) -> String {
    // SAFETY: what's serialized as a date is one.
    let date: &Date = unsafe { js::cast(Some(date)) };
    // JSON.stringify automatically calls Date.prototype.toJSON which calls toISOString.
    // We need only tack on a $D prefix.
    // An invalid date's `toJSON()` is `null`.
    "$D".to_owned() + &date.to_json().unwrap_or_else(|| "null".to_owned())
}

#[cfg_attr(rust_js, rust_js::name = "serializeDateFromDateJSON")]
fn serialize_date_from_date_json(date_json: &str) -> String {
    // JSON.stringify automatically calls Date.prototype.toJSON which calls toISOString.
    // We need only tack on a $D prefix.
    "$D".to_owned() + date_json
}

fn serialize_big_int(n: &js::BigInt) -> String {
    "$n".to_owned() + &n.to_string(Some(10))
}

fn serialize_row_header(tag: &str, id: f64) -> String {
    js::number::to_string(id, Some(16)) + ":" + tag
}

fn encode_reference_chunk(_request: &Rc<Request>, id: f64, reference: &str) -> &'static Chunk {
    let json = js::json::stringify(reference);
    let row = js::number::to_string(id, Some(16)) + ":" + &json + "\n";
    string_to_chunk(&row)
}

/// Whether a reference is an element's `type`, `parent[0] ===
/// REACT_ELEMENT_TYPE && parentPropertyName === '1'`.
fn is_element_type(parent: &Unknown, parent_property_name: &str) -> bool {
    SymbolFor::from_value(js::get(parent, &0u32)) == Some(REACT_ELEMENT_TYPE)
        && parent_property_name == "1"
}

fn serialize_client_reference(
    request: &Rc<Request>,
    parent: &'static Unknown,
    parent_property_name: &str,
    client_reference: &'static ClientReference,
) -> String {
    let client_reference_key = get_client_reference_key(client_reference);
    let written_client_references = request.written_client_references;
    let existing_id = written_client_references.get(client_reference_key);
    if let Some(existing_id) = existing_id {
        if is_element_type(parent, parent_property_name) {
            // If we're encoding the "type" of an element, we can refer
            // to that by a lazy reference instead of directly since React
            // knows how to deal with lazy values. This lets us suspend
            // on this component rather than its parent until the code has
            // loaded.
            return serialize_lazy_id(existing_id);
        }
        return serialize_by_value_id(existing_id);
    }
    match catch_unwind(AssertUnwindSafe(|| {
        let client_reference_metadata =
            resolve_client_reference_metadata(request.bundler_config, client_reference);
        // Stringify before claiming a chunk id so a throw can't leave it pending.
        let json = stringify_import_metadata(request, client_reference_metadata, false);
        request
            .pending_chunks
            .set(request.pending_chunks.get() + 1.0);
        let import_id = request.next_chunk_id.get();
        request.next_chunk_id.set(import_id + 1.0);
        emit_import_chunk(request, import_id, &json, false);
        written_client_references.set(client_reference_key, import_id);
        if is_element_type(parent, parent_property_name) {
            // If we're encoding the "type" of an element, we can refer
            // to that by a lazy reference instead of directly since React
            // knows how to deal with lazy values. This lets us suspend
            // on this component rather than its parent until the code has
            // loaded.
            return serialize_lazy_id(import_id);
        }
        serialize_by_value_id(import_id)
    })) {
        Ok(reference) => reference,
        Err(thrown) => {
            let x = thrown_value_of(&*thrown);
            request
                .pending_chunks
                .set(request.pending_chunks.get() + 1.0);
            let error_id = request.next_chunk_id.get();
            request.next_chunk_id.set(error_id + 1.0);
            let digest = log_recoverable_error(request, x, None);
            emit_error_chunk(request, error_id, &digest, x, false, None);
            serialize_by_value_id(error_id)
        }
    }
}

fn serialize_debug_client_reference(
    request: &Rc<Request>,
    parent: &'static Unknown,
    parent_property_name: &str,
    client_reference: &'static ClientReference,
) -> String {
    // Like serializeDebugClientReference but it doesn't dedupe in the regular set
    // and it writes to completedDebugChunk instead of imports.
    let client_reference_key = get_client_reference_key(client_reference);
    let written_client_references = request.written_client_references;
    let existing_id = written_client_references.get(client_reference_key);
    if let Some(existing_id) = existing_id {
        if is_element_type(parent, parent_property_name) {
            // If we're encoding the "type" of an element, we can refer
            // to that by a lazy reference instead of directly since React
            // knows how to deal with lazy values. This lets us suspend
            // on this component rather than its parent until the code has
            // loaded.
            return serialize_lazy_id(existing_id);
        }
        return serialize_by_value_id(existing_id);
    }
    match catch_unwind(AssertUnwindSafe(|| {
        let client_reference_metadata =
            resolve_client_reference_metadata(request.bundler_config, client_reference);
        let json = stringify_import_metadata(request, client_reference_metadata, true);
        request
            .pending_debug_chunks
            .set(request.pending_debug_chunks.get() + 1.0);
        let import_id = request.next_chunk_id.get();
        request.next_chunk_id.set(import_id + 1.0);
        emit_import_chunk(request, import_id, &json, true);
        if is_element_type(parent, parent_property_name) {
            // If we're encoding the "type" of an element, we can refer
            // to that by a lazy reference instead of directly since React
            // knows how to deal with lazy values. This lets us suspend
            // on this component rather than its parent until the code has
            // loaded.
            return serialize_lazy_id(import_id);
        }
        serialize_by_value_id(import_id)
    })) {
        Ok(reference) => reference,
        Err(thrown) => {
            let x = thrown_value_of(&*thrown);
            request
                .pending_debug_chunks
                .set(request.pending_debug_chunks.get() + 1.0);
            let error_id = request.next_chunk_id.get();
            request.next_chunk_id.set(error_id + 1.0);
            let digest = log_recoverable_error(request, x, None);
            emit_error_chunk(request, error_id, &digest, x, true, None);
            serialize_by_value_id(error_id)
        }
    }
}

fn outline_model(request: &Rc<Request>, value: Option<&'static ReactClientValue>) -> f64 {
    outline_model_with_format_context(
        request,
        value,
        // For deduped values we don't know which context it will be reused in
        // so we have to assume that it's the root context.
        create_root_format_context(),
    )
}

fn outline_model_with_format_context(
    request: &Rc<Request>,
    value: Option<&'static ReactClientValue>,
    format_context: &'static FormatContext,
) -> f64 {
    let new_task = create_task(
        request,
        value,
        None,           // The way we use outlining is for reusing an object.
        false,          // It makes no sense for that use case to be contextual.
        format_context, // Except for FormatContext we optimistically use it.
        &request.abortable_tasks,
        if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
            performance.now() // TODO: This should really inherit the time from the task.
        } else {
            0.0
        },
        None, // TODO: Currently we don't associate any debug information with
        None, // this object on the server. If it ends up erroring, it won't
        None, // have any context on the server but can on the client.
    );
    retry_task(request, &new_task);
    new_task.id
}

/// A server object reference's metadata, `{id}`.
struct ServerObjectReferenceMetadata {
    id: &'static ServerReferenceId,
}

unsafe impl js::Defined for ServerObjectReferenceMetadata {}

/// A server function reference's metadata, `{id, bound}`.
struct ServerReferenceMetadata {
    id: &'static ServerReferenceId,
    #[cfg_attr(rust_js, rust_js::nullable)]
    bound: Option<Promise<&'static [Option<&'static ReactClientValue>]>>,
}

unsafe impl js::Defined for ServerReferenceMetadata {}

/// A server function reference's metadata in DEV, `{id, bound, name, env,
/// location}`.
struct ServerReferenceMetadataDev {
    id: &'static ServerReferenceId,
    #[cfg_attr(rust_js, rust_js::nullable)]
    bound: Option<Promise<&'static [Option<&'static ReactClientValue>]>>,
    name: String,                    // DEV-only
    env: String,                     // DEV-only
    location: ReactFunctionLocation, // DEV-only
}

unsafe impl js::Defined for ServerReferenceMetadataDev {}

fn serialize_server_reference(
    request: &Rc<Request>,
    server_reference: &'static ServerReference,
) -> String {
    let is_object_reference = ENABLE_FLIGHT_OBJECT_REFERENCES.with(|&enabled| enabled)
        && js::type_of(Some(server_reference)) == "object";
    let written_server_references = request.written_server_references;
    let existing_id = written_server_references.get(server_reference);
    if let Some(existing_id) = existing_id {
        return if is_object_reference {
            serialize_server_object_reference_id(existing_id)
        } else {
            serialize_server_reference_id(existing_id)
        };
    }

    let id = get_server_reference_id(request.bundler_config, server_reference);
    let server_reference_metadata = if is_object_reference {
        // Objects share the manifest lookup with functions, but have no bound
        // arguments or function debug metadata.
        js::unknown_of(ServerObjectReferenceMetadata { id })
    } else {
        let bound_args =
            get_server_reference_bound_arguments(request.bundler_config, server_reference);
        let bound = bound_args.map(js::promise::resolve);

        let mut location: Option<ReactFunctionLocation> = None;
        if __DEV__ {
            let error = get_server_reference_location(request.bundler_config, server_reference);
            // $FlowFixMe[constant-condition]
            if let Some(error) = error {
                let frames = parse_stack_trace(error, 1);
                if !frames.is_empty() {
                    let first_frame = &frames[0];
                    location = Some((
                        first_frame.0.clone(),
                        first_frame.1.clone(),
                        first_frame.2, // The line and col of the callsite represents the
                        first_frame.3, // enclosing line and col of the function.
                    ));
                }
            }
        }

        if __DEV__ && let Some(location) = location {
            js::unknown_of(ServerReferenceMetadataDev {
                id,
                bound,
                name: if let Kind::Function(server_reference) = js::classify(Some(server_reference))
                {
                    js::function::name(server_reference)
                } else {
                    String::new()
                },
                env: (request.environment_name)(),
                location,
            })
        } else {
            js::unknown_of(ServerReferenceMetadata { id, bound })
        }
    };
    let metadata_id = outline_model(request, Some(server_reference_metadata));
    written_server_references.set(server_reference, metadata_id);
    if is_object_reference {
        serialize_server_object_reference_id(metadata_id)
    } else {
        serialize_server_reference_id(metadata_id)
    }
}

fn serialize_temporary_reference(_request: &Rc<Request>, reference: &str) -> String {
    "$T".to_owned() + reference
}

fn serialize_large_text_string(request: &Rc<Request>, text: &str) -> String {
    request
        .pending_chunks
        .set(request.pending_chunks.get() + 1.0);
    let text_id = request.next_chunk_id.get();
    request.next_chunk_id.set(text_id + 1.0);
    emit_text_chunk(request, text_id, text, false);
    serialize_by_value_id(text_id)
}

fn serialize_debug_large_text_string(request: &Rc<Request>, text: &str) -> String {
    request
        .pending_debug_chunks
        .set(request.pending_debug_chunks.get() + 1.0);
    let text_id = request.next_chunk_id.get();
    request.next_chunk_id.set(text_id + 1.0);
    emit_text_chunk(request, text_id, text, true);
    serialize_by_value_id(text_id)
}

fn serialize_map(request: &Rc<Request>, map: &'static Unknown) -> String {
    let entries: Vec<Option<&'static Unknown>> = js::array::from(map);
    let id = outline_model(request, Some(js::unknown_of(entries)));
    "$Q".to_owned() + &js::number::to_string(id, Some(16))
}

fn serialize_form_data(request: &Rc<Request>, form_data: &'static FormData) -> String {
    let entries = js::array::from(&form_data.entries());
    let id = outline_model(request, Some(js::unknown_of(entries)));
    "$K".to_owned() + &js::number::to_string(id, Some(16))
}

fn serialize_debug_form_data(request: &Rc<Request>, form_data: &'static FormData) -> String {
    let entries = js::array::from(&form_data.entries());
    let id = outline_debug_model(
        request,
        &Rc::new(Counter {
            object_limit: Cell::new(entries.len() as f64 * 2.0 + 1.0),
        }),
        Some(js::unknown_of(entries)),
    );
    "$K".to_owned() + &js::number::to_string(id, Some(16))
}

fn serialize_set(request: &Rc<Request>, set: &'static Unknown) -> String {
    let entries: Vec<Option<&'static Unknown>> = js::array::from(set);
    let id = outline_model(request, Some(js::unknown_of(entries)));
    "$W".to_owned() + &js::number::to_string(id, Some(16))
}

/// The DEV-only set of what's never limited, `doNotLimit`.
fn do_not_limit() -> &'static WeakSet<JsObject> {
    DO_NOT_LIMIT.with(|&do_not_limit| do_not_limit.unwrap())
}

fn serialize_debug_map(
    request: &Rc<Request>,
    counter: &Rc<Counter>,
    map: &'static Unknown,
) -> String {
    // Like serializeMap but for renderDebugModel.
    let entries: Vec<Option<&'static Unknown>> = js::array::from(map);
    // The Map itself doesn't take up any space but the outlined object does.
    counter.object_limit.set(counter.object_limit.get() + 1.0);
    for &entry in &entries {
        // Outline every object entry in case we run out of space to serialize them.
        // Because we can't mark these values as limited.
        // SAFETY: a map's entry is its `[key, value]` array.
        let entry_object: &JsObject = unsafe { js::cast(entry) };
        do_not_limit().add(entry_object);
        let key = js::get(entry, &0u32);
        let value = js::get(entry, &1u32);
        if let Some(key) = js::classify(key).object() {
            do_not_limit().add(key);
        }
        if let Some(value) = js::classify(value).object() {
            do_not_limit().add(value);
        }
    }
    let id = outline_debug_model(request, counter, Some(js::unknown_of(entries)));
    "$Q".to_owned() + &js::number::to_string(id, Some(16))
}

fn serialize_debug_set(
    request: &Rc<Request>,
    counter: &Rc<Counter>,
    set: &'static Unknown,
) -> String {
    // Like serializeMap but for renderDebugModel.
    let entries: Vec<Option<&'static Unknown>> = js::array::from(set);
    // The Set itself doesn't take up any space but the outlined object does.
    counter.object_limit.set(counter.object_limit.get() + 1.0);
    for &entry in &entries {
        // Outline every object entry in case we run out of space to serialize them.
        // Because we can't mark these values as limited.
        if let Some(entry) = js::classify(entry).object() {
            do_not_limit().add(entry);
        }
    }
    let id = outline_debug_model(request, counter, Some(js::unknown_of(entries)));
    "$W".to_owned() + &js::number::to_string(id, Some(16))
}

fn serialize_iterator(request: &Rc<Request>, iterator: &'static Unknown) -> String {
    let entries: Vec<Option<&'static Unknown>> = js::array::from(iterator);
    let id = outline_model(request, Some(js::unknown_of(entries)));
    "$i".to_owned() + &js::number::to_string(id, Some(16))
}

fn serialize_typed_array(
    request: &Rc<Request>,
    tag: &str,
    typed_array: &'static Unknown,
) -> String {
    request
        .pending_chunks
        .set(request.pending_chunks.get() + 1.0);
    let buffer_id = request.next_chunk_id.get();
    request.next_chunk_id.set(buffer_id + 1.0);
    emit_typed_array_chunk(request, buffer_id, tag, typed_array, false);
    serialize_by_value_id(buffer_id)
}

fn serialize_debug_typed_array(
    request: &Rc<Request>,
    tag: &str,
    typed_array: &'static Unknown,
) -> String {
    // SAFETY: binary data, an `ArrayBuffer` or a view, has a `byteLength`.
    let view: &ArrayBufferView = unsafe { js::cast(Some(typed_array)) };
    // SAFETY: binary data is an object.
    let key: &JsObject = unsafe { js::cast(Some(typed_array)) };
    if view.byte_length() > 1000.0 && !do_not_limit().has(key) {
        // Defer large typed arrays.
        return serialize_deferred_object(request, typed_array);
    }
    request
        .pending_debug_chunks
        .set(request.pending_debug_chunks.get() + 1.0);
    let buffer_id = request.next_chunk_id.get();
    request.next_chunk_id.set(buffer_id + 1.0);
    emit_typed_array_chunk(request, buffer_id, tag, typed_array, true);
    serialize_by_value_id(buffer_id)
}

/// A blob's read for the debug channel: upstream's `progress` and `error`
/// over serializeDebugBlob's variables.
struct DebugBlobReading {
    request: Rc<Request>,
    model: Rc<RefCell<Vec<Option<&'static Unknown>>>>,
    reader: &'static ReadableStreamDefaultReader,
    id: f64,
}

impl DebugBlobReading {
    fn progress(self: &Rc<Self>, entry: ReadableStreamReadResult) -> Option<Promise<()>> {
        if js::truthy(entry.done.as_ref()) {
            let length = self.model.borrow().len();
            emit_outlined_debug_model_chunk(
                &self.request,
                self.id,
                &Rc::new(Counter {
                    object_limit: Cell::new(length as f64 + 2.0),
                }),
                Some(js::unknown_of(Rc::clone(&self.model))),
            );
            enqueue_flush(&self.request);
            return None;
        }
        // TODO: Emit the chunk early and refer to it later by dedupe.
        self.model.borrow_mut().push(entry.value.flatten());
        let reading = Rc::clone(self);
        let erroring = Rc::clone(self);
        // $FlowFixMe[incompatible-type]
        Some(
            self.reader
                .read()
                .then(move |entry| reading.progress(entry))
                .catch(move |reason| erroring.error(reason.as_unknown())),
        )
    }

    fn error(&self, reason: Option<&'static Unknown>) {
        let digest = "";
        emit_error_chunk(&self.request, self.id, digest, reason, true, None);
        enqueue_flush(&self.request);
        // $FlowFixMe[incompatible-type] should be able to pass mixed
        let _: Promise<()> = self
            .reader
            .cancel(reason.map(|reason| reason as &dyn js::IntoUnknown))
            .then_with(|_| noop(), |_| noop());
    }
}

fn serialize_debug_blob(request: &Rc<Request>, blob: &'static Blob) -> String {
    let model: Rc<RefCell<Vec<Option<&'static Unknown>>>> =
        Rc::new(RefCell::new(vec![Some(js::unknown_of(blob.r#type()))]));
    let reader = blob.stream().get_reader();
    request
        .pending_debug_chunks
        .set(request.pending_debug_chunks.get() + 1.0);
    let id = request.next_chunk_id.get();
    request.next_chunk_id.set(id + 1.0);
    let reading = Rc::new(DebugBlobReading {
        request: Rc::clone(request),
        model,
        reader,
        id,
    });
    let progressing = Rc::clone(&reading);
    let erroring = Rc::clone(&reading);
    // $FlowFixMe[incompatible-type]
    let _: Promise<()> = reader
        .read()
        .then(move |entry| progressing.progress(entry))
        .catch(move |reason| erroring.error(reason.as_unknown()));
    "$B".to_owned() + &js::number::to_string(id, Some(16))
}

/// A blob's read into its task's model: upstream's `progress`, `error` and
/// `abortBlob` over serializeBlob's variables.
struct BlobReading {
    request: Rc<Request>,
    new_task: Rc<Task>,
    model: Rc<RefCell<Vec<Option<&'static Unknown>>>>,
    reader: &'static ReadableStreamDefaultReader,
    // The `abort` listener, which removes itself: set once it's made.
    abort_blob: OnceCell<&'static dyn Fn(&'static Event)>,
}

impl BlobReading {
    fn progress(self: &Rc<Self>, entry: ReadableStreamReadResult) -> Option<Promise<()>> {
        if self.new_task.status.get() != PENDING {
            return None;
        }
        if js::truthy(entry.done.as_ref()) {
            self.request
                .cache_controller
                .signal()
                .remove_event_listener(events::Abort, *self.abort_blob.get().unwrap(), None);
            ping_task(&self.request, &self.new_task);
            return None;
        }
        // TODO: Emit the chunk early and refer to it later by dedupe.
        self.model.borrow_mut().push(entry.value.flatten());
        let reading = Rc::clone(self);
        let erroring = Rc::clone(self);
        // $FlowFixMe[incompatible-type]
        Some(
            self.reader
                .read()
                .then(move |entry| reading.progress(entry))
                .catch(move |reason| erroring.error(reason.as_unknown())),
        )
    }

    fn error(self: &Rc<Self>, reason: Option<&'static Unknown>) {
        if self.new_task.status.get() != PENDING {
            return;
        }
        self.request
            .cache_controller
            .signal()
            .remove_event_listener(events::Abort, *self.abort_blob.get().unwrap(), None);
        errored_task(&self.request, &self.new_task, reason);
        enqueue_flush(&self.request);
        // $FlowFixMe[incompatible-type] should be able to pass mixed
        // $FlowFixMe[incompatible-use]
        self.cancel(reason);
    }

    /// `reader.cancel(reason).then(error, error)`.
    fn cancel(self: &Rc<Self>, reason: Option<&'static Unknown>) {
        let (resolving, rejecting) = (Rc::clone(self), Rc::clone(self));
        let _: Promise<()> = self
            .reader
            .cancel(reason.map(|reason| reason as &dyn js::IntoUnknown))
            .then_with(
                move |()| resolving.error(None),
                move |reason| rejecting.error(reason.as_unknown()),
            );
    }

    fn abort_blob(self: &Rc<Self>) {
        if self.new_task.status.get() != PENDING {
            return;
        }
        let signal = self.request.cache_controller.signal();
        signal.remove_event_listener(events::Abort, *self.abort_blob.get().unwrap(), None);
        let reason = signal.reason();
        if self.request.r#type == PRERENDER {
            self.request
                .abortable_tasks
                .borrow_mut()
                .remove(&self.new_task);
            halt_task(&self.new_task, &self.request);
            finish_halted_task(&self.new_task, &self.request);
        } else {
            // TODO: Make this use abortTask() instead.
            errored_task(&self.request, &self.new_task, reason);
            enqueue_flush(&self.request);
        }
        // $FlowFixMe[incompatible-use] should be able to pass mixed
        self.cancel(reason);
    }
}

fn serialize_blob(request: &Rc<Request>, blob: &'static Blob) -> String {
    let model: Rc<RefCell<Vec<Option<&'static Unknown>>>> =
        Rc::new(RefCell::new(vec![Some(js::unknown_of(blob.r#type()))]));
    let new_task = create_task(
        request,
        Some(js::unknown_of(Rc::clone(&model))),
        None,
        false,
        create_root_format_context(),
        &request.abortable_tasks,
        if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
            performance.now() // TODO: This should really inherit the time from the task.
        } else {
            0.0
        },
        None, // TODO: Currently we don't associate any debug information with
        None, // this object on the server. If it ends up erroring, it won't
        None, // have any context on the server but can on the client.
    );

    let reader = blob.stream().get_reader();

    let reading = Rc::new(BlobReading {
        request: Rc::clone(request),
        new_task: Rc::clone(&new_task),
        model,
        reader,
        abort_blob: OnceCell::new(),
    });
    let aborting = Rc::clone(&reading);
    let abort_blob = webapi::listener(move |_: &Event| aborting.abort_blob());
    _ = reading.abort_blob.set(abort_blob);

    request
        .cache_controller
        .signal()
        .add_event_listener(events::Abort, abort_blob, None);

    let progressing = Rc::clone(&reading);
    let erroring = Rc::clone(&reading);
    // $FlowFixMe[incompatible-type]
    let _: Promise<()> = reader
        .read()
        .then(move |entry| progressing.progress(entry))
        .catch(move |reason| erroring.error(reason.as_unknown()));

    "$B".to_owned() + &js::number::to_string(new_task.id, Some(16))
}

fn escape_string_value(value: &str) -> String {
    if value.starts_with('$') {
        // We need to escape $ prefixed strings since we use those to encode
        // references to IDs and as special symbol values.
        "$".to_owned() + value
    } else {
        value.to_owned()
    }
}

fn serialize_import_string(request: &Rc<Request>, value: &str) -> String {
    // No maximum length because import strings are short and repeat often.
    // Deduping model strings too would need one to skip very long strings.
    if (js::string::length(value) as f64) < MIN_DEDUPLICATED_IMPORT_STRING_LENGTH {
        return escape_string_value(value);
    }
    let written_strings = request.written_import_strings;
    let existing = written_strings.get(value);
    if let Some(existing) = existing {
        return existing;
    }
    let size = request.written_import_strings_size.get() + js::string::length(value) as f64;
    if size > MAX_DEDUPLICATED_IMPORT_STRINGS_SIZE {
        // The map is full. Strings already outlined keep deduping; new ones are
        // written out every time.
        return escape_string_value(value);
    }
    request.written_import_strings_size.set(size);
    // Chunk names are almost always shared, so the first occurrence is outlined
    // right away instead of waiting for a repeat.
    request
        .pending_chunks
        .set(request.pending_chunks.get() + 1.0);
    let outlined_id = request.next_chunk_id.get();
    request.next_chunk_id.set(outlined_id + 1.0);
    // $FlowFixMe[incompatible-type] stringify can return null
    let json = js::json::stringify(&*escape_string_value(value));
    // The client reads import metadata synchronously, so this row has to have
    // been written by the time the referencing row arrives. Import chunks are
    // flushed ahead of regular ones, which regular chunks can't guarantee.
    request
        .completed_import_chunks
        .borrow_mut()
        .push(string_to_chunk(
            &(js::number::to_string(outlined_id, Some(16)) + ":" + &json + "\n"),
        ));
    let r#ref = serialize_by_value_id(outlined_id);
    written_strings.set(value, r#ref.clone());
    r#ref
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::nullable)]
    static MODEL_ROOT: Cell<Option<&'static ReactClientValue>> =
        Cell::new(Some(js::unknown(&false)));
}

/// `parent`, an object or an array.
fn render_model(
    request: &Rc<Request>,
    task: &Rc<Task>,
    parent: &'static Unknown,
    key: &str,
    value: Option<&'static ReactClientValue>,
) -> Option<&'static ReactJsonValue> {
    // First time we're serializing the key, we should add it to the size.
    SERIALIZED_SIZE.set(SERIALIZED_SIZE.get() + js::string::length(key) as f64);

    let prev_key_path = task.key_path.borrow().clone();
    let prev_implicit_slot = task.implicit_slot.get();
    let thrown = match catch_unwind(AssertUnwindSafe(|| {
        render_model_destructive(request, task, parent, key, value)
    })) {
        Ok(rendered) => return rendered,
        Err(thrown) => thrown,
    };
    let thrown_value = thrown_value_of(&*thrown);
    // If the suspended/errored value was an element or lazy it can be reduced
    // to a lazy reference, so that it doesn't error the parent.
    let model = task.model.get();
    let was_react_node = if let Some(model) = js::classify(model).object() {
        matches!(
            SymbolFor::from_value(js::get(model, "$$typeof")),
            Some(REACT_ELEMENT_TYPE | REACT_LAZY_TYPE)
        )
    } else {
        false
    };

    if request.status.get() == ABORTING {
        task.status.set(ABORTED);
        if request.r#type == PRERENDER {
            // This will create a new task and refer to it in this slot
            // the new task won't be retried because we are aborting
            return outline_halted_task(request, task, was_react_node);
        }
        let error_id: f64 = unsafe { js::cast(request.fatal_error.get()) };
        if was_react_node {
            return Some(js::unknown_of(serialize_lazy_id(error_id)));
        }
        return Some(js::unknown_of(serialize_by_value_id(error_id)));
    }

    let x = if is_suspense_exception(thrown_value) {
        // This is a special type of exception used for Suspense. For historical
        // reasons, the rest of the Suspense implementation expects the thrown
        // value to be a thenable, because before `use` existed that was the
        // (unstable) API for suspending. This implementation detail can change
        // later, once we deprecate the old API in favor of `use`.
        Some(js::unknown(get_suspended_thenable()))
    } else {
        thrown_value
    };

    // $FlowFixMe[invalid-compare]
    if let Some(x_object) = js::classify(x).object()
        // $FlowFixMe[method-unbinding]
        && js::type_of(js::get(x_object, "then")) == "function"
    {
        // Something suspended, we'll need to create a new task and resolve it later.
        let new_task = create_task(
            request,
            task.model.get(),
            task.key_path.borrow().clone(),
            task.implicit_slot.get(),
            task.format_context.get(),
            &request.abortable_tasks,
            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
                task.time.get()
            } else {
                0.0
            },
            if __DEV__ {
                task.debug_owner.get()
            } else {
                js::null()
            },
            if __DEV__ {
                task.debug_stack.get()
            } else {
                js::null()
            },
            if __DEV__ {
                task.debug_task.get()
            } else {
                js::null()
            },
        );
        let (resolve, reject) = ping_of(request, &new_task);
        let thenable: &'static Thenable = unsafe { js::cast(x) };
        thenable.then(resolve, reject);
        *new_task.thenable_state.borrow_mut() = Some(get_thenable_state_after_suspending());

        // Restore the context. We assume that this will be restored by the inner
        // functions in case nothing throws so we don't use "finally" here.
        *task.key_path.borrow_mut() = prev_key_path;
        task.implicit_slot.set(prev_implicit_slot);

        if was_react_node {
            return Some(js::unknown_of(serialize_lazy_id(new_task.id)));
        }
        return Some(js::unknown_of(serialize_by_value_id(new_task.id)));
    }

    // Restore the context. We assume that this will be restored by the inner
    // functions in case nothing throws so we don't use "finally" here.
    *task.key_path.borrow_mut() = prev_key_path;
    task.implicit_slot.set(prev_implicit_slot);

    // Something errored. We'll still send everything we have up until this point.
    request
        .pending_chunks
        .set(request.pending_chunks.get() + 1.0);
    let error_id = request.next_chunk_id.get();
    request.next_chunk_id.set(error_id + 1.0);
    let digest = log_recoverable_error(request, x, Some(task));
    emit_error_chunk(
        request,
        error_id,
        &digest,
        x,
        false,
        if __DEV__ {
            unsafe { js::cast::<Option<&'static ReactComponentInfo>>(task.debug_owner.get()) }
        } else {
            None
        },
    );
    if was_react_node {
        // We'll replace this element with a lazy reference that throws on the client
        // once it gets rendered.
        return Some(js::unknown_of(serialize_lazy_id(error_id)));
    }
    // If we don't know if it was a React Node we render a direct reference and let
    // the client deal with it.
    Some(js::unknown_of(serialize_by_value_id(error_id)))
}

/// The written reference of `parent`, an object or an array.
fn written_reference_of(request: &Request, parent: &'static Unknown) -> Option<String> {
    request
        .written_objects
        .get(unsafe { js::cast::<&JsObject>(Some(parent)) })
}

/// `parent`, an object or an array.
fn render_model_destructive(
    request: &Rc<Request>,
    task: &Rc<Task>,
    parent: &'static Unknown,
    parent_property_name: &str,
    value: Option<&'static ReactClientValue>,
) -> Option<&'static ReactJsonValue> {
    // Set the currently rendering model
    task.model.set(value);

    if __DEV__ && parent_property_name == __PROTO__ {
        call_with_debug_context_in_dev(
            request,
            task,
            |()| {
                console::error((
                    "Expected not to serialize an object with own property `__proto__`. When parsed this property will be omitted.%s",
                    describe_object_for_error_message(parent, Some(parent_property_name)),
                ));
            },
            (),
        );
    }

    // Special Symbol, that's very common.
    if SymbolFor::from_value(value) == Some(REACT_ELEMENT_TYPE) {
        return Some(js::unknown("$"));
    }

    if matches!(js::classify(value), Kind::Null) {
        return js::null();
    }

    if let Some(object) = js::classify(value).object() {
        match SymbolFor::from_value(js::get(object, "$$typeof")) {
            Some(REACT_ELEMENT_TYPE) => {
                let mut element_reference = None;
                let written_objects = request.written_objects;
                if task.key_path.borrow().is_some() || task.implicit_slot.get() {
                    // If we're in some kind of context we can't reuse the result of this render or
                    // previous renders of this element. We only reuse elements if they're not wrapped
                    // by another Server Component.
                } else {
                    let existing_reference = written_objects.get(object);
                    if let Some(existing_reference) = existing_reference {
                        if js::strict_equals(MODEL_ROOT.get(), value) {
                            // This is the ID we're currently emitting so we need to write it
                            // once but if we discover it again, we refer to it by id.
                            MODEL_ROOT.set(None);
                        } else {
                            // We've already emitted this as an outlined object, so we can refer to that by its
                            // existing ID. TODO: We should use a lazy reference since, unlike plain objects,
                            // elements might suspend so it might not have emitted yet even if we have the ID for
                            // it. However, this creates an extra wrapper when it's not needed. We should really
                            // detect whether this already was emitted and synchronously available. In that
                            // case we can refer to it synchronously and only make it lazy otherwise.
                            // We currently don't have a data structure that lets us see that though.
                            return Some(js::unknown_of(existing_reference));
                        }
                    } else if js::string::index_of(parent_property_name, ":", None) == -1.0 {
                        // TODO: If the property name contains a colon, we don't dedupe. Escape instead.
                        let parent_reference = written_reference_of(request, parent);
                        if let Some(parent_reference) = parent_reference {
                            // If the parent has a reference, we can refer to this object indirectly
                            // through the property name inside that parent.
                            let reference = parent_reference + ":" + parent_property_name;
                            written_objects.set(object, reference.clone());
                            element_reference = Some(reference);
                        }
                    }
                }

                let element: &'static ReactElement = unsafe { js::cast(value) };

                if SERIALIZED_SIZE.get() > MAX_ROW_SIZE {
                    return defer_task(request, task);
                }

                if __DEV__ && let Some(debug_info) = element.debug_info {
                    // If this came from Flight, forward any debug info into this new row.
                    if !CAN_EMIT_DEBUG_INFO.get() {
                        // We don't have a chunk to assign debug info. We need to outline this
                        // component to assign it an ID.
                        return outline_task(request, task);
                    } else {
                        // Forward any debug info we have the first time we see it.
                        forward_debug_info(request, task, debug_info);
                    }
                }

                let props = element.props;
                // TODO: We should get the ref off the props object right before using
                // it.
                let ref_prop = js::get(props, "ref");
                let r#ref = if !matches!(js::classify(ref_prop), Kind::Undefined) {
                    ref_prop
                } else {
                    js::null()
                };

                // Attempt to render the Server Component.

                if __DEV__ {
                    task.debug_owner.set(js::get(element, "_owner"));
                    task.debug_stack.set(js::get(element, "_debugStack"));
                    task.debug_task.set(js::get(element, "_debugTask"));
                    if matches!(js::classify(js::get(element, "_owner")), Kind::Undefined)
                        || matches!(
                            js::classify(js::get(element, "_debugStack")),
                            Kind::Undefined
                        )
                        || matches!(
                            js::classify(js::get(element, "_debugTask")),
                            Kind::Undefined
                        )
                    {
                        let mut key = String::new();
                        if let Some(ReactKey::Text(element_key)) = &element.key {
                            key = " key=\"".to_owned() + element_key + "\"";
                        }

                        console::error((
                            concat!(
                                "Attempted to render <%s%s> without development properties. ",
                                "This is not supported. It can happen if:",
                                "\n- The element is created with a production version of React but rendered in development.",
                                "\n- The element was cloned with a custom function instead of `React.cloneElement`.\n",
                                "The props of this element may help locate this element: %o",
                            ),
                            element.r#type,
                            key,
                            element.props,
                        ));
                    }
                    // TODO: Pop this. Since we currently don't have a point where we can pop the stack
                    // this debug information will be used for errors inside sibling properties that
                    // are not elements. Leading to the wrong attribution on the server. We could fix
                    // that if we switch to a proper stack instead of resolveModel's recursive walk.
                    // Attribution on the client is still correct since it has a pop.
                }

                let new_child = render_element(
                    request,
                    task,
                    element.r#type,
                    // $FlowFixMe[incompatible-call] the key of an element is null | string | ReactOptimisticKey
                    element.key.clone(),
                    r#ref,
                    props,
                    if __DEV__ {
                        unsafe { js::cast(js::get(element.store.get(), "validated")) }
                    } else {
                        0.0
                    },
                );
                if let Some(new_child) = js::classify(new_child).object()
                    && let Some(element_reference) = element_reference
                {
                    // If this element renders another object, we can now refer to that object through
                    // the same location as this element.
                    if !written_objects.has(new_child) {
                        written_objects.set(new_child, element_reference);
                    }
                }
                return new_child;
            }
            Some(REACT_LAZY_TYPE) => {
                if SERIALIZED_SIZE.get() > MAX_ROW_SIZE {
                    return defer_task(request, task);
                }

                // Reset the task's thenable state before continuing. If there was one, it was
                // from suspending the lazy before.
                *task.thenable_state.borrow_mut() = None;

                let lazy: &'static LazyComponent = unsafe { js::cast(value) };
                let resolved_model = if __DEV__ {
                    CALL_LAZY_INIT_IN_DEV
                        .with(|call_lazy_init_in_dev| call_lazy_init_in_dev.unwrap())(
                        lazy
                    )
                } else {
                    let payload = &lazy.payload;
                    let init = lazy.init;
                    init(payload)
                };
                if request.status.get() == ABORTING {
                    // lazy initializers are user code and could abort during render
                    // we don't wan to return any value resolved from the lazy initializer
                    // if it aborts so we interrupt rendering here
                    // eslint-disable-next-line no-throw-literal
                    js::throw(js::null());
                }
                if __DEV__ {
                    let debug_info: Option<&'static ReactDebugInfo> =
                        unsafe { js::cast(js::get(lazy, "_debugInfo")) };
                    if let Some(debug_info) = debug_info {
                        // If this came from Flight, forward any debug info into this new row.
                        if !CAN_EMIT_DEBUG_INFO.get() {
                            // We don't have a chunk to assign debug info. We need to outline this
                            // component to assign it an ID.
                            return outline_task(request, task);
                        } else {
                            // Forward any debug info we have the first time we see it.
                            // We do this after init so that we have received all the debug info
                            // from the server by the time we emit it.
                            forward_debug_info(request, task, debug_info);
                        }
                    }
                }
                return render_model_destructive(
                    request,
                    task,
                    parent,
                    parent_property_name,
                    resolved_model,
                );
            }
            Some(REACT_LEGACY_ELEMENT_TYPE) => {
                panic!(concat!(
                    "A React Element from an older version of React was rendered. ",
                    "This is not supported. It can happen if:\n",
                    "- Multiple copies of the \"react\" package is used.\n",
                    "- A library pre-bundled an old copy of \"react\" or \"react/jsx-runtime\".\n",
                    "- A compiler tries to \"inline\" JSX instead of using the runtime.",
                ));
            }
            _ => {}
        }

        let object_value = js::unknown(object);

        if is_client_reference(object_value) {
            return Some(js::unknown_of(serialize_client_reference(
                request,
                parent,
                parent_property_name,
                object_value,
            )));
        }

        if ENABLE_FLIGHT_OBJECT_REFERENCES.with(|&enabled| enabled)
            && is_server_reference(object_value)
        {
            // An object registered as a Server Reference. This must be checked
            // before the thenable case below so that a reference that happens to
            // be a Promise is serialized by reference instead of being awaited.
            return Some(js::unknown_of(serialize_server_reference(
                request,
                object_value,
            )));
        }

        if let Some(temporary_references) = request.temporary_references {
            let temp_ref =
                resolve_temporary_reference(temporary_references, unsafe { js::cast(value) });
            if let Some(temp_ref) = temp_ref {
                return Some(js::unknown_of(serialize_temporary_reference(
                    request, &temp_ref,
                )));
            }
        }

        if ENABLE_TAINT.with(|&enabled| enabled) {
            let tainted = TAINT_REGISTRY_OBJECTS
                .with(|&objects| objects.unwrap())
                .get(unsafe { js::cast(value) });
            if let Some(tainted) = tainted {
                throw_taint_violation(&tainted);
            }
        }

        let written_objects = request.written_objects;
        let existing_reference = written_objects.get(object);
        // $FlowFixMe[method-unbinding]
        if js::type_of(js::get(object, "then")) == "function" {
            let thenable: &'static Thenable = unsafe { js::cast(value) };
            // A weak-pending thenable may never emit, so its reference is marked
            // on the wire ($w instead of $@). That way the client knows to leave
            // it forever pending, instead of erroring it, if the stream closes
            // first.
            if let Some(existing_reference) = existing_reference {
                if task.key_path.borrow().is_some() || task.implicit_slot.get() {
                    // If we're in some kind of context we can't reuse the result of this render or
                    // previous renders of this element. We only reuse Promises if they're not wrapped
                    // by another Server Component.
                    let promise_id = serialize_thenable(request, task, thenable);
                    return Some(js::unknown_of(
                        if ENABLE_FLIGHT_WEAK_THENABLES.with(|&enabled| enabled)
                            && thenable.status().as_deref() == Some("pending_weak")
                        {
                            serialize_weak_promise_id(promise_id)
                        } else {
                            serialize_promise_id(promise_id)
                        },
                    ));
                } else if js::strict_equals(MODEL_ROOT.get(), value) {
                    // This is the ID we're currently emitting so we need to write it
                    // once but if we discover it again, we refer to it by id.
                    MODEL_ROOT.set(None);
                } else {
                    // We've seen this promise before, so we can just refer to the same result.
                    return Some(js::unknown_of(existing_reference));
                }
            }
            // We assume that any object with a .then property is a "Thenable" type,
            // or a Promise type. Either of which can be represented by a Promise.
            let promise_id = serialize_thenable(request, task, thenable);
            let promise_reference = if ENABLE_FLIGHT_WEAK_THENABLES.with(|&enabled| enabled)
                && thenable.status().as_deref() == Some("pending_weak")
            {
                serialize_weak_promise_id(promise_id)
            } else {
                serialize_promise_id(promise_id)
            };
            written_objects.set(object, promise_reference.clone());
            return Some(js::unknown_of(promise_reference));
        }

        if let Some(existing_reference) = existing_reference {
            if js::strict_equals(MODEL_ROOT.get(), value) {
                if existing_reference != serialize_by_value_id(task.id) {
                    // Turns out that we already have this root at a different reference.
                    // Use that after all.
                    return Some(js::unknown_of(existing_reference));
                }
                // This is the ID we're currently emitting so we need to write it
                // once but if we discover it again, we refer to it by id.
                MODEL_ROOT.set(None);
            } else {
                // We've already emitted this as an outlined object, so we can
                // just refer to that by its existing ID.
                return Some(js::unknown_of(existing_reference));
            }
        } else if js::string::index_of(parent_property_name, ":", None) == -1.0 {
            // TODO: If the property name contains a colon, we don't dedupe. Escape instead.
            let parent_reference = written_reference_of(request, parent);
            if let Some(parent_reference) = parent_reference {
                // If the parent has a reference, we can refer to this object indirectly
                // through the property name inside that parent.
                let mut property_name = parent_property_name;
                if is_array(Some(parent))
                    && SymbolFor::from_value(js::get(parent, &0u32)) == Some(REACT_ELEMENT_TYPE)
                {
                    // For elements, we've converted it to an array but we'll have converted
                    // it back to an element before we read the references so the property
                    // needs to be aliased.
                    match parent_property_name {
                        "1" => property_name = "type",
                        "2" => property_name = "key",
                        "3" => property_name = "props",
                        "4" => property_name = "_owner",
                        _ => {}
                    }
                }
                written_objects.set(object, parent_reference + ":" + property_name);
            }
        }

        if is_array(value) {
            return render_fragment(request, task, object_value);
        }

        if js::instance_of(object, js::class::<Map<Unknown, Unknown>>()) {
            return Some(js::unknown_of(serialize_map(request, object_value)));
        }
        if js::instance_of(object, js::class::<js::Set<Unknown>>()) {
            return Some(js::unknown_of(serialize_set(request, object_value)));
        }
        // TODO: FormData is not available in old Node. Remove the typeof later.
        if js::is_function(webapi::form_data::new)
            && js::instance_of(object, js::class::<FormData>())
        {
            return Some(js::unknown_of(serialize_form_data(request, unsafe {
                js::cast(value)
            })));
        }
        if js::instance_of(object, js::class::<Error>()) {
            return Some(js::unknown_of(serialize_error_value(request, object_value)));
        }
        if js::instance_of(object, js::class::<ArrayBuffer>()) {
            let array_buffer: &'static ArrayBuffer = unsafe { js::cast(value) };
            return Some(js::unknown_of(serialize_typed_array(
                request,
                "A",
                js::unknown(js::uint8_array::new_with_buffer(array_buffer, None, None)),
            )));
        }
        if js::instance_of(object, js::class::<Int8Array>()) {
            // char
            return Some(js::unknown_of(serialize_typed_array(
                request,
                "O",
                object_value,
            )));
        }
        if js::instance_of(object, js::class::<Uint8Array>()) {
            // unsigned char
            return Some(js::unknown_of(serialize_typed_array(
                request,
                "o",
                object_value,
            )));
        }
        if js::instance_of(object, js::class::<Uint8ClampedArray>()) {
            // unsigned clamped char
            return Some(js::unknown_of(serialize_typed_array(
                request,
                "U",
                object_value,
            )));
        }
        if js::instance_of(object, js::class::<Int16Array>()) {
            // sort
            return Some(js::unknown_of(serialize_typed_array(
                request,
                "S",
                object_value,
            )));
        }
        if js::instance_of(object, js::class::<Uint16Array>()) {
            // unsigned short
            return Some(js::unknown_of(serialize_typed_array(
                request,
                "s",
                object_value,
            )));
        }
        if js::instance_of(object, js::class::<Int32Array>()) {
            // long
            return Some(js::unknown_of(serialize_typed_array(
                request,
                "L",
                object_value,
            )));
        }
        if js::instance_of(object, js::class::<Uint32Array>()) {
            // unsigned long
            return Some(js::unknown_of(serialize_typed_array(
                request,
                "l",
                object_value,
            )));
        }
        if js::instance_of(object, js::class::<Float32Array>()) {
            // float
            return Some(js::unknown_of(serialize_typed_array(
                request,
                "G",
                object_value,
            )));
        }
        if js::instance_of(object, js::class::<Float64Array>()) {
            // double
            return Some(js::unknown_of(serialize_typed_array(
                request,
                "g",
                object_value,
            )));
        }
        if js::instance_of(object, js::class::<BigInt64Array>()) {
            // number
            return Some(js::unknown_of(serialize_typed_array(
                request,
                "M",
                object_value,
            )));
        }
        if js::instance_of(object, js::class::<BigUint64Array>()) {
            // unsigned number
            // We use "m" instead of "n" since JSON can start with "null"
            return Some(js::unknown_of(serialize_typed_array(
                request,
                "m",
                object_value,
            )));
        }
        if js::instance_of(object, js::class::<DataView>()) {
            return Some(js::unknown_of(serialize_typed_array(
                request,
                "V",
                object_value,
            )));
        }
        // TODO: Blob is not available in old Node. Remove the typeof check later.
        if js::is_function(webapi::blob::new) && js::instance_of(object, js::class::<Blob>()) {
            return Some(js::unknown_of(serialize_blob(request, unsafe {
                js::cast(value)
            })));
        }

        let iterator_fn = get_iterator_fn(value);
        if let Some(iterator_fn) = iterator_fn {
            // TODO: Should we serialize the return value as well like we do for AsyncIterables?
            let iterator = js::function::call(
                unsafe { js::cast::<&dyn Fn() -> Option<&'static Unknown>>(Some(iterator_fn)) },
                Some(&value),
                (),
            );
            if js::strict_equals(iterator, value) {
                // Iterator, not Iterable
                return Some(js::unknown_of(serialize_iterator(request, unsafe {
                    js::cast(iterator)
                })));
            }
            return render_fragment(
                request,
                task,
                js::unknown_of(js::array::from::<Option<&'static Unknown>>(unsafe {
                    js::cast::<&'static Unknown>(iterator)
                })),
            );
        }

        // TODO: Blob is not available in old Node. Remove the typeof check later.
        if js::is_function(webapi::readable_stream::new)
            && js::instance_of(object, js::class::<ReadableStream>())
        {
            return Some(js::unknown_of(serialize_readable_stream(
                request,
                task,
                unsafe { js::cast(value) },
            )));
        }
        let get_async_iterator = js::get(
            object,
            ASYNC_ITERATOR.with(|&async_iterator| async_iterator),
        );
        if let Kind::Function(get_async_iterator) = js::classify(get_async_iterator) {
            // We treat AsyncIterables as a Fragment and as such we might need to key them.
            return render_async_fragment(
                request,
                task,
                object_value,
                js::unknown(get_async_iterator),
            );
        }

        // We put the Date check low b/c most of the time Date's will already have been serialized
        // before we process it in this function but when rendering a Date() as a top level it can
        // end up being a Date instance here. This is rare so we deprioritize it by putting it deep
        // in this function
        if js::instance_of(object, js::class::<Date>()) {
            return Some(js::unknown_of(serialize_date(object_value)));
        }

        // Verify that this is a simple plain object.
        let proto = GET_PROTOTYPE_OF.with(|get_prototype_of| get_prototype_of(value));
        if !js::strict_equals(proto, Some(js::object::prototype))
            && (matches!(js::classify(proto), Kind::Null)
                || !matches!(
                    js::classify(GET_PROTOTYPE_OF.with(|get_prototype_of| get_prototype_of(proto))),
                    Kind::Null
                ))
        {
            panic!(
                "Only plain objects, and a few built-ins, can be passed to Client Components from Server Components. Classes or null prototypes are not supported.{}",
                describe_object_for_error_message(parent, Some(parent_property_name))
            );
        }
        if __DEV__ {
            if object_name(value) != "Object" {
                call_with_debug_context_in_dev(
                    request,
                    task,
                    |()| {
                        console::error((
                            concat!(
                                "Only plain objects can be passed to Client Components from Server Components. ",
                                "%s objects are not supported.%s",
                            ),
                            object_name(value),
                            describe_object_for_error_message(parent, Some(parent_property_name)),
                        ));
                    },
                    (),
                );
            } else if !is_simple_object(object_value) {
                call_with_debug_context_in_dev(
                    request,
                    task,
                    |()| {
                        console::error((
                            concat!(
                                "Only plain objects can be passed to Client Components from Server Components. ",
                                "Classes or other objects with methods are not supported.%s",
                            ),
                            describe_object_for_error_message(parent, Some(parent_property_name)),
                        ));
                    },
                    (),
                );
            } else if js::is_function(js::object::get_own_property_symbols) {
                let symbols = js::object::get_own_property_symbols(object_value);
                if !symbols.is_empty() {
                    call_with_debug_context_in_dev(
                        request,
                        task,
                        |()| {
                            console::error((
                                concat!(
                                    "Only plain objects can be passed to Client Components from Server Components. ",
                                    "Objects with symbol properties like %s are not supported.%s",
                                ),
                                symbols[0].description(),
                                describe_object_for_error_message(
                                    parent,
                                    Some(parent_property_name),
                                ),
                            ));
                        },
                        (),
                    );
                }
            }
        }

        // $FlowFixMe[incompatible-type]
        return value;
    }

    if let Kind::String(value) = js::classify(value) {
        if ENABLE_TAINT.with(|&enabled| enabled) {
            let tainted = TAINT_REGISTRY_VALUES
                .with(|&values| values.unwrap())
                .get(&unsafe { js::cast::<EntryValue>(Some(js::unknown(value))) });
            if let Some(tainted) = tainted {
                throw_taint_violation(&tainted.message);
            }
        }
        SERIALIZED_SIZE.set(SERIALIZED_SIZE.get() + js::string::length(value) as f64);
        // TODO: Maybe too clever. If we support URL there's no similar trick.
        if value.ends_with('Z') {
            // Possibly a Date, whose toJSON automatically calls toISOString
            // $FlowFixMe[incompatible-use]
            let original_value = js::get(parent, parent_property_name);
            if js::instance_of(original_value, js::class::<Date>()) {
                return Some(js::unknown_of(serialize_date_from_date_json(value)));
            }
        }
        // $FlowFixMe[invalid-compare]
        if js::string::length(value) >= 1024 && BYTE_LENGTH_OF_CHUNK.is_some() {
            // For large strings, we encode them outside the JSON payload so that we
            // don't have to double encode and double parse the strings. This can also
            // be more compact in case the string has a lot of escaped characters.
            return Some(js::unknown_of(serialize_large_text_string(request, value)));
        }
        return Some(js::unknown_of(escape_string_value(value)));
    }

    if let Kind::Bool(_) = js::classify(value) {
        return value;
    }

    if let Kind::Number(value) = js::classify(value) {
        return Some(serialize_number(value));
    }

    if let Kind::Undefined = js::classify(value) {
        return Some(js::unknown_of(serialize_undefined()));
    }

    if let Kind::Function(function) = js::classify(value) {
        let function_value = js::unknown(function);
        if is_client_reference(function_value) {
            return Some(js::unknown_of(serialize_client_reference(
                request,
                parent,
                parent_property_name,
                function_value,
            )));
        }
        if is_server_reference(function_value) {
            return Some(js::unknown_of(serialize_server_reference(
                request,
                function_value,
            )));
        }
        if let Some(temporary_references) = request.temporary_references {
            let temp_ref =
                resolve_temporary_reference(temporary_references, unsafe { js::cast(value) });
            if let Some(temp_ref) = temp_ref {
                return Some(js::unknown_of(serialize_temporary_reference(
                    request, &temp_ref,
                )));
            }
        }

        if ENABLE_TAINT.with(|&enabled| enabled) {
            let tainted = TAINT_REGISTRY_OBJECTS
                .with(|&objects| objects.unwrap())
                .get(unsafe { js::cast(value) });
            if let Some(tainted) = tainted {
                throw_taint_violation(&tainted);
            }
        }

        if is_opaque_temporary_reference(function_value) {
            panic!(concat!(
                "Could not reference an opaque temporary reference. ",
                "This is likely due to misconfiguring the temporaryReferences options ",
                "on the server.",
            ));
        } else if js::reg_exp::new("^on[A-Z]", "").test(parent_property_name) {
            panic!(
                "Event handlers cannot be passed to Client Component props.{}\nIf you need interactivity, consider converting part of this to a Client Component.",
                describe_object_for_error_message(parent, Some(parent_property_name))
            );
        } else if __DEV__
            && (JSX_CHILDREN_PARENTS
                .with(|parents| parents.has(unsafe { js::cast::<&JsObject>(Some(parent)) }))
                || (JSX_PROPS_PARENTS
                    .with(|parents| parents.has(unsafe { js::cast::<&JsObject>(Some(parent)) }))
                    && parent_property_name == "children"))
        {
            let component_name = js::or(
                js::or(
                    js::get(function_value, "displayName"),
                    js::get(function_value, "name"),
                ),
                Some(js::unknown("Component")),
            );
            panic!(
                "Functions are not valid as a child of Client Components. This may happen if you return {} instead of <{} /> from render. Or maybe you meant to call this function rather than return it.{}",
                js::shown(component_name),
                js::shown(component_name),
                describe_object_for_error_message(parent, Some(parent_property_name))
            );
        } else {
            panic!(
                "Functions cannot be passed directly to Client Components unless you explicitly expose it by marking it with \"use server\". Or maybe you meant to call this function rather than return it.{}",
                describe_object_for_error_message(parent, Some(parent_property_name))
            );
        }
    }

    if let Kind::Symbol(symbol) = js::classify(value) {
        let written_symbols = request.written_symbols;
        let existing_id = written_symbols.get(symbol);
        if let Some(existing_id) = existing_id {
            return Some(js::unknown_of(serialize_by_value_id(existing_id)));
        }
        // $FlowFixMe[incompatible-type] `description` might be undefined
        let name = symbol
            .description()
            .unwrap_or_else(|| "undefined".to_owned());

        if !js::strict_equals(js::SymbolFor(&name), value) {
            panic!(
                "Only global symbols received from Symbol.for(...) can be passed to Client Components. The symbol Symbol.for({name}) cannot be found among global symbols.{}",
                describe_object_for_error_message(parent, Some(parent_property_name))
            );
        }

        request
            .pending_chunks
            .set(request.pending_chunks.get() + 1.0);
        let symbol_id = request.next_chunk_id.get();
        request.next_chunk_id.set(symbol_id + 1.0);
        emit_symbol_chunk(request, symbol_id, Some(&name));
        written_symbols.set(symbol, symbol_id);
        return Some(js::unknown_of(serialize_by_value_id(symbol_id)));
    }

    if let Kind::BigInt(big_int) = js::classify(value) {
        if ENABLE_TAINT.with(|&enabled| enabled) {
            let tainted = TAINT_REGISTRY_VALUES
                .with(|&values| values.unwrap())
                .get(&unsafe { js::cast::<EntryValue>(value) });
            if let Some(tainted) = tainted {
                throw_taint_violation(&tainted.message);
            }
        }
        return Some(js::unknown_of(serialize_big_int(big_int)));
    }

    panic!(
        "Type {} is not supported in Client Component props.{}",
        js::type_of(value),
        describe_object_for_error_message(parent, Some(parent_property_name))
    );
}

fn log_recoverable_error(
    request: &Rc<Request>,
    error: Option<&'static Unknown>,
    task: Option<&Rc<Task>>, // DEV-only
) -> String {
    let prev_request = CURRENT_REQUEST.with_borrow(Clone::clone);
    // We clear the request context so that console.logs inside the callback doesn't
    // get forwarded to the client.
    CURRENT_REQUEST.set(None);
    let result = catch_unwind(AssertUnwindSafe(|| {
        let on_error = &request.on_error;
        if __DEV__ && let Some(task) = task {
            // $FlowFixMe[constant-condition]
            if SUPPORTS_REQUEST_STORAGE {
                REQUEST_STORAGE.run(None, || {
                    call_with_debug_context_in_dev(request, task, &**on_error, error)
                })
            } else {
                call_with_debug_context_in_dev(request, task, &**on_error, error)
            }
            // $FlowFixMe[constant-condition]
        } else if SUPPORTS_REQUEST_STORAGE {
            // Exit the request context while running callbacks.
            REQUEST_STORAGE.run(None, || on_error(error))
        } else {
            on_error(error)
        }
    }));
    CURRENT_REQUEST.set(prev_request);
    let error_digest = match result {
        Ok(error_digest) => error_digest,
        Err(thrown) => resume_unwind(thrown),
    };
    if let Some(error_digest) = &error_digest
        && js::type_of(Some(js::unknown(error_digest))) != "string"
    {
        // eslint-disable-next-line react-internal/prod-error-codes
        panic!(
            "onError returned something with a type other than \"string\". onError should return a string and may return null or undefined but must not return anything else. It received something of type \"{}\" instead",
            js::type_of(Some(js::unknown(error_digest)))
        );
    }
    error_digest.unwrap_or_default()
}

fn fatal_error(request: &Rc<Request>, error: Option<&'static Unknown>) {
    let on_fatal_error = &request.on_fatal_error;
    on_fatal_error(error);
    if ENABLE_TAINT.with(|&enabled| enabled) {
        cleanup_taint_queue(request);
    }
    // This is called outside error handling code such as if an error happens in React internals.
    if let Some(destination) = request.destination.get() {
        request.status.set(CLOSED);
        close_with_error(destination, error);
    } else {
        request.status.set(CLOSING);
        request.fatal_error.set(error);
    }
    let abort_reason = js::error::new(
        Some("The render was aborted due to a fatal error."),
        Some(&ErrorOptions {
            cause: Some(&error),
        }),
    );
    request.cache_controller.abort(Some(abort_reason));
}

/// What an `Error` is sent as, before its `cause` and `errors` are set:
/// `{name, message, stack, env}`.
struct ErrorValueInfo {
    name: String,
    message: String,
    stack: ReactStackTrace,
    env: String,
}

// Safety: a JS object of its fields.
unsafe impl js::Defined for ErrorValueInfo {}

/// `error`'s name, message, stack and environment, as it's sent: where
/// reading them throws, what was read before it and a message saying so.
fn describe_error_value(
    request: &Request,
    error: &'static Error,
    name: &mut String,
    env: &mut String,
) -> (String, ReactStackTrace) {
    match catch_unwind(AssertUnwindSafe(|| {
        *name = error.name();
        // eslint-disable-next-line react-internal/safe-string-coercion
        let message = js::string(js::get(error, "message"));
        let stack = filter_stack_trace(request, &parse_stack_trace(error, 0));
        let error_env = js::get(error, "environmentName");
        if let Kind::String(error_env) = js::classify(error_env) {
            // This probably came from another FlightClient as a pass through.
            // Keep the environment name.
            *env = error_env.to_owned();
        }
        (message, stack)
    })) {
        Ok(described) => described,
        Err(_) => (
            "An error occurred but serializing the error message failed.".to_owned(),
            vec![],
        ),
    }
}

/// `error`, an `Error`.
fn serialize_error_value(request: &Rc<Request>, error: &'static Unknown) -> String {
    if __DEV__ {
        let error: &'static Error = unsafe { js::cast(Some(error)) };
        let mut name = "Error".to_owned();
        let mut env = (request.environment_name)();
        let (message, stack) = describe_error_value(request, error, &mut name, &mut env);
        let error_info = js::unknown_of(ErrorValueInfo {
            name,
            message,
            stack,
            env,
        });
        if js::has(error, "cause") {
            let cause = error.cause();
            let cause_id = outline_model(request, cause);
            js::set(error_info, "cause", serialize_by_value_id(cause_id));
        }
        if js::is_defined(js::aggregate_error::new)
            && js::instance_of(error, js::class::<AggregateError>())
        {
            let errors = js::get(error, "errors");
            let errors_id = outline_model(request, errors);
            js::set(error_info, "errors", serialize_by_value_id(errors_id));
        }
        let id = outline_model(request, Some(error_info));
        "$Z".to_owned() + &js::number::to_string(id, Some(16))
    } else {
        // In prod we don't emit any information about this Error object to avoid
        // unintentional leaks. Since this doesn't actually throw on the server
        // we don't go through onError and so don't register any digest neither.
        "$Z".to_owned()
    }
}

/// `error`, an `Error`.
fn serialize_debug_error_value(
    request: &Rc<Request>,
    counter: &Rc<Counter>,
    error: &'static Unknown,
) -> String {
    if __DEV__ {
        let error: &'static Error = unsafe { js::cast(Some(error)) };
        let mut name = "Error".to_owned();
        let mut env = (request.environment_name)();
        let (message, stack) = describe_error_value(request, error, &mut name, &mut env);
        // +1 compared to the stack alone since errorInfo itself also counts
        // against the limit.
        let object_limit = stack.len() as f64 * 2.0 + 2.0;
        let error_info = js::unknown_of(ErrorValueInfo {
            name,
            message,
            stack,
            env,
        });
        if js::has(error, "cause") {
            counter.object_limit.set(counter.object_limit.get() - 1.0);
            let cause = error.cause();
            let cause_id = outline_debug_model(request, counter, cause);
            js::set(error_info, "cause", serialize_by_value_id(cause_id));
        }
        if js::is_defined(js::aggregate_error::new)
            && js::instance_of(error, js::class::<AggregateError>())
        {
            counter.object_limit.set(counter.object_limit.get() - 1.0);
            let errors = js::get(error, "errors");
            let errors_id = outline_debug_model(request, counter, errors);
            js::set(error_info, "errors", serialize_by_value_id(errors_id));
        }
        let id = outline_debug_model(
            request,
            &Rc::new(Counter {
                object_limit: Cell::new(object_limit),
            }),
            Some(error_info),
        );
        "$Z".to_owned() + &js::number::to_string(id, Some(16))
    } else {
        // In prod we don't emit any information about this Error object to avoid
        // unintentional leaks. Since this doesn't actually throw on the server
        // we don't go through onError and so don't register any digest neither.
        "$Z".to_owned()
    }
}

fn emit_error_chunk(
    request: &Rc<Request>,
    id: f64,
    digest: &str,
    error: Option<&'static Unknown>,
    debug: bool,                                // DEV-only
    owner: Option<&'static ReactComponentInfo>, // DEV-only
) {
    let error_info = if __DEV__ {
        let mut name = "Error".to_owned();
        let mut env = (request.environment_name)();
        let mut cause_reference: Option<String> = None;
        let mut errors_reference: Option<String> = None;
        let described = catch_unwind(AssertUnwindSafe(|| {
            if js::instance_of(error, js::class::<Error>()) {
                let error: &'static Error = unsafe { js::cast(error) };
                name = error.name();
                // eslint-disable-next-line react-internal/safe-string-coercion
                let message = js::string(js::get(error, "message"));
                let stack = filter_stack_trace(request, &parse_stack_trace(error, 0));
                let error_env = js::get(error, "environmentName");
                if let Kind::String(error_env) = js::classify(error_env) {
                    // This probably came from another FlightClient as a pass through.
                    // Keep the environment name.
                    env = error_env.to_owned();
                }
                if js::has(error, "cause") {
                    let cause = error.cause();
                    let cause_id = if debug {
                        outline_debug_model(
                            request,
                            &Rc::new(Counter {
                                object_limit: Cell::new(5.0),
                            }),
                            cause,
                        )
                    } else {
                        outline_model(request, cause)
                    };
                    cause_reference = Some(serialize_by_value_id(cause_id));
                }
                if js::is_defined(js::aggregate_error::new)
                    && js::instance_of(error, js::class::<AggregateError>())
                {
                    let errors = js::get(error, "errors");
                    let errors_id = if debug {
                        outline_debug_model(
                            request,
                            &Rc::new(Counter {
                                object_limit: Cell::new(5.0),
                            }),
                            errors,
                        )
                    } else {
                        outline_model(request, errors)
                    };
                    errors_reference = Some(serialize_by_value_id(errors_id));
                }
                (message, stack)
            } else if let Some(error) = js::classify(error).object() {
                (
                    describe_object_for_error_message(js::unknown(error), None),
                    vec![],
                )
            } else {
                // eslint-disable-next-line react-internal/safe-string-coercion
                (js::string(error), vec![])
            }
        }));
        let (message, stack) = described.unwrap_or_else(|_| {
            (
                "An error occurred but serializing the error message failed.".to_owned(),
                vec![],
            )
        });
        let owner_ref = owner.map(|owner| outline_component_info(request, owner));
        ReactErrorInfo::Dev(ReactErrorInfoDev {
            digest: Some(digest.to_owned()),
            name,
            message,
            stack,
            env,
            owner: owner_ref,
            cause: cause_reference.map(js::unknown_of),
            errors: errors_reference.map(js::unknown_of),
        })
    } else {
        ReactErrorInfo::Prod(ReactErrorInfoProd {
            digest: digest.to_owned(),
        })
    };
    let row = serialize_row_header("E", id) + &stringify(Some(js::unknown(&error_info))) + "\n";
    let processed_chunk = string_to_chunk(&row);
    if __DEV__ && debug {
        request
            .completed_debug_chunks
            .borrow_mut()
            .push(processed_chunk);
    } else {
        request
            .completed_error_chunks
            .borrow_mut()
            .push(processed_chunk);
    }
}

thread_local! {
    // Null on the debug channel, which can't reference rows in the main stream.
    #[cfg_attr(rust_js, rust_js::nullable)]
    static IMPORT_STRING_REQUEST: RefCell<Option<Rc<Request>>> = const { RefCell::new(None) };
}

fn import_metadata_replacer(
    _key: &'static str,
    value: Option<&'static Unknown>,
) -> Option<&'static Unknown> {
    if let Kind::String(value) = js::classify(value) {
        let request = IMPORT_STRING_REQUEST.with_borrow(Clone::clone);
        let Some(request) = request else {
            return Some(js::unknown_of(escape_string_value(value)));
        };
        return Some(js::unknown_of(serialize_import_string(&request, value)));
    }
    value
}

fn stringify_import_metadata_with_replacer(
    request: &Rc<Request>,
    client_reference_metadata: &'static ClientReferenceMetadata,
    debug: bool,
) -> String {
    let prev_request = IMPORT_STRING_REQUEST.with_borrow(Clone::clone);
    IMPORT_STRING_REQUEST.set(if __DEV__ && debug {
        None
    } else {
        Some(Rc::clone(request))
    });
    let result = catch_unwind(AssertUnwindSafe(|| {
        // $FlowFixMe[incompatible-type] stringify can return null
        js::json::stringify_with(client_reference_metadata, import_metadata_replacer, 0)
            .unwrap()
            .unwrap()
    }));
    IMPORT_STRING_REQUEST.set(prev_request);
    match result {
        Ok(json) => json,
        Err(thrown) => resume_unwind(thrown),
    }
}

// Bundler metadata is two or three levels deep. The bound is only there so a
// cycle ends up in stringify itself, which throws its own error for it.
const MAX_IMPORT_METADATA_DEPTH: f64 = 16.0;

thread_local! {
    static NOT_PLAIN_IMPORT_METADATA: &'static Unknown =
        js::unknown(js::object::spread(None::<&Unknown>));
}

fn is_not_plain_import_metadata(value: Option<&'static Unknown>) -> bool {
    NOT_PLAIN_IMPORT_METADATA.with(|&not_plain| js::strict_equals(value, Some(not_plain)))
}

fn not_plain_import_metadata() -> Option<&'static Unknown> {
    Some(NOT_PLAIN_IMPORT_METADATA.with(|&not_plain| not_plain))
}

// Copies the metadata with every string replaced by its serialized form, so
// that stringify can run without a replacer. Anything stringify would treat
// specially (toJSON, boxed primitives, class instances) makes this give up
// instead, because the copy would not reproduce that treatment.
fn transform_import_metadata(
    request: &Rc<Request>,
    value: Option<&'static Unknown>,
    depth: f64,
) -> Option<&'static Unknown> {
    match js::classify(value) {
        Kind::String(value) => Some(js::unknown_of(serialize_import_string(request, value))),
        Kind::Number(_) | Kind::Bool(_) | Kind::Undefined => value,
        Kind::Null => js::null(),
        Kind::Array(_) | Kind::Object(_) => {
            if depth > MAX_IMPORT_METADATA_DEPTH {
                return not_plain_import_metadata();
            }
            if js::type_of(js::get(value, "toJSON")) == "function" {
                return not_plain_import_metadata();
            }
            if let Kind::Array(value) = js::classify(value) {
                let mut copy: Vec<Option<&'static Unknown>> = Vec::with_capacity(value.len());
                for &element in value {
                    if let Kind::String(element) = js::classify(element) {
                        copy.push(Some(js::unknown_of(serialize_import_string(
                            request, element,
                        ))));
                        continue;
                    }
                    let child = transform_import_metadata(request, element, depth + 1.0);
                    if is_not_plain_import_metadata(child) {
                        return not_plain_import_metadata();
                    }
                    copy.push(child);
                }
                return Some(js::unknown_of(copy));
            }
            let proto = GET_PROTOTYPE_OF.with(|get_prototype_of| get_prototype_of(value));
            if !js::strict_equals(proto, Some(js::object::prototype))
                && !matches!(js::classify(proto), Kind::Null)
            {
                return not_plain_import_metadata();
            }
            let keys = js::object::keys(unsafe { js::cast(value) });
            let copy = js::dict::new::<Option<&'static Unknown>>();
            for key in &keys {
                if js::has(js::object::prototype, key) {
                    // The copy inherits from Object.prototype, so assigning this key would
                    // hit an accessor like __proto__ or, if the prototype is frozen, throw.
                    return not_plain_import_metadata();
                }
                let element = js::get(value, key);
                if let Kind::String(element) = js::classify(element) {
                    js::dict::set(
                        copy,
                        key,
                        Some(js::unknown_of(serialize_import_string(request, element))),
                    );
                    continue;
                }
                let child = transform_import_metadata(request, element, depth + 1.0);
                if is_not_plain_import_metadata(child) {
                    return not_plain_import_metadata();
                }
                js::dict::set(copy, key, child);
            }
            Some(js::unknown(copy))
        }
        _ => not_plain_import_metadata(),
    }
}

fn stringify_import_metadata(
    request: &Rc<Request>,
    client_reference_metadata: &'static ClientReferenceMetadata,
    debug: bool,
) -> String {
    if !(__DEV__ && debug) {
        let copy = transform_import_metadata(request, Some(client_reference_metadata), 0.0);
        if !is_not_plain_import_metadata(copy) {
            // $FlowFixMe[incompatible-type] stringify can return null
            return stringify(copy);
        }
    }
    stringify_import_metadata_with_replacer(request, client_reference_metadata, debug)
}

fn emit_import_chunk(request: &Rc<Request>, id: f64, json: &str, debug: bool) {
    let row = serialize_row_header("I", id) + json + "\n";
    let processed_chunk = string_to_chunk(&row);
    if __DEV__ && debug {
        request
            .completed_debug_chunks
            .borrow_mut()
            .push(processed_chunk);
    } else {
        request
            .completed_import_chunks
            .borrow_mut()
            .push(processed_chunk);
    }
}

fn emit_hint_chunk(request: &Rc<Request>, code: &str, model: Option<&'static HintModel>) {
    let json = stringify(model);
    let row = ":H".to_owned() + code + &json + "\n";
    let processed_chunk = string_to_chunk(&row);
    request
        .completed_hint_chunks
        .borrow_mut()
        .push(processed_chunk);
}

fn emit_symbol_chunk(request: &Rc<Request>, id: f64, name: Option<&str>) {
    let symbol_reference = serialize_symbol_reference(name);
    let processed_chunk = encode_reference_chunk(request, id, &symbol_reference);
    request
        .completed_import_chunks
        .borrow_mut()
        .push(processed_chunk);
}

fn emit_model_chunk(request: &Rc<Request>, id: f64, json: &str) {
    let row = js::number::to_string(id, Some(16)) + ":" + json + "\n";
    let processed_chunk = string_to_chunk(&row);
    request
        .completed_regular_chunks
        .borrow_mut()
        .push(processed_chunk);
}

fn emit_debug_halt_chunk(request: &Rc<Request>, id: f64) {
    if !__DEV__ {
        // These errors should never make it into a build so we don't need to encode them in codes.json
        // eslint-disable-next-line react-internal/prod-error-codes
        panic!(
            "emitDebugHaltChunk should never be called in production mode. This is a bug in React."
        );
    }
    // This emits a marker that this row will never complete and should intentionally never resolve
    // even when the client stream is closed. We use just the lack of data to indicate this.
    let row = js::number::to_string(id, Some(16)) + ":\n";
    let processed_chunk = string_to_chunk(&row);
    request
        .completed_debug_chunks
        .borrow_mut()
        .push(processed_chunk);
}

fn emit_debug_chunk(request: &Rc<Request>, id: f64, debug_info: &'static ReactDebugInfoEntry) {
    if !__DEV__ {
        // These errors should never make it into a build so we don't need to encode them in codes.json
        // eslint-disable-next-line react-internal/prod-error-codes
        panic!("emitDebugChunk should never be called in production mode. This is a bug in React.");
    }

    let json = serialize_debug_model(request, 500.0, Some(debug_info));
    if request.debug_destination.get().is_some() {
        if json.starts_with("\"$") {
            // This is already an outlined reference so we can just emit it directly,
            // without an unnecessary indirection.
            let row = serialize_row_header("D", id) + &json + "\n";
            request
                .completed_regular_chunks
                .borrow_mut()
                .push(string_to_chunk(&row));
        } else {
            // Outline the debug information to the debug channel.
            let outlined_id = request.next_chunk_id.get();
            request.next_chunk_id.set(outlined_id + 1.0);
            let debug_row = js::number::to_string(outlined_id, Some(16)) + ":" + &json + "\n";
            request
                .pending_debug_chunks
                .set(request.pending_debug_chunks.get() + 1.0);
            request
                .completed_debug_chunks
                .borrow_mut()
                .push(string_to_chunk(&debug_row));
            let row = serialize_row_header("D", id)
                + "\"$"
                + &js::number::to_string(outlined_id, Some(16))
                + "\"\n";
            request
                .completed_regular_chunks
                .borrow_mut()
                .push(string_to_chunk(&row));
        }
    } else {
        let row = serialize_row_header("D", id) + &json + "\n";
        request
            .completed_regular_chunks
            .borrow_mut()
            .push(string_to_chunk(&row));
    }
}

// MOVE to ReactFlightAsyncSequence.rs: an async node is a JS object of its
// fields, which the debug dedupe set holds by identity.
unsafe impl js::Defined for AsyncSequence {}

/// The component info an outlined row says, without what can't be
/// serialized: its `debugTask` and `debugStack`. Its `env`, `owner`,
/// `stack` and `props` are written after, as upstream does.
struct ComponentDebugInfo {
    name: String,
    key: Option<ReactKey>,
}

unsafe impl js::Defined for ComponentDebugInfo {}

fn outline_component_info(
    request: &Rc<Request>,
    component_info: &'static ReactComponentInfo,
) -> String {
    if !__DEV__ {
        // These errors should never make it into a build so we don't need to encode them in codes.json
        // eslint-disable-next-line react-internal/prod-error-codes
        panic!(
            "outlineComponentInfo should never be called in production mode. This is a bug in React."
        );
    }

    // Safety: a component's info is an object.
    let component_info_object: &'static JsObject =
        unsafe { js::cast(Some(js::unknown(component_info))) };
    let existing_ref = request.written_debug_objects.get(component_info_object);
    if let Some(existing_ref) = existing_ref {
        // Already written
        return existing_ref;
    }

    if let Some(owner) = component_info.owner.get() {
        // Ensure the owner is already outlined.
        outline_component_info(request, owner);
    }

    // Limit the number of objects we write to prevent emitting giant props objects.
    let mut object_limit = 10.0;
    if let Some(stack) = &*component_info.stack.borrow() {
        // Ensure we have enough object limit to encode the stack trace.
        object_limit += stack.len() as f64;
    }

    // We use the console encoding so that we can dedupe objects but don't necessarily
    // use the full serialization that requires a task.
    let counter = Rc::new(Counter {
        object_limit: Cell::new(object_limit),
    });

    // We can't serialize the ConsoleTask/Error objects so we need to omit them before serializing.
    let component_debug_info = js::unknown_of(ComponentDebugInfo {
        name: component_info.name.clone(),
        key: component_info.key.clone(),
    });
    if let Some(env) = &component_info.env {
        // $FlowFixMe[cannot-write]
        js::set(component_debug_info, "env", env.as_str());
    }
    if let Some(owner) = component_info.owner.get() {
        // $FlowFixMe[cannot-write]
        js::set(component_debug_info, "owner", owner);
    }
    if component_info.stack.borrow().is_none()
        && let Some(debug_stack) = component_info.debug_stack.get()
    {
        // If we have a debugStack but no parsed stack we should parse it.
        // $FlowFixMe[cannot-write]
        js::set(
            component_debug_info,
            "stack",
            filter_stack_trace(request, &parse_stack_trace(debug_stack, 1)),
        );
    } else {
        let stack = component_info.stack.borrow().clone();
        if let Some(stack) = stack {
            // $FlowFixMe[cannot-write]
            js::set(component_debug_info, "stack", stack);
        }
    }
    // Ensure we serialize props after the stack to favor the stack being complete.
    // $FlowFixMe[cannot-write]
    js::set(component_debug_info, "props", component_info.props);

    let id = outline_debug_model(request, &counter, Some(component_debug_info));
    let r#ref = serialize_by_value_id(id);
    request
        .written_debug_objects
        .set(component_info_object, r#ref.clone());
    // We also store this in the main dedupe set so that it can be referenced by inline React Elements.
    request
        .written_objects
        .set(component_info_object, r#ref.clone());
    r#ref
}

/// An I/O row's info, without what can't be serialized: its `debugTask`
/// and `debugStack`. Its `env`, `stack`, `owner` and `value` are written
/// after, as upstream does.
struct IoDebugInfo {
    name: String,
    start: f64,
    end: f64,
}

unsafe impl js::Defined for IoDebugInfo {}

// `value`, the promise awaited, any value: left out where it's `undefined`.
#[cfg_attr(rust_js, rust_js::name = "emitIOInfoChunk")]
fn emit_io_info_chunk(
    request: &Rc<Request>,
    id: f64,
    name: &str,
    start: f64,
    end: f64,
    value: Option<&'static Unknown>,
    env: Option<&str>,
    owner: Option<&'static ReactComponentInfo>,
    stack: Option<&ReactStackTrace>,
) {
    if !__DEV__ {
        // These errors should never make it into a build so we don't need to encode them in codes.json
        // eslint-disable-next-line react-internal/prod-error-codes
        panic!(
            "emitIOInfoChunk should never be called in production mode. This is a bug in React."
        );
    }

    let mut object_limit = 10.0;
    if let Some(stack) = stack {
        object_limit += stack.len() as f64;
    }

    let relative_start_timestamp = start - request.time_origin.get();
    let relative_end_timestamp = end - request.time_origin.get();
    let debug_io_info = js::unknown_of(IoDebugInfo {
        name: name.to_owned(),
        start: relative_start_timestamp,
        end: relative_end_timestamp,
    });
    if let Some(env) = env {
        // $FlowFixMe[cannot-write]
        js::set(debug_io_info, "env", env);
    }
    if let Some(stack) = stack {
        // $FlowFixMe[cannot-write]
        js::set(debug_io_info, "stack", stack);
    }
    if let Some(owner) = owner {
        // $FlowFixMe[cannot-write]
        js::set(debug_io_info, "owner", owner);
    }
    if !matches!(js::classify(value), Kind::Undefined) {
        // $FlowFixMe[cannot-write]
        js::set(debug_io_info, "value", value);
    }
    let json: String = serialize_debug_model(request, object_limit, Some(debug_io_info));
    let row = js::number::to_string(id, Some(16)) + ":J" + &json + "\n";
    let processed_chunk = string_to_chunk(&row);
    request
        .completed_debug_chunks
        .borrow_mut()
        .push(processed_chunk);
}

#[cfg_attr(rust_js, rust_js::name = "outlineIOInfo")]
fn outline_io_info(request: &Rc<Request>, io_info: &'static ReactIOInfo) {
    // Safety: an I/O info is an object.
    let io_info_object: &'static JsObject = unsafe { js::cast(Some(js::unknown(io_info))) };
    if request.written_objects.has(io_info_object) {
        // Already written
        return;
    }
    // We can't serialize the ConsoleTask/Error objects so we need to omit them before serializing.
    request
        .pending_debug_chunks
        .set(request.pending_debug_chunks.get() + 1.0);
    let id = request.next_chunk_id.get();
    request.next_chunk_id.set(id + 1.0);
    let owner = io_info.owner;
    // Ensure the owner is already outlined.
    if let Some(owner) = owner {
        outline_component_info(request, owner);
    }
    let debug_stack;
    if io_info.stack.is_none()
        && let Some(io_info_debug_stack) = io_info.debug_stack.get()
    {
        // If we have a debugStack but no parsed stack we should parse it.
        debug_stack = Some(filter_stack_trace(
            request,
            &parse_stack_trace(io_info_debug_stack, 1),
        ));
    } else {
        debug_stack = io_info.stack.clone();
    }
    let mut env = io_info.env.clone();
    if env.is_none() {
        // If we're forwarding IO info from this environment, an empty env is effectively the "client" side.
        // The "client" from the perspective of our client will be this current environment.
        env = Some((request.environment_name)());
    }
    emit_io_info_chunk(
        request,
        id,
        io_info.name.get(),
        io_info.start.get(),
        io_info.end.get(),
        // `ReactIOInfo`'s `value` is read as any value: its `Option` reads
        // `None` of both `null` and `undefined`, which upstream tells apart
        // (it writes a `null` `value`, and leaves out an `undefined` one).
        js::get(io_info, "value"),
        env.as_deref(),
        owner,
        debug_stack.as_ref(),
    );
    request
        .written_debug_objects
        .set(io_info_object, serialize_by_value_id(id));
}

#[cfg_attr(rust_js, rust_js::name = "serializeIONode")]
fn serialize_io_node(
    request: &Rc<Request>,
    io_node: &Rc<AsyncSequence>,
    promise_ref: Option<&'static WeakRef<PromiseWithDebugInfo>>,
) -> String {
    // Safety: an async node is an object.
    let io_node_object: &'static JsObject = unsafe { js::cast(Some(js::unknown(&**io_node))) };
    let existing_ref = request.written_debug_objects.get(io_node_object);
    if let Some(existing_ref) = existing_ref {
        // Already written
        return existing_ref;
    }

    let mut stack = None;
    let mut name = String::new();
    if let Some(promise) = io_node.promise {
        // Pick an explicit name from the Promise itself if it exists.
        // Note that we don't use the promiseRef passed in since that's sometimes the awaiting Promise
        // which is the value observed but it's likely not the one with the name on it.
        let promise = promise.deref();
        if let Some(promise) = promise
            // $FlowFixMe[prop-missing]
            && let Kind::String(display_name) = js::classify(js::get(promise, "displayName"))
        {
            name = display_name.to_owned();
        }
    }
    if let Some(io_node_stack) = &io_node.stack {
        // The stack can contain some leading internal frames for the construction of the promise that we skip.
        let full_stack = strip_leading_promise_creation_frames(io_node_stack);
        stack = Some(filter_stack_trace(request, &full_stack));
        if name.is_empty() {
            // If we didn't have an explicit name, try finding one from the stack.
            name = find_called_function_name_from_stack_trace(request, &full_stack);
            // The name can include the object that this was called on but sometimes that's
            // just unnecessary context.
            if name.starts_with("Window.") {
                name = js::string::slice(&name, 7.0, None);
            } else if name.starts_with("<anonymous>.") {
                name = js::string::slice(&name, js::string::length("<anonymous>.") as f64, None);
            }
        }
    }
    let owner = io_node.owner;
    // Ensure the owner is already outlined.
    if let Some(owner) = owner {
        outline_component_info(request, owner);
    }

    let mut value = None;
    if let Some(promise_ref) = promise_ref {
        value = promise_ref.deref();
    }

    // We log the environment at the time when we serialize the I/O node.
    // The environment name may have changed from when the I/O was actually started.
    let env = (request.environment_name)();

    let end_time = if io_node.tag.get() == UNRESOLVED_PROMISE_NODE {
        // Mark the end time as now. It's arbitrary since it's not resolved but this
        // marks when we called abort and therefore stopped trying.
        request.abort_time.get()
    } else {
        io_node.end.get()
    };

    request
        .pending_debug_chunks
        .set(request.pending_debug_chunks.get() + 1.0);
    let id = request.next_chunk_id.get();
    request.next_chunk_id.set(id + 1.0);
    emit_io_info_chunk(
        request,
        id,
        &name,
        io_node.start.get(),
        end_time,
        value.map(js::unknown),
        Some(&env),
        owner,
        stack.as_ref(),
    );
    let r#ref = serialize_by_value_id(id);
    request
        .written_debug_objects
        .set(io_node_object, r#ref.clone());
    r#ref
}

fn emit_typed_array_chunk(
    request: &Rc<Request>,
    id: f64,
    tag: &str,
    typed_array: &'static Unknown,
    debug: bool,
) {
    // Safety: what's given is an `ArrayBufferView`.
    let view: &'static ArrayBufferView = unsafe { js::cast(Some(typed_array)) };
    if ENABLE_TAINT.with(|&enabled| enabled)
        && TAINT_REGISTRY_BYTE_LENGTHS
            .with(|&byte_lengths| byte_lengths.unwrap())
            .has(&view.byte_length())
    {
        // If we have had any tainted values of this length, we check
        // to see if these bytes matches any entries in the registry.
        let tainted = TAINT_REGISTRY_VALUES
            .with(|&values| values.unwrap())
            .get(&EntryValue::String(binary_to_comparable_string(view)));
        if let Some(tainted) = tainted {
            throw_taint_violation(&tainted.message);
        }
    }
    if debug {
        request
            .pending_debug_chunks
            .set(request.pending_debug_chunks.get() + 1.0);
    } else {
        request
            .pending_chunks
            .set(request.pending_chunks.get() + 1.0); // Extra chunk for the header.
    }
    // TODO: Convert to little endian if that's not the server default.
    let binary_chunk = typed_array_to_binary_chunk(typed_array);
    let binary_length = byte_length_of_binary_chunk(binary_chunk);
    let row = js::number::to_string(id, Some(16))
        + ":"
        + tag
        + &js::number::to_string(binary_length, Some(16))
        + ",";
    let header_chunk = string_to_chunk(&row);
    // Push a NEXT_TWO_CHUNKS_ARE_ATOMIC sentinel before the header so that
    // flushCompletedChunks can write the header and binary chunks atomically.
    // Otherwise, if the destination's backpressure flips between the two writes,
    // the content chunk would be stranded at the front of the queue and the next
    // drain would emit Import or Hint chunks between the header and the content —
    // and the Flight Client would frame those intervening bytes as this row's
    // content.
    let next_two_chunks_are_atomic = NEXT_TWO_CHUNKS_ARE_ATOMIC.with(|&marker| marker);
    if __DEV__ && debug {
        request.completed_debug_chunks.borrow_mut().extend([
            next_two_chunks_are_atomic,
            header_chunk,
            binary_chunk,
        ]);
    } else {
        request.completed_regular_chunks.borrow_mut().extend([
            next_two_chunks_are_atomic,
            header_chunk,
            binary_chunk,
        ]);
    }
}

fn emit_text_chunk(request: &Rc<Request>, id: f64, text: &str, debug: bool) {
    // $FlowFixMe[invalid-compare]
    let Some(byte_length_of_chunk) = BYTE_LENGTH_OF_CHUNK else {
        // eslint-disable-next-line react-internal/prod-error-codes
        panic!(
            "Existence of byteLengthOfChunk should have already been checked. This is a bug in React."
        );
    };
    if debug {
        request
            .pending_debug_chunks
            .set(request.pending_debug_chunks.get() + 1.0);
    } else {
        request
            .pending_chunks
            .set(request.pending_chunks.get() + 1.0); // Extra chunk for the header.
    }
    let text_chunk = string_to_chunk(text);
    let binary_length = byte_length_of_chunk(text_chunk);
    let row = js::number::to_string(id, Some(16))
        + ":T"
        + &js::number::to_string(binary_length, Some(16))
        + ",";
    let header_chunk = string_to_chunk(&row);
    // See emitTypedArrayChunk for why the pair is preceded by a sentinel.
    let next_two_chunks_are_atomic = NEXT_TWO_CHUNKS_ARE_ATOMIC.with(|&marker| marker);
    if __DEV__ && debug {
        request.completed_debug_chunks.borrow_mut().extend([
            next_two_chunks_are_atomic,
            header_chunk,
            text_chunk,
        ]);
    } else {
        request.completed_regular_chunks.borrow_mut().extend([
            next_two_chunks_are_atomic,
            header_chunk,
            text_chunk,
        ]);
    }
}

fn serialize_eval(source: &str) -> String {
    if !__DEV__ {
        // These errors should never make it into a build so we don't need to encode them in codes.json
        // eslint-disable-next-line react-internal/prod-error-codes
        panic!("serializeEval should never be called in production mode. This is a bug in React.");
    }
    "$E".to_owned() + source
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::nullable)]
    static CONSTRUCTOR_MARKER: Option<&'static Symbol> =
        if __DEV__ { Some(js::symbol::new(None)) } else { None };

    #[cfg_attr(rust_js, rust_js::nullable)]
    static DEBUG_MODEL_ROOT: Cell<Option<&'static Unknown>> = const { Cell::new(None) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static DEBUG_NO_OUTLINE: Cell<Option<&'static Unknown>> = const { Cell::new(None) };
}

/// A class instance's prototype, as a debug model marks it:
/// `{$$typeof: CONSTRUCTOR_MARKER, constructor}`.
struct ConstructorReference {
    #[cfg_attr(rust_js, rust_js::name = "$$typeof")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    r#typeof: Option<&'static Symbol>,
    constructor: Option<&'static Unknown>,
}

unsafe impl js::Defined for ConstructorReference {}

/// The DEV-only set of what's never deferred.
fn do_not_limit_add(value: &JsObject) {
    DO_NOT_LIMIT.with(|&do_not_limit| {
        // Safety: the set is made in DEV, where debug models are rendered.
        unsafe { do_not_limit.unwrap_unchecked() }.add(value);
    });
}

fn do_not_limit_has(value: &JsObject) -> bool {
    DO_NOT_LIMIT.with(|&do_not_limit| {
        // Safety: the set is made in DEV, where debug models are rendered.
        unsafe { do_not_limit.unwrap_unchecked() }.has(value)
    })
}

// This is a forked version of renderModel which should never error, never suspend and is limited
// in the depth it can encode.
fn render_debug_model(
    request: &Rc<Request>,
    counter: &Rc<Counter>,
    parent: &'static Unknown,
    parent_property_name: &str,
    value: Option<&'static ReactClientValue>,
) -> Option<&'static ReactJsonValue> {
    if matches!(js::classify(value), Kind::Null) {
        return js::null();
    }

    // Special Symbol, that's very common.
    if SymbolFor::from_value(value) == Some(REACT_ELEMENT_TYPE) {
        return Some(js::unknown_of("$".to_owned()));
    }

    if let Some(value) = value
        && let Some(object) = js::classify(Some(value)).object()
    {
        if is_client_reference(value) {
            // We actually have this value on the client so we could import it.
            // This might be confusing though because on the Server it won't actually
            // be this value, so if you're debugging client references maybe you'd be
            // better with a place holder.
            return Some(js::unknown_of(serialize_debug_client_reference(
                request,
                parent,
                parent_property_name,
                value,
            )));
        }
        if CONSTRUCTOR_MARKER.with(|&constructor_marker| {
            js::strict_equals(js::get(value, "$$typeof"), constructor_marker)
        }) {
            let constructor = js::get(value, "constructor");
            // Safety: a marked constructor is a function, an object.
            let constructor_object: &'static JsObject = unsafe { js::cast(constructor) };
            let mut r#ref = request.written_debug_objects.get(constructor_object);
            if r#ref.is_none() {
                let id = outline_debug_model(request, counter, constructor);
                r#ref = Some(serialize_by_value_id(id));
            }
            return Some(js::unknown_of(
                "$P".to_owned() + &js::string::slice(&r#ref.unwrap_or_default(), 1.0, None),
            ));
        }

        if let Some(temporary_references) = request.temporary_references
            // Safety: a temporary reference set is keyed by any object.
            && let Some(temp_ref) =
                resolve_temporary_reference(temporary_references, unsafe {
                    js::cast(Some(value))
                })
        {
            return Some(js::unknown_of(serialize_temporary_reference(
                request, &temp_ref,
            )));
        }

        let written_debug_objects = request.written_debug_objects;
        let existing_debug_reference = written_debug_objects.get(object);
        if let Some(existing_debug_reference) = existing_debug_reference {
            if DEBUG_MODEL_ROOT.with(|root| js::strict_equals(root.get(), Some(value))) {
                // This is the ID we're currently emitting so we need to write it
                // once but if we discover it again, we refer to it by id.
                DEBUG_MODEL_ROOT.set(None);
            } else {
                // We've already emitted this as a debug object. We favor that version if available.
                return Some(js::unknown_of(existing_debug_reference));
            }
        } else if !parent_property_name.contains(':') {
            // TODO: If the property name contains a colon, we don't dedupe. Escape instead.
            // Safety: a parent is an object or an array.
            let parent_object: &'static JsObject = unsafe { js::cast(Some(parent)) };
            let parent_reference = written_debug_objects.get(parent_object);
            if let Some(parent_reference) = parent_reference {
                // If the parent has a reference, we can refer to this object indirectly
                // through the property name inside that parent.
                if counter.object_limit.get() <= 0.0 && !do_not_limit_has(object) {
                    // If we are going to defer this, don't dedupe it since then we'd dedupe it to be
                    // deferred in future reference.
                    return Some(js::unknown_of(serialize_deferred_object(request, value)));
                }

                let mut property_name = parent_property_name;
                if is_array(Some(parent))
                    && SymbolFor::from_value(js::get(parent, &0u32)) == Some(REACT_ELEMENT_TYPE)
                {
                    // For elements, we've converted it to an array but we'll have converted
                    // it back to an element before we read the references so the property
                    // needs to be aliased.
                    match parent_property_name {
                        "1" => property_name = "type",
                        "2" => property_name = "key",
                        "3" => property_name = "props",
                        "4" => property_name = "_owner",
                        _ => {}
                    }
                }
                written_debug_objects.set(object, parent_reference + ":" + property_name);
            } else if !DEBUG_NO_OUTLINE
                .with(|no_outline| js::strict_equals(no_outline.get(), Some(value)))
            {
                // If this isn't the root object (like meta data) and we don't have an id for it, outline
                // it so that we can dedupe it by reference later.
                // $FlowFixMe[method-unbinding]
                if matches!(js::classify(js::get(value, "then")), Kind::Function(_)) {
                    // If this is a Promise we're going to assign it an external ID anyway which can be deduped.
                    // Safety: what has a `then` function is a thenable.
                    let thenable: &'static Thenable = unsafe { js::cast(Some(value)) };
                    return Some(js::unknown_of(serialize_debug_thenable(
                        request, counter, thenable,
                    )));
                } else {
                    let outlined_id = outline_debug_model(request, counter, Some(value));
                    return Some(js::unknown_of(serialize_by_value_id(outlined_id)));
                }
            }
        }

        let written_objects = request.written_objects;
        let existing_reference = written_objects.get(object);
        if let Some(existing_reference) = existing_reference {
            // We've already emitted this as a real object, so we can refer to that by its existing reference.
            // This might be slightly different serialization than what renderDebugModel would've produced.
            return Some(js::unknown_of(existing_reference));
        }

        if counter.object_limit.get() <= 0.0 && !do_not_limit_has(object) {
            // We've reached our max number of objects to serialize across the wire so we serialize this
            // as a marker so that the client can error or lazy load this when accessed by the console.
            return Some(js::unknown_of(serialize_deferred_object(request, value)));
        }

        counter.object_limit.set(counter.object_limit.get() - 1.0);

        if let Some(deferred_debug_objects) = &request.deferred_debug_objects {
            let deferred_id = deferred_debug_objects.existing.get(value);
            // We earlier deferred this same object. We're now going to eagerly emit it so let's emit it
            // at the same ID that we already used to refer to it.
            if let Some(deferred_id) = deferred_id {
                deferred_debug_objects.existing.delete(value);
                deferred_debug_objects.retained.delete(&deferred_id);
                emit_outlined_debug_model_chunk(request, deferred_id, counter, Some(value));
                return Some(js::unknown_of(serialize_by_value_id(deferred_id)));
            }
        }

        match SymbolFor::from_value(js::get(value, "$$typeof")) {
            Some(REACT_ELEMENT_TYPE) => {
                // Safety: what's of `REACT_ELEMENT_TYPE` is an element.
                let element: &'static ReactElement = unsafe { js::cast(Some(value)) };

                if let Some(owner) = element.owner {
                    // Safety: an element's owner is a component's info.
                    outline_component_info(request, unsafe { js::cast(Some(owner)) });
                }
                if let Some(r#type) = js::classify(element.r#type).object() {
                    // If the type is an object it can get cut off which shouldn't happen here.
                    do_not_limit_add(r#type);
                }
                if let Some(key) = &element.key
                    && let Some(key) = js::classify(Some(js::unknown(key))).object()
                {
                    // This should never happen but just in case.
                    do_not_limit_add(key);
                }
                // Safety: an element's props are an object.
                do_not_limit_add(unsafe { js::cast(Some(element.props)) });
                if let Some(owner) = element.owner {
                    // Safety: an element's owner is a component's info, an object.
                    do_not_limit_add(unsafe { js::cast(Some(owner)) });
                }

                let mut debug_stack: Option<&'static Unknown> = js::null();
                if let Some(element_debug_stack) = element.debug_stack {
                    // Outline the debug stack so that it doesn't get cut off.
                    let stack = js::unknown_of(filter_stack_trace(
                        request,
                        &parse_stack_trace(element_debug_stack, 1),
                    ));
                    // Safety: a stack trace is an array, an object.
                    do_not_limit_add(unsafe { js::cast(Some(stack)) });
                    if let Kind::Array(callsites) = js::classify(Some(stack)) {
                        for callsite in callsites {
                            // Safety: a call site is an array, an object.
                            do_not_limit_add(unsafe { js::cast(*callsite) });
                        }
                    }
                    debug_stack = Some(stack);
                }
                return Some(js::unknown_of(vec![
                    Some(js::unknown(&REACT_ELEMENT_TYPE)),
                    element.r#type,
                    element.key.as_ref().map(js::unknown),
                    Some(element.props),
                    element.owner,
                    debug_stack,
                    js::get(element.store.get(), "validated"),
                ]));
            }
            Some(REACT_LAZY_TYPE) => {
                // To avoid actually initializing a lazy causing a side-effect, we make
                // some assumptions about the structure of the payload even though
                // that's not really part of the contract. In practice, this is really
                // just coming from React.lazy helper or Flight.
                let payload = js::get(value, "_payload");

                if let Some(payload) = payload
                    && js::classify(Some(payload)).object().is_some()
                {
                    // React.lazy constructor
                    let status = js::get(payload, "_status");
                    // -1 Uninitialized and 0 Pending break.
                    if js::strict_equals(status, 1.0) {
                        // 1 Resolved
                        let id = outline_debug_model(request, counter, js::get(payload, "_result"));
                        return Some(js::unknown_of(serialize_lazy_id(id)));
                    } else if js::strict_equals(status, 2.0) {
                        // 2 Rejected
                        // We don't log these errors since they didn't actually throw into
                        // Flight.
                        let digest = "";
                        let id = request.next_chunk_id.get();
                        request.next_chunk_id.set(id + 1.0);
                        emit_error_chunk(
                            request,
                            id,
                            digest,
                            js::get(payload, "_result"),
                            true,
                            None,
                        );
                        return Some(js::unknown_of(serialize_lazy_id(id)));
                    }

                    // React Flight
                    if let Kind::String(status) = js::classify(js::get(payload, "status")) {
                        match status {
                            "pending" | "blocked" | "resolved_model" => {
                                // The value is an uninitialized model from the Flight client.
                                // It's not very useful to emit that.
                            }
                            "resolved_module" => {
                                // The value is client reference metadata from the Flight client.
                                // It's likely for SSR, so we choose not to emit it.
                            }
                            "fulfilled" => {
                                let id = outline_debug_model(
                                    request,
                                    counter,
                                    js::get(payload, "value"),
                                );
                                return Some(js::unknown_of(serialize_lazy_id(id)));
                            }
                            "rejected" => {
                                // We don't log these errors since they didn't actually throw into
                                // Flight.
                                let digest = "";
                                let id = request.next_chunk_id.get();
                                request.next_chunk_id.set(id + 1.0);
                                emit_error_chunk(
                                    request,
                                    id,
                                    digest,
                                    js::get(payload, "reason"),
                                    true,
                                    None,
                                );
                                return Some(js::unknown_of(serialize_lazy_id(id)));
                            }
                            _ => {}
                        }
                    }
                }

                // We couldn't emit a resolved or rejected value synchronously. For now,
                // we emit this as a halted chunk. TODO: We could maybe also handle
                // pending lazy debug models like we do in serializeDebugThenable,
                // if/when we determine that it's worth the added complexity.
                request
                    .pending_debug_chunks
                    .set(request.pending_debug_chunks.get() + 1.0);
                let id = request.next_chunk_id.get();
                request.next_chunk_id.set(id + 1.0);
                emit_debug_halt_chunk(request, id);
                return Some(js::unknown_of(serialize_lazy_id(id)));
            }
            _ => {}
        }

        // $FlowFixMe[method-unbinding]
        if matches!(js::classify(js::get(value, "then")), Kind::Function(_)) {
            // Safety: what has a `then` function is a thenable.
            let thenable: &'static Thenable = unsafe { js::cast(Some(value)) };
            return Some(js::unknown_of(serialize_debug_thenable(
                request, counter, thenable,
            )));
        }

        if let Kind::Array(items) = js::classify(Some(value)) {
            if items.len() > 200 && !do_not_limit_has(object) {
                // Defer large arrays. They're heavy to serialize.
                // TODO: Consider doing the same for objects with many properties too.
                return Some(js::unknown_of(serialize_deferred_object(request, value)));
            }
            return Some(value);
        }

        if js::instance_of(value, js::class::<Date>()) {
            return Some(js::unknown_of(serialize_date(value)));
        }
        if js::instance_of(value, js::class::<Map<Unknown, Unknown>>()) {
            return Some(js::unknown_of(serialize_debug_map(request, counter, value)));
        }
        if js::instance_of(value, js::class::<js::Set<Unknown>>()) {
            return Some(js::unknown_of(serialize_debug_set(request, counter, value)));
        }
        // TODO: FormData is not available in old Node. Remove the typeof later.
        if js::is_function(webapi::form_data::new)
            && js::instance_of(value, js::class::<FormData>())
        {
            // Safety: an instance of FormData is one.
            return Some(js::unknown_of(serialize_debug_form_data(request, unsafe {
                js::cast(Some(value))
            })));
        }
        if js::instance_of(value, js::class::<Error>()) {
            return Some(js::unknown_of(serialize_debug_error_value(
                request, counter, value,
            )));
        }
        if js::instance_of(value, js::class::<ArrayBuffer>()) {
            // Safety: an instance of ArrayBuffer is one.
            let buffer: &'static ArrayBuffer = unsafe { js::cast(Some(value)) };
            return Some(js::unknown_of(serialize_debug_typed_array(
                request,
                "A",
                js::unknown(js::uint8_array::new_with_buffer(buffer, None, None)),
            )));
        }
        if js::instance_of(value, js::class::<Int8Array>()) {
            // char
            return Some(js::unknown_of(serialize_debug_typed_array(
                request, "O", value,
            )));
        }
        if js::instance_of(value, js::class::<Uint8Array>()) {
            // unsigned char
            return Some(js::unknown_of(serialize_debug_typed_array(
                request, "o", value,
            )));
        }
        if js::instance_of(value, js::class::<Uint8ClampedArray>()) {
            // unsigned clamped char
            return Some(js::unknown_of(serialize_debug_typed_array(
                request, "U", value,
            )));
        }
        if js::instance_of(value, js::class::<Int16Array>()) {
            // sort
            return Some(js::unknown_of(serialize_debug_typed_array(
                request, "S", value,
            )));
        }
        if js::instance_of(value, js::class::<Uint16Array>()) {
            // unsigned short
            return Some(js::unknown_of(serialize_debug_typed_array(
                request, "s", value,
            )));
        }
        if js::instance_of(value, js::class::<Int32Array>()) {
            // long
            return Some(js::unknown_of(serialize_debug_typed_array(
                request, "L", value,
            )));
        }
        if js::instance_of(value, js::class::<Uint32Array>()) {
            // unsigned long
            return Some(js::unknown_of(serialize_debug_typed_array(
                request, "l", value,
            )));
        }
        if js::instance_of(value, js::class::<Float32Array>()) {
            // float
            return Some(js::unknown_of(serialize_debug_typed_array(
                request, "G", value,
            )));
        }
        if js::instance_of(value, js::class::<Float64Array>()) {
            // double
            return Some(js::unknown_of(serialize_debug_typed_array(
                request, "g", value,
            )));
        }
        if js::instance_of(value, js::class::<BigInt64Array>()) {
            // number
            return Some(js::unknown_of(serialize_debug_typed_array(
                request, "M", value,
            )));
        }
        if js::instance_of(value, js::class::<BigUint64Array>()) {
            // unsigned number
            // We use "m" instead of "n" since JSON can start with "null"
            return Some(js::unknown_of(serialize_debug_typed_array(
                request, "m", value,
            )));
        }
        if js::instance_of(value, js::class::<DataView>()) {
            return Some(js::unknown_of(serialize_debug_typed_array(
                request, "V", value,
            )));
        }
        // TODO: Blob is not available in old Node. Remove the typeof check later.
        if js::is_function(webapi::blob::new) && js::instance_of(value, js::class::<Blob>()) {
            // Safety: an instance of Blob is one.
            return Some(js::unknown_of(serialize_debug_blob(request, unsafe {
                js::cast(Some(value))
            })));
        }

        let iterator_fn = get_iterator_fn(Some(value));
        if iterator_fn.is_some() {
            return Some(js::unknown_of(js::array::from::<Option<&'static Unknown>>(
                value,
            )));
        }

        let proto = GET_PROTOTYPE_OF.with(|get_prototype_of| get_prototype_of(Some(value)));
        if let Some(proto) = proto
            && proto != js::object::prototype
        {
            let instance_description = js::object::create(js::null(), None);
            for prop_name in js::keys_in(value) {
                if HAS_OWN_PROPERTY
                    .with(|has_own_property| has_own_property.call(value, &prop_name))
                    || is_getter(Some(proto), &prop_name)
                {
                    // We intentionally invoke getters on the prototype to read any enumerable getters.
                    js::set(instance_description, &prop_name, js::get(value, &prop_name));
                }
            }
            let constructor = js::get(proto, "constructor");
            if matches!(js::classify(constructor), Kind::Function(_))
                && js::strict_equals(js::get(constructor, "prototype"), Some(proto))
            {
                // This is a simple class shape.
                if HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(value, ""))
                    || is_getter(Some(proto), "")
                {
                    // This object already has an empty property name. Skip encoding its prototype.
                } else {
                    js::set(
                        instance_description,
                        "",
                        js::unknown_of(ConstructorReference {
                            r#typeof: CONSTRUCTOR_MARKER
                                .with(|&constructor_marker| constructor_marker),
                            constructor,
                        }),
                    );
                }
            }
            return Some(instance_description);
        }

        // $FlowFixMe[incompatible-type]
        return Some(value);
    }

    match js::classify(value) {
        Kind::String(value) => {
            if js::string::length(value) > 1000000 {
                // Reconstructing a multi-megabyte string on the client blocks the main
                // thread for too long. We omit the actual value and send a placeholder
                // instead.
                return Some(js::unknown_of(format!(
                    "This string of length {} has been omitted by React to avoid sending too much data from the server.",
                    js::string::length(value)
                )));
            }
            if js::string::length(value) >= 1024 {
                // Large strings are counted towards the object limit.
                if counter.object_limit.get() <= 0.0 {
                    // We've reached our max number of objects to serialize across the wire so we serialize this
                    // as a marker so that the client can error or lazy load this when accessed by the console.
                    return Some(js::unknown_of(serialize_deferred_object(
                        request,
                        js::unknown(value),
                    )));
                }
                counter.object_limit.set(counter.object_limit.get() - 1.0);
                // For large strings, we encode them outside the JSON payload so that we
                // don't have to double encode and double parse the strings. This can also
                // be more compact in case the string has a lot of escaped characters.
                return Some(js::unknown_of(serialize_debug_large_text_string(
                    request, value,
                )));
            }
            Some(js::unknown_of(escape_string_value(value)))
        }
        Kind::Bool(_) => value,
        Kind::Number(value) => Some(serialize_number(value)),
        Kind::Undefined => Some(js::unknown_of(serialize_undefined())),
        Kind::Function(function) => {
            let value = js::unknown(function);
            if is_client_reference(value) {
                return Some(js::unknown_of(serialize_debug_client_reference(
                    request,
                    parent,
                    parent_property_name,
                    value,
                )));
            }
            if let Some(temporary_references) = request.temporary_references
                // Safety: a temporary reference set is keyed by any object.
                && let Some(temp_ref) =
                    resolve_temporary_reference(temporary_references, unsafe {
                        js::cast(Some(value))
                    })
            {
                return Some(js::unknown_of(serialize_temporary_reference(
                    request, &temp_ref,
                )));
            }

            // Serialize the body of the function as an eval so it can be printed.
            let written_debug_objects = request.written_debug_objects;
            let existing_reference = written_debug_objects.get(function::object(function));
            if let Some(existing_reference) = existing_reference {
                // We've already emitted this function, so we can
                // just refer to that by its existing reference.
                return Some(js::unknown_of(existing_reference));
            }

            // $FlowFixMe[method-unbinding]
            let function_body: String = function::to_string(function);

            let name = js::get(value, "name");
            let serialized_value =
                serialize_eval(&if let Kind::String(name) = js::classify(name) {
                    "Object.defineProperty(".to_owned()
                        + &function_body
                        + ",\"name\",{value:"
                        + &js::json::stringify(name)
                        + "})"
                } else {
                    "(".to_owned() + &function_body + ")"
                });
            request
                .pending_debug_chunks
                .set(request.pending_debug_chunks.get() + 1.0);
            let id = request.next_chunk_id.get();
            request.next_chunk_id.set(id + 1.0);
            let processed_chunk = encode_reference_chunk(request, id, &serialized_value);
            request
                .completed_debug_chunks
                .borrow_mut()
                .push(processed_chunk);
            let reference = serialize_by_value_id(id);
            written_debug_objects.set(function::object(function), reference.clone());
            Some(js::unknown_of(reference))
        }
        Kind::Symbol(value) => {
            let written_symbols = request.written_symbols;
            let existing_id = written_symbols.get(value);
            if let Some(existing_id) = existing_id {
                return Some(js::unknown_of(serialize_by_value_id(existing_id)));
            }
            // $FlowFixMe[incompatible-type] `description` might be undefined
            let name = value.description();
            // We use the Symbol.for version if it's not a global symbol. Close enough.
            request
                .pending_chunks
                .set(request.pending_chunks.get() + 1.0);
            let symbol_id = request.next_chunk_id.get();
            request.next_chunk_id.set(symbol_id + 1.0);
            emit_symbol_chunk(request, symbol_id, name.as_deref());
            Some(js::unknown_of(serialize_by_value_id(symbol_id)))
        }
        Kind::BigInt(value) => Some(js::unknown_of(serialize_big_int(value))),
        _ => Some(js::unknown_of(
            "unknown type ".to_owned() + &js::type_of(value),
        )),
    }
}

/// What a debug model's replacer gives of a value that threw: the message
/// it's sent as.
fn unknown_value_message(message: Option<&'static Unknown>) -> String {
    "Unknown Value: React could not send it from the server.\n".to_owned() + &js::string(message)
}

fn serialize_debug_model(
    request: &Rc<Request>,
    object_limit: f64,
    model: Option<&'static Unknown>,
) -> String {
    let counter = Rc::new(Counter {
        object_limit: Cell::new(object_limit),
    });

    let replacer = function::with_this(
        |this: Option<&'static Unknown>,
         (parent_property_name, _value): (&'static str, Option<&'static ReactClientValue>)|
         -> Option<&'static ReactJsonValue> {
            match catch_unwind(AssertUnwindSafe(|| {
                // By-pass toJSON and use the original value.
                // $FlowFixMe[incompatible-use]
                let original_value = js::get(this, parent_property_name);
                // Safety: JSON.stringify calls its replacer on the object or
                // array holding the key.
                let parent: &'static Unknown = unsafe { js::cast(this) };
                render_debug_model(
                    request,
                    &counter,
                    parent,
                    parent_property_name,
                    original_value,
                )
            })) {
                Ok(json) => json,
                Err(x) => Some(js::unknown_of(unknown_value_message(js::get(
                    thrown_value_of(&*x),
                    "message",
                )))),
            }
        },
    );

    let prev_no_outline = DEBUG_NO_OUTLINE.replace(model);
    // $FlowFixMe[incompatible-cast] stringify can return null
    // $FlowFixMe[incompatible-type]
    let json = match js::json::stringify_with(&model, replacer, 0) {
        Ok(json) => json.unwrap_or_default(),
        // $FlowFixMe[incompatible-cast] stringify can return null
        Err(x) => js::json::stringify(unknown_value_message(js::get(x, "message")).as_str()),
    };
    DEBUG_NO_OUTLINE.set(prev_no_outline);
    json
}

fn emit_outlined_debug_model_chunk(
    request: &Rc<Request>,
    id: f64,
    counter: &Rc<Counter>,
    model: Option<&'static ReactClientValue>,
) {
    if !__DEV__ {
        // These errors should never make it into a build so we don't need to encode them in codes.json
        // eslint-disable-next-line react-internal/prod-error-codes
        panic!(
            "emitOutlinedDebugModel should never be called in production mode. This is a bug in React."
        );
    }

    if let Some(model) = js::classify(model).object() {
        // We can't limit outlined values.
        do_not_limit_add(model);
    }

    let replacer = function::with_this(
        |this: Option<&'static Unknown>,
         (parent_property_name, _value): (&'static str, Option<&'static ReactClientValue>)|
         -> Option<&'static ReactJsonValue> {
            match catch_unwind(AssertUnwindSafe(|| {
                // By-pass toJSON and use the original value.
                // $FlowFixMe[incompatible-use]
                let original_value = js::get(this, parent_property_name);
                // Safety: JSON.stringify calls its replacer on the object or
                // array holding the key.
                let parent: &'static Unknown = unsafe { js::cast(this) };
                render_debug_model(
                    request,
                    counter,
                    parent,
                    parent_property_name,
                    original_value,
                )
            })) {
                Ok(json) => json,
                Err(x) => Some(js::unknown_of(unknown_value_message(js::get(
                    thrown_value_of(&*x),
                    "message",
                )))),
            }
        },
    );

    let prev_model_root = DEBUG_MODEL_ROOT.replace(model);
    if let Some(model) = js::classify(model).object() {
        // Future references can refer to this object by id.
        request
            .written_debug_objects
            .set(model, serialize_by_value_id(id));
    }
    // $FlowFixMe[incompatible-type] stringify can return null
    let json = match js::json::stringify_with(&model, replacer, 0) {
        Ok(json) => json.unwrap_or_default(),
        // $FlowFixMe[incompatible-type] stringify can return null
        Err(x) => js::json::stringify(unknown_value_message(js::get(x, "message")).as_str()),
    };
    DEBUG_MODEL_ROOT.set(prev_model_root);

    let row = js::number::to_string(id, Some(16)) + ":" + &json + "\n";
    let processed_chunk = string_to_chunk(&row);
    request
        .completed_debug_chunks
        .borrow_mut()
        .push(processed_chunk);
}

fn outline_debug_model(
    request: &Rc<Request>,
    counter: &Rc<Counter>,
    model: Option<&'static ReactClientValue>,
) -> f64 {
    if !__DEV__ {
        // These errors should never make it into a build so we don't need to encode them in codes.json
        // eslint-disable-next-line react-internal/prod-error-codes
        panic!(
            "outlineDebugModel should never be called in production mode. This is a bug in React."
        );
    }

    let id = request.next_chunk_id.get();
    request.next_chunk_id.set(id + 1.0);
    request
        .pending_debug_chunks
        .set(request.pending_debug_chunks.get() + 1.0);
    emit_outlined_debug_model_chunk(request, id, counter, model);
    id
}

fn emit_console_chunk(
    request: &Rc<Request>,
    method_name: &str,
    owner: Option<&'static ReactComponentInfo>,
    env: &str,
    stack_trace: &ReactStackTrace,
    args: &'static Unknown,
) {
    if !__DEV__ {
        // These errors should never make it into a build so we don't need to encode them in codes.json
        // eslint-disable-next-line react-internal/prod-error-codes
        panic!(
            "emitConsoleChunk should never be called in production mode. This is a bug in React."
        );
    }

    // Ensure the owner is already outlined.
    if let Some(owner) = owner {
        outline_component_info(request, owner);
    }

    let mut payload: Vec<Option<&'static Unknown>> = vec![
        Some(js::unknown_of(method_name.to_owned())),
        Some(js::unknown_of(stack_trace.clone())),
        owner.map(js::unknown),
        Some(js::unknown_of(env.to_owned())),
    ];
    // $FlowFixMe[method-unbinding]
    // Safety: a console call's arguments are an array.
    let args: &'static [Option<&'static Unknown>] = unsafe { js::cast(Some(args)) };
    payload.extend_from_slice(args);
    let object_limit = if request.deferred_debug_objects.is_none() {
        500.0
    } else {
        10.0
    };
    let mut json = serialize_debug_model(
        request,
        object_limit + stack_trace.len() as f64,
        Some(js::unknown_of(payload)),
    );
    if !json.starts_with('[') {
        // This looks like an error. Try a simpler object.
        json = serialize_debug_model(
            request,
            10.0 + stack_trace.len() as f64,
            Some(js::unknown_of(vec![
                Some(js::unknown_of(method_name.to_owned())),
                Some(js::unknown_of(stack_trace.clone())),
                owner.map(js::unknown),
                Some(js::unknown_of(env.to_owned())),
                Some(js::unknown(
                    "Unknown Value: React could not send it from the server.",
                )),
            ])),
        );
    }
    let row = ":W".to_owned() + &json + "\n";
    let processed_chunk = string_to_chunk(&row);
    request
        .completed_debug_chunks
        .borrow_mut()
        .push(processed_chunk);
}

fn emit_time_origin_chunk(request: &Rc<Request>, time_origin: f64) {
    // We emit the time origin once. All ReactTimeInfo timestamps later in the stream
    // are relative to this time origin. This allows for more compact number encoding
    // and lower precision loss.
    request
        .pending_debug_chunks
        .set(request.pending_debug_chunks.get() + 1.0);
    let row = ":N".to_owned() + &js::concat_text(time_origin) + "\n";
    let processed_chunk = string_to_chunk(&row);
    // TODO: Move to its own priority queue.
    request
        .completed_debug_chunks
        .borrow_mut()
        .push(processed_chunk);
}

/// An awaited entry's debug info, without what can't be serialized: its
/// `debugTask` and `debugStack`. Its `env`, `owner` and `stack` are written
/// after, as upstream does.
struct AsyncDebugInfo {
    awaited: &'static ReactIOInfo,
}

unsafe impl js::Defined for AsyncDebugInfo {}

fn forward_debug_info(request: &Rc<Request>, task: &Rc<Task>, debug_info: &'static ReactDebugInfo) {
    let id = task.id;
    for &info in debug_info {
        if let Kind::Number(time) = js::classify(js::get(info, "time")) {
            // When forwarding time we need to ensure to convert it to the time space of the payload.
            // We clamp the time to the starting render of the current component. It's as if it took
            // no time to render and await if we reuse cached content.
            mark_operation_end_time(request, task, time);
        } else if matches!(js::classify(js::get(info, "name")), Kind::String(_)) {
            // We outline this model eagerly so that we can refer to by reference as an owner.
            // If we had a smarter way to dedupe we might not have to do this if there ends up
            // being no references to this as an owner.
            // Safety: an entry with a `name` is a component's info.
            outline_component_info(request, unsafe { js::cast(info) });
            // Emit a reference to the outlined one.
            request
                .pending_chunks
                .set(request.pending_chunks.get() + 1.0);
            // Safety: an entry is an object.
            emit_debug_chunk(request, id, unsafe { js::cast(info) });
        } else if js::truthy(js::get(info, "awaited")) {
            // Safety: an entry that awaited is an async info.
            let info: &'static ReactAsyncInfo = unsafe { js::cast(info) };
            let io_info: &'static ReactIOInfo = &info.awaited;
            if io_info.end.get() <= request.time_origin.get() {
                // This was already resolved when we started this render. It must have been some
                // externally cached data. We exclude that information but we keep components and
                // awaits that happened inside this render but might have been deduped within the
                // render.
            } else {
                // Outline the IO info in case the same I/O is awaited in more than one place.
                outline_io_info(request, io_info);
                // Ensure the owner is already outlined.
                if let Some(owner) = info.owner.get() {
                    outline_component_info(request, owner);
                }
                // We can't serialize the ConsoleTask/Error objects so we need to omit them before serializing.
                let debug_stack;
                if info.stack.borrow().is_none()
                    && let Some(info_debug_stack) = info.debug_stack.get()
                {
                    // If we have a debugStack but no parsed stack we should parse it.
                    debug_stack = Some(filter_stack_trace(
                        request,
                        &parse_stack_trace(info_debug_stack, 1),
                    ));
                } else {
                    debug_stack = info.stack.borrow().clone();
                }
                let debug_async_info = js::unknown_of(AsyncDebugInfo { awaited: io_info });
                if let Some(env) = &info.env {
                    // $FlowFixMe[cannot-write]
                    js::set(debug_async_info, "env", env.as_str());
                } else {
                    // If we're forwarding IO info from this environment, an empty env is effectively the "client" side.
                    // The "client" from the perspective of our client will be this current environment.
                    // $FlowFixMe[cannot-write]
                    js::set(debug_async_info, "env", (request.environment_name)());
                }
                if let Some(owner) = info.owner.get() {
                    // $FlowFixMe[cannot-write]
                    js::set(debug_async_info, "owner", owner);
                }
                if let Some(debug_stack) = debug_stack {
                    // $FlowFixMe[cannot-write]
                    js::set(debug_async_info, "stack", debug_stack);
                }
                request
                    .pending_chunks
                    .set(request.pending_chunks.get() + 1.0);
                emit_debug_chunk(request, id, debug_async_info);
            }
        } else {
            request
                .pending_chunks
                .set(request.pending_chunks.get() + 1.0);
            // Safety: an entry is an object.
            emit_debug_chunk(request, id, unsafe { js::cast(info) });
        }
    }
}

fn forward_debug_info_from_thenable(
    request: &Rc<Request>,
    task: &Rc<Task>,
    thenable: &'static Thenable,
    owner: Option<&'static ReactComponentInfo>, // DEV-only
    stack: Option<&'static Error>,              // DEV-only
) {
    let mut debug_info: Option<&'static ReactDebugInfo> = None;
    if __DEV__ {
        // If this came from Flight, forward any debug info into this new row.
        // Safety: a thenable's `_debugInfo` is its debug info, if any.
        debug_info = unsafe { js::cast(js::get(thenable, "_debugInfo")) };
        if let Some(debug_info) = debug_info {
            forward_debug_info(request, task, debug_info);
        }
    }
    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
        let sequence = get_async_sequence_from_promise(js::unknown(thenable));
        if let Some(sequence) = sequence {
            emit_async_sequence(request, task, &sequence, debug_info, owner, stack);
        }
    }
}

fn forward_debug_info_from_current_context(
    request: &Rc<Request>,
    task: &Rc<Task>,
    thenable: &'static Thenable,
) {
    let mut debug_info: Option<&'static ReactDebugInfo> = None;
    if __DEV__ {
        // If this came from Flight, forward any debug info into this new row.
        // Safety: a thenable's `_debugInfo` is its debug info, if any.
        debug_info = unsafe { js::cast(js::get(thenable, "_debugInfo")) };
        if let Some(debug_info) = debug_info {
            forward_debug_info(request, task, debug_info);
        }
    }
    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
        let sequence = get_current_async_sequence();
        if let Some(sequence) = sequence {
            emit_async_sequence(request, task, &sequence, debug_info, None, None);
        }
    }
}

/// An await of what an aborted task waited on: the promise node itself,
/// deduped by this reference, as its I/O.
struct AbortedAsyncInfo {
    awaited: Rc<AsyncSequence>, // This is deduped by this reference.
    env: String,
}

unsafe impl js::Defined for AbortedAsyncInfo {}

fn forward_debug_info_from_aborted_task(request: &Rc<Request>, task: &Rc<Task>) {
    // If a task is aborted, we can still include as much debug info as we can from the
    // value that we have so far.
    let model = task.model.get();
    let Some(model) = model.filter(|&model| js::classify(Some(model)).object().is_some()) else {
        return;
    };
    let mut debug_info: Option<&'static ReactDebugInfo> = None;
    if __DEV__ {
        // If this came from Flight, forward any debug info into this new row.
        // Safety: a model's `_debugInfo` is its debug info, if any.
        debug_info = unsafe { js::cast(js::get(model, "_debugInfo")) };
        if let Some(debug_info) = debug_info {
            forward_debug_info(request, task, debug_info);
        }
    }
    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
        let mut thenable: Option<&'static Thenable> = None;
        if matches!(js::classify(js::get(model, "then")), Kind::Function(_)) {
            // Safety: what has a `then` function is a thenable.
            thenable = Some(unsafe { js::cast(Some(model)) });
        } else if SymbolFor::from_value(js::get(model, "$$typeof")) == Some(REACT_LAZY_TYPE) {
            let payload = js::get(model, "_payload");
            if matches!(js::classify(js::get(payload, "then")), Kind::Function(_)) {
                // Safety: what has a `then` function is a thenable.
                thenable = Some(unsafe { js::cast(payload) });
            }
        }
        if let Some(thenable) = thenable {
            let sequence = get_async_sequence_from_promise(js::unknown(thenable));
            if let Some(sequence) = sequence {
                let mut node = Rc::clone(&sequence);
                while node.tag.get() == UNRESOLVED_AWAIT_NODE
                    && let Some(awaited) = { node.awaited.borrow().clone() }
                {
                    // See if any of the dependencies are resolved yet.
                    node = awaited;
                }
                if node.tag.get() == UNRESOLVED_PROMISE_NODE {
                    // We don't know what Promise will eventually end up resolving this Promise and if it
                    // was I/O at all. However, we assume that it was some kind of I/O since it didn't
                    // complete in time before aborting.
                    // The best we can do is try to emit the stack of where this Promise was created.
                    serialize_io_node(request, &node, None);
                    request
                        .pending_chunks
                        .set(request.pending_chunks.get() + 1.0);
                    let env = (request.environment_name)();
                    let async_info = js::unknown_of(AbortedAsyncInfo { awaited: node, env });
                    // We don't have a start time for this await but in case there was no start time emitted
                    // we need to include something. TODO: We should maybe ideally track the time when we
                    // called .then() but without updating the task.time field since that's used for the cutoff.
                    advance_task_time(request, task, task.time.get());
                    emit_debug_chunk(request, task.id, async_info);
                } else {
                    // We have a resolved Promise. Its debug info can include both awaited data and rejected
                    // promises after the abort.
                    emit_async_sequence(request, task, &sequence, debug_info, None, None);
                }
            }
        }
    }
}

fn emit_timing_chunk(request: &Rc<Request>, id: f64, timestamp: f64) {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled) || !ENABLE_COMPONENT_PERFORMANCE_TRACK {
        return;
    }
    request
        .pending_chunks
        .set(request.pending_chunks.get() + 1.0);
    let relative_timestamp = timestamp - request.time_origin.get();
    let json = "{\"time\":".to_owned() + &js::concat_text(relative_timestamp) + "}";
    if request.debug_destination.get().is_some() {
        // Outline the actual timing information to the debug channel.
        let outlined_id = request.next_chunk_id.get();
        request.next_chunk_id.set(outlined_id + 1.0);
        let debug_row = js::number::to_string(outlined_id, Some(16)) + ":" + &json + "\n";
        request
            .pending_debug_chunks
            .set(request.pending_debug_chunks.get() + 1.0);
        request
            .completed_debug_chunks
            .borrow_mut()
            .push(string_to_chunk(&debug_row));
        let row = serialize_row_header("D", id)
            + "\"$"
            + &js::number::to_string(outlined_id, Some(16))
            + "\"\n";
        request
            .completed_regular_chunks
            .borrow_mut()
            .push(string_to_chunk(&row));
    } else {
        let row = serialize_row_header("D", id) + &json + "\n";
        request
            .completed_regular_chunks
            .borrow_mut()
            .push(string_to_chunk(&row));
    }
}

fn advance_task_time(request: &Rc<Request>, task: &Rc<Task>, timestamp: f64) {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
        return;
    }
    // Emits a timing chunk, if the new timestamp is higher than the previous timestamp of this task.
    if timestamp > task.time.get() {
        emit_timing_chunk(request, task.id, timestamp);
        task.time.set(timestamp);
    } else if !task.timed.get() {
        // If it wasn't timed before, e.g. an outlined object, we need to emit the first timestamp and
        // it is now timed.
        emit_timing_chunk(request, task.id, task.time.get());
    }
    task.timed.set(true);
}

fn mark_operation_end_time(request: &Rc<Request>, task: &Rc<Task>, timestamp: f64) {
    if !ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
        return;
    }
    // This is like advanceTaskTime() but always emits a timing chunk even if it doesn't advance.
    // This ensures that the end time of the previous entry isn't implied to be the start of the next one.
    if request.status.get() == ABORTING && timestamp > request.abort_time.get() {
        // If we're aborting then we don't emit any end times that happened after.
        return;
    }
    if timestamp > task.time.get() {
        emit_timing_chunk(request, task.id, timestamp);
        task.time.set(timestamp);
    } else {
        emit_timing_chunk(request, task.id, task.time.get());
    }
}

/// A stream config's `byteLengthOfChunk`, of a chunk's encoded length, or
fn emit_chunk(request: &Rc<Request>, task: &Rc<Task>, value: Option<&'static ReactClientValue>) {
    let id = task.id;
    // For certain types we have special types, we typically outlined them but
    // we can emit them directly for this row instead of through an indirection.
    // $FlowFixMe[invalid-compare]
    if let Kind::String(text) = js::classify(value)
        && BYTE_LENGTH_OF_CHUNK.is_some()
    {
        if ENABLE_TAINT.with(|&enabled| enabled) {
            let tainted = TAINT_REGISTRY_VALUES
                .with(|&values| values.unwrap().get(&EntryValue::String(text.to_owned())));
            if let Some(tainted) = tainted {
                throw_taint_violation(&tainted.message);
            }
        }
        emit_text_chunk(request, id, text, false);
        return;
    }
    if ENABLE_FLIGHT_OBJECT_REFERENCES.with(|&enabled| enabled)
        && let Some(object) = js::classify(value).object()
        && is_server_reference(js::unknown(object))
    {
        // Streamed values reach this function before model serialization. Keep
        // registered binary objects opaque instead of emitting their bytes.
        let json = stringify(Some(js::unknown(
            serialize_server_reference(request, js::unknown(object)).as_str(),
        )));
        emit_model_chunk(request, id, &json);
        return;
    }
    if let Some(value) = value {
        if js::instance_of(value, js::class::<ArrayBuffer>()) {
            let buffer: &'static ArrayBuffer = unsafe { js::cast(Some(value)) };
            emit_typed_array_chunk(
                request,
                id,
                "A",
                js::unknown(js::uint8_array::new_with_buffer(buffer, None, None)),
                false,
            );
            return;
        }
        if js::instance_of(value, js::class::<Int8Array>()) {
            // char
            emit_typed_array_chunk(request, id, "O", value, false);
            return;
        }
        if js::instance_of(value, js::class::<Uint8Array>()) {
            // unsigned char
            emit_typed_array_chunk(request, id, "o", value, false);
            return;
        }
        if js::instance_of(value, js::class::<Uint8ClampedArray>()) {
            // unsigned clamped char
            emit_typed_array_chunk(request, id, "U", value, false);
            return;
        }
        if js::instance_of(value, js::class::<Int16Array>()) {
            // sort
            emit_typed_array_chunk(request, id, "S", value, false);
            return;
        }
        if js::instance_of(value, js::class::<Uint16Array>()) {
            // unsigned short
            emit_typed_array_chunk(request, id, "s", value, false);
            return;
        }
        if js::instance_of(value, js::class::<Int32Array>()) {
            // long
            emit_typed_array_chunk(request, id, "L", value, false);
            return;
        }
        if js::instance_of(value, js::class::<Uint32Array>()) {
            // unsigned long
            emit_typed_array_chunk(request, id, "l", value, false);
            return;
        }
        if js::instance_of(value, js::class::<Float32Array>()) {
            // float
            emit_typed_array_chunk(request, id, "G", value, false);
            return;
        }
        if js::instance_of(value, js::class::<Float64Array>()) {
            // double
            emit_typed_array_chunk(request, id, "g", value, false);
            return;
        }
        if js::instance_of(value, js::class::<BigInt64Array>()) {
            // number
            emit_typed_array_chunk(request, id, "M", value, false);
            return;
        }
        if js::instance_of(value, js::class::<BigUint64Array>()) {
            // unsigned number
            // We use "m" instead of "n" since JSON can start with "null"
            emit_typed_array_chunk(request, id, "m", value, false);
            return;
        }
        if js::instance_of(value, js::class::<DataView>()) {
            emit_typed_array_chunk(request, id, "V", value, false);
            return;
        }
    }
    // For anything else we need to try to serialize it using JSON.
    // We resolve the model tree first in pure JS to avoid the C++->JS boundary
    // overhead of JSON.stringify's replacer callback.
    // `{'': value}`, an object literal of a key that's no identifier, can't
    // be written, so the key is set on an empty object.
    let root = js::unknown(js::object::spread(None::<&Unknown>));
    js::set(root, "", value);
    let resolved_model = resolve_model(request, task, root, "", value);
    // $FlowFixMe[incompatible-type] stringify can return null for undefined but we never do
    let json = stringify(resolved_model);
    emit_model_chunk(request, task.id, &json);
}

fn errored_task(request: &Rc<Request>, task: &Rc<Task>, error: Option<&'static Unknown>) {
    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && task.timed.get() {
        mark_operation_end_time(request, task, performance.now());
    }
    task.status.set(ERRORED);
    let digest = log_recoverable_error(request, error, Some(task));
    emit_error_chunk(
        request,
        task.id,
        &digest,
        error,
        false,
        if __DEV__ {
            unsafe { js::cast::<Option<&'static ReactComponentInfo>>(task.debug_owner.get()) }
        } else {
            None
        },
    );
    request.abortable_tasks.borrow_mut().remove(task);
    call_on_all_ready_if_ready(request);
}

thread_local! {
    static EMPTY_ROOT: &'static Unknown = js::unknown(js::object::spread(None::<&Unknown>));
}

fn retry_task(request: &Rc<Request>, task: &Rc<Task>) {
    if task.status.get() != PENDING {
        // We completed this by other means before we had a chance to retry it.
        return;
    }

    let prev_can_emit_debug_info = CAN_EMIT_DEBUG_INFO.get();
    task.status.set(RENDERING);

    // We stash the outer parent size so we can restore it when we exit.
    let parent_serialized_size = SERIALIZED_SIZE.get();
    // We don't reset the serialized size counter from reentry because that indicates that we
    // are outlining a model and we actually want to include that size into the parent since
    // it will still block the parent row. It only restores to zero at the top of the stack.
    let result = catch_unwind(AssertUnwindSafe(|| {
        // Track the root so we know that we have to emit this object even though it
        // already has an ID. This is needed because we might see this object twice
        // in the same resolveModel walk if it is cyclic.
        MODEL_ROOT.set(task.model.get());

        if __DEV__ {
            // Track that we can emit debug info for the current task.
            CAN_EMIT_DEBUG_INFO.set(true);
        }

        // We call the destructive form that mutates this task. That way if something
        // suspends again, we can reuse the same task instead of spawning a new one.
        let resolved_model = render_model_destructive(
            request,
            task,
            EMPTY_ROOT.with(|&empty_root| empty_root),
            "",
            task.model.get(),
        );

        if __DEV__ {
            // We're now past rendering this task and future renders will spawn new tasks for their
            // debug info.
            CAN_EMIT_DEBUG_INFO.set(false);
        }

        // Track the root again for the resolved object.
        MODEL_ROOT.set(resolved_model);

        // The keyPath resets at any terminal child node.
        task.key_path.replace(None);
        task.implicit_slot.set(false);

        if __DEV__ {
            let current_env = (request.environment_name)();
            if current_env != *task.environment_name.borrow() {
                request
                    .pending_chunks
                    .set(request.pending_chunks.get() + 1.0);
                // The environment changed since we last emitted any debug information for this
                // task. We emit an entry that just includes the environment name change.
                emit_debug_chunk(
                    request,
                    task.id,
                    js::unknown_of(ReactEnvironmentInfo { env: current_env }),
                );
            }
        }
        // We've finished rendering. Log the end time.
        if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && task.timed.get() {
            mark_operation_end_time(request, task, performance.now());
        }

        if let Some(resolved_object) = js::classify(resolved_model).object() {
            // We're not in a contextual place here so we can refer to this object by this ID for
            // any future references.
            request
                .written_objects
                .set(resolved_object, serialize_by_value_id(task.id));

            // Object might contain unresolved values like additional elements.
            // This is simulating what the JSON loop would do if this was part of it.
            emit_chunk(request, task, resolved_model);
        } else {
            // If the value is a string, it means it's a terminal value and we already escaped it.
            // We don't need to escape it again so it's not passed through resolveModel.
            // $FlowFixMe[incompatible-type] stringify can return null for undefined but we never do
            let json = stringify(resolved_model);
            emit_model_chunk(request, task.id, &json);
        }

        task.status.set(COMPLETED);
        request.abortable_tasks.borrow_mut().remove(task);
        call_on_all_ready_if_ready(request);
    }));
    let result = match result {
        Ok(()) => Ok(()),
        Err(thrown_value) => catch_unwind(AssertUnwindSafe(|| {
            if request.status.get() == ABORTING {
                request.abortable_tasks.borrow_mut().remove(task);
                task.status.set(PENDING);
                if request.r#type == PRERENDER {
                    // When aborting a prerener with halt semantics we don't emit
                    // anything into the slot for a task that aborts, it remains unresolved
                    halt_task(task, request);
                    finish_halted_task(task, request);
                } else {
                    // Otherwise we emit an error chunk into the task slot.
                    let error_id: f64 = unsafe { js::cast(request.fatal_error.get()) };
                    abort_task(task, request, error_id);
                    finish_aborted_task(task, request, error_id);
                }
                return;
            }

            let thrown_value = thrown_value_of(&*thrown_value);
            let x = if is_suspense_exception(thrown_value) {
                // This is a special type of exception used for Suspense. For historical
                // reasons, the rest of the Suspense implementation expects the thrown
                // value to be a thenable, because before `use` existed that was the
                // (unstable) API for suspending. This implementation detail can change
                // later, once we deprecate the old API in favor of `use`.
                Some(js::unknown(get_suspended_thenable()))
            } else {
                thrown_value
            };
            // $FlowFixMe[invalid-compare]
            if js::type_of(x) == "object"
                && !matches!(js::classify(x), Kind::Null)
                // $FlowFixMe[method-unbinding]
                && js::type_of(js::get(x, "then")) == "function"
            {
                // Something suspended again, let's pick it back up later.
                task.status.set(PENDING);
                task.thenable_state
                    .replace(Some(get_thenable_state_after_suspending()));
                // We've asserted that x is a thenable above
                let thenable: &'static Thenable = unsafe { js::cast(x) };
                let (resolve, reject) = ping_of(request, task);
                thenable.then(resolve, reject);
                return;
            }
            errored_task(request, task, x);
        })),
    };
    if __DEV__ {
        CAN_EMIT_DEBUG_INFO.set(prev_can_emit_debug_info);
    }
    SERIALIZED_SIZE.set(parent_serialized_size);
    if let Err(thrown_value) = result {
        resume_unwind(thrown_value);
    }
}

fn try_stream_task(request: &Rc<Request>, task: &Rc<Task>) {
    // This is used to try to emit something synchronously but if it suspends,
    // we emit a reference to a new outlined task immediately instead.
    let prev_can_emit_debug_info = CAN_EMIT_DEBUG_INFO.get();
    if __DEV__ {
        // We can't emit debug into to a specific row of a stream task. Instead we leave
        // it false so that we instead outline the row to get a new canEmitDebugInfo if needed.
        CAN_EMIT_DEBUG_INFO.set(false);
    }
    let parent_serialized_size = SERIALIZED_SIZE.get();
    let result = catch_unwind(AssertUnwindSafe(|| {
        emit_chunk(request, task, task.model.get());
    }));
    SERIALIZED_SIZE.set(parent_serialized_size);
    if __DEV__ {
        CAN_EMIT_DEBUG_INFO.set(prev_can_emit_debug_info);
    }
    if let Err(thrown_value) = result {
        resume_unwind(thrown_value);
    }
}

fn perform_work(request: &Rc<Request>) {
    mark_async_sequence_root_task();

    let prev_dispatcher = REACT_SHARED_INTERNALS.with(|internals| internals.h.get());
    REACT_SHARED_INTERNALS.with(|internals| {
        internals.h.set(Some(unsafe {
            js::cast(Some(js::unknown(&HOOKS_DISPATCHER)))
        }));
    });
    let prev_request = CURRENT_REQUEST.replace(Some(Rc::clone(request)));
    prepare_to_use_hooks_for_request(request);

    let result = catch_unwind(AssertUnwindSafe(|| {
        let pinged_tasks = request.pinged_tasks.replace(vec![]);
        for task in &pinged_tasks {
            retry_task(request, task);
        }
        flush_completed_chunks(request);
    }));
    let result = match result {
        Ok(()) => Ok(()),
        Err(error) => catch_unwind(AssertUnwindSafe(|| {
            let error = thrown_value_of(&*error);
            log_recoverable_error(request, error, None);
            fatal_error(request, error);
        })),
    };
    REACT_SHARED_INTERNALS.with(|internals| internals.h.set(prev_dispatcher));
    reset_hooks_for_request();
    CURRENT_REQUEST.set(prev_request);
    if let Err(thrown_value) = result {
        resume_unwind(thrown_value);
    }
}

fn abort_task(task: &Rc<Task>, _request: &Rc<Request>, _error_id: f64) {
    if task.status.get() != PENDING {
        // If this is already completed/errored we don't abort it.
        // If currently rendering it will be aborted by the render
        return;
    }
    task.status.set(ABORTED);
}

fn finish_aborted_task(task: &Rc<Task>, request: &Rc<Request>, error_id: f64) {
    if task.status.get() != ABORTED {
        return;
    }
    forward_debug_info_from_aborted_task(request, task);
    // Track when we aborted this task as its end time.
    if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) && task.timed.get() {
        mark_operation_end_time(request, task, request.abort_time.get());
    }
    // Instead of emitting an error per task.id, we emit a model that only
    // has a single value referencing the error.
    let r#ref = serialize_by_value_id(error_id);
    let processed_chunk = encode_reference_chunk(request, task.id, &r#ref);
    request
        .completed_error_chunks
        .borrow_mut()
        .push(processed_chunk);
}

// "Halting" a task means finishing it without emitting anything into its
// slot: the reference is intentionally left unfulfilled and never resolves
// on the client. This is how an aborted prerender leaves its pending work.
// It's also the same outcome as a weak-pending thenable that never settles
// (see serializeThenable) — halting is initiated by the request aborting,
// weakness by the value itself.
fn halt_task(task: &Rc<Task>, _request: &Rc<Request>) {
    if task.status.get() != PENDING {
        // If this is already completed/errored we don't abort it.
        // If currently rendering it will be aborted by the render
        return;
    }
    task.status.set(ABORTED);
}

fn finish_halted_task(task: &Rc<Task>, request: &Rc<Request>) {
    if task.status.get() != ABORTED {
        return;
    }
    forward_debug_info_from_aborted_task(request, task);
    // We don't actually emit anything for this task id because we are intentionally
    // leaving the reference unfulfilled.
    request
        .pending_chunks
        .set(request.pending_chunks.get() - 1.0);
}

fn is_next_two_chunks_are_atomic(item: &Chunk) -> bool {
    NEXT_TWO_CHUNKS_ARE_ATOMIC.with(|&marker| js::strict_equals(item, marker))
}

fn flush_completed_chunks(request: &Rc<Request>) {
    if __DEV__ && let Some(debug_destination) = request.debug_destination.get() {
        begin_writing(debug_destination);
        let result = catch_unwind(AssertUnwindSafe(|| {
            let debug_chunks = &request.completed_debug_chunks;
            let mut i = 0;
            while i < debug_chunks.borrow().len() {
                let item = debug_chunks.borrow()[i];
                if is_next_two_chunks_are_atomic(item) {
                    if i + 2 >= debug_chunks.borrow().len() {
                        panic!("A chunk pair is incomplete. This is a bug in React.");
                    }
                    request
                        .pending_debug_chunks
                        .set(request.pending_debug_chunks.get() - 2.0);
                    let header_chunk = debug_chunks.borrow()[i + 1];
                    write_chunk(debug_destination, header_chunk);
                    let content_chunk = debug_chunks.borrow()[i + 2];
                    write_chunk(debug_destination, content_chunk);
                    i += 2;
                } else {
                    request
                        .pending_debug_chunks
                        .set(request.pending_debug_chunks.get() - 1.0);
                    write_chunk(debug_destination, item);
                }
                i += 1;
            }
            debug_chunks.borrow_mut().drain(0..i);
        }));
        complete_writing(debug_destination);
        if let Err(thrown_value) = result {
            resume_unwind(thrown_value);
        }
        flush_buffered(debug_destination);
    }
    let destination = request.destination.get();
    if let Some(destination) = destination {
        begin_writing(destination);
        let result = catch_unwind(AssertUnwindSafe(|| {
            // We emit module chunks first in the stream so that
            // they can be preloaded as early as possible.
            let imports_chunks = &request.completed_import_chunks;
            let mut i = 0;
            while i < imports_chunks.borrow().len() {
                request
                    .pending_chunks
                    .set(request.pending_chunks.get() - 1.0);
                let chunk = imports_chunks.borrow()[i];
                let keep_writing = write_chunk_and_return(destination, chunk);
                if !keep_writing {
                    request.destination.set(None);
                    i += 1;
                    break;
                }
                i += 1;
            }
            imports_chunks.borrow_mut().drain(0..i);

            // Next comes hints.
            let hint_chunks = &request.completed_hint_chunks;
            i = 0;
            while i < hint_chunks.borrow().len() {
                let chunk = hint_chunks.borrow()[i];
                let keep_writing = write_chunk_and_return(destination, chunk);
                if !keep_writing {
                    request.destination.set(None);
                    i += 1;
                    break;
                }
                i += 1;
            }
            hint_chunks.borrow_mut().drain(0..i);

            // Debug meta data comes before the model data because it will often end up blocking the model from
            // completing since the JSX will reference the debug data.
            if __DEV__ && request.debug_destination.get().is_none() {
                let debug_chunks = &request.completed_debug_chunks;
                i = 0;
                while i < debug_chunks.borrow().len() {
                    let item = debug_chunks.borrow()[i];
                    let keep_writing: bool;
                    if is_next_two_chunks_are_atomic(item) {
                        if i + 2 >= debug_chunks.borrow().len() {
                            panic!("A chunk pair is incomplete. This is a bug in React.");
                        }
                        request
                            .pending_debug_chunks
                            .set(request.pending_debug_chunks.get() - 2.0);
                        let header_chunk = debug_chunks.borrow()[i + 1];
                        write_chunk(destination, header_chunk);
                        let content_chunk = debug_chunks.borrow()[i + 2];
                        keep_writing = write_chunk_and_return(destination, content_chunk);
                        i += 2;
                    } else {
                        request
                            .pending_debug_chunks
                            .set(request.pending_debug_chunks.get() - 1.0);
                        keep_writing = write_chunk_and_return(destination, item);
                    }
                    if !keep_writing {
                        request.destination.set(None);
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                debug_chunks.borrow_mut().drain(0..i);
            }

            // Next comes model data.
            let regular_chunks = &request.completed_regular_chunks;
            i = 0;
            while i < regular_chunks.borrow().len() {
                let item = regular_chunks.borrow()[i];
                let keep_writing: bool;
                if is_next_two_chunks_are_atomic(item) {
                    if i + 2 >= regular_chunks.borrow().len() {
                        panic!("A chunk pair is incomplete. This is a bug in React.");
                    }
                    request
                        .pending_chunks
                        .set(request.pending_chunks.get() - 2.0);
                    let header_chunk = regular_chunks.borrow()[i + 1];
                    write_chunk(destination, header_chunk);
                    let content_chunk = regular_chunks.borrow()[i + 2];
                    keep_writing = write_chunk_and_return(destination, content_chunk);
                    i += 2;
                } else {
                    request
                        .pending_chunks
                        .set(request.pending_chunks.get() - 1.0);
                    keep_writing = write_chunk_and_return(destination, item);
                }
                if !keep_writing {
                    request.destination.set(None);
                    i += 1;
                    break;
                }
                i += 1;
            }
            regular_chunks.borrow_mut().drain(0..i);

            // Finally, errors are sent. The idea is that it's ok to delay
            // any error messages and prioritize display of other parts of
            // the page.
            let error_chunks = &request.completed_error_chunks;
            i = 0;
            while i < error_chunks.borrow().len() {
                request
                    .pending_chunks
                    .set(request.pending_chunks.get() - 1.0);
                let chunk = error_chunks.borrow()[i];
                let keep_writing = write_chunk_and_return(destination, chunk);
                if !keep_writing {
                    request.destination.set(None);
                    i += 1;
                    break;
                }
                i += 1;
            }
            error_chunks.borrow_mut().drain(0..i);
        }));
        request.flush_scheduled.set(false);
        complete_writing(destination);
        if let Err(thrown_value) = result {
            resume_unwind(thrown_value);
        }
        flush_buffered(destination);
    }
    if request.pending_chunks.get() == 0.0 {
        // There are no pending chunks left, so the render is complete and its cache
        // signal is aborted here. Debug chunks can still be pending, but they carry
        // development-only instrumentation rather than the render's output.
        //
        // This runs before the stream bookkeeping below, because that bookkeeping
        // can close the main stream and set the status to CLOSED while debug chunks
        // are outstanding. The abort only happens below ABORTING, so a later flush
        // would skip it. Repeated flushes are safe, because aborting an aborted
        // controller does nothing a second time.
        //
        // The taint queue stays untouched here. Debug chunks are checked against
        // the taint registry as they are written, and a deferred debug object can
        // be written long after this point.
        if request.status.get() < ABORTING {
            let abort_reason = js::error::new(
                Some(
                    "This render completed successfully. All cacheSignals are now aborted to allow clean up of any unused resources.",
                ),
                None,
            );
            request.cache_controller.abort(Some(&abort_reason));
        }
        if __DEV__ {
            let debug_destination = request.debug_destination.get();
            if request.pending_debug_chunks.get() == 0.0 {
                // Continue fully closing both streams.
                if let Some(debug_destination) = debug_destination {
                    close(debug_destination);
                    request.debug_destination.set(None);
                }
            } else if debug_destination.is_none() {
                // We still have debug information to write.
                // We'll continue writing on this stream so nothing closes.
                return;
            } else {
                // We'll close the main stream but keep the debug stream open.
                // TODO: If this destination is not currently flowing we'll not close it when it resumes flowing.
                // We should keep a separate status for this.
                if let Some(destination) = request.destination.get() {
                    request.status.set(CLOSED);
                    close(destination);
                    request.destination.set(None);
                }
                return;
            }
        }
        // We're done.
        if ENABLE_TAINT.with(|&enabled| enabled) {
            cleanup_taint_queue(request);
        }
        if let Some(destination) = request.destination.get() {
            request.status.set(CLOSED);
            close(destination);
            request.destination.set(None);
        }
        if __DEV__ && let Some(debug_destination) = request.debug_destination.get() {
            close(debug_destination);
            request.debug_destination.set(None);
        }
    }
}

pub fn start_work(request: &Rc<Request>) {
    request
        .flush_scheduled
        .set(request.destination.get().is_some());
    // $FlowFixMe[constant-condition]
    if SUPPORTS_REQUEST_STORAGE {
        let request = Rc::clone(request);
        schedule_microtask(move || {
            REQUEST_STORAGE.run(Some(Rc::clone(&request)), || perform_work(&request));
        });
    } else {
        let request = Rc::clone(request);
        schedule_microtask(move || perform_work(&request));
    }
    let request = Rc::clone(request);
    schedule_work(move || {
        if request.status.get() == OPENING {
            request.status.set(OPEN);
        }
    });
}

fn enqueue_flush(request: &Rc<Request>) {
    if !request.flush_scheduled.get()
        // If there are pinged tasks we are going to flush anyway after work completes
        && request.pinged_tasks.borrow().is_empty()
        // If there is no destination there is nothing we can flush to. A flush will
        // happen when we start flowing again
        && (request.destination.get().is_some()
            || (__DEV__ && request.debug_destination.get().is_some()))
    {
        request.flush_scheduled.set(true);
        // Unlike startWork and pingTask we intetionally use scheduleWork
        // here even during prerenders to allow as much batching as possible
        let request = Rc::clone(request);
        schedule_work(move || {
            request.flush_scheduled.set(false);
            flush_completed_chunks(&request);
        });
    }
}

fn call_on_all_ready_if_ready(request: &Rc<Request>) {
    if request.abortable_tasks.borrow().is_empty() {
        let on_all_ready = &request.on_all_ready;
        on_all_ready();
    }
}

pub fn start_flowing(request: &Rc<Request>, destination: &'static Destination) {
    if request.status.get() == CLOSING {
        request.status.set(CLOSED);
        close_with_error(destination, request.fatal_error.get());
        return;
    }
    if request.status.get() == CLOSED {
        return;
    }
    if request.destination.get().is_some() {
        // We're already flowing.
        return;
    }
    request.destination.set(Some(destination));
    if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
        flush_completed_chunks(request);
    })) {
        let error = thrown_value_of(&*error);
        log_recoverable_error(request, error, None);
        fatal_error(request, error);
    }
}

pub fn start_flowing_debug(request: &Rc<Request>, debug_destination: &'static Destination) {
    if request.status.get() == CLOSING {
        request.status.set(CLOSED);
        close_with_error(debug_destination, request.fatal_error.get());
        return;
    }
    if request.status.get() == CLOSED {
        return;
    }
    if request.debug_destination.get().is_some() {
        // We're already flowing.
        return;
    }
    request.debug_destination.set(Some(debug_destination));
    if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
        flush_completed_chunks(request);
    })) {
        let error = thrown_value_of(&*error);
        log_recoverable_error(request, error, None);
        fatal_error(request, error);
    }
}

pub fn stop_flowing(request: &Rc<Request>) {
    request.destination.set(None);
}

fn finish_halt(request: &Rc<Request>, aborted_tasks: &AbortSet) {
    if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
        // Each task finished may make more, which are pending and so not
        // finished: a copy of the set is what's visited.
        let tasks: Vec<Rc<Task>> = aborted_tasks.borrow().iter().cloned().collect();
        for task in &tasks {
            finish_halted_task(task, request);
        }
        let on_all_ready = &request.on_all_ready;
        on_all_ready();
        flush_completed_chunks(request);
    })) {
        let error = thrown_value_of(&*error);
        log_recoverable_error(request, error, None);
        fatal_error(request, error);
    }
}

fn finish_abort(request: &Rc<Request>, aborted_tasks: &AbortSet, error_id: f64) {
    if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
        // Each task finished may make more, which are pending and so not
        // finished: a copy of the set is what's visited.
        let tasks: Vec<Rc<Task>> = aborted_tasks.borrow().iter().cloned().collect();
        for task in &tasks {
            finish_aborted_task(task, request, error_id);
        }
        let on_all_ready = &request.on_all_ready;
        on_all_ready();
        flush_completed_chunks(request);
    })) {
        let error = thrown_value_of(&*error);
        log_recoverable_error(request, error, None);
        fatal_error(request, error);
    }
}

// Aborts the request when the caller's signal aborts. The cache controller's
// signal bounds the listener's lifetime, so the runtime removes the listener as
// soon as that signal aborts. The cache controller aborts at every point that
// ends the render: a fatal error, the completion of the flush loop, and abort()
// itself. From any of those points on, abort() returns early, so the listener
// has nothing left to do.
//
// The listener has to be removed, because it would otherwise keep the whole
// Request reachable for as long as the caller's signal lives. A composite
// signal from AbortSignal.any() is itself retained by the runtime while it has
// any abort listener attached.
//
// A request whose stream is neither consumed nor cancelled never ends, so its
// listener stays attached for as long as the caller's signal lives.
pub fn attach_abort_signal(request: &Rc<Request>, signal: &'static AbortSignal) {
    if signal.aborted() {
        abort(request, signal.reason());
        return;
    }
    let cache_signal = request.cache_controller.signal();
    let request = Rc::clone(request);
    signal.add_event_listener(
        webapi::events::Abort,
        move |_: &Event| {
            abort(&request, signal.reason());
        },
        Some(&AddEventListenerOptions {
            capture: None,
            passive: None,
            once: None,
            signal: Some(cache_signal),
        }),
    );
}

pub fn abort(request: &Rc<Request>, reason: Option<&'static Unknown>) {
    // We define any status below OPEN as OPEN equivalent
    if request.status.get() > OPEN {
        return;
    }
    if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
        request.status.set(ABORTING);
        if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
            request.abort_time.set(performance.now());
        }
        request
            .cache_controller
            .abort(reason.map(|reason| reason as &dyn IntoUnknown));
        let abortable_tasks = Rc::clone(&request.abortable_tasks);
        if !abortable_tasks.borrow().is_empty() {
            if request.r#type == PRERENDER {
                // When prerendering with halt semantics we simply halt the task
                // and leave the reference unfulfilled.
                for task in abortable_tasks.borrow().iter() {
                    halt_task(task, request);
                }
                let request = Rc::clone(request);
                schedule_work(move || finish_halt(&request, &abortable_tasks));
            } else {
                let error = if matches!(js::classify(reason), Kind::Undefined) {
                    Some(js::unknown(js::error::new(
                        Some("The render was aborted by the server without a reason."),
                        None,
                    )))
                } else if js::type_of(reason) == "object"
                    && !matches!(js::classify(reason), Kind::Null)
                    && js::type_of(js::get(reason, "then")) == "function"
                {
                    Some(js::unknown(js::error::new(
                        Some("The render was aborted by the server with a promise."),
                        None,
                    )))
                } else {
                    reason
                };
                let digest = log_recoverable_error(request, error, None);
                // When rendering we produce a shared error chunk and then
                // fulfill each task with a reference to that chunk.
                let error_id = request.next_chunk_id.get();
                request.next_chunk_id.set(error_id + 1.0);
                request.fatal_error.set(Some(js::unknown_of(error_id)));
                request
                    .pending_chunks
                    .set(request.pending_chunks.get() + 1.0);
                emit_error_chunk(request, error_id, &digest, error, false, None);
                for task in abortable_tasks.borrow().iter() {
                    abort_task(task, request, error_id);
                }
                let request = Rc::clone(request);
                schedule_work(move || finish_abort(&request, &abortable_tasks, error_id));
            }
        } else {
            let on_all_ready = &request.on_all_ready;
            on_all_ready();
            flush_completed_chunks(request);
        }
    })) {
        let error = thrown_value_of(&*error);
        log_recoverable_error(request, error, None);
        fatal_error(request, error);
    }
}

fn from_hex(str: &str) -> f64 {
    js::parse_int(str, Some(16))
}

pub fn resolve_debug_message(request: &Rc<Request>, message: &str) {
    if !__DEV__ {
        // These errors should never make it into a build so we don't need to encode them in codes.json
        // eslint-disable-next-line react-internal/prod-error-codes
        panic!(
            "resolveDebugMessage should never be called in production mode. This is a bug in React."
        );
    }
    let Some(deferred_debug_objects) = &request.deferred_debug_objects else {
        panic!(
            "resolveDebugMessage/closeDebugChannel should not be called for a Request that wasn't kept alive. This is a bug in React."
        );
    };
    if message.is_empty() {
        close_debug_channel(request);
        return;
    }
    // This function lets the client ask for more data lazily through the debug channel.
    let command = js::string::char_code_at(message, 0);
    let ids: Vec<f64> = js::string::split(&js::string::slice(message, 2.0, None), ",")
        .iter()
        .map(|id| from_hex(id))
        .collect();
    match command as u32 {
        82 /* "R" */ => {
            // Release IDs
            for &id in &ids {
                let retained_value = deferred_debug_objects.retained.get(&id);
                if let Some(retained_value) = retained_value {
                    // We're no longer blocked on this. We won't emit it.
                    request
                        .pending_debug_chunks
                        .set(request.pending_debug_chunks.get() - 1.0);
                    deferred_debug_objects.retained.delete(&id);
                    deferred_debug_objects.existing.delete(retained_value);
                    enqueue_flush(request);
                }
            }
        }
        81 /* "Q" */ => {
            // Query IDs
            for &id in &ids {
                let retained_value = deferred_debug_objects.retained.get(&id);
                if let Some(retained_value) = retained_value {
                    // If we still have this object, and haven't emitted it before, emit it on the stream.
                    let counter = Rc::new(Counter {
                        object_limit: Cell::new(10.0),
                    });
                    deferred_debug_objects.retained.delete(&id);
                    deferred_debug_objects.existing.delete(retained_value);
                    emit_outlined_debug_model_chunk(request, id, &counter, Some(retained_value));
                    enqueue_flush(request);
                }
            }
        }
        80 /* "P" */ => {
            // Query Promise IDs
            for &id in &ids {
                let retained_value = deferred_debug_objects.retained.get(&id);
                if let Some(retained_value) = retained_value {
                    // If we still have this Promise, and haven't emitted it before, wait for it
                    // and then emit it on the stream.
                    let counter = Rc::new(Counter {
                        object_limit: Cell::new(10.0),
                    });
                    deferred_debug_objects.retained.delete(&id);
                    emit_requested_debug_thenable(request, id, &counter, unsafe {
                        js::cast(Some(retained_value))
                    });
                }
            }
        }
        _ => {
            panic!("Unknown command. The debugChannel was not wired up properly.");
        }
    }
}

pub fn close_debug_channel(request: &Rc<Request>) {
    if !__DEV__ {
        // These errors should never make it into a build so we don't need to encode them in codes.json
        // eslint-disable-next-line react-internal/prod-error-codes
        panic!(
            "closeDebugChannel should never be called in production mode. This is a bug in React."
        );
    }
    // This clears all remaining deferred objects, potentially resulting in the completion of the Request.
    let Some(deferred_debug_objects) = &request.deferred_debug_objects else {
        panic!(
            "resolveDebugMessage/closeDebugChannel should not be called for a Request that wasn't kept alive. This is a bug in React."
        );
    };
    deferred_debug_objects.retained.for_each(|value, id, _| {
        request
            .pending_debug_chunks
            .set(request.pending_debug_chunks.get() - 1.0);
        deferred_debug_objects.retained.delete(id);
        deferred_debug_objects.existing.delete(value);
    });
    enqueue_flush(request);
}
