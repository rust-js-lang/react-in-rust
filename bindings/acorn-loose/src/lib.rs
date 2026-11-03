//! Bindings of [acorn-loose](https://github.com/acornjs/acorn/tree/master/acorn-loose)
//! 8.3, acorn's error-tolerant parser: [`parse`] gives a program of any
//! text, as acorn types it, where acorn throws.
//!
//! ```rust,ignore
//! let program = acorn_loose::parse(source, &Options { ecma_version: 2024, source_type: Some("module") });
//! ```

// A binding's body never runs, so rustc's crate sees its parameters unread
// (rd330, the user's one crate-level allow for a bindings crate).
#![cfg_attr(not(rust_js), allow(unused_variables))]

use std::marker::PhantomData;

use js::{JsError, JsObject};

/// `parse(input, options)`: the program `input` is, as acorn's `Program`,
/// parts it can't read filled in; what it throws of `options` it can't
/// read, the `Err`.
#[cfg_attr(rust_js, rust_js::link_name = "acorn-loose#* as acorn.parse")]
pub fn parse(input: &str, options: &Options<'_>) -> Result<&'static Program, &'static JsError> {
    unreachable!()
}

/// acorn's `Options`, of what [`parse`] is given, each `None` left out.
#[derive(Clone, Copy)]
pub struct Options<'a> {
    /// The ECMAScript version to parse, its year, `2024`.
    pub ecma_version: u32,
    /// `"script"` or `"module"`.
    pub source_type: Option<&'a str>,
}

/// ESTree's `Program`, as acorn gives it.
pub struct Program(PhantomData<JsObject>);

impl Program {
    /// `program.body`: its statements, in order.
    #[cfg_attr(rust_js, rust_js::link_name = "get body")]
    pub fn body(&self) -> Vec<&'static Statement> {
        unreachable!()
    }
}

/// ESTree's `Statement | ModuleDeclaration`: a statement of a program.
pub struct Statement(PhantomData<JsObject>);

impl Statement {
    /// `node.type`: its kind, `"ExpressionStatement"`.
    #[cfg_attr(rust_js, rust_js::link_name = "get type")]
    pub fn r#type(&self) -> String {
        unreachable!()
    }

    /// `node.directive`: of a directive, an `ExpressionStatement` of a
    /// prologue's string, its text, `"use client"`; `None` of any other.
    #[cfg_attr(rust_js, rust_js::link_name = "get directive")]
    pub fn directive(&self) -> Option<String> {
        unreachable!()
    }
}
