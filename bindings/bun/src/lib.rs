//! Bindings of [Bun](https://bun.sh)'s own APIs, as bun-types 1.4 types
//! them: [`hash`], `Bun.hash`, and Bun's direct `ReadableStream`, made of a
//! [`DirectUnderlyingSource`] whose `pull` is given a
//! [`ReadableStreamDirectController`] to write to.
//!
//! ```rust,ignore
//! use bun::{DirectUnderlyingSource, ReadableStreamDirectController};
//! use webapi::readable_stream;
//!
//! let stream = readable_stream::new(
//!     Some(&DirectUnderlyingSource {
//!         r#type: "direct",
//!         pull: &|controller: &'static ReadableStreamDirectController| {
//!             controller.write("Hello");
//!             controller.end();
//!         },
//!         cancel: None,
//!     }),
//!     None,
//! );
//! ```

// A binding's body never runs, so rustc's crate sees its parameters unread
// (rd330, the user's one crate-level allow for a bindings crate).
#![cfg_attr(not(rust_js), allow(unused_variables))]

use std::marker::PhantomData;

use js::{
    ArrayBuffer, ArrayBufferView, DataView, JsError, JsObject, Promise, SharedArrayBuffer,
    Uint8Array, Unknown,
};

/// `Bun.hash(data, seed?)`: `data`'s Wyhash, a 64-bit `bigint`. Not a
/// cryptographic hash.
#[cfg_attr(rust_js, rust_js::link_name = "Bun.hash")]
pub fn hash(data: impl IntoHashData, seed: Option<&dyn IntoSeed>) -> u64 {
    unreachable!()
}

/// What [`hash`] hashes, `string | ArrayBufferView | ArrayBuffer |
/// SharedArrayBuffer`: each as it is (ADR 0229).
#[diagnostic::on_unimplemented(message = "`{Self}` is not a text nor bytes")]
#[cfg_attr(
    rust_js,
    rust_js::types = "string | ArrayBufferView | ArrayBuffer | SharedArrayBuffer"
)]
pub trait IntoHashData: sealed::Sealed {}
impl IntoHashData for &str {}
impl IntoHashData for &String {}
impl IntoHashData for &ArrayBufferView {}
impl IntoHashData for &Uint8Array {}
impl IntoHashData for &DataView {}
impl IntoHashData for &ArrayBuffer {}
impl IntoHashData for &SharedArrayBuffer {}

/// What [`hash`]'s seed is, `number | bigint`; an optional one, `Option<&dyn
/// IntoSeed>`, takes `Some(&value)` (ADR 0443).
#[diagnostic::on_unimplemented(message = "`{Self}` is not a number nor a bigint")]
#[cfg_attr(rust_js, rust_js::types = "number | bigint")]
pub trait IntoSeed: sealed::Sealed {}
impl IntoSeed for f64 {}
impl IntoSeed for u64 {}

/// bun-types' `Bun.DirectUnderlyingSource`: what a direct stream reads
/// from, `new ReadableStream({type: "direct", pull, cancel})`. Its `pull`
/// is given the stream's [`ReadableStreamDirectController`], which writes
/// to the stream's destination without a queue; each function is lent, as
/// webapi's `UnderlyingByteSource`'s are.
#[derive(Clone, Copy)]
pub struct DirectUnderlyingSource<'a> {
    /// `"direct"`.
    pub r#type: &'a str,
    /// Called for the stream's data: once by a destination that takes the
    /// whole body, again for each later read by a reader.
    pub pull: &'a dyn Fn(&'static ReadableStreamDirectController),
    /// Called with the reason the stream's consumer cancelled it.
    pub cancel: Option<&'a dyn Fn(Option<&'static Unknown>)>,
}

unsafe impl js::Defined for DirectUnderlyingSource<'_> {}

/// Bun's `ReadableStream` constructor takes a direct source too, bun-types'
/// overload of `new ReadableStream(underlyingSource, strategy)`.
impl webapi::IntoUnderlyingSource for DirectUnderlyingSource<'_> {}

/// bun-types' `ReadableStreamDirectController`: what a direct stream's
/// `pull` writes to, straight to the stream's destination.
#[cfg_attr(rust_js, rust_js::types = "ReadableStreamDirectController")]
pub struct ReadableStreamDirectController(PhantomData<JsObject>);

unsafe impl js::Defined for ReadableStreamDirectController {}

/// `a === b`: the same object.
impl PartialEq for ReadableStreamDirectController {
    #[cfg_attr(rust_js, rust_js::link_name = "===")]
    fn eq(&self, other: &Self) -> bool {
        core::ptr::eq(self, other)
    }
}

impl Eq for ReadableStreamDirectController {}

impl ReadableStreamDirectController {
    /// `close(error?)`: the stream finished, its buffered bytes flushed, as
    /// [`end`](Self::end) does; given an error, the stream fails instead,
    /// its buffered bytes dropped and its consumer rejected with `error`.
    #[cfg_attr(rust_js, rust_js::link_name = "close")]
    pub fn close(&self, error: Option<&JsError>) {
        unreachable!()
    }

    /// `write(data)`: `data` written to the destination, text as UTF-8.
    /// The bytes written, `0` once the destination has gone away, or, while
    /// its buffer is full, a promise of them once it drains; the chunk was
    /// taken either way.
    #[cfg_attr(rust_js, rust_js::link_name = "write")]
    pub fn write(&self, data: impl IntoDirectChunk) -> Written {
        unreachable!()
    }

    /// `end()`: the stream finished, its buffered bytes flushed.
    #[cfg_attr(rust_js, rust_js::link_name = "end")]
    pub fn end(&self) -> Written {
        unreachable!()
    }

    /// `flush(wait?)`: the bytes buffered here written to the destination;
    /// given `true`, a promise that waits until the destination has drained
    /// its own buffer too.
    #[cfg_attr(rust_js, rust_js::link_name = "flush")]
    pub fn flush(&self, wait: Option<bool>) -> Written {
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "start")]
    pub fn start(&self) {
        unreachable!()
    }
}

/// What [`ReadableStreamDirectController::write`] writes, `Bun.BufferSource
/// | ArrayBuffer | string`: each as it is (ADR 0229).
#[diagnostic::on_unimplemented(message = "`{Self}` is not a text nor bytes")]
#[cfg_attr(rust_js, rust_js::types = "Bun.BufferSource | ArrayBuffer | string")]
pub trait IntoDirectChunk: sealed::Sealed {}
impl IntoDirectChunk for &str {}
impl IntoDirectChunk for &String {}
impl IntoDirectChunk for &ArrayBufferView {}
impl IntoDirectChunk for &Uint8Array {}
impl IntoDirectChunk for &DataView {}
impl IntoDirectChunk for &ArrayBuffer {}
impl IntoDirectChunk for &SharedArrayBuffer {}

/// What a direct controller's write gives, `number | Promise<number>`.
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum Written {
    /// The bytes written.
    Bytes(f64),
    /// The destination's buffer is full: the bytes written, once it drains.
    Pending(Promise<f64>),
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for &str {}
    impl Sealed for &String {}
    impl Sealed for &js::ArrayBufferView {}
    impl Sealed for &js::Uint8Array {}
    impl Sealed for &js::DataView {}
    impl Sealed for &js::ArrayBuffer {}
    impl Sealed for &js::SharedArrayBuffer {}
    impl Sealed for f64 {}
    impl Sealed for u64 {}
}
