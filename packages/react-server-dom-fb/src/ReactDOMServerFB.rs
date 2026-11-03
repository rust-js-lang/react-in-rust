// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js::Unknown;
use react_dom_bindings::react_fizz_config_dom::{
    BootstrapScript, FormatContext, Nonce, OnHeaders, RenderState, ResumableState,
};
use react_server::react_fizz_server::{OnBrowserBailout, OnError, Request};
use react_server::react_server_stream_config_fb::Destination;
use shared::react_types::ReactNodeList;

use react_server::react_fizz_server::{
    abort, create_request, perform_work, start_flowing, start_work,
};

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-server/src/ReactFizzConfig#createResumableState"
)]
fn create_resumable_state(
    identifier_prefix: Option<&str>,
    external_runtime_config: Option<BootstrapScript<'static>>,
    bootstrap_script_content: Option<&'static str>,
    bootstrap_scripts: Option<&'static [BootstrapScript<'static>]>,
    bootstrap_modules: Option<&'static [BootstrapScript<'static>]>,
) -> ResumableState {
    let _ = (
        identifier_prefix,
        external_runtime_config,
        bootstrap_script_content,
        bootstrap_scripts,
        bootstrap_modules,
    );
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-server/src/ReactFizzConfig#createRenderState"
)]
fn create_render_state(
    resumable_state: &ResumableState,
    nonce: Option<Nonce<'static>>,
    external_runtime_config: Option<BootstrapScript<'static>>,
    import_map: Option<&'static Unknown>,
    on_headers: Option<Rc<OnHeaders>>,
    max_headers_length: Option<&'static Unknown>,
) -> RenderState {
    let _ = (
        resumable_state,
        nonce,
        external_runtime_config,
        import_map,
        on_headers,
        max_headers_length,
    );
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-server/src/ReactFizzConfig#createRootFormatContext"
)]
fn create_root_format_context(
    #[cfg_attr(rust_js, rust_js::name = "namespaceURI")] namespace_uri: Option<&str>,
) -> Rc<FormatContext> {
    let _ = namespace_uri;
    unreachable!()
}

pub struct Options {
    pub identifier_prefix: Option<&'static str>,
    pub bootstrap_script_content: Option<&'static str>,
    pub bootstrap_scripts: Option<&'static [BootstrapScript<'static>]>,
    pub bootstrap_modules: Option<&'static [BootstrapScript<'static>]>,
    pub progressive_chunk_size: Option<f64>,
    pub on_error: Rc<OnError>,
    pub on_browser_bailout: Option<Rc<OnBrowserBailout>>,
    #[cfg_attr(rust_js, rust_js::name = "unstable_externalRuntimeSrc")]
    pub unstable_external_runtime_src: Option<BootstrapScript<'static>>,
}

pub struct Stream {
    destination: Rc<Destination>,
    request: Rc<Request>,
}

pub fn render_to_stream(children: Option<&'static ReactNodeList>, options: &Options) -> Stream {
    let destination = Rc::new(Destination {
        buffer: RefCell::new(String::new()),
        done: Cell::new(false),
        fatal: Cell::new(false),
        error: Cell::new(js::null()),
    });
    let resumable_state = Rc::new(create_resumable_state(
        options.identifier_prefix,
        options.unstable_external_runtime_src,
        options.bootstrap_script_content,
        options.bootstrap_scripts,
        options.bootstrap_modules,
    ));
    let request = create_request(
        children,
        js::unknown_of(Rc::clone(&resumable_state)),
        js::unknown_of(create_render_state(
            &resumable_state,
            None,
            options.unstable_external_runtime_src,
            None,
            None,
            None,
        )),
        js::unknown_of(create_root_format_context(None)),
        options.progressive_chunk_size,
        Some(Rc::clone(&options.on_error)),
        options.on_browser_bailout.clone(),
        None,
        None,
        None,
        None,
        None,
    );
    start_work(&request);
    if destination.fatal.get() {
        js::throw(destination.error.get());
    }
    Stream {
        destination,
        request,
    }
}

pub fn abort_stream(stream: &Stream, reason: Option<&'static Unknown>) {
    abort(&stream.request, reason);
}

pub fn render_next_chunk(stream: &Stream) -> String {
    let Stream {
        request,
        destination,
    } = stream;
    perform_work(request);
    start_flowing(request, js::unknown_of(Rc::clone(destination)));
    if destination.fatal.get() {
        js::throw(destination.error.get());
    }
    // GAP rd669: the buffer taken is named `old`, where upstream names it
    // `chunk`.
    destination.buffer.take()
}

pub fn has_finished(stream: &Stream) -> bool {
    stream.destination.done.get()
}

/// What a stream's request has left to do, as upstream's `any` reads it.
struct DebugInfo {
    pending_root_tasks: Option<&'static Unknown>,
    client_rendered_boundaries: Option<&'static Unknown>,
    completed_boundaries: Option<&'static Unknown>,
    partial_boundaries: Option<&'static Unknown>,
    all_pending_tasks: Option<&'static Unknown>,
    pinged_tasks: Option<&'static Unknown>,
}

// Safety: a JS object of its fields.
unsafe impl js::Defined for DebugInfo {}

pub fn debug(stream: &Stream) -> &'static Unknown {
    // convert to any to silence flow errors from opaque type
    let request = js::unknown(&*stream.request);
    js::unknown_of(DebugInfo {
        pending_root_tasks: js::get(request, "pendingRootTasks"),
        client_rendered_boundaries: js::get(js::get(request, "clientRenderedBoundaries"), "length"),
        completed_boundaries: js::get(js::get(request, "completedBoundaries"), "length"),
        partial_boundaries: js::get(js::get(request, "partialBoundaries"), "length"),
        all_pending_tasks: js::get(request, "allPendingTasks"),
        pinged_tasks: js::get(js::get(request, "pingedTasks"), "length"),
    })
}
