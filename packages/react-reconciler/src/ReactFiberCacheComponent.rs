// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use js::{Class, Map, Unknown};
use react_isomorphic::react_context::ReactContext;
use shared::environment::__DEV__;
use shared::react_symbols::REACT_CONTEXT_TYPE;
use webapi::{AbortController, console};

use crate::react_fiber_new_context::{pop_provider, push_provider};
use crate::react_internal_types::Fiber;

// Intentionally not named imports because Rollup would
// use dynamic dispatch for CommonJS interop named imports.
#[cfg_attr(rust_js, rust_js::link_name = "scheduler#*.unstable_scheduleCallback")]
fn schedule_callback(priority_level: u8, callback: Box<dyn FnOnce()>) {
    unreachable!()
}

unsafe extern "Rust" {
    #[link_name = "scheduler#*.unstable_NormalPriority"]
    safe static NORMAL_PRIORITY: u8;
}

/// What's called when an `AbortControllerShim` aborts.
type Listener = Rc<dyn Fn()>;

/// What an `AbortSignalShim` is given a listener with.
type AddEventListener = dyn Fn(&str, Listener);

/// The signal of an `AbortControllerShim`: only what React uses of one.
struct AbortSignalShim {
    aborted: Cell<bool>,
    add_event_listener: Box<AddEventListener>,
}

#[cfg_attr(rust_js, rust_js::class)]
struct AbortControllerShim {
    signal: Rc<AbortSignalShim>,
    abort: Box<dyn Fn()>,
}

#[cfg_attr(rust_js, rust_js::class)]
impl AbortControllerShim {
    fn new() -> Self {
        let listeners: Rc<RefCell<Vec<Listener>>> = Rc::new(RefCell::new(Vec::new()));
        let signal = Rc::new(AbortSignalShim {
            aborted: Cell::new(false),
            add_event_listener: {
                let listeners = Rc::clone(&listeners);
                Box::new(move |_type, listener| listeners.borrow_mut().push(listener))
            },
        });

        AbortControllerShim {
            signal: Rc::clone(&signal),
            abort: Box::new(move || {
                signal.aborted.set(true);
                // As forEach does: the listeners there when it starts, each
                // called with no borrow held, so one may add another.
                let count = listeners.borrow().len();
                for index in 0..count {
                    let listener = Rc::clone(&listeners.borrow()[index]);
                    listener();
                }
            }),
        }
    }
}

thread_local! {
    // In environments without AbortController (e.g. tests)
    // replace it with a lightweight shim that only has the features we use.
    #[cfg_attr(rust_js, rust_js::name = "AbortControllerLocal")]
    static ABORT_CONTROLLER_LOCAL: &'static Class = if js::is_defined(webapi::abort_controller::new) {
        js::class::<AbortController>()
    } else {
        js::class::<AbortControllerShim>()
    };
}

pub struct Cache {
    pub controller: &'static AbortController,
    pub data: &'static Map<dyn Fn() -> Option<&'static Unknown>, Option<&'static Unknown>>,
    pub ref_count: Cell<i32>,
}

pub struct CacheComponentState {
    pub parent: Rc<Cache>,
    pub cache: Rc<Cache>,
}

pub struct SpawnedCachePool {
    pub parent: Rc<Cache>,
    pub pool: Rc<Cache>,
}

// Safety: each is a JS object: a CacheComponent's state, and a pool a
// boundary spawned.
unsafe impl js::Defined for CacheComponentState {}
unsafe impl js::Defined for SpawnedCachePool {}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "CacheContext")]
    pub static CACHE_CONTEXT: Rc<ReactContext<Option<Rc<Cache>>>> = Rc::new(ReactContext {
        r#typeof: REACT_CONTEXT_TYPE,
        // We don't use Consumer/Provider for Cache components. So we'll cheat.
        consumer: Cell::new(None),
        provider: Cell::new(Weak::new()),
        // We'll initialize these at the root.
        current_value: RefCell::new(js::null()),
        current_value2: RefCell::new(js::null()),
        thread_count: Cell::new(0.0),
        current_renderer: Cell::new(None),
        current_renderer2: Cell::new(None),
    });
}

js::on_load! {
    if __DEV__ {
        CACHE_CONTEXT.with(|cache_context| {
            cache_context.current_renderer.set(None);
            cache_context.current_renderer2.set(None);
        });
    }
}

// Creates a new empty Cache instance with a ref-count of 0. The caller is responsible
// for retaining the cache once it is in use (retainCache), and releasing the cache
// once it is no longer needed (releaseCache).
pub fn create_cache() -> Rc<Cache> {
    Rc::new(Cache {
        controller: ABORT_CONTROLLER_LOCAL.with(|abort_controller_local| {
            abort_controller_local.call_static(webapi::abort_controller::new, ())
        }),
        data: js::map::new(None),
        ref_count: Cell::new(0),
    })
}

pub fn retain_cache(cache: &Cache) {
    if __DEV__ && cache.controller.signal().aborted() {
        console::warn((concat!(
            "A cache instance was retained after it was already freed. ",
            "This likely indicates a bug in React."
        ),));
    }
    cache.ref_count.set(cache.ref_count.get() + 1);
}

// Cleanup a cache instance, potentially freeing it if there are no more references
pub fn release_cache(cache: &Rc<Cache>) {
    cache.ref_count.set(cache.ref_count.get() - 1);
    if __DEV__ && cache.ref_count.get() < 0 {
        console::warn((concat!(
            "A cache instance was released after it was already freed. ",
            "This likely indicates a bug in React."
        ),));
    }
    if cache.ref_count.get() == 0 {
        let cache = Rc::clone(cache);
        schedule_callback(
            NORMAL_PRIORITY,
            Box::new(move || {
                cache.controller.abort(None);
            }),
        );
    }
}

pub fn push_cache_provider(work_in_progress: &Rc<Fiber>, cache: Rc<Cache>) {
    CACHE_CONTEXT.with(|cache_context| push_provider(work_in_progress, cache_context, Some(cache)));
}

pub fn pop_cache_provider(work_in_progress: &Fiber, _cache: &Cache) {
    CACHE_CONTEXT.with(|cache_context| pop_provider(cache_context, work_in_progress));
}
