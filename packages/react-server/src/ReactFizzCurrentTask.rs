// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;
use std::marker::PhantomData;

use js::{JsObject, Unknown};
use shared::environment::__DEV__;
use webapi::{AbortSignal, ConsoleTask};

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

// ReactFizzServer's functions an entry calls, its JS's until it's ported:
// each of a renderer's own states, which its Fizz config makes.

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-server/src/ReactFizzServer#createRequest"
)]
pub fn create_request<S: ?Sized, R: ?Sized, F: ?Sized>(
    children: Option<&'static Unknown>,
    resumable_state: &S,
    render_state: &R,
    root_format_context: &F,
    progressive_chunk_size: Option<f64>,
    on_error: Option<&OnError>,
    on_browser_bailout: Option<&OnBrowserBailout>,
    on_all_ready: Option<&dyn Fn()>,
    on_shell_ready: Option<&dyn Fn()>,
    on_shell_error: Option<&dyn Fn(Option<&'static Unknown>)>,
    on_fatal_error: Option<&dyn Fn(Option<&'static Unknown>)>,
    form_state: Option<&'static Unknown>,
) -> &'static Request {
    let _ = (
        children,
        resumable_state,
        render_state,
        root_format_context,
        progressive_chunk_size,
        on_error,
        on_browser_bailout,
        on_all_ready,
        on_shell_ready,
        on_shell_error,
        on_fatal_error,
        form_state,
    );
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-server/src/ReactFizzServer#startWork"
)]
pub fn start_work(request: &Request) {
    let _ = request;
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-server/src/ReactFizzServer#startFlowing"
)]
pub fn start_flowing<D: ?Sized>(request: &Request, destination: &D) {
    let _ = (request, destination);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-server/src/ReactFizzServer#stopFlowing"
)]
pub fn stop_flowing(request: &Request) {
    let _ = request;
    unreachable!()
}

#[cfg_attr(rust_js, rust_js::link_name = "react-server/src/ReactFizzServer#abort")]
pub fn abort(request: &Request, reason: Option<&Unknown>) {
    let _ = (request, reason);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-server/src/ReactFizzServer#attachAbortSignal"
)]
pub fn attach_abort_signal(request: &Request, signal: &AbortSignal) {
    let _ = (request, signal);
    unreachable!()
}
