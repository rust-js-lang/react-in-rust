# @rust-js-bindings/bun

rust-js bindings of [Bun](https://bun.sh)'s own APIs, as bun-types 1.4
types them: `Bun.hash`, and Bun's direct `ReadableStream`, a
`DirectUnderlyingSource` whose `pull` is given a
`ReadableStreamDirectController` that writes straight to the stream's
destination. React's Fizz server for Bun streams its HTML with them, and
hashes useActionState's key paths with `Bun.hash`.

```rust
let stream = webapi::readable_stream::new(
    Some(&DirectUnderlyingSource {
        r#type: "direct",
        pull: &|controller: &'static ReadableStreamDirectController| {
            controller.write("Hello");
            controller.end();
        },
        cancel: None,
    }),
    Some(QueuingStrategy { high_water_mark: Some(2048.0) }),
);
let key = bun::hash("key", None);
```

```js
const stream = new ReadableStream(
  {
    type: "direct",
    pull(controller) {
      controller.write("Hello");
      controller.end();
    },
  },
  { highWaterMark: 2048 },
);
const key = Bun.hash("key");
```

It binds only these, not the rest of bun-types. It lives in
react-in-rust's workspace until rust-js-lang/bindings publishes it, laid
out as that repository's `bindings/bun/`.
