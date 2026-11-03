// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, OnceCell, RefCell};
use std::rc::Rc;

use js::{
    AsyncIterator, BigInt, JsObject, Kind, PropertyDescriptor, ProxyHandler, SymbolFor, Unknown,
    WeakKey, function, json,
};
use shared::environment::__DEV__;
use shared::react_types::{ReactCustomFormAction, Thenable, Wakeable};
use webapi::{
    AbortController, AbortSignal, Blob, BlobPart, Byob, EventTargetExt, FormData, ReadableStream,
    ReadableStreamGetByobReaderOptions, ReadableStreamReadResult, console,
};

use shared::react_symbols::{
    ASYNC_ITERATOR, REACT_CONTEXT_TYPE, REACT_ELEMENT_TYPE, REACT_LAZY_TYPE, get_iterator_fn,
};

use shared::react_serialization_errors::{
    describe_object_for_error_message, is_simple_object, object_name,
};

use crate::react_flight_temporary_references::{TemporaryReferenceSet, write_temporary_reference};

use shared::is_array::is_array;

use shared::get_prototype_of::GET_PROTOTYPE_OF;

use shared::react_feature_flags::ENABLE_FLIGHT_OBJECT_REFERENCES;

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightClientConfig.js#checkEvalAvailabilityOnceDev"
)]
fn check_eval_availability_once_dev() {
    unreachable!()
}

unsafe extern "Rust" {
    #[link_name = "./src/ReactFlightClientConfig.js#usedWithSSR"]
    safe static USED_WITH_SSR: bool;
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "ObjectPrototype")]
    static OBJECT_PROTOTYPE: &'static Unknown = js::object::prototype;
}

// Passed to replyLifetimeController.abort(). Nothing reads the reason, but a
// call to abort() without one constructs an AbortError DOMException. Capturing
// the stack trace dominates that cost, and the cost grows with the depth of the
// stack.
const REPLY_ENDED: &str = "The reply ended.";

/// TypeScript's `Function`, what a server reference is.
pub type ServerFunction = dyn Fn() -> Option<&'static Unknown>;

pub type ServerReference<T> = T;

pub type CallServerCallback =
    dyn Fn(&'static Unknown, &'static Unknown) -> js::Promise<Option<&'static Unknown>>;

pub type EncodeFormActionCallback =
    dyn Fn(ServerReferenceId, &'static Thenable) -> ReactCustomFormAction;

pub type ServerReferenceId = &'static Unknown;

/// What a reply resolves to: its JSON, or a FormData of its parts.
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum ReplyBody {
    Text(String),
    Form(&'static FormData),
}

struct ServerReferenceClosure {
    id: ServerReferenceId,
    original_bind: Option<&'static Unknown>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    bound: Option<&'static Thenable>,
}

unsafe impl js::Defined for ServerReferenceClosure {}
// An object, boundCache's key.
unsafe impl WeakKey for ServerReferenceClosure {}

thread_local! {
    static KNOWN_SERVER_REFERENCES: &'static js::WeakMap<ServerFunction, Rc<ServerReferenceClosure>> =
        js::weak_map::new();

    // Server References to objects are tracked separately from function
    // references. They have no bound arguments and are never callable, so a
    // reference of one kind must not be usable where the other is expected.
    static KNOWN_SERVER_OBJECT_REFERENCES: &'static js::WeakMap<dyn WeakKey, ServerReferenceId> =
        js::weak_map::new();
}

// Serializable values
#[cfg_attr(
    rust_js,
    rust_js::types = "ServerReference<any> | string | boolean | number | null | undefined | bigint | AsyncIterable<ReactServerValue> | AsyncIterator<ReactServerValue> | Iterable<ReactServerValue> | Iterator<ReactServerValue> | ReactServerValue[] | Map<ReactServerValue, ReactServerValue> | Set<ReactServerValue> | FormData | Date | { readonly [key: string]: ReactServerValue } | Promise<ReactServerValue>"
)]
pub type ReactServerValue = Unknown;

const __PROTO__: &str = "__proto__";

#[cfg_attr(rust_js, rust_js::name = "serializeByValueID")]
fn serialize_by_value_id(id: f64) -> String {
    "$".to_owned() + &js::number::to_string(id, Some(16))
}

#[cfg_attr(rust_js, rust_js::name = "serializePromiseID")]
fn serialize_promise_id(id: f64) -> String {
    "$@".to_owned() + &js::number::to_string(id, Some(16))
}

#[cfg_attr(rust_js, rust_js::name = "serializeServerReferenceID")]
fn serialize_server_reference_id(id: f64) -> String {
    "$h".to_owned() + &js::number::to_string(id, Some(16))
}

#[cfg_attr(rust_js, rust_js::name = "serializeServerObjectReferenceID")]
fn serialize_server_object_reference_id(id: f64) -> String {
    "$H".to_owned() + &js::number::to_string(id, Some(16))
}

fn serialize_temporary_reference_marker() -> &'static str {
    "$T"
}

fn serialize_form_data_reference(id: f64) -> String {
    "$K".to_owned() + &js::number::to_string(id, Some(16))
}

/// A number as it's written: itself, or a `$` text of what JSON can't hold.
#[cfg_attr(rust_js, rust_js::untagged)]
enum SerializedNumber {
    Text(&'static str),
    Number(f64),
}

fn serialize_number(number: f64) -> SerializedNumber {
    if number.is_finite() {
        if number == 0.0 && 1.0 / number == f64::NEG_INFINITY {
            SerializedNumber::Text("$-0")
        } else {
            SerializedNumber::Number(number)
        }
    } else if number == f64::INFINITY {
        SerializedNumber::Text("$Infinity")
    } else if number == f64::NEG_INFINITY {
        SerializedNumber::Text("$-Infinity")
    } else {
        SerializedNumber::Text("$NaN")
    }
}

fn serialize_undefined() -> &'static str {
    "$undefined"
}

#[cfg_attr(rust_js, rust_js::name = "serializeDateFromDateJSON")]
fn serialize_date_from_date_json(date_json: &str) -> String {
    // JSON.stringify automatically calls Date.prototype.toJSON which calls toISOString.
    // We need only tack on a $D prefix.
    "$D".to_owned() + date_json
}

fn serialize_big_int(n: &BigInt) -> String {
    "$n".to_owned() + &n.to_string(Some(10))
}

#[cfg_attr(rust_js, rust_js::name = "serializeMapID")]
fn serialize_map_id(id: f64) -> String {
    "$Q".to_owned() + &js::number::to_string(id, Some(16))
}

#[cfg_attr(rust_js, rust_js::name = "serializeSetID")]
fn serialize_set_id(id: f64) -> String {
    "$W".to_owned() + &js::number::to_string(id, Some(16))
}

#[cfg_attr(rust_js, rust_js::name = "serializeBlobID")]
fn serialize_blob_id(id: f64) -> String {
    "$B".to_owned() + &js::number::to_string(id, Some(16))
}

#[cfg_attr(rust_js, rust_js::name = "serializeIteratorID")]
fn serialize_iterator_id(id: f64) -> String {
    "$i".to_owned() + &js::number::to_string(id, Some(16))
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

/// processReply's scope: its variables, which its nested functions,
/// this impl's methods, read and write.
struct Reply {
    form_field_prefix: String,
    temporary_references: Option<TemporaryReferenceSet>,
    on_resolve: Box<dyn Fn(ReplyBody)>,
    on_reject: Box<dyn Fn(Option<&'static Unknown>)>,
    next_part_id: Cell<f64>,
    pending_parts: Cell<f64>,
    form_data: Cell<Option<&'static FormData>>,
    written_objects: &'static js::WeakMap<dyn WeakKey, String>,
    model_root: Cell<Option<&'static Unknown>>,
    settled: Cell<bool>,
    // Bounds the abort listener that attachAbortSignal attaches to the caller's
    // signal. Null until a signal is attached, so a reply that gets no signal
    // never creates a controller.
    reply_lifetime_controller: Cell<Option<&'static AbortController>>,
    json: OnceCell<String>,
}

impl Reply {
    // Ending the lifetime makes the runtime remove the caller's abort listener.
    // Without that, the listener keeps everything this reply serialized reachable
    // for as long as the caller's signal lives, and a composite signal from
    // AbortSignal.any() is itself retained by the runtime while it has any abort
    // listener attached.
    fn end_reply_lifetime(&self) {
        if let Some(reply_lifetime_controller) = self.reply_lifetime_controller.get() {
            reply_lifetime_controller.abort(Some(&REPLY_ENDED));
        }
    }

    fn resolve(&self, value: ReplyBody) {
        self.settled.set(true);
        self.end_reply_lifetime();
        (self.on_resolve)(value);
    }

    fn reject(&self, error: Option<&'static Unknown>) {
        self.settled.set(true);
        self.end_reply_lifetime();
        (self.on_reject)(error);
    }

    fn attach_abort_signal(self: &Rc<Self>, abort_signal: &'static AbortSignal) {
        if abort_signal.aborted() {
            self.abort(abort_signal.reason());
            return;
        }
        let reply_lifetime_controller = webapi::abort_controller::new();
        self.reply_lifetime_controller
            .set(Some(reply_lifetime_controller));
        let reply = Rc::clone(self);
        abort_signal.add_event_listener(
            webapi::events::Abort,
            move |_| {
                reply.abort(abort_signal.reason());
            },
            Some(&webapi::AddEventListenerOptions {
                signal: Some(reply_lifetime_controller.signal()),
                ..Default::default()
            }),
        );
    }

    /// The reply's form data, made the first time a part needs one.
    fn form_data(&self) -> &'static FormData {
        if self.form_data.get().is_none() {
            self.form_data.set(Some(webapi::form_data::new(None, None)));
        }
        self.form_data.get().unwrap()
    }

    fn take_part_id(&self) -> f64 {
        let id = self.next_part_id.get();
        self.next_part_id.set(id + 1.0);
        id
    }

    fn serialize_typed_array(&self, tag: &str, typed_array: &js::ArrayBufferView) -> String {
        let blob = webapi::blob::new(
            Some(&[
                // We should be able to pass the buffer straight through but Node < 18 treat
                // multi-byte array blobs differently so we first convert it to single-byte.
                BlobPart::from(js::uint8_array::new_with_buffer(
                    typed_array.buffer(),
                    Some(typed_array.byte_offset()),
                    Some(typed_array.byte_length()),
                )),
            ]),
            None,
        );
        let blob_id = self.take_part_id();
        let form_data = self.form_data();
        form_data.append(
            &(self.form_field_prefix.clone() + &blob_id.to_string()),
            blob,
        );
        "$".to_owned() + tag + &js::number::to_string(blob_id, Some(16))
    }

    fn serialize_binary_reader(
        self: &Rc<Self>,
        reader: &'static webapi::ReadableStreamBYOBReader,
    ) -> String {
        type Progress = Rc<dyn Fn(ReadableStreamReadResult)>;
        // Upgrade to use FormData to allow us to stream this value.
        let data = self.form_data();

        self.pending_parts.set(self.pending_parts.get() + 1.0);
        let stream_id = self.take_part_id();

        let buffer: Rc<RefCell<Vec<&'static js::Uint8Array>>> = Rc::default();
        // Stream chunks use a dedicated namespace so that the server can tell
        // them apart from one-shot chunks without scanning.
        let stream_prefix = self.form_field_prefix.clone() + "S" + &stream_id.to_string();

        let progress: Rc<RefCell<Option<Progress>>> = Rc::default();
        let reply = Rc::clone(self);
        let next = Rc::clone(&progress);
        *progress.borrow_mut() = Some(Rc::new(move |entry: ReadableStreamReadResult| {
            if entry.done.unwrap_or(false) {
                let blob_id = reply.take_part_id();
                let parts: Vec<BlobPart<'_>> = buffer
                    .borrow()
                    .iter()
                    .map(|&chunk| BlobPart::from(chunk))
                    .collect();
                data.append(
                    &(reply.form_field_prefix.clone() + &blob_id.to_string()),
                    webapi::blob::new(Some(&parts), None),
                );
                data.append(
                    &stream_prefix,
                    &("\"$o".to_owned() + &js::number::to_string(blob_id, Some(16)) + "\""),
                );
                data.append(&stream_prefix, "C"); // Close signal
                reply.pending_parts.set(reply.pending_parts.get() - 1.0);
                if reply.pending_parts.get() == 0.0 {
                    reply.resolve(ReplyBody::Form(data));
                }
            } else {
                // SAFETY: a BYOB reader's chunk is the Uint8Array it filled.
                buffer
                    .borrow_mut()
                    .push(unsafe { js::cast(entry.value.flatten()) });
                let progress = next.borrow().clone().unwrap();
                let reply = Rc::clone(&reply);
                reader
                    .read(js::uint8_array::new(1024), None)
                    .then_with::<(), _, _>(
                        move |entry| progress(entry),
                        move |error| reply.reject(error.as_unknown()),
                    );
            }
        }));
        let progress = progress.borrow().clone().unwrap();
        let reply = Rc::clone(self);
        reader
            .read(js::uint8_array::new(1024), None)
            .then_with::<(), _, _>(
                move |entry| progress(entry),
                move |error| reply.reject(error.as_unknown()),
            );

        "$r".to_owned() + &js::number::to_string(stream_id, Some(16))
    }

    fn serialize_reader(
        self: &Rc<Self>,
        reader: &'static webapi::ReadableStreamDefaultReader,
    ) -> String {
        type Progress = Rc<dyn Fn(ReadableStreamReadResult)>;
        // Upgrade to use FormData to allow us to stream this value.
        let data = self.form_data();

        self.pending_parts.set(self.pending_parts.get() + 1.0);
        let stream_id = self.take_part_id();

        // Stream chunks use a dedicated namespace so that the server can tell
        // them apart from one-shot chunks without scanning.
        let stream_prefix = self.form_field_prefix.clone() + "S" + &stream_id.to_string();

        let progress: Rc<RefCell<Option<Progress>>> = Rc::default();
        let reply = Rc::clone(self);
        let next = Rc::clone(&progress);
        *progress.borrow_mut() = Some(Rc::new(move |entry: ReadableStreamReadResult| {
            if entry.done.unwrap_or(false) {
                data.append(&stream_prefix, "C"); // Close signal
                reply.pending_parts.set(reply.pending_parts.get() - 1.0);
                if reply.pending_parts.get() == 0.0 {
                    reply.resolve(ReplyBody::Form(data));
                }
            } else {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let part_json = reply.stringify(entry.value.flatten());
                    data.append(&stream_prefix, &part_json);
                    let progress = next.borrow().clone().unwrap();
                    let rejecting = Rc::clone(&reply);
                    reader.read().then_with::<(), _, _>(
                        move |entry| progress(entry),
                        move |error| rejecting.reject(error.as_unknown()),
                    );
                }));
                if let Err(x) = result {
                    reply.reject(thrown(&*x));
                }
            }
        }));
        let progress = progress.borrow().clone().unwrap();
        let reply = Rc::clone(self);
        reader.read().then_with::<(), _, _>(
            move |entry| progress(entry),
            move |error| reply.reject(error.as_unknown()),
        );

        "$R".to_owned() + &js::number::to_string(stream_id, Some(16))
    }

    fn serialize_readable_stream(self: &Rc<Self>, stream: &'static ReadableStream) -> String {
        // Detect if this is a BYOB stream. BYOB streams should be able to be read as bytes on the
        // receiving side. For binary streams, we serialize them as plain Blobs.
        let binary_reader = match stream
            .get_reader_with_options(ReadableStreamGetByobReaderOptions { mode: Byob })
        {
            Ok(binary_reader) => binary_reader,
            Err(_) => return self.serialize_reader(stream.get_reader()),
        };
        self.serialize_binary_reader(binary_reader)
    }

    fn serialize_async_iterable(
        self: &Rc<Self>,
        iterable: &'static JsObject,
        iterator: &'static AsyncIterator<Option<&'static Unknown>>,
    ) -> String {
        // Upgrade to use FormData to allow us to stream this value.
        let data = self.form_data();

        self.pending_parts.set(self.pending_parts.get() + 1.0);
        let stream_id = self.take_part_id();

        // Generators/Iterators are Iterables but they're also their own iterator
        // functions. If that's the case, we treat them as single-shot. Otherwise,
        // we assume that this iterable might be a multi-shot and allow it to be
        // iterated more than once on the receiving server.
        let is_iterator =
            js::strict_equals(Some(js::unknown(iterable)), Some(js::unknown(iterator)));

        // Stream chunks use a dedicated namespace so that the server can tell
        // them apart from one-shot chunks without scanning.
        let stream_prefix = self.form_field_prefix.clone() + "S" + &stream_id.to_string();

        // There's a race condition between when the stream is aborted and when the promise
        // resolves so we track whether we already aborted it to avoid writing twice.
        type Progress = Rc<dyn Fn(js::IteratorResult<Option<&'static Unknown>>)>;
        let progress: Rc<RefCell<Option<Progress>>> = Rc::default();
        let reply = Rc::clone(self);
        let next = Rc::clone(&progress);
        *progress.borrow_mut() = Some(Rc::new(
            move |entry: js::IteratorResult<Option<&'static Unknown>>| {
                if entry.done.unwrap_or(false) {
                    if matches!(js::classify(entry.value), Kind::Undefined) {
                        data.append(&stream_prefix, "C"); // Close signal
                    } else {
                        // Unlike streams, the last value may not be undefined. If it's not
                        // we outline it and encode a reference to it in the closing instruction.
                        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            reply.stringify(entry.value)
                        })) {
                            Ok(part_json) => {
                                data.append(&stream_prefix, &("C".to_owned() + &part_json))
                            } // Close signal
                            Err(x) => {
                                reply.reject(thrown(&*x));
                                return;
                            }
                        }
                    }
                    reply.pending_parts.set(reply.pending_parts.get() - 1.0);
                    if reply.pending_parts.get() == 0.0 {
                        reply.resolve(ReplyBody::Form(data));
                    }
                } else {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let part_json = reply.stringify(entry.value);
                        data.append(&stream_prefix, &part_json);
                        let progress = next.borrow().clone().unwrap();
                        let rejecting = Rc::clone(&reply);
                        iterator.next().then_with::<(), _, _>(
                            move |entry| progress(entry),
                            move |error| rejecting.reject(error.as_unknown()),
                        );
                    }));
                    if let Err(x) = result {
                        reply.reject(thrown(&*x));
                    }
                }
            },
        ));
        let progress = progress.borrow().clone().unwrap();
        let reply = Rc::clone(self);
        iterator.next().then_with::<(), _, _>(
            move |entry| progress(entry),
            move |error| reply.reject(error.as_unknown()),
        );
        "$".to_owned()
            + if is_iterator { "x" } else { "X" }
            + &js::number::to_string(stream_id, Some(16))
    }

    /// `JSON.stringify(value, resolveToJSON)`: each key and value
    /// given resolveToJSON, `this` the object holding the key.
    fn stringify(self: &Rc<Self>, value: Option<&'static Unknown>) -> String {
        let reply = Rc::clone(self);
        json::stringify_with(
            &value,
            function::with_this(
                move |this, (key, value): (&str, Option<&'static Unknown>)| {
                    // SAFETY: JSON.stringify calls its replacer on the object or
                    // array holding the key.
                    let parent: &'static JsObject = unsafe { js::cast(this) };
                    reply.resolve_to_json(parent, key, value)
                },
            ),
            0,
        )
        .unwrap()
        .unwrap_or_default()
    }

    fn resolve_to_json(
        self: &Rc<Self>,
        parent: &'static JsObject,
        key: &str,
        value: Option<&'static Unknown>,
    ) -> Option<&'static Unknown> {
        if __DEV__ && key == __PROTO__ {
            console::error((
                "Expected not to serialize an object with own property `__proto__`. When parsed this property will be omitted.%s",
                describe_object_for_error_message(parent, Some(key)),
            ));
        }

        // Make sure that `parent[key]` wasn't JSONified before `value` was passed to us
        if __DEV__ {
            let original_value = js::get(parent, key);
            if js::type_of(original_value) == "object"
                && !js::strict_equals(original_value, value)
                && !js::instance_of(original_value, js::class::<js::Date>())
            {
                if object_name(original_value) != "Object" {
                    console::error((
                        concat!(
                            "Only plain objects can be passed to Server Functions from the Client. ",
                            "%s objects are not supported.%s"
                        ),
                        object_name(original_value),
                        describe_object_for_error_message(parent, Some(key)),
                    ));
                } else {
                    console::error((
                        concat!(
                            "Only plain objects can be passed to Server Functions from the Client. ",
                            "Objects with toJSON methods are not supported. Convert it manually ",
                            "to a simple value before passing it to props.%s"
                        ),
                        describe_object_for_error_message(parent, Some(key)),
                    ));
                }
            }
        }

        if matches!(js::classify(value), Kind::Null) {
            return js::null();
        }

        if let Some(object) = js::classify(value).object() {
            if ENABLE_FLIGHT_OBJECT_REFERENCES.with(|&enabled| enabled) {
                // Check before traversing the object or falling back to a temporary
                // reference. A Server Reference must always be encoded by id.
                let object_reference_id =
                    KNOWN_SERVER_OBJECT_REFERENCES.with(|references| references.get(object));
                if let Some(object_reference_id) = object_reference_id {
                    let existing_reference = self.written_objects.get(object);
                    if let Some(existing_reference) = existing_reference {
                        if js::strict_equals(self.model_root.get(), value) {
                            // serializeModel registered this root before we recognized it
                            // as a Server Reference. Encode its metadata on this first visit.
                            self.model_root.set(None);
                        } else {
                            return Some(js::unknown_of(existing_reference));
                        }
                    }
                    let reference_json =
                        self.stringify(Some(js::unknown_of(ServerObjectReferenceJson {
                            id: object_reference_id,
                        })));
                    // Upgrade to use FormData to allow us to stream this value.
                    let form_data = self.form_data();
                    // The reference to this object came from the same client so we can
                    // pass it back to the server where it resolves to the object.
                    let ref_id = self.take_part_id();
                    form_data.set(
                        &(self.form_field_prefix.clone() + &ref_id.to_string()),
                        &reference_json,
                    );
                    let server_object_reference_id = serialize_server_object_reference_id(ref_id);
                    // Store the reference ID for deduplication.
                    self.written_objects
                        .set(object, server_object_reference_id.clone());
                    return Some(js::unknown_of(server_object_reference_id));
                }
            }
            match SymbolFor::from_value(js::get(object, "$$typeof")) {
                Some(REACT_ELEMENT_TYPE) => {
                    if let Some(temporary_references) = &self.temporary_references
                        && !key.contains(':')
                    {
                        // TODO: If the property name contains a colon, we don't dedupe. Escape instead.
                        let parent_reference = self.written_objects.get(parent);
                        if let Some(parent_reference) = parent_reference {
                            // If the parent has a reference, we can refer to this object indirectly
                            // through the property name inside that parent.
                            let reference = parent_reference + ":" + key;
                            // Store this object so that the server can refer to it later in responses.
                            write_temporary_reference(temporary_references, reference, object);
                            return Some(js::unknown(serialize_temporary_reference_marker()));
                        }
                    }
                    // This element is the root of a serializeModel call (e.g. JSX
                    // passed directly to encodeReply, or a promise that resolved to
                    // JSX). It was already registered as a temporary reference by
                    // serializeModel so we just need to emit the marker.
                    if self.temporary_references.is_some()
                        && js::strict_equals(self.model_root.get(), value)
                    {
                        self.model_root.set(None);
                        return Some(js::unknown(serialize_temporary_reference_marker()));
                    }
                    panic!(
                        "{}",
                        concat!(
                            "React Element cannot be passed to Server Functions from the Client without a ",
                            "temporary reference set. Pass a TemporaryReferenceSet to the options."
                        )
                        .to_owned()
                            + &if __DEV__ { describe_object_for_error_message(parent, Some(key)) } else { String::new() }
                    );
                }
                Some(REACT_LAZY_TYPE) => {
                    // Resolve lazy as if it wasn't here. In the future this will be encoded as a Promise.
                    let payload = js::get(object, "_payload");
                    // SAFETY: a lazy node's `_init` is its initializer.
                    let init: &'static dyn Fn(
                        Option<&'static Unknown>,
                    ) -> Option<&'static Unknown> = unsafe { js::cast(js::get(object, "_init")) };
                    // Upgrade to use FormData to allow us to stream this value.
                    let data = self.form_data();
                    self.pending_parts.set(self.pending_parts.get() + 1.0);
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let resolved_model = init(payload);
                        // We always outline this as a separate part even though we could inline it
                        // because it ensures a more deterministic encoding.
                        let lazy_id = self.take_part_id();
                        let part_json = self.serialize_model(resolved_model, lazy_id);
                        data.append(
                            &(self.form_field_prefix.clone() + &lazy_id.to_string()),
                            &part_json,
                        );
                        serialize_by_value_id(lazy_id)
                    }));
                    let resolved = match result {
                        Ok(reference) => Some(js::unknown_of(reference)),
                        Err(payload) => {
                            let x = thrown(&*payload);
                            if js::type_of(x) == "object"
                                && !matches!(js::classify(x), Kind::Null)
                                && js::type_of(js::get(x, "then")) == "function"
                            {
                                // Suspended
                                self.pending_parts.set(self.pending_parts.get() + 1.0);
                                let lazy_id = self.take_part_id();
                                // SAFETY: an object with a `then` method is a thenable.
                                let thenable: &Wakeable = unsafe { js::cast(x) };
                                let reply = Rc::clone(self);
                                let retry = move || {
                                    // While the first promise resolved, its value isn't necessarily what we'll
                                    // resolve into because we might suspend again.
                                    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                                        || reply.serialize_model(value, lazy_id),
                                    )) {
                                        Ok(part_json) => {
                                            data.append(
                                                &(reply.form_field_prefix.clone()
                                                    + &lazy_id.to_string()),
                                                &part_json,
                                            );
                                            reply
                                                .pending_parts
                                                .set(reply.pending_parts.get() - 1.0);
                                            if reply.pending_parts.get() == 0.0 {
                                                reply.resolve(ReplyBody::Form(data));
                                            }
                                        }
                                        Err(reason) => reply.reject(thrown(&*reason)),
                                    }
                                };
                                thenable.then(&retry, &retry);
                                Some(js::unknown_of(serialize_by_value_id(lazy_id)))
                            } else {
                                // In the future we could consider serializing this as an error
                                // that throws on the server instead.
                                self.reject(x);
                                js::null()
                            }
                        }
                    };
                    self.pending_parts.set(self.pending_parts.get() - 1.0);
                    return resolved;
                }
                _ => {}
            }

            let existing_reference = self.written_objects.get(object);

            if js::type_of(js::get(object, "then")) == "function" {
                if let Some(existing_reference) = existing_reference {
                    if js::strict_equals(self.model_root.get(), value) {
                        // This is the ID we're currently emitting so we need to write it
                        // once but if we discover it again, we refer to it by id.
                        self.model_root.set(None);
                    } else {
                        // We've already emitted this as an outlined object, so we can
                        // just refer to that by its existing ID.
                        return Some(js::unknown_of(existing_reference));
                    }
                }

                // We assume that any object with a .then property is a "Thenable" type,
                // or a Promise type. Either of which can be represented by a Promise.
                // Upgrade to use FormData to allow us to stream this value.
                let data = self.form_data();
                self.pending_parts.set(self.pending_parts.get() + 1.0);
                let promise_id = self.take_part_id();
                let promise_reference = serialize_promise_id(promise_id);
                self.written_objects.set(object, promise_reference.clone());
                // SAFETY: an object with a `then` method is a thenable.
                let thenable: &Thenable = unsafe { js::cast(value) };
                let reply = Rc::clone(self);
                let rejecting = Rc::clone(self);
                thenable.then(
                    move |part_value| match std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                        || {
                            let previous_reference = match js::classify(part_value).object() {
                                Some(part) => reply.written_objects.get(part),
                                None => None,
                            };
                            match previous_reference {
                                Some(previous_reference) => json::stringify(&previous_reference),
                                None => reply.serialize_model(part_value, promise_id),
                            }
                        },
                    )) {
                        Ok(part_json) => {
                            data.append(
                                &(reply.form_field_prefix.clone() + &promise_id.to_string()),
                                &part_json,
                            );
                            reply.pending_parts.set(reply.pending_parts.get() - 1.0);
                            if reply.pending_parts.get() == 0.0 {
                                reply.resolve(ReplyBody::Form(data));
                            }
                        }
                        Err(reason) => reply.reject(thrown(&*reason)),
                    },
                    // In the future we could consider serializing this as an error
                    // that throws on the server instead.
                    move |reason| rejecting.reject(reason),
                );
                return Some(js::unknown_of(promise_reference));
            }

            match existing_reference {
                Some(existing_reference) => {
                    if js::strict_equals(self.model_root.get(), value) {
                        // This is the ID we're currently emitting so we need to write it
                        // once but if we discover it again, we refer to it by id.
                        self.model_root.set(None);
                    } else {
                        // We've already emitted this as an outlined object, so we can
                        // just refer to that by its existing ID.
                        return Some(js::unknown_of(existing_reference));
                    }
                }
                None if !key.contains(':') => {
                    // TODO: If the property name contains a colon, we don't dedupe. Escape instead.
                    let parent_reference = self.written_objects.get(parent);
                    if let Some(parent_reference) = parent_reference {
                        // If the parent has a reference, we can refer to this object indirectly
                        // through the property name inside that parent.
                        let reference = parent_reference + ":" + key;
                        self.written_objects.set(object, reference.clone());
                        if let Some(temporary_references) = &self.temporary_references {
                            // Store this object so that the server can refer to it later in responses.
                            write_temporary_reference(temporary_references, reference, object);
                        }
                    }
                }
                None => {}
            }

            if is_array(value) {
                return value;
            }
            // TODO: Should we the Object.prototype.toString.call() to test for cross-realm objects?
            if js::instance_of(object, js::class::<FormData>()) {
                // Upgrade to use FormData to allow us to use rich objects as its values.
                let data = self.form_data();
                let ref_id = self.take_part_id();
                // Copy all the form fields with a prefix for this reference.
                // These must come first in the form order because we assume that all the
                // fields are available before this is referenced.
                // We include a special marker so that the Server can detect FormData entries
                // that are values in referenced FormData objects.
                let prefix = self.form_field_prefix.clone() + "_" + &ref_id.to_string() + "_";
                // SAFETY: an instance of FormData is one.
                let value: &FormData = unsafe { js::cast(value) };
                value.for_each(|original_value, original_key, _| {
                    data.append(&(prefix.clone() + original_key), original_value);
                });
                return Some(js::unknown_of(serialize_form_data_reference(ref_id)));
            }
            if js::instance_of(object, js::class::<js::Map<Unknown, Unknown>>()) {
                let map_id = self.take_part_id();
                // SAFETY: an instance of Map is one.
                let map: &js::Map<Option<&'static Unknown>, Option<&'static Unknown>> =
                    unsafe { js::cast(value) };
                let part_json = self.serialize_model(
                    Some(js::unknown_of(map.entries().collect::<Vec<_>>())),
                    map_id,
                );
                self.form_data().append(
                    &(self.form_field_prefix.clone() + &map_id.to_string()),
                    &part_json,
                );
                return Some(js::unknown_of(serialize_map_id(map_id)));
            }
            if js::instance_of(object, js::class::<js::Set<Unknown>>()) {
                // SAFETY: an instance of Set is one.
                let set: &js::Set<Option<&'static Unknown>> = unsafe { js::cast(value) };
                let set_id = self.take_part_id();
                let part_json = self.serialize_model(
                    Some(js::unknown_of(set.values().collect::<Vec<_>>())),
                    set_id,
                );
                self.form_data().append(
                    &(self.form_field_prefix.clone() + &set_id.to_string()),
                    &part_json,
                );
                return Some(js::unknown_of(serialize_set_id(set_id)));
            }

            if js::instance_of(object, js::class::<js::ArrayBuffer>()) {
                // SAFETY: an instance of ArrayBuffer is one.
                let buffer: &js::ArrayBuffer = unsafe { js::cast(value) };
                let blob = webapi::blob::new(Some(&[BlobPart::from(buffer)]), None);
                let blob_id = self.take_part_id();
                self.form_data().append(
                    &(self.form_field_prefix.clone() + &blob_id.to_string()),
                    blob,
                );
                return Some(js::unknown_of(
                    "$".to_owned() + "A" + &js::number::to_string(blob_id, Some(16)),
                ));
            }
            if js::instance_of(object, js::class::<js::Int8Array>()) {
                // char
                // SAFETY: a typed array or DataView is an ArrayBufferView.
                return Some(js::unknown_of(
                    self.serialize_typed_array("O", unsafe { js::cast(value) }),
                ));
            }
            if js::instance_of(object, js::class::<js::Uint8Array>()) {
                // unsigned char
                // SAFETY: as Int8Array's.
                return Some(js::unknown_of(
                    self.serialize_typed_array("o", unsafe { js::cast(value) }),
                ));
            }
            if js::instance_of(object, js::class::<js::Uint8ClampedArray>()) {
                // unsigned clamped char
                // SAFETY: as Int8Array's.
                return Some(js::unknown_of(
                    self.serialize_typed_array("U", unsafe { js::cast(value) }),
                ));
            }
            if js::instance_of(object, js::class::<js::Int16Array>()) {
                // sort
                // SAFETY: as Int8Array's.
                return Some(js::unknown_of(
                    self.serialize_typed_array("S", unsafe { js::cast(value) }),
                ));
            }
            if js::instance_of(object, js::class::<js::Uint16Array>()) {
                // unsigned short
                // SAFETY: as Int8Array's.
                return Some(js::unknown_of(
                    self.serialize_typed_array("s", unsafe { js::cast(value) }),
                ));
            }
            if js::instance_of(object, js::class::<js::Int32Array>()) {
                // long
                // SAFETY: as Int8Array's.
                return Some(js::unknown_of(
                    self.serialize_typed_array("L", unsafe { js::cast(value) }),
                ));
            }
            if js::instance_of(object, js::class::<js::Uint32Array>()) {
                // unsigned long
                // SAFETY: as Int8Array's.
                return Some(js::unknown_of(
                    self.serialize_typed_array("l", unsafe { js::cast(value) }),
                ));
            }
            if js::instance_of(object, js::class::<js::Float32Array>()) {
                // float
                // SAFETY: as Int8Array's.
                return Some(js::unknown_of(
                    self.serialize_typed_array("G", unsafe { js::cast(value) }),
                ));
            }
            if js::instance_of(object, js::class::<js::Float64Array>()) {
                // double
                // SAFETY: as Int8Array's.
                return Some(js::unknown_of(
                    self.serialize_typed_array("g", unsafe { js::cast(value) }),
                ));
            }
            if js::instance_of(object, js::class::<js::BigInt64Array>()) {
                // number
                // SAFETY: as Int8Array's.
                return Some(js::unknown_of(
                    self.serialize_typed_array("M", unsafe { js::cast(value) }),
                ));
            }
            if js::instance_of(object, js::class::<js::BigUint64Array>()) {
                // unsigned number
                // We use "m" instead of "n" since JSON can start with "null"
                // SAFETY: as Int8Array's.
                return Some(js::unknown_of(
                    self.serialize_typed_array("m", unsafe { js::cast(value) }),
                ));
            }
            if js::instance_of(object, js::class::<js::DataView>()) {
                // SAFETY: as Int8Array's.
                return Some(js::unknown_of(
                    self.serialize_typed_array("V", unsafe { js::cast(value) }),
                ));
            }
            // TODO: Blob is not available in old Node/browsers. Remove the typeof check later.
            if js::is_function(webapi::blob::new) && js::instance_of(object, js::class::<Blob>()) {
                let blob_id = self.take_part_id();
                // SAFETY: an instance of Blob is one.
                let blob: &Blob = unsafe { js::cast(value) };
                self.form_data().append(
                    &(self.form_field_prefix.clone() + &blob_id.to_string()),
                    blob,
                );
                return Some(js::unknown_of(serialize_blob_id(blob_id)));
            }

            if let Some(iterator_fn) = get_iterator_fn(value) {
                // SAFETY: getIteratorFn gives a function.
                let iterator_fn: &dyn Fn() -> &'static Unknown =
                    unsafe { js::cast(Some(iterator_fn)) };
                let iterator = function::call(
                    iterator_fn,
                    value.map(|this| this as &dyn js::IntoUnknown),
                    (),
                )
                .unwrap();
                if js::strict_equals(Some(iterator), value) {
                    // Iterator, not Iterable
                    let iterator_id = self.take_part_id();
                    let part_json = self.serialize_model(
                        Some(js::unknown_of(js::array::from::<Option<&'static Unknown>>(
                            iterator,
                        ))),
                        iterator_id,
                    );
                    self.form_data().append(
                        &(self.form_field_prefix.clone() + &iterator_id.to_string()),
                        &part_json,
                    );
                    return Some(js::unknown_of(serialize_iterator_id(iterator_id)));
                }
                return Some(js::unknown_of(js::array::from::<Option<&'static Unknown>>(
                    iterator,
                )));
            }

            // TODO: ReadableStream is not available in old Node. Remove the typeof check later.
            if js::is_function(webapi::readable_stream::new)
                && js::instance_of(object, js::class::<ReadableStream>())
            {
                // SAFETY: an instance of ReadableStream is one.
                return Some(js::unknown_of(
                    self.serialize_readable_stream(unsafe { js::cast(value) }),
                ));
            }
            let get_async_iterator =
                ASYNC_ITERATOR.with(|&async_iterator| js::get(object, async_iterator));
            if let Kind::Function(get_async_iterator) = js::classify(get_async_iterator) {
                // We treat AsyncIterables as a Fragment and as such we might need to key them.
                // SAFETY: what `[Symbol.asyncIterator]()` gives is an async iterator.
                let iterator: &'static AsyncIterator<Option<&'static Unknown>> =
                    unsafe { js::cast(function::call(get_async_iterator, Some(&object), ())) };
                return Some(js::unknown_of(
                    self.serialize_async_iterable(object, iterator),
                ));
            }

            // Verify that this is a simple plain object.
            let proto = GET_PROTOTYPE_OF.with(|get_prototype_of| get_prototype_of(value));
            if !js::strict_equals(proto, OBJECT_PROTOTYPE.with(|&prototype| Some(prototype)))
                && (matches!(js::classify(proto), Kind::Null)
                    || !matches!(
                        js::classify(
                            GET_PROTOTYPE_OF.with(|get_prototype_of| get_prototype_of(proto))
                        ),
                        Kind::Null
                    ))
            {
                if self.temporary_references.is_none() {
                    panic!(
                        "{}",
                        concat!(
                            "Only plain objects, and a few built-ins, can be passed to Server Functions. ",
                            "Classes or null prototypes are not supported."
                        )
                        .to_owned()
                            + &if __DEV__ { describe_object_for_error_message(parent, Some(key)) } else { String::new() }
                    );
                }
                // We will have written this object to the temporary reference set above
                // so we can replace it with a marker to refer to this slot later.
                return Some(js::unknown(serialize_temporary_reference_marker()));
            }
            if __DEV__ {
                if SymbolFor::from_value(js::get(object, "$$typeof")) == Some(REACT_CONTEXT_TYPE) {
                    console::error((
                        "React Context Providers cannot be passed to Server Functions from the Client.%s",
                        describe_object_for_error_message(parent, Some(key)),
                    ));
                } else if object_name(value) != "Object" {
                    console::error((
                        concat!(
                            "Only plain objects can be passed to Server Functions from the Client. ",
                            "%s objects are not supported.%s"
                        ),
                        object_name(value),
                        describe_object_for_error_message(parent, Some(key)),
                    ));
                } else if !is_simple_object(object) {
                    console::error((
                        concat!(
                            "Only plain objects can be passed to Server Functions from the Client. ",
                            "Classes or other objects with methods are not supported.%s"
                        ),
                        describe_object_for_error_message(parent, Some(key)),
                    ));
                } else if js::is_function(js::object::get_own_property_symbols) {
                    let symbols = js::object::get_own_property_symbols(object);
                    if !symbols.is_empty() {
                        console::error((
                            concat!(
                                "Only plain objects can be passed to Server Functions from the Client. ",
                                "Objects with symbol properties like %s are not supported.%s"
                            ),
                            symbols[0].description(),
                            describe_object_for_error_message(parent, Some(key)),
                        ));
                    }
                }
            }

            return value;
        }

        match js::classify(value) {
            Kind::String(text) => {
                // TODO: Maybe too clever. If we support URL there's no similar trick.
                if text.ends_with('Z') {
                    // Possibly a Date, whose toJSON automatically calls toISOString
                    let original_value = js::get(parent, key);
                    if js::instance_of(original_value, js::class::<js::Date>()) {
                        return Some(js::unknown_of(serialize_date_from_date_json(text)));
                    }
                }

                Some(js::unknown_of(escape_string_value(text)))
            }
            Kind::Bool(_) => value,
            Kind::Number(number) => Some(match serialize_number(number) {
                SerializedNumber::Text(text) => js::unknown_of(text),
                SerializedNumber::Number(number) => js::unknown_of(number),
            }),
            Kind::Undefined => Some(js::unknown_of(serialize_undefined())),
            Kind::Function(function) => {
                let reference_closure =
                    KNOWN_SERVER_REFERENCES.with(|references| references.get(function));
                if let Some(reference_closure) = reference_closure {
                    let existing_reference = self.written_objects.get(function::object(function));
                    if let Some(existing_reference) = existing_reference {
                        return Some(js::unknown_of(existing_reference));
                    }
                    let ServerReferenceClosure { id, bound, .. } = *reference_closure;
                    let reference_closure_json =
                        self.stringify(Some(js::unknown_of(BoundReferenceJson { id, bound })));
                    // Upgrade to use FormData to allow us to stream this value.
                    let form_data = self.form_data();
                    // The reference to this function came from the same client so we can pass it back.
                    let ref_id = self.take_part_id();
                    form_data.set(
                        &(self.form_field_prefix.clone() + &ref_id.to_string()),
                        &reference_closure_json,
                    );
                    let server_reference_id = serialize_server_reference_id(ref_id);
                    // Store the server reference ID for deduplication.
                    self.written_objects
                        .set(function::object(function), server_reference_id.clone());
                    return Some(js::unknown_of(server_reference_id));
                }
                if let Some(temporary_references) = &self.temporary_references
                    && !key.contains(':')
                {
                    // TODO: If the property name contains a colon, we don't dedupe. Escape instead.
                    let parent_reference = self.written_objects.get(parent);
                    if let Some(parent_reference) = parent_reference {
                        // If the parent has a reference, we can refer to this object indirectly
                        // through the property name inside that parent.
                        let reference = parent_reference + ":" + key;
                        // Store this object so that the server can refer to it later in responses.
                        write_temporary_reference(
                            temporary_references,
                            reference,
                            js::unknown(function),
                        );
                        return Some(js::unknown(serialize_temporary_reference_marker()));
                    }
                }
                panic!(
                    "{}",
                    concat!(
                        "Client Functions cannot be passed directly to Server Functions. ",
                        "Only Functions passed from the Server can be passed back again."
                    )
                );
            }
            Kind::Symbol(symbol) => {
                if let Some(temporary_references) = &self.temporary_references
                    && !key.contains(':')
                {
                    // TODO: If the property name contains a colon, we don't dedupe. Escape instead.
                    let parent_reference = self.written_objects.get(parent);
                    if let Some(parent_reference) = parent_reference {
                        // If the parent has a reference, we can refer to this object indirectly
                        // through the property name inside that parent.
                        let reference = parent_reference + ":" + key;
                        // Store this object so that the server can refer to it later in responses.
                        write_temporary_reference(
                            temporary_references,
                            reference,
                            js::unknown(symbol),
                        );
                        return Some(js::unknown(serialize_temporary_reference_marker()));
                    }
                }
                panic!(
                    "{}",
                    concat!(
                        "Symbols cannot be passed to a Server Function without a ",
                        "temporary reference set. Pass a TemporaryReferenceSet to the options."
                    )
                    .to_owned()
                        + &if __DEV__ {
                            describe_object_for_error_message(parent, Some(key))
                        } else {
                            String::new()
                        }
                );
            }
            Kind::BigInt(n) => Some(js::unknown_of(serialize_big_int(n))),
            Kind::Null | Kind::Array(_) | Kind::Object(_) => {
                panic!(
                    "Type {} is not supported as an argument to a Server Function.",
                    js::type_of(value)
                )
            }
        }
    }

    fn serialize_model(self: &Rc<Self>, model: Option<&'static Unknown>, id: f64) -> String {
        if let Some(object) = js::classify(model).object() {
            let reference = serialize_by_value_id(id);
            self.written_objects.set(object, reference.clone());
            if let Some(temporary_references) = &self.temporary_references {
                // Store this object so that the server can refer to it later in responses.
                write_temporary_reference(temporary_references, reference, object);
            }
        }
        self.model_root.set(model);
        self.stringify(model)
    }

    fn abort(&self, _reason: Option<&'static Unknown>) {
        // Nothing can make the reply pending again from here, so the caller's
        // signal has no further effect on it.
        self.end_reply_lifetime();
        if self.pending_parts.get() > 0.0 {
            self.pending_parts.set(0.0); // Don't resolve again later.
            // Resolve with what we have so far, which may have holes at this point.
            // They'll error when the stream completes on the server.
            match self.form_data.get() {
                None => self.resolve(ReplyBody::Text(self.json.get().unwrap().clone())),
                Some(form_data) => self.resolve(ReplyBody::Form(form_data)),
            }
        }
    }
}

/// `{id: objectReferenceId}`, what a server object reference is written as.
struct ServerObjectReferenceJson {
    id: ServerReferenceId,
}

unsafe impl js::Defined for ServerObjectReferenceJson {}

/// `{id, bound}`, what a server reference is written as.
struct BoundReferenceJson {
    id: ServerReferenceId,
    #[cfg_attr(rust_js, rust_js::nullable)]
    bound: Option<&'static Thenable>,
}

unsafe impl js::Defined for BoundReferenceJson {}

/// What JS threw, of a caught unwind.
fn thrown(payload: &(dyn std::any::Any + Send)) -> Option<&'static Unknown> {
    payload
        .downcast_ref::<&'static js::JsError>()
        .and_then(|error| error.as_unknown())
}

pub fn process_reply(
    root: Option<&'static ReactServerValue>,
    form_field_prefix: &str,
    temporary_references: Option<TemporaryReferenceSet>,
    on_resolve: impl Fn(ReplyBody) + 'static,
    on_reject: impl Fn(Option<&'static Unknown>) + 'static,
    signal: Option<&'static AbortSignal>,
) {
    let reply = Rc::new(Reply {
        form_field_prefix: form_field_prefix.to_owned(),
        temporary_references,
        on_resolve: Box::new(on_resolve),
        on_reject: Box::new(on_reject),
        next_part_id: Cell::new(1.0),
        pending_parts: Cell::new(0.0),
        form_data: Cell::new(None),
        written_objects: js::weak_map::new(),
        model_root: Cell::new(root),
        settled: Cell::new(false),
        reply_lifetime_controller: Cell::new(None),
        json: OnceCell::new(),
    });

    if __DEV__ {
        // We use eval to create fake function stacks which includes Component stacks.
        // A warning would be noise if you used Flight without Components and don't encounter
        // errors. We're warning eagerly so that you configure your environment accordingly
        // before you encounter an error.
        check_eval_availability_once_dev();
    }

    let json = reply.json.get_or_init(|| reply.serialize_model(root, 0.0));

    match reply.form_data.get() {
        None => {
            // If it's a simple data structure, we just use plain JSON.
            reply.resolve(ReplyBody::Text(json.clone()));
        }
        Some(form_data) => {
            // Otherwise, we use FormData to let us stream in the result.
            form_data.set(&(reply.form_field_prefix.clone() + "0"), json);
            if reply.pending_parts.get() == 0.0 {
                reply.resolve(ReplyBody::Form(form_data));
            }
        }
    }

    // Wired up after serializing: abort() reads `json` and resolves with the
    // parts that finished, so it must not be reachable before then. A reply that
    // already settled gets no listener, since aborting it would be a no-op and
    // the lifetime that removes the listener has already ended.
    //
    // TODO: Skip serializing when the signal is already aborted, the way the
    // server entry points abort before rendering starts. Needs a decision on what
    // to resolve with, since abort() resolves with the parts that finished.
    if let Some(signal) = signal
        && !reply.settled.get()
    {
        reply.attach_abort_signal(signal);
    }
}

thread_local! {
    static BOUND_CACHE: &'static js::WeakMap<ServerReferenceClosure, &'static Thenable> = js::weak_map::new();
}

fn encode_form_data(reference: Option<&'static Unknown>) -> &'static Thenable {
    let mut resolve: Option<&'static dyn Fn(&'static FormData)> = None;
    type Reject = dyn Fn(Option<&Unknown>);
    let mut reject: Option<&'static Reject> = None;
    // We need to have a handle on the thenable so that we can synchronously set
    // its status from processReply, when it can complete synchronously.
    let promise = js::promise::new(|res, rej| {
        resolve = Some(res);
        reject = Some(rej);
    });
    let thenable: &'static Thenable = js::upcast(promise);
    let (resolve, reject) = (resolve.unwrap(), reject.unwrap());
    process_reply(
        reference,
        "",
        None, // TODO: This means React Elements can't be used as state in progressive enhancement.
        move |body| {
            let body: &'static FormData = match body {
                ReplyBody::Text(text) => {
                    let data = webapi::form_data::new(None, None);
                    data.append("0", &text);
                    data
                }
                ReplyBody::Form(data) => data,
            };
            let fulfilled: &Thenable = thenable;
            fulfilled.set_status("fulfilled");
            fulfilled.set_value(Some(js::unknown(body)));
            resolve(body);
        },
        move |e| {
            let rejected: &Thenable = thenable;
            rejected.set_status("rejected");
            rejected.set_reason(e);
            reject(e);
        },
        None,
    );
    thenable
}

/// The fields a form action's encoder gives React DOM.
fn form_action(name: String, data: Option<&'static FormData>) -> ReactCustomFormAction {
    ReactCustomFormAction {
        name: Some(name),
        method: Some("POST".to_owned()),
        enc_type: Some("multipart/form-data".to_owned()),
        data,
        action: RefCell::new(None),
        target: None,
    }
}

thread_local! {
    static DEFAULT_ENCODE_FORM_ACTION: &'static dyn Fn(&'static str) -> ReactCustomFormAction =
        function::with_this(|this, (identifier_prefix,): (&'static str,)| {
            // SAFETY: it's only a server reference's `$$FORM_ACTION`, called on it.
            let reference: &ServerFunction = unsafe { js::cast(this) };
            let Some(reference_closure) = KNOWN_SERVER_REFERENCES.with(|references| references.get(reference)) else {
                panic!(
                    "{}",
                    concat!(
                        "Tried to encode a Server Action from a different instance than the encoder is from. ",
                        "This is a bug in React."
                    )
                );
            };
            let mut data: Option<&'static FormData> = None;
            let name;
            let bound_promise = reference_closure.bound;
            if bound_promise.is_some() {
                let thenable = match BOUND_CACHE.with(|bound_cache| bound_cache.get(&reference_closure)) {
                    Some(thenable) => thenable,
                    None => {
                        let ServerReferenceClosure { id, bound, .. } = *reference_closure;
                        let thenable = encode_form_data(Some(js::unknown_of(BoundReferenceJson { id, bound })));
                        BOUND_CACHE.with(|bound_cache| bound_cache.set(&reference_closure, thenable));
                        thenable
                    }
                };
                if thenable.status().as_deref() == Some("rejected") {
                    js::throw(thenable.reason());
                } else if thenable.status().as_deref() != Some("fulfilled") {
                    js::throw(Some(js::unknown(thenable)));
                }
                // SAFETY: a fulfilled encoding's value is its FormData.
                let encoded_form_data: &FormData = unsafe { js::cast(thenable.value()) };
                // This is hacky but we need the identifier prefix to be added to
                // all fields but the suspense cache would break since we might get
                // a new identifier each time. So we just append it at the end instead.
                let prefixed_data = webapi::form_data::new(None, None);
                encoded_form_data.for_each(|value, key, _| {
                    prefixed_data.append(&("$ACTION_".to_owned() + identifier_prefix + ":" + key), value);
                });
                data = Some(prefixed_data);
                // We encode the name of the prefix containing the data.
                name = "$ACTION_REF_".to_owned() + identifier_prefix;
            } else {
                // This is the simple case so we can just encode the ID.
                name = "$ACTION_ID_".to_owned() + &js::string(Some(reference_closure.id));
            }
            form_action(name, data)
        });
}

fn custom_encode_form_action(
    reference: &ServerFunction,
    _identifier_prefix: &str,
    encode_form_action: &EncodeFormActionCallback,
) -> ReactCustomFormAction {
    let Some(reference_closure) =
        KNOWN_SERVER_REFERENCES.with(|references| references.get(reference))
    else {
        panic!(
            "{}",
            concat!(
                "Tried to encode a Server Action from a different instance than the encoder is from. ",
                "This is a bug in React."
            )
        );
    };
    let bound_promise: &'static Thenable = match reference_closure.bound {
        Some(bound) => bound,
        None => js::upcast(js::promise::resolve(Vec::<Option<&'static Unknown>>::new())),
    };
    encode_form_action(reference_closure.id, bound_promise)
}

thread_local! {
    static IS_SIGNATURE_EQUAL: &'static dyn Fn(ServerReferenceId, f64) -> bool =
        function::with_this(|this, (reference_id, number_of_bound_args): (ServerReferenceId, f64)| {
            // SAFETY: it's only a server reference's `$$IS_SIGNATURE_EQUAL`, called on it.
            let reference: &ServerFunction = unsafe { js::cast(this) };
            let Some(reference_closure) = KNOWN_SERVER_REFERENCES.with(|references| references.get(reference)) else {
                panic!(
                    "{}",
                    concat!(
                        "Tried to encode a Server Action from a different instance than the encoder is from. ",
                        "This is a bug in React."
                    )
                );
            };
            if !js::strict_equals(Some(reference_closure.id), Some(reference_id)) {
                // These are different functions.
                return false;
            }
            // Now check if the number of bound arguments is the same.
            let Some(bound_promise) = reference_closure.bound else {
                // No bound arguments.
                return number_of_bound_args == 0.0;
            };
            // Unwrap the bound arguments array by suspending, if necessary. As with
            // encodeFormData, this means isSignatureEqual can only be called while React
            // is rendering.
            match bound_promise.status().as_deref() {
                Some("fulfilled") => {
                    // SAFETY: the bound arguments are an array.
                    let bound_args: &[Option<&'static Unknown>] = unsafe { js::cast(bound_promise.value()) };
                    bound_args.len() as f64 == number_of_bound_args
                }
                Some("pending") => js::throw(Some(js::unknown(bound_promise))),
                Some("rejected") => js::throw(bound_promise.reason()),
                status => {
                    if status.is_some() {
                        // Only instrument the thenable if the status if not defined.
                    } else {
                        let pending_thenable: &Thenable = bound_promise;
                        pending_thenable.set_status("pending");
                        pending_thenable.then(
                            move |bound_args| {
                                let fulfilled_thenable: &Thenable = bound_promise;
                                fulfilled_thenable.set_status("fulfilled");
                                fulfilled_thenable.set_value(bound_args);
                            },
                            move |error| {
                                let rejected_thenable: &Thenable = bound_promise;
                                rejected_thenable.set_status("rejected");
                                rejected_thenable.set_reason(error);
                            },
                        );
                    }
                    js::throw(Some(js::unknown(bound_promise)))
                }
            }
        });
}

thread_local! {
    static FAKE_SERVER_FUNCTION_IDX: Cell<f64> = const { Cell::new(0.0) };
}

fn create_fake_server_function(
    mut name: String,
    mut filename: String,
    source_map: Option<String>,
    line: f64,
    col: f64,
    environment_name: &str,
    inner_function: &'static ServerFunction,
) -> &'static ServerFunction {
    // This creates a fake copy of a Server Module. It represents the Server Action on the server.
    // We use an eval so we can source map it to the original location.

    let comment = "/* This module is a proxy to a Server Action. Turn on Source Maps to see the server source. */";

    if name.is_empty() {
        // An eval:ed function with no name gets the name "eval". We give it something more descriptive.
        name = "<anonymous>".to_owned();
    }
    let encoded_name = json::stringify(&name);
    // We generate code where both the beginning of the function and its parenthesis is at the line
    // and column of the server executed code. We use a method form since that lets us name it
    // anything we want and because the beginning of the function and its parenthesis is the same
    // column. Because Chrome inspects the location of the parenthesis and Firefox inspects the
    // location of the beginning of the function. By not using a function expression we avoid the
    // ambiguity.
    let mut code;
    if line <= 1.0 {
        let min_size = js::string::length(&encoded_name) as f64 + 7.0;
        code = "s=>({".to_owned()
            + &encoded_name
            + &js::string::repeat(" ", if col < min_size { 0.0 } else { col - min_size })
            + ":"
            + "(...args) => s(...args)"
            + "})\n"
            + comment;
    } else {
        code = comment.to_owned()
            + &js::string::repeat("\n", line - 2.0)
            + "server=>({"
            + &encoded_name
            + ":\n"
            + &js::string::repeat(" ", if col < 1.0 { 0.0 } else { col - 1.0 })
            // The function body can get printed so we make it look nice.
            // This "calls the server with the arguments".
            + "(...args) => server(...args)"
            + "})";
    }

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
        let idx = FAKE_SERVER_FUNCTION_IDX.get();
        FAKE_SERVER_FUNCTION_IDX.set(idx + 1.0);
        code += &("\n//# sourceURL=about://React/".to_owned()
            + &js::encode_uri_component(environment_name)
            + "/"
            + &js::encode_uri(&filename)
            + "?s" // We add an extra s here to distinguish from the fake stack frames
            + &idx.to_string());
        code += &("\n//# sourceMappingURL=".to_owned() + &source_map);
    } else if !filename.is_empty() {
        code += &("\n//# sourceURL=".to_owned() + &filename);
    }

    // Eval a factory and then call it to create a closure over the inner function.
    // eslint-disable-next-line no-eval
    match js::eval(&code) {
        Ok(factory) => {
            // SAFETY: the code is a factory of an object of the named function.
            let factory: &dyn Fn(&ServerFunction) -> Option<&'static Unknown> =
                unsafe { js::cast(factory) };
            // SAFETY: as the factory's.
            unsafe { js::cast(js::get(factory(inner_function), &*name)) }
        }
        // If eval fails, such as if in an environment that doesn't support it,
        // we fallback to just returning the inner function.
        Err(_) => inner_function,
    }
}

/// The properties a server reference is given where it's used with SSR.
struct ServerReferenceProperties<'a> {
    #[cfg_attr(rust_js, rust_js::name = "$$FORM_ACTION")]
    form_action: PropertyDescriptor<'a>,
    #[cfg_attr(rust_js, rust_js::name = "$$IS_SIGNATURE_EQUAL")]
    is_signature_equal: PropertyDescriptor<'a>,
    bind: PropertyDescriptor<'a>,
}

unsafe impl js::PropertyDescriptorMap for ServerReferenceProperties<'_> {}

pub fn register_bound_server_reference(
    reference: &'static ServerFunction,
    id: ServerReferenceId,
    bound: Option<&'static Thenable>,
    encode_form_action: Option<&'static EncodeFormActionCallback>,
) {
    if KNOWN_SERVER_REFERENCES.with(|references| references.has(reference)) {
        return;
    }

    KNOWN_SERVER_REFERENCES.with(|references| {
        references.set(
            reference,
            Rc::new(ServerReferenceClosure {
                id,
                original_bind: js::get(reference, "bind"),
                bound,
            }),
        )
    });

    // Expose encoder for use by SSR, as well as a special bind that can be used to
    // keep server capabilities.
    if USED_WITH_SSR {
        // Only expose this in builds that would actually use it. Not needed in the browser.
        let form_action: &'static dyn Fn(&'static str) -> ReactCustomFormAction =
            match encode_form_action {
                None => DEFAULT_ENCODE_FORM_ACTION
                    .with(|&default_encode_form_action| default_encode_form_action),
                Some(encode_form_action) => {
                    function::with_this(move |this, (identifier_prefix,): (&'static str,)| {
                        // SAFETY: it's only a server reference's `$$FORM_ACTION`, called on it.
                        custom_encode_form_action(
                            unsafe { js::cast(this) },
                            identifier_prefix,
                            encode_form_action,
                        )
                    })
                }
            };
        js::object::define_properties(
            reference,
            &ServerReferenceProperties {
                form_action: PropertyDescriptor {
                    value: Some(&form_action),
                    ..Default::default()
                },
                is_signature_equal: PropertyDescriptor {
                    value: Some(&IS_SIGNATURE_EQUAL.with(|&is_signature_equal| is_signature_equal)),
                    ..Default::default()
                },
                bind: PropertyDescriptor {
                    value: Some(&BIND.with(|&bind| bind)),
                    ..Default::default()
                },
            },
        );
    }
}

pub fn register_server_reference(
    reference: &'static ServerFunction,
    id: ServerReferenceId,
    encode_form_action: Option<&'static EncodeFormActionCallback>,
) -> &'static ServerFunction {
    register_bound_server_reference(reference, id, None, encode_form_action);
    reference
}

thread_local! {
    static SERVER_OBJECT_REFERENCE_PROXY_HANDLERS: ProxyHandler = ProxyHandler {
        get: Some(Box::new(|target, name, _| {
            if let Kind::String(name) = js::classify(Some(name)) {
                match name {
                    // These names are read by the Flight runtime if you end up passing this
                    // reference along.
                    "$$typeof" => return js::get(target, "$$typeof"),
                    "name" => return None,
                    "displayName" => return None,
                    // We need to special case this because createElement reads it if we pass this
                    // reference.
                    "defaultProps" => return None,
                    // React looks for debugInfo on thenables.
                    "_debugInfo" => return None,
                    // Avoid this attempting to be serialized.
                    "toJSON" => return None,
                    "then" => {
                        // Even if the referenced object is a Promise, it must not appear
                        // thenable on the client or it would be awaited instead of passed
                        // along by reference.
                        return None;
                    }
                    _ => {}
                }
            } else if ASYNC_ITERATOR.with(|&async_iterator| js::strict_equals(Some(name), Some(js::unknown(async_iterator)))) {
                // React also probes streamed objects for async iterator debug info.
                return None;
            } else if js::strict_equals(Some(name), Some(js::unknown(js::symbol::to_primitive))) {
                return js::get(js::object::prototype, js::symbol::to_primitive);
            } else if js::strict_equals(Some(name), Some(js::unknown(js::symbol::to_string_tag))) {
                return js::get(js::object::prototype, js::symbol::to_string_tag);
            }
            panic!(
                // eslint-disable-next-line react-internal/safe-string-coercion
                "Cannot access {} on the client. You cannot read a Server Reference to an object on the client. You can only pass it back to the server.",
                js::string(Some(name))
            );
        })),
        set: Some(Box::new(|_, _, _, _| {
            panic!("Cannot assign to a Server Reference to an object on the client.");
        })),
        ..Default::default()
    };
}

/// `{}`, what a server object reference proxies.
struct ServerObjectReferenceTarget {}

unsafe impl js::Defined for ServerObjectReferenceTarget {}
unsafe impl WeakKey for ServerObjectReferenceTarget {}

pub fn create_server_object_reference(id: ServerReferenceId) -> &'static Unknown {
    let reference = SERVER_OBJECT_REFERENCE_PROXY_HANDLERS
        .with(|handlers| js::proxy::new(&ServerObjectReferenceTarget {}, handlers));
    // Register the reference so that passing it back to the server encodes it
    // by id. Object references are tracked separately from function references
    // so that one kind can never be encoded as the other.
    KNOWN_SERVER_OBJECT_REFERENCES.with(|references| references.set(reference, id));
    js::unknown(reference)
}

#[cfg_attr(rust_js, rust_js::name = "ArraySlice")]
use js::array::SLICE as ARRAY_SLICE;
#[cfg_attr(rust_js, rust_js::name = "FunctionBind")]
use js::function::BIND as FUNCTION_BIND;

thread_local! {
    static BIND: &'static dyn Fn() -> Option<&'static Unknown> = function::with_this(|this, ()| {
        // SAFETY: it's only a server reference's `bind`, called on it.
        let reference: &ServerFunction = unsafe { js::cast(this) };
        let Some(reference_closure) = KNOWN_SERVER_REFERENCES.with(|references| references.get(reference)) else {
            return function::apply(FUNCTION_BIND, this.map(|this| this as &dyn js::IntoUnknown), Some(js::arguments()));
        };

        // SAFETY: a function's `bind` is a function.
        let original_bind: &ServerFunction = unsafe { js::cast(reference_closure.original_bind) };
        // SAFETY: what `bind` gives is a function.
        let new_fn: &'static ServerFunction = unsafe {
            js::cast(function::apply(original_bind, this.map(|this| this as &dyn js::IntoUnknown), Some(js::arguments())))
        };

        if __DEV__ {
            let this_bind = js::get(js::arguments(), &0.0);
            if !js::loose_equals(this_bind, js::null::<&Unknown>()) {
                // This doesn't warn in browser environments since it's not instrumented outside
                // usedWithSSR. This makes this an SSR only warning which we don't generally do.
                // TODO: Consider a DEV only instrumentation in the browser.
                console::error((
                    "Cannot bind \"this\" of a Server Action. Pass null or undefined as the first argument to .bind().",
                ));
            }
        }

        // SAFETY: `slice` makes an array.
        let args: Vec<Option<&'static Unknown>> =
            unsafe { js::cast(function::call(ARRAY_SLICE, Some(js::arguments()), (1.0,))) };
        let bound_promise: &'static Thenable = match reference_closure.bound {
            Some(bound) => js::upcast(js::promise::resolve(bound).then(move |bound_args: &'static Thenable| {
                // SAFETY: the bound arguments are an array.
                let bound_args: Vec<Option<&'static Unknown>> = unsafe { js::cast(Some(js::unknown(bound_args))) };
                [bound_args, args].concat()
            })),
            None => js::upcast(js::promise::resolve(args)),
        };

        KNOWN_SERVER_REFERENCES.with(|references| {
            references.set(
                new_fn,
                Rc::new(ServerReferenceClosure {
                    id: reference_closure.id,
                    original_bind: js::get(new_fn, "bind"),
                    bound: Some(bound_promise),
                }),
            )
        });

        // Expose encoder for use by SSR, as well as a special bind that can be used to
        // keep server capabilities.
        if USED_WITH_SSR {
            // Only expose this in builds that would actually use it. Not needed on the client.
            js::object::define_properties(
                new_fn,
                &ServerReferenceProperties {
                    form_action: PropertyDescriptor { value: Some(&js::get(this, "$$FORM_ACTION")), ..Default::default() },
                    is_signature_equal: PropertyDescriptor {
                        value: Some(&IS_SIGNATURE_EQUAL.with(|&is_signature_equal| is_signature_equal)),
                        ..Default::default()
                    },
                    bind: PropertyDescriptor { value: Some(&BIND.with(|&bind| bind)), ..Default::default() },
                },
            );
        }

        Some(js::unknown(new_fn))
    });
}

pub type FindSourceMapURLCallback = dyn Fn(&str, &str) -> Option<String>;

/// createBoundServerReference's `metaData`.
pub struct BoundServerReferenceMetaData {
    pub id: ServerReferenceId,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub bound: Option<&'static Thenable>,
    pub name: Option<String>, // DEV-only
    pub env: Option<String>,  // DEV-only
    pub location: Option<shared::react_types::ReactFunctionLocation>, // DEV-only
}

pub fn create_bound_server_reference(
    meta_data: &BoundServerReferenceMetaData,
    call_server: &'static CallServerCallback,
    encode_form_action: Option<&'static EncodeFormActionCallback>,
    find_source_map_url: Option<&FindSourceMapURLCallback>, // DEV-only
) -> &'static ServerFunction {
    let id = meta_data.id;
    let bound = meta_data.bound;
    let mut action: &'static ServerFunction =
        function::with_this(move |_, ()| -> Option<&'static Unknown> {
            // SAFETY: `slice` makes an array.
            let args: Vec<Option<&'static Unknown>> =
                unsafe { js::cast(function::call(js::array::SLICE, Some(js::arguments()), ())) };
            let p = bound;
            let Some(p) = p else {
                return Some(js::unknown_of(call_server(id, js::unknown_of(args))));
            };
            if p.status().as_deref() == Some("fulfilled") {
                // SAFETY: the bound arguments are an array.
                let bound_args: Vec<Option<&'static Unknown>> = unsafe { js::cast(p.value()) };
                return Some(js::unknown_of(call_server(
                    id,
                    js::unknown_of([bound_args, args].concat()),
                )));
            }
            // Since this is a fake Promise whose .then doesn't chain, we have to wrap it.
            // TODO: Remove the wrapper once that's fixed.
            Some(js::unknown_of(js::promise::resolve(p).then::<Option<
                &'static Unknown,
            >, _>(
                move |bound_args: &'static Thenable| {
                    // SAFETY: the bound arguments are an array.
                    let bound_args: Vec<Option<&'static Unknown>> =
                        unsafe { js::cast(Some(js::unknown(bound_args))) };
                    call_server(id, js::unknown_of([bound_args, args].concat()))
                },
            )))
        });
    if __DEV__ && let Some(location) = &meta_data.location {
        let function_name = meta_data
            .name
            .clone()
            .filter(|name| !name.is_empty())
            .unwrap_or_default();
        let (_, filename, line, col) = location.clone();
        let env = meta_data
            .env
            .clone()
            .filter(|env| !env.is_empty())
            .unwrap_or_else(|| "Server".to_owned());
        let source_map = find_source_map_url
            .and_then(|find_source_map_url| find_source_map_url(&filename, &env));
        action = create_fake_server_function(
            function_name,
            filename,
            source_map,
            line,
            col,
            &env,
            action,
        );
    }
    register_bound_server_reference(action, id, bound, encode_form_action);
    action
}

thread_local! {
    // This matches either of these V8 formats.
    //     at name (filename:0:0)
    //     at filename:0:0
    //     at async filename:0:0
    static V8_FRAME_REG_EXP: &'static js::RegExp =
        js::reg_exp::new(r"^ {3} at (?:(.+) \((.+):(\d+):(\d+)\)|(?:async )?(.+):(\d+):(\d+))$", "");
    // This matches either of these JSC/SpiderMonkey formats.
    // name@filename:0:0
    // filename:0:0
    static JSC_SPIDER_MONKEY_FRAME_REG_EXP: &'static js::RegExp = js::reg_exp::new(r"(?:(.*)@)?(.*):(\d+):(\d+)", "");
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn parse_stack_location(error: &js::Error) -> Option<shared::react_types::ReactFunctionLocation> {
    // This parsing is special in that we know that the calling function will always
    // be a module that initializes the server action. We also need this part to work
    // cross-browser so not worth a Config. It's DEV only so not super code size
    // sensitive but also a non-essential feature.
    let mut stack = error.stack().unwrap_or_default();
    if stack.starts_with("Error: react-stack-top-frame\n") {
        // V8's default formatting prefixes with the error message which we
        // don't want/need.
        stack = js::string::slice(&stack, 29.0, None);
    }
    let end_of_first = js::string::index_of(&stack, "\n", None);
    let second_frame = if end_of_first != -1.0 {
        // Skip the first frame.
        let end_of_second = js::string::index_of(&stack, "\n", Some(end_of_first + 1.0));
        if end_of_second == -1.0 {
            js::string::slice(&stack, end_of_first + 1.0, None)
        } else {
            js::string::slice(&stack, end_of_first + 1.0, Some(end_of_second))
        }
    } else {
        stack
    };

    let parsed = match V8_FRAME_REG_EXP.with(|reg_exp| reg_exp.exec(&second_frame)) {
        Some(parsed) => parsed,
        None => JSC_SPIDER_MONKEY_FRAME_REG_EXP.with(|reg_exp| reg_exp.exec(&second_frame))?,
    };

    let mut name = parsed
        .get(1)
        .filter(|name| !name.is_empty())
        .unwrap_or_default();
    if name == "<anonymous>" {
        name = String::new();
    }
    let mut filename = parsed
        .get(2)
        .filter(|filename| !filename.is_empty())
        .or_else(|| parsed.get(5))
        .filter(|filename| !filename.is_empty())
        .unwrap_or_default();
    if filename == "<anonymous>" {
        filename = String::new();
    }
    // This is really the enclosingLine/Column.
    let line = js::to_number(
        &parsed
            .get(3)
            .filter(|line| !line.is_empty())
            .or_else(|| parsed.get(6)),
    );
    let col = js::to_number(
        &parsed
            .get(4)
            .filter(|col| !col.is_empty())
            .or_else(|| parsed.get(7)),
    );

    Some((name, filename, line, col))
}

pub fn create_server_reference(
    id: ServerReferenceId,
    call_server: &'static CallServerCallback,
    encode_form_action: Option<&'static EncodeFormActionCallback>,
    find_source_map_url: Option<&FindSourceMapURLCallback>, // DEV-only
    function_name: Option<String>,
) -> &'static ServerFunction {
    let mut action: &'static ServerFunction =
        function::with_this(move |_, ()| -> Option<&'static Unknown> {
            // SAFETY: `slice` makes an array.
            let args: Vec<Option<&'static Unknown>> =
                unsafe { js::cast(function::call(js::array::SLICE, Some(js::arguments()), ())) };
            Some(js::unknown_of(call_server(id, js::unknown_of(args))))
        });
    if __DEV__ {
        // Let's see if we can find a source map for the file which contained the
        // server action. We extract it from the runtime so that it's resilient to
        // multiple passes of compilation as long as we can find the final source map.
        let location = parse_stack_location(js::error::new(Some("react-stack-top-frame"), None));
        if let Some((_, filename, line, col)) = location {
            // While the environment that the Server Reference points to can be
            // in any environment, what matters here is where the compiled source
            // is from and that's in the currently executing environment. We hard
            // code that as the value "Client" in case the findSourceMapURL helper
            // needs it.
            let env = "Client";
            let source_map = find_source_map_url
                .and_then(|find_source_map_url| find_source_map_url(&filename, env));
            action = create_fake_server_function(
                function_name.unwrap_or_default(),
                filename,
                source_map,
                line,
                col,
                env,
                action,
            );
        }
    }
    register_bound_server_reference(action, id, None, encode_form_action);
    action
}
