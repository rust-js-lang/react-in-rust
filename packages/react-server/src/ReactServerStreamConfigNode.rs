// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;
use std::marker::PhantomData;
use std::ops::Deref;

use js::{
    ArrayBufferView, JsError, JsObject, Kind, Promise, Uint8Array, Unknown, array_buffer,
    uint8_array,
};
use node::buffer::Buffer;
use node::stream::Writable;
use shared::environment::__DEV__;
use webapi::{Blob, TextEncoderEncodeIntoResult, console};

use node::util::{TextEncoder, text_encoder};

use node::crypto::{BinaryToTextEncoding, create_hash};

use shared::has_own_property::HAS_OWN_PROPERTY;

/// A destination that might be flushable, `interface MightBeFlushable`.
pub struct Destination(PhantomData<JsObject>);

impl Deref for Destination {
    type Target = Writable;

    fn deref(&self) -> &Writable {
        unsafe { &*(self as *const Self).cast::<Writable>() }
    }
}

impl Destination {
    /// `flush?: () => void`.
    #[cfg_attr(rust_js, rust_js::link_name = "flush")]
    pub fn flush(&self) {
        unreachable!()
    }
}

pub type PrecomputedChunk = Uint8Array;
pub type BinaryChunk = Uint8Array;

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "supportsArrayBufferDetached")]
    static SUPPORTS_ARRAY_BUFFER_DETACHED: bool = HAS_OWN_PROPERTY.with(|has_own_property| {
        has_own_property.call(js::unknown(array_buffer::prototype()), "detached")
    });
    static DID_WARN_ABOUT_MISSING_ARRAY_BUFFER_DETACHED: Cell<bool> = const { Cell::new(false) };
}

pub fn schedule_work(callback: Box<dyn FnOnce()>) {
    js::set_immediate(callback);
}

thread_local! {
    pub static SCHEDULE_MICROTASK: fn(Box<dyn FnOnce()>) = webapi::global::queue_microtask;
}

pub fn flush_buffered(destination: &Destination) {
    // If we don't have any more data to send right now.
    // Flush whatever is in the buffer to the wire.
    if js::is_function((destination, Destination::flush)) {
        // By convention the Zlib streams provide a flush function for this purpose.
        // For Express, compression middleware adds this method.
        destination.flush();
    }
}

// A chunk larger than VIEW_SIZE is written to the destination as is. A smaller
// chunk is copied into currentView, which React allocates and owns, so that the
// destination receives fewer and larger writes.
const VIEW_SIZE: f64 = 4096.0;

thread_local! {
    static CURRENT_VIEW: Cell<Option<&'static Uint8Array>> = const { Cell::new(None) };
    static WRITTEN_BYTES: Cell<f64> = const { Cell::new(0.0) };
    static DESTINATION_HAS_CAPACITY: Cell<bool> = const { Cell::new(true) };
}

pub fn begin_writing(_destination: &Destination) {
    CURRENT_VIEW.set(Some(uint8_array::new(VIEW_SIZE)));
    WRITTEN_BYTES.set(0.0);
    DESTINATION_HAS_CAPACITY.set(true);
}

fn write_string_chunk(destination: &Destination, string_chunk: &str) {
    if js::string::length(string_chunk) == 0 {
        return;
    }
    // maximum possible view needed to encode entire string
    if f64::from(js::string::length(string_chunk)) * 3.0 > VIEW_SIZE {
        if WRITTEN_BYTES.get() > 0.0 {
            write_to_destination(
                destination,
                CURRENT_VIEW
                    .get()
                    .unwrap()
                    .subarray(Some(0.0), Some(WRITTEN_BYTES.get())),
            );
            CURRENT_VIEW.set(Some(uint8_array::new(VIEW_SIZE)));
            WRITTEN_BYTES.set(0.0);
        }
        // Write the raw string chunk and let the consumer handle the encoding.
        write_to_destination(destination, string_chunk);
        return;
    }

    let mut target = CURRENT_VIEW.get().unwrap();
    if WRITTEN_BYTES.get() > 0.0 {
        target = CURRENT_VIEW
            .get()
            .unwrap()
            .subarray(Some(WRITTEN_BYTES.get()), None);
    }
    let TextEncoderEncodeIntoResult { read, written } =
        TEXT_ENCODER.with(|text_encoder| text_encoder.encode_into(string_chunk, target));
    WRITTEN_BYTES.set(WRITTEN_BYTES.get() + written);

    if read < f64::from(js::string::length(string_chunk)) {
        write_to_destination(
            destination,
            CURRENT_VIEW
                .get()
                .unwrap()
                .subarray(Some(0.0), Some(WRITTEN_BYTES.get())),
        );
        CURRENT_VIEW.set(Some(uint8_array::new(VIEW_SIZE)));
        WRITTEN_BYTES.set(
            TEXT_ENCODER
                .with(|text_encoder| {
                    text_encoder.encode_into(
                        &js::string::slice(string_chunk, read, None),
                        CURRENT_VIEW.get().unwrap(),
                    )
                })
                .written,
        );
    }

    if WRITTEN_BYTES.get() == VIEW_SIZE {
        write_to_destination(destination, CURRENT_VIEW.get().unwrap());
        CURRENT_VIEW.set(Some(uint8_array::new(VIEW_SIZE)));
        WRITTEN_BYTES.set(0.0);
    }
}

fn write_view_chunk(destination: &Destination, chunk: &Uint8Array) {
    if chunk.byte_length() == 0.0 {
        if SUPPORTS_ARRAY_BUFFER_DETACHED.with(|supports| *supports) {
            if chunk.buffer().detached() {
                panic!(
                    "{}",
                    concat!(
                        "Cannot transport a detached buffer. ",
                        "This usually happens when trying to transport multiple buffer-spanning views on the same underyling buffer. ",
                        "Either reuse the exact same view by identity or copy the underlying buffer manually via `view.slice()`."
                    )
                );
            }
        } else if __DEV__ && !DID_WARN_ABOUT_MISSING_ARRAY_BUFFER_DETACHED.get() {
            console::error((concat!(
                "React encountered an empty buffer when writing in an environment that ",
                "does not support ArrayBuffer.prototype.detached. ",
                "This could be a security issue if the buffer was not actually empty. ",
                "Please upgrade to a Node.js version that supports ArrayBuffer.prototype.detached."
            ),));
            DID_WARN_ABOUT_MISSING_ARRAY_BUFFER_DETACHED.set(true);
        }

        return;
    }
    if chunk.byte_length() > VIEW_SIZE {
        // this chunk may overflow a single view which implies it was not
        // one that is cached by the streaming renderer. We will enqueu
        // it directly and expect it is not re-used
        if WRITTEN_BYTES.get() > 0.0 {
            write_to_destination(
                destination,
                CURRENT_VIEW
                    .get()
                    .unwrap()
                    .subarray(Some(0.0), Some(WRITTEN_BYTES.get())),
            );
            CURRENT_VIEW.set(Some(uint8_array::new(VIEW_SIZE)));
            WRITTEN_BYTES.set(0.0);
        }
        write_to_destination(destination, chunk);
        return;
    }

    let mut bytes_to_write = chunk;
    let allowable_bytes = CURRENT_VIEW.get().unwrap().length() - WRITTEN_BYTES.get();
    if allowable_bytes < bytes_to_write.byte_length() {
        // this chunk would overflow the current view. We enqueue a full view
        // and start a new view with the remaining chunk
        if allowable_bytes == 0.0 {
            // the current view is already full, send it
            write_to_destination(destination, CURRENT_VIEW.get().unwrap());
        } else {
            // fill up the current view and apply the remaining chunk bytes
            // to a new view.
            CURRENT_VIEW.get().unwrap().set(
                bytes_to_write.subarray(Some(0.0), Some(allowable_bytes)),
                Some(WRITTEN_BYTES.get()),
            );
            WRITTEN_BYTES.set(WRITTEN_BYTES.get() + allowable_bytes);
            write_to_destination(destination, CURRENT_VIEW.get().unwrap());
            bytes_to_write = bytes_to_write.subarray(Some(allowable_bytes), None);
        }
        CURRENT_VIEW.set(Some(uint8_array::new(VIEW_SIZE)));
        WRITTEN_BYTES.set(0.0);
    }
    CURRENT_VIEW
        .get()
        .unwrap()
        .set(bytes_to_write, Some(WRITTEN_BYTES.get()));
    WRITTEN_BYTES.set(WRITTEN_BYTES.get() + bytes_to_write.byte_length());

    if WRITTEN_BYTES.get() == VIEW_SIZE {
        write_to_destination(destination, CURRENT_VIEW.get().unwrap());
        CURRENT_VIEW.set(Some(uint8_array::new(VIEW_SIZE)));
        WRITTEN_BYTES.set(0.0);
    }
}

/// `chunk`, a `PrecomputedChunk | Chunk | BinaryChunk`: a chunk's text, or
/// its bytes.
pub fn write_chunk(destination: &Destination, chunk: &Unknown) {
    if let Kind::String(chunk) = js::classify(Some(chunk)) {
        write_string_chunk(destination, chunk);
    } else {
        write_view_chunk(destination, unsafe { js::cast(Some(chunk)) });
    }
}

fn write_to_destination(destination: &Destination, view: impl node::stream::IntoChunk) {
    let current_has_capacity = destination.write(view);
    DESTINATION_HAS_CAPACITY.set(DESTINATION_HAS_CAPACITY.get() && current_has_capacity);
}

pub fn write_chunk_and_return(destination: &Destination, chunk: &Unknown) -> bool {
    write_chunk(destination, chunk);
    DESTINATION_HAS_CAPACITY.get()
}

pub fn complete_writing(destination: &Destination) {
    if let Some(current_view) = CURRENT_VIEW.get()
        && WRITTEN_BYTES.get() > 0.0
    {
        destination.write(current_view.subarray(Some(0.0), Some(WRITTEN_BYTES.get())));
    }
    CURRENT_VIEW.set(None);
    WRITTEN_BYTES.set(0.0);
    DESTINATION_HAS_CAPACITY.set(true);
}

pub fn close(destination: &Destination) {
    destination.end();
}

thread_local! {
    pub static TEXT_ENCODER: &'static TextEncoder = text_encoder::new();
}

pub fn string_to_chunk(content: &str) -> String {
    content.to_owned()
}

pub fn string_to_precomputed_chunk(content: &str) -> &'static PrecomputedChunk {
    let precomputed_chunk = TEXT_ENCODER.with(|text_encoder| text_encoder.encode(Some(content)));

    if __DEV__ && precomputed_chunk.byte_length() > VIEW_SIZE {
        console::error((
            "precomputed chunks must be smaller than the view size configured for this host. This is a bug in React.",
        ));
    }

    precomputed_chunk
}

pub fn typed_array_to_binary_chunk(content: &ArrayBufferView) -> &'static BinaryChunk {
    // Convert any non-Uint8Array array to Uint8Array. We could avoid this for Uint8Arrays.
    let view = uint8_array::new_with_buffer(
        content.buffer(),
        Some(content.byte_offset()),
        Some(content.byte_length()),
    );
    // A byte stream transfers, and thereby detaches, the whole ArrayBuffer of
    // every chunk it receives. React writes a chunk larger than VIEW_SIZE
    // directly to the destination, so such a chunk must own its whole buffer.
    //
    // The render consumes a typed array that spans its whole buffer, in the same
    // way that it consumes a one shot iterable.
    //
    // React copies a view into a part of a buffer, because the rest of that
    // buffer does not belong to this chunk. Node serves an allocation below
    // `Buffer.poolSize >>> 1` from its own internal view that it shares between unrelated
    // Buffers, so a Buffer of that size is such a view.
    if content.byte_length() > VIEW_SIZE {
        // TODO: We should not slice if the Destination does not detach the underlying buffer.
        // Then we can also add a dev warning if we see the same underlying buffer
        // in multiple buffer-spanning views.

        // As a heuristic we assume partial views mean multiple views on the same
        // underlying buffer are being transported. We slice to avoid buffer detachment.
        let is_view_buffer_spanning =
            content.byte_offset() == 0.0 && content.byte_length() == content.buffer().byte_length();
        if !is_view_buffer_spanning
            // To allow resizing buffers after they have been transported.
            // Also as a precaution in case the underlying buffer gets resized between
            // render and write.
            || content.buffer().resizable()
        {
            return view.slice(None, None);
        }
        if !SUPPORTS_ARRAY_BUFFER_DETACHED.with(|supports| *supports) {
            // We're defensively slicing for older environments where we can't reliably
            // check later if the ArrayBuffer became detached.
            if __DEV__ && !DID_WARN_ABOUT_MISSING_ARRAY_BUFFER_DETACHED.get() {
                console::error((concat!(
                    "React copied a partial view into a fresh buffer as a safety precaution ",
                    "because the environment does not implement ArrayBuffer.prototype.detached. ",
                    "This negatively impacts performance. ",
                    "Please upgrade to a modern Node.js version."
                ),));
                DID_WARN_ABOUT_MISSING_ARRAY_BUFFER_DETACHED.set(true);
            }
            return view.slice(None, None);
        }
    }
    // These are likely unique views on the underlying buffer so we can transfer them safely.
    // We'll check again when writing if the underlying buffer became detached.
    view
}

/// A `Chunk | PrecomputedChunk`'s length in bytes.
pub fn byte_length_of_chunk(chunk: &Unknown) -> f64 {
    match js::classify(Some(chunk)) {
        Kind::String(chunk) => {
            f64::from(Buffer::byte_length(chunk, Some(node::BufferEncoding::Utf8)))
        }
        _ => unsafe { js::cast::<&Uint8Array>(Some(chunk)) }.byte_length(),
    }
}

pub fn byte_length_of_binary_chunk(chunk: &BinaryChunk) -> f64 {
    chunk.byte_length()
}

pub fn close_with_error(destination: &Destination, error: Option<&Unknown>) {
    // $FlowFixMe[incompatible-type]: This is an Error object or the destination accepts other types.
    destination.destroy(unsafe { js::cast::<Option<&JsError>>(error) });
}

pub fn create_fast_hash(input: &str) -> String {
    let hash = create_hash("md5");
    hash.update(input);
    hash.digest(BinaryToTextEncoding::Hex)
}

#[cfg_attr(rust_js, rust_js::name = "readAsDataURL")]
pub fn read_as_data_url(blob: &'static Blob) -> Promise<String> {
    blob.array_buffer().then(move |array_buffer| {
        let encoded = Buffer::from_with_array_buffer(array_buffer, None, None).to_string(
            Some(node::BufferEncoding::Base64),
            None,
            None,
        );
        let r#type = blob.r#type();
        let mime_type = if r#type.is_empty() {
            "application/octet-stream"
        } else {
            &r#type
        };
        "data:".to_owned() + mime_type + ";base64," + &encoded
    })
}
