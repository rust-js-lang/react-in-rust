# ReactFlightReplyServer port: rust-js gaps (rd480–rd489)

Port of `packages/react-server/src/ReactFlightReplyServer.js` to
`ReactFlightReplyServer.rs`, generated on rust-js `9a5b41379`, on
react-in-rust `2aea64b`. Each open gap is marked `// GAP rdNNN` in the
draft. Gates: the server suites (react-markup, react-server-dom-webpack,
-turbopack, -fb, react-server, react-dom-bindings, react-client) 468 / 468;
`-r=stable` react-server, react-server-dom-webpack, react-client 450 / 450.

Snippets are `src/lib.rs` of a crate using `js` and `webapi`; "prints" is
what rust-js writes, "refused" what it says. The refusals and printed forms
quoted were seen building this draft; the snippets are the minimal shape of
the same code, not each built on its own.

## rd480: resolved but for a function's name: a field named by a symbol

rust-js 8ba13da9c (ADR 1051, the user's decision, option A): a struct's
field `#[cfg_attr(rust_js, rust_js::name(SYMBOL))]` is keyed by that symbol.
`ModelReason`'s `response` is `#[rust_js::name(RESPONSE_SYMBOL)]` of the
module's own `thread_local! { static RESPONSE_SYMBOL: &'static Symbol =
js::symbol::new(None); }`, printed upstream's `{id, [RESPONSE_SYMBOL]:
response}` and `chunk.reason[RESPONSE_SYMBOL]`; the async iterable is an
`AsyncIterable` struct whose closure field is
`#[rust_js::name(ASYNC_ITERATOR)]`, printed `{[ASYNC_ITERATOR]() { .. }}`,
and `iterator ? iterable[ASYNC_ITERATOR]() : iterable`, as upstream's.

Left: `FlightIterator.prototype[ASYNC_ITERATOR] = function () { .. }`, a
`function::with_this` closure, is unnamed where upstream's is
`function asyncIterator() { .. }`; and initializeModelChunk reads
`chunk.reason[RESPONSE_SYMBOL]` and `chunk.reason.id` where upstream
destructures, `const {[RESPONSE_SYMBOL]: response, id} = chunk.reason;`
(a pattern of a symbol-named field reads each, ADR 1051's next step).

## rd481: resolved, a chunk inherits `Promise.prototype`

Written as rust-js's classes test has it (rd393, 707419ac9; #448's
refusal is of a prototype replaced after its methods): the class, then

```rust
js::on_load! {
    // We subclass Promise.prototype so that we get other methods like .catch
    js::class::<ReactPromise>().set_prototype(js::object::create(
        Some(js::class::<js::Promise<()>>().prototype()),
        None,
    ));
}
```

prints upstream's `ReactPromise.prototype = Object.create(Promise.prototype);`
and its `then`, `self: &Rc<Self>`, as `Object.defineProperty(ReactPromise.prototype,
'then', {writable: true, enumerable: true, configurable: true, value:
function (resolve, reject) {..}})`. Only the function's name,
`reactPromiseThen`, is gone.

## rd482: a flattened state's arms that call out, and its `&mut` bindings

`#[rust_js::flatten] state: RefCell<ChunkState>` (ADR 1044) refuses a borrow
held while code runs, so every upstream `switch (chunk.status)` whose arms
call a function becomes a momentary read per status (`if (chunk.status ===
'fulfilled') {..}`, `const x = chunk.status === 'fulfilled' ? .. : undefined`).
And a `&mut` binding of a variant's field prints a handle named after the
binding: one variant's, `const value = chunk; value.value ??= [];`; an
or-pattern's (rd127's one arm for `case PENDING: case BLOCKED:`), getter and
setter objects, `const borrowMut = { get value() { return chunk; }, set
value(v) { .. } }; value = { get value() { return value$1.value; }, .. }`.

```rust
pub fn listen(chunk: &ReactPromise, f: std::rc::Rc<dyn Fn()>) {
    if let ChunkState::Pending { value, .. } | ChunkState::Blocked { value, .. } =
        &mut *chunk.state.borrow_mut()
    {
        value.get_or_insert_with(Vec::new).push(f);
    }
}
```

(with `ChunkState`/`ReactPromise` as test/classes-flattened-state.test.ts's,
a `Blocked` variant added) prints the getter/setter objects where upstream
writes `if (chunk.value === null) chunk.value = []; chunk.value.push(f);`.
`value.take()` also writes `chunk.value = null` before the whole-state write
that follows.

## rd483: reading a value of any shape as what it is

No checked narrowing from `&Unknown`, so the port casts (`unsafe js::cast`)
where upstream reads the value as what it tested, or where its Flow casts:
`inspectedValue instanceof ReactPromise` then `inspectedValue.status`; the
metadata object as a `WeakMap<JsObject, ..>` key; `rootArrayContexts.get(value)`
after `isArray`; `ArrayBuffer.isView(value)` then `value.byteLength`
(`&ArrayBufferView`); a byte stream's `controller.enqueue(value)`
(`webapi::ArrayBufferView`); `new Map(model)` and `new Set(model)` of an
array of any values. `scratch.get('file') as any` is a match whose string arm
comes first, so it prints `typeof file !== 'string'` and not `instanceof File`
(which would throw on the one host the fallback is for).

```rust
pub fn status(value: Option<&'static js::Unknown>) -> u32 {
    if js::instance_of(value, js::class::<P>()) {
        let p: &P = unsafe { js::cast(value) }; // no `Kind`/downcast of a class
        return p.n;
    }
    0
}
```

## rd484: a chunk's `reason` read as upstream reads it

Upstream reads any chunk that's neither pending (resolveModelChunk) nor
pending or blocked (triggerErrorOnChunk) as a stream chunk, `const
controller = (chunk as any).reason; controller.enqueueModel(value)`. That
path is reachable from the client: resolveField resolves any chunk the
response has by its id, so a reply that sends field `1` again after chunk 1
initialized calls `enqueueModel` of its array context (a TypeError,
"controller.enqueueModel is not a function"), or of `null` (another
TypeError). So the port does upstream's own read, `js::get(chunk,
"reason")` cast to the controller, and calls through it, printed
`controller.enqueueModel(value)`, so JS throws upstream's TypeError. The
cast is upstream's `as any`; there's no checked form (rd483).

readServerReference's `default: throw chunk.reason;` is `js::throw(js::get(chunk,
"reason"))` of every other status, as upstream's.

getOutlinedModel tests an initialized chunk's reason by
`InitializedReason`'s `#[rust_js::test]`, `typeof reason.error ===
'function'`, where upstream tests `'error' in arrayRootOrController`. The
two agree on every reason an initialized chunk holds: an array context made
by initializeModelChunk or resolveReference (`{count, fork}`, no `error`),
a stream controller made by parseReadableStream or parseAsyncIterable
(whose `error` is a function), or `null` (resolveServerReferenceChunk,
parseTypedArray, the iterator's last step), which both pass. No client
input reaches the field. reportGlobalError's test is upstream's own,
`typeof maybeController.error === 'function'`.

## rd485: `Promise.resolve(thenable)` and `Promise.all([thenable, thenable])`

`js::promise::resolve`/`all` take and give `Promise<T>`; `preloadModule`'s
result and the bound chunk are `Thenable`s (and a `ReactPromise`), so the
port casts both ways. Prints as upstream's.

```rust
use shared::react_types::Thenable;
pub fn both(a: &'static Thenable, b: Option<&'static js::Unknown>) -> &'static Thenable {
    unsafe { js::cast(Some(js::unknown(&js::promise::all(vec![js::cast(Some(js::unknown(a))), js::cast(b)])))) }
}
```

## rd486: calling what JS holds: `value.bind.apply(..)` and `new constructor(buffer)`

`boundArgs.unshift(null); value = value.bind.apply(value, boundArgs);` is
`bound_args.insert(0, js::null())` and `function::apply(bind, Some(&value),
Some(&bound_args))` of the value's own `bind`, read by `js::get` and cast
to a function of any shape (rd483), printed `const bind = value.bind;
value = bind.apply(value, boundArgs);`, so a value without one throws JS's
TypeError as upstream's does. (The draft before wrote `function::bind(f,
null, args)` behind a `typeof value !== 'function'` test, which threw the
port's own Error for an export the client can name.)

`parseTypedArray(.., constructor, ..)` does `constructor === ArrayBuffer ?
buffer : new constructor(buffer)`; `js::new` of a class held is refused:

```rust
pub fn make(c: &'static js::Class, b: &'static js::ArrayBuffer) -> &'static js::Unknown {
    js::new(js::unknown(c), (b,))
}
```

refused: "a class held, an untyped `js::Class`, is made by its constructor's
binding .. not as a value of any shape (ADR 1009)". The port passes each
case's constructor binding as a `fn`, `|buffer| js::unknown(js::int8_array::new_with_buffer(buffer, None, None))`,
so 13 arrows replace upstream's 13 classes and the `=== ArrayBuffer` test is
gone.

## rd487: the draft's `TemporaryReference` is a function

ReactFlightServerTemporaryReferences.rs (the other session's draft) types
`TemporaryReference = dyn Fn() -> ..` and `registerTemporaryReference(set,
object: &TemporaryReference, id)`. The reply server registers any object or
array of the reply, `registerTemporaryReference(response._temporaryReferences,
value, reference)`, so the port casts. Upstream's `TemporaryReference<T>` is
any object; the draft's type is narrower than upstream's.

## rd488: shapes Rust can't write as upstream's

- **`InitializationHandler.reason`** is a thrown value or (not errored) an
  array context. An untagged enum can't tell a struct from any thrown value,
  so the array context is a field of its own, `arrayRoot` (the handler has
  one more key).
- **`{done: true, value: undefined}`**: `js::IteratorResult` isn't
  `js::Defined`, so it can't be a chunk's value (a binding gap), and a
  struct's `None` field is left out (ADR 0280), a `js::Kind::Undefined`
  field too, so a struct prints `{ done: true }`, where `'value' in step`
  is false. The port writes the entries, `js::object::from_entries(vec![
  ("done".to_owned(), Some(js::unknown_of(true))), ("value".to_owned(),
  None)])`, printed `Object.fromEntries([["done", true], ["value",
  undefined]])`: upstream's object, keys and all, not its literal.

  ```rust
  pub fn done() -> &'static js::Unknown {
      js::unknown_of(js::IteratorResult::<Option<&'static js::Unknown>> { done: Some(true), value: None })
  }
  ```
  rustc: `IteratorResult<..>: AnyShape<_>` not satisfied.
- **`new ReadableStream({type: type, start(c) {..}})`**: webapi has a byte
  source (`type: &str`, a `ReadableByteStreamController`) and a default one
  (no `type`, a `ReadableStreamDefaultController`), and no union of the two
  controllers, so the port makes one of two streams by `type`, and keeps
  each controller in a variable of its own, `controller` and
  `byteController`, each call testing `byteController != null`. An untagged
  enum of the two was the first draft: its `enqueue` tested `controller
  instanceof ReadableStreamDefaultController`, a global jest's environment
  lacks, and ReactFlightDOMReply's "should supports streaming ReadableStream
  with objects" threw a ReferenceError (a wrong answer the gates caught).
- **`flightController = { enqueueModel(json) {..}, close(json) {..},
  error(error) {..} }`** whose `enqueue` and `chunk.then(enqueue,
  flightController.error)` read its own `error`: a struct of `Rc<dyn Fn>`s,
  `error` made first (`const error = (error) => {..}; const error$1 = error;
  .. const error$2 = error;`). A call of its `error` is hoisted off its
  object, `const error$1 = controller.error; error$1(error);`, where
  upstream calls `controller.error(error)`: `error` is an `Rc<dyn
  Fn(Option<&Unknown>)>`, the type of a `Listener::Function`, which JS gives
  (`then`'s callbacks), so rust-js calls it with no `this`, as Rust does
  (`enqueueModel` and `close`, of types JS never gives, are method calls). A
  struct of its own alone doesn't hoist (built apart). Every controller's
  `error` is the crate's arrow, which reads no `this`, so results agree;
  only triggerErrorOnChunk's TypeError of a chunk that's no stream would be
  worded `error$1 is not a function`, and that path needs a bundler's
  `preloadModule` thenable to call both its callbacks. (`Rc::new_cyclic`/`Weak` for the
  self-reference made every `dyn Fn` of the crate drop, refused by ADR 0626
  as react_isomorphic's drop nothing.)

## rd489: smaller lowering forms

- **`Some(*value)` of an `Option<&Unknown>` prints its inner `None` as
  `null`**: `chunk.status === 'fulfilled' ? (chunk.value ?? null) : undefined`,
  and the `Some`'s payload read on is that `null`, so `resolve(value)` /
  `enqueue(value)` would be given `null` for a chunk whose value is
  `undefined` (a wrong answer the port avoids by testing the status, then
  reading `chunk.value`):

  ```rust
  use js::Unknown;

  pub fn first(v: Option<&'static Unknown>, f: &dyn Fn(Option<&'static Unknown>)) {
      let o = if v.is_some() { Some(v) } else { Some(None) };
      if let Some(v) = o {
          f(v);
      }
  }
  ```

  built alone on rust-js 9a5b41379, prints `const o = v != null ? (v ??
  null) : null; if (o !== undefined) { f(o); }`, and `first(undefined, f)`
  calls `f(null)`, run in Node: still open.
- **A `catch_unwind` whose closure gives a value is an arrow called at once**:
  `value = (() => { .. return value; })();` inside the `try`.
- **A trailing `None` argument's comment is dropped**: reviveModel's `null,
  // The array context resets when we're entering a non-array` loses both
  (the argument by rd271's form, the comment with it).
- **`Kind::BigInt` is an `i64`**, narrower than JS's bigint (the JS reads
  `Math.abs(Number(value))` of the bigint itself, as upstream).

## Behavior differences left

- **rd480:** a resolved model chunk's `reason` is `{id, response}`, where
  upstream's is `{id, [RESPONSE_SYMBOL]: response}`. The chunk is what
  `getRoot` and a `$@` reference give, so a caller sees `response` among
  `Object.keys(chunk.reason)` while it's resolved but not read (upstream's
  symbol is its module's own, but `Object.getOwnPropertySymbols` finds it).
- **rd480, rd481, rd488: function names.** `chunk.then.name` is `value`
  (an anonymous function in the descriptor), upstream's `reactPromiseThen`;
  `FlightIterator.prototype[ASYNC_ITERATOR].name` is `''`, upstream's
  `asyncIterator`; the iterable's method is an arrow,
  `iterable[ASYNC_ITERATOR] = asyncIterator`, where upstream's is a method.
  None reads `this` or its name.
- **rd488's hoisted `controller.error`** words triggerErrorOnChunk's
  TypeError for a chunk that's no stream `error$1 is not a function`
  (upstream: `controller.error is not a function`). The path needs a
  `preloadModule` thenable that calls both its callbacks: every other
  caller tests the chunk pending or blocked first (reportGlobalError,
  rejectReference, parseTypedArray's reject), or errors a chunk of its own
  still blocked or pending (loadServerReference's, parseAsyncIterable's
  unwritten ones).
- **Declared types:** createResponse's `backingFormData` and
  `arraySizeLimit` are `unknown` in the `.d.ts`, where upstream's are
  `FormData` and `number`: what tells `null` from `undefined` (PORTING).

Paths the port proves unreached, keeping a panic: initializeModelChunk's
`_ => unreachable!()` (each caller tests or makes the chunk `resolved_model`
first: `then`, getOutlinedModel, resolveModelChunk, enqueueModel);
createFileFromHandle's string entry (FormData's `append` of a Blob with a
filename stores a File, by the spec); `$index(path, 0)` (`split` gives one
part at least); a stream's `$unwrap(controller)` (ReadableStream runs
`start` in its constructor, by the spec); `$index(buffer, i)` (each read
tests `i < buffer.length` or that it's the length before pushing).

## Port-side forms (not gaps)

- The `map` functions take upstream's four parameters, `createMap(response,
  model, parentObject, key)`, where upstream declares two: they're one `fn`
  type, `MapModel`.
- `PENDING`..`ERRORED` aren't constants: they're `ChunkState`'s
  `#[rust_js::name]`s, printed as their strings.
- `createErroredChunk` is `new ReactPromise('blocked', null, null)` then
  `chunk.status = 'rejected'; chunk.reason = reason;`, upstream's object
  `{status: 'rejected', value: null, reason}`, where upstream writes `new
  ReactPromise(ERRORED, null, reason)`. ChunkState's `Errored` has no
  `value` (rd488's kind): triggerErrorOnChunk and initializeModelChunk's
  `catch` write only `status` and `reason`, as upstream's do, leaving the
  `value` before; a `#[rust_js::nullable] value` would make each of them
  write `value` too, and `js::null()` needs a field to give it to.
- `initializeModelChunk` reads the chunk's reason before saving the
  handler; the handler is put back by a guard's `Drop`, `try { .. } finally
  { initializingHandler = old ?? null; }`.
- `createResponse` defaults `undefined` alone, as upstream's default
  parameters do: `backingFormData` and `arraySizeLimit` are `Option<&Unknown>`
  tested `js::classify(..)` `Kind::Undefined` (PORTING's rule where null and
  undefined differ), printed `x === undefined ? default : x`. `_arraySizeLimit`
  keeps what's given, `null` too, and bumpArrayCount compares it as JS does,
  `newCount > arraySizeLimit`, or `newCount > 0` of `null`, which is JS's
  `newCount > null`. Their declared types are `unknown`, looser than
  upstream's `FormData` and `number`.
- `key.includes(':')` for `key.indexOf(':') === -1`; `String(i)` for `'' +
  i`; error messages with numbers are template literals (`format!`).
- The upstream `buffer[nextWriteIndex] = chunk` where the index is the
  length is `buffer.push(chunk)`; reads are `$index(buffer, i)` (h03).
- Cross-package imports of `shared` print relative paths
  (`'../../shared/ReactFeatureFlags.js'`), react-client's the bare
  `'react-client/src/ReactFlightClientConfig'` (rd349).
