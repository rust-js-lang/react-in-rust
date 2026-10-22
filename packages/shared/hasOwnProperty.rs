// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::object::HasOwnProperty;

thread_local! {
    pub(crate) static HAS_OWN_PROPERTY: &'static HasOwnProperty = js::object::has_own_property;
}

js::export_default!(HAS_OWN_PROPERTY);
