// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::Uint8Array;
use webapi::{TextDecodeOptions, TextDecoder, TextDecoderOptions, text_decoder};

pub type StringDecoder = TextDecoder;

pub fn create_string_decoder(preserve_byte_order_mark: Option<bool>) -> &'static StringDecoder {
    text_decoder::new(
        Some("utf-8"),
        Some(TextDecoderOptions {
            ignore_bom: Some(preserve_byte_order_mark.unwrap_or(false)),
            ..Default::default()
        }),
    )
}

#[cfg_attr(rust_js, rust_js::name = "decoderOptions")]
static DECODER_OPTIONS: TextDecodeOptions = TextDecodeOptions { stream: Some(true) };

pub fn read_partial_string_chunk(decoder: &StringDecoder, buffer: &Uint8Array) -> String {
    decoder.decode(Some(&buffer), Some(DECODER_OPTIONS))
}

pub fn read_final_string_chunk(decoder: &StringDecoder, buffer: &Uint8Array) -> String {
    decoder.decode(Some(&buffer), None)
}
