// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::rc::{Rc, Weak};

use js::{Dict, JsError, JsObject, Kind, PropertyDescriptor, SymbolFor, Unknown, WeakSet};
use react_isomorphic::react_lazy::LazyComponent;
use shared::environment::__DEV__;
use shared::react_feature_flags::{DISABLE_LEGACY_CONTEXT, ENABLE_CPU_SUSPENSE};
use shared::react_owner_stack_reset::reset_owner_stack_limit;
use shared::react_recoverable::REACT_RECOVERABLE_DIGEST;
use shared::react_serialization_errors::describe_object_for_error_message;
use shared::react_shared_internals::REACT_SHARED_INTERNALS;
use shared::react_symbols::{REACT_ELEMENT_TYPE, REACT_LAZY_TYPE};
use shared::react_types::{ReactFormState, ReactNodeList, SuspenseProps};
use webapi::{AbortController, ConsoleTask, console};

use crate::react_fizz_async_dispatcher::DEFAULT_ASYNC_DISPATCHER;
use crate::react_fizz_component_stack::{
    ComponentStackNode, get_owner_stack_by_component_stack_node_in_dev,
    get_stack_by_component_stack_node,
};
use crate::react_fizz_current_task::{CURRENT_TASK_IN_DEV, set_current_task_in_dev};
use crate::react_fizz_hooks::{
    CURRENT_RESUMABLE_STATE, HOOKS_DISPATCHER, is_recoverable_error, reset_hooks_state,
    set_current_resumable_state,
};
use crate::react_fizz_legacy_context::{Context as LegacyContext, EMPTY_CONTEXT_OBJECT};
use crate::react_fizz_new_context::{
    ContextSnapshot, ROOT_CONTEXT_SNAPSHOT, get_active_context, switch_context,
};
use crate::react_fizz_thenable::{
    ThenableState, ensure_suspendable_thenable_state_dev, get_suspended_call_site_debug_task_dev,
    get_suspended_call_site_stack_dev, set_capture_suspended_call_site_dev,
};
use crate::react_fizz_tree_context::{EMPTY_TREE_CONTEXT, TreeContext};

// ReactServerStreamConfig's types, which its fork for each host defines: a
// destination, and the chunks written to it.

/// Where a request writes, the host's stream.
pub type Destination = Unknown;
/// What's written: text, or its encoded bytes.
pub type Chunk = Unknown;
/// A chunk encoded once, as a module loads.
pub type PrecomputedChunk = Unknown;

// ReactFizzConfig's types, which a renderer's config defines.

pub type RenderState = Unknown;
pub type ResumableState = Unknown;
pub type PreambleState = Unknown;
pub type FormatContext = Unknown;
pub type HoistableState = Unknown;

// ReactServerStreamConfig's functions, of its fork for the host.

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactServerStreamConfig.js#scheduleWork"
)]
fn schedule_work(callback: impl FnOnce() + 'static) {
    let _ = callback;
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactServerStreamConfig.js#scheduleMicrotask"
)]
fn schedule_microtask(callback: impl FnOnce() + 'static) {
    let _ = callback;
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactServerStreamConfig.js#beginWriting"
)]
fn begin_writing(destination: &Destination) {
    let _ = destination;
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactServerStreamConfig.js#writeChunk"
)]
fn write_chunk(destination: &Destination, chunk: &Chunk) {
    let _ = (destination, chunk);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactServerStreamConfig.js#writeChunkAndReturn"
)]
fn write_chunk_and_return(destination: &Destination, chunk: &Chunk) -> bool {
    let _ = (destination, chunk);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactServerStreamConfig.js#completeWriting"
)]
fn complete_writing(destination: &Destination) {
    let _ = destination;
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactServerStreamConfig.js#flushBuffered"
)]
fn flush_buffered(destination: &Destination) {
    let _ = destination;
    unreachable!()
}

#[cfg_attr(rust_js, rust_js::link_name = "./src/ReactServerStreamConfig.js#close")]
fn close(destination: &Destination) {
    let _ = destination;
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactServerStreamConfig.js#closeWithError"
)]
fn close_with_error(destination: &Destination, error: Option<&Unknown>) {
    let _ = (destination, error);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactServerStreamConfig.js#byteLengthOfChunk"
)]
fn byte_length_of_chunk(chunk: &Chunk) -> f64 {
    let _ = chunk;
    unreachable!()
}

// ReactFizzConfig's functions and values, of the renderer's config.

/// A segment's chunks, which the config pushes to.
pub type Chunks = RefCell<Vec<&'static Chunk>>;

unsafe extern "Rust" {
    #[link_name = "./src/ReactFizzConfig.js#isWorkLoopExternallyDriven"]
    safe static IS_WORK_LOOP_EXTERNALLY_DRIVEN: bool;
    #[link_name = "./src/ReactFizzConfig.js#supportsRequestStorage"]
    safe static SUPPORTS_REQUEST_STORAGE: bool;
    // GAP rd492: node has no `async_hooks` binding, so `AsyncLocalStorage`
    // is bound here as ReactFlightCurrentOwner binds its storage.
    #[link_name = "./src/ReactFizzConfig.js#requestStorage"]
    safe static REQUEST_STORAGE: &'static RequestStorage;
}

/// The config's `AsyncLocalStorage<Request | void>`, where it has one.
pub struct RequestStorage(std::marker::PhantomData<JsObject>);

unsafe impl js::Defined for RequestStorage {}

impl RequestStorage {
    #[cfg_attr(rust_js, rust_js::link_name = "getStore")]
    fn get_store(&self) -> Option<Rc<Request>> {
        unreachable!()
    }
}

#[cfg_attr(rust_js, rust_js::link_name = "./src/ReactFizzConfig.js#bindToConsole")]
fn bind_to_console(
    method_name: &str,
    args: Vec<Option<&'static Unknown>>,
    bad_name: &str,
) -> &'static dyn Fn() {
    let _ = (method_name, args, bad_name);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#createHoistableState"
)]
fn create_hoistable_state() -> &'static HoistableState {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#createPreambleState"
)]
fn create_preamble_state() -> &'static PreambleState {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#hoistHoistables"
)]
fn hoist_hoistables(parent_state: &HoistableState, child_state: &HoistableState) {
    let _ = (parent_state, child_state);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#hasSuspenseyContent"
)]
fn has_suspensey_content(hoistable_state: &HoistableState, flushing_in_shell: bool) -> bool {
    let _ = (hoistable_state, flushing_in_shell);
    unreachable!()
}

// Linked list representing the identity of a component given the component/tag name and key.
// The name might be minified but we assume that it's going to be the same generated name. Typically
// because it's just the same compiled output in practice.
/// `[parent, name, key]`.
pub struct KeyNode(
    #[cfg_attr(rust_js, rust_js::nullable)] pub Option<Rc<KeyNode>>, /* parent */
    #[cfg_attr(rust_js, rust_js::nullable)] pub Option<String>,      /* name */
    pub NodeKey,                                                     /* key */
);

unsafe impl js::Defined for KeyNode {}

// Safety: an array of strings, numbers, `null` and such arrays.
unsafe impl js::JsonText for KeyNode {}

// A key path is itself, as a `Map` keyed by it holds each.
impl PartialEq for KeyNode {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Eq for KeyNode {}

impl Hash for KeyNode {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::ptr::hash(self, state);
    }
}

/// A node's key in its parent: the key it was given, or its index.
#[cfg_attr(rust_js, rust_js::untagged)]
#[derive(Clone, PartialEq)]
pub enum NodeKey {
    Key(String),
    Index(f64),
}

/// What to resume: nothing, the segment ID at the root position, or the
/// segment ID at each index.
#[cfg_attr(rust_js, rust_js::untagged)]
#[derive(Clone)]
pub enum ResumeSlots {
    // nothing to resume
    #[cfg_attr(rust_js, rust_js::name = "null")]
    None,
    // resume with segment ID at the root position
    Root(f64),
    // resume with segmentID at the index
    Indices(Rc<Dict<f64>>),
}

// GAP rd490: a replay node is an array of 4 items, or a boundary's of 6,
// which upstream tells apart by its length (`node.length === 4`); an
// untagged enum of the two tuple structs is the Rust, which rust-js can't
// tell apart.
/// `[name, key, children, slots]`.
pub struct ReplayElement(
    #[cfg_attr(rust_js, rust_js::nullable)] pub Option<String>, /* name */
    pub NodeKey,                                                /* key */
    pub Rc<RefCell<Vec<ReplayNode>>>,                           /* keyed children */
    pub RefCell<ResumeSlots>,                                   /* resumable slots */
);

/// `[name, key, content, contentSlots, fallback, rootSegmentID]`.
pub struct ReplaySuspenseBoundary(
    #[cfg_attr(rust_js, rust_js::nullable)] pub Option<String>, /* name */
    pub NodeKey,                                                /* key */
    pub Rc<RefCell<Vec<ReplayNode>>>,                           /* content keyed children */
    pub RefCell<ResumeSlots>,                                   /* content resumable slots */
    #[cfg_attr(rust_js, rust_js::nullable)] pub RefCell<Option<ReplayNode>>, /* fallback content */
    pub f64,                                                    /* rootSegmentID */
);

#[cfg_attr(rust_js, rust_js::untagged)]
#[derive(Clone)]
pub enum ReplayNode {
    Element(Rc<ReplayElement>),
    Boundary(Rc<ReplaySuspenseBoundary>),
}

struct PostponedHoles {
    working_map: RefCell<HashMap<Rc<KeyNode>, ReplayNode>>,
    root_nodes: Rc<RefCell<Vec<ReplayNode>>>,
    root_slots: RefCell<ResumeSlots>,
}

struct SuspenseListRow {
    pending_tasks: Cell<f64>, // The number of tasks, previous rows and inner suspense boundaries blocking this row.
    #[cfg_attr(rust_js, rust_js::nullable)]
    boundaries: RefCell<Option<Vec<Rc<SuspenseBoundary>>>>, // The boundaries in this row waiting to be unblocked by the previous row. (null means this row is not blocked)
    hoistables: &'static HoistableState, // Any dependencies that this row depends on. Future rows need to also depend on it.
    #[cfg_attr(rust_js, rust_js::nullable)]
    inherited_hoistables: Cell<Option<&'static HoistableState>>, // Any dependencies that previous row depend on, that new boundaries of this row needs.
    together: Cell<bool>, // All the boundaries within this row must be revealed together.
    #[cfg_attr(rust_js, rust_js::nullable)]
    next: RefCell<Option<Rc<SuspenseListRow>>>, // The next row blocked by this one.
}

const CLIENT_RENDERED: i32 = 4; // if it errors or infinitely suspends

/// The task set a task belongs to, which aborting it cancels.
pub type AbortSet = Rc<RefCell<HashSet<Rc<Task>>>>;

struct Tracked {
    #[cfg_attr(rust_js, rust_js::nullable)]
    content_key_path: RefCell<Option<Rc<KeyNode>>>, // used to track the path for replay nodes
    #[cfg_attr(rust_js, rust_js::nullable)]
    fallback_node: RefCell<Option<ReplayNode>>, // used to track the fallback for replay nodes
}

struct SuspenseBoundary {
    status: Cell<i32>,
    #[cfg_attr(rust_js, rust_js::name = "rootSegmentID")]
    root_segment_id: Cell<f64>,
    parent_flushed: Cell<bool>,
    pending_tasks: Cell<f64>, // when it reaches zero we can show this boundary's content
    #[cfg_attr(rust_js, rust_js::nullable)]
    row: RefCell<Option<Rc<SuspenseListRow>>>, // the row that this boundary blocks from completing.
    completed_segments: RefCell<Vec<Rc<Segment>>>, // completed but not yet flushed segments.
    byte_size: Cell<f64>,     // used to determine whether to inline children boundaries.
    defer: bool,              // never inline deferred boundaries
    fallback_abortable_tasks: AbortSet, // used to cancel task on the fallback if the boundary completes or gets canceled.
    content_state: &'static HoistableState,
    fallback_state: &'static HoistableState,
    #[cfg_attr(rust_js, rust_js::nullable)]
    preamble: Option<Rc<Preamble>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    tracked: RefCell<Option<Rc<Tracked>>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    error_digest: RefCell<Option<String>>, // the error hash if it errors
    // DEV-only fields
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    error_message: RefCell<Option<String>>, // the error string if it errors
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    error_stack: RefCell<Option<String>>, // the error stack if it errors
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    error_component_stack: RefCell<Option<String>>, // the error component stack if it errors
}

struct Ping {
    resolve: Rc<dyn Fn()>,
    reject: Rc<dyn Fn(Option<&'static Unknown>)>,
}

struct ReplaySet {
    nodes: Rc<RefCell<Vec<ReplayNode>>>, // the possible paths to follow down the replaying
    slots: RefCell<ResumeSlots>,         // slots to resume
    pending_tasks: Cell<f64>, // tracks the number of tasks currently tracking this set of nodes
                              // if pending tasks reach zero but there are still nodes left, it means we couldn't find
                              // them all in the tree, so we need to abort and client render the boundary.
}

/// A render task, which writes to its segment, or a replay task, which
/// writes to none: its `replay` is `null` where it renders.
pub struct Task {
    #[cfg_attr(rust_js, rust_js::nullable)]
    replay: RefCell<Option<Rc<ReplaySet>>>,
    node: Cell<Option<&'static ReactNodeList>>,
    child_index: Cell<f64>,
    ping: Ping,
    #[cfg_attr(rust_js, rust_js::nullable)]
    blocked_boundary: RefCell<Option<Rc<SuspenseBoundary>>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    blocked_segment: RefCell<Option<Rc<Segment>>>, // the segment we'll write to
    #[cfg_attr(rust_js, rust_js::nullable)]
    blocked_preamble: Cell<Option<&'static PreambleState>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    hoistable_state: Cell<Option<&'static HoistableState>>, // Boundary state we'll mutate while rendering. This may not equal the state of the blockedBoundary
    abort_set: RefCell<AbortSet>, // the abortable set that this task belongs to
    #[cfg_attr(rust_js, rust_js::nullable)]
    key_path: RefCell<Option<Rc<KeyNode>>>, // the path of all parent keys currently rendering
    format_context: Cell<&'static FormatContext>, // the format's specific context (e.g. HTML/SVG/MathML)
    context: RefCell<ContextSnapshot>, // the current new context that this task is executing in
    pub tree_context: RefCell<Rc<TreeContext>>, // the current tree context that this task is executing in
    #[cfg_attr(rust_js, rust_js::nullable)]
    row: RefCell<Option<Rc<SuspenseListRow>>>, // the current SuspenseList row that this is rendering inside
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub component_stack: RefCell<Option<Rc<ComponentStackNode>>>, // stack frame description of the currently rendering component
    #[cfg_attr(rust_js, rust_js::nullable)]
    thenable_state: RefCell<Option<Rc<ThenableState>>>,
    // GAP rd493: upstream sets `legacyContext` only `if
    // (!disableLegacyContext)` and `debugTask` only in DEV, to keep the
    // object at 16 fields in production; a struct's fields are always set.
    legacy_context: Cell<&'static LegacyContext>, // the current legacy context that this task is executing in
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub debug_task: Cell<Option<&'static ConsoleTask>>, // DEV only
                                                  // DON'T ANY MORE FIELDS. We at 16 in prod already which otherwise requires converting to a constructor.
                                                  // Consider splitting into multiple objects or consolidating some fields.
}

// A task is itself: two are equal only where they're one, as a JS `Set` of
// them holds each.
impl PartialEq for Task {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Eq for Task {}

impl Hash for Task {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::ptr::hash(self, state);
    }
}

const PENDING: i32 = 0;
const COMPLETED: i32 = 1;
const FLUSHED: i32 = 2;
const ABORTED: i32 = 3;
const ERRORED: i32 = 4;
const POSTPONED: i32 = 5;

struct Segment {
    status: Cell<i32>,
    parent_flushed: Cell<bool>, // typically a segment will be flushed by its parent, except if its parent was already flushed
    id: Cell<f64>,              // starts as 0 and is lazily assigned if the parent flushes early
    index: f64,                 // the index within the parent's chunks or 0 at the root
    chunks: Chunks,
    children: RefCell<Vec<Rc<Segment>>>,
    preamble_children: RefCell<Vec<Rc<Segment>>>,
    // The context that this segment was created in.
    parent_format_context: &'static FormatContext,
    // If this segment represents a fallback, this is the content that will replace that fallback.
    #[cfg_attr(rust_js, rust_js::nullable)]
    boundary: RefCell<Option<Rc<SuspenseBoundary>>>,
    // used to discern when text separator boundaries are needed
    last_pushed_text: Cell<bool>,
    text_embedded: Cell<bool>,
}

// The ordering of these statuses matters. OPENING and OPEN are the only
// statuses in which newly scheduled work may be performed. Any status greater
// than OPEN represents a request that no longer admits work.
const OPENING: i32 = 10;
const OPEN: i32 = 11;
const CLOSING: i32 = 12;
const CLOSED: i32 = 13;
const STALLED_DEV: i32 = 14;

// Passed to renderLifetimeController.abort(). Nothing reads the reason, but a
// call to abort() without one constructs an AbortError DOMException. Capturing
// the stack trace dominates that cost, and the cost grows with the depth of the
// stack.
const RENDER_ENDED: &str = "The render ended.";

/// What a request tells `onError` and `onBrowserBailout` of an error: its
/// component stack, made as it's first read.
pub struct ThrownInfo(std::marker::PhantomData<JsObject>);

unsafe impl js::Defined for ThrownInfo {}

impl ThrownInfo {
    #[cfg_attr(rust_js, rust_js::link_name = "get componentStack")]
    pub fn component_stack(&self) -> Option<String> {
        unreachable!()
    }
}

pub type ErrorInfo = ThrownInfo;

/// A request's `onError`: of what was thrown and where, the digest it's
/// sent to the client as, if any.
pub type OnError = dyn Fn(Option<&'static Unknown>, &ErrorInfo) -> Option<&'static Unknown>;

/// A request's `onBrowserBailout`, of what made a boundary render on the
/// client instead, and where.
pub type OnBrowserBailout = dyn Fn(Option<&'static Unknown>, &ErrorInfo);

/// A request's `onShellError` and `onFatalError`, of what was thrown.
pub type OnFatal = dyn Fn(Option<&'static Unknown>);

pub struct Request {
    #[cfg_attr(rust_js, rust_js::nullable)]
    destination: Cell<Option<&'static Destination>>,
    flush_scheduled: Cell<bool>,
    resumable_state: &'static ResumableState,
    render_state: &'static RenderState,
    root_format_context: &'static FormatContext,
    progressive_chunk_size: f64,
    status: Cell<i32>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    fatal_error: Cell<Option<&'static Unknown>>,
    aborted: Cell<bool>,
    next_segment_id: Cell<f64>,
    all_pending_tasks: Cell<f64>, // when it reaches zero, we can close the connection.
    pending_root_tasks: Cell<f64>, // when this reaches zero, we've finished at least the root boundary.
    #[cfg_attr(rust_js, rust_js::nullable)]
    completed_root_segment: RefCell<Option<Rc<Segment>>>, // Completed but not yet flushed root segments.
    #[cfg_attr(rust_js, rust_js::nullable)]
    completed_preamble_segments: RefCell<Option<Vec<Vec<Rc<Segment>>>>>, // contains the ready-to-flush segments that make up the preamble
    byte_size: Cell<f64>, // counts the number of bytes accumulated in the shell
    abortable_tasks: AbortSet,
    pinged_tasks: RefCell<Vec<Rc<Task>>>, // High priority tasks that should be worked on first.
    #[cfg_attr(rust_js, rust_js::nullable)]
    current_task: RefCell<Option<Rc<Task>>>, // The task currently executing in this request.
    // Queues to flush in order of priority
    client_rendered_boundaries: RefCell<Vec<Rc<SuspenseBoundary>>>, // Errored or client rendered but not yet flushed.
    completed_boundaries: RefCell<Vec<Rc<SuspenseBoundary>>>, // Completed but not yet fully flushed boundaries to show.
    partial_boundaries: RefCell<Vec<Rc<SuspenseBoundary>>>, // Partially completed boundaries that can flush its segments early.
    #[cfg_attr(rust_js, rust_js::nullable)]
    tracked_postpones: RefCell<Option<Rc<PostponedHoles>>>, // Gets set to non-null while we want to track postponed holes. I.e. during a prerender.
    // While prerendering a postponed request that produced a real shell, this
    // holds the PostponedState returned by getPostponedState. getPostponedState
    // snapshots nextSegmentId before the (pull-driven) prelude flush runs, but the
    // flush outlines completed boundaries and advances nextSegmentId past that
    // snapshot. We finalize the snapshot from flushCompletedQueues so the resumed
    // render allocates segment ids strictly above the shell's; otherwise the shell
    // and resume emit duplicate B:/S: ids once concatenated. Stays null for live
    // renders and resumes.
    #[cfg_attr(rust_js, rust_js::nullable)]
    postponed_state: RefCell<Option<Rc<PostponedState>>>,
    // onError is called when an error happens anywhere in the tree. It might recover.
    // The return string is used in production  primarily to avoid leaking internals, secondarily to save bytes.
    // Returning null/undefined will cause a default error message in production
    on_error: Rc<OnError>,
    // onBrowserBailout is called when Fizz recovers by intentionally deferring
    // rendering to the browser.
    on_browser_bailout: Rc<OnBrowserBailout>,
    // onAllReady is called when all pending task is done but it may not have flushed yet.
    // This is a good time to start writing if you want only HTML and no intermediate steps.
    on_all_ready: Rc<dyn Fn()>,
    // onShellReady is called when there is at least a root fallback ready to show.
    // Typically you don't need this callback because it's best practice to always have a
    // root fallback ready so there's no need to wait.
    on_shell_ready: Rc<dyn Fn()>,
    // onShellError is called when the shell didn't complete. That means you probably want to
    // emit a different response to the stream instead.
    on_shell_error: Rc<OnFatal>,
    on_fatal_error: Rc<OnFatal>,
    // Aborted once the render ends, whether it completed, failed fatally or was
    // aborted. Bounds the lifetime of anything that must not outlive the render.
    // Null until attachAbortSignal creates it, so a render that is given no
    // signal constructs no controller.
    #[cfg_attr(rust_js, rust_js::nullable)]
    render_lifetime_controller: Cell<Option<&'static AbortController>>,
    // Form state that was the result of an MPA submission, if it was provided.
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub form_state: Option<&'static FormState>,
    // DEV-only, warning dedupe
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    did_warn_for_key: Cell<Option<&'static WeakSet<ComponentStackNode>>>,
}

/// The result of an MPA submission.
pub type FormState = ReactFormState<Option<&'static Unknown>, Option<&'static Unknown>>;

struct Preamble {
    content: &'static PreambleState,
    fallback: &'static PreambleState,
}

fn create_preamble() -> Preamble {
    Preamble {
        content: create_preamble_state(),
        fallback: create_preamble_state(),
    }
}

// This is a default heuristic for how to split up the HTML content into progressive
// loading. Our goal is to be able to display additional new content about every 500ms.
// Faster than that is unnecessary and should be throttled on the client. It also
// adds unnecessary overhead to do more splits. We don't know if it's a higher or lower
// end device but higher end suffer less from the overhead than lower end does from
// not getting small enough pieces. We error on the side of low end.
// We base this on low end 3G speeds which is about 500kbits per second. We assume
// that there can be a reasonable drop off from max bandwidth which leaves you with
// as little as 80%. We can receive half of that each 500ms - at best. In practice,
// a little bandwidth is lost to processing and contention - e.g. CSS and images that
// are downloaded along with the main content. So we estimate about half of that to be
// the lower end throughput. In other words, we expect that you can at least show
// about 12.5kb of content per 500ms. Not counting starting latency for the first
// paint.
// 500 * 1024 / 8 * .8 * 0.5 / 2
const DEFAULT_PROGRESSIVE_CHUNK_SIZE: f64 = 12800.0;

fn get_blocking_render_max_size(request: &Request) -> f64 {
    // We want to make sure that we can block the reveal of a well designed complete
    // shell but if you have constructed a too large shell (e.g. by not adding any
    // Suspense boundaries) then that might take too long to render. We shouldn't
    // punish users (or overzealous metrics tracking) in that scenario.
    // There's a trade off here. If this limit is too low then you can't fit a
    // reasonably well built UI within it without getting errors. If it's too high
    // then things that accidentally fall below it might take too long to load.
    // Web Vitals target 1.8 seconds for first paint and our goal to have the limit
    // be fast enough to hit that. For this argument we assume that most external
    // resources are already cached because it's a return visit, or inline styles.
    // If it's not, then it's highly unlikely that any render blocking instructions
    // we add has any impact what so ever on the paint.
    // Assuming a first byte of about 600ms which is kind of bad but common with a
    // decent static host. If it's longer e.g. due to dynamic rendering, then you
    // are going to bound by dynamic production of the content and you're better off
    // with Suspense boundaries anyway. This number doesn't matter much. Then you
    // have about 1.2 seconds left for bandwidth. On 3G that gives you about 112.5kb
    // worth of data. That's worth about 10x in terms of uncompressed bytes. Then we
    // half that just to account for longer latency, slower bandwidth and CPU processing.
    // Now we're down to about 500kb. In fact, looking at metrics we've collected with
    // rel="expect" examples and other documents, the impact on documents smaller than
    // that is within the noise. That's because there's enough happening within that
    // start up to not make HTML streaming not significantly better.
    // Content above the fold tends to be about 100-200kb tops. Therefore 500kb should
    // be enough head room for a good loading state. After that you should use
    // Suspense or SuspenseList to improve it.
    // Since this is highly related to the reason you would adjust the
    // progressiveChunkSize option, and always has to be higher, we define this limit
    // in terms of it. So if you want to increase the limit because you have high
    // bandwidth users, then you can adjust it up. If you are concerned about even
    // slower bandwidth then you can adjust it down.
    request.progressive_chunk_size * 40.0 // 512kb by default.
}

fn is_eligible_for_outlining(_request: &Request, boundary: &SuspenseBoundary) -> bool {
    // For very small boundaries, don't bother producing a fallback for outlining.
    // The larger this limit is, the more we can save on preparing fallbacks in case we end up
    // outlining.
    (boundary.byte_size.get() > 500.0
        || has_suspensey_content(boundary.content_state, /* flushingInShell */ false)
        || boundary.defer)
        // For boundaries that can possibly contribute to the preamble we don't want to outline
        // them regardless of their size since the fallbacks should only be emitted if we've
        // errored the boundary.
        && boundary.preamble.is_none()
}

fn default_error_handler(
    error: Option<&'static Unknown>,
    _error_info: &ErrorInfo,
) -> Option<&'static Unknown> {
    if js::type_of(error) == "object"
        && !matches!(js::classify(error), Kind::Null)
        && let Kind::String(environment_name) =
            js::classify(js::get(error.unwrap(), "environmentName"))
    {
        // This was a Server error. We print the environment name in a badge just like we do with
        // replays of console logs to indicate that the source of this throw as actually the Server.
        bind_to_console("error", vec![error], environment_name)();
    } else {
        console::error((error,)); // Don't transform to our wrapper
    }
    js::null()
}

// GAP rd494: upstream makes a request with `new RequestInstance(..)`, a
// constructor function, for its hidden class; a struct literal is an object
// literal.
fn request_instance(
    resumable_state: &'static ResumableState,
    render_state: &'static RenderState,
    root_format_context: &'static FormatContext,
    progressive_chunk_size: Option<f64>,
    on_error: Option<Rc<OnError>>,
    on_browser_bailout: Option<Rc<OnBrowserBailout>>,
    on_all_ready: Option<Rc<dyn Fn()>>,
    on_shell_ready: Option<Rc<dyn Fn()>>,
    on_shell_error: Option<Rc<OnFatal>>,
    on_fatal_error: Option<Rc<OnFatal>>,
    form_state: Option<&'static FormState>,
) -> Rc<Request> {
    let pinged_tasks: Vec<Rc<Task>> = vec![];
    let abort_set: AbortSet = Rc::new(RefCell::new(HashSet::new()));
    // GAP rd491: `noop` is no callback of one or two parameters, so each
    // default is a closure that ignores them.
    Rc::new(Request {
        destination: Cell::new(None),
        flush_scheduled: Cell::new(false),
        resumable_state,
        render_state,
        root_format_context,
        progressive_chunk_size: progressive_chunk_size.unwrap_or(DEFAULT_PROGRESSIVE_CHUNK_SIZE),
        // $FlowFixMe[constant-condition]
        status: Cell::new(if IS_WORK_LOOP_EXTERNALLY_DRIVEN {
            OPEN
        } else {
            OPENING
        }),
        fatal_error: Cell::new(None),
        aborted: Cell::new(false),
        next_segment_id: Cell::new(0.0),
        all_pending_tasks: Cell::new(0.0),
        pending_root_tasks: Cell::new(0.0),
        completed_root_segment: RefCell::new(None),
        completed_preamble_segments: RefCell::new(None),
        byte_size: Cell::new(0.0),
        abortable_tasks: abort_set,
        pinged_tasks: RefCell::new(pinged_tasks),
        current_task: RefCell::new(None),
        client_rendered_boundaries: RefCell::new(vec![]),
        completed_boundaries: RefCell::new(vec![]),
        partial_boundaries: RefCell::new(vec![]),
        tracked_postpones: RefCell::new(None),
        postponed_state: RefCell::new(None),
        on_error: on_error.unwrap_or_else(|| Rc::new(default_error_handler)),
        on_browser_bailout: on_browser_bailout.unwrap_or_else(|| Rc::new(|_, _| {})),
        on_all_ready: on_all_ready.unwrap_or_else(|| Rc::new(shared::noop::noop)),
        on_shell_ready: on_shell_ready.unwrap_or_else(|| Rc::new(shared::noop::noop)),
        on_shell_error: on_shell_error.unwrap_or_else(|| Rc::new(|_| {})),
        on_fatal_error: on_fatal_error.unwrap_or_else(|| Rc::new(|_| {})),
        render_lifetime_controller: Cell::new(None),
        form_state,
        did_warn_for_key: Cell::new(None),
    })
}

pub fn create_request(
    children: Option<&'static ReactNodeList>,
    resumable_state: &'static ResumableState,
    render_state: &'static RenderState,
    root_format_context: &'static FormatContext,
    progressive_chunk_size: Option<f64>,
    on_error: Option<Rc<OnError>>,
    on_browser_bailout: Option<Rc<OnBrowserBailout>>,
    on_all_ready: Option<Rc<dyn Fn()>>,
    on_shell_ready: Option<Rc<dyn Fn()>>,
    on_shell_error: Option<Rc<OnFatal>>,
    on_fatal_error: Option<Rc<OnFatal>>,
    form_state: Option<&'static FormState>,
) -> Rc<Request> {
    if __DEV__ {
        reset_owner_stack_limit();
    }

    // $FlowFixMe[invalid-constructor]: the shapes are exact here but Flow doesn't like constructors
    let request = request_instance(
        resumable_state,
        render_state,
        root_format_context,
        progressive_chunk_size,
        on_error,
        on_browser_bailout,
        on_all_ready,
        on_shell_ready,
        on_shell_error,
        on_fatal_error,
        form_state,
    );

    // This segment represents the root fallback.
    let root_segment = create_pending_segment(
        &request,
        0.0,
        None,
        root_format_context,
        // Root segments are never embedded in Text on either edge
        false,
        false,
    );
    // There is no parent so conceptually, we're unblocked to flush this segment.
    root_segment.parent_flushed.set(true);
    let root_task = create_render_task(
        &request,
        None,
        children,
        -1.0,
        None,
        root_segment,
        None,
        None,
        Rc::clone(&request.abortable_tasks),
        None,
        root_format_context,
        ROOT_CONTEXT_SNAPSHOT.with(Clone::clone),
        EMPTY_TREE_CONTEXT.with(Rc::clone),
        None,
        None,
        EMPTY_CONTEXT_OBJECT.with(|empty_context_object| *empty_context_object),
        None,
    );
    push_component_stack(&root_task);
    request.pinged_tasks.borrow_mut().push(root_task);
    request
}

pub fn create_prerender_request(
    children: Option<&'static ReactNodeList>,
    resumable_state: &'static ResumableState,
    render_state: &'static RenderState,
    root_format_context: &'static FormatContext,
    progressive_chunk_size: Option<f64>,
    on_error: Option<Rc<OnError>>,
    on_browser_bailout: Option<Rc<OnBrowserBailout>>,
    on_all_ready: Option<Rc<dyn Fn()>>,
    on_shell_ready: Option<Rc<dyn Fn()>>,
    on_shell_error: Option<Rc<OnFatal>>,
    on_fatal_error: Option<Rc<OnFatal>>,
) -> Rc<Request> {
    let request = create_request(
        children,
        resumable_state,
        render_state,
        root_format_context,
        progressive_chunk_size,
        on_error,
        on_browser_bailout,
        on_all_ready,
        on_shell_ready,
        on_shell_error,
        on_fatal_error,
        None,
    );
    // Start tracking postponed holes during this render.
    request
        .tracked_postpones
        .replace(Some(Rc::new(PostponedHoles {
            working_map: RefCell::new(HashMap::new()),
            root_nodes: Rc::new(RefCell::new(vec![])),
            root_slots: RefCell::new(ResumeSlots::None),
        })));
    request
}

pub fn resume_request(
    children: Option<&'static ReactNodeList>,
    postponed_state: &PostponedState,
    render_state: &'static RenderState,
    on_error: Option<Rc<OnError>>,
    on_browser_bailout: Option<Rc<OnBrowserBailout>>,
    on_all_ready: Option<Rc<dyn Fn()>>,
    on_shell_ready: Option<Rc<dyn Fn()>>,
    on_shell_error: Option<Rc<OnFatal>>,
    on_fatal_error: Option<Rc<OnFatal>>,
) -> Rc<Request> {
    if __DEV__ {
        reset_owner_stack_limit();
    }

    // $FlowFixMe[invalid-constructor]: the shapes are exact here but Flow doesn't like constructors
    let request = request_instance(
        postponed_state.resumable_state,
        render_state,
        postponed_state.root_format_context,
        Some(postponed_state.progressive_chunk_size),
        on_error,
        on_browser_bailout,
        on_all_ready,
        on_shell_ready,
        on_shell_error,
        on_fatal_error,
        js::null(),
    );
    request
        .next_segment_id
        .set(postponed_state.next_segment_id.get());

    if let ResumeSlots::Root(_) = *postponed_state.replay_slots.borrow() {
        // We have a resume slot at the very root. This is effectively just a full rerender.
        let root_segment = create_pending_segment(
            &request,
            0.0,
            None,
            postponed_state.root_format_context,
            // Root segments are never embedded in Text on either edge
            false,
            false,
        );
        // There is no parent so conceptually, we're unblocked to flush this segment.
        root_segment.parent_flushed.set(true);
        let root_task = create_render_task(
            &request,
            None,
            children,
            -1.0,
            None,
            root_segment,
            None,
            None,
            Rc::clone(&request.abortable_tasks),
            None,
            postponed_state.root_format_context,
            ROOT_CONTEXT_SNAPSHOT.with(Clone::clone),
            EMPTY_TREE_CONTEXT.with(Rc::clone),
            None,
            None,
            EMPTY_CONTEXT_OBJECT.with(|empty_context_object| *empty_context_object),
            None,
        );
        push_component_stack(&root_task);
        request.pinged_tasks.borrow_mut().push(root_task);
        return request;
    }

    let replay = Rc::new(ReplaySet {
        nodes: Rc::clone(&postponed_state.replay_nodes),
        slots: RefCell::new(postponed_state.replay_slots.borrow().clone()),
        pending_tasks: Cell::new(0.0),
    });
    let root_task = create_replay_task(
        &request,
        None,
        replay,
        children,
        -1.0,
        None,
        None,
        Rc::clone(&request.abortable_tasks),
        None,
        postponed_state.root_format_context,
        ROOT_CONTEXT_SNAPSHOT.with(Clone::clone),
        EMPTY_TREE_CONTEXT.with(Rc::clone),
        None,
        None,
        EMPTY_CONTEXT_OBJECT.with(|empty_context_object| *empty_context_object),
        None,
    );
    push_component_stack(&root_task);
    request.pinged_tasks.borrow_mut().push(root_task);
    request
}

pub fn resume_and_prerender_request(
    children: Option<&'static ReactNodeList>,
    postponed_state: &PostponedState,
    render_state: &'static RenderState,
    on_error: Option<Rc<OnError>>,
    on_browser_bailout: Option<Rc<OnBrowserBailout>>,
    on_all_ready: Option<Rc<dyn Fn()>>,
    on_shell_ready: Option<Rc<dyn Fn()>>,
    on_shell_error: Option<Rc<OnFatal>>,
    on_fatal_error: Option<Rc<OnFatal>>,
) -> Rc<Request> {
    let request = resume_request(
        children,
        postponed_state,
        render_state,
        on_error,
        on_browser_bailout,
        on_all_ready,
        on_shell_ready,
        on_shell_error,
        on_fatal_error,
    );
    // Start tracking postponed holes during this render.
    request
        .tracked_postpones
        .replace(Some(Rc::new(PostponedHoles {
            working_map: RefCell::new(HashMap::new()),
            root_nodes: Rc::new(RefCell::new(vec![])),
            root_slots: RefCell::new(ResumeSlots::None),
        })));
    request
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::nullable)]
    static CURRENT_REQUEST: RefCell<Option<Rc<Request>>> = const { RefCell::new(None) };
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn resolve_request() -> Option<Rc<Request>> {
    if let Some(current_request) = CURRENT_REQUEST.with_borrow(Clone::clone) {
        return Some(current_request);
    }
    // $FlowFixMe[constant-condition]
    if SUPPORTS_REQUEST_STORAGE {
        let store = REQUEST_STORAGE.get_store();
        if let Some(store) = store {
            return Some(store);
        }
    }
    None
}

fn ping_task(request: &Rc<Request>, task: Rc<Task>) {
    let pinged_tasks = &request.pinged_tasks;
    pinged_tasks.borrow_mut().push(task);
    // $FlowFixMe[constant-condition]
    if IS_WORK_LOOP_EXTERNALLY_DRIVEN {
        return;
    }
    if request.pinged_tasks.borrow().len() == 1 {
        request
            .flush_scheduled
            .set(request.destination.get().is_some());
        let request = Rc::clone(request);
        if request.tracked_postpones.borrow().is_some() || request.status.get() == OPENING {
            schedule_microtask(move || perform_work(&request));
        } else {
            schedule_work(move || perform_work(&request));
        }
    }
}

fn ping_rejected_task(request: &Rc<Request>, task: Rc<Task>, error: Option<&'static Unknown>) {
    if !request.aborted.get() {
        // Replaying the task is what gives ordinary render errors their complete
        // component stack.
        ping_task(request, task);
        return;
    }
    let abort_set = Rc::clone(&task.abort_set.borrow());
    if !abort_set.borrow_mut().remove(&task) {
        // finishAbort already completed this task with the request's abort reason.
        return;
    }
    // abortTask synchronously claimed this task before abort listeners could
    // reject its wakeable. Finish it with the more specific reason before the
    // scheduled final abort uses the reason for the whole request.
    if __DEV__ {
        finish_aborted_task_dev(&task, request, error);
    } else {
        finish_aborted_task(&task, request, error);
    }
}

fn create_suspense_boundary(
    request: &Request,
    row: Option<Rc<SuspenseListRow>>,
    fallback_abortable_tasks: AbortSet,
    preamble: Option<Rc<Preamble>>,
    defer: bool,
) -> Rc<SuspenseBoundary> {
    let boundary = Rc::new(SuspenseBoundary {
        status: Cell::new(PENDING),
        root_segment_id: Cell::new(-1.0),
        parent_flushed: Cell::new(false),
        pending_tasks: Cell::new(0.0),
        row: RefCell::new(row.clone()),
        completed_segments: RefCell::new(vec![]),
        byte_size: Cell::new(0.0),
        defer,
        fallback_abortable_tasks,
        error_digest: RefCell::new(js::null()),
        content_state: create_hoistable_state(),
        fallback_state: create_hoistable_state(),
        preamble,
        tracked: RefCell::new(None),
        // DEV-only fields for hidden class
        // GAP rd493: set in production too, `null`.
        error_message: RefCell::new(None),
        error_stack: RefCell::new(None),
        error_component_stack: RefCell::new(None),
    });
    if let Some(row) = row {
        // This boundary will block this row from completing.
        row.pending_tasks.set(row.pending_tasks.get() + 1.0);
        let mut blocked_boundaries = row.boundaries.borrow_mut();
        if let Some(blocked_boundaries) = blocked_boundaries.as_mut() {
            // Previous rows will block this boundary itself from completing.
            request
                .all_pending_tasks
                .set(request.all_pending_tasks.get() + 1.0);
            boundary
                .pending_tasks
                .set(boundary.pending_tasks.get() + 1.0);
            blocked_boundaries.push(Rc::clone(&boundary));
        }
        let inherited_hoistables = row.inherited_hoistables.get();
        if let Some(inherited_hoistables) = inherited_hoistables {
            hoist_hoistables(boundary.content_state, inherited_hoistables);
        }
    }
    boundary
}

fn ping_of(request: &Rc<Request>, task: &Weak<Task>) -> Ping {
    let (resolve_request, resolve_task) = (Rc::clone(request), Weak::clone(task));
    let (reject_request, reject_task) = (Rc::clone(request), Weak::clone(task));
    Ping {
        resolve: Rc::new(move || ping_task(&resolve_request, resolve_task.upgrade().unwrap())),
        reject: Rc::new(move |error| {
            ping_rejected_task(&reject_request, reject_task.upgrade().unwrap(), error)
        }),
    }
}

fn create_render_task(
    request: &Rc<Request>,
    thenable_state: Option<Rc<ThenableState>>,
    node: Option<&'static ReactNodeList>,
    child_index: f64,
    blocked_boundary: Option<Rc<SuspenseBoundary>>,
    blocked_segment: Rc<Segment>,
    blocked_preamble: Option<&'static PreambleState>,
    hoistable_state: Option<&'static HoistableState>,
    abort_set: AbortSet,
    key_path: Option<Rc<KeyNode>>,
    format_context: &'static FormatContext,
    context: ContextSnapshot,
    tree_context: Rc<TreeContext>,
    row: Option<Rc<SuspenseListRow>>,
    component_stack: Option<Rc<ComponentStackNode>>,
    legacy_context: &'static LegacyContext,
    debug_task: Option<&'static ConsoleTask>,
) -> Rc<Task> {
    request
        .all_pending_tasks
        .set(request.all_pending_tasks.get() + 1.0);
    if let Some(blocked_boundary) = &blocked_boundary {
        blocked_boundary
            .pending_tasks
            .set(blocked_boundary.pending_tasks.get() + 1.0);
    } else {
        request
            .pending_root_tasks
            .set(request.pending_root_tasks.get() + 1.0);
    }
    if let Some(row) = &row {
        row.pending_tasks.set(row.pending_tasks.get() + 1.0);
    }
    // The task's ping closures name the task they're in, which Rust makes
    // with `Rc::new_cyclic` and a `Weak` (rd96), where upstream's arrows
    // close over the `const task` they initialize.
    let task = Rc::new_cyclic(|task: &Weak<Task>| Task {
        replay: RefCell::new(None),
        node: Cell::new(node),
        child_index: Cell::new(child_index),
        ping: ping_of(request, task),
        blocked_boundary: RefCell::new(blocked_boundary),
        blocked_segment: RefCell::new(Some(blocked_segment)),
        blocked_preamble: Cell::new(blocked_preamble),
        hoistable_state: Cell::new(hoistable_state),
        abort_set: RefCell::new(Rc::clone(&abort_set)),
        key_path: RefCell::new(key_path),
        format_context: Cell::new(format_context),
        context: RefCell::new(context),
        tree_context: RefCell::new(tree_context),
        row: RefCell::new(row),
        component_stack: RefCell::new(component_stack),
        thenable_state: RefCell::new(thenable_state),
        legacy_context: Cell::new(if !DISABLE_LEGACY_CONTEXT {
            legacy_context
        } else {
            EMPTY_CONTEXT_OBJECT.with(|empty_context_object| *empty_context_object)
        }),
        debug_task: Cell::new(if __DEV__ { debug_task } else { None }),
    });
    abort_set.borrow_mut().insert(Rc::clone(&task));
    task
}

fn create_replay_task(
    request: &Rc<Request>,
    thenable_state: Option<Rc<ThenableState>>,
    replay: Rc<ReplaySet>,
    node: Option<&'static ReactNodeList>,
    child_index: f64,
    blocked_boundary: Option<Rc<SuspenseBoundary>>,
    hoistable_state: Option<&'static HoistableState>,
    abort_set: AbortSet,
    key_path: Option<Rc<KeyNode>>,
    format_context: &'static FormatContext,
    context: ContextSnapshot,
    tree_context: Rc<TreeContext>,
    row: Option<Rc<SuspenseListRow>>,
    component_stack: Option<Rc<ComponentStackNode>>,
    legacy_context: &'static LegacyContext,
    debug_task: Option<&'static ConsoleTask>,
) -> Rc<Task> {
    request
        .all_pending_tasks
        .set(request.all_pending_tasks.get() + 1.0);
    if let Some(blocked_boundary) = &blocked_boundary {
        blocked_boundary
            .pending_tasks
            .set(blocked_boundary.pending_tasks.get() + 1.0);
    } else {
        request
            .pending_root_tasks
            .set(request.pending_root_tasks.get() + 1.0);
    }
    if let Some(row) = &row {
        row.pending_tasks.set(row.pending_tasks.get() + 1.0);
    }
    replay.pending_tasks.set(replay.pending_tasks.get() + 1.0);
    let task = Rc::new_cyclic(|task: &Weak<Task>| Task {
        replay: RefCell::new(Some(replay)),
        node: Cell::new(node),
        child_index: Cell::new(child_index),
        ping: ping_of(request, task),
        blocked_boundary: RefCell::new(blocked_boundary),
        blocked_segment: RefCell::new(None),
        blocked_preamble: Cell::new(None),
        hoistable_state: Cell::new(hoistable_state),
        abort_set: RefCell::new(Rc::clone(&abort_set)),
        key_path: RefCell::new(key_path),
        format_context: Cell::new(format_context),
        context: RefCell::new(context),
        tree_context: RefCell::new(tree_context),
        row: RefCell::new(row),
        component_stack: RefCell::new(component_stack),
        thenable_state: RefCell::new(thenable_state),
        legacy_context: Cell::new(if !DISABLE_LEGACY_CONTEXT {
            legacy_context
        } else {
            EMPTY_CONTEXT_OBJECT.with(|empty_context_object| *empty_context_object)
        }),
        debug_task: Cell::new(if __DEV__ { debug_task } else { None }),
    });
    abort_set.borrow_mut().insert(Rc::clone(&task));
    task
}

fn create_pending_segment(
    _request: &Request,
    index: f64,
    boundary: Option<Rc<SuspenseBoundary>>,
    parent_format_context: &'static FormatContext,
    last_pushed_text: bool,
    text_embedded: bool,
) -> Rc<Segment> {
    Rc::new(Segment {
        status: Cell::new(PENDING),
        parent_flushed: Cell::new(false),
        id: Cell::new(-1.0), // lazily assigned later
        index,
        chunks: RefCell::new(vec![]),
        children: RefCell::new(vec![]),
        preamble_children: RefCell::new(vec![]),
        parent_format_context,
        boundary: RefCell::new(boundary),
        last_pushed_text: Cell::new(last_pushed_text),
        text_embedded: Cell::new(text_embedded),
    })
}

fn get_current_stack_in_dev() -> String {
    if __DEV__ {
        let Some(current_task_in_dev) = CURRENT_TASK_IN_DEV.with_borrow(Clone::clone) else {
            return String::new();
        };
        let Some(component_stack) = current_task_in_dev.component_stack.borrow().clone() else {
            return String::new();
        };
        return get_owner_stack_by_component_stack_node_in_dev(&component_stack);
    }
    String::new()
}

fn get_stack_from_node(stack_node: &Rc<ComponentStackNode>) -> String {
    get_stack_by_component_stack_node(stack_node)
}

fn push_halted_await_on_component_stack(task: &Task, debug_info: Option<&'static Unknown>) {
    if !__DEV__ {
        // eslint-disable-next-line react-internal/prod-error-codes
        panic!(
            "pushHaltedAwaitOnComponentStack should never be called in production. This is a bug in React."
        );
    }
    if let Some(debug_info) = debug_info {
        let debug_info: &'static Vec<Option<&'static Unknown>> =
            unsafe { js::cast(Some(debug_info)) };
        for &info in debug_info.iter().rev() {
            if js::get(info, "awaited").is_some() {
                let async_info = info;
                let best_stack = if js::get(async_info, "debugStack").is_none() {
                    js::get(async_info, "awaited")
                } else {
                    async_info
                };
                if !matches!(
                    js::classify(js::get(best_stack, "debugStack")),
                    Kind::Undefined
                ) {
                    let parent = task.component_stack.take();
                    task.component_stack
                        .replace(Some(Rc::new(ComponentStackNode {
                            parent,
                            r#type: async_info.unwrap(),
                            owner: js::get(best_stack, "owner"),
                            stack: Cell::new(js::get(best_stack, "debugStack")),
                        })));
                    task.debug_task
                        .set(unsafe { js::cast(js::get(best_stack, "debugTask")) });
                    break;
                }
            }
        }
    }
}

// performWork + retryTask without mutation
fn rerender_stalled_task(request: &Rc<Request>, task: &Rc<Task>) {
    let prev_status = request.status.get();
    let prev_aborted = request.aborted.get();
    request.status.set(STALLED_DEV);
    // This diagnostic replay must reach the suspended call site instead of
    // taking the abort path.
    request.aborted.set(false);

    let prev_context = get_active_context();
    let prev_dispatcher = REACT_SHARED_INTERNALS.h();
    HOOKS_DISPATCHER.with(|hooks_dispatcher| REACT_SHARED_INTERNALS.set_h(Some(hooks_dispatcher)));
    let prev_async_dispatcher = REACT_SHARED_INTERNALS.a();
    DEFAULT_ASYNC_DISPATCHER.with(|default_async_dispatcher| {
        REACT_SHARED_INTERNALS.set_a(Some(default_async_dispatcher))
    });

    let prev_request = CURRENT_REQUEST.replace(Some(Rc::clone(request)));

    let prev_get_current_stack_impl = REACT_SHARED_INTERNALS.get_current_stack();
    REACT_SHARED_INTERNALS.set_get_current_stack(Some(get_current_stack_in_dev));

    let prev_resumable_state = CURRENT_RESUMABLE_STATE.get();
    set_current_resumable_state(Some(request.resumable_state));
    switch_context(task.context.borrow().clone());
    let prev_task_in_dev = CURRENT_TASK_IN_DEV.with_borrow(Clone::clone);
    set_current_task_in_dev(Some(Rc::clone(task)));
    let result = catch_unwind(AssertUnwindSafe(|| retry_node(request, task)));
    if result.is_err() {
        // Suspended again.
        reset_hooks_state();
    }
    set_current_task_in_dev(prev_task_in_dev);
    set_current_resumable_state(prev_resumable_state);

    REACT_SHARED_INTERNALS.set_h(prev_dispatcher);
    REACT_SHARED_INTERNALS.set_a(prev_async_dispatcher);

    // GAP rd496 (blocking): `getCurrentStack` reads back a JS function,
    // `&dyn Fn`, where its setter takes a `fn`, so what was read can't be put
    // back; this puts back this renderer's own where one was set, which is
    // wrong where another renderer's was.
    REACT_SHARED_INTERNALS.set_get_current_stack(
        prev_get_current_stack_impl.map(|_| get_current_stack_in_dev as fn() -> String),
    );
    if HOOKS_DISPATCHER.with(|hooks_dispatcher| {
        js::strict_equals(
            prev_dispatcher.map(js::unknown),
            Some(js::unknown(hooks_dispatcher)),
        )
    }) {
        // This means that we were in a reentrant work loop. This could happen
        // in a renderer that supports synchronous work like renderToString,
        // when it's called from within another renderer.
        // Normally we don't bother switching the contexts to their root/default
        // values when leaving because we'll likely need the same or similar
        // context again. However, when we're inside a synchronous loop like this
        // we'll to restore the context to what it was before returning.
        switch_context(prev_context);
    }
    CURRENT_REQUEST.set(prev_request);
    request.status.set(prev_status);
    request.aborted.set(prev_aborted);
}

fn push_suspended_call_site_on_component_stack(request: &Rc<Request>, task: &Rc<Task>) {
    set_capture_suspended_call_site_dev(true);
    let restore_thenable_state = ensure_suspendable_thenable_state_dev(
        // refined at the callsite
        task.thenable_state.borrow().as_ref().unwrap(),
    );
    let result = catch_unwind(AssertUnwindSafe(|| rerender_stalled_task(request, task)));
    restore_thenable_state();
    set_capture_suspended_call_site_dev(false);
    if let Err(error) = result {
        resume_unwind(error);
    }

    let suspend_call_site_stack = get_suspended_call_site_stack_dev();
    let suspend_call_site_debug_task = get_suspended_call_site_debug_task_dev();

    if let Some(suspend_call_site_stack) = suspend_call_site_stack {
        let owner_stack = task.component_stack.take();
        task.component_stack
            .replace(Some(Rc::new(ComponentStackNode {
                // The owner of the suspended call site would be the owner of this task.
                // We need the task itself otherwise we'd miss a frame.
                // GAP rd495: a node an `Rc` holds is no `&'static Unknown`, the
                // owner field's type, without a cast.
                owner: owner_stack
                    .map(|owner_stack| unsafe { js::cast(Some(js::unknown(&*owner_stack))) }),
                parent: suspend_call_site_stack.parent.clone(),
                stack: Cell::new(suspend_call_site_stack.stack.get()),
                r#type: suspend_call_site_stack.r#type,
            })));
    }
    task.debug_task.set(suspend_call_site_debug_task);
}

fn push_server_component_stack(task: &Task, debug_info: Option<&'static Unknown>) {
    if !__DEV__ {
        // eslint-disable-next-line react-internal/prod-error-codes
        panic!(
            "pushServerComponentStack should never be called in production. This is a bug in React."
        );
    }
    // Build a Server Component parent stack from the debugInfo.
    if let Some(debug_info) = debug_info {
        let stack: &'static Vec<Option<&'static Unknown>> = unsafe { js::cast(Some(debug_info)) };
        for &component_info in stack.iter() {
            if !matches!(
                js::classify(js::get(component_info, "name")),
                Kind::String(_)
            ) {
                continue;
            }
            if matches!(
                js::classify(js::get(component_info, "debugStack")),
                Kind::Undefined
            ) {
                continue;
            }
            let parent = task.component_stack.take();
            task.component_stack
                .replace(Some(Rc::new(ComponentStackNode {
                    parent,
                    r#type: component_info.unwrap(),
                    owner: js::get(component_info, "owner"),
                    stack: Cell::new(js::get(component_info, "debugStack")),
                })));
            task.debug_task
                .set(unsafe { js::cast(js::get(component_info, "debugTask")) });
        }
    }
}

fn push_component_stack(task: &Task) {
    let node = task.node.get();
    // Create the Component Stack frame for the element we're about to try.
    // It's unfortunate that we need to do this refinement twice. Once for
    // the stack frame and then once again while actually
    if js::type_of(node) == "object" && !matches!(js::classify(node), Kind::Null) {
        let node = node.unwrap();
        match SymbolFor::from_value(js::get(node, "$$typeof")) {
            Some(REACT_ELEMENT_TYPE) => {
                let element = node;
                let r#type = js::get(element, "type");
                let owner = if __DEV__ {
                    js::get(element, "_owner")
                } else {
                    js::null()
                };
                let stack = if __DEV__ {
                    js::get(element, "_debugStack")
                } else {
                    js::null()
                };
                if __DEV__ {
                    push_server_component_stack(task, js::get(element, "_debugInfo"));
                    task.debug_task
                        .set(unsafe { js::cast(js::get(element, "_debugTask")) });
                }
                let parent = task.component_stack.take();
                task.component_stack
                    .replace(Some(create_component_stack_from_type(
                        parent,
                        r#type.unwrap(),
                        owner,
                        stack,
                    )));
            }
            Some(REACT_LAZY_TYPE) => {
                if __DEV__ {
                    let lazy_node: &LazyComponent = unsafe { js::cast(Some(node)) };
                    push_server_component_stack(task, js::get(lazy_node, "_debugInfo"));
                }
            }
            _ => {
                if __DEV__ {
                    let maybe_usable = node;
                    if js::is_function(js::get(maybe_usable, "then")) {
                        let thenable = maybe_usable;
                        push_server_component_stack(task, js::get(thenable, "_debugInfo"));
                    }
                }
            }
        }
    }
}

fn create_component_stack_from_type(
    parent: Option<Rc<ComponentStackNode>>,
    r#type: &'static Unknown,
    owner: Option<&'static Unknown>, // DEV only
    stack: Option<&'static Unknown>, // DEV only
) -> Rc<ComponentStackNode> {
    if __DEV__ {
        return Rc::new(ComponentStackNode {
            parent,
            r#type,
            owner,
            stack: Cell::new(stack),
        });
    }
    Rc::new(ComponentStackNode {
        parent,
        r#type,
        owner: None,
        stack: Cell::new(None),
    })
}

fn replace_suspense_component_stack_with_suspense_fallback_stack(
    component_stack: Option<&Rc<ComponentStackNode>>,
) -> Option<Rc<ComponentStackNode>> {
    let component_stack = component_stack?;
    Some(create_component_stack_from_type(
        component_stack.parent.clone(),
        js::unknown("Suspense Fallback"),
        if __DEV__ {
            component_stack.owner
        } else {
            js::null()
        },
        if __DEV__ {
            component_stack.stack.get()
        } else {
            js::null()
        },
    ))
}

fn get_thrown_info(node: Option<Rc<ComponentStackNode>>) -> &'static ThrownInfo {
    let error_info: &'static JsObject = js::object::spread(None::<&Unknown>);
    if let Some(node) = node {
        let get = move || {
            // Lazyily generate the stack since it's expensive.
            let stack = get_stack_from_node(&node);
            js::object::define_property(
                error_info,
                "componentStack",
                &PropertyDescriptor {
                    value: Some(&stack),
                    ..Default::default()
                },
            );
            // GAP rd495: a getter gives a `&'static Unknown`, which the text
            // it made is only by a cast.
            unsafe { js::cast(Some(js::unknown(stack.as_str()))) }
        };
        js::object::define_property(
            error_info,
            "componentStack",
            &PropertyDescriptor {
                configurable: Some(true),
                enumerable: Some(true),
                get: Some(&get),
                ..Default::default()
            },
        );
    }
    unsafe { js::cast(Some(js::unknown(error_info))) }
}

fn encode_error_for_boundary(
    boundary: &SuspenseBoundary,
    digest: Option<String>,
    error: Option<&'static Unknown>,
    thrown_info: &ThrownInfo,
    was_aborted: bool,
) {
    boundary.error_digest.replace(digest);
    if __DEV__ {
        if is_recoverable_error(error) {
            boundary.error_message.replace(Some(
                if was_aborted {
                    concat!(
                        "Switched to client rendering because the server render was aborted ",
                        "with a request to render on the client."
                    )
                } else {
                    "Switched to client rendering because a component requested it."
                }
                .to_owned(),
            ));
            boundary
                .error_component_stack
                .replace(thrown_info.component_stack());
            return;
        }
        // In dev we additionally encode the error message and component stack on the boundary
        let (message, stack) = if js::instance_of(error, js::class::<js::Error>()) {
            let error = error.unwrap();
            (
                // eslint-disable-next-line react-internal/safe-string-coercion
                js::string(js::get(error, "message")),
                // eslint-disable-next-line react-internal/safe-string-coercion
                Some(js::string(js::get(error, "stack"))),
            )
        } else if js::type_of(error) == "object" && !matches!(js::classify(error), Kind::Null) {
            (
                describe_object_for_error_message(error.unwrap(), None),
                None,
            )
        } else {
            // eslint-disable-next-line react-internal/safe-string-coercion
            (js::string(error), None)
        };
        let prefix = if was_aborted {
            "Switched to client rendering because the server rendering aborted due to:\n\n"
        } else {
            "Switched to client rendering because the server rendering errored:\n\n"
        };
        boundary
            .error_message
            .replace(Some(prefix.to_owned() + &message));
        boundary
            .error_stack
            .replace(stack.map(|stack| prefix.to_owned() + &stack));
        boundary
            .error_component_stack
            .replace(thrown_info.component_stack());
    }
}

fn log_recoverable_error(
    request: &Request,
    error: Option<&'static Unknown>,
    error_info: &'static ThrownInfo,
    debug_task: Option<&'static ConsoleTask>,
) -> Option<String> {
    if is_recoverable_error(error) {
        log_browser_bailout(request, error, error_info, debug_task);
        return Some(REACT_RECOVERABLE_DIGEST.to_owned());
    }

    // If this callback errors, we intentionally let that error bubble up to become a fatal error
    // so that someone fixes the error reporting instead of hiding it.
    let on_error = &request.on_error;
    let error_digest = if __DEV__ && let Some(debug_task) = debug_task {
        debug_task.run(|| on_error(error, error_info))
    } else {
        on_error(error, error_info)
    };
    let error_digest = match js::classify(error_digest) {
        Kind::Null | Kind::Undefined => None,
        Kind::String(error_digest) => Some(error_digest.to_owned()),
        _ => {
            // We used to throw here but since this gets called from a variety of unprotected places it
            // seems better to just warn and discard the returned value.
            if __DEV__ {
                console::error((
                    "onError returned something with a type other than \"string\". onError should return a string and may return null or undefined but must not return anything else. It received something of type \"%s\" instead",
                    js::type_of(error_digest),
                ));
            }
            return None;
        }
    };
    // An empty digest is reserved for React's internal client-render signal.
    // Historically an empty digest was omitted from the wire format, so
    // normalizing it to undefined preserves the existing user-space semantics.
    error_digest.filter(|error_digest| !error_digest.is_empty())
}

fn log_browser_bailout(
    request: &Request,
    error: Option<&'static Unknown>,
    error_info: &ThrownInfo,
    debug_task: Option<&'static ConsoleTask>,
) {
    // If this callback errors, we intentionally let that error bubble up to
    // become a fatal error, matching the behavior of onError.
    let on_browser_bailout = &request.on_browser_bailout;
    if __DEV__ && let Some(debug_task) = debug_task {
        debug_task.run(|| on_browser_bailout(error, error_info));
    } else {
        on_browser_bailout(error, error_info);
    }
}

fn fatal_error(
    request: &Request,
    error: Option<&'static Unknown>,
    _error_info: &ThrownInfo,
    debug_task: Option<&'static ConsoleTask>,
) {
    // This is called outside error handling code such as if the root errors outside
    // a suspense boundary or if the root suspense boundary's fallback errors.
    // It's also called if React itself or its host configs errors.
    let on_shell_error = &request.on_shell_error;
    let on_fatal_error = &request.on_fatal_error;
    // Once the shell has completed it can't error anymore, so onShellError only
    // fires while root tasks are still pending. onFatalError always fires because
    // the error is always fatal to the request.
    let shell_complete = request.pending_root_tasks.get() == 0.0;
    if __DEV__ && let Some(debug_task) = debug_task {
        if !shell_complete {
            debug_task.run(|| on_shell_error(error));
        }
        debug_task.run(|| on_fatal_error(error));
    } else {
        if !shell_complete {
            on_shell_error(error);
        }
        on_fatal_error(error);
    }
    end_render_lifetime(request);
    if let Some(destination) = request.destination.get() {
        request.status.set(CLOSED);
        close_with_error(destination, error);
    } else {
        request.status.set(CLOSING);
        // abort() already stored the reason that every remaining task must
        // observe. This error may only be a fatal diagnostic derived from it.
        if !request.aborted.get() {
            request.fatal_error.set(error);
        }
    }
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#getSuspenseContentFormatContext"
)]
fn get_suspense_content_format_context(
    resumable_state: &ResumableState,
    parent_context: &FormatContext,
) -> &'static FormatContext {
    let _ = (resumable_state, parent_context);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#getSuspenseFallbackFormatContext"
)]
fn get_suspense_fallback_format_context(
    resumable_state: &ResumableState,
    parent_context: &FormatContext,
) -> &'static FormatContext {
    let _ = (resumable_state, parent_context);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#canHavePreamble"
)]
fn can_have_preamble(format_context: &FormatContext) -> bool {
    let _ = format_context;
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#pushSegmentFinale"
)]
fn push_segment_finale(
    target: &Chunks,
    render_state: &RenderState,
    last_pushed_text: bool,
    text_embedded: bool,
) {
    let _ = (target, render_state, last_pushed_text, text_embedded);
    unreachable!()
}

/// What a `catch` is given: the JS value thrown, or `None` of a panic.
fn thrown_value_of(payload: &(dyn std::any::Any + Send)) -> Option<&'static Unknown> {
    payload
        .downcast_ref::<&'static JsError>()
        .and_then(|error| error.as_unknown())
}

fn fallback_key_path_of(key_path: &KeyNode) -> Rc<KeyNode> {
    Rc::new(KeyNode(
        key_path.0.clone(),
        Some("Suspense Fallback".to_owned()),
        key_path.2.clone(),
    ))
}

fn render_suspense_boundary(
    request: &Rc<Request>,
    some_task: &Rc<Task>,
    key_path: &Rc<KeyNode>,
    props: &SuspenseProps,
) {
    if some_task.replay.borrow().is_some() {
        // If we're replaying through this pass, it means we're replaying through
        // an already completed Suspense boundary. It's too late to do anything about it
        // so we can just render through it.
        let prev_key_path = some_task.key_path.replace(Some(Rc::clone(key_path)));
        let prev_context = some_task.format_context.get();
        let prev_row = some_task.row.take();
        some_task
            .format_context
            .set(get_suspense_content_format_context(
                request.resumable_state,
                prev_context,
            ));
        let content = props.children;
        let result = catch_unwind(AssertUnwindSafe(|| {
            render_node(request, some_task, content, -1.0);
        }));
        some_task.key_path.replace(prev_key_path);
        some_task.format_context.set(prev_context);
        some_task.row.replace(prev_row);
        if let Err(thrown_value) = result {
            resume_unwind(thrown_value);
        }
        return;
    }
    // $FlowFixMe[incompatible-type]: Refined.
    let task = some_task;

    let prev_key_path = task.key_path.borrow().clone();
    let prev_context = task.format_context.get();
    let prev_row = task.row.borrow().clone();
    let parent_boundary = task.blocked_boundary.borrow().clone();
    let parent_preamble = task.blocked_preamble.get();
    let parent_hoistable_state = task.hoistable_state.get();
    let parent_segment = task.blocked_segment.borrow().clone().unwrap();

    // Each time we enter a suspense boundary, we split out into a new segment for
    // the fallback so that we can later replace that segment with the content.
    // This also lets us split out the main content even if it doesn't suspend,
    // in case it ends up generating a large subtree of content.
    let fallback = props.fallback;
    let content = props.children;
    let defer = ENABLE_CPU_SUSPENSE.with(|&enabled| enabled) && props.defer == Some(true);

    let fallback_abort_set: AbortSet = Rc::new(RefCell::new(HashSet::new()));
    let new_boundary = create_suspense_boundary(
        request,
        task.row.borrow().clone(),
        Rc::clone(&fallback_abort_set),
        if can_have_preamble(task.format_context.get()) {
            Some(Rc::new(create_preamble()))
        } else {
            None
        },
        defer,
    );

    let insertion_index = parent_segment.chunks.borrow().len() as f64;
    // The children of the boundary segment is actually the fallback.
    let boundary_segment = create_pending_segment(
        request,
        insertion_index,
        Some(Rc::clone(&new_boundary)),
        task.format_context.get(),
        // boundaries never require text embedding at their edges because comment nodes bound them
        false,
        false,
    );
    parent_segment
        .children
        .borrow_mut()
        .push(Rc::clone(&boundary_segment));
    // The parentSegment has a child Segment at this index so we reset the lastPushedText marker on the parent
    parent_segment.last_pushed_text.set(false);

    // This segment is the actual child content. We can start rendering that immediately.
    let content_root_segment = create_pending_segment(
        request,
        0.0,
        None,
        task.format_context.get(),
        // boundaries never require text embedding at their edges because comment nodes bound them
        false,
        false,
    );
    // We mark the root segment as having its parent flushed. It's not really flushed but there is
    // no parent segment so there's nothing to wait on.
    content_root_segment.parent_flushed.set(true);

    let tracked_postpones = request.tracked_postpones.borrow().clone();
    if tracked_postpones.is_some() || defer {
        // This is a prerender or deferred boundary. In this mode we want to render the fallback synchronously
        // and schedule the content to render later. This is the opposite of what we do during a normal render
        // where we try to skip rendering the fallback if the content itself can render synchronously

        // Stash the original stack frame.
        let suspense_component_stack = task.component_stack.borrow().clone();

        let fallback_key_path = fallback_key_path_of(key_path);
        if let Some(tracked_postpones) = &tracked_postpones {
            let fallback_replay_node = ReplayNode::Element(Rc::new(ReplayElement(
                fallback_key_path.1.clone(),
                fallback_key_path.2.clone(),
                Rc::new(RefCell::new(vec![])),
                RefCell::new(ResumeSlots::None),
            )));
            tracked_postpones
                .working_map
                .borrow_mut()
                .insert(Rc::clone(&fallback_key_path), fallback_replay_node.clone());
            new_boundary.tracked.replace(Some(Rc::new(Tracked {
                content_key_path: RefCell::new(Some(Rc::clone(key_path))),
                // We are rendering the fallback before the boundary content so we keep track of
                // the fallback replay node until we determine if the primary content suspends
                fallback_node: RefCell::new(Some(fallback_replay_node)),
            })));
        }

        task.blocked_segment
            .replace(Some(Rc::clone(&boundary_segment)));
        task.blocked_preamble.set(
            new_boundary
                .preamble
                .as_ref()
                .map(|preamble| preamble.fallback),
        );
        task.key_path.replace(Some(fallback_key_path));
        task.format_context
            .set(get_suspense_fallback_format_context(
                request.resumable_state,
                prev_context,
            ));
        task.component_stack.replace(
            replace_suspense_component_stack_with_suspense_fallback_stack(
                suspense_component_stack.as_ref(),
            ),
        );
        let result = catch_unwind(AssertUnwindSafe(|| {
            render_node(request, task, fallback, -1.0);
            push_segment_finale(
                &boundary_segment.chunks,
                request.render_state,
                boundary_segment.last_pushed_text.get(),
                boundary_segment.text_embedded.get(),
            );
            boundary_segment.status.set(COMPLETED);
            finished_segment(request, parent_boundary.as_ref(), &boundary_segment);
        }));
        if result.is_err() {
            if request.aborted.get() {
                boundary_segment.status.set(ABORTED);
            } else {
                boundary_segment.status.set(ERRORED);
            }
        }
        task.blocked_segment.replace(Some(parent_segment));
        task.blocked_preamble.set(parent_preamble);
        task.key_path.replace(prev_key_path);
        task.format_context.set(prev_context);
        if let Err(thrown_value) = result {
            resume_unwind(thrown_value);
        }

        // We create a suspended task for the primary content because we want to allow
        // sibling fallbacks to be rendered first.
        let suspended_primary_task = create_render_task(
            request,
            None,
            content,
            -1.0,
            Some(Rc::clone(&new_boundary)),
            content_root_segment,
            new_boundary
                .preamble
                .as_ref()
                .map(|preamble| preamble.content),
            Some(new_boundary.content_state),
            Rc::clone(&task.abort_set.borrow()),
            Some(Rc::clone(key_path)),
            get_suspense_content_format_context(request.resumable_state, task.format_context.get()),
            task.context.borrow().clone(),
            Rc::clone(&task.tree_context.borrow()),
            None, // The row gets reset inside the Suspense boundary.
            suspense_component_stack,
            if !DISABLE_LEGACY_CONTEXT {
                task.legacy_context.get()
            } else {
                EMPTY_CONTEXT_OBJECT.with(|empty_context_object| *empty_context_object)
            },
            if __DEV__ { task.debug_task.get() } else { None },
        );
        push_component_stack(&suspended_primary_task);
        request
            .pinged_tasks
            .borrow_mut()
            .push(suspended_primary_task);
    } else {
        // This is a normal render. We will attempt to synchronously render the boundary content
        // If it is successful we will elide the fallback task but if it suspends or errors we schedule
        // the fallback to render. Unlike with prerenders we attempt to deprioritize the fallback render

        // Currently this is running synchronously. We could instead schedule this to pingedTasks.
        // I suspect that there might be some efficiency benefits from not creating the suspended task
        // and instead just using the stack if possible.
        // TODO: Call this directly instead of messing with saving and restoring contexts.

        // We can reuse the current context and task to render the content immediately without
        // context switching. We just need to temporarily switch which boundary and which segment
        // we're writing to. If something suspends, it'll spawn new suspended task with that context.
        task.blocked_boundary
            .replace(Some(Rc::clone(&new_boundary)));
        task.blocked_preamble.set(
            new_boundary
                .preamble
                .as_ref()
                .map(|preamble| preamble.content),
        );
        task.hoistable_state.set(Some(new_boundary.content_state));
        task.blocked_segment
            .replace(Some(Rc::clone(&content_root_segment)));
        task.key_path.replace(Some(Rc::clone(key_path)));
        task.format_context.set(get_suspense_content_format_context(
            request.resumable_state,
            prev_context,
        ));
        task.row.replace(None);
        let result = catch_unwind(AssertUnwindSafe(|| {
            // We use the safe form because we don't handle suspending here. Only error handling.
            render_node(request, task, content, -1.0);
            push_segment_finale(
                &content_root_segment.chunks,
                request.render_state,
                content_root_segment.last_pushed_text.get(),
                content_root_segment.text_embedded.get(),
            );
            content_root_segment.status.set(COMPLETED);
            finished_segment(request, Some(&new_boundary), &content_root_segment);
            queue_completed_segment(&new_boundary, &content_root_segment);
            if new_boundary.pending_tasks.get() == 0.0 && new_boundary.status.get() == PENDING {
                // This must have been the last segment we were waiting on. This boundary is now complete.
                new_boundary.status.set(COMPLETED);
                // Therefore we won't need the fallback. We early return so that we don't have to create
                // the fallback. However, if this boundary ended up big enough to be eligible for outlining
                // we can't do that because we might still need the fallback if we outline it.
                if !is_eligible_for_outlining(request, &new_boundary) {
                    if let Some(prev_row) = &prev_row {
                        // If we have synchronously completed the boundary and it's not eligible for outlining
                        // then we don't have to wait for it to be flushed before we unblock future rows.
                        // This lets us inline small rows in order.
                        prev_row
                            .pending_tasks
                            .set(prev_row.pending_tasks.get() - 1.0);
                        if prev_row.pending_tasks.get() == 0.0 {
                            finish_suspense_list_row(request, prev_row);
                        }
                    }
                    if request.pending_root_tasks.get() == 0.0
                        && task.blocked_preamble.get().is_some()
                    {
                        // The root is complete and this boundary may contribute part of the preamble.
                        // We eagerly attempt to prepare the preamble here because we expect most requests
                        // to have few boundaries which contribute preambles and it allow us to do this
                        // preparation work during the work phase rather than the when flushing.
                        prepare_preamble(request);
                    }
                    return true;
                }
            } else {
                let boundary_row = &prev_row;
                if let Some(boundary_row) = boundary_row
                    && boundary_row.together.get()
                {
                    try_to_resolve_together_row(request, boundary_row);
                }
            }
            false
        }));
        if let Err(thrown_value) = &result {
            new_boundary.status.set(CLIENT_RENDERED);
            let error = if request.aborted.get() {
                content_root_segment.status.set(ABORTED);
                request.fatal_error.get()
            } else {
                content_root_segment.status.set(ERRORED);
                thrown_value_of(&**thrown_value)
            };

            let thrown_info = get_thrown_info(task.component_stack.borrow().clone());
            let error_digest = log_recoverable_error(
                request,
                error,
                thrown_info,
                if __DEV__ { task.debug_task.get() } else { None },
            );
            encode_error_for_boundary(&new_boundary, error_digest, error, thrown_info, false);

            untrack_boundary(request, &new_boundary);

            // We don't need to decrement any task numbers because we didn't spawn any new task.
            // We don't need to schedule any task because we know the parent has written yet.
            // We do need to fallthrough to create the fallback though.
        }
        task.blocked_boundary.replace(parent_boundary.clone());
        task.blocked_preamble.set(parent_preamble);
        task.hoistable_state.set(parent_hoistable_state);
        task.blocked_segment.replace(Some(parent_segment));
        task.key_path.replace(prev_key_path);
        task.format_context.set(prev_context);
        task.row.replace(prev_row);
        if let Ok(true) = result {
            return;
        }

        let fallback_key_path = fallback_key_path_of(key_path);
        // We create suspended task for the fallback because we don't want to actually work
        // on it yet in case we finish the main content, so we queue for later.
        let suspended_fallback_task = create_render_task(
            request,
            None,
            fallback,
            -1.0,
            parent_boundary,
            boundary_segment,
            new_boundary
                .preamble
                .as_ref()
                .map(|preamble| preamble.fallback),
            Some(new_boundary.fallback_state),
            fallback_abort_set,
            Some(fallback_key_path),
            get_suspense_fallback_format_context(
                request.resumable_state,
                task.format_context.get(),
            ),
            task.context.borrow().clone(),
            Rc::clone(&task.tree_context.borrow()),
            task.row.borrow().clone(),
            replace_suspense_component_stack_with_suspense_fallback_stack(
                task.component_stack.borrow().as_ref(),
            ),
            if !DISABLE_LEGACY_CONTEXT {
                task.legacy_context.get()
            } else {
                EMPTY_CONTEXT_OBJECT.with(|empty_context_object| *empty_context_object)
            },
            if __DEV__ { task.debug_task.get() } else { None },
        );
        push_component_stack(&suspended_fallback_task);
        // TODO: This should be queued at a separate lower priority queue so that we only work
        // on preparing fallbacks if we don't have any more main content to task on.
        request
            .pinged_tasks
            .borrow_mut()
            .push(suspended_fallback_task);
    }
}

fn replay_suspense_boundary(
    request: &Rc<Request>,
    task: &Rc<Task>,
    key_path: &Rc<KeyNode>,
    props: &SuspenseProps,
    id: f64,
    child_nodes: Rc<RefCell<Vec<ReplayNode>>>,
    child_slots: ResumeSlots,
    fallback_nodes: Rc<RefCell<Vec<ReplayNode>>>,
    fallback_slots: ResumeSlots,
) {
    let prev_key_path = task.key_path.borrow().clone();
    let prev_context = task.format_context.get();
    let prev_row = task.row.borrow().clone();
    let previous_replay_set = task.replay.borrow().clone();

    let parent_boundary = task.blocked_boundary.borrow().clone();
    let parent_hoistable_state = task.hoistable_state.get();

    let content = props.children;
    let fallback = props.fallback;
    let defer = ENABLE_CPU_SUSPENSE.with(|&enabled| enabled) && props.defer == Some(true);

    let fallback_abort_set: AbortSet = Rc::new(RefCell::new(HashSet::new()));
    let resumed_boundary = create_suspense_boundary(
        request,
        task.row.borrow().clone(),
        Rc::clone(&fallback_abort_set),
        if can_have_preamble(task.format_context.get()) {
            Some(Rc::new(create_preamble()))
        } else {
            None
        },
        defer,
    );
    resumed_boundary.parent_flushed.set(true);
    // We restore the same id of this boundary as was used during prerender.
    resumed_boundary.root_segment_id.set(id);

    let fallback_key_path = fallback_key_path_of(key_path);
    if let Some(tracked_postpones) = request.tracked_postpones.borrow().as_ref() {
        let fallback_replay_node = ReplayNode::Element(Rc::new(ReplayElement(
            fallback_key_path.1.clone(),
            fallback_key_path.2.clone(),
            Rc::new(RefCell::new(vec![])),
            RefCell::new(ResumeSlots::None),
        )));
        tracked_postpones
            .working_map
            .borrow_mut()
            .insert(Rc::clone(&fallback_key_path), fallback_replay_node.clone());
        resumed_boundary.tracked.replace(Some(Rc::new(Tracked {
            content_key_path: RefCell::new(Some(Rc::clone(key_path))),
            fallback_node: RefCell::new(Some(fallback_replay_node)),
        })));
    }

    // We can reuse the current context and task to render the content immediately without
    // context switching. We just need to temporarily switch which boundary and replay node
    // we're writing to. If something suspends, it'll spawn new suspended task with that context.
    task.blocked_boundary
        .replace(Some(Rc::clone(&resumed_boundary)));
    task.hoistable_state
        .set(Some(resumed_boundary.content_state));
    task.key_path.replace(Some(Rc::clone(key_path)));
    task.format_context.set(get_suspense_content_format_context(
        request.resumable_state,
        prev_context,
    ));
    task.row.replace(None);
    let replay = Rc::new(ReplaySet {
        nodes: child_nodes,
        slots: RefCell::new(child_slots),
        pending_tasks: Cell::new(1.0),
    });
    task.replay.replace(Some(Rc::clone(&replay)));

    let result = catch_unwind(AssertUnwindSafe(|| {
        // We use the safe form because we don't handle suspending here. Only error handling.
        render_node(request, task, content, -1.0);

        if replay.pending_tasks.get() == 1.0 && !replay.nodes.borrow().is_empty() {
            panic!(concat!(
                "Couldn't find all resumable slots by key/index during replaying. ",
                "The tree doesn't match so React will fallback to client rendering."
            ));
        }
        replay.pending_tasks.set(replay.pending_tasks.get() - 1.0);
        if resumed_boundary.pending_tasks.get() == 0.0 && resumed_boundary.status.get() == PENDING {
            // This must have been the last segment we were waiting on. This boundary is now complete.
            // Therefore we won't need the fallback. We early return so that we don't have to create
            // the fallback.
            resumed_boundary.status.set(COMPLETED);
            request
                .completed_boundaries
                .borrow_mut()
                .push(Rc::clone(&resumed_boundary));
            // We restore the parent componentStack. Semantically this is the same as
            // popComponentStack(task) but we do this instead because it should be slightly
            // faster
            return true;
        }
        false
    }));
    if let Err(thrown_value) = &result {
        resumed_boundary.status.set(CLIENT_RENDERED);
        let error = if request.aborted.get() {
            request.fatal_error.get()
        } else {
            thrown_value_of(&**thrown_value)
        };
        let thrown_info = get_thrown_info(task.component_stack.borrow().clone());
        let error_digest = log_recoverable_error(
            request,
            error,
            thrown_info,
            if __DEV__ { task.debug_task.get() } else { None },
        );
        encode_error_for_boundary(&resumed_boundary, error_digest, error, thrown_info, false);

        replay.pending_tasks.set(replay.pending_tasks.get() - 1.0);

        // The parent already flushed in the prerender so we need to schedule this to be emitted.
        request
            .client_rendered_boundaries
            .borrow_mut()
            .push(Rc::clone(&resumed_boundary));

        // We don't need to decrement any task numbers because we didn't spawn any new task.
        // We don't need to schedule any task because we know the parent has written yet.
        // We do need to fallthrough to create the fallback though.
    }
    task.blocked_boundary.replace(parent_boundary.clone());
    task.hoistable_state.set(parent_hoistable_state);
    task.replay.replace(previous_replay_set);
    task.key_path.replace(prev_key_path);
    task.format_context.set(prev_context);
    task.row.replace(prev_row);
    if let Ok(true) = result {
        return;
    }

    // We create suspended task for the fallback because we don't want to actually work
    // on it yet in case we finish the main content, so we queue for later.
    let fallback_replay = Rc::new(ReplaySet {
        nodes: fallback_nodes,
        slots: RefCell::new(fallback_slots),
        pending_tasks: Cell::new(0.0),
    });
    let suspended_fallback_task = create_replay_task(
        request,
        None,
        fallback_replay,
        fallback,
        -1.0,
        parent_boundary,
        Some(resumed_boundary.fallback_state),
        fallback_abort_set,
        Some(fallback_key_path),
        get_suspense_fallback_format_context(request.resumable_state, task.format_context.get()),
        task.context.borrow().clone(),
        Rc::clone(&task.tree_context.borrow()),
        task.row.borrow().clone(),
        replace_suspense_component_stack_with_suspense_fallback_stack(
            task.component_stack.borrow().as_ref(),
        ),
        if !DISABLE_LEGACY_CONTEXT {
            task.legacy_context.get()
        } else {
            EMPTY_CONTEXT_OBJECT.with(|empty_context_object| *empty_context_object)
        },
        if __DEV__ { task.debug_task.get() } else { None },
    );

    push_component_stack(&suspended_fallback_task);
    // TODO: This should be queued at a separate lower priority queue so that we only work
    // on preparing fallbacks if we don't have any more main content to task on.
    request
        .pinged_tasks
        .borrow_mut()
        .push(suspended_fallback_task);
}

fn finish_suspense_list_row(request: &Rc<Request>, row: &SuspenseListRow) {
    // This row finished. Now we have to unblock all the next rows that were blocked on this.
    unblock_suspense_list_row(request, row.next.borrow().clone(), Some(row.hoistables));
}

fn unblock_suspense_list_row(
    request: &Rc<Request>,
    mut unblocked_row: Option<Rc<SuspenseListRow>>,
    mut inherited_hoistables: Option<&'static HoistableState>,
) {
    // We do this in a loop to avoid stack overflow for very long lists that get unblocked.
    while let Some(row) = unblocked_row {
        if let Some(inherited_hoistables) = inherited_hoistables {
            // Hoist any hoistables from the previous row into the next row so that it can be
            // later transferred to all the rows.
            hoist_hoistables(row.hoistables, inherited_hoistables);
            // Mark the row itself for any newly discovered Suspense boundaries to inherit.
            // This is different from hoistables because that also includes hoistables from
            // all the boundaries below this row and not just previous rows.
            row.inherited_hoistables.set(Some(inherited_hoistables));
        }
        // Unblocking the boundaries will decrement the count of this row but we keep it above
        // zero so they never finish this row recursively.
        let unblocked_boundaries = row.boundaries.take();
        if let Some(unblocked_boundaries) = unblocked_boundaries {
            for unblocked_boundary in &unblocked_boundaries {
                if let Some(inherited_hoistables) = inherited_hoistables {
                    hoist_hoistables(unblocked_boundary.content_state, inherited_hoistables);
                }
                finished_task(request, Some(unblocked_boundary), None, None);
            }
        }
        // Instead we decrement at the end to keep it all in this loop.
        row.pending_tasks.set(row.pending_tasks.get() - 1.0);
        if row.pending_tasks.get() > 0.0 {
            // Still blocked.
            break;
        }
        inherited_hoistables = Some(row.hoistables);
        unblocked_row = row.next.borrow().clone();
    }
}

fn track_postponed_suspense_list_row(
    request: &Rc<Request>,
    tracked_postpones: &PostponedHoles,
    postponed_row: Option<&Rc<SuspenseListRow>>,
) {
    // TODO: Because we unconditionally call this, it will be called by finishedTask
    // and so ends up recursive which can lead to stack overflow for very long lists.
    if let Some(postponed_row) = postponed_row {
        let postponed_boundaries = postponed_row.boundaries.take();
        if let Some(postponed_boundaries) = postponed_boundaries {
            for postponed_boundary in &postponed_boundaries {
                track_postponed_boundary(request, tracked_postpones, postponed_boundary);
                finished_task(request, Some(postponed_boundary), None, None);
            }
        }
    }
}

fn try_to_resolve_together_row(request: &Rc<Request>, together_row: &Rc<SuspenseListRow>) {
    // If we have a "together" row and all the pendingTasks are really the boundaries themselves,
    // and we won't outline any of them then we can unblock this row early so that we can inline
    // all the boundaries at once.
    let all_complete_and_inlinable = {
        let boundaries = together_row.boundaries.borrow();
        let Some(boundaries) = boundaries.as_ref() else {
            return;
        };
        if together_row.pending_tasks.get() != boundaries.len() as f64 {
            return;
        }
        boundaries.iter().all(|row_boundary| {
            !(row_boundary.pending_tasks.get() != 1.0
                || row_boundary.parent_flushed.get()
                || is_eligible_for_outlining(request, row_boundary))
        })
    };
    if all_complete_and_inlinable {
        unblock_suspense_list_row(
            request,
            Some(Rc::clone(together_row)),
            Some(together_row.hoistables),
        );
    }
}

fn create_suspense_list_row(previous_row: Option<&Rc<SuspenseListRow>>) -> Rc<SuspenseListRow> {
    let new_row = Rc::new(SuspenseListRow {
        pending_tasks: Cell::new(1.0), // At first the row is blocked on attempting rendering itself.
        boundaries: RefCell::new(None),
        hoistables: create_hoistable_state(),
        inherited_hoistables: Cell::new(None),
        together: Cell::new(false),
        next: RefCell::new(None),
    });
    if let Some(previous_row) = previous_row
        && previous_row.pending_tasks.get() > 0.0
    {
        // If the previous row is not done yet, we add ourselves to be blocked on it.
        // When it finishes, we'll decrement our pending tasks.
        new_row.pending_tasks.set(new_row.pending_tasks.get() + 1.0);
        new_row.boundaries.replace(Some(vec![]));
        previous_row.next.replace(Some(Rc::clone(&new_row)));
    }
    new_row
}

// DRAFT: the sections below are not ported yet.

fn render_node(
    _request: &Rc<Request>,
    _task: &Rc<Task>,
    _node: Option<&'static ReactNodeList>,
    _child_index: f64,
) {
    todo!()
}

fn finished_segment(
    _request: &Rc<Request>,
    _boundary: Option<&Rc<SuspenseBoundary>>,
    _segment: &Rc<Segment>,
) {
    todo!()
}

fn queue_completed_segment(_boundary: &Rc<SuspenseBoundary>, _segment: &Rc<Segment>) {
    todo!()
}

fn finished_task(
    _request: &Rc<Request>,
    _boundary: Option<&Rc<SuspenseBoundary>>,
    _row: Option<&Rc<SuspenseListRow>>,
    _segment: Option<&Rc<Segment>>,
) {
    todo!()
}

fn untrack_boundary(_request: &Request, _boundary: &Rc<SuspenseBoundary>) {
    todo!()
}

fn prepare_preamble(_request: &Rc<Request>) {
    todo!()
}

fn track_postponed_boundary(
    _request: &Rc<Request>,
    _tracked_postpones: &PostponedHoles,
    _boundary: &Rc<SuspenseBoundary>,
) {
    todo!()
}
fn retry_node(_request: &Rc<Request>, _task: &Rc<Task>) {
    todo!()
}

pub fn perform_work(_request: &Rc<Request>) {
    todo!()
}

fn finish_aborted_task(_task: &Rc<Task>, _request: &Rc<Request>, _error: Option<&'static Unknown>) {
    todo!()
}

fn finish_aborted_task_dev(
    _task: &Rc<Task>,
    _request: &Rc<Request>,
    _error: Option<&'static Unknown>,
) {
    todo!()
}

fn end_render_lifetime(_request: &Request) {
    todo!()
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_form_state(request: &Request) -> Option<&'static FormState> {
    request.form_state
}

pub struct PostponedState {
    next_segment_id: Cell<f64>,
    root_format_context: &'static FormatContext,
    progressive_chunk_size: f64,
    resumable_state: &'static ResumableState,
    replay_nodes: Rc<RefCell<Vec<ReplayNode>>>,
    replay_slots: RefCell<ResumeSlots>,
}
