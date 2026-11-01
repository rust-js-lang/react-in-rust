// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};

use js::{ArrayBufferView, Promise, Unknown};
use webapi::Blob;

pub struct Destination {
    pub buffer: RefCell<String>,
    pub done: Cell<bool>,
    pub fatal: Cell<bool>,
    pub error: Cell<Option<&'static Unknown>>,
}

unsafe impl js::Defined for Destination {}

pub fn schedule_microtask(callback: Box<dyn FnOnce()>) {
    // TODO: Consider unifying this with the FB Flight stream config and
    // adopting its microtask scheduling implementation.
    callback();
}

pub fn schedule_work(callback: Box<dyn FnOnce()>) {
    // TODO: Consider unifying this with the FB Flight stream config and
    // adopting its work scheduling implementation.
    callback();
}

pub fn flush_buffered(_destination: &Destination) {}

#[cfg_attr(rust_js, rust_js::name = "supportsRequestStorage")]
pub const SUPPORTS_REQUEST_STORAGE: bool = false;

thread_local! {
    pub static REQUEST_STORAGE: Option<&'static Unknown> = const { js::null() };
}

pub fn begin_writing(_destination: &Destination) {}

pub fn write_chunk(destination: &Destination, chunk: &str) {
    destination.buffer.borrow_mut().push_str(chunk);
}

pub fn write_chunk_and_return(destination: &Destination, chunk: &str) -> bool {
    destination.buffer.borrow_mut().push_str(chunk);
    true
}

pub fn complete_writing(_destination: &Destination) {}

pub fn close(destination: &Destination) {
    destination.done.set(true);
}

pub fn string_to_chunk(content: &str) -> String {
    content.to_owned()
}

pub fn string_to_precomputed_chunk(content: &str) -> String {
    content.to_owned()
}

pub fn typed_array_to_binary_chunk(_content: &ArrayBufferView) -> String {
    panic!("Not implemented.");
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static BYTE_LENGTH_OF_CHUNK: Option<fn(&str) -> f64> = const { None };
}

pub fn byte_length_of_binary_chunk(_chunk: &str) -> f64 {
    panic!("Not implemented.");
}

pub fn close_with_error(destination: &Destination, error: Option<&'static Unknown>) {
    destination.done.set(true);
    destination.fatal.set(true);
    destination.error.set(error);
}

pub use crate::create_fast_hash_js::create_fast_hash_js as create_fast_hash;

#[cfg_attr(rust_js, rust_js::name = "readAsDataURL")]
pub fn read_as_data_url(_blob: &Blob) -> Promise<String> {
    panic!("Not implemented.");
}
