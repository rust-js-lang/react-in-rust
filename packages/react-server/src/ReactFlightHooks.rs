// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::rc::Rc;

use js::{JsObject, Map, SymbolFor, Unknown};
use react_reconciler::react_fiber_hooks::CacheRefresh;
use react_reconciler::react_internal_types::{Dispatcher, MemoCacheArray};
use shared::environment::__DEV__;
use shared::react_symbols::{REACT_CONTEXT_TYPE, REACT_MEMO_CACHE_SENTINEL};
use shared::react_types::{ReactComponentInfo, Thenable};
use webapi::{AbortController, AbortSignal};

use crate::react_flight_server_temporary_references::TemporaryReferenceSet;
use crate::react_flight_thenable::{ThenableState, create_thenable_state, track_used_thenable};

/// ReactFlightServer's request, its JS's until it's ported: what's read of
/// it here.
pub struct Request(PhantomData<JsObject>);

unsafe impl js::Defined for Request {}

impl Request {
    #[cfg_attr(rust_js, rust_js::link_name = "get identifierCount")]
    fn identifier_count(&self) -> f64 {
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "set identifierCount")]
    fn set_identifier_count(&self, count: f64) {
        let _ = count;
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "get identifierPrefix")]
    fn identifier_prefix(&self) -> String {
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "get cacheController")]
    pub(crate) fn cache_controller(&self) -> &'static AbortController {
        unreachable!()
    }
}

/// What a resource type of `cache()` makes: any value, made once per cache.
pub type ResourceType = dyn Fn() -> Option<&'static Unknown>;

/// A request's cache, of each resource type.
pub type Cache = Map<ResourceType, Option<&'static Unknown>>;

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightServer.js#resolveRequest"
)]
#[cfg_attr(rust_js, rust_js::nullable)]
pub(crate) fn resolve_request() -> Option<&'static Request> {
    unreachable!()
}

#[cfg_attr(rust_js, rust_js::link_name = "./src/ReactFlightServer.js#getCache")]
pub(crate) fn get_cache(request: &Request) -> &'static Cache {
    let _ = request;
    unreachable!()
}

// ReactFlightServer's functions a server entry calls, its JS's until it's
// ported: of each bundler's own client manifest, `C`.

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
pub fn create_request<C: ?Sized>(
    model: Option<&'static ReactClientValue>,
    bundler_config: &C,
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
pub fn create_prerender_request<C: ?Sized>(
    model: Option<&'static ReactClientValue>,
    bundler_config: &C,
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

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightServerConfig.js#isClientReference"
)]
fn is_client_reference(reference: Option<&'static Unknown>) -> bool {
    let _ = reference;
    unreachable!()
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::nullable)]
    static CURRENT_REQUEST: Cell<Option<&'static Request>> = const { Cell::new(None) };
    static THENABLE_INDEX_COUNTER: Cell<usize> = const { Cell::new(0) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static THENABLE_STATE: RefCell<Option<Rc<ThenableState>>> = const { RefCell::new(None) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static CURRENT_COMPONENT_DEBUG_INFO: Cell<Option<&'static ReactComponentInfo>> = const { Cell::new(None) };
}

pub fn prepare_to_use_hooks_for_request(request: &'static Request) {
    CURRENT_REQUEST.set(Some(request));
}

pub fn reset_hooks_for_request() {
    CURRENT_REQUEST.set(None);
}

#[cfg_attr(rust_js, rust_js::nullable(prev_thenable_state, component_debug_info))]
pub fn prepare_to_use_hooks_for_component(
    prev_thenable_state: Option<Rc<ThenableState>>,
    component_debug_info: Option<&'static ReactComponentInfo>,
) {
    THENABLE_INDEX_COUNTER.set(0);
    THENABLE_STATE.set(prev_thenable_state);
    if __DEV__ {
        CURRENT_COMPONENT_DEBUG_INFO.set(component_debug_info);
    }
}

pub fn get_thenable_state_after_suspending() -> Rc<ThenableState> {
    // If you use() to Suspend this should always exist but if you throw a Promise instead,
    // which is not really supported anymore, it will be empty. We use the empty set as a
    // marker to know if this was a replay of the same component or first attempt.
    let state = THENABLE_STATE
        .with_borrow(Clone::clone)
        .unwrap_or_else(|| Rc::new(create_thenable_state()));
    if __DEV__ {
        // This is a hack but we stash the debug info here so that we don't need a completely
        // different data structure just for this in DEV. Not too happy about it.
        js::set(
            js::unknown(&*state),
            "_componentDebugInfo",
            CURRENT_COMPONENT_DEBUG_INFO.get(),
        );
        CURRENT_COMPONENT_DEBUG_INFO.set(None);
    }
    THENABLE_STATE.set(None);
    state
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_tracked_thenables_after_rendering() -> Option<Rc<ThenableState>> {
    THENABLE_STATE.with_borrow(Clone::clone)
}

#[cfg_attr(rust_js, rust_js::name = "HooksDispatcher")]
pub static HOOKS_DISPATCHER: Dispatcher = Dispatcher {
    read_context: |_| unsupported_context(),

    r#use,
    use_callback: |callback, _| callback,
    use_context: |_| unsupported_context(),
    use_effect: |_, _| unsupported_hook(),
    use_imperative_handle: |_, _, _| unsupported_hook(),
    use_layout_effect: |_, _| unsupported_hook(),
    use_insertion_effect: |_, _| unsupported_hook(),
    use_memo: |next_create, _| next_create(),
    use_reducer: |_, _, _| unsupported_hook(),
    use_ref: |_| unsupported_hook(),
    use_state: |_| unsupported_hook(),
    use_debug_value: |_, _| {},
    use_deferred_value: |_, _| unsupported_hook(),
    use_transition: || unsupported_hook(),
    use_sync_external_store: |_, _, _| unsupported_hook(),
    use_id,
    use_host_transition_status: || unsupported_hook(),
    use_form_state: |_, _, _| unsupported_hook(),
    use_action_state: |_, _, _| unsupported_hook(),
    use_optimistic: |_, _| unsupported_hook(),
    use_memo_cache,
    use_cache_refresh,
    use_effect_event: |_| unsupported_hook(),
};

fn unsupported_hook() -> ! {
    panic!("This Hook is not supported in Server Components.");
}

fn unsupported_refresh(_fetch: Option<&'static Unknown>, _seed_key: Option<&'static Unknown>) {
    panic!("Refreshing the cache is not supported in Server Components.");
}

fn unsupported_context() -> ! {
    panic!("Cannot read a Client Context from a Server Component.");
}

fn use_id() -> String {
    let Some(request) = CURRENT_REQUEST.get() else {
        panic!("useId can only be used while React is rendering");
    };
    let id = request.identifier_count();
    request.set_identifier_count(id + 1.0);
    // use 'S' for Flight components to distinguish from 'R' and 'r' in Fizz/Client
    "_".to_owned()
        + &request.identifier_prefix()
        + "S_"
        + &js::number::to_string(id, Some(32))
        + "_"
}

fn r#use(usable: Option<&'static Unknown>) -> Option<&'static Unknown> {
    if let Some(usable) = usable
        && (js::type_of(Some(usable)) == "object" || js::type_of(Some(usable)) == "function")
    {
        // $FlowFixMe[method-unbinding]
        if js::type_of(js::get(usable, "then")) == "function" {
            // This is a thenable.
            let thenable: &'static Thenable = unsafe { js::cast(Some(usable)) };

            // Track the position of the thenable within this fiber.
            let index = THENABLE_INDEX_COUNTER.get();
            THENABLE_INDEX_COUNTER.set(index + 1);

            let thenable_state = THENABLE_STATE.with_borrow_mut(|thenable_state| {
                Rc::clone(thenable_state.get_or_insert_with(|| Rc::new(create_thenable_state())))
            });
            return track_used_thenable(&thenable_state, thenable, index);
        } else if SymbolFor::from_value(js::get(usable, "$$typeof")) == Some(REACT_CONTEXT_TYPE) {
            unsupported_context();
        }
    }

    if is_client_reference(usable) {
        let value = js::get(usable, "value");
        if value.is_some()
            && SymbolFor::from_value(js::get(value, "$$typeof")) == Some(REACT_CONTEXT_TYPE)
        {
            // Show a more specific message since it's a common mistake.
            panic!("Cannot read a Client Context from a Server Component.");
        } else {
            panic!("Cannot use() an already resolved Client Reference.");
        }
    } else {
        // eslint-disable-next-line react-internal/safe-string-coercion
        panic!(
            "An unsupported type was passed to use(): {}",
            js::string(usable)
        );
    }
}

fn use_memo_cache(size: usize) -> MemoCacheArray {
    Rc::new(RefCell::new(vec![
        Some(js::unknown(
            &REACT_MEMO_CACHE_SENTINEL
        ));
        size
    ]))
}

fn use_cache_refresh() -> &'static CacheRefresh {
    &unsupported_refresh
}
