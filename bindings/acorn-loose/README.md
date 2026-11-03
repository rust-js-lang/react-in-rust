# @rust-js-bindings/acorn-loose

rust-js bindings of [acorn-loose](https://github.com/acornjs/acorn/tree/master/acorn-loose)
8.3, acorn's error-tolerant JavaScript parser, with which React's Flight
Node loaders, registers and webpack plugin find a module's directives and
exports:
`parse(input, options)`, acorn's `Options`, and the ESTree `Program` and
nodes it makes, a `Node` tagged by its `type`, of the kinds a program reads,
and `Node::Other` of the rest.

```rust
let program = acorn_loose::parse(source, &Options {
    ecma_version: EcmaVersion::Version(2024),
    source_type: Some(SourceType::Module),
    locations: None,
    on_comment: None,
    preserve_parens: None,
})?;
```

```js
import * as acorn from "acorn-loose";

const program = acorn.parse(source, { ecmaVersion: 2024, sourceType: "module" });
```

acorn-loose ships no types of its own, so these are acorn 8's and
`@types/estree`'s. It lives in react-in-rust's workspace until
rust-js-lang/bindings publishes it, laid out as that repository's
`bindings/acorn-loose/`.
