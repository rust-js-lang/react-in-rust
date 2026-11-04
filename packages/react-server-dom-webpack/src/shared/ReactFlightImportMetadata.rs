// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::fmt;

/// A webpack module's id: its name, or its number, as webpack's production
/// builds give it.
#[derive(Clone, PartialEq, Eq, Hash)]
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum ModuleId {
    Name(String),
    Number(u32),
}

// A module's id names its exports in a bundler config, `bundlerConfig[id]`.
impl js::PropertyKey for ModuleId {}

impl fmt::Display for ModuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModuleId::Name(name) => write!(f, "{name}"),
            ModuleId::Number(number) => write!(f, "{number}"),
        }
    }
}

pub struct ImportManifestEntry {
    pub id: ModuleId,
    // chunks is a double indexed array of chunkId / chunkFilename pairs
    pub chunks: &'static [String],
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
            /* id */ ModuleId,
            /* chunks id/filename pairs, double indexed */ &'static [String],
            /* name */ String,
            /* async */ Async,
        ),
    ),
    Sync(
        (
            /* id */ ModuleId,
            /* chunks id/filename pairs, double indexed */ &'static [String],
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
