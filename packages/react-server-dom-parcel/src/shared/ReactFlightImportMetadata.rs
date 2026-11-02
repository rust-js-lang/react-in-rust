// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use js::Dict;

// This is the parsed shape of the wire format which is why it is
// condensed to only the essentialy information
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum ImportMetadata {
    WithImportMap(
        (
            /* id */ String,
            /* name */ String,
            /* bundles */ &'static [String],
            /* importMap */ &'static Dict<String>,
        ),
    ),
    Plain(
        (
            /* id */ String,
            /* name */ String,
            /* bundles */ &'static [String],
        ),
    ),
}

pub const ID: usize = 0;
pub const NAME: usize = 1;
pub const BUNDLES: usize = 2;
pub const IMPORT_MAP: usize = 3;
