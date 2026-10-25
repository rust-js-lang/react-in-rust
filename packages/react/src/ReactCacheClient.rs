// Copyright (c) Meta Platforms, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{IntoUnknown, Kind, Rest, function};
use shared::react_feature_flags::DISABLE_CLIENT_CACHE;
use webapi::AbortSignal;

use crate::react_cache_impl::{
    AnyFunction, Cached, cache as cache_impl, cache_signal as cache_signal_impl,
};

fn noop_cache(r#fn: &'static AnyFunction) -> Cached {
    // On the client (i.e. not a Server Components environment) `cache` has
    // no caching behavior. We just return the function as-is.
    //
    // We intend to implement client caching in a future major release. In the
    // meantime, it's only exposed as an API so that Shared Components can use
    // per-request caching on the server without breaking on the client. But it
    // does mean they need to be aware of the behavioral difference.
    //
    // The rest of the behavior is the same as the server implementation — it
    // returns a new reference, extra properties like `displayName` are not
    // preserved, the length of the new function is 0, etc. That way apps can't
    // accidentally depend on those details.
    Box::new(move |args: &dyn Rest<dyn IntoUnknown>| {
        // $FlowFixMe[incompatible-type]: We don't want to use rest arguments since we transpile the code.
        function::apply(r#fn, Some(&Kind::Null), Some(&args))
    })
}

thread_local! {
    pub static CACHE: fn(&'static AnyFunction) -> Cached = if DISABLE_CLIENT_CACHE {
        noop_cache
    } else {
        cache_impl
    };
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn noop_cache_signal() -> Option<&'static AbortSignal> {
    None
}

thread_local! {
    pub static CACHE_SIGNAL: fn() -> Option<&'static AbortSignal> = if DISABLE_CLIENT_CACHE {
        noop_cache_signal
    } else {
        cache_signal_impl
    };
}
