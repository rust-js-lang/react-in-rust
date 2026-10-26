// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;
use std::rc::Rc;

use js::{Kind, Unknown};
use shared::environment::__DEV__;
use webapi::AbortSignal;

use crate::react_current_fiber::CURRENT as CURRENT_OWNER;
use crate::react_fiber_cache_component::{CACHE_CONTEXT, Cache};
use crate::react_fiber_new_context::read_context;
use crate::react_internal_types::Fiber;

/// What a resource type of `cache()` makes: any value, made once per cache.
type ResourceType = dyn Fn() -> Option<&'static Unknown>;

/// What gives the component rendering now, a Fiber, `None` where none is.
type GetOwner = fn() -> Option<Rc<Fiber>>;

/// The reconciler's `AsyncDispatcher`: what `ReactSharedInternals.A` is
/// while it renders.
pub struct DefaultAsyncDispatcher {
    pub get_cache_for_type: fn(&'static ResourceType) -> Option<&'static Unknown>,
    pub cache_signal: fn() -> Option<&'static AbortSignal>,
    // DEV only: the component rendering now, a Fiber.
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub get_owner: Cell<Option<GetOwner>>,
}

fn get_cache_for_type(resource_type: &'static ResourceType) -> Option<&'static Unknown> {
    let cache: Rc<Cache> = CACHE_CONTEXT.with(read_context).unwrap();
    let mut cache_for_type = cache.data.get(resource_type).flatten();
    if matches!(js::classify(cache_for_type), Kind::Undefined) {
        cache_for_type = resource_type();
        cache.data.set(resource_type, cache_for_type);
    }
    cache_for_type
}

fn cache_signal() -> Option<&'static AbortSignal> {
    let cache: Rc<Cache> = CACHE_CONTEXT.with(read_context).unwrap();
    Some(cache.controller.signal())
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
            dispatcher
                .get_owner
                .set(Some(|| CURRENT_OWNER.with_borrow(Clone::clone)));
        });
    }
}
