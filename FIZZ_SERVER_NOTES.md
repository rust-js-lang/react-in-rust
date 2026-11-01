# ReactFizzServer port: notes

Branch `fizz-server`. The draft is `packages/react-server/src/ReactFizzServer.rs`,
declared in `server.rs` so clippy checks it. **Don't build react-server from
this branch until the draft is whole**: a build overwrites upstream's
`ReactFizzServer.js` with a partial module. The generated JS is not committed.

Check: `cargo fmt -p react-server` and
`node ../rust-js/tooling/cli.js clippy --package react-server -- -D warnings`:
clean at the branch's head, the draft whole (no stubs left).

While this was drafted, another session had uncommitted changes in the
shared `../rust-js` (`js::error::new` taking `Option<&str>`) that break
react-isomorphic, so clippy was run against rust-js HEAD d02873fe1's
`builtins`, `webapi`, `node` and `react` crates exported with `git archive`
to a scratch directory, the worktree's `Cargo.toml` and
`packages/react-art/Cargo.toml` pointed there only while checking (not
committed).

**`rust-js build --package react-server` is refused** (rd495 below), so no
JS has been generated or compared yet.

## Representation

- **Request, Task, Segment, SuspenseBoundary, SuspenseListRow, ReplaySet,
  Preamble, PostponedHoles** are Rust structs held by `Rc`, their mutable
  fields `Cell`/`RefCell`, as the reconciler's Fiber is. Task and KeyNode
  compare and hash by identity (`ptr::eq`/`ptr::hash`), as Fiber does, so a
  `HashSet<Rc<Task>>` is upstream's `Set<Task>` and a `HashMap<Rc<KeyNode>,
  ReplayNode>` its `workingMap`. `clippy.toml` lists both in
  `ignore-interior-mutability`, and its argument threshold is 17
  (createRenderTask's).
- **A task's set** is `AbortSet = Rc<RefCell<HashSet<Rc<Task>>>>`, shared.
- **The config's types** (ReactFizzConfig's RenderState, ResumableState,
  PreambleState, FormatContext, HoistableState; ReactServerStreamConfig's
  Destination, Chunk, PrecomputedChunk) are aliases of `Unknown`, as
  shared::react_types' ReactNodeList is, held `&'static`: the config is a
  shimmed module (`./src/ReactFizzConfig.js#..`, as ReactFizzHooks reaches
  it). A segment's chunks are `Chunks = RefCell<Vec<&'static Chunk>>`, the
  config DOM's `Chunks` shape.
- **Callbacks** are `Rc<dyn Fn ..>` (rd308's form for the reconciler's error
  handlers): `on_error: Rc<OnError>` etc., `None` defaulting as upstream does.
- **ThrownInfo/ErrorInfo** stays a JS object (its `componentStack` is a lazy
  getter defined with `Object.defineProperty`), an opaque type with a getter.
- **KeyNode** is a tuple struct `[parent, name, key]`, `NodeKey` an untagged
  `Key(String) | Index(f64)`.

## Migration of ReactFizzCurrentTask's bindings (planned; phase A done)

Phase A (done in this branch, react-server only):
- `ReactFizzCurrentTask`'s opaque `Task` is gone: it imports
  `react_fizz_server::Task`; `CURRENT_TASK_IN_DEV` is
  `RefCell<Option<Rc<Task>>>`, `set_current_task_in_dev(Option<Rc<Task>>)`.
- Its opaque `KeyNode` is gone: ReactFizzHooks uses `react_fizz_server::KeyNode`
  (`CURRENTLY_RENDERING_KEY_PATH: RefCell<Option<Rc<KeyNode>>>`,
  createPostbackActionStateKey takes `Option<&KeyNode>`).
- ReactFizzHooks' `get_form_state` binding (`./src/ReactFizzServer.js#getFormState`)
  is a `use` of the Rust function; its Task/Request thread-locals hold `Rc`s,
  and `prepare_to_use_hooks(&Rc<Request>, &Rc<Task>, Option<Rc<KeyNode>>, ..)`.
- ReactFizzThenable and ReactFizzAsyncDispatcher read
  `task.component_stack.borrow()` / `task.debug_task.get()`;
  AsyncDispatcher's `GetOwner` gives `Option<Rc<ComponentStackNode>>`.
- ReactFizzViewTransitionComponent's opaque `ResumableState` is gone; it and
  ReactFizzHooks use `react_fizz_server::ResumableState`.
- ReactFizzComponentStack: `ComponentStackNode` is a `WeakKey` (the request's
  `didWarnForKey`), and `get_owner_stack_by_component_stack_node_in_dev`
  takes `&ComponentStackNode` of any lifetime.

Phase B (when the draft is whole, in the commit that builds it):
1. Delete from ReactFizzCurrentTask the opaque `Request`, `PostponedState<S>`,
   `ErrorInfo`, `OnError`, `OnBrowserBailout` and every
   `react-server/src/ReactFizzServer#..` binding (createRequest,
   resumeRequest, createPrerenderRequest, resumeAndPrerenderRequest,
   getPostponedState, prepareForStartFlowingIfBeforeAllReady, startWork,
   startFlowing, stopFlowing, abort, attachAbortSignal). The module is then
   upstream's: `currentTaskInDEV` and `setCurrentTaskInDEV` only.
2. react-dom's server entries (`packages/react-dom/src/server/ReactDOMFizz*.rs`,
   `ReactDOMLegacyServerImpl.rs`) `use react_server::react_fizz_server::{..}`.
   Their changes: the request is `Rc<Request>` (their
   `Cell<Option<&'static Request>>` becomes a `RefCell<Option<Rc<Request>>>`
   or `OnceCell`, which the stream's closures capture by `Rc`); the
   callbacks they pass are `Rc::new(..)` of move closures, where they now
   pass `&closure`s of the stack; their config states are passed as
   `js::unknown(&resumable_state)` etc. (or the entries hold them `Rc` and
   pass the reference), as react-server sees the config's types as
   `Unknown`; `get_postponed_state` gives `Option<Rc<PostponedState>>`.
3. react-dom-bindings' ReactFizzConfigDOM: its four
   `react-server/src/ReactFizzServer#..` bindings (resolveRequest,
   getResumableState, getRenderState, flushResources) become `use`s of
   `react_server::react_fizz_server::{resolve_request, get_resumable_state,
   get_render_state, flush_resources}`; `Request` is the server's, held as
   `Rc<Request>`, and the states come back as `&'static Unknown`, which
   ConfigDOM casts to its own `ResumableState`/`RenderState` (or the getters
   become generic). This needs the coordinator: ConfigDOM's JS imports
   ReactFizzServer by its package path, which stays as it is.
4. react-noop-renderer's ReactNoopServer reaches `react-server#default`, the
   `$$$config` factory; it stays JS (the npm build wraps the module), so
   nothing there moves.

## GAPs (rd490-rd499)

Resolved:
- **rd490** replay nodes: fixed in rust-js 9b4defe30 (ADR 1046). `ReplayNode`
  is an untagged enum of `ReplayElement` and `ReplaySuspenseBoundary`, held
  `Rc<RefCell<ReplayNode>>`, grown and shrunk by `*node = ..`.
- **rd492** `AsyncLocalStorage`: node's `async_hooks` binding (rust-js
  0e1cea0b0) is used for `requestStorage`.
- **rd495** the dyn-Fn drop refusal: **the user's decision (2026-11-01),
  "Pings made at then()"**: a task has no `ping` field; `ping_of(request,
  task)` makes the resolve and reject closures where they're given to
  `then`, each holding the request and the task strongly, as upstream's
  arrows do. The task object has no `ping` property (accepted). This also
  removes the draft's `Weak::upgrade().unwrap()`, which would have
  panicked once a task was gone.
- **rd496** getCurrentStack: fixed in react-in-rust 4cff83c; what was read
  is put back.

Open:
- **rd491** `noop` is no callback of one or two parameters:
  `on_shell_error.unwrap_or_else(|| Rc::new(|_| {}))` prints an arrow where
  upstream writes `noop`. Repro: `fn f(cb: Option<Rc<dyn Fn(f64)>>) -> Rc<dyn Fn(f64)> { cb.unwrap_or_else(|| Rc::new(|_| {})) }`.
- **rd493** a field set only under a condition (a task's `legacyContext`
  and `debugTask`, a boundary's DEV error fields): always written. Repro:
  `struct T { #[rust_js::optional] d: Cell<Option<f64>> }` made with `None`
  in production, where upstream never sets `t.d`.
- **rd494** `new RequestInstance(..)`: the request is an object literal
  (`requestInstance(..)` returns `{..}`).
- **rd497** node's `AsyncLocalStorage<T>::run` takes a `&'static T`; the
  request an `Rc` holds is given by `unsafe { js::cast(Some(js::unknown(&**request))) }`
  (`request_store`). Upstream's `requestStorage.run(request, performWork,
  request)` prints `requestStorage.run(requestStore(request), () =>
  performWork(request))`. Repro: `fn f(s: &AsyncLocalStorage<Rc<R>>, r: &Rc<R>) { s.run(r, || ()) }`
  doesn't borrow-check.
- **rd498** an `extern "Rust"` static with no `#[link_name]` (the draft lost
  one in an edit) compiles and prints its Rust name, `REQUEST_STORAGE`,
  unimported: a ReferenceError at run time (852 test failures before the
  fix). rust-js should refuse it. Repro: `unsafe extern "Rust" { safe static X: bool; } pub fn f() -> bool { X }`.
- **rd499** labels in the generated JS (the gates' check wants none):
  - flushSubtree: `for next_child in children.iter() { while (chunk_idx as f64) < next_child.index { ..; chunk_idx += 1; } r = flush_segment(..); }`
    prints `bb28: while (..) { while (true) { if (!(..)) { r = ..; continue bb28; } .. } }`.
    Repro: `for x in v { while i < x { i += 1; } g(); }`.
  - renderNode (`bb368`) and flushCompletedQueues (`bb97`): a `let .. else
    { return; }` inside a `match` arm or a `catch_unwind` closure, with code
    after the match, prints a labeled block broken out of.

## GAPs (rd570-rd579)

- **rd570** (P1, wrong answer; a regression of rust-js 1d159bcd0, which
  0c233f18a had fixed once): a module's imports are sorted by specifier
  (ADR 0295's one block), so JS loads them in that order, not the `use`s'.
  React DOM's server entries import `react-server/src/ReactFizzServer`
  before `react-dom-bindings/src/server/ReactFizzConfigDOM`, and jest's
  module cycle depends on it (entry -> ConfigDOM -> ReactFizzServer ->
  ReactFizzHooks -> ReactFizzConfig -> ConfigDOM, half-loaded, so
  `supportsClientAPIs` is `undefined` when `HooksDispatcher` is made and
  every `useState` throws "Cannot use state or effect Hooks in
  renderToHTML"). Regenerated on rust-js 090616b60, all eight entries
  (FizzServer Browser/Edge/Node, FizzStatic Browser/Edge/Node,
  LegacyServerImpl) print ConfigDOM first: react-dom 4153/111/2 and the
  server set 470/1 (turbopack's DOM object references). With the
  ReactFizzServer import moved back above by hand (test only, never
  committed) the gate is 4264/1/2. No Rust can order them: rustfmt sorts a
  group's `use`s, and after Phase B both are a dependency crate's subpaths,
  `react-dom-bindings/..` sorting before `react-server/..`. Repro (one
  crate):
  ```rust
  pub mod bindings {
      #[cfg_attr(rust_js, rust_js::link_name = "zeta/Server#z")]
      pub fn z() {}
      #[cfg_attr(rust_js, rust_js::link_name = "alpha/Config#a")]
      pub fn a() {}
  }
  // zeta/Server must load first: alpha/Config reads what it sets as it loads.
  use bindings::z;

  use bindings::a;

  pub fn f() { z(); a(); }
  ```
  prints `import { a } from "alpha/Config";` before `import { z } from
  "zeta/Server";`. Suggested: keep the `use`s' order (each rustfmt group's
  order, groups as written), or never reorder imports across modules
  that aren't side-effect free. The entries' JS stays main's until it's
  fixed.

## Deep-tree resume (ReactDOMFizzServer-test, "can recover from very deep trees during resume")

**Cause:** the resume replays 1200 nested `Deep` elements, each through
replayElement's catch. The stack overflows deep inside; at the innermost
replayElement whose element threw, the catch must pop the replay set it
pushed (`task.replay = replay`) before rethrowing, so the retried task
replays from the right set. Upstream's handler does only property reads
before that write. The port's ran calls first (`thrownValueOf`,
`isSuspending`), and the generated catch's own prologue,
`const x = $unwound(thrown)` (which calls `$isPanic`), runs before any of
the program's code. With the stack exhausted one of those overflows again,
the handler throws a fresh RangeError, and `task.replay` stays the child's
set: "Expected the resume to render <Suspense> .. rendered <Deep>".

**Port fix (in the Rust):** the same-node pop is the handler's first act,
before `thrown_value_of`/`is_suspending` (semantics unchanged: every path
that pops it still does, the different-node suspend path still splices).
Alone this isn't enough: the test then fails with the content missing.

**Compiler gap (needs an id, the coordinator's):** a `catch_unwind`'s
`Err` arm is lowered as `catch (thrown) { const x = $unwound(thrown); ..;
throw $unwinding(x); }`, calls before the program's first statement and at
its rethrow, so no Rust can write state first while the stack is exhausted.
Moving the pop above `$unwound` by hand in the JS makes both deep-tree tests
pass (checked). Repro:
`fn f(d: u32, s: &Cell<u32>) { if d == 0 { return overflow(); } if let Err(e) = catch_unwind(AssertUnwindSafe(|| f(d - 1, s))) { s.set(d); resume_unwind(e); } }`
prints `catch (thrown) { const e = $unwound(thrown); s.value = d; throw $unwinding(e); }`;
upstream's form is `catch (x) { s = d; throw x; }`. Suggested: compute the
payload only where it's read, and rethrow an untouched payload as `throw thrown`.

## Output differences expected (not built yet; from the Rust's forms)

- `Rc::new_cyclic` + `Weak::upgrade().unwrap()` in the task's ping closures
  (rd96's form), where upstream's arrows close over `const task`.
- `then(move |_| resolve(), move |error| reject(error))` of the ping's
  `Rc`s where upstream writes `then(ping.resolve, ping.reject)` (rd491).
- Counters are `f64` cells: `x.set(x.get() + 1.0)` / `- 1.0` and a test
  after, where upstream writes `x++` and `--x === 0`.
- `try {} catch {} finally {}` is `catch_unwind`, the finally's statements,
  then `resume_unwind`; a `return` inside the `try` is the closure's `bool`
  (renderSuspenseBoundary, replaySuspenseBoundary); a catch that itself
  may throw is a second `catch_unwind` (retryRenderTask, retryReplayTask,
  performWork). What renderNode rethrows is the caught payload, or the
  suspended thenable `js::throw`n where SuspenseException was caught.
- `debugTask.run(replayElement.bind(null, ..))` and `onError.bind(null, ..)`
  are closures, `debugTask.run(() => replayElement(..))`.
- A Set's `forEach(f)` / `forEach(abortTaskSoft, request)` is a `for`
  loop over the set's borrow; `pingedTasks.splice(0, i)` and friends are
  `drain(0..i)`; `segment.children.length = n` is `truncate(n)`;
  `nodes.length = 0` and `completedSegments.length = 0` are `clear()`.
- A resume slot's index is the `Dict` key of its text,
  `js::dict::get(slots, &i.to_string())` / `js::delete(slots, ..)`,
  where upstream indexes by the number.
- Port-side helpers upstream doesn't have: `thrown_value_of` (a catch's JS
  value), `fallback_key_path_of`, `is_backwards`, `is_suspending`,
  `is_suspense_exception`, `is_current_task`, `next_segment_id`
  (`request.nextSegmentId++`), `new_replay_node`, `new_thrown_info`
  (`const errorInfo = {}`), `component_name_or_unknown`
  (`getComponentNameFromType(C) || 'Unknown'`), and `impl` of none.
- `renderElement`'s symbol switch is a `match`; ViewTransition falls
  through to Scope as one arm (rd127); preparePreambleFromSegment's
  POSTPONED → CLIENT_RENDERED → default fallthrough is one `_` arm with
  the status tests; flushSubtree's PENDING → POSTPONED is one arm.
- pingTask and startWork: `if (isWorkLoopExternallyDriven) return;` then
  the rest, where upstream nests it in an `else` (clippy's
  `needless_return`).
- encodeErrorForBoundary and getPostponedState: rd45's two lets of a
  tuple.
- logRecoverableError classifies the digest in a match.
- defaultErrorHandler reads `error.environmentName` once (a let-chain).
- The request is an object literal (rd494), a task's `legacyContext` and
  `debugTask` and a boundary's DEV error fields always written (rd493).
- renderSuspenseList's sync-iterator branch casts `children` to the rows
  it renders, as upstream passes `children`, not the `rows` it read.

## Where the draft stands

The whole module is drafted, in upstream's order, and clippy-clean.
Phase A of the migration is done. Next:

1. Get rd495 decided (or restructure the pings and callbacks so no `dyn Fn`
   holds an `Rc`, e.g. `function::bind` of a plain function, rd209), then
   `rust-js build --package react-server` and compare
   `scripts/port/fndiff.sh react-server/src/ReactFizzServer.js` function by
   function.
2. rd490 (replay nodes' length) and rd496 (getCurrentStack) are wrong
   answers until fixed; rd491-rd494 are readability.
3. Phase B of the migration (above), then build react-dom and
   react-dom-bindings and run the server gates.
