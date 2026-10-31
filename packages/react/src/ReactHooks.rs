// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::Unknown;
use shared::environment::__DEV__;
use shared::react_shared_internals::{Dispatcher, REACT_SHARED_INTERNALS};
use shared::react_symbols::REACT_CONSUMER_TYPE;
use shared::react_types::{ReactContext, RefObject, StartTransitionOptions, Usable};
use webapi::console;

/// The next state, or what makes it of the one before.
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum BasicStateAction<S: 'static> {
    Update(&'static dyn Fn(S) -> S),
    #[cfg_attr(rust_js, rust_js::otherwise)]
    State(S),
}

/// The first state, or what makes it.
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum InitialState<S: 'static> {
    Init(&'static dyn Fn() -> S),
    #[cfg_attr(rust_js, rust_js::otherwise)]
    State(S),
}

pub type Dispatch<A> = dyn Fn(A);

/// What an effect gives: the function that cleans it up, or nothing.
pub type Create = dyn Fn() -> Option<&'static dyn Fn()>;

/// What `useTransition` gives to start a Transition: of the callback to
/// call in it.
pub type StartTransition = dyn Fn(&'static dyn Fn(), Option<&StartTransitionOptions>);

/// A ref `useImperativeHandle` sets: an object's `current`, or a function
/// it calls.
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum Ref {
    Callback(&'static dyn Fn(Option<&'static Unknown>) -> Option<&'static Unknown>),
    #[cfg_attr(rust_js, rust_js::otherwise)]
    Object(&'static RefObject),
}

/// The reconciler's `Dispatcher`'s hooks, `ReactInternalTypes`'.
mod dispatcher {
    use super::*;

    #[cfg_attr(rust_js, rust_js::link_name = "useContext")]
    pub fn use_context<T>(this: &Dispatcher, context: &ReactContext<T>) -> T {
        let _ = (this, context);
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "useState")]
    pub fn use_state<S>(
        this: &Dispatcher,
        initial_state: InitialState<S>,
    ) -> (S, &'static Dispatch<BasicStateAction<S>>) {
        let _ = (this, initial_state);
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "useReducer")]
    pub fn use_reducer<S, I, A>(
        this: &Dispatcher,
        reducer: &'static dyn Fn(S, A) -> S,
        initial_arg: I,
        init: Option<&'static dyn Fn(I) -> S>,
    ) -> (S, &'static Dispatch<A>) {
        let _ = (this, reducer, initial_arg, init);
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "useRef")]
    pub fn use_ref(
        this: &Dispatcher,
        initial_value: Option<&'static Unknown>,
    ) -> &'static RefObject {
        let _ = (this, initial_value);
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "useEffect")]
    pub fn use_effect(
        this: &Dispatcher,
        create: Option<&'static Create>,
        deps: Option<&[Option<&'static Unknown>]>,
    ) {
        let _ = (this, create, deps);
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "useInsertionEffect")]
    pub fn use_insertion_effect(
        this: &Dispatcher,
        create: Option<&'static Create>,
        deps: Option<&[Option<&'static Unknown>]>,
    ) {
        let _ = (this, create, deps);
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "useLayoutEffect")]
    pub fn use_layout_effect(
        this: &Dispatcher,
        create: Option<&'static Create>,
        deps: Option<&[Option<&'static Unknown>]>,
    ) {
        let _ = (this, create, deps);
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "useCallback")]
    pub fn use_callback<T>(
        this: &Dispatcher,
        callback: T,
        deps: Option<&[Option<&'static Unknown>]>,
    ) -> T {
        let _ = (this, callback, deps);
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "useMemo")]
    pub fn use_memo<T>(
        this: &Dispatcher,
        create: &'static dyn Fn() -> T,
        deps: Option<&[Option<&'static Unknown>]>,
    ) -> T {
        let _ = (this, create, deps);
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "useImperativeHandle")]
    pub fn use_imperative_handle(
        this: &Dispatcher,
        r#ref: Option<Ref>,
        create: &'static dyn Fn() -> Option<&'static Unknown>,
        deps: Option<&[Option<&'static Unknown>]>,
    ) {
        let _ = (this, r#ref, create, deps);
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "useDebugValue")]
    pub fn use_debug_value<T>(
        this: &Dispatcher,
        value: T,
        formatter_fn: Option<&'static dyn Fn(T) -> Option<&'static Unknown>>,
    ) {
        let _ = (this, value, formatter_fn);
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "useTransition")]
    pub fn use_transition(this: &Dispatcher) -> (bool, &'static StartTransition) {
        let _ = this;
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "useDeferredValue")]
    pub fn use_deferred_value<T>(this: &Dispatcher, value: T, initial_value: Option<T>) -> T {
        let _ = (this, value, initial_value);
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "useId")]
    pub fn use_id(this: &Dispatcher) -> String {
        let _ = this;
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "useSyncExternalStore")]
    pub fn use_sync_external_store<T>(
        this: &Dispatcher,
        subscribe: &'static dyn Fn(&'static dyn Fn()) -> &'static dyn Fn(),
        get_snapshot: &'static dyn Fn() -> T,
        get_server_snapshot: Option<&'static dyn Fn() -> T>,
    ) -> T {
        let _ = (this, subscribe, get_snapshot, get_server_snapshot);
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "useCacheRefresh")]
    pub fn use_cache_refresh(this: &Dispatcher) -> &'static CacheRefresh {
        let _ = this;
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "use")]
    pub fn r#use<T>(this: &Dispatcher, usable: impl Usable<T>) -> T {
        let _ = (this, usable);
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "useMemoCache")]
    pub fn use_memo_cache(this: &Dispatcher, size: f64) -> Vec<Option<&'static Unknown>> {
        let _ = (this, size);
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "useEffectEvent")]
    pub fn use_effect_event<F>(this: &Dispatcher, callback: F) -> F {
        let _ = (this, callback);
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "useOptimistic")]
    pub fn use_optimistic<S, A>(
        this: &Dispatcher,
        passthrough: S,
        reducer: Option<&'static dyn Fn(S, A) -> S>,
    ) -> (S, &'static Dispatch<A>) {
        let _ = (this, passthrough, reducer);
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "useActionState")]
    pub fn use_action_state<S, P, R>(
        this: &Dispatcher,
        action: &'static dyn Fn(S, P) -> R,
        initial_state: S,
        permalink: Option<&str>,
    ) -> (S, &'static Dispatch<P>, bool) {
        let _ = (this, action, initial_state, permalink);
        unreachable!()
    }
}

/// What `useCacheRefresh` gives: what refreshes the cache, of what makes
/// its next value, and the value it seeds it with.
pub type CacheRefresh =
    dyn Fn(Option<&'static dyn Fn() -> Option<&'static Unknown>>, Option<&'static Unknown>);

fn resolve_dispatcher() -> &'static Dispatcher {
    let dispatcher = REACT_SHARED_INTERNALS.h();
    if __DEV__ && dispatcher.is_none() {
        console::error((concat!(
            "Invalid hook call. Hooks can only be called inside of the body of a function component. This could happen for",
            " one of the following reasons:\n",
            "1. You might have mismatching versions of React and the renderer (such as React DOM)\n",
            "2. You might be breaking the Rules of Hooks\n",
            "3. You might have more than one copy of React in the same app\n",
            "See https://react.dev/link/invalid-hook-call for tips about how to debug and fix this problem."
        ),));
    }
    // Will result in a null access error if accessed outside render phase. We
    // intentionally don't throw our own error because this is in a hot path.
    // Also helps ensure this is inlined.
    unsafe { dispatcher.unwrap_unchecked() }
}

pub fn get_cache_for_type<T>(resource_type: fn() -> T) -> T {
    let Some(dispatcher) = REACT_SHARED_INTERNALS.a() else {
        // If there is no dispatcher, then we treat this as not being cached.
        return resource_type();
    };
    dispatcher.get_cache_for_type(resource_type)
}

pub fn use_context<T>(
    #[cfg_attr(rust_js, rust_js::name = "Context")] context: &ReactContext<T>,
) -> T {
    let dispatcher = resolve_dispatcher();
    if __DEV__ && context.r#typeof == REACT_CONSUMER_TYPE {
        console::error((concat!(
            "Calling useContext(Context.Consumer) is not supported and will cause bugs. ",
            "Did you mean to call useContext(Context) instead?"
        ),));
    }
    dispatcher::use_context(dispatcher, context)
}

pub fn use_state<S>(initial_state: InitialState<S>) -> (S, &'static Dispatch<BasicStateAction<S>>) {
    let dispatcher = resolve_dispatcher();
    dispatcher::use_state(dispatcher, initial_state)
}

pub fn use_reducer<S, I, A>(
    reducer: &'static dyn Fn(S, A) -> S,
    initial_arg: I,
    init: Option<&'static dyn Fn(I) -> S>,
) -> (S, &'static Dispatch<A>) {
    let dispatcher = resolve_dispatcher();
    dispatcher::use_reducer(dispatcher, reducer, initial_arg, init)
}

pub fn use_ref(initial_value: Option<&'static Unknown>) -> &'static RefObject {
    let dispatcher = resolve_dispatcher();
    dispatcher::use_ref(dispatcher, initial_value)
}

pub fn use_effect(create: Option<&'static Create>, deps: Option<&[Option<&'static Unknown>]>) {
    if __DEV__ && create.is_none() {
        console::warn((
            "React Hook useEffect requires an effect callback. Did you forget to pass a callback to the hook?",
        ));
    }

    let dispatcher = resolve_dispatcher();
    dispatcher::use_effect(dispatcher, create, deps)
}

pub fn use_insertion_effect(
    create: Option<&'static Create>,
    deps: Option<&[Option<&'static Unknown>]>,
) {
    if __DEV__ && create.is_none() {
        console::warn((
            "React Hook useInsertionEffect requires an effect callback. Did you forget to pass a callback to the hook?",
        ));
    }

    let dispatcher = resolve_dispatcher();
    dispatcher::use_insertion_effect(dispatcher, create, deps)
}

pub fn use_layout_effect(
    create: Option<&'static Create>,
    deps: Option<&[Option<&'static Unknown>]>,
) {
    if __DEV__ && create.is_none() {
        console::warn((
            "React Hook useLayoutEffect requires an effect callback. Did you forget to pass a callback to the hook?",
        ));
    }

    let dispatcher = resolve_dispatcher();
    dispatcher::use_layout_effect(dispatcher, create, deps)
}

pub fn use_callback<T>(callback: T, deps: Option<&[Option<&'static Unknown>]>) -> T {
    let dispatcher = resolve_dispatcher();
    dispatcher::use_callback(dispatcher, callback, deps)
}

pub fn use_memo<T>(create: &'static dyn Fn() -> T, deps: Option<&[Option<&'static Unknown>]>) -> T {
    let dispatcher = resolve_dispatcher();
    dispatcher::use_memo(dispatcher, create, deps)
}

pub fn use_imperative_handle(
    r#ref: Option<Ref>,
    create: &'static dyn Fn() -> Option<&'static Unknown>,
    deps: Option<&[Option<&'static Unknown>]>,
) {
    let dispatcher = resolve_dispatcher();
    dispatcher::use_imperative_handle(dispatcher, r#ref, create, deps)
}

pub fn use_debug_value<T>(
    value: T,
    formatter_fn: Option<&'static dyn Fn(T) -> Option<&'static Unknown>>,
) {
    if __DEV__ {
        let dispatcher = resolve_dispatcher();
        dispatcher::use_debug_value(dispatcher, value, formatter_fn)
    }
}

pub fn use_transition() -> (bool, &'static StartTransition) {
    let dispatcher = resolve_dispatcher();
    dispatcher::use_transition(dispatcher)
}

pub fn use_deferred_value<T>(value: T, initial_value: Option<T>) -> T {
    let dispatcher = resolve_dispatcher();
    dispatcher::use_deferred_value(dispatcher, value, initial_value)
}

pub fn use_id() -> String {
    let dispatcher = resolve_dispatcher();
    dispatcher::use_id(dispatcher)
}

pub fn use_sync_external_store<T>(
    subscribe: &'static dyn Fn(&'static dyn Fn()) -> &'static dyn Fn(),
    get_snapshot: &'static dyn Fn() -> T,
    get_server_snapshot: Option<&'static dyn Fn() -> T>,
) -> T {
    let dispatcher = resolve_dispatcher();
    dispatcher::use_sync_external_store(dispatcher, subscribe, get_snapshot, get_server_snapshot)
}

pub fn use_cache_refresh() -> &'static CacheRefresh {
    let dispatcher = resolve_dispatcher();
    // $FlowFixMe[not-a-function] This is unstable, thus optional
    dispatcher::use_cache_refresh(dispatcher)
}

pub fn r#use<T>(usable: impl Usable<T>) -> T {
    let dispatcher = resolve_dispatcher();
    dispatcher::r#use(dispatcher, usable)
}

pub fn use_memo_cache(size: f64) -> Vec<Option<&'static Unknown>> {
    let dispatcher = resolve_dispatcher();
    // $FlowFixMe[not-a-function] This is unstable, thus optional
    dispatcher::use_memo_cache(dispatcher, size)
}

pub fn use_effect_event<F>(callback: F) -> F {
    let dispatcher = resolve_dispatcher();
    // $FlowFixMe[not-a-function] This is unstable, thus optional
    dispatcher::use_effect_event(dispatcher, callback)
}

pub fn use_optimistic<S, A>(
    passthrough: S,
    reducer: Option<&'static dyn Fn(S, A) -> S>,
) -> (S, &'static Dispatch<A>) {
    let dispatcher = resolve_dispatcher();
    dispatcher::use_optimistic(dispatcher, passthrough, reducer)
}

pub fn use_action_state<S, P, R>(
    action: &'static dyn Fn(S, P) -> R,
    initial_state: S,
    permalink: Option<&str>,
) -> (S, &'static Dispatch<P>, bool) {
    let dispatcher = resolve_dispatcher();
    dispatcher::use_action_state(dispatcher, action, initial_state, permalink)
}
