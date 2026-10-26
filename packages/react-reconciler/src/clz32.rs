// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// TODO: This is pretty well supported by browsers. Maybe we can drop it.

thread_local! {
    pub static CLZ32: fn(u32) -> u32 = if js::is_defined(u32::leading_zeros) {
        u32::leading_zeros
    } else {
        clz32_fallback
    };
}

// Count leading zeros.
// Based on:
// https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Math/clz32
#[cfg_attr(rust_js, rust_js::name = "log")]
const LOG: fn(f64) -> f64 = f64::ln;
const LN2: f64 = std::f64::consts::LN_2;
fn clz32_fallback(x: u32) -> u32 {
    let as_uint = x;
    if as_uint == 0 {
        return 32;
    }
    31 - (LOG(as_uint as f64) / LN2) as u32
}
