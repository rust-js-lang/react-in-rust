// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::marker::PhantomData;

/// The state React shares with its renderers, the `react` package's own.
pub struct SharedStateClient(PhantomData<js::JsObject>);

impl SharedStateClient {
    /// How many owner stacks were made since the limit was last reset.
    #[cfg_attr(rust_js, rust_js::link_name = "set recentlyCreatedOwnerStacks")]
    pub fn set_recently_created_owner_stacks(&self, count: f64) {
        let _ = count;
        unreachable!()
    }
}

unsafe extern "Rust" {
    #[cfg_attr(rust_js, rust_js::name = "ReactSharedInternals")]
    #[link_name = "react#*.__CLIENT_INTERNALS_DO_NOT_USE_OR_WARN_USERS_THEY_CANNOT_UPGRADE"]
    pub safe static REACT_SHARED_INTERNALS: &'static SharedStateClient;
}

js::export_default!(REACT_SHARED_INTERNALS);
