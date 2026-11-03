// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

/* global Bun */

use std::marker::PhantomData;

use js::{ArrayBufferView, JsObject, Promise, Unknown};
use node::buffer::Buffer;
use webapi::Blob;

/// Where a request writes: a Bun direct stream's controller,
/// `BunReadableStreamController`, `ReadableStreamController & { end(),
/// write(data), error(error), flush?() }`, or a Node `Writable &
/// MightBeFlushable`, read by what the two have alike.
#[cfg_attr(
    rust_js,
    rust_js::types = "ReadableStreamController<any> & { end(): unknown; write(data: string | ArrayBufferView): void; error(error: Error): void; flush?(): void } | stream#Writable & { flush?(): void }"
)]
pub struct Destination(PhantomData<JsObject>);

// Safety: a destination is a JS object, a controller or a Writable.
unsafe impl js::Defined for Destination {}

impl Destination {
    /// `write(data)`: the controller's, or the Writable's, whose result
    /// says whether it would take more now.
    #[cfg_attr(rust_js, rust_js::link_name = "write")]
    pub fn write(&self, data: &Unknown) -> Option<&'static Unknown> {
        let _ = data;
        unreachable!()
    }

    /// `end()`: either's.
    #[cfg_attr(rust_js, rust_js::link_name = "end")]
    pub fn end(&self) {
        unreachable!()
    }

    /// `flush?: () => void`, a Bun direct stream's, and a Writable's that
    /// might be flushable.
    #[cfg_attr(rust_js, rust_js::link_name = "flush")]
    pub fn flush(&self) {
        unreachable!()
    }

    /// `error(error)`, the controller's.
    #[cfg_attr(rust_js, rust_js::link_name = "error")]
    pub fn error(&self, error: Option<&Unknown>) {
        let _ = error;
        unreachable!()
    }

    /// `destroy(error)`, the Writable's.
    #[cfg_attr(rust_js, rust_js::link_name = "destroy")]
    pub fn destroy(&self, error: Option<&Unknown>) {
        let _ = error;
        unreachable!()
    }

    /// `close()`, an earlier controller's.
    #[cfg_attr(rust_js, rust_js::link_name = "close")]
    pub fn close(&self) {
        unreachable!()
    }
}

pub type PrecomputedChunk = String;
pub type Chunk = String;
pub type BinaryChunk = ArrayBufferView;

pub fn schedule_work(callback: Box<dyn FnOnce()>) {
    js::set_timeout(callback, Some(0.0));
}

thread_local! {
    pub static SCHEDULE_MICROTASK: fn(Box<dyn FnOnce()>) = webapi::global::queue_microtask;
}

pub fn flush_buffered(destination: &Destination) {
    // Bun direct streams provide a flush function.
    // If we don't have any more data to send right now.
    // Flush whatever is in the buffer to the wire.
    if js::is_function((destination, Destination::flush)) {
        destination.flush();
    }
}

pub fn begin_writing(_destination: &Destination) {}

pub fn write_chunk(destination: &Destination, chunk: &Unknown) {
    if js::strict_equals(js::get(chunk, "length"), 0.0) {
        return;
    }

    // $FlowFixMe[incompatible-type]: write() is compatible with both types in Bun
    destination.write(chunk);
}

pub fn write_chunk_and_return(destination: &Destination, chunk: &Unknown) -> bool {
    // $FlowFixMe[incompatible-type]: write() is compatible with both types in Bun
    js::truthy(destination.write(chunk))
}

pub fn complete_writing(_destination: &Destination) {}

pub fn close(destination: &Destination) {
    destination.end();
}

pub fn string_to_chunk(content: &str) -> Chunk {
    content.to_owned()
}

pub fn string_to_precomputed_chunk(content: &str) -> PrecomputedChunk {
    content.to_owned()
}

pub fn typed_array_to_binary_chunk(content: &'static ArrayBufferView) -> &'static BinaryChunk {
    // TODO: Does this needs to be cloned if it's transferred in enqueue()?
    content
}

pub fn byte_length_of_chunk(chunk: &str) -> u32 {
    Buffer::byte_length(chunk, Some(node::BufferEncoding::Utf8))
}

pub fn byte_length_of_binary_chunk(chunk: &BinaryChunk) -> f64 {
    chunk.byte_length()
}

pub fn close_with_error(destination: &Destination, error: Option<&Unknown>) {
    // $FlowFixMe[incompatible-use]
    // $FlowFixMe[method-unbinding]
    if js::is_function((destination, Destination::error)) {
        // $FlowFixMe[incompatible-type]: This is an Error object or the destination accepts other types.
        destination.error(error);

        // $FlowFixMe[incompatible-use]
        // $FlowFixMe[method-unbinding]
    } else if js::is_function((destination, Destination::destroy)) {
        // $FlowFixMe[incompatible-type]: This is an Error object or the destination accepts other types.
        destination.destroy(error);

        // $FlowFixMe[incompatible-use]
        // $FlowFixMe[method-unbinding]
    } else if js::is_function((destination, Destination::close)) {
        // Earlier implementations doesn't support this method. In that environment you're
        // supposed to throw from a promise returned but we don't return a promise in our
        // approach. We could fork this implementation but this is environment is an edge
        // case to begin with. It's even less common to run this in an older environment.
        // Even then, this is not where errors are supposed to happen and they get reported
        // to a global callback in addition to this anyway. So it's fine just to close this.
        destination.close();
    }
}

pub fn create_fast_hash(input: &str) -> u64 {
    bun::hash(input, None)
}

#[cfg_attr(rust_js, rust_js::name = "readAsDataURL")]
pub fn read_as_data_url(blob: &'static Blob) -> Promise<String> {
    blob.array_buffer().then(move |array_buffer| {
        let encoded = Buffer::from_with_array_buffer(array_buffer, None, None).to_string(
            Some(node::BufferEncoding::Base64),
            None,
            None,
        );
        // GAP rd664: the type, read once and tested, is named, `const type =
        // blob.type;`, where upstream reads `blob.type || ..` in place.
        let r#type = blob.r#type();
        let mime_type = if r#type.is_empty() {
            "application/octet-stream"
        } else {
            &r#type
        };
        "data:".to_owned() + mime_type + ";base64," + &encoded
    })
}
