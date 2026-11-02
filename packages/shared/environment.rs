//! React's build globals, which its build replaces and its tests define per
//! run, and the bundlers' runtime globals, as `scripts/flow/environment.js`
//! declares them: read where the code runs, `if (__DEV__) {..}`. Bindings
//! alone, so no JS of its own.

use js::{Promise, Unknown};

unsafe extern "Rust" {
    /// Whether this is a development build.
    pub safe static __DEV__: bool;

    /// Whether this build profiles.
    pub safe static __PROFILE__: bool;

    /// Whether this is the experimental release channel's build.
    pub safe static __EXPERIMENTAL__: bool;

    /// Webpack's chunk filename function, `__webpack_require__.u`.
    #[link_name = "__webpack_require__.u"]
    pub safe static WEBPACK_REQUIRE_U: &'static dyn Fn(&str) -> String;
}

/// Webpack's `__webpack_chunk_load__(id)`.
#[cfg_attr(rust_js, rust_js::link_name = "__webpack_chunk_load__")]
pub fn webpack_chunk_load(id: &str) -> Promise<Option<&'static Unknown>> {
    unreachable!()
}

/// Webpack's `__webpack_get_script_filename__(id)`.
#[cfg_attr(rust_js, rust_js::link_name = "__webpack_get_script_filename__")]
pub fn webpack_get_script_filename(id: &str) -> String {
    unreachable!()
}

/// Webpack's `__webpack_require__(id)`.
#[cfg_attr(rust_js, rust_js::link_name = "__webpack_require__")]
pub fn webpack_require(id: &str) -> &'static Unknown {
    unreachable!()
}

/// `__webpack_require__.u = u`, Webpack's chunk filename function replaced.
#[cfg_attr(rust_js, rust_js::link_name = "set __webpack_require__.u")]
pub fn set_webpack_require_u(u: impl Fn(&str) -> String + 'static) {
    unreachable!()
}

/// Turbopack's `__turbopack_load_by_url__(id)`, of a chunk: a plain chunk
/// filename, or a merged chunk, `[mergedChunkFilename,
/// componentChunkFilenames, componentChunkSizes]`.
#[cfg_attr(rust_js, rust_js::link_name = "__turbopack_load_by_url__")]
pub fn turbopack_load_by_url<C>(id: &C) -> Promise<Option<&'static Unknown>> {
    unreachable!()
}

/// Turbopack's `__turbopack_require__(id)`.
#[cfg_attr(rust_js, rust_js::link_name = "__turbopack_require__")]
pub fn turbopack_require(id: &str) -> &'static Unknown {
    unreachable!()
}
