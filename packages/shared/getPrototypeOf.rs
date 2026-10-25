// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::Unknown;

thread_local! {
    pub static GET_PROTOTYPE_OF: fn(&Unknown) -> Option<&'static Unknown> = js::object::get_prototype_of;
}

js::export_default!(GET_PROTOTYPE_OF);
