# React, in Rust

This repository is [react/react](https://github.com/react/react) being
ported to Rust, module by module, with [rust-js](https://github.com/rust-js-lang/rust-js):
each JavaScript file is replaced by a Rust module that rust-js compiles to
the JavaScript beside it, which reads as the original does. React's own
build and tests run on that JavaScript as they ran on the original.

The tree started as react/react at
[`ab0d927`](https://github.com/react/react/commit/ab0d927f53f6e1b5dcfc2c4e9fc438bdb5ffb678)
(2026-10-20). This repository's root commit, `83b9c29`, is that tree: its
`packages/` are ab0d927's byte for byte. What isn't Rust yet is still
upstream's JavaScript.

This page says how to port a module today. What Rust to write, and the JS
rust-js makes of it, is in [rust-js's guide](../rust-js/docs/guide/README.md),
so read that first. rust-js's [AGENTS.md](../rust-js/AGENTS.md) is the north
star both repositories follow.

## Setting up

Clone the two repositories side by side:

```sh
git clone git@github.com:rust-js-lang/rust-js.git
git clone git@github.com:rust-js-lang/react-in-rust.git
(cd rust-js && bun install && bun run build)
cd react-in-rust && yarn install
```

react-in-rust reaches rust-js only through `../rust-js`:

- **Cargo:** the workspace's `Cargo.toml` takes `js`, `webapi` and `node`
  as path dependencies: `../rust-js/builtins`, `../rust-js/webapi` and
  `../rust-js/node`.
- **npm:** `package.json` links `@rust-js/runtime` and `@rust-js/std`, the
  packages the generated JS imports, to `../rust-js/runtime` and
  `../rust-js/std`. `yarn rust` runs `../rust-js/tooling/cli.js build`.

After you pull rust-js, run `bun run build` there again before building here.

## Building

Each package's Rust is a crate in the workspace, and the crate's modules are
the package's files: `packages/scheduler/src/SchedulerMinHeap.rs` compiles
to `SchedulerMinHeap.js` and `.d.ts` beside it. A crate's root file is
Cargo.toml's `[lib] path`: `shared.rs`, `src/forks/Scheduler.rs`,
`react.rs`, `reconciler.rs`, `bindings.rs`, `react_dom.rs`, `noop.rs`,
`test_renderer.rs`, `react_art.rs`, `client.rs` or `markup.rs`. It declares each module as
`#[path = "src/..."] pub mod ..;` and writes no JS of its own.

The crates are `shared`, `scheduler`, `react-isomorphic` (packages/react),
`react-reconciler`, `react-dom-bindings`, `react-dom`,
`react-noop-renderer`, `react-test-renderer`, `react-art`, `react-client`, `react-server-dom-webpack`, `react-server-dom-turbopack`, `react-server-dom-parcel`, `react-server-dom-esm`, `react-server-dom-unbundled` and `react-markup`. A new package is a new crate,
added to the workspace's `members`.

**A react-server-dom-* package is one crate of both sessions' modules**
(the coordinator, 2026-11-02, with the second session): its root is
Cargo's default `src/lib.rs`, `pub mod client; pub mod shared;` (the second
session adds `pub mod server;`), and `src/client.rs` and `src/shared.rs`
declare their directory's modules, `#[path = "client/X.rs"] pub mod x;`.
None of the three writes JS. `src/shared/ReactFlightImportMetadata` is the
first session's (the second's server modules import it): `ImportMetadata`
is an untagged enum of upstream's two tuples, `Async((id, chunks, name,
Async))` and `Sync((id, chunks, name))`, told apart by length, so
`isAsyncImport` prints upstream's `metadata.length === 4`; its chunks, and
`ImportManifestEntry`'s, are `&'static [String]`, JS's arrays shared as
upstream shares them. Webpack's runtime globals (`__webpack_require__`,
`.u`, `__webpack_chunk_load__`, `__webpack_get_script_filename__`) are
`shared::environment`'s, as `scripts/flow/environment.js` declares them.

**A JS library's binding is a crate of its own** (the user, 2026-11-01):
`bindings/art` is `@rust-js-bindings/art`, the `art` library's, laid out
as rust-js-lang/bindings' `bindings/<library>/` (ADRs 0118, 0119):
`Cargo.toml` (rust-js-bindings-art, `[package.metadata.rust-js] bindings =
true`), `package.json`, `src/lib.rs` and `README.md`, written by hand from
the library's JS, so it can move to that repository unchanged. It names
`js` by version; the workspace's `[patch.crates-io]` says where it is. A
crate using it takes `art.workspace = true`. `bindings/busboy` (busboy,
for the Flight servers' Node reply) and `bindings/bun` (Bun's own APIs,
for its Fizz server) are laid out alike. The generated `.js` and `.d.ts` files are
committed, as upstream's JS was. Source maps are not committed (see
`.gitignore`).

```sh
yarn rust                                  # compile every crate to its JS, at once
yarn rust --package react-dom-bindings     # compile one crate, and what it uses
cargo fmt -p react-dom-bindings            # rustfmt, this crate only
node ../rust-js/tooling/cli.js clippy --package react-dom-bindings -- -D warnings
```

- **Regenerate the whole port with plain `yarn rust`**, one build: each
  crate is checked once and Cargo checks the crates that don't use each
  other on every core (rust-js ADR 1068). `--package` may be given several
  times, as Cargo's `-p`. Never loop over the crates one `yarn rust
  --package` at a time.
- **Touch the root.** Right after you build one crate, building a crate that
  depends on it can say `Finished` and write nothing. If the JS you expected
  didn't change, `touch` the crate's root file (for example
  `packages/react-dom-bindings/bindings.rs`) and build again. Check
  `git status` after every build.
- **Run `cargo fmt -p <crate>`, never `cargo fmt --all`.** `--all` follows
  the path dependencies and reformats rust-js's crates.
- **Use rust-js's clippy** (ADR 0888), the command above. It must be clean
  with `-D warnings`. Don't use plain `cargo clippy`: it reports items that
  only JS reads as dead code.

## The gates

`scripts/port/gates.sh <tag>` runs React's suites on the tree's JS. Each
suite runs under `timeout 300`, and the script prints one line per gate.
Logs go to `target/port/logs/<tag>-<gate>.log`.

Before every commit, run the gates your change reaches. Their counts must
match the numbers below, and any change in a count is a finding to explain,
not noise. Lower `PORT_WORKERS` (default 4) when the machine is shared. If a
run times out, split it; never raise the timeout.

Every command below also takes `--maxWorkers=4`. Counts are passed / failed /
skipped, measured on the tree at 92e4fac.

| Gate | Command | Expected |
|---|---|---|
| dev | `yarn test packages/react-reconciler packages/react/src packages/shared` | 1557 / 0 / 20 (110 suites)³ |
| prod | `yarn test --prod packages/react-reconciler packages/react/src packages/shared` | 1534 / 0 / 20 (110 suites)³ |
| dom | `yarn test react-dom/src/__tests__/ReactDOM` | 3356 / 1 / 2 (84 suites)¹ |
| misc | `yarn test PerformanceTrack Profiler TestSelectors ViewTransition ErrorBoundaries Activity SuspenseList Fragment` | 551 / 0 / 1 (28 suites) |
| sched | `yarn test packages/scheduler` | 63 / 0 / 1 (6 suites) |
| schedwww | `yarn test -r=www-modern packages/scheduler` (profiling on) | 71 / 0 / 1 (6 suites) |
| domrest | `yarn test packages/react-dom/src/__tests__ --testPathIgnorePatterns 'ReactDOM[A-Z]'` | 695 / 0 / 0 (49 suites) |
| ev | `yarn test $(cat scripts/port/ev.txt)` (events, forms, inputs, ReactDOMComponent) | 1057 / 0 / 1 (29 suites) |

¹ The one failure is ReactDOMServerPartialHydration's `[GATED, SHOULD FAIL]
renders a hidden LegacyHidden component inside a Suspense boundary`, the
baseline failure every recorded run of this gate has had.

³ With the port's own `packages/react/src/__tests__/ReactChildrenGiven-test.js`
(3 tests: Children.map gives back null or undefined children as given),
added in Batch 42; upstream's suites alone are 1554 and 1531 in 109.

### Server side

`scripts/port/gates.sh <tag> --server` adds the gates below, and
`--server-only` runs only them. These suites hold react-server,
react-client, react-server-dom-* and react-dom-bindings/src/server.

All of them run in the default channel (experimental) and need no flag. For
react-server, react-client, the Fizz suites and react-server-dom-fb,
`-r=stable` and `-r=www-modern` give the same counts.

| Gate | Command | Expected |
|---|---|---|
| fizz | `yarn test react-dom/src/__tests__/ReactDOMFizz` (14 suites, the Static ones included) | 381 / 0 / 0⁶ |
| ssr | `yarn test react-dom/src/__tests__/ReactDOMServerIntegration` (21 suites) | 1602 / 0 / 0 |
| float | `yarn test react-dom/src/__tests__/ReactDOMFloat react-dom/src/__tests__/ReactDOMLegacyFloat` (2 suites) | 143 / 0 / 1 |
| ssrmisc | `yarn test react-dom/src/__tests__/ReactServerRendering react-dom/src/__tests__/ReactDOMServerLifecycles react-dom/src/__tests__/ReactDOMInReactServer react-dom/src/__tests__/ReactClassComponentPropResolutionFizz packages/react-markup packages/use-sync-external-store/src/__tests__/useSyncExternalStoreShimServer` (9 suites) | 104 / 0 / 0 |
| server | `yarn test packages/react-server/src/__tests__` (8 suites) | 41 / 0 / 0²⁵⁶ |
| client | `yarn test packages/react-client/src/__tests__` (4 suites) | 98 / 0 / 0⁴ |
| webpack | `yarn test packages/react-server-dom-webpack/src/__tests__` (9 suites) | 278 / 0 / 0 |
| turbopack | `yarn test packages/react-server-dom-turbopack/src/__tests__` (7 suites) | 43 / 0 / 0 |
| fb | `yarn test packages/react-server-dom-fb/src/__tests__` (1 suite) | 8 / 0 / 0 |

The `ssr` gate's 21 suites are the 20 `ReactDOMServerIntegration*` files plus
`ReactDOMserverIntegrationProgress` (lowercase s), which the same pattern
matches. Many server suites also run inside `dom` and `domrest`.

⁴ With the port's own `packages/react-client/src/__tests__/ReactFlightReplayOrder-test.js`
(1 test, DEV: a console entry that arrives while an earlier one is still
blocked replays after it, as upstream's resolveConsoleEntry does), added in
Batch 52 with the fix of its unblock, and
`packages/react-client/src/__tests__/ReactFlightUndefinedReference-test.js`
(2 tests: a root that is only a reference to a row sent later resolves to
that row's `undefined` or `null` as it is), added in Batch 53 with the
handler's `value` and `reason` holding what they're given; upstream's suites
alone are 95 in 2.

⁵ With the port's own `ReactFizzPostponedState-test.js`,
`ReactFlightSymbolDescription-test.js` (1 test, DEV: a logged symbol of no
description is sent as `"$Sundefined"`) and
`ReactFlightGivenDebugValues-test.js` (3 tests: an element without an
`_owner` throws where upstream reads its owner; a forwarded I/O info's
`null` value is sent and an `undefined` one left out), the last added with
ReactFlightServer's migration; upstream's suites alone are 26 in 4.

⁶ With the port's own `ReactDOMFizzServerBun-test.js` (7 tests) and
`ReactServerStreamConfigBun-test.js` (10 tests), added with the Bun server
entry, as no upstream test runs it.

² One run in five of `-r=www-modern packages/react-server/src/__tests__`
failed one test (25 / 1 / 0) while the machine was loaded. The default
channel and the reruns passed. If it fails for you, rerun that gate alone
before you treat it as a finding.

The gates' last check looks for **labels** that rust-js made in the
generated JS (`bbN:`, `break bbN`). There must be none; the script prints
any it finds.

## Comparing with upstream

Before committing, compare **every generated file you changed, function by
function**, with upstream's. The tools take a path from the repository root
or from under `packages/`, and they read upstream from `83b9c29`
(`UPSTREAM=` overrides that):

```sh
scripts/port/cmp.sh react-dom-bindings/src/client/ReactDOMInput.js     # whole-file diff
scripts/port/fndiff.sh react-dom-bindings/src/client/ReactDOMInput.js  # each function's count of differing lines
scripts/port/fndiff.sh react-dom-bindings/src/client/ReactDOMInput.js updateInput initInput  # those functions' diffs
scripts/port/score.sh <file.js>...                 # differing lines at HEAD -> in the working tree
python3 scripts/port/aliases.py [packages/<pkg>]   # `const a = b;` copies upstream lacks
```

One side of each diff is upstream's file with its Flow types removed. The
other is the generated file. Both go through the repository's prettier, so
only code differs (the license header and Flow-only comments always do).
`fndiff.sh` also lists functions that are MISSING from, or EXTRA in, the
generated file.

Each difference you find is one of these:

- **A rust-js gap.** Fix it in rust-js, or give it an id and a minimal repro
  (a `.rs` file and the `.js` it makes) and send it to the coordinator.
- **An accepted difference or form** from the list below. Name its id in
  the commit.
- **A port-side form.** List it in the commit message's "Port differences".

## Holding a module

A module is **held** (its Rust stays out of the tree and upstream's JS
stays in) only for one of these:

- **A JS label** that rust-js made.
- **A behavior difference** from upstream: a wrong answer, a failing or
  changed test, or a value or exception upstream doesn't have.
- **A scaffold:** a stand-in binding, a stub, or a types-only module
  standing in for code.
- **Bent Rust:** Rust written to get past rust-js, not as a Rust engineer
  would write it.

Anything else is a readability gap: a renamed local, a moved comment, a
temporary, or `$unwrap` where upstream lets JS throw (h03). A readability gap
doesn't hold the module. Commit the module and list each gap by id in the
commit message.

## The rules

- **Write idiomatic Rust**, as rust-js's [AGENTS.md](../rust-js/AGENTS.md#idiomatic-rust-in)
  says: std's types and idioms, clean under rustfmt and clippy. No
  `#[allow]`, no `Box::leak` and no stand-in bindings. A stand-in is a local
  binding for a member that webapi or js should have, or a `link_name` cast.
  A missing binding is a rust-js gap.
- **Use `js::get` and `js::set` only** where TypeScript's types lack the
  member or upstream passes `as any`. A member that TypeScript declares gets
  a typed binding.
- **Preserve behavior exactly:** null versus undefined, evaluation order,
  exceptions and `this`. Where upstream lets JS throw, the port throws too.
- **Keep types no narrower than upstream's.** A parameter that upstream
  types `?T` or `T | null` stays an `Option`. One it types `any` or `mixed`
  is `&Unknown` or `Option<&Unknown>`. Upstream's parameters stay:
  batchedUpdates keeps its `b`.
- **Mirror upstream's flag tests per site** (ft1): write `(flags & X) != 0`
  where React tests by truth, and `!= NO_FLAGS` where React compares with
  NoFlags.
- **Make live shared arrays `Rc<RefCell<..>>`.** An array that JS code
  holds and mutates while others hold it too is one shared
  `Rc<RefCell<Vec<..>>>`, never a copy.
- **Write a true fallthrough as one arm** (rd127):
  `A | B => { if tag == A { .. } ..B's body.. }`. Never repeat a call or copy
  a body.
- **Reach not-yet-ported JS through a JS-path binding**,
  `#[cfg_attr(rust_js, rust_js::link_name = "./src/client/ReactFiberConfigDOM.js#name")]`,
  where the path starts at the crate's directory. When that module becomes
  Rust, the binding becomes a `use`.
- **A module JS hands around as a value is a namespace** (rust-js's ADR
  1037, rd292). ReactFiberReconciler is `#[cfg_attr(rust_js,
  rust_js::namespace)] pub struct ReactFiberReconciler;` and its exported
  functions are `&self` methods in namespace impls, kept in the file's order
  between its private items. Callers name it,
  `ReactFiberReconciler.should_error(fiber)`, which prints
  `shouldError(fiber)` with its named import; a renderer given the namespace
  calls through the value, `noop_renderer.create_container(..)`, printed
  `NoopRenderer.createContainer(..)`.
- **Port fallbacks too** (the user, 2026-10-22). A polyfill, or a branch
  for a host without `Object.is` or `performance.now`, is the program's to
  express, whatever hosts rust-js targets.
- **Use `shared::react_types` for shared/ReactTypes' types** (the user,
  2026-10-31). `packages/shared/ReactTypes.rs` is their one Rust
  definition: `ReactKey`, `ReactContext`, `ReactConsumerType`, `ReactPortal`,
  `RefObject`, `Thenable`, `Wakeable`, `RejectedThenable`, `ReactIOInfo`,
  `ReactAsyncInfo`, `ReactComponentInfo`, `ViewTransitionProps`,
  `ViewTransitionClass`, the props types and the rest. Import them from
  there, never declare a local copy; a value whose type is one of them but
  that the port reads untyped is written with its alias,
  `Option<&'static ReactNodeList>`. Three stay where their code is: the
  reconciler's `ActivityProps` (its `mode` is the reconciler's
  `OffscreenMode`, which upstream's Flow lets `'hidden' | 'visible'` flow
  into), `ReactScopeInstance` (ReactFiberScope's object, whose methods it
  implements) and react's `GestureTransitionOptions` (upstream's inline
  `GestureOptions & StartTransitionOptions`). `ReactDebugInfoEntry` is
  `Unknown`: an untagged enum can't tell its four kinds apart (rd291). These
  four exceptions are accepted (the user, 2026-10-31).
  **m6-wife:** your server crates had no local copies, so nothing of yours
  moved; a server type of shared/ReactTypes' (`ReactComponentInfo`,
  `ReactDebugInfo`, `ReactErrorInfo`, `ReactStackTrace`, `Thenable`, ..) is
  in `shared::react_types` for your ports.
- **Write a host feature test as upstream does.** `typeof X === 'function'`
  is `js::is_function(binding)`, `typeof X !== 'undefined'` is
  `js::is_defined(binding)`, and `typeof performance === 'object' && typeof
  performance.now === 'function'` is `js::is_object(webapi::performance) &&
  js::is_function(webapi::Performance::now)`. `js::is_function` of a path
  alone asks for its first part too, `typeof performance !== 'undefined' &&
  typeof performance.getEntriesByType === 'function'` (rd320, rust-js
  2f8ab2e73), so a host without the global gives `false` where upstream's
  bare `typeof performance.getEntriesByType` throws (estimateBandwidth,
  accepted below).
- **Read a global as upstream does.** A bare global is its webapi binding;
  one upstream reads as the global object's property, `global.scheduler`,
  is `webapi::global_this.scheduler()`, `globalThis.scheduler` (rd319). A
  global no binding declares, `global.IS_REACT_NATIVE_TEST_ENVIRONMENT`, is
  a global-path static, `#[link_name = "global.IS_REACT_NATIVE_TEST_ENVIRONMENT"]`.
- **Port type-only modules as Rust and their `.d.ts`**, and remove
  upstream's Flow `.js` (the user, 2026-10-28). A package's entry,
  `index.js`, keeps only plain JS re-exports, and its Flow types move to
  `index.d.ts` (rd55).

## Imports in upstream's order

rust-js prints each JS import where the first `use` that makes it sits, and
an import a module's own binding makes (`link_name = "./ReactFiberConfig.js#.."`)
where that binding is declared (ADRs 1063, rd570 and rd608). So a module's
`use`s are written in upstream's import order, one rustfmt group (a run
between blank lines, which rustfmt keeps in order) per stretch that sorts,
and its first binding of each JS-path module sits among them where
upstream imports that module; `use`s that make no import (std, js, webapi,
type-only modules) come first, in rustfmt's std / external / crate groups.
`b55/imp/imports.py` (the first session's scratch) checks and regroups
them. A `use` of another crate's binding through the module that holds it
(`shared::react_shared_internals::REACT_SHARED_INTERNALS`, shared's
`assign`) places that module's import at the `use` too (rd609, rust-js
5c63e9442), and so does a `use` of another crate's namespace
(`ReactFiberReconciler`) or of a binding crate's module (`art::{mode,
transform}`) (rd611, 40051fd39), and a `use crate::module;` of the crate's own module
called qualified (ReactFiberReconciler's `react_portal`) and a `pub use`'s
re-export print at the `use` too (rd620, 157e916f1). So a module of
`pub use`s, ReactClient's or ReactServer's, writes them in upstream's
import order, its first of each module where upstream imports that module.
ReactClient's `export * as __COMPILER_RUNTIME` stays where upstream's
export list has it, after `__CLIENT_INTERNALS_DO_NOT_USE_OR_WARN_USERS_THEY_CANNOT_UPGRADE`,
though upstream imports ReactCompilerRuntime last (the user, 2026-11-02,
Batch 58), and ReactServer.experimental's taint re-export lists
`taintObjectReference` before `taintUniqueValue`, as rustfmt sorts one
`pub use` run, where upstream lists `taintUniqueValue` first (the user,
2026-11-02, Batch 58).

## Null and undefined

rust-js's rule (the user, 2026-10-31): **`None` is absent. On write, the
destination's TypeScript type picks `undefined` or `null`; on read, both are
`None`. Where a program must tell them apart, it takes `&Unknown`.**

So an `Option` parameter or field reads JS `null` and `undefined` alike, and
the port writes `Option<T>` wherever upstream treats the two the same. Where
upstream's behavior depends on which one it was given, the value is
`&Unknown`, tested with `js::classify` (`Kind::Null`, `Kind::Undefined`).

createReactNoop's `rootID` is the decided case: upstream's
`getChildren(rootID: string = DEFAULT_ROOT_ID)` defaults only for
`undefined`, and the port takes `Option<&str>`, so JS `null` and `undefined`
both mean the default root, as `rootID ??= DEFAULT_ROOT_ID` would (the user,
2026-10-31).

Two more are decided the same way (the user, 2026-10-31):

- **ReactDOMRoot's options** are `Option` fields tested `!= null` where
  upstream tests `!== undefined`: identifierPrefix, onUncaughtError,
  onCaughtError, onRecoverableError, onDefaultTransitionIndicator,
  unstable_transitionCallbacks and formState. An option given as `null`
  keeps its default, where upstream passes the `null` on.
- **ReactDOMRoot's render and unmount** test `root == null` and
  `root != null` where upstream tests `=== null` and `!== null`.
  `_internalRoot` is a root or `null`, so only a method called with a
  foreign `this` tells them apart.

**A value given back as it was given is `&Unknown`** (the coordinator,
2026-11-01, applying the rule above). React.Children.map gives back null or
undefined children as given, upstream's `if (children == null) { return
children; }`: `map_children` takes `children: &'static Unknown`, tests
`js::loose_equals(children, js::null::<&Unknown>())` (printed `children ==
null`) and returns `MappedChildren`, an untagged enum of the mapped `Vec`
or, `otherwise`, the `&Unknown` it was given. `Children.map(undefined, f)`
is `undefined` again; `ReactChildrenGiven-test.js` holds it, as no
upstream test does. This supersedes 730df40's "Children.map(undefined)
returns null", accepted before the rule.

**The Flight targets' `moduleLoading !== null`** (rd613, rust-js
93a3bfc39, Batch 57): each Target's prepareDestinationWithChunks (webpack,
turbopack, Node) and each Bundler's prepareDestinationForModule (webpack,
turbopack, ESM, and the link declarations that call them) say
`nullable(module_loading)`, and react-client's `_moduleLoading` is a
`nullable` field, so the targets test `!== null` as upstream does. Parcel's
`ModuleLoading` is upstream's `null` alone, an enum, and takes none.
**Accepted by the user** (2026-11-02, by name; it changes a result, ADR
0262's third case): ResponseInstance stores `this._moduleLoading =
moduleLoading ?? null;` where upstream stores `moduleLoading`, because the
JS DOM clients give a manifest's `moduleLoading` as it is, `undefined`
where the key is left out. There ours preinits nothing, as Rust's `None`
does, and upstream's `undefined !== null` throws on `.prefix`.
ReactFlightDOMNodeModuleLoading-test.js holds it. createResponse's
`bundlerConfig`, `serverReferenceConfig` and `moduleLoading` are
`nullable` (Batch 59), and the Edge client, Rust since Batch 59, gives the
manifest's `moduleMap`, `serverModuleMap` and `moduleLoading` as upstream
does, each a `nullable` field of its `ServerConsumerManifest`. The store
keeps `?? null` while the Node clients, which that test runs, are JS:
revisit it when they are Rust.
resolveModule reads `response._moduleLoading` as upstream does (rd628,
rust-js e2669d625).

**Fall-through comments are upstream's** (rd635, rd638; rust-js c62897ba7,
4510511c8): rust-js writes `// falls through` before a case another falls
through to, and keeps the program's own comment there instead. So the port
writes upstream's text above the arm fallen to: `// Fallthrough`,
`// fallthrough` (ReactDOMComponent), `/* falls through */`
(SimpleEventPlugin), `// Intentional fallthrough to next branch`
(ReactFiberCommitWork). SelectEventPlugin's is rust-js's own, upstream's
`// falls through`.

**Pending in rust-js from Batch 59** (none changes a result): rd640,
ReplyClient's isSignatureEqual reads `let Some(bound_promise) =
reference_closure.bound else ..` in place, `referenceClosure.bound` at each
use, and each of its two `then` callbacks copies it, `const boundPromise =
referenceClosure.bound; const boundPromise$1 = referenceClosure.bound;`,
where upstream's `const boundPromise` is one variable both read.

**The imports' blank lines are the Rust's** (rd637, rd652, rust-js
4c84d5bb9..0f95f0962, Batch 60): each `use` is its import's statement, and
the blank lines between `use`s are those between the imports. The port's
`use` groups follow upstream's import groups (Batch 60: 54 files, 82 blank
lines added, 8 removed, Scheduler's `// TODO: Use symbols?` restored;
`b60/tools/usegroups.py` matches each JS import to its `use` by the source
map). **Accepted by the user** (2026-11-03, by name; rustfmt's kind, no
result changes): where upstream lists imports in one group in an order
rustfmt doesn't sort them in, the port keeps upstream's order and a blank
line between the `use`s that rustfmt would otherwise reorder: 272 blank
lines in 80 files (react-reconciler/src/ReactFiberWorkLoop 22, react-reconciler/src/ReactFiberBeginWork 17, react-dom-bindings/src/client/ReactFiberConfigDOM 13, react-reconciler/src/ReactFiberHooks 12, react-reconciler/src/ReactFiberReconciler 12, react-reconciler/src/ReactFiberCompleteWork 11, react-reconciler/src/ReactFiberCommitWork 10, react-dom-bindings/src/events/DOMPluginEventSystem 9, react-reconciler/src/ReactFiberThrow 8, react-dom-bindings/src/client/ReactDOMComponent 6, react-reconciler/src/ReactFiberCommitEffects 6, react-reconciler/src/ReactFiberUnwindWork 6, react-reconciler/src/ReactFiber 5, react-reconciler/src/ReactFiberApplyGesture 5, react-reconciler/src/ReactFiberClassUpdateQueue 5, react/src/ReactClient 5, react-dom-bindings/src/events/ReactDOMEventReplaying 4, react-dom-bindings/src/events/plugins/EnterLeaveEventPlugin 4, react-dom-bindings/src/events/plugins/SelectEventPlugin 4, react-reconciler/src/ReactFiberClassComponent 4, react-reconciler/src/ReactFiberRootScheduler 4, react-reconciler/src/ReactFiberTransition 4, react/src/ReactServer 4, react/src/ReactServer.experimental 4, react/src/ReactServer.experimental.development 4, react/src/ReactServer.fb 4, react-dom-bindings/src/events/ReactDOMEventListener 3, react-dom-bindings/src/events/plugins/ChangeEventPlugin 3, react-reconciler/src/ReactChildFiber 3, react-reconciler/src/ReactFiberConcurrentUpdates 3, react-reconciler/src/ReactFiberNewContext 3, react/src/jsx/ReactJSXElement 3, react-dom-bindings/src/client/CSSPropertyOperations 2, react-dom-bindings/src/client/ReactDOMInput 2, react-dom-bindings/src/events/plugins/ScrollEndEventPlugin 2, react-dom/src/shared/ReactDOM 2, react-noop-renderer/src/createReactNoop 2, react-reconciler/src/ReactFiberAsyncAction 2, react-reconciler/src/ReactFiberCommitHostEffects 2, react-reconciler/src/ReactFiberCommitViewTransitions 2, react-reconciler/src/ReactFiberHydrationContext 2, react-reconciler/src/ReactFiberLegacyContext 2, react-reconciler/src/ReactFiberRoot 2, react-reconciler/src/ReactFiberSuspenseContext 2, react/src/ReactChildren 2, react/src/ReactTransitionType 2, react-client/src/ReactFlightReplyClient 1, react-dom-bindings/src/client/DOMPropertyOperations 1, react-dom-bindings/src/client/ReactDOMEventHandle 1, react-dom-bindings/src/client/ReactDOMSelect 1, react-dom-bindings/src/client/ReactDOMTextarea 1, react-dom-bindings/src/client/ReactInputSelection 1, react-dom-bindings/src/events/DOMEventProperties 1, react-dom-bindings/src/events/SyntheticEvent 1, react-dom-bindings/src/events/plugins/BeforeInputEventPlugin 1, react-dom-bindings/src/events/plugins/FormActionEventPlugin 1, react-dom-bindings/src/shared/ReactDOMFormActions 1, react-dom/src/client/ReactDOMRoot 1, react-dom/src/shared/ReactDOMFlushSync 1, react-noop-renderer/src/ReactFiberConfigNoop 1, react-reconciler/src/ReactFiberAsyncDispatcher 1, react-reconciler/src/ReactFiberHiddenContext 1, react-reconciler/src/ReactFiberHotReloading 1, react-reconciler/src/ReactFiberHydrationDiffs 1, react-reconciler/src/ReactFiberSuspenseComponent 1, react-reconciler/src/ReactFiberTracingMarkerComponent 1, react-reconciler/src/ReactFiberTransitionTypes 1, react-reconciler/src/ReactFiberTreeContext 1, react-reconciler/src/ReactFiberTreeReflection 1, react-reconciler/src/ReactPortal 1, react-reconciler/src/ReactStrictModeWarnings 1, react-test-renderer/src/ReactFiberConfigTestHost 1, react-test-renderer/src/ReactTestRenderer 1, react/src/ReactCacheClient 1, react/src/jsx/ReactJSX 1, react/src/jsx/ReactJSXServer 1, scheduler/src/forks/Scheduler 1, shared/ReactPerformanceTrackProperties 1, shared/ReactSerializationErrors 1, shared/shallowEqual 1).

**Fixed in rust-js from Batch 57**: rd632 (rust-js 542398380, 31a532910,
Batch 60), the five nullable parameters are strict `T | null` again, their
tests `!== null` and no `?? null` (cachePool, `return next`,
diffHydratedProperties/diffHydratedText, thenableState, modelRoot,
promiseRef). rd641 (406943eee, Batch 60): encodeReply's `/* We don't use
URLSearchParams yet but maybe */` prints after its parameters, as
upstream's. rd621 is fixed
(rust-js 51d56e765, Batch 59): the DEV `then`'s callbacks write
`wrapperPromise._debugInfo` as upstream's do.

**`nullable(param)` promises `null`, never `undefined`** (the coordinator,
2026-11-02, Batch 57). Where each of the crate's calls gives a
`#[rust_js::nullable(param)]` parameter `null` or a value, rust-js tests it
`!== null` (rd613, rust-js 93a3bfc39), and its `.d.ts` is `T | null`. A
public function whose upstream type is `?T`, which a JS caller may leave
out, takes a plain `Option` instead: the reconciler's `updateContainer` and
`updateContainerSync` (`parentComponent: ?component`, `callback:
?Function`), whose private `updateContainerImpl` and
`getContextForSubtree` keep `nullable` and so test `!= null`/`== null`.
With `nullable` on the public entries, a renderer's
`updateContainer(element, container)` logged "Expected the last optional
`callback` argument to be a function" in DEV;
`ReactFiberUpdateContainerOptional-test.internal.js` holds it, as no
upstream test does.

## Accepted differences and forms

The user decided each of these. Name the id in a commit where one applies.
Nothing else is accepted without the user's decision.

These differences from upstream's output are accepted:

- **th2a** concise arrows; **th2b** `stack ?? ''`; **fo1** `for … of` loops.
- **ci1**: clippy's `collapsible_if` merging upstream's nested ifs.
- **dn1**: null tests dropped for non-Option values, or guards added where
  upstream passes null to a null-handling function.
- **mb1**: a bool where React keeps a number it only tests
  (maySuspendCommit).
- **opt1**: feature tests of optional methods print
  `typeof x.m === "function"`, via `js::is_function((value, Type::method))`,
  where upstream tests truth.
- **rd33**: warnValidStyle converts with `String(value)` before `test` and
  `replace`.
- **rd39**: getVendorPrefixedEventName's three statements where upstream
  writes `return (prefixedEventNames[eventName] = prefixMap[styleProp])`.
- **rd51 and rd120**: thread-locals, exported and private, are made when
  the module loads, like upstream's `const x = f();`. One whose initializer
  panics throws at load. In an import cycle whose order can't be decided, a
  private one stays lazy.
- **rd64g**: `typeof normalize === "function"`.
- **rd80 and wl1**: an arrow where upstream binds
  (createEventListenerWrapperWithPriority, and the work loop's
  `callback.bind(null, ..)`), and the arrow of the legacy FB one-time
  listener.
- **rd87**: the hoisted `const value = ..` of a DOM getter.
- **rd114 alias**: `let Selected = ui::Empty;` prints
  `import * as Ui; const Selected = Ui.Empty;`.
- **rd153**: upstream's `export * from '..'` is written `pub use m::*;`,
  which prints `export {..each name..} from '..'`.
- **rd183**: `stack.push(...current.children)` is written
  `stack.extend(webapi::html_collection::iter(current.children()))`, which
  prints rd365's index loop, `const items = current.children; for (let i =
  0; i < items.length; i++) { stack.push(items[i]); }` (rust-js b06243598);
  Rust's never-throwing `extend` is kept. createReactNoop's log,
  `buffered_log.extend(args)`, prints the same loop for
  `bufferedLog.push(...args)`.
- **A written let stays** (rd189 reverted, a95591661): a `let` the program
  wrote prints as a `const` even where upstream reads the field in place,
  so FragmentInstance's methods keep `const fragmentFiber =
  this._fragmentFiber;` (`let fragment_fiber = Rc::clone(&self.fragment_fiber.borrow());`,
  which ends the borrow before the traversal calls back).
- **rd221** (6412fc19d): a struct the program builds is its own data, so
  its fields read in place. A struct JS gave (memoizedProps, a host config's
  result) is read as often as the Rust reads it.
- **A value match prints as a conditional** (the user): the scheduler's
  `let start_time = match options.and_then(|options| options.delay) {
  Some(delay) if delay > 0.0 => current_time + delay, _ => current_time };`
  prints `const startTime = delay != null && delay > 0 ? currentTime + delay
  : currentTime;`, where upstream declares `var startTime;` and assigns it
  in nested ifs.
- **rd226** (the user): a `let` the program wrote stays, but a binding that
  only a pattern makes, `if let Some(x) = s.field`, is read in place. So
  prepareForCommit (`focusedElem`) and createTextInstance (the ancestor)
  read the field twice where upstream names it once.
- **rd243** (the user): a struct literal's field given a variable of its
  name is the shorthand, `{ crossOrigin }`, which clippy's
  `redundant_field_names` asks of the Rust, where upstream writes
  `crossOrigin: crossOrigin` (ReactFlightServerConfigDOM's processImg).
- **The Fizz instruction set's forms** (the user, rd264 and h03 aside):
  ReactDOMFizzInstructionSetShared's restore queue is a `Vec` of tuples,
  `[element, name, class]` each, where upstream pushes three flat values;
  `suspenseyImages.push.apply(..)` is `extend`, `$extend(..)` (rd267, as
  rd183); camelCase parameters, `suspenseBoundaryId`, where upstream writes
  `suspenseBoundaryID` (rd272); `Promise.resolve(clientHeight)` in the
  blocking promises, a typed `Vec`; `cleanupWith(cb, _event)`, an
  `onload` handler's event; `form.ownerDocument` without `|| form`, an
  element's never null (rd207's kind). completeBoundaryWithStyles's one
  `while (true)` of a `sheetMode` flag is two loops, its stylesheets' and
  its `<style>` tags', and an `insertResource` helper of what both insert,
  as Rust writes a loop of two modes. Upstream's restore reads
  `previousClassName` from the name's slot, which the port keeps.
- **A closure that only forwards to a function is that function** (the
  user's extended forwarding rule, rd158's, rd196's and rd212's kind): a
  closure that passes its parameters on, as given and in order, to one
  function or a namespace's method read through a value is that function,
  `batchedUpdates: move |f, a| noop_renderer.batched_updates(f, a)` printed
  `batchedUpdates: NoopRenderer.batchedUpdates`, and `Rc::new(move |e, i|
  noop_renderer.default_on_uncaught_error(e, i))` printed
  `NoopRenderer.defaultOnUncaughtError`. Its `length` and identity are the
  function's, as rd196's are (rust-js 2e20abe4d). Its scope is only a
  namespace's function called through a value: a closure that forwards to a
  plain function is that function only where rd158, rd196 or rd212 says so.
  An `Rc` made of such a closure is the function too where the crate
  compares no `Rc`'s pointer (rd313, rust-js 51934f325, option A), so
  createReactNoop's `Rc` handlers print upstream's
  `NoopRenderer.defaultOnUncaughtError`, and
  `options?.onUncaughtError ?? NoopRenderer.defaultOnUncaughtError`. A
  thunk, a closure of no parameters, stays an arrow (ADR 0605).
- **rd158** (the user; rust-js's guide, difference 32): a closure that
  captures nothing, held as a `fn`, and only calls one host function with
  what it's given (its optional parameters `None`) is that function:
  `|callback| { js::set_timeout(callback, None); }` prints `setTimeout`, as
  in ReactFiberConfigDOM's scheduleMicrotask. A JS caller of the exported
  value sees two differences, both accepted: the function gives
  setTimeout's id where the closure gave undefined, and a delay or other
  extra arguments it passes reach setTimeout where the closure dropped them.
- **rd196 and rd212** (the user): a closure whose parameters are all unused
  and whose body only calls a function with no arguments, a function JS
  may call with any `this` and arguments it ignores, is that function:
  `then_with(move |_| ping(), move |_| ping())` prints `then(ping, ping)`,
  and `move |_| layout_callback()` prints `layoutCallback`. The function's
  `length` and identity differ from the closure's, seen only by a JS caller
  of an exported value.
- **rd236** (the user): localRequestAnimationFrame's callback takes
  `Option<f64>`, requestAnimationFrame's time as `Some` and nothing, from
  the setTimeout that stands in for it, as `None`. Its fallback is upstream's
  scheduleTimeout (`SCHEDULE_TIMEOUT`), so a host with neither
  requestAnimationFrame nor setTimeout gets undefined as upstream does, not
  a ReferenceError as the module loads.
- **rd246** (the user; ADR 0262's rare and harmless): startViewTransition
  and startGestureTransition give the browser a copy of the transition's
  types, `transition_types.as_ref().map(|types| types.borrow().clone())`,
  printed `transitionTypes?.slice()`, where upstream gives the shared array.
  A borrow held across the call is refused (ADR 0863), rightly: a
  startViewTransition may call `update` at once, and the commit it runs may
  borrow the types. Only a replacement of startViewTransition that keeps
  the array and compares it, or watches it change, tells them apart; the
  browser copies `types` into the ViewTransition's set.

- **rd229** (the user): getViewTransitionName's two Rust bindings of one
  name, `if let Some(name) = &props.name && name != "auto" { return
  name.clone(); }` and the later `let name = "_".to_owned() + ..;`, print as
  one reassigned JS `let name = props.name;`, where upstream reads
  `props.name` and declares `const name` after.

- **rd271** (the user; ADR 0262's rare and harmless, rust-js 21f8572f6, ADR
  0443): a trailing `None` given to a function value (a `dyn Fn`, a
  pointer, a closure or one `function::bind` makes) is no argument,
  `callback()` of `callback(None)`, as a binding's is. Only a callee that
  counts its arguments (`arguments.length`) can tell.

- **rd625** (the user, 2026-11-02, by name, rd271's kind): the crate's own
  `None` given to a `?T` parameter, a plain `Option` (see "`nullable(param)`
  promises `null`"), is left out where it's trailing and `undefined`
  otherwise, where upstream passes `null`: ReactDOMRoot's
  `updateContainer(children, root)` and `updateContainerSync(null, root)`,
  ReactTestRenderer's three `updateContainer(element, root)`,
  ReactFiberHotReloading's `updateContainerSync(element, root)`, ReactART's
  three `updateContainerSync(children, mountNode, this)`, and createReactNoop's
  three `NoopRenderer.updateContainer(element, root, undefined, callback)`.
  updateContainer reads both as none.

- **The Flight Node client's `() => close(response)`** (the user,
  2026-11-02, by name; rust-js ADR 0605): its stream's `end` handler is
  the zero-parameter closure `Rc::new(move || close(&closing))`, printed as
  an arrow, where upstream passes `close.bind(null, response)`.

- **rd627** (the user, 2026-11-02, by name): BundlerESM's requireModule
  is guard-first, `if (promise.status !== 'fulfilled') { throw
  promise.reason; } const moduleExports = promise.value;`, where upstream
  writes `let moduleExports; if (promise.status === 'fulfilled') {
  moduleExports = promise.value; } else { throw promise.reason; }` (the
  Rust is clippy's `let module_exports = if .. else { js::throw(..) };`),
  and its resolveServerReference returns `{specifier: fullUrl, name:
  exportName}` over four lines where upstream writes it on one.

- **BundlerNode (unbundled)** (the user, 2026-11-02, by name, as
  BundlerWebpack's and BundlerESM's): the import metadata's tuple read into
  locals, `const id = metadata[0]; const metadataName = metadata[2];`, where
  upstream reads `metadata[ID]`/`metadata[NAME]`; `moduleExports?.[..]` and
  `!= null` for upstream's `&&` and truth; the template-literal error;
  shorthand properties; `metadata.async === true`; `!metadata.name` for
  `=== ''`; requireModule ends without `return undefined`; `value =>
  value.default` for upstream's `function (value) { return .. }`; and rd627
  A's guard-first requireModule.
- **StartViewTransitionOptions' `types: types ?? null`** (the user,
  2026-11-02, by name; rust-js ae7515268): a dictionary's optional nullable
  member is an `Option<Option<T>>`, so startViewTransition and
  startGestureTransition give `types: types ?? null` where upstream gives
  `types: transitionTypes`.

- **rd285** (the user; ADR 0262's rare and harmless, rust-js 15fac97c2, ADR
  1034): a dispatcher call whose arguments need statements hoists the
  receiver as React does, `const d = ReactDOMSharedInternals.d; ..;
  d.L(href, as, {..})`, so the call keeps its `this`; the dispatcher field
  is read after the argument statements, where upstream reads it before.

- **createReactNoop's own functions** (the user, 2026-10-31):
  getOrCreateRootContainer and renderToRootWithID are plain Rust functions
  the ReactNoop object's methods call, where upstream calls them through the
  object, `ReactNoop.getOrCreateRootContainer(..)`. A test that replaces
  `ReactNoop.renderToRootWithID` no longer reaches the other methods; none
  does.

- **ReactNoop and ReactNoopPersistent** (Batch 56, rust-js 83c345b71,
  rd610): each is a `#[rust_js::namespace]` thread-local of its own exports
  struct (`ReactNoopExports`, `ReactNoopPersistentExports`), upstream's
  list in upstream's order, built `x: noop.x` from createReactNoop's
  `ReactNoop`, so each prints upstream's `export const {..} =
  createReactNoop(ReactFiberReconciler, true);`. `act` and
  `flushDiscreteUpdates` are `None` fields of `ReactNoop`, which the literal
  leaves out, so they are exports that are `undefined`, as upstream's are.
  Pending in rust-js, not accepted: the list's comments (`// TODO: Remove
  this ..`, `// $FlowFixMe[signature-verification-failure]`) print in the
  `.d.ts` but not yet in the destructuring.
- **createReactNoop's createRoot** (the user, 2026-10-31): its error
  handlers are `options?.onUncaughtError ?? NoopRenderer.defaultOnUncaughtError`
  (and `onCaughtError`), where upstream tests truth, `options &&
  options.onUncaughtError ? options.onUncaughtError : ..`; only a falsy
  handler that isn't a function, outside the option's type, tells them
  apart. Its roots pass `isStrictMode` as `false`, the reconciler's `bool`,
  where upstream passes `null`, which the reconciler only tests by truth.

- **rw2** (the user, 2026-10-31): react-is's typeOf braces each `case` that
  declares a name, `case REACT_ELEMENT_TYPE: { const type = object.type; .. }`,
  as rust-js braces every case that declares one, where upstream declares
  them unbraced.

- **rw5** (the user, 2026-10-31): typeOf ends without upstream's trailing
  `return undefined;`; a function that falls off its end gives `undefined`
  too.

- **gives_plain_data** (#411, 77029cdc4): a binding whose result is a fresh
  object of data properties that JS builds says so with
  `#[cfg_attr(rust_js, rust_js::gives_plain_data)]`, and the struct's fields
  are read in place. The js crate's `getOwnPropertyDescriptor` says it, so
  inputValueTracking tests `descriptor == null || descriptor.get == null ||
  descriptor.set == null` as upstream does. The reconciler's host-config
  bindings getSuspenseInstanceFallbackErrorDetails,
  describeHydratableInstanceForDevWarnings, getBoundingRect and
  setupIntersectionObserver say it too.

- **rd19** (the user, 2026-11-01): React DOM's getLeafNode keeps the
  Rust's single read of each node's `firstChild`, `let firstChild; while
  ((firstChild = node?.firstChild) != null) { node = firstChild; }`, where
  upstream reads it twice, `while (node && node.firstChild) { node =
  node.firstChild; }`.

- **estimateBandwidth's guard** (the user, 2026-11-01; ADR 0262's rare and
  harmless, rd320): `js::is_function` of `performance.getEntriesByType`
  prints `typeof performance !== 'undefined' && typeof
  performance.getEntriesByType === 'function'`, so a host without
  `performance` goes on to the `navigator.connection` estimate, where upstream's bare
  `typeof performance.getEntriesByType` throws a ReferenceError.

- **TestHost's getInstanceFromNode and getInstanceFromScope** (the user,
  2026-11-01): `if let Some(instance) = map.get(mock_node)` prints
  `if (instance != null)` where upstream tests `instance !== undefined`,
  and getInstanceFromScope's `map.get(scope_instance)` returned as an
  `Option` prints `?? null` where upstream writes `|| null`. The map holds
  only instances, never `null` or a falsy value, so no test tells them
  apart.

- **ReactART's fill classes read `arguments`** (rd332, 2026-11-02; was
  `(...args)`, the user's of 2026-11-01): LinearGradient, RadialGradient
  and Pattern take upstream's named parameters, so `.length` is theirs, and
  keep `function::call(slice, Some(js::arguments()), ())` of `use
  js::array::SLICE as slice;`, printed `const slice =
  Array.prototype.slice;` and `slice.call(arguments)` as upstream (rust-js
  3393c355a, ADR 0622). `applyFill` calls the node's method read as a
  value, `node.fillLinear.apply(node, this._args)`. Accepted on the user's
  behalf while away, 2026-11-02: the `const` is where the Rust's `use` is,
  after the imports, where upstream declares it above LinearGradient,
  after `Mode.setCurrent(..)`; both read the global as the module loads,
  and nothing between replaces it. ReactART.d.ts declares the private
  `slice` with the binding's Rust signature (rd517, P3, rust-js's).
- **ReactART's JS is `ReactART.jsx`** (the user, 2026-11-01; 8839806):
  rust-js names a module with JSX `.jsx`, so upstream's ReactART.js is
  removed and jest resolves the `jsx` extension
  (scripts/jest/config.base.js's moduleFileExtensions); index.js still
  imports `./src/ReactART`.

- **react-art's `prevProps ??= {}`** (the user, 2026-11-01; rd333): an
  applyXProps function's `prev_props.unwrap_or(object::spread(None))`
  prints `prevProps ??= {};` as its first statement, where upstream
  defaults the parameter, `prevProps = {}`. `??=` also replaces a `null`,
  which only a caller passing `null` sees (upstream then throws reading
  `null.cursor`); React never does.

- **Surface's `OnceCell::set` guard** (the user, 2026-11-01): Surface's
  `_mount_node` is a `#[rust_js::optional] OnceCell<Rc<FiberRoot>>`, set as
  it mounts, `_ = self._mount_node.set(ReactFiberReconciler.create_container(..))`,
  which prints `const value = createContainer(..); if (this._mountNode ===
  undefined) { this._mountNode = value; }` where upstream assigns
  `this._mountNode = createContainer(..)`. React mounts an instance once, so
  the guard always holds; its later reads are Rust's `get().unwrap()`,
  `$unwrap(this._mountNode)`.

- **`Vec::clear` as `length = 0`** (the user, 2026-11-01): a shared
  array's `children.borrow_mut().clear()` prints `container.children.length
  = 0;` where upstream writes `container.children.splice(0)` (TestHost's
  clearContainer); both empty the array in place, and the removed items
  upstream's splice returns are dropped.

- **rd322's one-key literal layout** (the user, 2026-11-01): a struct
  literal whose fields but one are `None` and so print nothing keeps
  rustfmt's line breaks: the object and its one key are on lines of their
  own inside the array, where upstream writes `[{awaited: ioInfo}]`
  (ReactLazy's `_debugInfo`).

- **The ReactClientConsoleConfigs' bindToConsole** (the user, 2026-11-01;
  rd294's form): the method is read once, `const method =
  console[methodName];`, before the switch and bound by
  `function::bind(method, Some(&console), &new_args)`, printed
  `method.bind(console, ...newArgs)`, where upstream writes `const bind =
  Function.prototype.bind;`, `newArgs.unshift(console)` and
  `bind.apply(console[methodName], newArgs)`. A method that isn't a
  function throws the port's own Error (`console.x is no function`) where
  upstream's bind throws a TypeError. The non-string branch inserts at
  `Math.min(offset, newArgs.length)` (Rust's splice panics past the end,
  JS's clamps); `const format = newArgs[offset];` is read once (rd226's
  kind); and the cases' braces upstream writes without declarations are
  dropped.

- **StreamConfigWeb's `?? false`** (the user, 2026-11-01):
  createStringDecoder's `{fatal: false, ignoreBOM: preserveByteOrderMark ??
  false}` where upstream defaults the parameter, `preserveByteOrderMark =
  false`; a `null` gives `false` in both, as TextDecoder converts it.

- **ReactFlightClientDevToolsHook's last test** (the user, 2026-11-01):
  `if (hook.checkDCE) { // This is the real DevTools. return true; } ..
  return false;` with no `else`, which clippy's `needless_bool` asks of
  the Rust, where upstream writes `} else { .. return false; }`.

- **ReactFlightPerformanceTrack's forms** (the user, 2026-11-01, for the
  held draft): `env != null && !isPrimaryEnv ? name + .. : name`, a match
  with a guard, where upstream writes `isPrimaryEnv || env === undefined ?
  name : ..`; `??` where upstream writes `||` of a debugTask, which is an
  object or absent. By name (2026-11-02, Batch 56): the extra `const rootEnv
  = response._rootEnvironmentName;` before the call at its three sites
  (`let root_env = ..`).

- **ReactTestRenderer's forms** (the user, 2026-11-01, 9421ead's list):
  `{...inst.props}` then `delete props.children` for upstream's `const
  {children, ...props} = inst.props` (rd323); an `isHidden(inst)` helper
  for `inst.isHidden`, and toJSON's container children copied, `.slice()`,
  before it recurses (a borrow held across calls that may run JS); toJSON's
  renderedChildren `undefined` until its first child and returned `??
  null`, its `container.children && container.children.length` guard the
  loop; create's `__DEV__` warning's nested if merged (ci1), its flags
  tested by truth where upstream writes `=== true` and `=== false` (they
  are bools), `root: undefined` left out of the renderer object (rd327,
  since printed), and the `const` copies Rust's borrows make (unmount's
  `const current = root`, toTree's `const current = root.current`, the
  root getter's `const fiberRoot = root`); nodeAndSiblingsArray,
  getChildren and wrapFiber naming a field before reassigning it, and
  wrapFiber returning in each branch; flatten's `while ((n = stack.pop())
  != null)`, `$index(n.array, n.i)` and `n.i = (n.i + 1) >>> 0`; findAll's
  `for (const child of root.children) { if (typeof child !== 'object')
  continue; .. }` for upstream's `forEach` testing `typeof child ===
  'string'`, and `$extend(results, ..)` for `results.push(...)` (rd183's
  form); `export function unstable_batchedUpdates(fn, a) { return
  batchedUpdates(fn, a); }` for `batchedUpdates as
  unstable_batchedUpdates`, and defaultOnDefaultTransitionIndicator a
  function returning nothing where upstream passes `noop`; and toTree's
  HostRoot and HostPortal as one arm.

- **The ReactFlightClientConfig forks stay JS** (the user, 2026-11-01), as
  the reconciler's ReactFiberConfig forks do: `export *` re-exports of the
  configs and the `$$$config` shims.

- **The Flight client's forms** (the user, 2026-11-01): ResponseInstance,
  the constructor function `Class::of(function::with_this(..))` makes, ends
  `return this;`; `missingCall` takes the parameters of the callback it
  stands in for, where upstream's takes none; the fake function fallback's
  parameter upstream names `_` is `callback` (Rust can't name a parameter
  `_` and read it); and an object literal's key whose value is `None` is
  left out, where upstream writes the key with `undefined` or `null` and
  nothing tells the two apart (rd391). Of that kind, startAsyncIterable's
  closed iterator result is `{done: true}`, where upstream writes `{done:
  true, value: undefined}` (rd526). FlightStreamController's arrow
  properties, the old variant's keys left on a chunk and ReactPromise's
  constructor of upstream's parameters were accepted before.
- **The Flight client's constructors as values** (the user, 2026-11-01):
  an Error-family constructor used as a value, `Error.bind(null, ..)`,
  prints as the class itself (rd404); the typed-array constructors
  resolveTypedArray is handed share one type,
  `Class<fn(&ArrayBuffer, f64, f64) -> &'static ArrayBufferView>` (rd410).
- **ReactART's `updateContainerSync(.., null)`** (the user, 2026-11-01):
  updateContainerSync's `callback` stays nullable, as two of upstream's
  three callers pass `null`, so ReactART's call prints a trailing `null`
  where upstream leaves the argument out. Its type is `?Function`, and the
  reconciler tests it `!= null`.
- **The Flight client's chunk state** (the user, 2026-11-01), the chunk's
  `RefCell<ChunkState>` read as Rust's borrow rules allow:
  - a value or reason is copied out of the chunk's state before a listener
    or callback is called, `settled = {TAG: 'Ok', _0: chunk.value}` in
    `then`, and the helpers `initializedValue(chunk)` and
    `erroredReason(chunk)` read it for a moment, where upstream reads
    `chunk.value` and `chunk.reason` while the code it calls runs (a
    `borrow()` held across the call would panic if the listener wrote the
    chunk);
  - a blocked chunk's listener list is made `chunk.value ??= []`, where
    upstream writes `if (chunk.value === null) chunk.value = [];` (the list
    is `Option<Vec<Listener>>`, `get_or_insert_with`);
  - createErrorChunk makes `new ReactPromise('rejected', undefined, error)`,
    where upstream passes `null`: the Errored variant has no value, and
    its three writes of status and reason are upstream's;
  - `streamController(chunk)` panics "Expected a stream chunk." where
    upstream's `chunk.reason.enqueueValue(..)` would throw a TypeError of a
    chunk that isn't a fulfilled stream (it never is one, by the code that
    makes them);
  - resolveStream passes `stream` to wakeChunk, where upstream passes
    `chunk.value`, the same value it has just written;
  - rd435 (the user, by name, 2026-11-02): a chunk's status tag prints as
    its literal, `chunk.status === 'fulfilled'`, `case 'resolved_model':`,
    where upstream names its constants (`INITIALIZED`, `RESOLVED_MODEL`,
    ..); the eight status constants are not in the module.
- **Flight's async debug info** (the user, 2026-11-02):
  ReactFlightServerConfigDebugNode's `pendingOperations` is always
  `new Map()`, where upstream makes it `__DEV__ ? new Map() : null`: every
  read is DEV-only, so production allocates one empty map nothing reads.
- **The Flight reply server's names and types** (the user, 2026-11-02),
  each gap left open for rust-js:
  - a chunk's `then` is named `value`, where upstream's is
    `reactPromiseThen`, and FlightIterator's `[ASYNC_ITERATOR]` method an
    unnamed function where upstream's is `asyncIterator`: nothing reads a
    name (rd480, rd488);
  - initializeModelChunk reads `chunk.reason[RESPONSE_SYMBOL]` where
    upstream destructures it, a symbol-named field's pattern not yet
    rust-js's (rd480);
  - a stream controller's `error`, called off the object
    (`const error$1 = controller.error; error$1(error)`), would word the
    TypeError of a chunk that isn't a stream's `error$1 is not a function`,
    on a path only a bundler's thenable calling both its callbacks reaches
    (rd488);
  - createResponse's `backingFormData` and `arraySizeLimit` are declared
    `unknown`, where upstream's are `FormData` and `number`: each is told
    `undefined` from `null` as upstream's defaults are.
  - chunk statuses print as string literals, `chunk.status ===
    'fulfilled'`, `case 'resolved_model':`, where upstream names constants
    (PENDING, BLOCKED, INITIALIZED, ..), the user accepting it by name
    (rd435).
- **react-server-dom-webpack's server (the user, 2026-11-02)**:
  ReactFlightWebpackReferences, ReactFlightServerConfigWebpackBundler and
  ReactFlightDOMServerBrowser, -Edge and -Node are Rust; the plain
  re-export entries `react-flight-dom-server.*.js` stay JS (rd55), as do
  ReactFlightWebpackPlugin, NodeLoader and NodeRegister. An entry calls
  ReactFlightServer's functions through `use
  react_server::react_flight_server::{..}`, its bundler's client manifest
  given as `js::unknown(..)` (Parcel's `None`, upstream's `null`).
  These differences from upstream are accepted, each gap left open:
  - `options?.x` where upstream writes `options ? options.x : undefined`,
    and `!= null` where upstream tests `!== undefined` or truth (an option
    given as `null` is absent, ReactDOMRoot's kind);
  - each error message is one template literal where upstream
    concatenates its parts;
  - decodeReplyFromAsyncIterable's `iterable[Symbol.asyncIterator]()`,
    `js::AsyncIterable::async_iterator`, where upstream writes
    `iterable[ASYNC_ITERATOR]()`;
  - bind's and registerServerReference's descriptor locals are `typeof$`
    and `id` where upstream names them `$$typeof` and `$$id`, and their two
    property maps are two `Object.defineProperties(..)` calls under `__DEV__
    ? .. : ..`, where upstream passes `__DEV__ ? {..} : {..}` to one;
  - bind's bound arguments are `[...bound, ...args]` where upstream writes
    `this.$$bound.concat(args)`;
  - prerender's and prerenderToNodeStream's request, and the web streams'
    writable, made after the closures that read them, are read
    `$unwrap(request)` (the Fizz entries' form);
  - decodeReplyFromBusboy copies a file's queued fields out before
    resolving them, `current.queuedFields.slice()`, so no `RefCell` borrow
    is held across resolveField; its file handle's borrows are counted,
    `$borrow(current.file, true)` (rd590);
  - the fake Writables (of a WebSocket, a web stream's controller and a
    Node Readable) are object literals of `write`, `end` and `destroy`,
    cast to a `Writable` as upstream's `as any`;
  - types: Node's `debugChannel` is `unknown`, told apart by what it has as
    upstream does, where upstream types `Readable | Writable | Duplex |
    WebSocket`; `environmentName` is an untagged `string | (() => string)`;
    `arraySizeLimit` is `unknown`, as createResponse's; and
    registerServerReference's `exportName` is `unknown`, so `exportName ===
    null` keeps a missing name's `id#undefined`;
  - readability gaps: rd582 (a conditional's text named after a callee,
    `readPartialStringChunk$1`), rd583 (`$someValue(value)` of a read),
    rd587 (no `export type {..}` of a re-exported type), rd589
    (getReference's `switch` of a string or a symbol is a string `switch`
    and an `if` of each symbol; `then`'s function is unnamed, `function
    (resolve, _reject)`), rd591 (`readable.on('end', () => onClose())`
    where upstream passes `onClose`), rd592 (`import {Readable} from
    'stream'` last) and rd593 (ReactFlightReplyServer's createFileFromHandle
    writes `handle.filename ?? 'undefined'` and two `append`s, webapi's
    `new File` naming by `&str` and FormData's optional file name a second
    method).
- **react-server-dom-{turbopack,parcel,unbundled,esm}'s servers (the user,
  2026-11-02)**: each package's References, its bundler config
  (ReactFlightServerConfig*Bundler) and its ReactFlightDOMServer entries
  (Browser, Edge and Node, as the package has them) are Rust, on webpack's
  port; the `react-flight-dom-server.*.js` re-exports stay JS (rd55), as do
  the plugins and the Node loaders and registers. Turbopack's and
  unbundled's are webpack's with their names (Turbopack's References add
  registerServerObjectReference); Parcel keeps the manifest
  registerServerActions is given, `let serverManifest = {};`, and adds
  decodeAction, decodeFormState and loadServerAction on it; ESM's manifest
  is its module base path, a `string`. react-server's ReplyServer and
  ActionServer take each bundler's server manifest as it is:
  createResponse, decodeAction and decodeFormState of a type parameter `M`,
  so each entry gives its own (webpack's and Turbopack's client configs'
  `ServerManifest`, Parcel's, ESM's `string`, and react_server's
  `ServerManifest` for unbundled, whose client config is still JS); the
  response keeps it as the `unknown` only resolveServerReference reads.
  Accepted, each gap left open:
  - every item of react-server-dom-webpack's server's list above, where the
    same code is;
  - ci1: registerServerObjectReference's `if (__DEV__ && typeof reference
    === 'function')`;
  - rd594: registerServerObjectReference's `$$id` value is a `const value
    = ..` before `Object.defineProperties(..)`, where upstream writes the
    conditional in the literal;
  - rd595: Parcel's ClientReference's `$$importMap` is declared `|
    undefined` alone in the `.d.ts`, though createClientReference stores
    the `null` it may be given (the JS is upstream's);
  - rd596: loadServerAction's middle promise, `.then(() =>
    preloadModule(reference))`, is typed of Parcel's thenable itself in the
    Rust, js's `Fulfils` having none of a thenable that isn't a `Promise`
    (the JS is upstream's).
- **react-markup (the user, 2026-11-02: its forms accepted)**: its four modules
  are Rust, a crate of their own (`markup.rs`); the `*.stable.js` entries,
  which only throw, stay JS, as the re-export entries do (rd55).
  ReactFizzConfigMarkup is ReactFizzConfigDOMLegacy's form: a `pub use` of
  ReactFizzConfigDOM's items for upstream's `export {..} from`, and its
  own functions, pushStartInstance's `for (const propKey in props)` as
  `js::keys_in` and `HAS_OWN_PROPERTY`. ReactMarkupLegacyClientStreamConfig's
  `StringDecoder` is upstream's `null` alone, an untagged enum, as
  Parcel's `ModuleLoading` is. The two entries give Fizz, and Flight's
  server, react-dom-bindings' legacy `Destination` (`push`, `destroy`),
  as upstream's markup stream config does; the server entry reaches
  ReactSharedInternalsServer, still JS, by its JS path, and swaps its
  `getCurrentStack` for the client's around the user's `onError` with a
  guard whose `Drop` is upstream's `finally`. Port differences, none
  changing a result:
  - ReactMarkupServer makes its two destinations after the two requests,
    as Rust makes a closure after what it captures, where upstream's
    closures read `fizzRequest` and `flightRequest` declared after them
    (ReactDOMDefaultTransitionIndicator's order);
  - the finished `push(null)` resolves `buffer.take()`, `const old =
    buffer; buffer = ''; resolve(old);`, where upstream resolves `buffer`
    and keeps it: nothing reads it after (rd654:
    `resolve(buffer.borrow().clone())` is refused, a borrow held across
    `resolve`, ADR 0863, though `resolve` of a string runs no crate code);
  - noServerCallOrFormAction is generic, `(id, args)`, given as both the
    Flight client's callServer and encodeFormAction;
  - stashErrorIdx is a number, as upstream's, its text `String(stashErrorIdx)`
    and its increment a statement of its own, where upstream writes `'' +
    stashErrorIdx++` (rd653 fixed in rust-js f87ccdf31);
  - the webpack server entries' forms: `options?.x` and `!= null`, error
    messages as template literals, trailing `undefined`s left out, `null`
    arguments of an `Option` given `undefined` (createFlightResponse's
    first three, as the `.d.ts` types them);
  - ReactFizzConfigDOMLegacy's: the `export {..}` list in rustfmt's order,
    `return;` and `return writePreambleStartImpl(..)` as statements (rd364),
    and `export const isPrimaryRenderer`/`supportsClientAPIs` after the
    imports, where upstream declares them between two (rd652).
  Readability gaps (rust-js), not holding (rd557, a hoisted closure's
  comments, fixed in rust-js 4b8d58215): rd558 (an `Option` a
  `HashMap` holds is boxed, `$someValue($unwrap($someGet(stashedErrors,
  id), ..))` for `stashedErrors.get(id)`); rd652 and rd654 above (rd650,
  the entries' imports from './ReactFizzConfigMarkup.js', and rd651, the
  `ReactSharedInternalsClient` alias, fixed in rust-js a6797eac5 and
  d72ed8bb3); and `getCurrentStack = .. ?? null`, the Fizz server's
  form of a nullable field set from an `Option` (no id).
- **The server side's import groups: rustfmt's blank lines** (the user,
  2026-11-03, keeping rustfmt's defaults, with `reorder_imports = false`
  offered): since rust-js rd637/rd652 an import's blank lines are its
  `use`'s, and a server file's `use`s keep upstream's import order, which
  rustfmt would sort within a group; so where upstream writes two imports
  in one group that rustfmt would reorder, the port keeps a blank line
  between their `use`s and the JS between its imports. Accepted by name:
  ReactFizzServer's 8 (NewContext/Hooks/AsyncDispatcher,
  TreeContext/CurrentTask/CallUserSpace,
  ReactSymbols/SharedInternals/FeatureFlags, noop/getComponentNameFromType,
  ReactRecoverable/ReactFizzThenable), ReactFizzHooks' 4,
  ReactFizzClassComponent's 2, one each in ReactFizzLegacyContext,
  ReactFizzThenable, ReactFlightHooks and ReactFlightReplyServer,
  ReactServerStreamConfigNode's util/crypto and ReactFizzConfigDOM's 3. The
  imports of a config fork (ReactFizzConfig, ReactFlightServerConfig,
  ReactServerStreamConfig, react-client's ReactFlightClientConfig) come from
  bindings no `use` places, so no blank line moves them. A comment above a
  `js::import!` is dropped (rust-js rd666).
- **React DOM's Bun server (the user, 2026-11-03: port Bun now)**:
  react-server's ReactServerStreamConfigBun and react-dom's
  ReactDOMFizzServerBun are Rust; the `react-dom-server.bun.js` and
  `.bun.stable.js` re-exports stay JS (rd55). Bun's own APIs are
  `bindings/bun`, `@rust-js-bindings/bun`, bun-types 1.4's `Bun.hash` and
  its direct stream (`DirectUnderlyingSource`, which implements webapi's
  `IntoUnderlyingSource`, as bun-types overloads `new ReadableStream`, and
  `ReadableStreamDirectController`), laid out as `bindings/busboy` is. The
  stream config's `Destination` is upstream's union,
  `BunReadableStreamController | (Writable & MightBeFlushable)`, a type of
  the module's own read by what the two have alike (`write`, `end`, and
  `flush`, `error`, `destroy` and `close` asked by `js::is_function`), as
  webapi's `ReadableStreamController` is; its `.d.ts` writes upstream's
  union. writeChunk tests `chunk.length === 0` by `js::get`, as
  TypeScript's `string | ArrayBufferView` has no `length` (a DataView's is
  `undefined`, so even an empty one is written, as upstream's);
  writeChunkAndReturn is `js::truthy(destination.write(chunk))`, `!!`;
  createFastHash gives `Bun.hash`'s `bigint`, a `u64`. No upstream test
  runs either module: the port's `ReactServerStreamConfigBun-test.js` (10
  tests, each function on fake destinations and a fake `Bun.hash`) and
  `ReactDOMFizzServerBun-test.js` (7 tests, renderToReadableStream on a
  fake direct `ReadableStream`: the source and strategy it makes, the
  shell, a boundary and `allReady`, a shell error, a cancel's and a
  signal's abort, `onHeaders`, and a form state's key hashed by
  `Bun.hash`) hold them, and pass on upstream's JS too. Differences from
  upstream, none changing a result:
  - the entry's forms are ReactDOMFizzServerEdge's: `options?.x` and `!=
    null`, `const signal = options?.signal; if (signal != null)`, the
    request read `$unwrap(request)` of a `let` set after createRequest
    (rd456), `pull(controller) {..}` and `cancel(reason) {..}` methods
    where upstream writes arrows (rd458), `onAllReady = () => { res(); }`
    (rd599), `export function` (rd350), the imports' names in rustfmt's
    order, and `export {default as version} from 'shared/ReactVersion'`;
  - readability gaps (rust-js), not holding: rd664 (readAsDataURL names
    `const type = blob.type;` before `type || 'application/octet-stream'`,
    where upstream reads `blob.type ||` in place; ReactServerStreamConfigNode's
    alike) and rd665 (the strategy prints across three lines, `{
    highWaterMark: 2048, }`, as rustfmt lays out its struct, where upstream
    writes `{highWaterMark: 2048}`; Edge's, Static's and the Flight
    entries' `{highWaterMark: 0}` alike).
- **The Flight clients' names and arms** (the user, 2026-11-01; its
  acronym rule extended to the servers, the user, 2026-11-02, ESM's
  resolveClientReferenceMetadata's `baseUrl` and `fullUrl`):
  - readChunk's default arm is split by variant, `throw chunk.reason` of
    an errored chunk and `throw null` of a resolved module's, where
    upstream's one `default: throw chunk.reason;` reads a reason that is
    `null` for the second: the same throw, but the variants type their
    reason differently;
  - a local or parameter whose upstream name holds an acronym is
    camelCased as Rust's snake_case gives it, `iteratorResultJson`,
    `rowId`, `findSourceMapUrl`, where upstream writes
    `iteratorResultJSON`, `rowID`, `findSourceMapURL` (rd272's kind;
    fields and exports keep upstream's names through `#[rust_js::name]`);
  - a local upstream names `$$typeof` is `typeof$`, as Rust can't name a
    binding with `$`;
  - ReplyClient's `retry` is an arrow, `() => {..}`, where upstream writes
    `function () {..}`: it reads no `this` or `arguments`.
- **ReplyClient's forms** (the user, 2026-11-01):
  - encodeFormData's `resolve` and `reject` are captured as `Option`s the
    promise's executor fills, `let resolve; let reject;` then
    `const resolve$1 = $unwrap(resolve);` once `new Promise(..)` has run
    it, where upstream calls the variables the executor wrote;
  - parseStackLocation reads `error.stack ?? ''`, where upstream reads
    `error.stack` (a `string` it never finds missing);
  - the lazy case's `finally { pendingParts--; }` is a statement after
    the try/catch, which each of their paths reaches, each returning
    the value it made after it.
- **react-art's event handler** (the user, rd335, 2026-11-02): the closure
  ReactFiberConfigART returns as a node's listener prints as an anonymous
  function, where upstream names it `function handleEvent`.
- **ReactFizzServer's forms** (the user, 2026-11-02), each but the last two
  a filed rust-js gap; none changes a result:
  - a `try` whose result is read after it is a `$catchUnwind` closure,
    `const result = $catchUnwind(() => rerenderStalledTask(request,
    task)); restoreThenableState(); .. if (result.TAG === 'Err') { throw
    $unwinding(result._0); }`, where upstream writes `try {..} finally
    {..}`;
  - a `RefCell` collection, a segment's chunks and children, a replay's
    nodes, an abort set, is read through `$borrow(..)` and
    `$borrowMut(..)`, `$borrowMut(abortSet).delete(task)`, where upstream
    reads the array or set itself;
  - an `Option` is tested `!= null` where upstream tests `!== null` or
    `!== undefined` (ReactDOMRoot's kind);
  - an index's `usize` arithmetic is `>>> 0`, `(((totalChildren - 1) >>>
    0) - n) >>> 0`, where upstream writes `totalChildren - 1 - n`;
  - `for (i = 0; i < pingedTasks.length; i++) {..} pingedTasks.splice(0,
    i);` is `while (i < pingedTasks.length) {..; i++; }
    $drain(pingedTasks, 0, i);`, and completedSegments' alike;
  - the port's helpers upstream doesn't have: `thrownValueOf`,
    `fallbackKeyPathOf`, `isBackwards`, `isSuspending`,
    `isSuspenseException`, `isCurrentTask`, `nextSegmentId`,
    `newThrownInfo` and `componentNameOrUnknown`;
  - a resume slot is read by its index's text, `resumeSlots[String(i)]`
    and `delete resumeSlots[String(i)]`, where upstream indexes by the
    number;
  - a task object has no `ping` property (rd495, the user's decision of
    2026-11-01, "Pings made at then()"): `pingOf(request, task)` makes its
    resolve and reject where they're given to `then`.
  - an entry's `onAllReady = () => { res(); }`, where upstream gives `res`
    itself: the server's `Rc<dyn Fn()>` of a `Promise<()>`'s resolve, a
    readability difference that keeps Rust's behavior, as `res` reads an
    argument a JS caller may give where the closure gives none (the user,
    2026-11-02, rd599).
  - ReactDOMServerPartialHydration's `[GATED, SHOULD FAIL] renders a hidden
    LegacyHidden component` trips jest's loop guard ("exceeded 6000
    iterations") escaping a DEV error stack, whose length is the
    checkout's path's and the `$catchUnwind` frames': a known gate failure,
    gone with the first item's gap (the user, 2026-11-02).
- **ReactFlightServer's forms (the user, 2026-11-02)**: ReactFlightServer
  is Rust. Its `Request` and `Task` are Rust structs held by `Rc`, their
  mutable fields `Cell`s and `RefCell`s, as Fizz's are; ReactFlightHooks,
  ReactFlightAsyncDispatcher, ReactFlightServerConfigDebugNode and the
  react-server-dom-* entries `use` its items. A request's `onAllReady` is
  private to react-server, as Fizz's is (rust-js's ADR 0626: a public
  field's callback may be cloned out and outlive the request, so a
  prerender's, which holds the request's `Rc<OnceCell<Weak<..>>>`, isn't
  owned by createPrerenderRequest's return). A task's `debugOwner`,
  `debugStack` and `debugTask` hold what the element gave,
  `Option<&'static Unknown>`, and upstream's `!== null` tests of them are
  `js::classify`'s `Kind::Null`, so a production element's `undefined`
  passes them and throws where upstream's does
  (`ReactFlightGivenDebugValues-test.js`). Fizz's forms above are accepted
  here too, where the same code is: a `try` whose result is read after it
  as `$catchUnwind`, `$borrow(..)` and `$borrowMut(..)` of a `RefCell`
  collection, `!= null` of an `Option` where upstream tests `!== null` or
  `!== undefined`, the pinged tasks' `while` and `$drain`, and an entry's
  `onAllReady = () => { res(); }` (rd599). Also accepted, none changing a
  result:
  - upstream's `const stringify = JSON.stringify;` is a function,
    `stringify(value)`, which throws what `JSON.stringify` throws, of a
    model that's never `undefined`;
  - `ObjectPrototype` is `Object.prototype`, read where it's compared,
    where upstream names it once;
  - the request's `filterStackFrame` is called with four arguments,
    `filterStackFrame(url, functionName, lineNumber, columnNumber)`, as
    upstream's calls are, where upstream's option type takes two;
  - `debugTask.run(callback.bind(null, arg))` and
    `componentStorage.run(.., fn.bind(null, ..))` are arrows, `() =>
    callback(arg)`;
  - `voidHandler` is a function declaration, `function voidHandler() {}`,
    where upstream's is `const voidHandler = () => {};`;
  - emitChunk's root, upstream's `{'': value}`, is made as `const root =
    {}; root[''] = value;`, as Rust writes no literal of a key that's no
    identifier;
  - an array the port holds in a `RefCell` (a component's tracked
    thenables, a component info's stack, a blob's model) is copied before
    a call that may run JS, where upstream iterates it in place, so no
    borrow is held across the call;
  - the port's helpers and structs upstream doesn't have: `stringify`,
    `taskDebugStackTrace`, `taskDebugOwnerInfo`, `thrownValueOf`,
    `isSuspenseException`, `keyedFragment`, `hasDisplayName`,
    `forwardPromiseDebugInfo`, `pingOf`, `isElementType`, `doNotLimit`,
    `doNotLimitAdd`, `doNotLimitHas`, `writtenReferenceOf`,
    `describeErrorValue`, `isNotPlainImportMetadata`,
    `notPlainImportMetadata`, `unknownValueMessage`,
    `isNextTwoChunksAreAtomic`, and the types `Counter`,
    `DeferredDebugStore`, `LazyThenable`, `MultiShotIterable`,
    `MultiShotAsyncIterable`, `FragmentProps`, `Found`, `Visited`,
    `AsyncInfo`, `ServerObjectReferenceMetadata`, `ServerReferenceMetadata`,
    `ServerReferenceMetadataDev`, `DebugBlobReading`, `BlobReading`,
    `ErrorValueInfo`, `ComponentDebugInfo`, `IoDebugInfo`,
    `ConstructorReference`, `AsyncDebugInfo` and `AbortedAsyncInfo`;
- **Fizz's and Flight's `Request` is a class, `RequestInstance`** (rd494,
  rust-js rd554; the user, 2026-11-02): `fn new(..) -> Rc<Self>` is
  upstream's constructor function, `function RequestInstance(..) {
  this.destination = null; .. }`, and each `Request::new(..)` is `new
  RequestInstance(..)`. Its `.d.ts` type is `RequestInstance`, ADR 0761's
  one name of a class, where upstream's Flow type is `Request`; ReactFizzHooks
  and ReactFlightHooks import it by that name.
- **ReactDOMFlightServerHostDispatcher, ReactFlightReplyBackingFormData and
  react-server's ReactSharedInternalsServer are Rust** (the user,
  2026-11-02: the forms below accepted; createBackingFormData is upstream's
  `formData.forEach(..)`, as webapi's `forEach` gives values a callback may
  keep, rust-js rd574). Each is used: the first by
  every DOM Flight server's ReactFlightServerConfigDOM, which imports it to
  install its dispatcher; the second by ReactFlightReplyServer; the third
  by ReactFlightServer, react-markup and the `react-server` builds'
  `shared/ReactSharedInternals` fork. None is a config shim.
  - The host dispatcher's hints are ReactFlightServerConfigDOM's `Hints`,
    a `RefCell<HashSet<String>>` (the request's `Set`), read `unsafe {
    js::cast(Some(get_hints(&request))) }` because ReactFlightServer's
    `getHints` gives its fork's opaque `Unknown`; a generic `get_hints<H>`,
    as Fizz's `get_resumable_state<S>` is, would take the cast away.
  - preload and preloadModule are exported, and ReactFlightServerConfigDOM
    calls them with what its props hold, so their `href` and `as` are any
    value, tested `typeof href === 'string'` as upstream does; the
    dispatcher's `L` is `L(href, as, options) { preload(href, as, options);
    }` where upstream gives `preload`, as the shared `HostDispatcher` types
    `L`'s `href` and `as` as text. The other dispatcher functions take
    text, as `HostDispatcher` types them, so upstream's `typeof href ===
    'string'` wrappers are gone (prefetchDNS's `&& href` is `if (href)`),
    their bodies one level out.
  - preload's fallback, without a request, calls the previous dispatcher
    only of an `as` that's text (`else if (typeof as === 'string')`),
    where upstream passes any `as` on: `HostDispatcher`'s `L` takes text.
    Only processLink's `props.as` is anything else, and processLink runs
    inside a render, which has its request; the previous dispatcher there
    is react-dom's no-op or Fizz's, which does nothing of a missing `as`.
  - each `return emitHint(..)` is `emitHint(..)`, a call of nothing to
    return; `request != null` and `trimmed != null` where upstream tests
    truth; preconnect's key is `${crossOrigin ?? 'null'}`; getImagePreloadKey
    tests `imageSrcSet` by truth after `typeof .. === 'string'`, where
    upstream writes `!== ''`; the dispatcher object's keys have no
    `/* prefetchDNS */` comments (Fizz's form).
  - BackingFormData is a struct of `RefCell` and `Cell` fields, each
    `None` `null` (`nullable`), its maps `HashMap`s. A field made when
    first needed is `backingStore.blobs ??= new Map(); const blobs =
    backingStore.blobs;` (`get_or_insert_with`), where upstream writes
    `let blobs = ..; if (blobs === null) { blobs = backingStore.blobs = new
    Map(); }`. Its doc comment is the `.d.ts`'s, on `BackingFormData`.
    getStreamChunks gives a copy, `value.slice()`, where upstream gives the
    array (ReactFlightServer's copied-before-JS-runs form).
    consumeBackingEntries names the stripped key, `const name =
    key.slice(formPrefix.length);`. ReplyServer's `resolveFile` takes a
    `&'static File`, which the store keeps.
  - ReactSharedInternalsServer's check reads
    `React.__SERVER_INTERNALS_DO_NOT_USE_OR_WARN_USERS_THEY_CANNOT_UPGRADE
    == null` where upstream tests `!ReactSharedInternalsServer`, and its
    message is one literal. Its default export is
    `Option<&'static SharedStateServer>`, as the binding may be
    `undefined`; ReactFlightServer keeps its JS-path binding of it, typed
    `&'static SharedStateServer` (the module throws before it's
    `undefined`).
  - createBackingFormData's callback is an arrow, `formData.forEach((value,
    key) => { appendBackingEntry(backingStore, value, key); })`, where
    upstream writes `formData.forEach(appendBackingEntry.bind(null,
    backingStore))`: Rust's closure, which converts each value to the
    store's own.
  - a kept entry is the port's `BackingEntryValue`, `Str(String)` or
    `File(&'static File)`, not webapi's `FormDataEntryValue<'static>`:
    ReplyServer's resolveField gives the store text it's only lent, `&str`,
    which an owned `String` keeps (rd559, closed 2026-11-02: the idiomatic
    store, no gap).
  - gaps: rust-js's readability gaps rd657 (a block
    comment before a call's first argument on its own line is placed
    after it, `previousDispatcher.C(href, /* preconnect */ crossOrigin)`,
    or before the statement of a call of one argument), rd658 (a `let` of
    a `RefCell` field's borrow is read as the field, its name lost:
    `backingStore.stringChunks.get(key)` where upstream names `const
    stringChunks`), rd659 (a static renamed by `rust_js::name`, read in a
    `js::on_load!`, re-reads its namespace's member, not the `const` it's
    declared as) and rd660 (a closure that only gives its arguments on,
    `|h, a, o| preload(Some(js::unknown(h)), ..)`, could be the function
    itself, upstream's `L: preload`, and createBackingFormData's arrow,
    upstream's `appendBackingEntry.bind(null, backingStore)`); rd655 (ReplyServer's
    `(controller.error)(error)`, a `Rc<dyn Fn>` field only the crate's own
    closures fill, which read no `this`, prints `(0, controller.error)(error)`,
    rd579's form, where upstream writes `controller.error(error)`; fixed in
    rust-js 6dd3eee3d, its JS regenerated with rd574); rd663 (webapi's
    observer and timer callbacks are still lent their arguments, where
    `forEach`'s are JS's own, `'static`, since rd574); rd661 (a module whose load throws of
    a `None` export still exports its `Option`, so ReactFlightServer and
    react-markup bind ReactSharedInternalsServer by its JS path rather than
    `use` it).
- **Accepted on the user's behalf while away, 2026-11-02** (each changes no
  result; the coordinator logs them for the user):
  - the Flight client's DEV `then` holds its wrapper promise in a cell its
    executor's callbacks read, `let wrapper; .. wrapper = wrapperPromise;`
    and `$unwrap(wrapper)`, where upstream's callbacks read
    `wrapperPromise` itself (Rust can't name the promise inside the
    executor `new Promise` runs);
  - that `then` attaches `wrapperPromise.then(onFulfilled, reason =>
    {..})`, or `wrapperPromise.then(onFulfilled)` where it's given no
    `reject`, where upstream writes `wrapperPromise.then(resolveCallback,
    rejectCallback)` (the binding takes no optional handler, rd516): a
    missing `reject` leaves the rejection unhandled either way;
  - clippy's `collapsible_match` makes a `match` arm's nested
    `if __DEV__` a guard, `"4" if __DEV__ => ..`, as ci1 merges nested ifs.
- **The Flight client's names and parameters** (decided for the user while
  away, 2026-11-02; neither changes a result):
  - its functions and module-level constants keep upstream's acronyms
    through `#[rust_js::name]`, `createFakeJSXCallStackInDEV`,
    `getIOInfoTaskName`, `hasGCedResponse`, `markIOStarted`, as its fields
    and exports do (locals stay camelCased, the entry above);
  - createResponse keeps `#[allow(clippy::too_many_arguments)]` for
    upstream's 14 positional parameters, and createFakeFunction and
    logComponentInfo for their eight: the parameters are upstream's API;
  - the DEV-only `createFakeJSXCallStackInDEV` and
    `replayConsoleWithCallStackInDEV`, `null` outside DEV as upstream's
    are, are called through `unwrap_unchecked` where every caller runs in
    DEV, so the JS calls the constant as upstream does.
- **The Flight client's line-by-line read, accepted on the user's behalf
  while away, 2026-11-02** (Batch 52; each changes no result a program
  sees; the gaps found are rd518-rd529):
  - functions given as a callback take the callback type's parameters,
    `missingCall(id, args)`, `createBlob(response, model, parentObject,
    key)` and createMap, createSet, createFormData, extractIterator, where
    upstream's take fewer: only their `.length` differs, which nothing
    reads;
  - a closure field of an object literal is a method, `react_stack_bottom_frame(..) {..}`
    and a property descriptor's `get() {..}` and `set() {}`, where upstream
    writes `name: function (..) {..}`: each is called, never constructed;
  - resolveConsoleEntry's listeners unwrap the entry, `$unwrap(v)`, so a
    server's malformed entry panics where upstream's destructuring throws a
    TypeError (an unchecked unwrap of the server's data would be unsound);
  - createElement writes `key: key ?? null` and `props: props ?? null`,
    where upstream passes them as given: a server's element tuple always
    has both;
  - triggerErrorOnChunk's and resolveStream's DEV blocks set
    `initializingChunk` without upstream's `(enableProfilerTimer && ..) ||
    __DEV__` test, which is true inside them;
  - a closure that passes its argument on to a function of captured values
    prints as that function's `bind`, `triggerErrorOnChunk.bind(null,
    response, rejectedChunk)` in resolveModule, where upstream writes the
    arrow (rust-js's one form, which initializeIOInfo's upstream writes);
  - processFullStringRow reads a hint's code `row[0] ?? ''`, where
    upstream's is `undefined` of an empty row: dispatchHint matches
    neither;
  - initializeIOInfo and flushComponentPerformance switch on
    `promise.status`, where upstream names the promise first, `const
    thenable: Thenable<mixed> = promise as any;`: the port's `js::cast` to
    shared's `Thenable` prints no alias (Batch 53).
- **The Flight client's stream source is webapi's** (2026-11-02): its
  local `UnderlyingSource` stand-in is gone; startReadableStream gives
  `readable_stream::new` webapi's `UnderlyingSource { r#type, start, ..
  }`, its controller a `ReadableStreamController` (rd403, rust-js
  7f2381b7f), printed `new ReadableStream({type, start(c) { controller =
  c; }})` as upstream.

The audit of 2026-11-01 (Batch 41) listed these; the user accepted them the
same day:

- **Blank lines are the Rust's** (layout): the JS has a blank line where the
  Rust has one, and rustfmt drops some upstream has (a block's first line, two
  in a row). Where the Rust can naturally write upstream's blank line, between
  two statements or items, the port writes it.
- **rd350, `export function` inline** (layout): a `pub fn` prints `export
  function name(..)` where it is declared, where upstream declares a plain
  `function` and lists it in an `export {..}` at the module's end.
- **ReactDOMDefaultTransitionIndicator's order** (layout): startFakeNavigation
  is declared before handleNavigateComplete and the listeners are added after
  both, as Rust makes a closure before the one that captures it; upstream
  relies on hoisting.
- **`args.slice()`** for `args.slice(0)` (the ReactClientConsoleConfigs'
  `to_vec()`).
- **ReactDOMSharedInternals' `f: () => undefined`** for `f: noop` in the
  DefaultDispatcher (`f`'s type gives `Option<bool>`; the client replaces `d`
  when it loads).
- **ReactDOMRoot and ReactDOMClient's `options ?? null`** for upstream's
  ternary, and hydrationCallbacks being `options` itself.
- **ReactTestUtils' `if (!didWarnAboutUsingAct)`** for `=== false` of a bool.
- **createReactNoop's childToJSX copies the children**, `child.slice()`,
  before it recurses (a borrow held across calls that may run JS), as
  ReactTestRenderer's toJSON does.
- **SchedulerPostTask's forms**: `const {performance: perf, setTimeout} =
  window;` for two consts; postTask given block arrows, `() => {
  runTask(..); }`; `_postTaskPriority` named back with `rust_js::name`; and
  `typeof scheduler.yield !== 'undefined'` for `!== undefined` (opt1's kind).
- **react-dom shared's four**: `unstable_batchedUpdates` declared under its
  exported name where upstream exports `batchedUpdates as`; createPortal's
  `key` without `= null` (createPortalImpl treats undefined and null alike);
  the error descriptors a typeof switch, not a nested conditional; and
  ReactDOMTypes' `string | undefined` for Flow's `?string`.
- **ReactFiberConfigDOM's Observer, onUnsuspend and callback**: Observer an
  untagged enum compared by `js::unknown(a) != js::unknown(&b)`;
  onUnsuspend and onUnsuspendImg `const x = function () { const state =
  this; .. }` thread-locals; requestPostPaintCallback passing `callback`
  itself for `time => callback(time)` (rd158).
- **ReactDOMComponent's forms**: `x.multiple = ..` set through `js::set` of
  an if; `isNaN(value)` and `value >= 1` through `js::cast`; setSrcObject's
  `try` as `catch_unwind` (`$catchUnwind`, rd130); `const end =
  style.length`; two-case typeof switches as if chains (ADR 0629);
  `prevValue?.__html`; `!s.is_empty()` as `s`; and
  didWarnForNewBooleanPropsWithEmptyValue `{}` in production too. Its
  `continue` at a case's end printed `break`, and the comments after `try {}
  catch {}`, are rust-js's control-flow gaps, as ConfigDOM's `$retain` is.
- **ReactDOMSelection and ReactInputSelection's forms**: isSameOriginFrame
  reading `href` and returning true in its `try`; containsNode and
  hasSelectionCapabilities as returns and if chains; `++i === x` as `i++; if
  (i === x)`; `(next = node.firstChild) === null` as a const and its test;
  `parentNode == anchorNode`; four consts for the selection's destructuring
  (r10); `?.` and `??` for `(a && a.defaultView) || window`; the ancestors'
  `while ((parentNode = ..) !== null)`; and the swap's `t`.
- **DOMPluginEventSystem and ReactDOMEventReplaying's forms**: the `for (; i
  < q.length; i += 3)` loop as a `while`; a let chain that swaps its
  branches and drops a `targetContainer !== null` test (dn1); the queued
  lists walked by index with a borrow per step; `queuedPointers.get(id)`
  without `|| null`; and `__DEV__ &&` ifs merged (ci1).
- **ReactDOMEventHandle's "Do nothing" branches** as one `if (a ||
  (enableScopeAPI && b))` (clippy's `if_same_then_else`).
- **ReactFiberConfigART's `.scale(scaleX, scaleY)`**, where upstream also
  passes originX and originY, which art's `scale` ignores.

The audit's last list (Batch 42) the user accepted on 2026-11-01 too:

- **A detached `/** */` block prints `/* */`** (ReactBaseClasses,
  ReactChildren, ReactFiberConfigART, ReactInputSelection): a block that
  documents no declaration. In Rust `/**` is a doc comment, which clippy
  (`empty_line_after_doc_comments`) and rustc (`unused_doc_comments`)
  refuse there, so the port writes `/* .. */`.
- **The small value forms:** ReactComponentInfoStack's
  getOwnerStackByComponentInfoInDev tests `owner == null` without upstream's
  `typeof componentInfo.name === 'string'` (`name` is a `String`, dn1's
  kind); ReactFiberConfigART's `props.opacity ?? 1`, appendInitialChild's
  `inject` and NO_CONTEXT declared a line before upstream's; `?? noop` for
  `|| noop` of the default transition indicator's result
  (ReactFiberAsyncAction, ReactFiberRootScheduler); and ReactDOMRoot's
  `unstable_scheduleHydration` a wrapper, `function (target) {
  scheduleHydration(target); }`, where upstream assigns `scheduleHydration`
  itself (routed to rust-js's forwarding rule; accepted until it lands).
- **`Date.now()`** in ReactOwnerStackReset and the scheduler, where upstream
  captures `const localDate = Date` as the module loads: only a `Date`
  replaced later, on a host without `performance`, tells them apart.
- **The fiber's `_debug*` fields stored with `?? null`**: `_debugOwner`,
  `_debugStack`, `_debugTask` and `_debugHookTypes` store `x ?? null` where
  upstream stores `x`, so a hand-made element without `_debugStack` or
  `_debugTask` gives the fiber `null` where upstream gives `undefined`.
  React's own elements always carry them in DEV.

- **ReactProfilerTimer's `fiber !== null`** (the user, 2026-11-02, by
  name): startUpdateTimerByLane's three `__DEV__ && fiber !== null`
  (its gesture, blocking and transition lanes) where upstream writes `fiber
  != null`: rd613's strict test of a nullable parameter each caller gives a
  fiber or `null`.

These forms are decided:

- **tr1**: test an Option with `== null` or `!= null`, never by truth. Where
  its None can only be one of the two, the test is strict: `=== undefined`
  or `=== null`.
- **Bool ternaries**: `c ? b : false` prints `c && b`, and `c ? true : b`
  prints `c || b`. `c ? false : b` and `c ? b : true` stay ternaries.
- **Matches and returned values**: a match's last arm prints as an `else`.
  A value match at a function's end is `return c ? a : b`. **rd23**:
  returned values follow pt3; if every arm is plain they make one nested
  conditional, otherwise the full if/else chain.
- **rd69**: a returned if/else that gives undefined folds to
  `c ? undefined : null` only where the other branch is null.
- **wl4**: labels the program wrote are kept. **rd109**: the continue and
  break statements the program wrote are kept, even where redundant.
- **ag5**: a conditional keeps a branch's trailing comment right after
  that branch's value.
- **rd45**: a tuple-of-if `let` prints as two lets, assigned in each
  branch.
- **Cast let**: every written `let` stays a `const`. Write casts inline
  where upstream does (`(children as any)._payload`).
- **`'' + x` is `js::concat_text(x)`** (the user, 2026-11-01): upstream's
  string concatenation of a value of any shape stays
  `js::concat_text(x)`, its `Result` unwrapped where upstream lets JS
  throw, which prints `'' + x` (ReactTestRenderer's getChildren).
- **Names**: `#[rust_js::name]` on a `let` names the JS variable
  (`$$typeof`). A `_name` parameter loses its `_` only when the body doesn't
  read it (ADR 0678).
- **ADR 0994**: WebIDL number getters are `f64`, and required number
  parameters take `impl Into<f64>`. **Collection loops**:
  `x.length() as usize` prints `x.length` (there is no `len()`).
- **rd46**: nullable attribute setters take `impl Into<Option<T>>`.
  `Math.min` keeps TypeScript's `f64`, so the port writes `?? 0`.
- **d13a**: DOM setters that TypeScript types as string but the DOM
  stringifies (setAttribute, setAttributeNS, style.setProperty, cssFloat)
  take any value. Ports use no unsafe cast for them.
- **rd128**: on* setters take `Option<&'static dyn Fn(&E)>`. The port writes
  a one-parameter noop (`function noop(event) {}`) and compares with
  `ptr::eq`.
- **rd185**: webapi's EventTarget has one `add_event_listener`, of a
  union of the listener (a Rust closure, or the JS listener a caller gives,
  its identity kept for removal) and its options as given. Every closure
  given to it annotates its parameter: `|e: &PointerEvent| ..`.
- **rd10**: `js::set_timeout`, the global, is the one timer API, and it can
  be held as a value.
- **webapi PartialEq**: webapi references compare by identity (`==` prints
  `===`).
- **rd142**: a synthetic event's prototype functions are methods bound by
  `link_name`, so `event.prevent_default()` prints `event.preventDefault()`.
- **rd64**: a factory of constructor functions is a `with_this` function
  held as a typed `Class` (`Class::of(f)`, `js::new`). **rd64c**: a Dict
  literal with a leading spread is `js::object::spread`. **rd64e**: a struct
  field's `///` prints before that field. **rd64d**: write `js::null()` where
  upstream passes null to a typed class constructor; None stays undefined.
- **rd96**: a closure that refers to itself is written with `Rc::new_cyclic`
  and `Weak`.
- **A function JS makes is `&'static dyn Fn`** (rd209; f3bdcf0e8 and
  2ace44dd3): `function::with_this(..)` and `function::bind(f, this, args)`
  give the JS function itself. Hold it as `&'static dyn Fn(..)`
  (wrapCallback, SyntheticEvent's prototype functions, the hooks'
  dispatches, a retry or ping given to `then`), never as a `Box`. Where one
  field holds Rust closures too, as an effect's `Rc<Create>` does, write
  `Rc::from(function::bind(..))`.
- **A root's error handlers are `Rc`s** (rd308, the user): the reconciler's
  createContainer, createHydrationContainer and FiberRoot take and hold
  `Rc<OnError>` and `Rc<OnCaughtError>` (onUncaughtError, onCaughtError,
  onRecoverableError), owned callbacks a renderer may make by capturing,
  `Rc::new(move |e, i| noop_renderer.default_on_uncaught_error(e, i))`.
  ReactDOMRoot gives `Rc::new(default_on_uncaught_error)` or
  `Rc::new(on_error)` of the option JS gave, printed as before. The reader
  borrows it, `let on_uncaught_error = &root.on_uncaught_error;`.
  onDefaultTransitionIndicator stays `&'static`, a nullable `Cell` field
  every renderer fills with a plain function.
- **`=== true` of an option JS gives** (the user): React's
  `options.unstable_strictMode === true` is `js::strict_equals(
  options.unstable_strict_mode, true)`. `== Some(true)` of an
  `Option<bool>` prints the value's truth (rust-js's ADR 0298), so a truthy
  value that isn't `true` would turn strict mode on.
- **Props**, by use: a `js::Defined` struct where code reads named
  Flow-declared members, and `&'static Unknown` with `js::get` where code
  iterates all keys (ReactDOMComponent).
- **setCurrentUpdatePriority**: `impl From<EventPriority> for Lane`, and the
  port writes `set_p(i32::from(p))`.
- **wl8**: copies named `<name>Boxed` and `<name>Unboxed` are kept.
- **rd349, imports by workspace specifier** (the user, 2026-11-01): a module
  of another package is imported as upstream imports it,
  `'shared/ReactFeatureFlags'`, not by its relative file with `.js`. rust-js
  is building it; until it lands, every cross-package import differs, and a
  regeneration then changes them all.
- **rd365, `extend` of an indexed list** (the user, 2026-11-01): a
  `Vec::extend` of a NodeList's items, the Fizz instruction set's
  `suspensey_images.extend(node_list::iter(appearing_images))`, is an
  index loop pushing each item, where upstream writes
  `suspenseyImages.push.apply(suspenseyImages, appearingImages)`, which
  throws a RangeError past the engine's argument limit while Rust's
  `extend` never fails. rust-js is building it; rd274's inline strings
  wait for it.

## Committing

- **Commit only your own paths.** Another session may be working in the
  same tree. Read `git status`, and `git add` the files you changed by name.
  Never use `git add -A` or `git add .`.
- **Commit on `main` and push**, with no branches. Put one module, or one
  regeneration, in each commit.
- **Subjects** follow two patterns: `Port <package>' <Module>` (for example
  `Port react-dom-bindings' ReactDOMComponent`), and
  `Regenerate on rust-js <sha>; <what changed> (<ids>)`.
- **The body** says what changed and why. Then, as they apply, it lists:
  the decided forms used, by id; "Port differences", the port-side forms;
  "Readability gaps (rust-js), not holding", by id; and the gates' numbers,
  with "clippy -D warnings and rustfmt clean, no labels". It names the
  rust-js commit the JS was generated on.
- **End with** the session's `Co-Authored-By:` and `Claude-Session:` lines.

## Who ports what

The port is split between sessions by package:

- **The first session** has scheduler, shared, react, react-reconciler, and
  react-dom-bindings' client and events modules. react-dom-bindings' client
  side is fully ported, ReactFiberConfigDOM included. react-dom's
  src/shared, ReactDOMSharedInternals, ReactDOMRoot, ReactDOMClient and
  ReactDOMDefaultTransitionIndicator are Rust. The indicator's listeners
  are one per event, `let on_success = webapi::listener(move |_: &Event|
  handle_navigate_complete());`, each given to both `add_event_listener`
  and `remove_event_listener`, where `handle_navigate_complete: &'static dyn
  Fn()` is `function::with_this(move |_, ()| ..)`. They print upstream's
  `navigation.addEventListener('navigatesuccess', handleNavigateComplete)`
  (rd284b, rust-js c66569bdf).
  shared/ReactDOMFragmentRefShared is react-dom-bindings' module, declared
  in its root as `#[path = "../shared/ReactDOMFragmentRefShared.rs"] pub mod
  react_dom_fragment_ref_shared;` (it reads the reconciler, which `shared`
  can't), its JS beside its `.rs` where React Native's Fabric config imports
  it.
  react-noop-renderer's createReactNoop and ReactFiberConfigNoop are Rust.
  react-test-renderer's ReactFiberConfigTestHost and ReactTestRenderer are
  Rust; react-art's ReactARTInternals, ReactFiberConfigART and ReactART
  are Rust. ReactART's Surface and Text are class components (rust-js ADR
  1041), its module `ReactART.jsx`, as rust-js names a module with JSX, which
  jest resolves by `jsx` in `scripts/jest/config.base.js`'s
  `moduleFileExtensions`; react-art's `index.js` imports `./src/ReactART`
  unchanged.
  bindToConsole binds the console's method to its arguments,
  `function::bind(method, Some(&console), args)`, printed
  `method.bind(console, ...args)` where upstream writes
  `Function.prototype.bind.apply(console[methodName], [console].concat(args))`
  (rd294, a readability difference; a method that isn't a function throws
  an Error of its own where upstream's bind throws a TypeError). rd308 is decided: the reconciler holds its
  error handlers as `Rc`s. ReactNoop.js and ReactNoopPersistent.js stay JS
  until rd293.
  **react-client** (the Flight client) is the first session's too (agreed
  with the second, 2026-11-01). ReactFlightTemporaryReferences,
  ReactFlightClientStreamConfigWeb, the three ReactClientConsoleConfigs, the
  three ReactClientDebugConfigs (`js::eval("null").is_err()`, printed
  upstream's `try { (0, eval)('null'); } catch {..}`) and
  ReactFlightClientDevToolsHook are Rust; the ReactFlightClientConfig forks
  stay JS, as the reconciler's ReactFiberConfig forks do (re-exports and
  `$$$config` shims). ReactFlightClientStreamConfigNode (rd357) and
  ReactFlightPerformanceTrack (rd343) are Rust.
  ReactFlightClient is Rust (2026-11-02): its chunk is ADR 0761's
  `ReactPromise` class and its Response a constructor function the
  program writes, `Class::of(function::with_this(..))`, whose `this` gives
  `new WeakRef(this)`.
  ReactFlightReplyClient is Rust (2026-11-02, Batch 54): processReply's
  state is a struct, `Reply`, made once in it, whose `self: &Rc<Self>`
  methods are upstream's nested functions (rust-js ADR 1043); its
  `serialize*ID`s keep upstream's names by `#[rust_js::name]`;
  `const FunctionBind = Function.prototype.bind;` and `const ArraySlice =
  Array.prototype.slice;` are `use js::function::BIND as FUNCTION_BIND;`
  and `use js::array::SLICE as ARRAY_SLICE;` named so (rd510); an async
  iterable's iterator is `js::AsyncIterator` (rd507); serializeReadableStream
  is `stream.get_reader_with_options(ReadableStreamGetByobReaderOptions {
  mode: Byob })`, upstream's `try { .. getReader({mode: 'byob'}) } catch
  { .. }` (rd506). The Flight client calls its
  createBoundServerReference, registerBoundServerReference and
  createServerObjectReference by their Rust signatures: a server
  reference's metadata, read off the model, is cast to
  `BoundServerReferenceMetaData`, its module to a function, its bound
  arguments to a `Thenable`, as upstream's `metaData.id`/`metaData.bound`.
  **Decided** (logged for the user, 2026-11-02): rd541, six fields only
  JS reads (`ServerObjectReferenceJson`'s `id`, `BoundReferenceJson`'s
  `id` and `bound`, `ServerReferenceProperties`' `form_action`,
  `is_signature_equal` and `bind`) are `never read` in a native `cargo
  check`: rust-js's clippy (ADRs 0440, 0888) is the lint, and it's clean,
  so no change and no `#[allow]`. **Accepted on the user's behalf**
  (2026-11-02): rd539, the ReplyClient's `.d.ts` keeps its exported
  `BoundServerReferenceMetaData` interface where upstream's type is an
  inline parameter type.
  processReply's functions are nested in it over its variables, as
  upstream's (rd536, rust-js bdfb187df, taken in Batch 55's
  regeneration).
  **Pending in rust-js, not accepted** (each changes no result):
  - rd425: a reader's or iterator's `progress` is a closure held in a cell
    that hands itself on, `let progress; progress = entry => {..}` and
    `const progress$1 = $unwrap(progress)`, where upstream's `function
    progress(entry)` names itself;
  - rd426 and rd508: upstream's `JSON.stringify(value, resolveToJSON)` at
    each site is one `stringify` method, `JSON.stringify(value, function
    (key, value) { return resolveToJson(reply, this, key, value); }, 0) ??
    ''`, a replacer calling resolveToJSON with its `this` cast to the
    holding object;
  - rd387: a caught error is read off the unwind, `catch (thrown$1) {
    const x = $unwound(thrown$1); .. thrown(x) ..}`, where upstream's is
    `catch (x)`.
  **react-server-dom-*'s client side** (`src/client/**`, the client
  entries, `src/shared/`) is the first session's (agreed with the second,
  2026-11-02). Webpack's ReactFlightImportMetadata,
  ReactFlightClientConfigBundlerWebpackBrowser and -Server and
  ReactFlightClientConfigTargetWebpackBrowser and -Server are Rust (Batch
  55), and so is ReactFlightClientConfigBundlerWebpack (after rd546, rust-js
  47f20158e: `HasOwnProperty::call` takes any value, so requireModule's
  `hasOwnProperty.call(moduleExports, name)` is upstream's). Its chunk
  pairs are read `chunks.get(i)`, as upstream's `chunks[i++]` reads
  `undefined` past an odd list's end, which the async tests send. Turbopack's
  ReactFlightImportMetadata (its `Chunk`, an untagged `File(String) |
  Merged((filename, filenames, sizes))`, a merged chunk's tuple told by
  `typeof chunk === 'string'` as upstream's), and its
  ReactFlightClientConfigBundlerTurbopack, -Browser and -Server and
  ReactFlightClientConfigTargetTurbopackBrowser and -Server are Rust, of
  webpack's forms (BundlerTurbopack's chunk sets are `js::WeakSet`s of the
  chunks' promises); `__turbopack_load_by_url__` and `__turbopack_require__`
  are `shared::environment`'s too. Parcel's ReactFlightImportMetadata (`[id,
  name, bundles]` or with its import map, an untagged enum told by
  length), ReactFlightClientConfigBundlerParcel and
  ReactFlightClientConfigTargetParcelBrowser and -Server are Rust;
  `parcelRequire`, `.load`, `.extendImportMap` and `.meta.publicUrl` are
  `shared::environment`'s. Port differences (for the user while away;
  none changes a result): resolveServerReference's `bundles
  == null` for `!bundles` (an array); TargetParcelServer's loop reads
  `const bundle = bundles[i];` where upstream reads it in place.
  BundlerESM (react-server-dom-esm) and BundlerNode
  (react-server-dom-unbundled) are held on rd607: rust-js has no
  `import()` of a specifier the program has at run time. react-server-dom-esm's
  ReactFlightClientConfigTargetESMBrowser and -Server are Rust (its
  ModuleLoading an untagged `null | string | {prefix, crossOrigin?}`, the
  object `#[rust_js::otherwise]`, so the tests print upstream's `typeof
  moduleLoading === 'string'` and `moduleLoading !== null`; the string
  case's call leaves out upstream's trailing `undefined`, accepted below);
  react-server-dom-unbundled's ReactFlightImportMetadata and
  ReactFlightClientConfigTargetNodeServer are webpack's, as upstream's
  are. The plain re-export
  entries (`client*.js`, `react-flight-dom-client.*.js`) stay JS (rd55).
  Webpack's ReactFlightDOMClientEdge, ReactFlightDOMClientBrowser and
  ReactFlightDOMClientNode are Rust (Batch 59), and so are Turbopack's three
  (Batch 60), webpack's sources with their bundler's names, as upstream's
  are webpack's, and Parcel's three and ESM's Browser and Node (Batch 60):
  webpack's sources with upstream's changes, Parcel's `findSourceMapURL`
  (`#[rust_js::nullable]`, upstream's `return null;`, of
  `shared::environment`'s `parcelRequire.meta.devServer`), its Browser's
  module `callServer` (a `nullable` thread-local) and `setServerCallback`,
  createServerReference's `id + '#' + exportName`, and ESM's
  `moduleRootPath`/`moduleBaseURL`; the Rust keeps upstream's blank lines
  (none between `let callServer` and `setServerCallback`, nor before ESM's
  createFromReadableStream). ESM's Browser gives `options?.moduleBaseURL ??
  ''` for upstream's `options && options.moduleBaseURL ? .. : ''`, the
  accepted `?? false` kind. Upstream has no tests of either package's
  clients: ReactFlightParcelDOMClients-test.js (10) and
  ReactFlightESMDOMClients-test.js (6) hold them, and pass on upstream's
  JS too. **Pending in rust-js** (rd646, not accepted): Parcel's Browser
  reads the module's `callServer` into `const callServer$1 = callServer;`
  before its `== null` test, where upstream tests and calls `callServer`
  in place. The Node client's `pub use
  crate::client::react_flight_dom_client_edge::*;` prints rd153's export
  list for upstream's `export * from './ReactFlightDOMClientEdge'`. Their `startReadingFromStream`'s `progress` is rd425's
  cell, and their createResponse arguments are the forms accepted above
  (`?.`, `?? false`). The DOM clients' DebugChannel is an `Rc`, shared by
  the response and the FinalizationRegistry that closes it.
  **Accepted by the user** (2026-11-03, by name, the Edge client's; none
  changes a result for React's inputs): noServerCall takes `(id, args)`,
  the CallServerCallback Rust's fn type requires, where upstream declares
  none; the debug channel is tested `options.debugChannel != null` and
  `readable != null` (an `Option`) where upstream writes `!== undefined`;
  `__DEV__ ? options.startTime : undefined` (and endTime) where upstream
  writes `startTime != null ? startTime : undefined`; the stream counter's
  `streamDoneCount++; if (streamDoneCount === 2)` for `if
  (++streamDoneCount === 2)`; `() => { close(response); }` for
  `close.bind(null, response)`, and createFromFetch's callbacks as arrows
  where upstream writes `function (r)` and `function (e)`; the let-chain's
  `const readable = options.debugChannel.readable;` after the test, where
  upstream reads it in the call; progress reading `entry.done` and
  `entry.value` where upstream destructures `{done, value}`, ending with
  `onDone(); return;` and the next read's chain unreturned, and its `error`
  a `reportGlobalError.bind(null, response)` declared before it, where
  upstream declares `function error(e)` after it; the names in one import
  in rustfmt's order, where upstream lists them in its own; and
  ReplyClient's `// Serializable values`, printed only in the `.d.ts`
  above ReactServerValue's union. Also by name (2026-11-03, the Browser
  client's): `options != null && options.debugChannel != null` where
  upstream tests `options && ..` by truth (an object or absent), and
  `__DEV__ && (options == null || options.replayConsoleLogs !== false)` for
  `__DEV__ ? (options ? options.replayConsoleLogs !== false : true) :
  false`. Also by name (2026-11-03, the Node client's): its DEV debug
  channel `{hasReadable: true, callback: null}` printed over four lines,
  as rustfmt splits the `DebugChannel` struct literal, where upstream
  writes it on one (rd627 B's kind: the printer breaks an object literal
  where the Rust breaks it).
  **Pending in rust-js, not accepted**: rd643, encodeReply's promise is
  `Promise<ReplyBody>`, `string | FormData` in the `.d.ts`, where upstream
  declares `Promise<string | URLSearchParams | FormData>`: processReply
  resolves with `ReplyBody`, an untagged `Text(String) | Form(&FormData)`,
  upstream's `string | FormData`, because a third variant no code builds
  prints an unreachable branch, `else if (body instanceof FormData) ..
  else throw ..`, in encodeFormData; rd642,
  the `.d.ts`'s optional `nullable` fields and its missing `export type
  {TemporaryReferenceSet}`.
  **Accepted by the user** (2026-11-02, by name): rd543, a tuple's
  element is read by destructuring, `const chunks = metadata[1];`, where
  upstream reads `metadata[CHUNKS]` in place; rd544, the async marker,
  Flow's literal type `1`, is `#[repr(u8)] enum Async { Async = 1 }`, which
  exports `const Async = 1;` and prints `Async` where upstream writes `1 /*
  async */`. Also by name (2026-11-02, Batch 56): parcel's preloadModule
  tests `metadata.length === 4` where upstream tests
  `metadata[IMPORT_MAP]`'s truth, and extends the map with `metadata[3]`
  where upstream reads `metadata[IMPORT_MAP]` (rd543's kind): parcel's
  server pushes the map only `if (clientReference.$$importMap)`, so a
  fourth item is never falsy; esm's TargetESMServer calls
  `preinitModuleForSSR(moduleLoading + mod, nonce)` where upstream passes a
  trailing `undefined` (rd271's kind: a trailing `None` is left out);
  BundlerWebpack's `chunks[i]; i++;` as two statements for upstream's
  `chunks[i++]` (Rust has no post-increment); the patched
  `__webpack_require__.u` as an arrow where upstream writes `function
  (chunkId)` (it reads no `this`); BundlerWebpack's two errors' messages
  as template literals of `panic!("..{id}..")` where upstream
  concatenates; BundlerWebpack's `!name` for `name === ''` of a string, and
  the DOM clients' `if (!message)` for `message === ''` (clippy's
  `comparison_to_empty`); the DOM clients' optional chaining and `?? false`
  (`options?.callServer`, `options?.unstable_allowPartialStream ?? false`)
  where upstream writes `options && x ? x : undefined` (or `: false`), and
  `const body = r.body;` (rd87's kind) for `r.body as any`; the DOM
  clients' `.catch(error => console.error(error))` where upstream passes
  `console.error`, and inline `export function f` where upstream declares
  `function f` and ends with an `export {..}` list (rd616's, by name).
  **Pending in rust-js, not accepted** (each changes no result):
  - rd548: addChunkDebugInfo's fill-if-missing reads `const cached =
    chunkIOInfoCache.get(chunkId); let ioInfo; if (cached != null) {..}`
    where upstream writes `let ioInfo = ..; if (ioInfo === undefined)`;
  Port differences (decided for the user while away, 2026-11-02; none
  changes a result): the chunk map's `flightChunk != null`
  where upstream tests `!== undefined` (the map holds strings); the fake
  stack's `(fakeStack.stack ?? '')` (ReplyClient's parseStackLocation
  kind); `typeof performance !== 'undefined' &&` before
  `getEntriesByType`'s test (rd320, estimateBandwidth's kind); and the
  `/**` comment above the patch prints `/*`. BundlerWebpack's: resolveServerReference
  throws after its `if`/`else`, `if (resolvedModuleData == null) throw ..`,
  where upstream throws inside the `else` (the `if` branch has the module);
  `resolvedModuleData.async === true` where upstream tests truth (an
  optional boolean); the cache's `resolve` is a function declaration of
  `chunkCache.set(chunkId, null)` where upstream binds `chunkCache.set`
  (rd80's kind); `ignoreReject(reason)` takes the callback's parameter;
  requireModule ends without `return undefined;` (rw5's kind).
- **The second session** has react-server, react-server-dom-*'s server
  side and react-dom-bindings/src/server.

Before editing another session's modules, regeneration included, message
the coordinator (m1-arch).
