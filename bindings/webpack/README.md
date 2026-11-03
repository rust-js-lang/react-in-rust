# @rust-js-bindings/webpack

rust-js bindings of [webpack](https://webpack.js.org) 5.82, what a plugin
of its compiler uses, as webpack's own `types.d.ts` types it: the
`Compiler` a plugin's `apply` is given, its hooks (tapable's, of
@rust-js-bindings/tapable) and the `Compilation` each makes, its chunks,
modules and assets, and `ModuleDependency`, the class a plugin's
dependency extends.

```rust
compiler.hooks().make().tap("My Plugin", |compilation| { .. });
```

```js
compiler.hooks.make.tap("My Plugin", (compilation) => { .. });
```

It lives in react-in-rust's workspace until rust-js-lang/bindings
publishes it, laid out as that repository's `bindings/webpack/`.
