// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use react_isomorphic::react_shared_internals_server::SharedStateServer;

unsafe extern "Rust" {
    /// The `react` package's server internals: `None` where `react` resolved
    /// to its client entry, which has none.
    #[cfg_attr(rust_js, rust_js::name = "ReactSharedInternalsServer")]
    #[link_name = "react#*.__SERVER_INTERNALS_DO_NOT_USE_OR_WARN_USERS_THEY_CANNOT_UPGRADE"]
    pub safe static REACT_SHARED_INTERNALS_SERVER: Option<&'static SharedStateServer>;
}

js::on_load! {
    if REACT_SHARED_INTERNALS_SERVER.is_none() {
        panic!(
            "The \"react\" package in this environment is not configured correctly. \
             The \"react-server\" condition must be enabled in any environment that \
             runs React Server Components."
        );
    }
}

js::export_default!(REACT_SHARED_INTERNALS_SERVER);
