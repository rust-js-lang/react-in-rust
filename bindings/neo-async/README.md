# @rust-js-bindings/neo-async

rust-js bindings of [neo-async](https://github.com/suguru03/neo-async) 2.6,
the `async` library webpack's ecosystem uses, what React's Flight webpack
plugin runs its resolutions with: `map` and `filter` of a collection, each
item's function given a callback it calls once it's done.

```rust
neo_async::map(&paths, |path, done| done(None, Some(path.len())), |err, lengths| { .. });
```

```js
import asyncLib from "neo-async";

asyncLib.map(paths, (path, done) => done(null, path.length), (err, lengths) => { .. });
```

It lives in react-in-rust's workspace until rust-js-lang/bindings
publishes it, laid out as that repository's `bindings/neo-async/`.
