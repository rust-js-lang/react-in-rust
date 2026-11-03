//! Bindings of [webpack-sources](https://github.com/webpack/webpack-sources)
//! 3: its source map helpers, [`helpers`], which read and write a source
//! map's `mappings`.
//!
//! webpack-sources ships no types of these, so they're written from their
//! JS doc comments: each number is JS's, a line from 1 and a column from 0,
//! and an index that a mapping has none of `-1`.

// A binding's body never runs, so rustc's crate sees its parameters unread
// (rd330, the user's one crate-level allow for a bindings crate).
#![cfg_attr(not(rust_js), allow(unused_variables))]

/// `webpack-sources/lib/helpers/*`: one function each module exports.
pub mod helpers {
    /// What [`create_mappings_serializer`] makes: of a mapping, its
    /// generated line and column, its source's index, its original line and
    /// column and its name's index, each index `-1` of none, the text of
    /// `mappings` that adds it after the last it was given.
    pub type MappingSerializer = dyn Fn(f64, f64, f64, f64, f64, f64) -> String;

    /// `readMappings(mappings, onMapping)`, of
    /// `webpack-sources/lib/helpers/readMappings.js`: each mapping of
    /// `mappings` given to `on_mapping` in order, before it returns, as
    /// its generated line and column, its source's index, its original line
    /// and column and its name's index, each index `-1` of none.
    #[cfg_attr(
        rust_js,
        rust_js::link_name = "webpack-sources/lib/helpers/readMappings.js#default"
    )]
    pub fn read_mappings(mappings: &str, on_mapping: &mut dyn FnMut(f64, f64, f64, f64, f64, f64)) {
        unreachable!()
    }

    /// `createMappingsSerializer(options)`, of
    /// `webpack-sources/lib/helpers/createMappingsSerializer.js`: a
    /// [`MappingSerializer`], of lines only where `options`' `columns` is
    /// `false`.
    #[cfg_attr(
        rust_js,
        rust_js::link_name = "webpack-sources/lib/helpers/createMappingsSerializer.js#default"
    )]
    pub fn create_mappings_serializer(
        options: Option<&SerializerOptions>,
    ) -> &'static MappingSerializer {
        unreachable!()
    }

    /// What [`create_mappings_serializer`] takes.
    #[derive(Clone, Copy, Default)]
    pub struct SerializerOptions {
        /// `false` of a serializer of lines only, with no columns.
        pub columns: Option<bool>,
    }
}
