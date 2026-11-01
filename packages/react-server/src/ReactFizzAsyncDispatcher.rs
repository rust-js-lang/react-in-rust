// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;

use js::Unknown;
use shared::environment::__DEV__;
use shared::react_shared_internals::CacheDispatcher;
use webapi::AbortSignal;

use crate::react_fizz_component_stack::ComponentStackNode;
use crate::react_fizz_current_task::CURRENT_TASK_IN_DEV;

/// What a resource type of `cache()` makes.
type ResourceType = dyn Fn() -> Option<&'static Unknown>;

/// What gives the component rendering now, its stack's node.
type GetOwner = fn() -> Option<&'static ComponentStackNode>;

/// Fizz's `AsyncDispatcher`: what `ReactSharedInternals.A` is while it renders.
pub struct DefaultAsyncDispatcher {
    pub get_cache_for_type: fn(&'static ResourceType) -> Option<&'static Unknown>,
    pub cache_signal: fn() -> Option<&'static AbortSignal>,
    // DEV only: the component rendering now.
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub get_owner: Cell<Option<GetOwner>>,
}

impl CacheDispatcher for DefaultAsyncDispatcher {}

fn get_cache_for_type(_resource_type: &'static ResourceType) -> Option<&'static Unknown> {
    panic!("Not implemented.");
}

fn cache_signal() -> Option<&'static AbortSignal> {
    panic!("Not implemented.");
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "DefaultAsyncDispatcher")]
    pub static DEFAULT_ASYNC_DISPATCHER: DefaultAsyncDispatcher = DefaultAsyncDispatcher {
        get_cache_for_type,
        cache_signal,
        get_owner: Cell::new(None),
    };
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn get_owner() -> Option<&'static ComponentStackNode> {
    let current_task_in_dev = CURRENT_TASK_IN_DEV.get()?;
    current_task_in_dev.component_stack()
}

js::on_load! {
    if __DEV__ {
        DEFAULT_ASYNC_DISPATCHER.with(|dispatcher| {
            dispatcher.get_owner.set(Some(get_owner));
        });
    }
}
