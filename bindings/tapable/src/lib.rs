//! Bindings of [tapable](https://github.com/webpack/tapable) 2.2, the hooks
//! webpack's compiler and compilation are made of, as its `tapable.d.ts`
//! types them: a hook of the arguments its taps are called with, a tuple,
//! `SyncHook<(&Compilation, &CompilationParams)>`, which a plugin taps by
//! its name, or by a [`Tap`] that says its stage too; and a [`HookMap`],
//! a hook of each key.
//!
//! ```rust,ignore
//! use tapable::SyncHook;
//!
//! fn listen(hook: &SyncHook<(&'static str,)>) {
//!     hook.tap("My Plugin", |name| println!("{name}"));
//! }
//! ```

// A binding's body never runs, so rustc's crate sees its parameters unread
// (rd330, the user's one crate-level allow for a bindings crate).
#![cfg_attr(not(rust_js), allow(unused_variables))]

use std::marker::PhantomData;
use std::ops::Deref;

use js::{Error, JsObject};

/// What a hook is tapped by, tapable's `string | Tap`: its name, or a
/// [`Tap`].
#[cfg_attr(
    rust_js,
    rust_js::types = "string | { name: string; before?: string; stage?: number }"
)]
pub trait IntoTap {}

impl IntoTap for &str {}
impl IntoTap for &Tap<'_> {}

/// tapable's `Tap`: the tap's name, and where it runs among the others,
/// each `None` left out.
#[derive(Clone, Copy, Default)]
pub struct Tap<'a> {
    pub name: &'a str,
    /// The tap it runs before, by name.
    pub before: Option<&'a str>,
    /// Its stage: lower runs first, 0 by default.
    pub stage: Option<f64>,
}

/// tapable's `Hook<T, R>`, which each hook extends: what's called with
/// the arguments `T`, a tuple, each tap's result an `R`.
pub struct Hook<T, R = ()>(PhantomData<JsObject>, PhantomData<(T, R)>);

impl<A: 'static, R: 'static> Hook<(A,), R> {
    /// [`hook.tap(options, fn)`](https://github.com/webpack/tapable#hook-types):
    /// `f` called with the hook's argument each time it's called.
    #[cfg_attr(rust_js, rust_js::link_name = "tap")]
    pub fn tap(&self, options: impl IntoTap, f: impl Fn(A) -> R + 'static) {
        unreachable!()
    }
}

impl<A: 'static, B: 'static, R: 'static> Hook<(A, B), R> {
    /// [`hook.tap(options, fn)`](https://github.com/webpack/tapable#hook-types):
    /// `f` called with the hook's two arguments each time it's called.
    #[cfg_attr(rust_js, rust_js::link_name = "tap")]
    pub fn tap(&self, options: impl IntoTap, f: impl Fn(A, B) -> R + 'static) {
        unreachable!()
    }
}

/// tapable's `SyncHook<T, R>`: a hook that calls each tap in turn, at once.
#[cfg_attr(rust_js, rust_js::types = "tapable#SyncHook")]
pub struct SyncHook<T, R = ()>(PhantomData<JsObject>, PhantomData<(T, R)>);

impl<T, R> Deref for SyncHook<T, R> {
    type Target = Hook<T, R>;

    fn deref(&self) -> &Hook<T, R> {
        // Never runs: rust-js compiles this `Deref` to the object itself.
        unsafe { &*(self as *const Self as *const Hook<T, R>) }
    }
}

/// tapable's `SyncBailHook<T, R>`: a sync hook that stops at the first tap
/// giving other than `undefined`.
#[cfg_attr(rust_js, rust_js::types = "tapable#SyncBailHook")]
pub struct SyncBailHook<T, R>(PhantomData<JsObject>, PhantomData<(T, R)>);

impl<T, R> Deref for SyncBailHook<T, R> {
    type Target = SyncHook<T, R>;

    fn deref(&self) -> &SyncHook<T, R> {
        // Never runs: rust-js compiles this `Deref` to the object itself.
        unsafe { &*(self as *const Self as *const SyncHook<T, R>) }
    }
}

/// tapable's `AsyncHook<T, R>`, which each async hook extends: a hook its
/// taps may finish later, each given a callback to call once it's done.
pub struct AsyncHook<T, R = ()>(PhantomData<JsObject>, PhantomData<(T, R)>);

impl<T, R> Deref for AsyncHook<T, R> {
    type Target = Hook<T, R>;

    fn deref(&self) -> &Hook<T, R> {
        // Never runs: rust-js compiles this `Deref` to the object itself.
        unsafe { &*(self as *const Self as *const Hook<T, R>) }
    }
}

impl<A: 'static> AsyncHook<(A,)> {
    /// [`hook.tapAsync(options, fn)`](https://github.com/webpack/tapable#hook-types):
    /// `f` called with the hook's argument and the callback it calls once
    /// it's done, given what failed, if anything, tapable's `InnerCallback`.
    #[cfg_attr(rust_js, rust_js::link_name = "tapAsync")]
    pub fn tap_async(
        &self,
        options: impl IntoTap,
        f: impl Fn(A, &'static dyn Fn(Option<&'static Error>)) + 'static,
    ) {
        unreachable!()
    }
}

/// tapable's `AsyncSeriesHook<T>`: an async hook that calls each tap
/// after the one before it is done.
#[cfg_attr(rust_js, rust_js::types = "tapable#AsyncSeriesHook")]
pub struct AsyncSeriesHook<T>(PhantomData<JsObject>, PhantomData<T>);

impl<T> Deref for AsyncSeriesHook<T> {
    type Target = AsyncHook<T>;

    fn deref(&self) -> &AsyncHook<T> {
        // Never runs: rust-js compiles this `Deref` to the object itself.
        unsafe { &*(self as *const Self as *const AsyncHook<T>) }
    }
}

/// tapable's `AsyncParallelHook<T>`: an async hook that calls each tap at
/// once, done when all are.
#[cfg_attr(rust_js, rust_js::types = "tapable#AsyncParallelHook")]
pub struct AsyncParallelHook<T>(PhantomData<JsObject>, PhantomData<T>);

impl<T> Deref for AsyncParallelHook<T> {
    type Target = AsyncHook<T>;

    fn deref(&self) -> &AsyncHook<T> {
        // Never runs: rust-js compiles this `Deref` to the object itself.
        unsafe { &*(self as *const Self as *const AsyncHook<T>) }
    }
}

/// tapable's `HookMap<H>`: a hook of each key, made as it's first asked for.
#[cfg_attr(rust_js, rust_js::types = "tapable#HookMap")]
pub struct HookMap<H>(PhantomData<JsObject>, PhantomData<H>);

impl<H> HookMap<H> {
    /// `hookMap.for(key)`: the hook of `key`, made if it's the first time.
    #[cfg_attr(rust_js, rust_js::link_name = "for")]
    pub fn r#for(&self, key: &str) -> &'static H {
        unreachable!()
    }
}
