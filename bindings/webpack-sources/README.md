# @rust-js-bindings/webpack-sources

rust-js bindings of [webpack-sources](https://github.com/webpack/webpack-sources)
3's source map helpers, with which React's Flight Node loaders map the
code they add back to the module's original lines: `readMappings`, which
reads a source map's `mappings`, and `createMappingsSerializer`, which
writes them.

```rust
use webpack_sources::helpers::{create_mappings_serializer, read_mappings};

read_mappings(&mappings, &mut |line, column, source, original_line, original_column, name| { .. });
let create_mapping = create_mappings_serializer(None);
```

```js
import readMappings from "webpack-sources/lib/helpers/readMappings.js";
import createMappingsSerializer from "webpack-sources/lib/helpers/createMappingsSerializer.js";

readMappings(mappings, (line, column, source, originalLine, originalColumn, name) => { .. });
const createMapping = createMappingsSerializer();
```

webpack-sources ships no types of these, its `lib/helpers` modules, so
they're written from their JS doc comments. It lives in react-in-rust's
workspace until rust-js-lang/bindings publishes it, laid out as that
repository's `bindings/webpack-sources/`.
