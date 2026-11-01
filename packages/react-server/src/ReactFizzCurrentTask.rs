// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

use js::{JsObject, Unknown};
use shared::environment::__DEV__;
use webapi::AbortSignal;

use crate::react_fizz_server::Task;

/// ReactFizzServer's request, its JS's until it's ported.
pub struct Request(PhantomData<JsObject>);

unsafe impl js::Defined for Request {}

/// What a prerender left to resume, ReactFizzServer's, its JS's until
/// it's ported: of a renderer's resumable state, `S`.
pub struct PostponedState<S>(PhantomData<(JsObject, S)>);

unsafe impl<S> js::Defined for PostponedState<S> {}

impl<S: 'static> PostponedState<S> {
    #[cfg_attr(rust_js, rust_js::link_name = "get resumableState")]
    pub fn resumable_state(&self) -> &'static S {
        unreachable!()
    }
}

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

thread_local! {
    // DEV-only global reference to the currently executing task
    #[cfg_attr(rust_js, rust_js::name = "currentTaskInDEV", rust_js::nullable)]
    pub static CURRENT_TASK_IN_DEV: RefCell<Option<Rc<Task>>> = const { RefCell::new(None) };
}

#[cfg_attr(rust_js, rust_js::name = "setCurrentTaskInDEV")]
pub fn set_current_task_in_dev(task: Option<Rc<Task>>) {
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
    rust_js::link_name = "react-server/src/ReactFizzServer#resumeRequest"
)]
pub fn resume_request<S, R: ?Sized>(
    children: Option<&'static Unknown>,
    postponed_state: &PostponedState<S>,
    render_state: &R,
    on_error: Option<&OnError>,
    on_browser_bailout: Option<&OnBrowserBailout>,
    on_all_ready: Option<&dyn Fn()>,
    on_shell_ready: Option<&dyn Fn()>,
    on_shell_error: Option<&dyn Fn(Option<&'static Unknown>)>,
    on_fatal_error: Option<&dyn Fn(Option<&'static Unknown>)>,
) -> &'static Request {
    let _ = (
        children,
        postponed_state,
        render_state,
        on_error,
        on_browser_bailout,
        on_all_ready,
        on_shell_ready,
        on_shell_error,
        on_fatal_error,
    );
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-server/src/ReactFizzServer#createPrerenderRequest"
)]
pub fn create_prerender_request<S: ?Sized, R: ?Sized, F: ?Sized>(
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
    );
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-server/src/ReactFizzServer#resumeAndPrerenderRequest"
)]
pub fn resume_and_prerender_request<S, R: ?Sized>(
    children: Option<&'static Unknown>,
    postponed_state: &PostponedState<S>,
    render_state: &R,
    on_error: Option<&OnError>,
    on_browser_bailout: Option<&OnBrowserBailout>,
    on_all_ready: Option<&dyn Fn()>,
    on_shell_ready: Option<&dyn Fn()>,
    on_shell_error: Option<&dyn Fn(Option<&'static Unknown>)>,
    on_fatal_error: Option<&dyn Fn(Option<&'static Unknown>)>,
) -> &'static Request {
    let _ = (
        children,
        postponed_state,
        render_state,
        on_error,
        on_browser_bailout,
        on_all_ready,
        on_shell_ready,
        on_shell_error,
        on_fatal_error,
    );
    unreachable!()
}

/// What a prerender left to resume, or `null` where it finished.
#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-server/src/ReactFizzServer#getPostponedState"
)]
#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_postponed_state<S: 'static>(request: &Request) -> Option<&'static PostponedState<S>> {
    let _ = request;
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-server/src/ReactFizzServer#prepareForStartFlowingIfBeforeAllReady"
)]
pub fn prepare_for_start_flowing_if_before_all_ready(request: &Request) {
    let _ = request;
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
