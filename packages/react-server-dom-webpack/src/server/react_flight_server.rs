//! ReactFlightServer's functions the server entries call, its JS's until
//! it's ported. Bindings alone, so no JS of its own.

use js::Unknown;
use react_server::react_flight_hooks::Request;
use react_server::react_flight_server_temporary_references::TemporaryReferenceSet;
use webapi::AbortSignal;

use super::react_flight_server_config_webpack_bundler::ClientManifest;

/// Any value a server component may render or pass to the client.
pub type ReactClientValue = Unknown;

/// A request's `onError`, told of each error.
pub type OnError = dyn Fn(Option<&'static Unknown>);

/// A request's `environmentName`: the name itself, or what gives it.
#[cfg_attr(rust_js, rust_js::untagged)]
#[derive(Clone, Copy)]
pub enum EnvironmentName {
    Str(&'static str),
    Fn(&'static dyn Fn() -> String),
}

/// A request's `filterStackFrame`: whether a frame is shown, of its URL and
/// its function's name.
pub type FilterStackFrame = dyn Fn(&str, &str) -> bool;

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-server/src/ReactFlightServer#createRequest"
)]
pub fn create_request(
    model: Option<&'static ReactClientValue>,
    bundler_config: &ClientManifest,
    on_error: Option<&OnError>,
    identifier_prefix: Option<&str>,
    temporary_references: Option<&TemporaryReferenceSet>,
    debug_start_time: Option<f64>,                 // Profiling-only
    environment_name: Option<EnvironmentName>,     // DEV-only
    filter_stack_frame: Option<&FilterStackFrame>, // DEV-only
    keep_debug_alive: bool,                        // DEV-only
) -> &'static Request {
    let _ = (
        model,
        bundler_config,
        on_error,
        identifier_prefix,
        temporary_references,
        debug_start_time,
        environment_name,
        filter_stack_frame,
        keep_debug_alive,
    );
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-server/src/ReactFlightServer#createPrerenderRequest"
)]
pub fn create_prerender_request(
    model: Option<&'static ReactClientValue>,
    bundler_config: &ClientManifest,
    on_all_ready: &dyn Fn(),
    on_fatal_error: &dyn Fn(Option<&'static Unknown>),
    on_error: Option<&OnError>,
    identifier_prefix: Option<&str>,
    temporary_references: Option<&TemporaryReferenceSet>,
    debug_start_time: Option<f64>,                 // Profiling-only
    environment_name: Option<EnvironmentName>,     // DEV-only
    filter_stack_frame: Option<&FilterStackFrame>, // DEV-only
    keep_debug_alive: bool,                        // DEV-only
) -> &'static Request {
    let _ = (
        model,
        bundler_config,
        on_all_ready,
        on_fatal_error,
        on_error,
        identifier_prefix,
        temporary_references,
        debug_start_time,
        environment_name,
        filter_stack_frame,
        keep_debug_alive,
    );
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-server/src/ReactFlightServer#startWork"
)]
pub fn start_work(request: &Request) {
    let _ = request;
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-server/src/ReactFlightServer#startFlowing"
)]
pub fn start_flowing<D: ?Sized>(request: &Request, destination: &D) {
    let _ = (request, destination);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-server/src/ReactFlightServer#startFlowingDebug"
)]
pub fn start_flowing_debug<D: ?Sized>(request: &Request, debug_destination: &D) {
    let _ = (request, debug_destination);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-server/src/ReactFlightServer#stopFlowing"
)]
pub fn stop_flowing(request: &Request) {
    let _ = request;
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-server/src/ReactFlightServer#abort"
)]
pub fn abort(request: &Request, reason: Option<&Unknown>) {
    let _ = (request, reason);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-server/src/ReactFlightServer#attachAbortSignal"
)]
pub fn attach_abort_signal(request: &Request, signal: &AbortSignal) {
    let _ = (request, signal);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-server/src/ReactFlightServer#resolveDebugMessage"
)]
pub fn resolve_debug_message(request: &Request, message: &str) {
    let _ = (request, message);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-server/src/ReactFlightServer#closeDebugChannel"
)]
pub fn close_debug_channel(request: &Request) {
    let _ = request;
    unreachable!()
}
