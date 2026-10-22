// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::Unknown;

thread_local! {
    static IS_ARRAY_IMPL: fn(Option<&Unknown>) -> bool = js::array::is_array;
}

js::export_default! {
    fn is_array(a: Option<&Unknown>) -> bool {
        IS_ARRAY_IMPL.with(|is_array_impl| is_array_impl(a))
    }
}
