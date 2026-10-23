// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{Symbol, SymbolFor, Unknown};

// ATTENTION
// When adding new symbols to this file,
// Please consider also adding to 'react-devtools-shared/src/backend/ReactSymbols'

// The Symbol used to tag the ReactElement-like types.
pub const REACT_LEGACY_ELEMENT_TYPE: SymbolFor = SymbolFor("react.element");
pub const REACT_ELEMENT_TYPE: SymbolFor = SymbolFor("react.transitional.element");

pub const REACT_PORTAL_TYPE: SymbolFor = SymbolFor("react.portal");
pub const REACT_FRAGMENT_TYPE: SymbolFor = SymbolFor("react.fragment");
pub const REACT_STRICT_MODE_TYPE: SymbolFor = SymbolFor("react.strict_mode");
pub const REACT_PROFILER_TYPE: SymbolFor = SymbolFor("react.profiler");
pub const REACT_CONSUMER_TYPE: SymbolFor = SymbolFor("react.consumer");
pub const REACT_CONTEXT_TYPE: SymbolFor = SymbolFor("react.context");
pub const REACT_FORWARD_REF_TYPE: SymbolFor = SymbolFor("react.forward_ref");
pub const REACT_SUSPENSE_TYPE: SymbolFor = SymbolFor("react.suspense");
pub const REACT_SUSPENSE_LIST_TYPE: SymbolFor = SymbolFor("react.suspense_list");
pub const REACT_MEMO_TYPE: SymbolFor = SymbolFor("react.memo");
pub const REACT_LAZY_TYPE: SymbolFor = SymbolFor("react.lazy");
pub const REACT_SCOPE_TYPE: SymbolFor = SymbolFor("react.scope");
pub const REACT_ACTIVITY_TYPE: SymbolFor = SymbolFor("react.activity");
pub const REACT_LEGACY_HIDDEN_TYPE: SymbolFor = SymbolFor("react.legacy_hidden");
pub const REACT_TRACING_MARKER_TYPE: SymbolFor = SymbolFor("react.tracing_marker");

pub const REACT_MEMO_CACHE_SENTINEL: SymbolFor = SymbolFor("react.memo_cache_sentinel");

pub const REACT_VIEW_TRANSITION_TYPE: SymbolFor = SymbolFor("react.view_transition");

pub const REACT_RECOVERABLE_TYPE: SymbolFor = SymbolFor("react.recoverable");

thread_local! {
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
}

pub const REACT_OPTIMISTIC_KEY: SymbolFor = SymbolFor("react.optimistic_key");

// This is actually a symbol but Flow doesn't support comparison of symbols to refine.
// We use a boolean since in our code we often expect string (key) or number (index),
// so by pretending to be a boolean we cover a lot of cases that don't consider this case.
