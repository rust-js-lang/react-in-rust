// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// This is forked in server builds where the default stack frame may be source mapped.

use node::js_error::PrepareStackTrace;

js::export_default! {
    #[cfg_attr(rust_js, rust_js::name = "DefaultPrepareStackTrace")]
    pub const DEFAULT_PREPARE_STACK_TRACE: Option<&'static PrepareStackTrace<String>> = None;
}
