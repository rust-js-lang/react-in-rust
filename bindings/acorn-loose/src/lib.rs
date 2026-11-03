//! Bindings of [acorn-loose](https://github.com/acornjs/acorn/tree/master/acorn-loose)
//! 8.3, acorn's error-tolerant JavaScript parser: [`parse`], which makes a
//! [`Program`] of any text, its nodes ESTree's, as acorn makes them.
//!
//! acorn-loose ships no types: these are acorn 8's `Options` and ESTree's
//! nodes, as `@types/estree` declares them, of the kinds a program reads:
//! React's Flight Node loaders and registers, and its webpack plugin.
//! A node of any other kind is [`Node::Other`], so a `match` of a node is a
//! `switch` of its `type` with a `default`.
//!
//! ```rust,ignore
//! use acorn_loose::{EcmaVersion, Node, Options, SourceType};
//!
//! let program = acorn_loose::parse(source, &Options {
//!     ecma_version: EcmaVersion::Version(2024),
//!     source_type: Some(SourceType::Module),
//!     locations: None,
//!     on_comment: None,
//!     preserve_parens: None,
//! })?;
//! for node in program.body {
//!     if let Node::ExpressionStatement { directive: Some(directive) } = node { .. }
//! }
//! ```

// A binding's body never runs, so rustc's crate sees its parameters unread
// (rd330, the user's one crate-level allow for a bindings crate).
#![cfg_attr(not(rust_js), allow(unused_variables))]

use js::{JsError, JsObject, Unknown};

/// `acorn.parse(input, options)`, acorn-loose's, read of its namespace as
/// upstream imports it, `import * as acorn from "acorn-loose"`: the program
/// `input` is, as near as it can make one of text that isn't JavaScript;
/// what it threw, of an option it can't take.
#[cfg_attr(rust_js, rust_js::link_name = "acorn-loose#* as acorn.parse")]
pub fn parse(input: &str, options: &Options<'_>) -> Result<&'static Program, &'static JsError> {
    unreachable!()
}

/// acorn's `Options`: how [`parse`] reads its input, each `None` left out.
pub struct Options<'a> {
    /// The version of JavaScript to read, required.
    pub ecma_version: EcmaVersion,
    /// A script's or a module's: whether `import` and `export` may appear.
    pub source_type: Option<SourceType>,
    /// Whether each node has its `loc`, its lines and columns.
    pub locations: Option<bool>,
    /// What each comment is given to, as it's read: whether it's a block
    /// comment, its text, where it starts and ends, and, with `locations`,
    /// its first and last line and column.
    pub on_comment:
        Option<&'a mut dyn FnMut(bool, &str, f64, f64, Option<&Position>, Option<&Position>)>,
    /// Whether a parenthesized expression is a `ParenthesizedExpression`.
    pub preserve_parens: Option<bool>,
}

/// acorn's `ecmaVersion`: a year or an edition, `2024`, or `"latest"`.
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum EcmaVersion {
    Version(u32),
    #[cfg_attr(rust_js, rust_js::name = "latest")]
    Latest,
}

/// acorn's `sourceType`.
pub enum SourceType {
    #[cfg_attr(rust_js, rust_js::name = "script")]
    Script,
    #[cfg_attr(rust_js, rust_js::name = "module")]
    Module,
}

/// acorn's `Position`: a line, from 1, and a column, from 0.
pub struct Position {
    pub line: f64,
    pub column: f64,
}

/// acorn's `SourceLocation`: where a node starts and ends.
pub struct SourceLocation {
    pub start: Position,
    pub end: Position,
}

/// ESTree's `Program`: what [`parse`] makes.
pub struct Program {
    /// Its statements, directives and module declarations, in order.
    pub body: &'static [Node],
    /// Where it is, with `locations`.
    pub loc: Option<&'static SourceLocation>,
}

/// ESTree's `Node`, as acorn-loose makes it: one of the kinds below, or
/// [`Node::Other`]. Each has its `loc`, with `locations`, and fields of
/// the nodes it holds.
#[cfg_attr(rust_js, rust_js::tag = "type")]
pub enum Node {
    Identifier {
        name: String,
        loc: Option<&'static SourceLocation>,
    },
    /// A string, a number, a boolean, `null`, a regular expression or a
    /// bigint, its `value`.
    Literal {
        value: Option<&'static Unknown>,
        loc: Option<&'static SourceLocation>,
    },
    ObjectPattern {
        properties: &'static [Node],
    },
    /// Its elements, `None` of a hole, `[, b]`.
    ArrayPattern {
        elements: &'static [Option<Node>],
    },
    Property {
        value: &'static Node,
    },
    AssignmentPattern {
        left: &'static Node,
    },
    RestElement {
        argument: &'static Node,
    },
    /// With `preserve_parens`.
    ParenthesizedExpression {
        expression: &'static Node,
    },
    /// An expression statement, its `directive` that of a directive
    /// prologue's, `"use strict"`'s `use strict`.
    ExpressionStatement {
        directive: Option<String>,
    },
    /// `export * from ".."`, `export * as name from ".."`.
    ExportAllDeclaration {
        exported: Option<&'static Node>,
        source: &'static SourceLiteral,
    },
    /// `export default ..`: a declaration or an expression.
    ExportDefaultDeclaration {
        declaration: &'static Node,
    },
    /// `export const a = ..`, `export function f() {}`, `export {a, b as
    /// c}`: its declaration, any statement of text that isn't
    /// JavaScript, `export if (a) {}`'s, or its specifiers.
    ExportNamedDeclaration {
        declaration: Option<&'static Node>,
        specifiers: &'static [ExportSpecifier],
    },
    VariableDeclaration {
        declarations: &'static [VariableDeclarator],
    },
    /// Its `id`, `None` of `export default function () {}`'s.
    FunctionDeclaration {
        id: Option<&'static Node>,
    },
    /// Its `id`, `None` of `export default class {}`'s.
    ClassDeclaration {
        id: Option<&'static Node>,
    },
    /// A node of any other kind.
    #[cfg_attr(rust_js, rust_js::otherwise)]
    Other(&'static JsObject),
}

/// ESTree's `VariableDeclarator`, of a `VariableDeclaration`: its `id`, an
/// `Identifier` or a pattern.
pub struct VariableDeclarator {
    pub id: &'static Node,
}

/// ESTree's `ExportSpecifier`, of an `export {..}`: its `local` and
/// `exported`, each an `Identifier` or, `export {"a b" as c}`, a string
/// `Literal`.
pub struct ExportSpecifier {
    pub local: &'static Node,
    pub exported: &'static Node,
}

/// ESTree's `Literal` that is a module's source, `export * from "./a.js"`'s:
/// its `value`, the module's specifier.
pub struct SourceLiteral {
    pub value: String,
}

impl Node {
    /// `node.name`: an `Identifier`'s name; `None` of a node of another
    /// kind, a string `Literal`'s, which has none.
    #[cfg_attr(rust_js, rust_js::link_name = "get name")]
    pub fn name(&self) -> Option<String> {
        unreachable!()
    }

    /// `node.loc`: where it is, with `locations`.
    #[cfg_attr(rust_js, rust_js::link_name = "get loc")]
    pub fn loc(&self) -> Option<&'static SourceLocation> {
        unreachable!()
    }

    /// `node.id`: a declaration's name, an `Identifier`; `None` of one
    /// without, `export default function () {}`'s, and of a node of a kind
    /// that has none.
    #[cfg_attr(rust_js, rust_js::link_name = "get id")]
    pub fn id(&self) -> Option<&'static Node> {
        unreachable!()
    }
}
