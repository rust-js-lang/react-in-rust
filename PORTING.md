# React, in Rust

This repository is [react/react](https://github.com/react/react) being
ported to Rust, piece by piece, with [rust-js](https://github.com/rust-js-lang/rust-js):
each JavaScript file is replaced by a Rust module that rust-js compiles to
the JavaScript beside it, which should read as the original does. React's
own build and tests run on that JavaScript as they ran on the original.

The tree started as react/react at
[`ab0d927`](https://github.com/react/react/commit/ab0d927f53f6e1b5dcfc2c4e9fc438bdb5ffb678)
(2026-10-20). What isn't Rust yet is still upstream's JavaScript.

## Setting up

rust-js is built from source, in a checkout beside this one:

```sh
git clone https://github.com/rust-js-lang/rust-js ../rust-js
(cd ../rust-js && bun install && cargo build)
yarn install --ignore-scripts
```

`../rust-js` is where this repository's Cargo workspace finds rust-js's
crates (`Cargo.toml`), and its `package.json` the runtime packages the
generated JavaScript imports.

## The loop

```sh
yarn rust --package scheduler              # compile a package's Rust to its JS
yarn test packages/scheduler               # React's tests, on that JS
```

A package's Rust is a crate in the workspace (`Cargo.toml`), whose modules
are its files: `src/SchedulerMinHeap.rs` is compiled to
`src/SchedulerMinHeap.js` beside it. The generated JS is committed, as
upstream's was, so the build and tests need no compiler.

## The rules

- **The Rust is idiomatic Rust.** Nothing is bent to get past rust-js.
- **The JS reads as the original.** Every difference from upstream's file
  (a rename, a helper, a cast, a lost comment) is a gap in rust-js, fixed
  there before the port goes on.
- **React's tests pass** on the generated JS, as they did on the original.
