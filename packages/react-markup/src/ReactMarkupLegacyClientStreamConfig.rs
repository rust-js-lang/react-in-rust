// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::Uint8Array;

#[cfg_attr(rust_js, rust_js::untagged)]
pub enum StringDecoder {
    #[cfg_attr(rust_js, rust_js::null)]
    Null,
}

pub fn create_string_decoder(_preserve_byte_order_mark: Option<bool>) -> StringDecoder {
    StringDecoder::Null
}

pub fn read_partial_string_chunk(_decoder: &StringDecoder, _buffer: &Uint8Array) -> String {
    panic!("Not implemented.");
}

pub fn read_final_string_chunk(_decoder: &StringDecoder, _buffer: &Uint8Array) -> String {
    panic!("Not implemented.");
}
