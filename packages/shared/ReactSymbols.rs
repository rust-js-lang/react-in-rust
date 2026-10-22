// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{Symbol, Unknown};

// ATTENTION
// When adding new symbols to this file,
// Please consider also adding to 'react-devtools-shared/src/backend/ReactSymbols'

thread_local! {
    // The Symbol used to tag the ReactElement-like types.
    pub static REACT_LEGACY_ELEMENT_TYPE: &'static Symbol = js::symbol::r#for("react.element");
    pub static REACT_ELEMENT_TYPE: &'static Symbol = js::symbol::r#for("react.transitional.element");

    pub static REACT_PORTAL_TYPE: &'static Symbol = js::symbol::r#for("react.portal");
    pub static REACT_FRAGMENT_TYPE: &'static Symbol = js::symbol::r#for("react.fragment");
    pub static REACT_STRICT_MODE_TYPE: &'static Symbol = js::symbol::r#for("react.strict_mode");
    pub static REACT_PROFILER_TYPE: &'static Symbol = js::symbol::r#for("react.profiler");
    pub static REACT_CONSUMER_TYPE: &'static Symbol = js::symbol::r#for("react.consumer");
    pub static REACT_CONTEXT_TYPE: &'static Symbol = js::symbol::r#for("react.context");
    pub static REACT_FORWARD_REF_TYPE: &'static Symbol = js::symbol::r#for("react.forward_ref");
    pub static REACT_SUSPENSE_TYPE: &'static Symbol = js::symbol::r#for("react.suspense");
    pub static REACT_SUSPENSE_LIST_TYPE: &'static Symbol = js::symbol::r#for("react.suspense_list");
    pub static REACT_MEMO_TYPE: &'static Symbol = js::symbol::r#for("react.memo");
    pub static REACT_LAZY_TYPE: &'static Symbol = js::symbol::r#for("react.lazy");
    pub static REACT_SCOPE_TYPE: &'static Symbol = js::symbol::r#for("react.scope");
    pub static REACT_ACTIVITY_TYPE: &'static Symbol = js::symbol::r#for("react.activity");
    pub static REACT_LEGACY_HIDDEN_TYPE: &'static Symbol = js::symbol::r#for("react.legacy_hidden");
    pub static REACT_TRACING_MARKER_TYPE: &'static Symbol = js::symbol::r#for("react.tracing_marker");

    pub static REACT_MEMO_CACHE_SENTINEL: &'static Symbol = js::symbol::r#for("react.memo_cache_sentinel");

    pub static REACT_VIEW_TRANSITION_TYPE: &'static Symbol = js::symbol::r#for("react.view_transition");

    pub static REACT_RECOVERABLE_TYPE: &'static Symbol = js::symbol::r#for("react.recoverable");

    static MAYBE_ITERATOR_SYMBOL: &'static Symbol = js::symbol::iterator;
}
const FAUX_ITERATOR_SYMBOL: &str = "@@iterator";

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_iterator_fn(maybe_iterable: Option<&'static Unknown>) -> Option<&'static Unknown> {
    let maybe_iterable = match maybe_iterable {
        Some(value) if js::type_of(Some(value)) == "object" => value,
        _ => return None,
    };
    let maybe_iterator = match MAYBE_ITERATOR_SYMBOL.with(|symbol| js::get(maybe_iterable, *symbol))
    {
        found if js::truthy(found) => found,
        _ => js::get(maybe_iterable, FAUX_ITERATOR_SYMBOL),
    };
    if js::type_of(maybe_iterator) == "function" {
        return maybe_iterator;
    }
    None
}

thread_local! {
    pub static ASYNC_ITERATOR: &'static Symbol = js::symbol::async_iterator;

    pub static REACT_OPTIMISTIC_KEY: &'static Symbol = js::symbol::r#for("react.optimistic_key");
}

// This is actually a symbol but Flow doesn't support comparison of symbols to refine.
// We use a boolean since in our code we often expect string (key) or number (index),
// so by pretending to be a boolean we cover a lot of cases that don't consider this case.
