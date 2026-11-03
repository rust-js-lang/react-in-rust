# @rust-js-bindings/tapable

rust-js bindings of [tapable](https://github.com/webpack/tapable) 2.2, the
hooks webpack's compiler and compilation are made of, as its own
`tapable.d.ts` types them: a hook of the arguments it calls its taps with,
`SyncHook<(A, B)>`, tapped by name or by a [`Tap`], and a `HookMap` of
hooks by key.

```rust
compiler.hooks().this_compilation().tap("My Plugin", |compilation, params| { .. });
```

```js
compiler.hooks.thisCompilation.tap("My Plugin", (compilation, params) => { .. });
```

It lives in react-in-rust's workspace until rust-js-lang/bindings
publishes it, laid out as that repository's `bindings/tapable/`.
