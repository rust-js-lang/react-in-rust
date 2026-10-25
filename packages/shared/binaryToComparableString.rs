// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::{ArrayBufferView, string, uint8_array};

// Turns a TypedArray or ArrayBuffer into a string that can be used for comparison
// in a Map to see if the bytes are the same.
js::export_default! {
    pub fn binary_to_comparable_string(view: &ArrayBufferView) -> String {
        string::from_char_code(uint8_array::new_with_buffer(
            view.buffer(),
            Some(view.byte_offset()),
            Some(view.byte_length()),
        ))
    }
}
