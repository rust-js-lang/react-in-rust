// Copyright (c) Meta Platforms, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;
use std::panic::{self, AssertUnwindSafe};
use std::rc::Rc;

use js::{
    IntoUnknown, JsError, JsObject, Kind, Map, Rest, Unknown, WeakMap, function, map, weak_map,
};
use shared::react_shared_internals::REACT_SHARED_INTERNALS;
use webapi::AbortSignal;

#[derive(Clone, Copy, PartialEq)]
#[repr(u8)]
enum Status {
    #[cfg_attr(rust_js, rust_js::name = "UNTERMINATED")]
    Unterminated = 0,
    #[cfg_attr(rust_js, rust_js::name = "TERMINATED")]
    Terminated = 1,
    #[cfg_attr(rust_js, rust_js::name = "ERRORED")]
    Errored = 2,
}

/// What a function has cached of the arguments up to one of them.
struct CacheNode {
    s: Cell<Status>,
    v: Cell<Option<&'static Unknown>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    o: Cell<Option<&'static WeakMap<JsObject, Rc<CacheNode>>>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    p: Cell<Option<&'static Map<Option<&'static Unknown>, Rc<CacheNode>>>>,
}

unsafe impl js::Defined for CacheNode {}

/// A JS function of any arguments, TypeScript's `Function`.
pub type AnyFunction = dyn Fn() -> Option<&'static Unknown>;

/// What `cache` gives of a function: one JS calls with the same arguments,
/// cached.
pub type Cached = Box<dyn Fn(&dyn Rest<dyn IntoUnknown>) -> Option<&'static Unknown>>;

fn create_cache_root() -> &'static WeakMap<AnyFunction, Rc<CacheNode>> {
    weak_map::new()
}

fn create_cache_node() -> Rc<CacheNode> {
    Rc::new(CacheNode {
        s: Cell::new(Status::Unterminated), // status, represents whether the cached computation returned a value or threw an error
        v: Cell::new(None), // value, either the cached result or an error, depending on s
        o: Cell::new(None), // object cache, a WeakMap where non-primitive arguments are stored
        p: Cell::new(None), // primitive cache, a regular Map where primitive arguments are stored.
    })
}

pub fn cache(r#fn: &'static AnyFunction) -> Cached {
    Box::new(move |args: &dyn Rest<dyn IntoUnknown>| {
        let Some(dispatcher) = REACT_SHARED_INTERNALS.a() else {
            // If there is no dispatcher, then we treat this as not being cached.
            // $FlowFixMe[incompatible-type]: We don't want to use rest arguments since we transpile the code.
            return function::apply(r#fn, Some(&Kind::Null), Some(&args));
        };
        let fn_map = dispatcher.get_cache_for_type(create_cache_root);
        let fn_node = fn_map.get(r#fn);
        let mut cache_node;
        match fn_node {
            None => {
                cache_node = create_cache_node();
                fn_map.set(r#fn, Rc::clone(&cache_node));
            }
            Some(fn_node) => cache_node = fn_node,
        }
        for &arg in args.iter() {
            if let Kind::Function(_)
            // $FlowFixMe[invalid-compare]
            | Kind::Object(_)
            | Kind::Array(_) = js::classify(arg)
            {
                // Objects go into a WeakMap
                let arg: &JsObject = unsafe { js::cast(arg) };
                let object_cache = match cache_node.o.get() {
                    Some(object_cache) => object_cache,
                    None => {
                        let object_cache = weak_map::new();
                        cache_node.o.set(Some(object_cache));
                        object_cache
                    }
                };
                let object_node = object_cache.get(arg);
                match object_node {
                    None => {
                        cache_node = create_cache_node();
                        object_cache.set(arg, Rc::clone(&cache_node));
                    }
                    Some(object_node) => cache_node = object_node,
                }
            } else {
                // Primitives go into a regular Map
                let primitive_cache = match cache_node.p.get() {
                    Some(primitive_cache) => primitive_cache,
                    None => {
                        let primitive_cache = map::new(None);
                        cache_node.p.set(Some(primitive_cache));
                        primitive_cache
                    }
                };
                let primitive_node = primitive_cache.get(&arg);
                match primitive_node {
                    None => {
                        cache_node = create_cache_node();
                        primitive_cache.set(&arg, Rc::clone(&cache_node));
                    }
                    Some(primitive_node) => cache_node = primitive_node,
                }
            }
        }
        if cache_node.s.get() == Status::Terminated {
            return cache_node.v.get();
        }
        if cache_node.s.get() == Status::Errored {
            js::throw(cache_node.v.get());
        }
        match panic::catch_unwind(AssertUnwindSafe(|| {
            // $FlowFixMe[incompatible-type]: We don't want to use rest arguments since we transpile the code.
            function::apply(r#fn, Some(&Kind::Null), Some(&args))
        })) {
            Ok(result) => {
                let terminated_node = &cache_node;
                terminated_node.s.set(Status::Terminated);
                terminated_node.v.set(result);
                result
            }
            Err(error) => {
                // We store the first error that's thrown and rethrow it.
                let errored_node = &cache_node;
                errored_node.s.set(Status::Errored);
                errored_node.v.set(
                    error
                        .downcast_ref::<&'static JsError>()
                        .and_then(|error| error.as_unknown()),
                );
                panic::resume_unwind(error)
            }
        }
    })
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn cache_signal() -> Option<&'static AbortSignal> {
    let Some(dispatcher) = REACT_SHARED_INTERNALS.a() else {
        // If there is no dispatcher, then we treat this as not having an AbortSignal
        // since in the same context, a cached function will be allowed to be called
        // but it won't be cached. So it's neither an infinite AbortSignal nor an
        // already resolved one.
        return None;
    };
    dispatcher.cache_signal()
}
