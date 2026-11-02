# @rust-js-bindings/busboy

rust-js bindings of [busboy](https://github.com/mscdex/busboy) 1.6, the
streaming parser of HTML form data React's Flight server decodes a Node
reply with, as @types/busboy types it: `busboy(config)`, the `Busboy` it
makes, a node `Writable`, and its events, each a type of its own as the
node crate's are (`event::Field`, `event::File`, `event::Finish`, ..).

```rust
let parser = busboy::busboy(&BusboyConfig { headers: Some(request.headers()), ..Default::default() });
parser.on(event::Field, Box::new(|name: &str, value: &str, _: &FieldInfo| { .. }));
```

```js
import busboy from "busboy";

const parser = busboy({ headers: request.headers });
parser.on("field", (name, value, info) => { .. });
```

It lives in react-in-rust's workspace until rust-js-lang/bindings
publishes it, laid out as that repository's `bindings/busboy/`.
