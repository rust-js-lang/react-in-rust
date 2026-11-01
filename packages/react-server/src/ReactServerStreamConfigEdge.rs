// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;

use js::{
    ArrayBufferView, Class, JsError, Promise, Uint8Array, Unknown, array_buffer, uint8_array,
};
use node::buffer::Buffer;
use shared::environment::__DEV__;
use shared::has_own_property::HAS_OWN_PROPERTY;
use webapi::{Blob, ReadableByteStreamController, TextEncoder, console, global, text_encoder};

pub type Destination = ReadableByteStreamController;

pub type PrecomputedChunk = Uint8Array;
pub type Chunk = Uint8Array;
pub type BinaryChunk = Uint8Array;

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "supportsArrayBufferDetached")]
    static SUPPORTS_ARRAY_BUFFER_DETACHED: bool = HAS_OWN_PROPERTY.with(|has_own_property| {
        has_own_property.call(js::unknown(array_buffer::prototype()), "detached")
    });
    static DID_WARN_ABOUT_MISSING_ARRAY_BUFFER_DETACHED: Cell<bool> = const { Cell::new(false) };
}

fn handle_error_in_next_tick(error: &'static JsError) {
    js::set_timeout(move || js::throw(error.as_unknown()), None);
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "LocalPromise")]
    static LOCAL_PROMISE: &'static Class = js::class::<Promise<()>>();
}

type ScheduleMicrotask = fn(Box<dyn FnOnce()>);

thread_local! {
    pub static SCHEDULE_MICROTASK: ScheduleMicrotask = if js::is_function(global::queue_microtask) {
        global::queue_microtask
    } else {
        |callback| {
            LOCAL_PROMISE.with(|local_promise| {
                local_promise
                    .call_static(js::promise::resolve, (js::null::<&Unknown>(),))
                    .then::<(), _>(move |_| callback())
                    .catch(handle_error_in_next_tick);
            });
        }
    };
}

pub fn schedule_work(callback: Box<dyn FnOnce()>) {
    js::set_timeout(callback, Some(0.0));
}

pub fn flush_buffered(_destination: &Destination) {
    // WHATWG Streams do not yet have a way to flush the underlying
    // transform streams. https://github.com/whatwg/streams/issues/960
}

// A chunk larger than VIEW_SIZE is written to the destination as is. A smaller
// chunk is copied into currentView, which React allocates and owns, so that the
// destination receives fewer and larger writes.
const VIEW_SIZE: f64 = 4096.0;

thread_local! {
    static CURRENT_VIEW: Cell<Option<&'static Uint8Array>> = const { Cell::new(None) };
    static WRITTEN_BYTES: Cell<f64> = const { Cell::new(0.0) };
}

pub fn begin_writing(_destination: &Destination) {
    CURRENT_VIEW.set(Some(uint8_array::new(VIEW_SIZE)));
    WRITTEN_BYTES.set(0.0);
}

pub fn write_chunk(destination: &Destination, chunk: &Uint8Array) {
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
                "Please use a modern Edge runtime that supports ArrayBuffer.prototype.detached."
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
            destination.enqueue(uint8_array::new_with_buffer(
                CURRENT_VIEW.get().unwrap().buffer(),
                Some(0.0),
                Some(WRITTEN_BYTES.get()),
            ));
            CURRENT_VIEW.set(Some(uint8_array::new(VIEW_SIZE)));
            WRITTEN_BYTES.set(0.0);
        }
        destination.enqueue(chunk);
        return;
    }

    let mut bytes_to_write = chunk;
    let allowable_bytes = CURRENT_VIEW.get().unwrap().length() - WRITTEN_BYTES.get();
    if allowable_bytes < bytes_to_write.byte_length() {
        // this chunk would overflow the current view. We enqueue a full view
        // and start a new view with the remaining chunk
        if allowable_bytes == 0.0 {
            // the current view is already full, send it
            destination.enqueue(CURRENT_VIEW.get().unwrap());
        } else {
            // fill up the current view and apply the remaining chunk bytes
            // to a new view.
            CURRENT_VIEW.get().unwrap().set(
                bytes_to_write.subarray(Some(0.0), Some(allowable_bytes)),
                Some(WRITTEN_BYTES.get()),
            );
            // writtenBytes += allowableBytes; // this can be skipped because we are going to immediately reset the view
            destination.enqueue(CURRENT_VIEW.get().unwrap());
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
}

pub fn write_chunk_and_return(destination: &Destination, chunk: &Uint8Array) -> bool {
    write_chunk(destination, chunk);
    // in web streams there is no backpressure so we can alwas write more
    true
}

pub fn complete_writing(destination: &Destination) {
    if let Some(current_view) = CURRENT_VIEW.get()
        && WRITTEN_BYTES.get() > 0.0
    {
        destination.enqueue(uint8_array::new_with_buffer(
            current_view.buffer(),
            Some(0.0),
            Some(WRITTEN_BYTES.get()),
        ));
        CURRENT_VIEW.set(None);
        WRITTEN_BYTES.set(0.0);
    }
}

pub fn close(destination: &Destination) {
    destination.close();
}

thread_local! {
    static TEXT_ENCODER: &'static TextEncoder = text_encoder::new();
}

pub fn string_to_chunk(content: &str) -> &'static Chunk {
    TEXT_ENCODER.with(|text_encoder| text_encoder.encode(Some(content)))
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
    // If we passed through this straight to enqueue we wouldn't have to convert it but since
    // we need to copy the buffer in that case, we need to convert it to copy it.
    // When we copy it into another array using set() it needs to be a Uint8Array.
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
    // buffer does not belong to this chunk.
    if content.byte_length() > VIEW_SIZE {
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
                    "Please upgrade to a modern Edge runtime."
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

pub fn byte_length_of_chunk(chunk: &Uint8Array) -> f64 {
    chunk.byte_length()
}

pub fn byte_length_of_binary_chunk(chunk: &BinaryChunk) -> f64 {
    chunk.byte_length()
}

pub fn close_with_error(destination: &Destination, error: Option<&'static Unknown>) {
    if js::is_function((destination, ReadableByteStreamController::error)) {
        // $FlowFixMe[incompatible-type]: This is an Error object or the destination accepts other types.
        destination.error(error.map(|error| error as &dyn js::IntoUnknown));
    } else {
        // Earlier implementations doesn't support this method. In that environment you're
        // supposed to throw from a promise returned but we don't return a promise in our
        // approach. We could fork this implementation but this is environment is an edge
        // case to begin with. It's even less common to run this in an older environment.
        // Even then, this is not where errors are supposed to happen and they get reported
        // to a global callback in addition to this anyway. So it's fine just to close this.
        destination.close();
    }
}

pub use crate::create_fast_hash_js::create_fast_hash_js as create_fast_hash;

#[cfg_attr(rust_js, rust_js::name = "readAsDataURL")]
pub fn read_as_data_url(blob: &'static Blob) -> Promise<String> {
    blob.array_buffer().then(move |array_buffer| {
        let encoded = if js::is_function(Buffer::from_with_array_buffer) {
            Buffer::from_with_array_buffer(array_buffer, None, None).to_string(
                Some(node::BufferEncoding::Base64),
                None,
                None,
            )
        } else {
            global::btoa(&js::string::from_char_code(uint8_array::new_with_buffer(
                array_buffer,
                None,
                None,
            )))
        };
        let r#type = blob.r#type();
        let mime_type = if r#type.is_empty() {
            "application/octet-stream"
        } else {
            &r#type
        };
        "data:".to_owned() + mime_type + ";base64," + &encoded
    })
}
