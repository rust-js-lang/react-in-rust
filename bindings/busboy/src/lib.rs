//! Bindings of [busboy](https://github.com/mscdex/busboy) 1.6, a streaming
//! parser of HTML form data, as @types/busboy types it: [`busboy`], what
//! makes a parser of a request's headers, a [`Busboy`], the `Writable` the
//! request's body is piped to, which emits each field and file it parses.
//!
//! ```rust,ignore
//! use busboy::{Busboy, FieldInfo, event};
//! use node::events::EventEmitterExt;
//!
//! fn fields(parser: &Busboy, log: &'static dyn Fn(&str, &str)) {
//!     parser.on(event::Field, Box::new(move |name: &str, value: &str, _: &FieldInfo| log(name, value)));
//! }
//! ```

// A binding's body never runs, so rustc's crate sees its parameters unread
// (rd330, the user's one crate-level allow for a bindings crate).
#![cfg_attr(not(rust_js), allow(unused_variables))]

use std::marker::PhantomData;
use std::ops::Deref;

use js::{Dict, JsObject, Unknown};
use node::events::{Emits, EventEmitterExt};
use node::http::IncomingHttpHeader;
use node::stream::{Readable, Writable};

/// @types/busboy's `Busboy`: the parser, a `Writable` the form's body is
/// written to, which emits what it parses.
#[cfg_attr(rust_js, rust_js::types = "busboy#Busboy")]
pub struct Busboy(PhantomData<JsObject>);

impl Deref for Busboy {
    type Target = Writable;

    fn deref(&self) -> &Writable {
        // Never runs: rust-js compiles this `Deref` to the object itself.
        unsafe { &*(self as *const Self as *const Writable) }
    }
}

/// `busboy(config)`, busboy's export: a parser of the form whose request's
/// `headers` the config gives, its `content-type` the form's boundary.
#[cfg_attr(rust_js, rust_js::link_name = "busboy#default")]
pub fn busboy(config: &BusboyConfig<'_>) -> &'static Busboy {
    unreachable!()
}

/// @types/busboy's `BusboyConfig`: what [`busboy`] takes, each `None` left out.
#[derive(Clone, Copy, Default)]
pub struct BusboyConfig<'a> {
    /// The request's headers, `request.headers`.
    pub headers: Option<&'a Dict<IncomingHttpHeader>>,
    pub high_water_mark: Option<f64>,
    pub file_hwm: Option<f64>,
    pub def_charset: Option<&'a str>,
    pub def_param_charset: Option<&'a str>,
    pub preserve_path: Option<bool>,
    pub limits: Option<&'a Limits>,
}

/// @types/busboy's `Limits`: the most of each a form may have, each `None`
/// none.
#[derive(Clone, Copy, Default)]
pub struct Limits {
    pub field_name_size: Option<f64>,
    pub field_size: Option<f64>,
    pub fields: Option<f64>,
    pub file_size: Option<f64>,
    pub files: Option<f64>,
    pub parts: Option<f64>,
    pub header_pairs: Option<f64>,
}

/// @types/busboy's `FileInfo`: what a file's part says of it.
pub struct FileInfo {
    /// Its name, where its part gives one.
    pub filename: Option<String>,
    pub encoding: String,
    pub mime_type: String,
}

/// @types/busboy's `FieldInfo`: what a field's part says of it.
pub struct FieldInfo {
    pub name_truncated: bool,
    pub value_truncated: bool,
    pub encoding: String,
    pub mime_type: String,
}

/// What a file's part is read from, @types/busboy's `Readable & {
/// truncated?: boolean }`: a [`Readable`] of its bytes.
pub struct FileStream(PhantomData<JsObject>);

impl Deref for FileStream {
    type Target = Readable;

    fn deref(&self) -> &Readable {
        // Never runs: rust-js compiles this `Deref` to the object itself.
        unsafe { &*(self as *const Self as *const Readable) }
    }
}

impl FileStream {
    /// Whether the file was cut short at `limits.file_size`, once it's
    /// read; `None` before.
    #[cfg_attr(rust_js, rust_js::link_name = "get truncated")]
    pub fn truncated(&self) -> Option<bool> {
        unreachable!()
    }
}

/// busboy's events, @types/busboy's `BusboyEvents`, each a name of its own
/// (ADR 0361).
pub mod event {
    /// `"file"`: a file's part, its name, what it's read from, and its info.
    #[cfg_attr(rust_js, rust_js::name = "file")]
    pub struct File;

    /// `"field"`: a field's part, its name, its value and its info.
    #[cfg_attr(rust_js, rust_js::name = "field")]
    pub struct Field;

    /// `"partsLimit"`: `limits.parts` reached.
    #[cfg_attr(rust_js, rust_js::name = "partsLimit")]
    pub struct PartsLimit;

    /// `"filesLimit"`: `limits.files` reached.
    #[cfg_attr(rust_js, rust_js::name = "filesLimit")]
    pub struct FilesLimit;

    /// `"fieldsLimit"`: `limits.fields` reached.
    #[cfg_attr(rust_js, rust_js::name = "fieldsLimit")]
    pub struct FieldsLimit;

    pub use node::event::{Close, Error, Finish};
}

impl EventEmitterExt for Busboy {}

/// `"file"`: `(name, stream, info)`.
impl Emits<event::File> for Busboy {
    type Listener = dyn Fn(&str, &FileStream, &FileInfo);
    type Args = (&'static str, &'static FileStream, &'static FileInfo);
}

/// `"field"`: `(name, value, info)`.
impl Emits<event::Field> for Busboy {
    type Listener = dyn Fn(&str, &str, &FieldInfo);
    type Args = (&'static str, &'static str, &'static FieldInfo);
}

/// `"partsLimit"`.
impl Emits<event::PartsLimit> for Busboy {
    type Listener = dyn Fn();
    type Args = ();
}

/// `"filesLimit"`.
impl Emits<event::FilesLimit> for Busboy {
    type Listener = dyn Fn();
    type Args = ();
}

/// `"fieldsLimit"`.
impl Emits<event::FieldsLimit> for Busboy {
    type Listener = dyn Fn();
    type Args = ();
}

/// `"error"`: what failed parsing, any value, as @types/busboy's `unknown`.
impl Emits<event::Error> for Busboy {
    type Listener = dyn Fn(Option<&Unknown>);
    type Args = (Option<&'static Unknown>,);
}

/// `"finish"`: all of the form parsed, each file's part read.
impl Emits<event::Finish> for Busboy {
    type Listener = dyn Fn();
    type Args = ();
}

/// `"close"`: the parser closed.
impl Emits<event::Close> for Busboy {
    type Listener = dyn Fn();
    type Args = ();
}
