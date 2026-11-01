# @rust-js-bindings/art

rust-js bindings of [art](https://github.com/sebmarkbage/art) 0.10, the
vector graphics library React ART draws with: its `Transform`
(`art/core/transform`), its current mode (`art/modes/current`) and the
nodes a mode makes, and the `fast-noSideEffects` mode.

art has no TypeScript types, so the bindings are written from its JS, and
its values are `any` in the `.d.ts` rust-js writes.

```rust
let transform = art::transform::new();
transform.transform_to(1.0, 0.0, 0.0, 1.0, 0.0, 0.0).r#move(10.0, 20.0);
```

```js
import Transform from "art/core/transform";

const transform = new Transform();
transform.transformTo(1, 0, 0, 1, 0, 0).move(10, 20);
```

It lives in react-in-rust's workspace until rust-js-lang/bindings
publishes it, laid out as that repository's `bindings/art/`.
