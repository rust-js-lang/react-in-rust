# @rust-js-bindings/acorn-loose

rust-js bindings of [acorn-loose](https://github.com/acornjs/acorn/tree/master/acorn-loose)
8.3, acorn's error-tolerant parser, what React's Flight webpack plugin
finds a module's `"use client"` directive with: `parse`, and the program's
statements it gives, as acorn's ESTree types them.

```rust
let program = acorn_loose::parse(source, &Options { ecma_version: 2024, source_type: Some("module") });
```

```js
import * as acorn from "acorn-loose";

const program = acorn.parse(source, { ecmaVersion: 2024, sourceType: "module" });
```

It lives in react-in-rust's workspace until rust-js-lang/bindings
publishes it, laid out as that repository's `bindings/acorn-loose/`.
