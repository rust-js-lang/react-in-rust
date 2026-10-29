// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::marker::PhantomData;

use js::{ArrayBuffer, ArrayBufferView, JsObject, Promise, Unknown};
use node::BufferEncoding;
use node::buffer::Buffer;
use webapi::Blob;
use webapi::global::btoa;

/// What a legacy render's stream writes to: a readable's `push`, which takes
/// a chunk, or `null` at the end, and its `destroy`.
pub struct Destination(PhantomData<JsObject>);

unsafe impl js::Defined for Destination {}

impl Destination {
    #[cfg_attr(rust_js, rust_js::link_name = "push")]
    #[cfg_attr(rust_js, rust_js::nullable(chunk))]
    pub fn push(&self, chunk: Option<&str>) -> bool {
        let _ = chunk;
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "destroy")]
    pub fn destroy(&self, error: Option<&Unknown>) -> Option<&'static Unknown> {
        let _ = error;
        unreachable!()
    }
}

pub type PrecomputedChunk = String;
pub type Chunk = String;
pub type BinaryChunk = String;

pub fn schedule_work(callback: &dyn Fn()) {
    callback();
}

pub fn schedule_microtask(callback: &dyn Fn()) {
    // While this defies the method name the legacy builds have special
    // overrides that make work scheduling sync. At the moment scheduleMicrotask
    // isn't used by any legacy APIs so this is somewhat academic but if they
    // did in the future we'd probably want to have this be in sync with scheduleWork
    callback();
}

pub fn flush_buffered(_destination: &Destination) {}

pub fn begin_writing(_destination: &Destination) {}

pub fn write_chunk(destination: &Destination, chunk: &str) {
    write_chunk_and_return(destination, chunk);
}

pub fn write_chunk_and_return(destination: &Destination, chunk: &str) -> bool {
    destination.push(Some(chunk))
}

pub fn complete_writing(_destination: &Destination) {}

pub fn close(destination: &Destination) {
    destination.push(None);
}

pub fn string_to_chunk(content: String) -> Chunk {
    content
}

pub fn string_to_precomputed_chunk(content: String) -> PrecomputedChunk {
    content
}

pub fn typed_array_to_binary_chunk(_content: &ArrayBufferView) -> BinaryChunk {
    panic!("Not implemented.");
}

#[cfg_attr(rust_js, rust_js::name = "byteLengthOfChunk")]
#[cfg_attr(rust_js, rust_js::nullable)]
pub const BYTE_LENGTH_OF_CHUNK: Option<fn(&str) -> f64> = None;

pub fn byte_length_of_binary_chunk(_chunk: &str) -> f64 {
    panic!("Not implemented.");
}

pub fn close_with_error(destination: &Destination, error: Option<&Unknown>) {
    destination.destroy(error);
}

pub use react_server::create_fast_hash_js::create_fast_hash_js as create_fast_hash;

#[cfg_attr(rust_js, rust_js::name = "readAsDataURL")]
pub fn read_as_data_url(blob: &'static Blob) -> Promise<String> {
    blob.array_buffer()
        .then(move |array_buffer: &'static ArrayBuffer| {
            let encoded = if js::is_function(Buffer::new)
                && js::is_function(Buffer::from_with_array_buffer)
            {
                Buffer::from_with_array_buffer(array_buffer, None, None).to_string(
                    Some(BufferEncoding::Base64),
                    None,
                    None,
                )
            } else {
                btoa(&js::string::from_char_code(
                    js::uint8_array::new_with_buffer(array_buffer, None, None),
                ))
            };
            let mime_type = Some(blob.r#type())
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| "application/octet-stream".to_string());
            "data:".to_string() + &mime_type + ";base64," + &encoded
        })
}
