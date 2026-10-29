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
`react.rs`, `reconciler.rs` or `bindings.rs`. It declares each module as
`#[path = "src/..."] pub mod ..;` and writes no JS of its own.

The crates are `shared`, `scheduler`, `react-isomorphic` (packages/react),
`react-reconciler` and `react-dom-bindings`. A new package is a new crate,
added to the workspace's `members`. The generated `.js` and `.d.ts` files are
committed, as upstream's JS was. Source maps are not committed (see
`.gitignore`).

```sh
yarn rust --package react-dom-bindings     # compile one crate to its JS
cargo fmt -p react-dom-bindings            # rustfmt, this crate only
node ../rust-js/tooling/cli.js clippy --package react-dom-bindings -- -D warnings
```

- **Name the package.** `yarn rust` needs `--package`; plain `yarn rust`
  refuses a workspace.
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
| dev | `yarn test packages/react-reconciler packages/react/src packages/shared` | 1554 / 0 / 20 (109 suites) |
| prod | `yarn test --prod packages/react-reconciler packages/react/src packages/shared` | 1531 / 0 / 20 (109 suites) |
| dom | `yarn test react-dom/src/__tests__/ReactDOM` | 3356 / 1 / 2 (84 suites)¹ |
| misc | `yarn test PerformanceTrack Profiler TestSelectors ViewTransition ErrorBoundaries Activity SuspenseList Fragment` | 551 / 0 / 1 (28 suites) |
| sched | `yarn test packages/scheduler` | 63 / 0 / 1 (6 suites) |
| schedwww | `yarn test -r=www-modern packages/scheduler` (profiling on) | 71 / 0 / 1 (6 suites) |
| domrest | `yarn test packages/react-dom/src/__tests__ --testPathIgnorePatterns 'ReactDOM[A-Z]'` | 695 / 0 / 0 (49 suites) |
| ev | `yarn test $(cat scripts/port/ev.txt)` (events, forms, inputs, ReactDOMComponent) | 1057 / 0 / 1 (29 suites) |

¹ The one failure is ReactDOMServerPartialHydration's `[GATED, SHOULD FAIL]
renders a hidden LegacyHidden component inside a Suspense boundary`, the
baseline failure every recorded run of this gate has had.

### Server side

`scripts/port/gates.sh <tag> --server` adds the gates below, and
`--server-only` runs only them. These suites hold react-server,
react-client, react-server-dom-* and react-dom-bindings/src/server.

All of them run in the default channel (experimental) and need no flag. For
react-server, react-client, the Fizz suites and react-server-dom-fb,
`-r=stable` and `-r=www-modern` give the same counts.

| Gate | Command | Expected |
|---|---|---|
| fizz | `yarn test react-dom/src/__tests__/ReactDOMFizz` (13 suites, the Static ones included) | 374 / 0 / 0 |
| ssr | `yarn test react-dom/src/__tests__/ReactDOMServerIntegration` (21 suites) | 1602 / 0 / 0 |
| float | `yarn test react-dom/src/__tests__/ReactDOMFloat react-dom/src/__tests__/ReactDOMLegacyFloat` (2 suites) | 143 / 0 / 1 |
| ssrmisc | `yarn test react-dom/src/__tests__/ReactServerRendering react-dom/src/__tests__/ReactDOMServerLifecycles react-dom/src/__tests__/ReactDOMInReactServer react-dom/src/__tests__/ReactClassComponentPropResolutionFizz packages/react-markup packages/use-sync-external-store/src/__tests__/useSyncExternalStoreShimServer` (9 suites) | 104 / 0 / 0 |
| server | `yarn test packages/react-server/src/__tests__` (4 suites) | 26 / 0 / 0² |
| client | `yarn test packages/react-client/src/__tests__` (2 suites) | 95 / 0 / 0 |
| webpack | `yarn test packages/react-server-dom-webpack/src/__tests__` (9 suites) | 278 / 0 / 0 |
| turbopack | `yarn test packages/react-server-dom-turbopack/src/__tests__` (7 suites) | 43 / 0 / 0 |
| fb | `yarn test packages/react-server-dom-fb/src/__tests__` (1 suite) | 8 / 0 / 0 |

The `ssr` gate's 21 suites are the 20 `ReactDOMServerIntegration*` files plus
`ReactDOMserverIntegrationProgress` (lowercase s), which the same pattern
matches. Many server suites also run inside `dom` and `domrest`.

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
- **Port fallbacks too** (the user, 2026-10-22). A polyfill, or a branch
  for a host without `Object.is` or `performance.now`, is the program's to
  express, whatever hosts rust-js targets.
- **Port type-only modules as Rust and their `.d.ts`**, and remove
  upstream's Flow `.js` (the user, 2026-10-28). A package's entry,
  `index.js`, keeps only plain JS re-exports, and its Flow types move to
  `index.d.ts` (rd55).

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
  prints `$extend(stack, Iterator.from(current.children))`; Rust's
  never-throwing `extend` is kept.

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
- **Props**, by use: a `js::Defined` struct where code reads named
  Flow-declared members, and `&'static Unknown` with `js::get` where code
  iterates all keys (ReactDOMComponent).
- **setCurrentUpdatePriority**: `impl From<EventPriority> for Lane`, and the
  port writes `set_p(i32::from(p))`.
- **wl8**: copies named `<name>Boxed` and `<name>Unboxed` are kept.

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
  react-dom-bindings' client and events modules. ReactFiberConfigDOM is
  next.
- **The second session** has react-server, react-client, react-server-dom-*
  and react-dom-bindings/src/server.

Before editing another session's modules, regeneration included, message
the coordinator (m1-arch).
