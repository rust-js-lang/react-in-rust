// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{SymbolFor, Unknown};
use shared::react_symbols::REACT_RECOVERABLE_TYPE;

/// shared/ReactTypes' `ReactRecoverableReason`: a message, or what gives one.
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum ReactRecoverableReason {
    Message(String),
    Lazy(&'static dyn Fn() -> Option<&'static Unknown>),
}

/// shared/ReactTypes' `ReactRecoverable`.
// A recoverable lets an intermediate renderer defer a subtree to a downstream
// renderer. It does not produce a value: a renderer either continues through
// it or interrupts the current render so that a later renderer can recover the
// subtree. The reason is initialized only by a renderer that defers the work.
pub struct ReactRecoverable {
    #[cfg_attr(rust_js, rust_js::name = "$$typeof")]
    pub r#typeof: SymbolFor<'static>,
    #[cfg_attr(rust_js, rust_js::name = "_reason")]
    pub reason: Option<ReactRecoverableReason>,
}

pub fn browser(reason: Option<ReactRecoverableReason>) -> ReactRecoverable {
    // This also runs in the browser, where the reason is never observed. Keep the
    // value cheap and let an SSR renderer initialize the error if it defers work.
    ReactRecoverable {
        r#typeof: REACT_RECOVERABLE_TYPE,
        reason,
    }
}
