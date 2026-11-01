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

## GAPs (rd490-rd496)

Each is marked `// GAP rdNNN` in the source.

- **rd490** a replay node is `[name, key, children, slots]` or a boundary's
  `[name, key, children, slots, fallback, rootSegmentID]`, told apart by
  `node.length === 4`; Rust's form is an untagged enum of two tuple structs,
  which rust-js can't tell apart (rd291's kind). Repro:
  `#[rust_js::untagged] enum N { A(Rc<(String, f64)>), B(Rc<(String, f64, f64)>) } fn f(n: &N) -> bool { matches!(n, N::A(_)) }` wants `n.length === 2`.
- **rd491** a function value given as a callback: `noop` (a `fn()`) is no
  `dyn Fn(A)` or `dyn Fn(A, B)`, so `onShellError === undefined ? noop :
  onShellError` needs `Rc::new(|_| {})`, printed as an arrow; and a task's
  ping functions (`Rc<dyn Fn()>`) can't be given to `Thenable::then`, whose
  parameters are `impl FnOnce`, as themselves (`then(ping.resolve,
  ping.reject)`). Repro: `fn f(cb: Option<Rc<dyn Fn(f64)>>) -> Rc<dyn Fn(f64)> { cb.unwrap_or_else(|| Rc::new(|_| {})) }` wants `cb ?? noop`.
- **rd492** node has no `async_hooks` binding: ReactFizzConfig's
  `requestStorage` (`AsyncLocalStorage<Request | void>`) is bound locally,
  `RequestStorage` with `getStore`, as ReactFlightCurrentOwner binds its
  `ComponentStorage`. Repro: `node::async_hooks::AsyncLocalStorage::<T>::get_store` doesn't exist.
- **rd493** a field set only under a condition: upstream sets a task's
  `legacyContext` only `if (!disableLegacyContext)` and `debugTask` only in
  DEV (to stay at 16 fields), and a boundary's errorMessage/errorStack/
  errorComponentStack only in DEV. A struct's fields are always written;
  `#[rust_js::optional]` on a `RefCell<Option<..>>` should leave them
  absent until set. Repro: `struct T { #[rust_js::optional] d: Cell<Option<f64>> } fn new(dev: bool) -> T { T { d: Cell::new(if dev { Some(1.0) } else { None }) } }`, upstream `const t = {}; if (dev) t.d = 1;`.
- **rd494** `new RequestInstance(..)`: upstream makes the request with a
  constructor function for its hidden class; a Rust struct literal prints an
  object literal. Repro: any struct of >16 fields made by one function.
- **rd495 (blocking the build)** ADR 0626's refusal: "this crate's
  `dyn Fn`s may hold a value with a destructor, which their drop runs, but
  `react_isomorphic`, a rust-js library whose `dyn Fn`s drop nothing, holds
  one of them". The server's `dyn Fn`s hold `Rc`s (a task's ping closures
  capture the `Rc<Request>` and a `Weak<Task>`; the request's callbacks are
  `Rc<dyn Fn>` of closures an entry makes), and react-server reaches
  react_isomorphic's `BasicStateAction::Update(&'static dyn Fn(S) -> S)`
  through ReactFizzHooks. ADR 0626's amendment records this refusal as the
  recommendation while the user's decision is pending (rust-js's
  `decisions-pending.md`, rd57). Repro: library crate `dep` exports
  `pub enum E { F(&'static dyn Fn()) }`; a consumer that uses `dep::E` and
  makes `let r = Rc::new(1); let f: Rc<dyn Fn()> = Rc::new(move || { let _ = &r; });`
  is refused. (The draft's earlier rd495, an `Rc` or a `String` given to
  JS as `&'static Unknown`, was its own mistake: `js::unknown_of(value)`
  is that form, and the draft uses it.)
- **rd496 (blocking, a wrong answer)** `ReactSharedInternals.getCurrentStack`:
  its getter gives `Option<&'static dyn Fn() -> String>`, its setter takes
  `Option<fn() -> String>`, so rerenderStalledTask can't put back what it
  read. The draft puts back its own function, wrong where another
  renderer's was set. Repro: `let p = R.get_current_stack(); R.set_get_current_stack(p);` doesn't type-check.

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
