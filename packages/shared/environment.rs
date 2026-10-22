//! React's build globals, which its build replaces and its tests define per
//! run, as `scripts/flow/environment.js` declares them: read where the code
//! runs, `if (__DEV__) {..}`. Bindings alone, so no JS of its own.

unsafe extern "Rust" {
    /// Whether this is a development build.
    pub safe static __DEV__: bool;

    /// Whether this build profiles.
    pub safe static __PROFILE__: bool;

    /// Whether this is the experimental release channel's build.
    pub safe static __EXPERIMENTAL__: bool;
}
