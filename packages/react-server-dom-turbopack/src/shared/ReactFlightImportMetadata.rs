// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// A chunk is either a plain chunk filename, or a merged chunk that bundles
// several component chunks together, emitted as
// `[mergedChunkFilename, componentChunkFilenames, componentChunkSizes]`.
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum Chunk {
    File(String),
    Merged(
        (
            /* merged chunk filename */ String,
            /* component chunk filenames */ &'static [String],
            /* component chunk sizes */ &'static [f64],
        ),
    ),
}

pub struct ImportManifestEntry {
    pub id: String,
    pub chunks: &'static [Chunk],
    pub name: String,
    pub r#async: Option<bool>,
}

/// The 4th tuple member of an async module's metadata, `1`.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Async {
    Async = 1,
}

// This is the parsed shape of the wire format which is why it is
// condensed to only the essentialy information
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum ImportMetadata {
    Async(
        (
            /* id */ String,
            /* chunks */ &'static [Chunk],
            /* name */ String,
            /* async */ Async,
        ),
    ),
    Sync(
        (
            /* id */ String,
            /* chunks */ &'static [Chunk],
            /* name */ String,
        ),
    ),
}

pub const ID: usize = 0;
pub const CHUNKS: usize = 1;
pub const NAME: usize = 2;
// export const ASYNC = 3;

// This logic is correct because currently only include the 4th tuple member
// when the module is async. If that changes we will need to actually assert
// the value is true. We don't index into the 4th slot because flow does not
// like the potential out of bounds access
pub fn is_async_import(metadata: &ImportMetadata) -> bool {
    matches!(metadata, ImportMetadata::Async(_))
}
