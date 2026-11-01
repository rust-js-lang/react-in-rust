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

Phase B: step 3 (ConfigDOM's four bindings) is done (b0dc024). Steps 1
and 2 are stopped, for three reasons (each needs the coordinator or the
user):

- **rd570 blocks them.** Any regenerated entry imports ReactFizzConfigDOM
  before ReactFizzServer, which fails 111 react-dom tests and one
  turbopack test; after step 2 both are a dependency's subpaths and sort
  so too. The entries' JS is main's until rust-js keeps the order.
- **The legacy entry's destination isn't `'static`.** The server holds
  its destination, `start_flowing(&Rc<Request>, &'static Destination)`,
  and ReactDOMLegacyServerStreamConfig's `Destination<'a>` is two `&'a
  dyn Fn` of the stack (`push` writes a stack `RefCell<String>`). Making it
  `'static` changes that stream config (react-dom-bindings, not this
  branch's) to `Rc<dyn Fn>` fields of `Rc` cells, with no `Box::leak`.
- **A request the entry's callbacks read is a box, and a cycle
  (rd573).** Each streaming entry's `onShellReady`/`pull`/`cancel` read the
  request `createRequest` returns, which the server holds them in, so they
  must be `'static` `Rc<dyn Fn>`s holding the request: an
  `Rc<OnceCell<Rc<Request>>>` prints `const request = {};`,
  `$unwrap(request.value)` and `$getOrInit(request, () => createRequest(..))`
  where main's entries print upstream's `let request; .. request =
  createRequest(..)`, and in Rust it's an `Rc` cycle (request -> callback ->
  cell -> request), a leak unless the cell holds a `Weak`, which adds
  `upgrade()`s. Repro:
  ```rust
  pub fn render(log: &'static dyn Fn(f64)) {
      let request: Rc<OnceCell<Rc<Request>>> = Rc::default();
      let on_shell_ready = {
          let request = Rc::clone(&request);
          move || log(request.get().unwrap().n)
      };
      let request = request.get_or_init(|| create_request(Some(Rc::new(on_shell_ready))));
      start(request);
  }
  ```
  prints `const request = {}; function onShellReady() {
  log($unwrap(request.value).n); } const request$1 = $getOrInit(request,
  () => createRequest(onShellReady));` (an `Rc<RefCell<Option<..>>>`
  prints `$borrow`s). The user's choice: accept the box, or a rust-js form
  where a cell set once after the closures are made is the `let` they
  close over.
- Options' callbacks: an `Options` field typed `Option<Rc<OnError>>`
  (not `&'static OnError`) reads `options?.onError` with no wrapper, so the
  server's `Rc` callbacks need no arrows there. The server's `OnError`
  returns `Option<&'static Unknown>` (upstream checks the digest's type),
  CurrentTask's `Option<String>`: the entries' `on_error`s change type.

The plan, for when they're unblocked (step 1 lands with step 2):
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

- **rd571** (readability) a field read into a `let` so its `RefCell`
  guard isn't held through a call prints the `let`: `let row =
  task.row.borrow().clone(); finished_task(request, None, row.as_ref(),
  Some(segment));` is `const row = task.row; finishedTask(request, null,
  row, segment);` where upstream writes `finishedTask(request, null,
  task.row, segment)`. The `let` is what keeps the guard out of the call
  (in Rust, `f(t.row.borrow().as_ref())` holds it through `f`, so rust-js
  counts the field everywhere: `$borrow($fieldCell(task, 'row', true))`).
  About 40 such lets (the create task calls, finishedTask/erroredTask,
  getThrownInfo, switchContext). Repro:
  `fn f(t: &T) { let row = t.row.borrow().clone(); g(t, row); }` prints
  `const row = t.row; g(t, row);`; suggested: a `const` of a plain field
  read, used once as the next statement's argument with nothing between
  that could write it, read in place.

- **rd572** (readability, no minimal repro yet) ConfigDOM's preload, of
  `let resources = if has_as_type { let resources = *dict::get(..).unwrap();
  if .. { return; } resources } else { .. };`, printed upstream's `resources
  = $unwrap(unknownResources[as]);` before phase B's step 3 and prints
  `const resources$1 = ..; .. resources = resources$1;` after it, though
  the step changed only where `resumable_state` comes from (the server's
  generic `get_resumable_state::<ResumableState>`, where it was a
  binding). The module-unknown site below it printed `resources$1` before
  too. The same function alone, in a crate of its own, prints upstream's
  form.

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

## Where the draft stands (2026-11-02, after the rebase on main)

Built and committed: ReactFizzServer.js is generated (rust-js 090616b60)
and passes the gates: react-dom 4264/1/2 (the 1 is PartialHydration's
`[GATED, SHOULD FAIL]` LegacyHidden), the server set 472/472 (with the
port's ReactFizzPostponedState-test), `-r=stable` 454/454, "very deep
trees" 6/6 (rd551 fixed the deep-tree resume; the prologue gap above is
gone), no labels.

- Step 3 (readability): the task fields' `$fieldCell` borrows are gone
  but for a together row's `boundaries` (6), held through
  isEligibleForOutlining, which calls the config's hasSuspenseyContent.
- A resume slot's `None` is `#[rust_js::null]`: it was
  `#[rust_js::name = "null"]`, the string `'null'`, in every postponed
  state's `replaySlots` and replay node, where upstream has `null` (a
  wrong answer, in the JSON an app stores; the port resumed its own
  alike, so no upstream test saw it). The port's
  `packages/react-server/src/__tests__/ReactFizzPostponedState-test.js`
  holds it.
- Phase B: step 3 done; steps 1 and 2 stopped (above: rd570, the legacy
  destination, rd573).

Next: rd570's fix in rust-js, then the user's choice on rd573 and the
legacy stream config's destination, then steps 2 and 1, then the
remaining fndiff differences (the report's list).
