// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;

use js::{Dict, Error, SymbolFor, Unknown};
use webapi::ConsoleTask;

use crate::react_types::{ReactDebugInfo, ReactKey};

pub struct ReactElement {
    #[cfg_attr(rust_js, rust_js::name = "$$typeof")]
    pub r#typeof: SymbolFor<'static>,
    pub r#type: Option<&'static Unknown>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub key: Option<ReactKey>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub r#ref: Option<&'static Unknown>,
    pub props: &'static Unknown,
    // __DEV__ or for string refs
    #[cfg_attr(rust_js, rust_js::name = "_owner")]
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub owner: Option<&'static Unknown>,

    // __DEV__
    #[cfg_attr(rust_js, rust_js::name = "_store")]
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub store: Cell<Option<&'static Dict<f64>>>, // 0: not validated, 1: validated, 2: force fail
    #[cfg_attr(rust_js, rust_js::name = "_debugInfo")]
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub debug_info: Option<&'static ReactDebugInfo>,
    #[cfg_attr(rust_js, rust_js::name = "_debugStack")]
    pub debug_stack: Option<&'static Error>,
    #[cfg_attr(rust_js, rust_js::name = "_debugTask")]
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub debug_task: Option<&'static ConsoleTask>,
}

unsafe impl js::Defined for ReactElement {}
