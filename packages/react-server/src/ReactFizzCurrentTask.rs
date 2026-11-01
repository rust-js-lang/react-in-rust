// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;
use std::marker::PhantomData;

use js::{JsObject, Unknown};
use shared::environment::__DEV__;
use webapi::ConsoleTask;

use crate::react_fizz_component_stack::ComponentStackNode;
use crate::react_fizz_tree_context::TreeContext;

/// A task of ReactFizzServer's, its JS's until it's ported: what's read of
/// it here.
pub struct Task(PhantomData<JsObject>);

unsafe impl js::Defined for Task {}

/// ReactFizzServer's request, its JS's until it's ported.
pub struct Request(PhantomData<JsObject>);

unsafe impl js::Defined for Request {}

/// What ReactFizzServer tells a request's `onError` of an error, its JS's
/// until it's ported: `{ componentStack }`.
pub struct ErrorInfo(PhantomData<JsObject>);

unsafe impl js::Defined for ErrorInfo {}

/// A request's `onError`: of what was thrown and where, the digest it's
/// sent to the client as, if any.
pub type OnError = dyn Fn(Option<&'static Unknown>, &ErrorInfo) -> Option<String>;

/// A request's `onBrowserBailout`, of what made a boundary render on the
/// client instead, and where.
pub type OnBrowserBailout = dyn Fn(Option<&'static Unknown>, &ErrorInfo);

/// ReactFizzServer's key path, its JS's until it's ported: `[parent, name,
/// key]`, of strings, numbers and `null`, nested.
pub struct KeyNode(PhantomData<JsObject>);

unsafe impl js::Defined for KeyNode {}

// Safety: an array of strings, numbers, `null` and such arrays.
unsafe impl js::JsonText for KeyNode {}

impl Task {
    // the current tree context that this task is executing in
    #[cfg_attr(rust_js, rust_js::link_name = "get treeContext")]
    pub fn tree_context(&self) -> &'static TreeContext {
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "get componentStack")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub fn component_stack(&self) -> Option<&'static ComponentStackNode> {
        unreachable!()
    }

    // DEV only
    #[cfg_attr(rust_js, rust_js::link_name = "get debugTask")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub fn debug_task(&self) -> Option<&'static ConsoleTask> {
        unreachable!()
    }
}

thread_local! {
    // DEV-only global reference to the currently executing task
    #[cfg_attr(rust_js, rust_js::name = "currentTaskInDEV", rust_js::nullable)]
    pub static CURRENT_TASK_IN_DEV: Cell<Option<&'static Task>> = const { Cell::new(None) };
}

#[cfg_attr(rust_js, rust_js::name = "setCurrentTaskInDEV")]
pub fn set_current_task_in_dev(task: Option<&'static Task>) {
    if __DEV__ {
        CURRENT_TASK_IN_DEV.set(task);
    }
}
