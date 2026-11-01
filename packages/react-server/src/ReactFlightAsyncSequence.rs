// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::rc::Rc;

use js::{JsObject, WeakKey, WeakRef};
use shared::react_types::{ReactComponentInfo, ReactStackTrace};

pub const IO_NODE: u8 = 0;
pub const PROMISE_NODE: u8 = 1;
pub const AWAIT_NODE: u8 = 2;
pub const UNRESOLVED_PROMISE_NODE: u8 = 3;
pub const UNRESOLVED_AWAIT_NODE: u8 = 4;

/// A promise, with what debug info Flight put on it, `_debugInfo`.
pub struct PromiseWithDebugInfo(PhantomData<JsObject>);

unsafe impl js::Defined for PromiseWithDebugInfo {}

// Safety: a promise, an object.
unsafe impl WeakKey for PromiseWithDebugInfo {}

/// A node of the async work Flight follows, its `tag` one of the five:
/// an I/O node, a promise's, an await's, and an unresolved promise's or
/// await's, each the fields of all of them.
pub struct AsyncSequence {
    pub tag: Cell<u8>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub owner: Option<&'static ReactComponentInfo>,
    // callsite that spawned the I/O, created the Promise, or awaited
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub stack: Option<Rc<ReactStackTrace>>,
    // start time when the first part of the I/O sequence started, or when we started blocking
    pub start: Cell<f64>,
    // end time when the Promise was resolved, or when we unblocked; -1.1 until it's resolved
    pub end: Cell<f64>,
    // a reference to this Promise if still referenced; not used on I/O
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub promise: Option<&'static WeakRef<PromiseWithDebugInfo>>,
    // the thing that ended up resolving this promise, or the promise we were waiting on
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub awaited: RefCell<Option<Rc<AsyncSequence>>>,
    // the sequence that was blocking us, or the preceeding await that spawned this new work
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub previous: RefCell<Option<Rc<AsyncSequence>>>,
}
