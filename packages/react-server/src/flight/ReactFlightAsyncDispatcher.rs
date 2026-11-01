// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;

use js::{Kind, Unknown};
use shared::environment::__DEV__;
use shared::react_shared_internals::CacheDispatcher;
use shared::react_types::ReactComponentInfo;
use webapi::AbortSignal;

use crate::react_flight_current_owner::resolve_owner;
use crate::react_flight_hooks::{Cache, ResourceType, get_cache, resolve_request};

/// What gives the component rendering now, its info.
type GetOwner = fn() -> Option<&'static ReactComponentInfo>;

/// Flight's `AsyncDispatcher`: what `ReactSharedInternals.A` is while it
/// renders.
pub struct DefaultAsyncDispatcher {
    pub get_cache_for_type: fn(&'static ResourceType) -> Option<&'static Unknown>,
    pub cache_signal: fn() -> Option<&'static AbortSignal>,
    // DEV only: the component rendering now.
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub get_owner: Cell<Option<GetOwner>>,
}

impl CacheDispatcher for DefaultAsyncDispatcher {}

fn resolve_cache() -> &'static Cache {
    let request = resolve_request();
    if let Some(request) = request {
        return get_cache(request);
    }
    js::map::new(None)
}

fn get_cache_for_type(resource_type: &'static ResourceType) -> Option<&'static Unknown> {
    let cache = resolve_cache();
    let mut entry = cache.get(resource_type).flatten();
    if matches!(js::classify(entry), Kind::Undefined) {
        entry = resource_type();
        // TODO: Warn if undefined?
        cache.set(resource_type, entry);
    }
    entry
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn cache_signal() -> Option<&'static AbortSignal> {
    let request = resolve_request();
    if let Some(request) = request {
        return Some(request.cache_controller().signal());
    }
    None
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "DefaultAsyncDispatcher")]
    pub static DEFAULT_ASYNC_DISPATCHER: DefaultAsyncDispatcher = DefaultAsyncDispatcher {
        get_cache_for_type,
        cache_signal,
        get_owner: Cell::new(None),
    };
}

js::on_load! {
    if __DEV__ {
        DEFAULT_ASYNC_DISPATCHER.with(|dispatcher| {
            dispatcher.get_owner.set(Some(resolve_owner));
        });
    }
}
