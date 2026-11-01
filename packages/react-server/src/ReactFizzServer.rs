// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::rc::Rc;

use js::{
    Dict, Iterator, JsError, JsObject, Kind, PropertyDescriptor, SymbolFor, Unknown, WeakSet,
};
use node::async_hooks::AsyncLocalStorage;
use react_isomorphic::react_lazy::LazyComponent;
use shared::assign::assign;
use shared::environment::__DEV__;
use shared::get_component_name_from_type::get_component_name_from_type;
use shared::is_array::is_array;
use shared::react_feature_flags::{
    DISABLE_LEGACY_CONTEXT, DISABLE_LEGACY_CONTEXT_FOR_FUNCTION_COMPONENTS,
    ENABLE_ASYNC_ITERABLE_CHILDREN, ENABLE_CPU_SUSPENSE, ENABLE_FIZZ_BLOCKING_RENDER,
    ENABLE_SCOPE_API, ENABLE_VIEW_TRANSITION, ENABLE_VIEW_TRANSITION_PARENT_ENTER_EXIT,
};
use shared::react_owner_stack_reset::reset_owner_stack_limit;
use shared::react_recoverable::REACT_RECOVERABLE_DIGEST;
use shared::react_serialization_errors::describe_object_for_error_message;
use shared::react_shared_internals::REACT_SHARED_INTERNALS;
use shared::react_symbols::{
    ASYNC_ITERATOR, REACT_ACTIVITY_TYPE, REACT_CONSUMER_TYPE, REACT_CONTEXT_TYPE,
    REACT_ELEMENT_TYPE, REACT_FORWARD_REF_TYPE, REACT_FRAGMENT_TYPE, REACT_LAZY_TYPE,
    REACT_LEGACY_HIDDEN_TYPE, REACT_MEMO_TYPE, REACT_PORTAL_TYPE, REACT_PROFILER_TYPE,
    REACT_RECOVERABLE_TYPE, REACT_SCOPE_TYPE, REACT_STRICT_MODE_TYPE, REACT_SUSPENSE_LIST_TYPE,
    REACT_SUSPENSE_TYPE, REACT_VIEW_TRANSITION_TYPE, get_iterator_fn,
};
use shared::react_types::{
    ReactFormState, ReactNodeList, SuspenseListProps, SuspenseListRevealOrder, SuspenseProps,
    Thenable, ViewTransitionProps,
};
use webapi::{
    AbortController, AbortSignal, AddEventListenerOptions, ConsoleTask, Event, EventTargetExt,
    console,
};

use crate::react_fizz_async_dispatcher::DEFAULT_ASYNC_DISPATCHER;
use crate::react_fizz_call_user_space::{
    CALL_COMPONENT_IN_DEV, CALL_LAZY_INIT_IN_DEV, CALL_RENDER_IN_DEV, ClassInstance, Component,
};
use crate::react_fizz_class_component::{construct_class_instance, mount_class_instance};
use crate::react_fizz_component_stack::{
    ComponentStackNode, get_owner_stack_by_component_stack_node_in_dev,
    get_stack_by_component_stack_node,
};
use crate::react_fizz_current_task::{CURRENT_TASK_IN_DEV, set_current_task_in_dev};
use crate::react_fizz_hooks::{
    CURRENT_RESUMABLE_STATE, HOOKS_DISPATCHER, check_did_render_id_hook,
    clone_recoverable_error_as_fatal, create_recoverable_error, finish_hooks,
    get_action_state_count, get_action_state_matching_index, get_thenable_state_after_suspending,
    is_recoverable_error, prepare_to_use_hooks, prepare_to_use_thenable_state,
    read_previous_thenable_from_state, reset_hooks_state, set_current_resumable_state,
    unwrap_thenable,
};
use crate::react_fizz_legacy_context::{
    Context as LegacyContext, EMPTY_CONTEXT_OBJECT, get_masked_context, process_child_context,
};
use crate::react_fizz_new_context::{
    Context, ContextSnapshot, ROOT_CONTEXT_SNAPSHOT, get_active_context, pop_provider,
    push_provider, read_context, switch_context,
};
use crate::react_fizz_thenable::{
    SUSPENSE_EXCEPTION, ThenableState, ensure_suspendable_thenable_state_dev,
    get_suspended_call_site_debug_task_dev, get_suspended_call_site_stack_dev,
    get_suspended_thenable, set_capture_suspended_call_site_dev,
};
use crate::react_fizz_tree_context::{EMPTY_TREE_CONTEXT, TreeContext, push_tree_context};
use crate::react_fizz_view_transition_component::{
    get_view_transition_class_name, get_view_transition_name,
};

// ReactServerStreamConfig's types, which its fork for each host defines: a
// destination, and the chunks written to it.

/// Where a request writes, the host's stream.
pub type Destination = Unknown;
/// What's written: text, or its encoded bytes.
pub struct Chunk(std::marker::PhantomData<JsObject>);

unsafe impl js::Defined for Chunk {}
/// A chunk encoded once, as a module loads.
pub type PrecomputedChunk = Chunk;

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

/// A chunk's length in bytes, `null` where the host's config can't tell.
pub type ByteLengthOfChunk = dyn Fn(&Chunk) -> f64;

// ReactFizzConfig's functions and values, of the renderer's config.

/// A segment's chunks, which the config pushes to.
pub type Chunks = RefCell<Vec<&'static Chunk>>;

unsafe extern "Rust" {
    #[link_name = "./src/ReactFizzConfig.js#isWorkLoopExternallyDriven"]
    safe static IS_WORK_LOOP_EXTERNALLY_DRIVEN: bool;
    #[link_name = "./src/ReactFizzConfig.js#supportsRequestStorage"]
    safe static SUPPORTS_REQUEST_STORAGE: bool;
    #[link_name = "./src/ReactFizzConfig.js#requestStorage"]
    safe static REQUEST_STORAGE: &'static AsyncLocalStorage<Rc<Request>>;
    #[link_name = "./src/ReactServerStreamConfig.js#byteLengthOfChunk"]
    safe static BYTE_LENGTH_OF_CHUNK: Option<&'static ByteLengthOfChunk>;
}

/// Gives JS's `AsyncLocalStorage::run` the request a caller holds.
// GAP rd497: node's `AsyncLocalStorage<T>::run` takes a `&'static T`; a
// store an `Rc` holds is the object itself, but is that only by a cast.
fn request_store(request: &Rc<Request>) -> &'static Rc<Request> {
    unsafe { js::cast(Some(js::unknown(&**request))) }
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
    Indices(&'static Dict<f64>),
}

/// `[name, key, children, slots]`.
#[derive(Clone)]
pub struct ReplayElement(
    #[cfg_attr(rust_js, rust_js::nullable)] pub Option<String>, /* name */
    pub NodeKey,                                                /* key */
    pub ReplayNodes,                                            /* keyed children */
    pub ResumeSlots,                                            /* resumable slots */
);

/// `[name, key, content, contentSlots, fallback, rootSegmentID]`.
#[derive(Clone)]
pub struct ReplaySuspenseBoundary(
    #[cfg_attr(rust_js, rust_js::nullable)] pub Option<String>, /* name */
    pub NodeKey,                                                /* key */
    pub ReplayNodes,                                            /* content keyed children */
    pub ResumeSlots,                                            /* content resumable slots */
    #[cfg_attr(rust_js, rust_js::nullable)] pub Option<Rc<RefCell<ReplayNode>>>, /* fallback content */
    pub f64,                                                                     /* rootSegmentID */
);

/// A replay node, which trackPostponedBoundary grows in place to a
/// boundary's and untrackBoundary shrinks back: told apart by its length
/// (rust-js's ADR 1046).
#[cfg_attr(rust_js, rust_js::untagged)]
#[derive(Clone)]
pub enum ReplayNode {
    Element(ReplayElement),
    Boundary(ReplaySuspenseBoundary),
}

/// A list of replay nodes, which a replay set shares with its node.
pub type ReplayNodes = Rc<RefCell<Vec<Rc<RefCell<ReplayNode>>>>>;

struct PostponedHoles {
    working_map: RefCell<HashMap<Rc<KeyNode>, Rc<RefCell<ReplayNode>>>>,
    root_nodes: ReplayNodes,
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
    fallback_node: RefCell<Option<Rc<RefCell<ReplayNode>>>>, // used to track the fallback for replay nodes
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

struct ReplaySet {
    nodes: ReplayNodes,          // the possible paths to follow down the replaying
    slots: RefCell<ResumeSlots>, // slots to resume
    pending_tasks: Cell<f64>,    // tracks the number of tasks currently tracking this set of nodes
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
    preamble_children: Rc<RefCell<Vec<Rc<Segment>>>>,
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

// Safety: a request is a JS object of its fields.
unsafe impl js::Defined for Request {}

pub struct Request {
    #[cfg_attr(rust_js, rust_js::nullable)]
    destination: Cell<Option<&'static Destination>>,
    flush_scheduled: Cell<bool>,
    resumable_state: &'static ResumableState,
    render_state: &'static RenderState,
    root_format_context: &'static FormatContext,
    progressive_chunk_size: f64,
    status: Cell<i32>,
    // What was thrown, `undefined` too: `null` until then.
    fatal_error: Cell<Option<&'static Unknown>>,
    aborted: Cell<bool>,
    next_segment_id: Cell<f64>,
    all_pending_tasks: Cell<f64>, // when it reaches zero, we can close the connection.
    pending_root_tasks: Cell<f64>, // when this reaches zero, we've finished at least the root boundary.
    #[cfg_attr(rust_js, rust_js::nullable)]
    completed_root_segment: RefCell<Option<Rc<Segment>>>, // Completed but not yet flushed root segments.
    #[cfg_attr(rust_js, rust_js::nullable)]
    completed_preamble_segments: RefCell<Option<Rc<PreambleSegments>>>, // contains the ready-to-flush segments that make up the preamble
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

/// The segments of a preamble: each segment's preamble children, as it
/// holds them.
type PreambleSegments = Vec<Rc<RefCell<Vec<Rc<Segment>>>>>;

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
        fatal_error: Cell::new(js::null()),
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
        let store = REQUEST_STORAGE.get_store().cloned();
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

// The user's decision (rd495, 2026-11-01): a task's ping is made where it's
// given to `then`, its two closures holding the request and the task as
// upstream's `ping.resolve` and `ping.reject` arrows do; the task object has
// no `ping` property.
fn ping_of(
    request: &Rc<Request>,
    task: &Rc<Task>,
) -> (
    impl FnOnce(Option<&'static Unknown>) + 'static,
    impl FnOnce(Option<&'static Unknown>) + 'static,
) {
    let (resolve_request, resolve_task) = (Rc::clone(request), Rc::clone(task));
    let (reject_request, reject_task) = (Rc::clone(request), Rc::clone(task));
    (
        move |_| ping_task(&resolve_request, resolve_task),
        move |error| ping_rejected_task(&reject_request, reject_task, error),
    )
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
    let task = Rc::new(Task {
        replay: RefCell::new(None),
        node: Cell::new(node),
        child_index: Cell::new(child_index),
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
    let task = Rc::new(Task {
        replay: RefCell::new(Some(replay)),
        node: Cell::new(node),
        child_index: Cell::new(child_index),
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
        preamble_children: Rc::new(RefCell::new(vec![])),
        parent_format_context,
        boundary: RefCell::new(boundary),
        last_pushed_text: Cell::new(last_pushed_text),
        text_embedded: Cell::new(text_embedded),
    })
}

#[cfg_attr(rust_js, rust_js::name = "getCurrentStackInDEV")]
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
                            r#type: async_info,
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
    REACT_SHARED_INTERNALS.set_get_current_stack(Some(&get_current_stack_in_dev));

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

    REACT_SHARED_INTERNALS.set_get_current_stack(prev_get_current_stack_impl);
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
                owner: owner_stack.map(js::unknown_of),
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
                    r#type: component_info,
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
                        parent, r#type, owner, stack,
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
                    if js::type_of(js::get(maybe_usable, "then")) == "function" {
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
    r#type: Option<&'static Unknown>,
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
        Some(js::unknown("Suspense Fallback")),
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
            Some(js::unknown_of(stack))
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
            let fallback_replay_node = Rc::new(RefCell::new(ReplayNode::Element(ReplayElement(
                fallback_key_path.1.clone(),
                fallback_key_path.2.clone(),
                Rc::new(RefCell::new(vec![])),
                ResumeSlots::None,
            ))));
            tracked_postpones.working_map.borrow_mut().insert(
                Rc::clone(&fallback_key_path),
                Rc::clone(&fallback_replay_node),
            );
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
    child_nodes: ReplayNodes,
    child_slots: ResumeSlots,
    fallback_nodes: ReplayNodes,
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
        let fallback_replay_node = Rc::new(RefCell::new(ReplayNode::Element(ReplayElement(
            fallback_key_path.1.clone(),
            fallback_key_path.2.clone(),
            Rc::new(RefCell::new(vec![])),
            ResumeSlots::None,
        ))));
        tracked_postpones.working_map.borrow_mut().insert(
            Rc::clone(&fallback_key_path),
            Rc::clone(&fallback_replay_node),
        );
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

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#getChildFormatContext"
)]
fn get_child_format_context(
    parent_context: &FormatContext,
    r#type: &str,
    props: &Unknown,
) -> &'static FormatContext {
    let _ = (parent_context, r#type, props);
    unreachable!()
}

fn is_backwards(reveal_order: Option<SuspenseListRevealOrder>) -> bool {
    matches!(
        reveal_order,
        Some(SuspenseListRevealOrder::Backwards | SuspenseListRevealOrder::UnstableLegacyBackwards)
    )
}

fn render_suspense_list_rows(
    request: &Rc<Request>,
    task: &Rc<Task>,
    key_path: &Rc<KeyNode>,
    rows: &[Option<&'static ReactNodeList>],
    reveal_order: Option<SuspenseListRevealOrder>,
) {
    // This is a fork of renderChildrenArray that's aware of tracking rows.
    let prev_key_path = task.key_path.replace(Some(Rc::clone(key_path)));
    let prev_tree_context = Rc::clone(&task.tree_context.borrow());
    let prev_row = task.row.borrow().clone();
    let previous_component_stack = task.component_stack.borrow().clone();
    let mut previous_debug_task = None;
    if __DEV__ {
        previous_debug_task = task.debug_task.get();
        // We read debugInfo from task.node.props.children instead of rows because it
        // might have been an unwrapped iterable so we read from the original node.
        push_server_component_stack(
            task,
            js::get(
                js::get(js::get(task.node.get(), "props"), "children"),
                "_debugInfo",
            ),
        );
    }

    let total_children = rows.len();
    let mut previous_suspense_list_row: Option<Rc<SuspenseListRow>> = None;
    let replay = task.replay.borrow().clone();
    if let Some(replay) = replay {
        // Replay
        // First we need to check if we have any resume slots at this level.
        let resume_slots = replay.slots.borrow().clone();
        if let ResumeSlots::Indices(resume_slots) = resume_slots {
            for n in 0..total_children {
                // Since we are going to resume into a slot whose order was already
                // determined by the prerender, we can safely resume it even in reverse
                // render order.
                let i = if !is_backwards(reveal_order) {
                    n
                } else {
                    total_children - 1 - n
                };
                let node = rows[i];
                let row = create_suspense_list_row(previous_suspense_list_row.as_ref());
                task.row.replace(Some(Rc::clone(&row)));
                previous_suspense_list_row = Some(Rc::clone(&row));
                task.tree_context.replace(push_tree_context(
                    &prev_tree_context,
                    total_children as i32,
                    i as i32,
                ));
                let resume_segment_id = js::dict::get(resume_slots, &i.to_string()).copied();
                // TODO: If this errors we should still continue with the next sibling.
                if let Some(resume_segment_id) = resume_segment_id {
                    resume_node(request, task, resume_segment_id, node, i as f64);
                    // We finished rendering this node, so now we can consume this
                    // slot. This must happen after in case we rerender this task.
                    js::delete(resume_slots, &i.to_string());
                } else {
                    render_node(request, task, node, i as f64);
                }
                row.pending_tasks.set(row.pending_tasks.get() - 1.0);
                if row.pending_tasks.get() == 0.0 {
                    finish_suspense_list_row(request, &row);
                }
            }
        } else {
            for n in 0..total_children {
                // Since we are going to resume into a slot whose order was already
                // determined by the prerender, we can safely resume it even in reverse
                // render order.
                let i = if !is_backwards(reveal_order) {
                    n
                } else {
                    total_children - 1 - n
                };
                let node = rows[i];
                if __DEV__ {
                    warn_for_missing_key(request, task, node);
                }
                let row = create_suspense_list_row(previous_suspense_list_row.as_ref());
                task.row.replace(Some(Rc::clone(&row)));
                previous_suspense_list_row = Some(Rc::clone(&row));
                task.tree_context.replace(push_tree_context(
                    &prev_tree_context,
                    total_children as i32,
                    i as i32,
                ));
                render_node(request, task, node, i as f64);
                row.pending_tasks.set(row.pending_tasks.get() - 1.0);
                if row.pending_tasks.get() == 0.0 {
                    finish_suspense_list_row(request, &row);
                }
            }
        }
    } else if !is_backwards(reveal_order) {
        // Forwards direction
        for (i, &node) in rows.iter().enumerate() {
            if __DEV__ {
                warn_for_missing_key(request, task, node);
            }
            let row = create_suspense_list_row(previous_suspense_list_row.as_ref());
            task.row.replace(Some(Rc::clone(&row)));
            previous_suspense_list_row = Some(Rc::clone(&row));
            task.tree_context.replace(push_tree_context(
                &prev_tree_context,
                total_children as i32,
                i as i32,
            ));
            render_node(request, task, node, i as f64);
            row.pending_tasks.set(row.pending_tasks.get() - 1.0);
            if row.pending_tasks.get() == 0.0 {
                finish_suspense_list_row(request, &row);
            }
        }
    } else {
        // For backwards direction we need to do things a bit differently.
        // We give each row its own segment so that we can render the content in
        // reverse order but still emit it in the right order when we flush.
        let parent_segment = task.blocked_segment.borrow().clone().unwrap();
        let child_index = parent_segment.children.borrow().len();
        let insertion_index = parent_segment.chunks.borrow().len() as f64;
        for n in 0..total_children {
            let i = if reveal_order == Some(SuspenseListRevealOrder::UnstableLegacyBackwards) {
                total_children - 1 - n
            } else {
                n
            };
            let node = rows[i];
            let row = create_suspense_list_row(previous_suspense_list_row.as_ref());
            task.row.replace(Some(Rc::clone(&row)));
            previous_suspense_list_row = Some(Rc::clone(&row));
            task.tree_context.replace(push_tree_context(
                &prev_tree_context,
                total_children as i32,
                i as i32,
            ));
            let new_segment = create_pending_segment(
                request,
                insertion_index,
                None,
                task.format_context.get(),
                // Assume we are text embedded at the trailing edges
                if i == 0 {
                    parent_segment.last_pushed_text.get()
                } else {
                    true
                },
                true,
            );
            // Insert in the beginning of the sequence, which will insert before any previous rows.
            parent_segment
                .children
                .borrow_mut()
                .insert(child_index, Rc::clone(&new_segment));
            task.blocked_segment.replace(Some(Rc::clone(&new_segment)));
            if __DEV__ {
                warn_for_missing_key(request, task, node);
            }
            let result = catch_unwind(AssertUnwindSafe(|| {
                render_node(request, task, node, i as f64);
                push_segment_finale(
                    &new_segment.chunks,
                    request.render_state,
                    new_segment.last_pushed_text.get(),
                    new_segment.text_embedded.get(),
                );
                new_segment.status.set(COMPLETED);
                finished_segment(
                    request,
                    task.blocked_boundary.borrow().as_ref(),
                    &new_segment,
                );
                row.pending_tasks.set(row.pending_tasks.get() - 1.0);
                if row.pending_tasks.get() == 0.0 {
                    finish_suspense_list_row(request, &row);
                }
            }));
            if let Err(thrown_value) = result {
                if request.aborted.get() {
                    new_segment.status.set(ABORTED);
                } else {
                    new_segment.status.set(ERRORED);
                }
                resume_unwind(thrown_value);
            }
        }
        task.blocked_segment
            .replace(Some(Rc::clone(&parent_segment)));
        // Reset lastPushedText for current Segment since the new Segments "consumed" it
        parent_segment.last_pushed_text.set(false);
    }

    if let Some(prev_row) = &prev_row
        && let Some(previous_suspense_list_row) = &previous_suspense_list_row
        && previous_suspense_list_row.pending_tasks.get() > 0.0
    {
        // If we are part of an outer SuspenseList and our last row is still pending, then that blocks
        // the parent row from completing. We can continue the chain.
        prev_row
            .pending_tasks
            .set(prev_row.pending_tasks.get() + 1.0);
        previous_suspense_list_row
            .next
            .replace(Some(Rc::clone(prev_row)));
    }

    // Because this context is always set right before rendering every child, we
    // only need to reset it to the previous value at the very end.
    task.tree_context.replace(prev_tree_context);
    task.row.replace(prev_row);
    task.key_path.replace(prev_key_path);
    if __DEV__ {
        task.component_stack.replace(previous_component_stack);
        task.debug_task.set(previous_debug_task);
    }
}

fn render_suspense_list(
    request: &Rc<Request>,
    task: &Rc<Task>,
    key_path: &Rc<KeyNode>,
    props: &SuspenseListProps,
) {
    let children = props.children;
    let reveal_order = props.reveal_order;
    // TODO: Support tail hidden/collapsed modes.
    // const tailMode: SuspenseListTailMode = props.tail;
    if reveal_order != Some(SuspenseListRevealOrder::Independent)
        && reveal_order != Some(SuspenseListRevealOrder::Together)
    {
        // For ordered reveal, we need to produce rows from the children.
        if is_array(children) {
            let children: &[Option<&'static ReactNodeList>] = unsafe { js::cast(children) };
            render_suspense_list_rows(request, task, key_path, children, reveal_order);
            return;
        }
        let iterator_fn = get_iterator_fn(children);
        if let Some(iterator_fn) = iterator_fn {
            let iterator: Option<&'static Iterator<Option<&'static ReactNodeList>>> = unsafe {
                js::cast(js::function::call(
                    js::cast::<&dyn Fn() -> Option<&'static Unknown>>(Some(iterator_fn)),
                    Some(&children),
                    (),
                ))
            };
            if request.aborted.get() {
                // eslint-disable-next-line no-throw-literal
                js::throw(js::null());
            }
            if let Some(iterator) = iterator {
                if __DEV__ {
                    validate_iterable(task, children, -1.0, iterator, iterator_fn);
                }
                // TODO: We currently use the same id algorithm as regular nodes
                // but we need a new algorithm for SuspenseList that doesn't require
                // a full set to be loaded up front to support Async Iterable.
                // When we have that, we shouldn't buffer anymore.
                let mut step = iterator.next();
                if request.aborted.get() {
                    // eslint-disable-next-line no-throw-literal
                    js::throw(js::null());
                }
                if step.done != Some(true) {
                    let mut rows = vec![];
                    loop {
                        rows.push(step.value);
                        step = iterator.next();
                        if step.done == Some(true) {
                            break;
                        }
                    }
                    if request.aborted.get() {
                        // eslint-disable-next-line no-throw-literal
                        js::throw(js::null());
                    }
                    // Upstream renders `children` here, not the `rows` it read,
                    // which the port keeps.
                    let children: &[Option<&'static ReactNodeList>] = unsafe { js::cast(children) };
                    render_suspense_list_rows(request, task, key_path, children, reveal_order);
                }
                return;
            }
        }
        if ENABLE_ASYNC_ITERABLE_CHILDREN.with(|&enabled| enabled)
            && js::type_of(ASYNC_ITERATOR.with(|&async_iterator| js::get(children, async_iterator)))
                == "function"
        {
            let iterator: Option<&'static AsyncIterator> = unsafe {
                js::cast(js::function::call(
                    js::cast::<&dyn Fn() -> Option<&'static Unknown>>(
                        ASYNC_ITERATOR.with(|&async_iterator| js::get(children, async_iterator)),
                    ),
                    Some(&children),
                    (),
                ))
            };
            if request.aborted.get() {
                // eslint-disable-next-line no-throw-literal
                js::throw(js::null());
            }
            if let Some(iterator) = iterator {
                if __DEV__ {
                    validate_async_iterable(task, children, -1.0, iterator);
                }
                // TODO: Update the task.children to be the iterator to avoid asking
                // for new iterators, but we currently warn for rendering these
                // so needs some refactoring to deal with the warning.

                // Restore the thenable state before resuming.
                let prev_thenable_state = task.thenable_state.take();
                prepare_to_use_thenable_state(prev_thenable_state);

                // We need to know how many total rows are in this set, so that we
                // can allocate enough id slots to acommodate them. So we must exhaust
                // the iterator before we start recursively rendering the rows.
                // TODO: This is not great but I think it's inherent to the id
                // generation algorithm.

                let mut rows = vec![];

                let mut done = false;

                // $FlowFixMe[invalid-compare]
                if js::strict_equals(Some(js::unknown(iterator)), children) {
                    // If it's an iterator we need to continue reading where we left
                    // off. We can do that by reading the first few rows from the previous
                    // thenable state.
                    // $FlowFixMe[underconstrained-implicit-instantiation]
                    let mut step = read_previous_thenable_from_state();
                    while !matches!(js::classify(step), Kind::Undefined) {
                        if js::truthy(js::get(step, "done")) {
                            done = true;
                            break;
                        }
                        rows.push(js::get(step, "value"));
                        step = read_previous_thenable_from_state();
                    }
                }

                if !done {
                    let mut step = unwrap_thenable(iterator.next());
                    while !js::truthy(js::get(step, "done")) {
                        rows.push(js::get(step, "value"));
                        step = unwrap_thenable(iterator.next());
                    }
                }
                if request.aborted.get() {
                    // eslint-disable-next-line no-throw-literal
                    js::throw(js::null());
                }
                render_suspense_list_rows(request, task, key_path, &rows, reveal_order);
                return;
            }
        }
        // This case will warn on the client. It's the same as independent revealOrder.
    }

    if reveal_order == Some(SuspenseListRevealOrder::Together) {
        let prev_key_path = task.key_path.borrow().clone();
        let prev_row = task.row.borrow().clone();
        let new_row = create_suspense_list_row(None);
        task.row.replace(Some(Rc::clone(&new_row)));
        // This will cause boundaries to block on this row, but there's nothing to
        // unblock them. We'll use the partial flushing pass to unblock them.
        new_row.boundaries.replace(Some(vec![]));
        new_row.together.set(true);
        task.key_path.replace(Some(Rc::clone(key_path)));
        render_node_destructive(request, task, children, -1.0);
        new_row.pending_tasks.set(new_row.pending_tasks.get() - 1.0);
        if new_row.pending_tasks.get() == 0.0 {
            finish_suspense_list_row(request, &new_row);
        }
        task.key_path.replace(prev_key_path);
        task.row.replace(prev_row.clone());
        if let Some(prev_row) = &prev_row
            && new_row.pending_tasks.get() > 0.0
        {
            // If we are part of an outer SuspenseList and our row is still pending, then that blocks
            // the parent row from completing. We can continue the chain.
            prev_row
                .pending_tasks
                .set(prev_row.pending_tasks.get() + 1.0);
            new_row.next.replace(Some(Rc::clone(prev_row)));
        }
        return;
    }
    // For other reveal order modes, we just render it as a fragment.
    let prev_key_path = task.key_path.replace(Some(Rc::clone(key_path)));
    render_node_destructive(request, task, children, -1.0);
    task.key_path.replace(prev_key_path);
}

/// An async iterator of a node list's rows: its `next()` gives a thenable
/// of each step.
pub struct AsyncIterator(std::marker::PhantomData<JsObject>);

unsafe impl js::Defined for AsyncIterator {}

impl AsyncIterator {
    #[cfg_attr(rust_js, rust_js::link_name = "next")]
    fn next(&self) -> &'static Thenable {
        unreachable!()
    }
}

fn render_preamble(
    request: &Rc<Request>,
    task: &Rc<Task>,
    blocked_segment: &Rc<Segment>,
    node: Option<&'static ReactNodeList>,
) {
    let preamble_segment =
        create_pending_segment(request, 0.0, None, task.format_context.get(), false, false);
    blocked_segment
        .preamble_children
        .borrow_mut()
        .push(Rc::clone(&preamble_segment));
    task.blocked_segment
        .replace(Some(Rc::clone(&preamble_segment)));
    let result = catch_unwind(AssertUnwindSafe(|| {
        render_node(request, task, node, -1.0);
        push_segment_finale(
            &preamble_segment.chunks,
            request.render_state,
            preamble_segment.last_pushed_text.get(),
            preamble_segment.text_embedded.get(),
        );
        preamble_segment.status.set(COMPLETED);
        finished_segment(
            request,
            task.blocked_boundary.borrow().as_ref(),
            &preamble_segment,
        );
    }));
    task.blocked_segment
        .replace(Some(Rc::clone(blocked_segment)));
    if let Err(thrown_value) = result {
        resume_unwind(thrown_value);
    }
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#pushStartInstance"
)]
fn push_start_instance(
    target: &Chunks,
    r#type: &str,
    props: &'static Unknown,
    resumable_state: &ResumableState,
    render_state: &RenderState,
    preamble_state: Option<&PreambleState>,
    hoistable_state: Option<&HoistableState>,
    format_context: &FormatContext,
    text_embedded: bool,
) -> Option<&'static ReactNodeList> {
    let _ = (
        target,
        r#type,
        props,
        resumable_state,
        render_state,
        preamble_state,
        hoistable_state,
        format_context,
        text_embedded,
    );
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#pushEndInstance"
)]
fn push_end_instance(
    target: &Chunks,
    r#type: &str,
    props: &Unknown,
    resumable_state: &ResumableState,
    format_context: &FormatContext,
) {
    let _ = (target, r#type, props, resumable_state, format_context);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#isPreambleContext"
)]
fn is_preamble_context(format_context: &FormatContext) -> bool {
    let _ = format_context;
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#pushFormStateMarkerIsMatching"
)]
fn push_form_state_marker_is_matching(target: &Chunks) {
    let _ = target;
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#pushFormStateMarkerIsNotMatching"
)]
fn push_form_state_marker_is_not_matching(target: &Chunks) {
    let _ = target;
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#pushStartActivityBoundary"
)]
fn push_start_activity_boundary(target: &Chunks, render_state: &RenderState) {
    let _ = (target, render_state);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#pushEndActivityBoundary"
)]
fn push_end_activity_boundary(target: &Chunks, render_state: &RenderState) {
    let _ = (target, render_state);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#getViewTransitionFormatContext"
)]
fn get_view_transition_format_context(
    resumable_state: &ResumableState,
    parent_context: &FormatContext,
    update: Option<&str>,
    enter: Option<&str>,
    exit: Option<&str>,
    share: Option<&str>,
    parent_enter: Option<&Unknown>,
    parent_exit: Option<&Unknown>,
    has_parent_enter_handler: bool,
    has_parent_exit_handler: bool,
    name: Option<&str>,
    auto_name: &str,
) -> &'static FormatContext {
    let _ = (
        resumable_state,
        parent_context,
        update,
        enter,
        exit,
        share,
        parent_enter,
        parent_exit,
        has_parent_enter_handler,
        has_parent_exit_handler,
        name,
        auto_name,
    );
    unreachable!()
}

fn render_host_element(
    request: &Rc<Request>,
    task: &Rc<Task>,
    key_path: &Rc<KeyNode>,
    r#type: &str,
    props: &'static Unknown,
) {
    let segment = task.blocked_segment.borrow().clone();
    match segment {
        None => {
            // Replay
            let children = js::get(props, "children"); // TODO: Make this a Config for replaying.
            let prev_context = task.format_context.get();
            let prev_key_path = task.key_path.borrow().clone();
            task.format_context
                .set(get_child_format_context(prev_context, r#type, props));
            task.key_path.replace(Some(Rc::clone(key_path)));

            // We use the non-destructive form because if something suspends, we still
            // need to pop back up and finish this subtree of HTML.
            render_node(request, task, children, -1.0);

            // We expect that errors will fatal the whole task and that we don't need
            // the correct context. Therefore this is not in a finally.
            task.format_context.set(prev_context);
            task.key_path.replace(prev_key_path);
        }
        Some(segment) => {
            // Render
            // RenderTask always has a preambleState
            let children = push_start_instance(
                &segment.chunks,
                r#type,
                props,
                request.resumable_state,
                request.render_state,
                task.blocked_preamble.get(),
                task.hoistable_state.get(),
                task.format_context.get(),
                segment.last_pushed_text.get(),
            );
            segment.last_pushed_text.set(false);
            let prev_context = task.format_context.get();
            let prev_key_path = task.key_path.replace(Some(Rc::clone(key_path)));

            let new_context = get_child_format_context(prev_context, r#type, props);
            task.format_context.set(new_context);
            if is_preamble_context(new_context) {
                // $FlowFixMe[incompatible-type]: Refined
                render_preamble(request, task, &segment, children);
            } else {
                // We use the non-destructive form because if something suspends, we still
                // need to pop back up and finish this subtree of HTML.
                render_node(request, task, children, -1.0);
            }

            // We expect that errors will fatal the whole task and that we don't need
            // the correct context. Therefore this is not in a finally.
            task.format_context.set(prev_context);
            task.key_path.replace(prev_key_path);
            push_end_instance(
                &segment.chunks,
                r#type,
                props,
                request.resumable_state,
                prev_context,
            );
            segment.last_pushed_text.set(false);
        }
    }
}

fn should_construct(component: &Unknown) -> bool {
    let prototype = js::get(component, "prototype");
    js::truthy(prototype) && js::truthy(js::get(prototype, "isReactComponent"))
}

fn render_with_hooks(
    request: &Rc<Request>,
    task: &Rc<Task>,
    key_path: &Rc<KeyNode>,
    component: &'static Component,
    props: Option<&'static Unknown>,
    second_arg: Option<&'static Unknown>,
) -> Option<&'static Unknown> {
    // Reset the task's thenable state before continuing, so that if a later
    // component suspends we can reuse the same task object. If the same
    // component suspends again, the thenable state will be restored.
    let prev_thenable_state = task.thenable_state.take();
    let component_identity = js::unknown(js::dict::new::<()>());
    prepare_to_use_hooks(
        request,
        task,
        Some(Rc::clone(key_path)),
        component_identity,
        prev_thenable_state,
    );
    let result = if __DEV__ {
        CALL_COMPONENT_IN_DEV.with(|call_component_in_dev| call_component_in_dev.unwrap())(
            component, props, second_arg,
        )
    } else {
        component(props, second_arg)
    };
    finish_hooks(component, props, result, second_arg)
}

fn finish_class_component(
    request: &Rc<Request>,
    task: &Rc<Task>,
    key_path: &Rc<KeyNode>,
    instance: &'static Unknown,
    component: &'static Unknown,
    props: &'static Unknown,
) {
    let instance_of_class: &ClassInstance = unsafe { js::cast(Some(instance)) };
    let next_children = if __DEV__ {
        CALL_RENDER_IN_DEV.with(|call_render_in_dev| call_render_in_dev.unwrap())(instance_of_class)
    } else {
        instance_of_class.render()
    };
    if request.aborted.get() {
        // eslint-disable-next-line no-throw-literal
        js::throw(js::null());
    }

    if __DEV__ && !js::strict_equals(js::get(instance, "props"), Some(props)) {
        if !DID_WARN_ABOUT_REASSIGNING_PROPS.get() {
            console::error((
                concat!(
                    "It looks like %s is reassigning its own `this.props` while rendering. ",
                    "This is not supported and can lead to confusing bugs."
                ),
                get_component_name_from_type(Some(component))
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| "a component".to_owned()),
            ));
        }
        DID_WARN_ABOUT_REASSIGNING_PROPS.set(true);
    }

    if !DISABLE_LEGACY_CONTEXT {
        let child_context_types = js::get(component, "childContextTypes");
        if let Some(child_context_types) = child_context_types {
            let previous_context = task.legacy_context.get();
            let merged_context =
                process_child_context(instance, component, previous_context, child_context_types);
            task.legacy_context.set(merged_context);
            render_node_destructive(request, task, next_children, -1.0);
            task.legacy_context.set(previous_context);
            return;
        }
    }

    let prev_key_path = task.key_path.replace(Some(Rc::clone(key_path)));
    render_node_destructive(request, task, next_children, -1.0);
    task.key_path.replace(prev_key_path);
}

pub fn resolve_class_component_props(
    component: &'static Unknown,
    base_props: &'static Unknown,
) -> &'static Unknown {
    let mut new_props = base_props;

    // Remove ref from the props object, if it exists.
    if js::has(base_props, "ref") {
        new_props = js::unknown(js::dict::new::<()>());
        for prop_name in js::keys_in(base_props) {
            if prop_name != "ref" {
                js::set(new_props, &prop_name, js::get(base_props, &prop_name));
            }
        }
    }

    // Resolve default props.
    let default_props = js::get(component, "defaultProps");
    if let Some(default_props) =
        default_props.filter(|default_props| js::truthy(Some(*default_props)))
    {
        // We may have already copied the props object above to remove ref. If so,
        // we can modify that. Otherwise, copy the props object with Object.assign.
        if std::ptr::eq(new_props, base_props) {
            new_props = assign(js::unknown(js::dict::new::<()>()), (new_props, base_props));
        }
        // Taken from old JSX runtime, where this used to live.
        for prop_name in js::keys_in(default_props) {
            if matches!(
                js::classify(js::get(new_props, &prop_name)),
                Kind::Undefined
            ) {
                js::set(new_props, &prop_name, js::get(default_props, &prop_name));
            }
        }
    }

    new_props
}

fn render_class_component(
    request: &Rc<Request>,
    task: &Rc<Task>,
    key_path: &Rc<KeyNode>,
    component: &'static Unknown,
    props: &'static Unknown,
) {
    let resolved_props = resolve_class_component_props(component, props);
    let masked_context = if !DISABLE_LEGACY_CONTEXT {
        Some(js::unknown(get_masked_context(
            component,
            task.legacy_context.get(),
        )))
    } else {
        None
    };
    let instance = construct_class_instance(component, Some(resolved_props), masked_context);
    mount_class_instance(instance, component, Some(resolved_props), masked_context);
    finish_class_component(request, task, key_path, instance, component, resolved_props);
}

thread_local! {
    static DID_WARN_ABOUT_BAD_CLASS: &'static Dict<bool> = js::dict::new();
    static DID_WARN_ABOUT_CONTEXT_TYPES: &'static Dict<bool> = js::dict::new();
    static DID_WARN_ABOUT_CONTEXT_TYPE_ON_FUNCTION_COMPONENT: &'static Dict<bool> = js::dict::new();
    static DID_WARN_ABOUT_GET_DERIVED_STATE_ON_FUNCTION_COMPONENT: &'static Dict<bool> =
        js::dict::new();
    static DID_WARN_ABOUT_REASSIGNING_PROPS: Cell<bool> = const { Cell::new(false) };
    static DID_WARN_ABOUT_GENERATORS: Cell<bool> = const { Cell::new(false) };
    static DID_WARN_ABOUT_MAPS: Cell<bool> = const { Cell::new(false) };
}

fn component_name_or_unknown(component: &Unknown) -> String {
    get_component_name_from_type(Some(component))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Unknown".to_owned())
}

fn render_function_component(
    request: &Rc<Request>,
    task: &Rc<Task>,
    key_path: &Rc<KeyNode>,
    component: &'static Unknown,
    props: &'static Unknown,
) {
    let mut legacy_context = None;
    if !DISABLE_LEGACY_CONTEXT && !DISABLE_LEGACY_CONTEXT_FOR_FUNCTION_COMPONENTS {
        legacy_context = Some(js::unknown(get_masked_context(
            component,
            task.legacy_context.get(),
        )));
    }
    if __DEV__ {
        let prototype = js::get(component, "prototype");
        if js::truthy(prototype) && js::type_of(js::get(prototype, "render")) == "function" {
            let component_name = component_name_or_unknown(component);

            DID_WARN_ABOUT_BAD_CLASS.with(|&did_warn_about_bad_class| {
                if !js::dict::get(did_warn_about_bad_class, &component_name)
                    .copied()
                    .unwrap_or(false)
                {
                    console::error((
                        concat!(
                            "The <%s /> component appears to have a render method, but doesn't extend React.Component. ",
                            "This is likely to cause errors. Change %s to extend React.Component instead."
                        ),
                        component_name.as_str(),
                        component_name.as_str(),
                    ));
                    js::dict::set(did_warn_about_bad_class, &component_name, true);
                }
            });
        }
    }

    let value = render_with_hooks(
        request,
        task,
        key_path,
        unsafe { js::cast(Some(component)) },
        Some(props),
        legacy_context,
    );
    if request.aborted.get() {
        // eslint-disable-next-line no-throw-literal
        js::throw(js::null());
    }

    let has_id = check_did_render_id_hook();
    let action_state_count = get_action_state_count();
    let action_state_matching_index = get_action_state_matching_index();

    if __DEV__ && js::truthy(js::get(component, "contextTypes")) {
        let component_name = component_name_or_unknown(component);
        DID_WARN_ABOUT_CONTEXT_TYPES.with(|&did_warn_about_context_types| {
            if !js::dict::get(did_warn_about_context_types, &component_name)
                .copied()
                .unwrap_or(false)
            {
                js::dict::set(did_warn_about_context_types, &component_name, true);
                if DISABLE_LEGACY_CONTEXT {
                    console::error((
                        concat!(
                            "%s uses the legacy contextTypes API which was removed in React 19. ",
                            "Use React.createContext() with React.useContext() instead. ",
                            "(https://react.dev/link/legacy-context)"
                        ),
                        component_name.as_str(),
                    ));
                } else {
                    console::error((
                        concat!(
                            "%s uses the legacy contextTypes API which will be removed soon. ",
                            "Use React.createContext() with React.useContext() instead. ",
                            "(https://react.dev/link/legacy-context)"
                        ),
                        component_name.as_str(),
                    ));
                }
            }
        });
    }
    if __DEV__ {
        validate_function_component_in_dev(component);
    }
    finish_function_component(
        request,
        task,
        key_path,
        value,
        has_id,
        action_state_count,
        action_state_matching_index,
    );
}

fn finish_function_component(
    request: &Rc<Request>,
    task: &Rc<Task>,
    key_path: &Rc<KeyNode>,
    children: Option<&'static ReactNodeList>,
    has_id: bool,
    action_state_count: f64,
    action_state_matching_index: f64,
) {
    let mut did_emit_action_state_markers = false;
    if action_state_count != 0.0 && request.form_state.is_some() {
        // For each useActionState hook, emit a marker that indicates whether we
        // rendered using the form state passed at the root. We only emit these
        // markers if form state is passed at the root.
        let segment = task.blocked_segment.borrow().clone();
        if let Some(segment) = segment {
            did_emit_action_state_markers = true;
            let target = &segment.chunks;
            let mut i = 0.0;
            while i < action_state_count {
                if i == action_state_matching_index {
                    push_form_state_marker_is_matching(target);
                } else {
                    push_form_state_marker_is_not_matching(target);
                }
                i += 1.0;
            }
        } else {
            // Implies we're in reumable mode.
        }
    }

    let prev_key_path = task.key_path.replace(Some(Rc::clone(key_path)));
    if has_id {
        // This component materialized an id. We treat this as its own level, with
        // a single "child" slot.
        let prev_tree_context = Rc::clone(&task.tree_context.borrow());
        let total_children = 1;
        let index = 0;
        // Modify the id context. Because we'll need to reset this if something
        // suspends or errors, we'll use the non-destructive render path.
        task.tree_context
            .replace(push_tree_context(&prev_tree_context, total_children, index));
        render_node(request, task, children, -1.0);
        // Like the other contexts, this does not need to be in a finally block
        // because renderNode takes care of unwinding the stack.
        task.tree_context.replace(prev_tree_context);
    } else if did_emit_action_state_markers {
        // If there were useActionState hooks, we must use the non-destructive path
        // because this component is not a pure indirection; we emitted markers
        // to the stream.
        render_node(request, task, children, -1.0);
    } else {
        // We're now successfully past this task, and we haven't modified the
        // context stack. We don't have to pop back to the previous task every
        // again, so we can use the destructive recursive form.
        render_node_destructive(request, task, children, -1.0);
    }
    task.key_path.replace(prev_key_path);
}

fn validate_function_component_in_dev(component: &'static Unknown) {
    if __DEV__ {
        if js::truthy(Some(component)) && js::truthy(js::get(component, "childContextTypes")) {
            let display_name = js::get(component, "displayName");
            let name = js::get(component, "name");
            console::error((
                concat!(
                    "childContextTypes cannot be defined on a function component.\n",
                    "  %s.childContextTypes = ..."
                ),
                if js::truthy(display_name) {
                    display_name
                } else if js::truthy(name) {
                    name
                } else {
                    Some(js::unknown("Component"))
                },
            ));
        }

        if js::type_of(js::get(component, "getDerivedStateFromProps")) == "function" {
            let component_name = component_name_or_unknown(component);

            DID_WARN_ABOUT_GET_DERIVED_STATE_ON_FUNCTION_COMPONENT.with(|&did_warn| {
                if !js::dict::get(did_warn, &component_name)
                    .copied()
                    .unwrap_or(false)
                {
                    console::error((
                        "%s: Function components do not support getDerivedStateFromProps.",
                        component_name.as_str(),
                    ));
                    js::dict::set(did_warn, &component_name, true);
                }
            });
        }

        let context_type = js::get(component, "contextType");
        if js::type_of(context_type) == "object"
            && !matches!(js::classify(context_type), Kind::Null)
        {
            let component_name = component_name_or_unknown(component);

            DID_WARN_ABOUT_CONTEXT_TYPE_ON_FUNCTION_COMPONENT.with(|&did_warn| {
                if !js::dict::get(did_warn, &component_name)
                    .copied()
                    .unwrap_or(false)
                {
                    console::error((
                        "%s: Function components do not support contextType.",
                        component_name.as_str(),
                    ));
                    js::dict::set(did_warn, &component_name, true);
                }
            });
        }
    }
}

fn render_forward_ref(
    request: &Rc<Request>,
    task: &Rc<Task>,
    key_path: &Rc<KeyNode>,
    r#type: &'static Unknown,
    props: &'static Unknown,
    r#ref: Option<&'static Unknown>,
) {
    let props_without_ref = if js::has(props, "ref") {
        // `ref` is just a prop now, but `forwardRef` expects it to not appear in
        // the props object. This used to happen in the JSX runtime, but now we do
        // it here.
        let props_without_ref = js::unknown(js::dict::new::<()>());
        for key in js::keys_in(props) {
            // Since `ref` should only appear in props via the JSX transform, we can
            // assume that this is a plain object. So we don't need a
            // hasOwnProperty check.
            if key != "ref" {
                js::set(props_without_ref, &key, js::get(props, &key));
            }
        }
        props_without_ref
    } else {
        props
    };

    let children = render_with_hooks(
        request,
        task,
        key_path,
        unsafe { js::cast(js::get(r#type, "render")) },
        Some(props_without_ref),
        r#ref,
    );
    if request.aborted.get() {
        // eslint-disable-next-line no-throw-literal
        js::throw(js::null());
    }
    let has_id = check_did_render_id_hook();
    let action_state_count = get_action_state_count();
    let action_state_matching_index = get_action_state_matching_index();
    finish_function_component(
        request,
        task,
        key_path,
        children,
        has_id,
        action_state_count,
        action_state_matching_index,
    );
}

fn render_memo(
    request: &Rc<Request>,
    task: &Rc<Task>,
    key_path: &Rc<KeyNode>,
    r#type: &'static Unknown,
    props: &'static Unknown,
    r#ref: Option<&'static Unknown>,
) {
    let inner_type = js::get(r#type, "type");
    render_element(request, task, key_path, inner_type, props, r#ref);
}

fn render_context_consumer(
    request: &Rc<Request>,
    task: &Rc<Task>,
    key_path: &Rc<KeyNode>,
    context: &'static Context,
    props: &'static Unknown,
) {
    let render = js::get(props, "children");

    if __DEV__ && js::type_of(render) != "function" {
        console::error((concat!(
            "A context consumer was rendered with multiple children, or a child ",
            "that isn't a function. A context consumer expects a single child ",
            "that is a function. If you did pass a function, make sure there ",
            "is no trailing or leading whitespace around it."
        ),));
    }

    let new_value = read_context(context);
    let render: &dyn Fn(Option<&'static Unknown>) -> Option<&'static ReactNodeList> =
        unsafe { js::cast(render) };
    let new_children = render(new_value);
    if request.aborted.get() {
        // eslint-disable-next-line no-throw-literal
        js::throw(js::null());
    }

    let prev_key_path = task.key_path.replace(Some(Rc::clone(key_path)));
    render_node_destructive(request, task, new_children, -1.0);
    task.key_path.replace(prev_key_path);
}

fn render_context_provider(
    request: &Rc<Request>,
    task: &Rc<Task>,
    key_path: &Rc<KeyNode>,
    context: &'static Context,
    props: &'static Unknown,
) {
    let value = js::get(props, "value");
    let children = js::get(props, "children");
    let mut prev_snapshot = None;
    if __DEV__ {
        prev_snapshot = task.context.borrow().clone();
    }
    let prev_key_path = task.key_path.borrow().clone();
    task.context.replace(push_provider(context, value));
    task.key_path.replace(Some(Rc::clone(key_path)));
    render_node_destructive(request, task, children, -1.0);
    task.context.replace(pop_provider(context));
    task.key_path.replace(prev_key_path);
    if __DEV__ && prev_snapshot != *task.context.borrow() {
        console::error((
            "Popping the context provider did not return back to the original snapshot. This is a bug in React.",
        ));
    }
}

fn render_lazy_component(
    request: &Rc<Request>,
    task: &Rc<Task>,
    key_path: &Rc<KeyNode>,
    lazy_component: &'static LazyComponent,
    props: &'static Unknown,
    r#ref: Option<&'static Unknown>,
) {
    let component = if __DEV__ {
        CALL_LAZY_INIT_IN_DEV.with(|call_lazy_init_in_dev| call_lazy_init_in_dev.unwrap())(
            lazy_component,
        )
    } else {
        let payload = &lazy_component.payload;
        let init = lazy_component.init;
        init(payload)
    };
    if request.aborted.get() {
        // eslint-disable-next-line no-throw-literal
        js::throw(js::null());
    }
    render_element(request, task, key_path, component, props, r#ref);
}

fn render_activity(
    request: &Rc<Request>,
    task: &Rc<Task>,
    key_path: &Rc<KeyNode>,
    props: &'static Unknown,
) {
    let segment = task.blocked_segment.borrow().clone();
    match segment {
        None => {
            // Replay
            let mode = js::get(props, "mode");
            if js::strict_equals(mode, Some(js::unknown("hidden"))) {
                // A hidden Activity boundary is not server rendered. Prerendering happens
                // on the client.
            } else {
                // A visible Activity boundary has its children rendered inside the boundary.
                let prev_key_path = task.key_path.replace(Some(Rc::clone(key_path)));
                render_node(request, task, js::get(props, "children"), -1.0);
                task.key_path.replace(prev_key_path);
            }
        }
        Some(segment) => {
            // Render
            let mode = js::get(props, "mode");
            if js::strict_equals(mode, Some(js::unknown("hidden"))) {
                // A hidden Activity boundary is not server rendered. Prerendering happens
                // on the client.
            } else {
                // An Activity boundary is delimited so that we can hydrate it separately.
                push_start_activity_boundary(&segment.chunks, request.render_state);
                segment.last_pushed_text.set(false);
                // A visible Activity boundary has its children rendered inside the boundary.
                let prev_key_path = task.key_path.replace(Some(Rc::clone(key_path)));
                // We use the non-destructive form because if something suspends, we still
                // need to pop back up and finish the end comment.
                render_node(request, task, js::get(props, "children"), -1.0);
                task.key_path.replace(prev_key_path);
                push_end_activity_boundary(&segment.chunks, request.render_state);
                segment.last_pushed_text.set(false);
            }
        }
    }
}

fn render_view_transition(
    request: &Rc<Request>,
    task: &Rc<Task>,
    key_path: &Rc<KeyNode>,
    props: &ViewTransitionProps,
) {
    let prev_context = task.format_context.get();
    let prev_key_path = task.key_path.borrow().clone();
    // Get the name off props or generate an auto-generated one in case we need it.
    let auto_name =
        get_view_transition_name(props, &task.tree_context.borrow(), request.resumable_state);
    let enable_parent_enter_exit =
        ENABLE_VIEW_TRANSITION_PARENT_ENTER_EXIT.with(|&enabled| enabled);
    // Pass `undefined` (rather than the resolved class) when the prop is absent
    // so the format context can distinguish "no parentEnter/parentExit" (which
    // stops the relay) from an explicit "auto"/class (which continues it).
    let parent_enter = if enable_parent_enter_exit && props.parent_enter.is_some() {
        Some(get_view_transition_class_name(
            props.default,
            props.parent_enter,
        ))
    } else {
        None
    };
    let parent_exit = if enable_parent_enter_exit && props.parent_exit.is_some() {
        Some(get_view_transition_class_name(
            props.default,
            props.parent_exit,
        ))
    } else {
        None
    };
    task.format_context.set(get_view_transition_format_context(
        request.resumable_state,
        prev_context,
        get_view_transition_class_name(props.default, props.update).as_deref(),
        get_view_transition_class_name(props.default, props.enter).as_deref(),
        get_view_transition_class_name(props.default, props.exit).as_deref(),
        get_view_transition_class_name(props.default, props.share).as_deref(),
        // GAP rd490's kind: a class, `null` of none, or `undefined` where the
        // prop is absent, is an `Option<Option<String>>` given as `&Unknown`.
        match &parent_enter {
            Some(Some(class_name)) => Some(js::unknown(class_name.as_str())),
            Some(None) => js::null(),
            None => None,
        },
        match &parent_exit {
            Some(Some(class_name)) => Some(js::unknown(class_name.as_str())),
            Some(None) => js::null(),
            None => None,
        },
        // A ViewTransition with an onParentEnter/onParentExit handler but no class
        // still relays the activation to its descendants, so the relay must continue
        // through it even though the handler itself emits no annotation.
        enable_parent_enter_exit && props.on_parent_enter.is_some(),
        enable_parent_enter_exit && props.on_parent_exit.is_some(),
        props.name.as_deref(),
        &auto_name,
    ));
    task.key_path.replace(Some(Rc::clone(key_path)));
    if let Some(name) = &props.name
        && name != "auto"
    {
        render_node_destructive(request, task, props.children, -1.0);
    } else {
        // This will be auto-assigned a name which claims a "useId" slot.
        // This component materialized an id. We treat this as its own level, with
        // a single "child" slot.
        let prev_tree_context = Rc::clone(&task.tree_context.borrow());
        let total_children = 1;
        let index = 0;
        // Modify the id context. Because we'll need to reset this if something
        // suspends or errors, we'll use the non-destructive render path.
        task.tree_context
            .replace(push_tree_context(&prev_tree_context, total_children, index));
        render_node(request, task, props.children, -1.0);
        // Like the other contexts, this does not need to be in a finally block
        // because renderNode takes care of unwinding the stack.
        task.tree_context.replace(prev_tree_context);
    }
    task.format_context.set(prev_context);
    task.key_path.replace(prev_key_path);
}

fn render_element(
    request: &Rc<Request>,
    task: &Rc<Task>,
    key_path: &Rc<KeyNode>,
    r#type: Option<&'static Unknown>,
    props: &'static Unknown,
    r#ref: Option<&'static Unknown>,
) {
    if let Some(r#type) = r#type
        && js::type_of(Some(r#type)) == "function"
    {
        if should_construct(r#type) {
            render_class_component(request, task, key_path, r#type, props);
            return;
        } else {
            render_function_component(request, task, key_path, r#type, props);
            return;
        }
    }
    if let Kind::String(r#type) = js::classify(r#type) {
        render_host_element(request, task, key_path, r#type, props);
        return;
    }

    let symbol = SymbolFor::from_value(r#type);
    match symbol {
        // LegacyHidden acts the same as a fragment. This only works because we
        // currently assume that every instance of LegacyHidden is accompanied by a
        // host component wrapper. In the hidden mode, the host component is given a
        // `hidden` attribute, which ensures that the initial HTML is not visible.
        // To support the use of LegacyHidden as a true fragment, without an extra
        // DOM node, we would have to hide the initial HTML in some other way.
        // TODO: Delete in LegacyHidden. It's an unstable API only used in the
        // www build. As a migration step, we could add a special prop to Offscreen
        // that simulates the old behavior (no hiding, no change to effects).
        // $FlowFixMe[invalid-compare]
        // $FlowFixMe[invalid-compare] -- falls through
        // $FlowFixMe[invalid-compare] -- falls through
        // $FlowFixMe[invalid-compare] -- falls through
        Some(
            REACT_LEGACY_HIDDEN_TYPE
            | REACT_STRICT_MODE_TYPE
            | REACT_PROFILER_TYPE
            | REACT_FRAGMENT_TYPE,
        ) => {
            let prev_key_path = task.key_path.replace(Some(Rc::clone(key_path)));
            render_node_destructive(request, task, js::get(props, "children"), -1.0);
            task.key_path.replace(prev_key_path);
            return;
        }
        // $FlowFixMe[invalid-compare]
        Some(REACT_ACTIVITY_TYPE) => {
            render_activity(request, task, key_path, props);
            return;
        }
        // $FlowFixMe[invalid-compare]
        Some(REACT_SUSPENSE_LIST_TYPE) => {
            render_suspense_list(request, task, key_path, unsafe { js::cast(Some(props)) });
            return;
        }
        // $FlowFixMe[invalid-compare]
        // $FlowFixMe[invalid-compare]
        Some(REACT_VIEW_TRANSITION_TYPE | REACT_SCOPE_TYPE) => {
            if symbol == Some(REACT_VIEW_TRANSITION_TYPE) && ENABLE_VIEW_TRANSITION {
                render_view_transition(request, task, key_path, unsafe { js::cast(Some(props)) });
                return;
            }
            // Fallthrough
            if ENABLE_SCOPE_API {
                let prev_key_path = task.key_path.replace(Some(Rc::clone(key_path)));
                render_node_destructive(request, task, js::get(props, "children"), -1.0);
                task.key_path.replace(prev_key_path);
                return;
            }
            panic!("ReactDOMServer does not yet support scope components.");
        }
        // $FlowFixMe[invalid-compare]
        Some(REACT_SUSPENSE_TYPE) => {
            render_suspense_boundary(request, task, key_path, unsafe { js::cast(Some(props)) });
            return;
        }
        _ => {}
    }

    // $FlowFixMe[invalid-compare]
    if js::type_of(r#type) == "object"
        && let Some(r#type) = r#type
    {
        match SymbolFor::from_value(js::get(r#type, "$$typeof")) {
            // $FlowFixMe[invalid-compare]
            Some(REACT_FORWARD_REF_TYPE) => {
                render_forward_ref(request, task, key_path, r#type, props, r#ref);
                return;
            }
            // $FlowFixMe[invalid-compare]
            Some(REACT_MEMO_TYPE) => {
                render_memo(request, task, key_path, r#type, props, r#ref);
                return;
            }
            // $FlowFixMe[invalid-compare]
            Some(REACT_CONTEXT_TYPE) => {
                let context = unsafe { js::cast(Some(r#type)) };
                render_context_provider(request, task, key_path, context, props);
                return;
            }
            // $FlowFixMe[invalid-compare]
            Some(REACT_CONSUMER_TYPE) => {
                let context: &'static Context = unsafe { js::cast(js::get(r#type, "_context")) };
                render_context_consumer(request, task, key_path, context, props);
                return;
            }
            // $FlowFixMe[invalid-compare]
            Some(REACT_LAZY_TYPE) => {
                render_lazy_component(
                    request,
                    task,
                    key_path,
                    unsafe { js::cast(Some(r#type)) },
                    props,
                    r#ref,
                );
                return;
            }
            _ => {}
        }
    }

    let mut info = String::new();
    if __DEV__
        && (matches!(js::classify(r#type), Kind::Undefined)
            || (js::type_of(r#type) == "object"
                // $FlowFixMe[invalid-compare]
                && r#type.is_some_and(|r#type| js::object::keys(r#type).is_empty())))
    {
        info += concat!(
            " You likely forgot to export your component from the file ",
            "it's defined in, or you might have mixed up default and ",
            "named imports."
        );
    }

    panic!(
        "Element type is invalid: expected a string (for built-in components) or a class/function (for composite components) but got: {}.{}",
        if js::loose_equals(r#type, js::null::<Option<&Unknown>>()) {
            js::string(r#type)
        } else {
            js::type_of(r#type).to_owned()
        },
        info
    );
}

fn resume_node(
    request: &Rc<Request>,
    task: &Rc<Task>,
    segment_id: f64,
    node: Option<&'static ReactNodeList>,
    child_index: f64,
) {
    let prev_replay = task.replay.borrow().clone();
    let blocked_boundary = task.blocked_boundary.borrow().clone();
    let resumed_segment =
        create_pending_segment(request, 0.0, None, task.format_context.get(), false, false);
    resumed_segment.id.set(segment_id);
    resumed_segment.parent_flushed.set(true);
    let result = catch_unwind(AssertUnwindSafe(|| {
        // Convert the current ReplayTask to a RenderTask.
        let render_task = task;
        render_task.replay.replace(None);
        render_task
            .blocked_segment
            .replace(Some(Rc::clone(&resumed_segment)));
        render_node(request, task, node, child_index);
        resumed_segment.status.set(COMPLETED);
        finished_segment(request, blocked_boundary.as_ref(), &resumed_segment);
        match &blocked_boundary {
            None => {
                request
                    .completed_root_segment
                    .replace(Some(Rc::clone(&resumed_segment)));
            }
            Some(blocked_boundary) => {
                queue_completed_segment(blocked_boundary, &resumed_segment);
                if blocked_boundary.parent_flushed.get() {
                    request
                        .partial_boundaries
                        .borrow_mut()
                        .push(Rc::clone(blocked_boundary));
                }
            }
        }
    }));
    // Restore to a ReplayTask.
    task.replay.replace(prev_replay);
    task.blocked_segment.replace(None);
    if let Err(thrown_value) = result {
        resume_unwind(thrown_value);
    }
}

fn is_suspending(x: Option<&'static Unknown>) -> bool {
    js::type_of(x) == "object"
        && !matches!(js::classify(x), Kind::Null)
        && (SUSPENSE_EXCEPTION.with(|&suspense_exception| {
            js::strict_equals(x, Some(js::unknown(suspense_exception)))
        }) || js::type_of(js::get(x, "then")) == "function"
            // Rethrow so retryReplayTask can trampoline on stack overflow.
            || js::strict_equals(
                js::get(x, "message"),
                Some(js::unknown("Maximum call stack size exceeded")),
            ))
}

fn replay_element(
    request: &Rc<Request>,
    task: &Rc<Task>,
    key_path: &Rc<KeyNode>,
    name: Option<&str>,
    key_or_index: &NodeKey,
    _child_index: f64,
    r#type: Option<&'static Unknown>,
    props: &'static Unknown,
    r#ref: Option<&'static Unknown>,
    replay: &Rc<ReplaySet>,
) {
    // We're replaying. Find the path to follow.
    let replay_nodes = &replay.nodes;
    let mut i = 0;
    while i < replay_nodes.borrow().len() {
        // Flow doesn't support refinement on tuples so we do it manually here.
        let node = replay_nodes.borrow()[i].borrow().clone();
        let (ReplayNode::Element(ReplayElement(_, node_key, ..))
        | ReplayNode::Boundary(ReplaySuspenseBoundary(_, node_key, ..))) = &node;
        if key_or_index != node_key {
            i += 1;
            continue;
        }
        match node {
            ReplayNode::Element(ReplayElement(node_name, _, child_nodes, child_slots)) => {
                // Matched a replayable path.
                // Let's double check that the component name matches as a precaution.
                if let Some(name) = name
                    && Some(name) != node_name.as_deref()
                {
                    panic!(
                        "Expected the resume to render <{}> in this slot but instead it rendered <{}>. The tree doesn't match so React will fallback to client rendering.",
                        js::string(node_name.as_deref().map(js::unknown)),
                        name
                    );
                }
                let current_node = task.node.get();
                let child_replay = Rc::new(ReplaySet {
                    nodes: Rc::clone(&child_nodes),
                    slots: RefCell::new(child_slots.clone()),
                    pending_tasks: Cell::new(1.0),
                });
                task.replay.replace(Some(Rc::clone(&child_replay)));
                let result = catch_unwind(AssertUnwindSafe(|| {
                    render_element(request, task, key_path, r#type, props, r#ref);
                    if child_replay.pending_tasks.get() == 1.0
                        && !child_replay.nodes.borrow().is_empty()
                    // TODO check remaining slots
                    {
                        panic!(concat!(
                            "Couldn't find all resumable slots by key/index during replaying. ",
                            "The tree doesn't match so React will fallback to client rendering."
                        ));
                    }
                    child_replay
                        .pending_tasks
                        .set(child_replay.pending_tasks.get() - 1.0);
                }));
                if let Err(x) = result {
                    let thrown_value = thrown_value_of(&*x);
                    if is_suspending(thrown_value) {
                        // Suspend
                        if js::strict_equals(task.node.get(), current_node) {
                            // This same element suspended so we need to pop the replay we just added.
                            task.replay.replace(Some(Rc::clone(replay)));
                        } else {
                            // We finished rendering this node, so now we can consume this slot.
                            replay_nodes.borrow_mut().remove(i);
                        }
                        resume_unwind(x);
                    }
                    child_replay
                        .pending_tasks
                        .set(child_replay.pending_tasks.get() - 1.0);
                    // Unlike regular render, we don't terminate the siblings if we error
                    // during a replay. That's because this component didn't actually error
                    // in the original prerender. What's unable to complete is the child
                    // replay nodes which might be Suspense boundaries which are able to
                    // absorb the error and we can still continue with siblings.
                    let thrown_info = get_thrown_info(task.component_stack.borrow().clone());
                    errored_replay(
                        request,
                        task.blocked_boundary.borrow().clone(),
                        if request.aborted.get() {
                            request.fatal_error.get()
                        } else {
                            thrown_value
                        },
                        thrown_info,
                        &child_nodes,
                        &child_slots,
                        if __DEV__ { task.debug_task.get() } else { None },
                    );
                }
                task.replay.replace(Some(Rc::clone(replay)));
            }
            ReplayNode::Boundary(ReplaySuspenseBoundary(
                _,
                _,
                children,
                slots,
                fallback,
                root_segment_id,
            )) => {
                // Let's double check that the component type matches.
                if SymbolFor::from_value(r#type) != Some(REACT_SUSPENSE_TYPE) {
                    let expected_type = "Suspense";
                    panic!(
                        "Expected the resume to render <{}> in this slot but instead it rendered <{}>. The tree doesn't match so React will fallback to client rendering.",
                        expected_type,
                        get_component_name_from_type(r#type)
                            .filter(|name| !name.is_empty())
                            .unwrap_or_else(|| "Unknown".to_owned())
                    );
                }
                // Matched a replayable path.
                let (fallback_nodes, fallback_slots) = match fallback {
                    None => (Rc::new(RefCell::new(vec![])), ResumeSlots::None),
                    Some(fallback) => match &*fallback.borrow() {
                        ReplayNode::Element(ReplayElement(_, _, nodes, slots))
                        | ReplayNode::Boundary(ReplaySuspenseBoundary(_, _, nodes, slots, ..)) => {
                            (Rc::clone(nodes), slots.clone())
                        }
                    },
                };
                replay_suspense_boundary(
                    request,
                    task,
                    key_path,
                    unsafe { js::cast(Some(props)) },
                    root_segment_id,
                    children,
                    slots,
                    fallback_nodes,
                    fallback_slots,
                );
            }
        }
        // We finished rendering this node, so now we can consume this
        // slot. This must happen after in case we rerender this task.
        replay_nodes.borrow_mut().remove(i);
        return;
    }
    // We didn't find any matching nodes. We assume that this element was already
    // rendered in the prelude and skip it.
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#pushTextInstance"
)]
fn push_text_instance(
    target: &Chunks,
    text: &str,
    render_state: &RenderState,
    text_embedded: bool,
) -> bool {
    let _ = (target, text, render_state, text_embedded);
    unreachable!()
}

fn validate_iterable(
    task: &Rc<Task>,
    iterable: Option<&'static Unknown>,
    child_index: f64,
    iterator: &'static Iterator<Option<&'static ReactNodeList>>,
    iterator_fn: &'static Unknown,
) {
    if __DEV__ {
        if js::strict_equals(Some(js::unknown(iterator)), iterable) {
            // We don't support rendering Generators as props because it's a mutation.
            // See https://github.com/react/react/issues/12995
            // We do support generators if they were created by a GeneratorFunction component
            // as its direct child since we can recreate those by rerendering the component
            // as needed.
            let component_stack = task.component_stack.borrow().clone();
            let is_generator_component = child_index == -1.0 // Only the root child is valid
                && component_stack.as_ref().is_some_and(|component_stack| {
                    js::type_of(component_stack.r#type) == "function" // FunctionComponent
                        // $FlowFixMe[method-unbinding]
                        && js::object::to_string(component_stack.r#type).unwrap()
                            == "[object GeneratorFunction]"
                })
                // $FlowFixMe[method-unbinding]
                && js::object::to_string(Some(js::unknown(iterator))).unwrap()
                    == "[object Generator]";
            if !is_generator_component {
                if !DID_WARN_ABOUT_GENERATORS.get() {
                    console::error((concat!(
                        "Using Iterators as children is unsupported and will likely yield ",
                        "unexpected results because enumerating a generator mutates it. ",
                        "You may convert it to an array with `Array.from()` or the ",
                        "`[...spread]` operator before rendering. You can also use an ",
                        "Iterable that can iterate multiple times over the same items."
                    ),));
                }
                DID_WARN_ABOUT_GENERATORS.set(true);
            }
        } else if js::strict_equals(js::get(iterable, "entries"), Some(iterator_fn)) {
            // Warn about using Maps as children
            if !DID_WARN_ABOUT_MAPS.get() {
                console::error((concat!(
                    "Using Maps as children is not supported. ",
                    "Use an array of keyed ReactElements instead."
                ),));
                DID_WARN_ABOUT_MAPS.set(true);
            }
        }
    }
}

fn validate_async_iterable(
    task: &Rc<Task>,
    iterable: Option<&'static Unknown>,
    child_index: f64,
    iterator: &'static AsyncIterator,
) {
    if __DEV__ && js::strict_equals(Some(js::unknown(iterator)), iterable) {
        // We don't support rendering Generators as props because it's a mutation.
        // See https://github.com/react/react/issues/12995
        // We do support generators if they were created by a GeneratorFunction component
        // as its direct child since we can recreate those by rerendering the component
        // as needed.
        let component_stack = task.component_stack.borrow().clone();
        let is_generator_component = child_index == -1.0 // Only the root child is valid
            && component_stack.as_ref().is_some_and(|component_stack| {
                js::type_of(component_stack.r#type) == "function" // FunctionComponent
                    // $FlowFixMe[method-unbinding]
                    && js::object::to_string(component_stack.r#type).unwrap()
                        == "[object AsyncGeneratorFunction]"
            })
            // $FlowFixMe[method-unbinding]
            && js::object::to_string(Some(js::unknown(iterator))).unwrap()
                == "[object AsyncGenerator]";
        if !is_generator_component {
            if !DID_WARN_ABOUT_GENERATORS.get() {
                console::error((concat!(
                    "Using AsyncIterators as children is unsupported and will likely yield ",
                    "unexpected results because enumerating a generator mutates it. ",
                    "You can use an AsyncIterable that can iterate multiple times over ",
                    "the same items."
                ),));
            }
            DID_WARN_ABOUT_GENERATORS.set(true);
        }
    }
}

fn warn_on_function_type(invalid_child: &'static Unknown) {
    if __DEV__ {
        let display_name = js::get(invalid_child, "displayName");
        let name = js::get(invalid_child, "name");
        let name = if js::truthy(display_name) {
            display_name
        } else if js::truthy(name) {
            name
        } else {
            Some(js::unknown("Component"))
        };
        console::error((
            concat!(
                "Functions are not valid as a React child. This may happen if ",
                "you return %s instead of <%s /> from render. ",
                "Or maybe you meant to call this function rather than return it."
            ),
            name,
            name,
        ));
    }
}

fn warn_on_symbol_type(invalid_child: &'static Unknown) {
    if __DEV__ {
        // eslint-disable-next-line react-internal/safe-string-coercion
        let name = js::string(Some(invalid_child));
        console::error((
            concat!("Symbols are not valid as a React child.\n", "  %s"),
            name,
        ));
    }
}

// This function by it self renders a node and consumes the task by mutating it
// to update the current execution state.
fn render_node_destructive(
    request: &Rc<Request>,
    task: &Rc<Task>,
    node: Option<&'static ReactNodeList>,
    child_index: f64,
) {
    let replay = task.replay.borrow().clone();
    if let Some(replay) = replay
        && let ResumeSlots::Root(resume_segment_id) = *replay.slots.borrow()
    {
        // TODO: Figure out a cheaper place than this hot path to do this check.
        resume_node(request, task, resume_segment_id, node, child_index);
        return;
    }
    // Stash the node we're working on. We'll pick up from this task in case
    // something suspends.
    task.node.set(node);
    task.child_index.set(child_index);

    let previous_component_stack = task.component_stack.borrow().clone();
    let previous_debug_task = if __DEV__ { task.debug_task.get() } else { None };

    push_component_stack(task);

    retry_node(request, task);

    task.component_stack.replace(previous_component_stack);
    if __DEV__ {
        task.debug_task.set(previous_debug_task);
    }
}

fn retry_node(request: &Rc<Request>, task: &Rc<Task>) {
    let node = task.node.get();
    let child_index = task.child_index.get();

    if matches!(js::classify(node), Kind::Null) {
        return;
    }

    // Handle object types
    if js::type_of(node) == "object" {
        let node = node.unwrap();
        match SymbolFor::from_value(js::get(node, "$$typeof")) {
            Some(REACT_ELEMENT_TYPE) => {
                let element = node;
                let r#type = js::get(element, "type");
                let key = js::get(element, "key");
                let props = js::get(element, "props").unwrap();

                // TODO: We should get the ref off the props object right before using
                // it.
                let ref_prop = js::get(props, "ref");
                let r#ref = if !matches!(js::classify(ref_prop), Kind::Undefined) {
                    ref_prop
                } else {
                    js::null()
                };

                let debug_task = if __DEV__ { task.debug_task.get() } else { None };

                let name = get_component_name_from_type(r#type);
                let key_or_index = match js::classify(key) {
                    Kind::String(key) => NodeKey::Key(key.to_owned()),
                    // A `null` key, or the optimistic key.
                    _ => NodeKey::Index(if child_index == -1.0 {
                        0.0
                    } else {
                        child_index
                    }),
                };
                let key_path = Rc::new(KeyNode(
                    task.key_path.borrow().clone(),
                    name.clone(),
                    key_or_index.clone(),
                ));
                let replay = task.replay.borrow().clone();
                if let Some(replay) = replay {
                    if let Some(debug_task) = debug_task {
                        debug_task.run(|| {
                            replay_element(
                                request,
                                task,
                                &key_path,
                                name.as_deref(),
                                &key_or_index,
                                child_index,
                                r#type,
                                props,
                                r#ref,
                                &replay,
                            )
                        });
                    } else {
                        replay_element(
                            request,
                            task,
                            &key_path,
                            name.as_deref(),
                            &key_or_index,
                            child_index,
                            r#type,
                            props,
                            r#ref,
                            &replay,
                        );
                    }
                    // No matches found for this node. We assume it's already emitted in the
                    // prelude and skip it during the replay.
                } else {
                    // We're doing a plain render.
                    if let Some(debug_task) = debug_task {
                        debug_task
                            .run(|| render_element(request, task, &key_path, r#type, props, r#ref));
                    } else {
                        render_element(request, task, &key_path, r#type, props, r#ref);
                    }
                }
                return;
            }
            Some(REACT_PORTAL_TYPE) => {
                panic!(concat!(
                    "Portals are not currently supported by the server renderer. ",
                    "Render them conditionally so that they only appear on the client render."
                ));
            }
            Some(REACT_LAZY_TYPE) => {
                let lazy_node: &LazyComponent = unsafe { js::cast(Some(node)) };
                let resolved_node = if __DEV__ {
                    CALL_LAZY_INIT_IN_DEV
                        .with(|call_lazy_init_in_dev| call_lazy_init_in_dev.unwrap())(
                        lazy_node
                    )
                } else {
                    let payload = &lazy_node.payload;
                    let init = lazy_node.init;
                    init(payload)
                };
                if request.aborted.get() {
                    // eslint-disable-next-line no-throw-literal
                    js::throw(js::null());
                }
                // Now we render the resolved node
                render_node_destructive(request, task, resolved_node, child_index);
                return;
            }
            _ => {}
        }

        if is_array(Some(node)) {
            render_children_array(request, task, unsafe { js::cast(Some(node)) }, child_index);
            return;
        }

        let iterator_fn = get_iterator_fn(Some(node));
        if let Some(iterator_fn) = iterator_fn {
            let iterator: Option<&'static Iterator<Option<&'static ReactNodeList>>> = unsafe {
                js::cast(js::function::call(
                    js::cast::<&dyn Fn() -> Option<&'static Unknown>>(Some(iterator_fn)),
                    Some(&node),
                    (),
                ))
            };
            if request.aborted.get() {
                // eslint-disable-next-line no-throw-literal
                js::throw(js::null());
            }
            if let Some(iterator) = iterator {
                if __DEV__ {
                    validate_iterable(task, Some(node), child_index, iterator, iterator_fn);
                }
                // We need to know how many total children are in this set, so that we
                // can allocate enough id slots to acommodate them. So we must exhaust
                // the iterator before we start recursively rendering the children.
                // TODO: This is not great but I think it's inherent to the id
                // generation algorithm.
                let mut step = iterator.next();
                if request.aborted.get() {
                    // eslint-disable-next-line no-throw-literal
                    js::throw(js::null());
                }
                if step.done != Some(true) {
                    let mut children = vec![];
                    loop {
                        children.push(step.value);
                        step = iterator.next();
                        if request.aborted.get() {
                            // eslint-disable-next-line no-throw-literal
                            js::throw(js::null());
                        }
                        if step.done == Some(true) {
                            break;
                        }
                    }
                    render_children_array(request, task, &children, child_index);
                }
                return;
            }
        }

        if ENABLE_ASYNC_ITERABLE_CHILDREN.with(|&enabled| enabled)
            && js::type_of(ASYNC_ITERATOR.with(|&async_iterator| js::get(node, async_iterator)))
                == "function"
        {
            let iterator: Option<&'static AsyncIterator> = unsafe {
                js::cast(js::function::call(
                    js::cast::<&dyn Fn() -> Option<&'static Unknown>>(
                        ASYNC_ITERATOR.with(|&async_iterator| js::get(node, async_iterator)),
                    ),
                    Some(&node),
                    (),
                ))
            };
            if request.aborted.get() {
                // eslint-disable-next-line no-throw-literal
                js::throw(js::null());
            }
            if let Some(iterator) = iterator {
                if __DEV__ {
                    validate_async_iterable(task, Some(node), child_index, iterator);
                }
                // TODO: Update the task.node to be the iterator to avoid asking
                // for new iterators, but we currently warn for rendering these
                // so needs some refactoring to deal with the warning.

                // Restore the thenable state before resuming.
                let prev_thenable_state = task.thenable_state.take();
                prepare_to_use_thenable_state(prev_thenable_state);

                // We need to know how many total children are in this set, so that we
                // can allocate enough id slots to acommodate them. So we must exhaust
                // the iterator before we start recursively rendering the children.
                // TODO: This is not great but I think it's inherent to the id
                // generation algorithm.
                let mut children = vec![];

                let mut done = false;

                // $FlowFixMe[invalid-compare]
                if js::strict_equals(Some(js::unknown(iterator)), Some(node)) {
                    // If it's an iterator we need to continue reading where we left
                    // off. We can do that by reading the first few rows from the previous
                    // thenable state.
                    // $FlowFixMe[underconstrained-implicit-instantiation]
                    let mut step = read_previous_thenable_from_state();
                    while !matches!(js::classify(step), Kind::Undefined) {
                        if js::truthy(js::get(step, "done")) {
                            done = true;
                            break;
                        }
                        children.push(js::get(step, "value"));
                        step = read_previous_thenable_from_state();
                    }
                }

                if !done {
                    let mut step = unwrap_thenable(iterator.next());
                    if request.aborted.get() {
                        // eslint-disable-next-line no-throw-literal
                        js::throw(js::null());
                    }
                    while !js::truthy(js::get(step, "done")) {
                        children.push(js::get(step, "value"));
                        step = unwrap_thenable(iterator.next());
                        if request.aborted.get() {
                            // eslint-disable-next-line no-throw-literal
                            js::throw(js::null());
                        }
                    }
                }
                render_children_array(request, task, &children, child_index);
                return;
            }
        }

        // Usables are a valid React node type. When React encounters a Usable in
        // a child position, it unwraps it using the same algorithm as `use`. For
        // example, for promises, React will throw an exception to unwind the
        // stack, then replay the component once the promise resolves.
        //
        // A difference from `use` is that React will keep unwrapping the value
        // until it reaches a non-Usable type.
        //
        // e.g. Usable<Usable<Usable<T>>> should resolve to T
        let maybe_usable = node;
        if js::type_of(js::get(maybe_usable, "then")) == "function" {
            // Clear any previous thenable state that was created by the unwrapping.
            task.thenable_state.replace(None);
            let thenable: &'static Thenable = unsafe { js::cast(Some(maybe_usable)) };
            let resolved_node = unwrap_thenable(thenable);
            if request.aborted.get() {
                // eslint-disable-next-line no-throw-literal
                js::throw(js::null());
            }
            render_node_destructive(request, task, resolved_node, child_index);
            return;
        }

        if SymbolFor::from_value(js::get(maybe_usable, "$$typeof")) == Some(REACT_CONTEXT_TYPE) {
            let context: &'static Context = unsafe { js::cast(Some(maybe_usable)) };
            render_node_destructive(request, task, read_context(context), child_index);
            return;
        }

        // $FlowFixMe[method-unbinding]
        let child_string = js::object::to_string(Some(node)).unwrap();

        panic!(
            "Objects are not valid as a React child (found: {}). If you meant to render a collection of children, use an array instead.",
            if child_string == "[object Object]" {
                "object with keys {".to_owned() + &js::object::keys(node).join(", ") + "}"
            } else {
                child_string
            }
        );
    }

    if let Kind::String(node) = js::classify(node) {
        let segment = task.blocked_segment.borrow().clone();
        if let Some(segment) = segment {
            segment.last_pushed_text.set(push_text_instance(
                &segment.chunks,
                node,
                request.render_state,
                segment.last_pushed_text.get(),
            ));
        } else {
            // We assume a text node doesn't have a representation in the replay set,
            // since it can't postpone. If it does, it'll be left unmatched and error.
        }
        return;
    }

    if js::type_of(node) == "number" || js::type_of(node) == "bigint" {
        let segment = task.blocked_segment.borrow().clone();
        if let Some(segment) = segment {
            segment.last_pushed_text.set(push_text_instance(
                &segment.chunks,
                &js::concat_text(node).unwrap(),
                request.render_state,
                segment.last_pushed_text.get(),
            ));
        } else {
            // We assume a text node doesn't have a representation in the replay set,
            // since it can't postpone. If it does, it'll be left unmatched and error.
        }
        return;
    }

    if __DEV__ {
        if js::type_of(node) == "function" {
            warn_on_function_type(node.unwrap());
        }
        if js::type_of(node) == "symbol" {
            warn_on_symbol_type(node.unwrap());
        }
    }
}

fn replay_fragment(
    request: &Rc<Request>,
    task: &Rc<Task>,
    children: &[Option<&'static ReactNodeList>],
    child_index: f64,
) {
    // If we're supposed follow this array, we'd expect to see a ReplayNode matching
    // this fragment.
    let replay = task.replay.borrow().clone().unwrap();
    let replay_nodes = &replay.nodes;
    let mut j = 0;
    while j < replay_nodes.borrow().len() {
        let node = replay_nodes.borrow()[j].borrow().clone();
        let (ReplayNode::Element(ReplayElement(_, node_key, child_nodes, child_slots))
        | ReplayNode::Boundary(ReplaySuspenseBoundary(
            _,
            node_key,
            child_nodes,
            child_slots,
            ..,
        ))) = node;
        if node_key != NodeKey::Index(child_index) {
            j += 1;
            continue;
        }
        // Matched a replayable path.
        let child_replay = Rc::new(ReplaySet {
            nodes: Rc::clone(&child_nodes),
            slots: RefCell::new(child_slots.clone()),
            pending_tasks: Cell::new(1.0),
        });
        task.replay.replace(Some(Rc::clone(&child_replay)));
        let result = catch_unwind(AssertUnwindSafe(|| {
            render_children_array(request, task, children, -1.0);
            if child_replay.pending_tasks.get() == 1.0 && !child_replay.nodes.borrow().is_empty() {
                panic!(concat!(
                    "Couldn't find all resumable slots by key/index during replaying. ",
                    "The tree doesn't match so React will fallback to client rendering."
                ));
            }
            child_replay
                .pending_tasks
                .set(child_replay.pending_tasks.get() - 1.0);
        }));
        if let Err(x) = result {
            let thrown_value = thrown_value_of(&*x);
            if js::type_of(thrown_value) == "object"
                && !matches!(js::classify(thrown_value), Kind::Null)
                && (SUSPENSE_EXCEPTION.with(|&suspense_exception| {
                    js::strict_equals(thrown_value, Some(js::unknown(suspense_exception)))
                }) || js::type_of(js::get(thrown_value, "then")) == "function")
            {
                // Suspend
                resume_unwind(x);
            }
            child_replay
                .pending_tasks
                .set(child_replay.pending_tasks.get() - 1.0);
            // Unlike regular render, we don't terminate the siblings if we error
            // during a replay. That's because this component didn't actually error
            // in the original prerender. What's unable to complete is the child
            // replay nodes which might be Suspense boundaries which are able to
            // absorb the error and we can still continue with siblings.
            // This is an error, stash the component stack if it is null.
            let thrown_info = get_thrown_info(task.component_stack.borrow().clone());
            errored_replay(
                request,
                task.blocked_boundary.borrow().clone(),
                if request.aborted.get() {
                    request.fatal_error.get()
                } else {
                    thrown_value
                },
                thrown_info,
                &child_nodes,
                &child_slots,
                if __DEV__ { task.debug_task.get() } else { None },
            );
        }
        task.replay.replace(Some(Rc::clone(&replay)));
        // We finished rendering this node, so now we can consume this
        // slot. This must happen after in case we rerender this task.
        replay_nodes.borrow_mut().remove(j);
        break;
    }
}

fn warn_for_missing_key(request: &Rc<Request>, task: &Rc<Task>, child: Option<&'static Unknown>) {
    if __DEV__ {
        let Some(child) = child.filter(|_| js::type_of(child) == "object") else {
            return;
        };
        if !matches!(
            SymbolFor::from_value(js::get(child, "$$typeof")),
            Some(REACT_ELEMENT_TYPE | REACT_PORTAL_TYPE)
        ) {
            return;
        }

        let store = js::get(child, "_store");
        if !js::truthy(store)
            || ((js::truthy(js::get(store, "validated"))
                || !js::loose_equals(js::get(child, "key"), js::null::<Option<&Unknown>>()))
                && !js::strict_equals(js::get(store, "validated"), Some(js::unknown(&2.0))))
        {
            return;
        }

        if js::type_of(store) != "object" {
            panic!(concat!(
                "React Component in warnForMissingKey should have a _store. ",
                "This error is likely caused by a bug in React. Please file an issue."
            ));
        }

        // $FlowFixMe[cannot-write] unable to narrow type from mixed to writable object
        js::set(store.unwrap(), "validated", 1.0);

        let did_warn_for_key = match request.did_warn_for_key.get() {
            Some(did_warn_for_key) => did_warn_for_key,
            None => {
                let did_warn_for_key = js::weak_set::new();
                request.did_warn_for_key.set(Some(did_warn_for_key));
                did_warn_for_key
            }
        };
        let Some(parent_stack_frame) = task.component_stack.borrow().clone() else {
            // We already warned for other children in this parent.
            return;
        };
        if did_warn_for_key.has(&parent_stack_frame) {
            // We already warned for other children in this parent.
            return;
        }
        did_warn_for_key.add(&parent_stack_frame);

        let component_name = get_component_name_from_type(js::get(child, "type"));
        let child_owner = js::get(child, "_owner");
        let parent_owner = parent_stack_frame.owner;

        let mut current_component_error_info = String::new();
        if js::truthy(parent_owner)
            && !matches!(js::classify(js::get(parent_owner, "type")), Kind::Undefined)
            && let Some(name) = get_component_name_from_type(js::get(parent_owner, "type"))
                .filter(|name| !name.is_empty())
        {
            current_component_error_info =
                "\n\nCheck the render method of `".to_owned() + &name + "`.";
        }
        if current_component_error_info.is_empty()
            && let Some(component_name) = component_name.as_ref().filter(|name| !name.is_empty())
        {
            current_component_error_info =
                format!("\n\nCheck the top-level render call using <{component_name}>.");
        }

        // Usually the current owner is the offender, but if it accepts children as a
        // property, it may be the creator of the child that's responsible for
        // assigning it a key.
        let mut child_owner_appendix = String::new();
        if !js::loose_equals(child_owner, js::null::<Option<&Unknown>>())
            && !js::strict_equals(parent_owner, child_owner)
        {
            let mut owner_name = None;
            if !matches!(js::classify(js::get(child_owner, "type")), Kind::Undefined) {
                owner_name = get_component_name_from_type(js::get(child_owner, "type"));
            } else if let Kind::String(name) = js::classify(js::get(child_owner, "name")) {
                owner_name = Some(name.to_owned());
            }
            if let Some(owner_name) = owner_name.filter(|name| !name.is_empty()) {
                // Give the component that originally created this child.
                child_owner_appendix = format!(" It was passed a child from {owner_name}.");
            }
        }

        // We create a fake component stack for the child to log the stack trace from.
        let previous_component_stack = task.component_stack.borrow().clone();
        let stack_frame = create_component_stack_from_type(
            task.component_stack.borrow().clone(),
            js::get(child, "type"),
            js::get(child, "_owner"),
            js::get(child, "_debugStack"),
        );
        task.component_stack.replace(Some(stack_frame));
        console::error((
            concat!(
                "Each child in a list should have a unique \"key\" prop.",
                "%s%s See https://react.dev/link/warning-keys for more information."
            ),
            current_component_error_info,
            child_owner_appendix,
        ));
        task.component_stack.replace(previous_component_stack);
    }
}

fn render_children_array(
    request: &Rc<Request>,
    task: &Rc<Task>,
    children: &[Option<&'static ReactNodeList>],
    child_index: f64,
) {
    let prev_key_path = task.key_path.borrow().clone();
    let previous_component_stack = task.component_stack.borrow().clone();
    let mut previous_debug_task = None;
    if __DEV__ {
        previous_debug_task = task.debug_task.get();
        // We read debugInfo from task.node instead of children because it might have been an
        // unwrapped iterable so we read from the original node.
        push_server_component_stack(task, js::get(task.node.get(), "_debugInfo"));
    }
    if child_index != -1.0 {
        task.key_path.replace(Some(Rc::new(KeyNode(
            prev_key_path.clone(),
            Some("Fragment".to_owned()),
            NodeKey::Index(child_index),
        ))));
        if task.replay.borrow().is_some() {
            replay_fragment(
                request,
                // $FlowFixMe[incompatible-type]: Refined.
                task,
                children,
                child_index,
            );
            task.key_path.replace(prev_key_path);
            if __DEV__ {
                task.component_stack.replace(previous_component_stack);
                task.debug_task.set(previous_debug_task);
            }
            return;
        }
    }

    let prev_tree_context = Rc::clone(&task.tree_context.borrow());
    let total_children = children.len();

    let replay = task.replay.borrow().clone();
    if let Some(replay) = replay {
        // Replay
        // First we need to check if we have any resume slots at this level.
        let resume_slots = replay.slots.borrow().clone();
        if let ResumeSlots::Indices(resume_slots) = resume_slots {
            for (i, &node) in children.iter().enumerate() {
                task.tree_context.replace(push_tree_context(
                    &prev_tree_context,
                    total_children as i32,
                    i as i32,
                ));
                // We need to use the non-destructive form so that we can safely pop back
                // up and render the sibling if something suspends.
                let resume_segment_id = js::dict::get(resume_slots, &i.to_string()).copied();
                // TODO: If this errors we should still continue with the next sibling.
                if let Some(resume_segment_id) = resume_segment_id {
                    resume_node(request, task, resume_segment_id, node, i as f64);
                    // We finished rendering this node, so now we can consume this
                    // slot. This must happen after in case we rerender this task.
                    js::delete(resume_slots, &i.to_string());
                } else {
                    render_node(request, task, node, i as f64);
                }
            }
            task.tree_context.replace(prev_tree_context);
            task.key_path.replace(prev_key_path);
            if __DEV__ {
                task.component_stack.replace(previous_component_stack);
                task.debug_task.set(previous_debug_task);
            }
            return;
        }
    }

    for (i, &node) in children.iter().enumerate() {
        if __DEV__ {
            warn_for_missing_key(request, task, node);
        }
        task.tree_context.replace(push_tree_context(
            &prev_tree_context,
            total_children as i32,
            i as i32,
        ));
        // We need to use the non-destructive form so that we can safely pop back
        // up and render the sibling if something suspends.
        render_node(request, task, node, i as f64);
    }

    // Because this context is always set right before rendering every child, we
    // only need to reset it to the previous value at the very end.
    task.tree_context.replace(prev_tree_context);
    task.key_path.replace(prev_key_path);
    if __DEV__ {
        task.component_stack.replace(previous_component_stack);
        task.debug_task.set(previous_debug_task);
    }
}

fn next_segment_id(request: &Request) -> f64 {
    let id = request.next_segment_id.get();
    request.next_segment_id.set(id + 1.0);
    id
}

fn track_postponed_boundary(
    request: &Request,
    tracked_postpones: &PostponedHoles,
    boundary: &SuspenseBoundary,
) -> Rc<RefCell<ReplayNode>> {
    boundary.status.set(POSTPONED);
    if boundary.root_segment_id.get() == -1.0 {
        // Assign an ID before flushing. Resumed boundaries keep the ID already
        // referenced by the previous prerender's HTML.
        boundary.root_segment_id.set(next_segment_id(request));
    }

    let Some(tracked) = boundary.tracked.borrow().clone() else {
        panic!("It should not be possible to postpone at the root. This is a bug in React.");
    };

    let Some(boundary_key_path) = tracked.content_key_path.borrow().clone() else {
        panic!("It should not be possible to postpone at the root. This is a bug in React.");
    };

    let fallback_replay_node = tracked.fallback_node.borrow().clone();

    let children: ReplayNodes = Rc::new(RefCell::new(vec![]));
    let boundary_node = tracked_postpones
        .working_map
        .borrow()
        .get(&boundary_key_path)
        .cloned();
    match boundary_node {
        None => {
            let suspense_boundary =
                Rc::new(RefCell::new(ReplayNode::Boundary(ReplaySuspenseBoundary(
                    boundary_key_path.1.clone(),
                    boundary_key_path.2.clone(),
                    children,
                    ResumeSlots::None,
                    fallback_replay_node,
                    boundary.root_segment_id.get(),
                ))));
            tracked_postpones
                .working_map
                .borrow_mut()
                .insert(Rc::clone(&boundary_key_path), Rc::clone(&suspense_boundary));
            add_to_replay_parent(
                &suspense_boundary,
                boundary_key_path.0.as_ref(),
                tracked_postpones,
            );
            suspense_boundary
        }
        Some(boundary_node) => {
            // Upgrade to ReplaySuspenseBoundary.
            let suspense_boundary = boundary_node;
            let mut node = suspense_boundary.borrow_mut();
            let (ReplayNode::Element(ReplayElement(name, key, children, slots))
            | ReplayNode::Boundary(ReplaySuspenseBoundary(name, key, children, slots, ..))) =
                &*node;
            *node = ReplayNode::Boundary(ReplaySuspenseBoundary(
                name.clone(),
                key.clone(),
                Rc::clone(children),
                slots.clone(),
                fallback_replay_node,
                boundary.root_segment_id.get(),
            ));
            drop(node);
            suspense_boundary
        }
    }
}

fn track_postpone(
    request: &Request,
    tracked_postpones: &PostponedHoles,
    task: &Task,
    segment: &Segment,
) {
    segment.status.set(POSTPONED);

    let key_path = task.key_path.borrow().clone();
    let boundary = task.blocked_boundary.borrow().clone();

    let Some(boundary) = boundary else {
        segment.id.set(next_segment_id(request));
        tracked_postpones
            .root_slots
            .replace(ResumeSlots::Root(segment.id.get()));
        if let Some(completed_root_segment) = request.completed_root_segment.borrow().as_ref() {
            // Postpone the root if this was a deeper segment.
            completed_root_segment.status.set(POSTPONED);
        }
        return;
    };

    // $FlowFixMe[invalid-compare]
    if boundary.status.get() == PENDING {
        let boundary_node = track_postponed_boundary(request, tracked_postpones, &boundary);
        if let Some(tracked) = boundary.tracked.borrow().as_ref()
            && match (&*tracked.content_key_path.borrow(), &key_path) {
                (Some(content_key_path), Some(key_path)) => Rc::ptr_eq(content_key_path, key_path),
                (None, None) => true,
                _ => false,
            }
            && task.child_index.get() == -1.0
        {
            // Assign ID
            if segment.id.get() == -1.0 {
                if segment.parent_flushed.get() {
                    // If this segment's parent was already flushed, it means we really just
                    // skipped the parent and this segment is now the root.
                    segment.id.set(boundary.root_segment_id.get());
                } else {
                    segment.id.set(next_segment_id(request));
                }
            }
            // We postponed directly inside the Suspense boundary so we mark this for resuming.
            let (ReplayNode::Element(ReplayElement(_, _, _, slots))
            | ReplayNode::Boundary(ReplaySuspenseBoundary(_, _, _, slots, ..))) =
                &mut *boundary_node.borrow_mut();
            *slots = ResumeSlots::Root(segment.id.get());
            return;
        }
        // Otherwise, fall through to add the child node.
    }

    // We know that this will leave a hole so we might as well assign an ID now.
    // We might have one already if we had a parent that gave us its ID.
    if segment.id.get() == -1.0 {
        // $FlowFixMe[invalid-compare]
        if segment.parent_flushed.get() {
            // If this segment's parent was already flushed, it means we really just
            // skipped the parent and this segment is now the root.
            segment.id.set(boundary.root_segment_id.get());
        } else {
            segment.id.set(next_segment_id(request));
        }
    }

    if task.child_index.get() == -1.0 {
        // Resume starting from directly inside the previous parent element.
        match key_path {
            None => {
                tracked_postpones
                    .root_slots
                    .replace(ResumeSlots::Root(segment.id.get()));
            }
            Some(key_path) => {
                let working_map = &tracked_postpones.working_map;
                let resumable_node = working_map.borrow().get(&key_path).cloned();
                match resumable_node {
                    None => {
                        let resumable_node =
                            Rc::new(RefCell::new(ReplayNode::Element(ReplayElement(
                                key_path.1.clone(),
                                key_path.2.clone(),
                                Rc::new(RefCell::new(vec![])),
                                ResumeSlots::Root(segment.id.get()),
                            ))));
                        add_to_replay_parent(
                            &resumable_node,
                            key_path.0.as_ref(),
                            tracked_postpones,
                        );
                    }
                    Some(resumable_node) => {
                        let (ReplayNode::Element(ReplayElement(_, _, _, slots))
                        | ReplayNode::Boundary(ReplaySuspenseBoundary(_, _, _, slots, ..))) =
                            &mut *resumable_node.borrow_mut();
                        *slots = ResumeSlots::Root(segment.id.get());
                    }
                }
            }
        }
    } else {
        let slots = match key_path {
            None => {
                let slots = tracked_postpones.root_slots.borrow().clone();
                match slots {
                    ResumeSlots::None => {
                        let slots = js::dict::new();
                        tracked_postpones
                            .root_slots
                            .replace(ResumeSlots::Indices(slots));
                        slots
                    }
                    ResumeSlots::Root(_) => {
                        panic!(concat!(
                            "It should not be possible to postpone both at the root of an element ",
                            "as well as a slot below. This is a bug in React."
                        ));
                    }
                    ResumeSlots::Indices(slots) => slots,
                }
            }
            Some(key_path) => {
                let working_map = &tracked_postpones.working_map;
                let resumable_node = working_map.borrow().get(&key_path).cloned();
                match resumable_node {
                    None => {
                        let slots = js::dict::new();
                        let resumable_node =
                            Rc::new(RefCell::new(ReplayNode::Element(ReplayElement(
                                key_path.1.clone(),
                                key_path.2.clone(),
                                Rc::new(RefCell::new(vec![])),
                                ResumeSlots::Indices(slots),
                            ))));
                        working_map
                            .borrow_mut()
                            .insert(Rc::clone(&key_path), Rc::clone(&resumable_node));
                        add_to_replay_parent(
                            &resumable_node,
                            key_path.0.as_ref(),
                            tracked_postpones,
                        );
                        slots
                    }
                    Some(resumable_node) => {
                        let mut resumable_node = resumable_node.borrow_mut();
                        let (ReplayNode::Element(ReplayElement(_, _, _, resumable_slots))
                        | ReplayNode::Boundary(ReplaySuspenseBoundary(
                            _,
                            _,
                            _,
                            resumable_slots,
                            ..,
                        ))) = &mut *resumable_node;
                        let slots = resumable_slots.clone();
                        match slots {
                            ResumeSlots::None => {
                                let slots = js::dict::new();
                                *resumable_slots = ResumeSlots::Indices(slots);
                                slots
                            }
                            ResumeSlots::Root(_) => {
                                panic!(concat!(
                                    "It should not be possible to postpone both at the root of an element ",
                                    "as well as a slot below. This is a bug in React."
                                ));
                            }
                            ResumeSlots::Indices(slots) => slots,
                        }
                    }
                }
            }
        };
        js::dict::set(slots, &task.child_index.get().to_string(), segment.id.get());
    }
}

// In case a boundary errors, we need to stop tracking it because we won't
// resume it.
fn untrack_boundary(request: &Request, boundary: &SuspenseBoundary) {
    let Some(tracked_postpones) = request.tracked_postpones.borrow().clone() else {
        return;
    };
    let Some(tracked) = boundary.tracked.borrow().clone() else {
        return;
    };
    let Some(boundary_key_path) = tracked.content_key_path.borrow().clone() else {
        return;
    };
    let Some(boundary_node) = tracked_postpones
        .working_map
        .borrow()
        .get(&boundary_key_path)
        .cloned()
    else {
        return;
    };

    // Downgrade to plain ReplayNode since we won't replay through it.
    // $FlowFixMe[cannot-write]: We intentionally downgrade this to the other tuple.
    let mut node = boundary_node.borrow_mut();
    let (ReplayNode::Element(ReplayElement(name, key, ..))
    | ReplayNode::Boundary(ReplaySuspenseBoundary(name, key, ..))) = &*node;
    // Remove any resumable slots.
    *node = ReplayNode::Element(ReplayElement(
        name.clone(),
        key.clone(),
        Rc::new(RefCell::new(vec![])),
        ResumeSlots::None,
    ));

    // TODO: We should really just remove the boundary from all parent paths too so
    // we don't replay the path to it.
}

fn spawn_new_suspended_replay_task(
    request: &Rc<Request>,
    task: &Task,
    thenable_state: Option<Rc<ThenableState>>,
) -> Rc<Task> {
    create_replay_task(
        request,
        thenable_state,
        task.replay.borrow().clone().unwrap(),
        task.node.get(),
        task.child_index.get(),
        task.blocked_boundary.borrow().clone(),
        task.hoistable_state.get(),
        Rc::clone(&task.abort_set.borrow()),
        task.key_path.borrow().clone(),
        task.format_context.get(),
        task.context.borrow().clone(),
        Rc::clone(&task.tree_context.borrow()),
        task.row.borrow().clone(),
        task.component_stack.borrow().clone(),
        if !DISABLE_LEGACY_CONTEXT {
            task.legacy_context.get()
        } else {
            EMPTY_CONTEXT_OBJECT.with(|empty_context_object| *empty_context_object)
        },
        if __DEV__ { task.debug_task.get() } else { None },
    )
}

fn spawn_new_suspended_render_task(
    request: &Rc<Request>,
    task: &Task,
    thenable_state: Option<Rc<ThenableState>>,
) -> Rc<Task> {
    // Something suspended, we'll need to create a new segment and resolve it later.
    let segment = task.blocked_segment.borrow().clone().unwrap();
    let insertion_index = segment.chunks.borrow().len() as f64;
    let new_segment = create_pending_segment(
        request,
        insertion_index,
        None,
        task.format_context.get(),
        // Adopt the parent segment's leading text embed
        segment.last_pushed_text.get(),
        // Assume we are text embedded at the trailing edge
        true,
    );
    segment.children.borrow_mut().push(Rc::clone(&new_segment));
    // Reset lastPushedText for current Segment since the new Segment "consumed" it
    segment.last_pushed_text.set(false);
    create_render_task(
        request,
        thenable_state,
        task.node.get(),
        task.child_index.get(),
        task.blocked_boundary.borrow().clone(),
        new_segment,
        task.blocked_preamble.get(),
        task.hoistable_state.get(),
        Rc::clone(&task.abort_set.borrow()),
        task.key_path.borrow().clone(),
        task.format_context.get(),
        task.context.borrow().clone(),
        Rc::clone(&task.tree_context.borrow()),
        task.row.borrow().clone(),
        task.component_stack.borrow().clone(),
        if !DISABLE_LEGACY_CONTEXT {
            task.legacy_context.get()
        } else {
            EMPTY_CONTEXT_OBJECT.with(|empty_context_object| *empty_context_object)
        },
        if __DEV__ { task.debug_task.get() } else { None },
    )
}

fn postpone_new_suspended_render_task(
    request: &Rc<Request>,
    task: &Task,
    thenable_state: Option<Rc<ThenableState>>,
) {
    // Preserve the segment rendered so far and leave a hole at this node. The
    // request has already been aborted, so this new task must be finished now
    // rather than scheduled for another attempt.
    let postponed_task = spawn_new_suspended_render_task(request, task, thenable_state);
    if __DEV__ {
        abort_task_dev(&postponed_task, request);
    } else {
        abort_task(&postponed_task, request);
    }
    let abort_set = Rc::clone(&postponed_task.abort_set.borrow());
    abort_set.borrow_mut().remove(&postponed_task);
    if __DEV__ {
        finish_aborted_task_dev(&postponed_task, request, request.fatal_error.get());
    } else {
        finish_aborted_task(&postponed_task, request, request.fatal_error.get());
    }
}

// This is a non-destructive form of rendering a node. If it suspends it spawns
// a new task and restores the context of this task to what it was before.
fn render_node(
    request: &Rc<Request>,
    task: &Rc<Task>,
    node: Option<&'static ReactNodeList>,
    child_index: f64,
) {
    // Snapshot the current context in case something throws to interrupt the
    // process.
    let previous_format_context = task.format_context.get();
    let previous_legacy_context = if !DISABLE_LEGACY_CONTEXT {
        task.legacy_context.get()
    } else {
        EMPTY_CONTEXT_OBJECT.with(|empty_context_object| *empty_context_object)
    };
    let previous_context = task.context.borrow().clone();
    let previous_key_path = task.key_path.borrow().clone();
    let previous_tree_context = Rc::clone(&task.tree_context.borrow());
    let previous_component_stack = task.component_stack.borrow().clone();
    let previous_debug_task = if __DEV__ { task.debug_task.get() } else { None };
    let x;
    let thrown;
    // Store how much we've pushed at this point so we can reset it in case something
    // suspended partially through writing something.
    let segment = task.blocked_segment.borrow().clone();
    match segment {
        None => {
            // Replay
            let previous_replay_set = task.replay.borrow().clone();
            let Err(thrown_value) = catch_unwind(AssertUnwindSafe(|| {
                render_node_destructive(request, task, node, child_index)
            })) else {
                return;
            };
            reset_hooks_state();

            let is_suspense_exception = is_suspense_exception(thrown_value_of(&*thrown_value));
            x = if is_suspense_exception {
                // This is a special type of exception used for Suspense. For historical
                // reasons, the rest of the Suspense implementation expects the thrown
                // value to be a thenable, because before `use` existed that was the
                // (unstable) API for suspending. This implementation detail can change
                // later, once we deprecate the old API in favor of `use`.
                Some(js::unknown(get_suspended_thenable()))
            } else {
                thrown_value_of(&*thrown_value)
            };
            thrown = thrown_value;

            if request.aborted.get() {
                // We are aborting so we can just bubble up to the task by falling through
                // $FlowFixMe[invalid-compare]
            } else if js::type_of(x) == "object" && !matches!(js::classify(x), Kind::Null) {
                // $FlowFixMe[method-unbinding]
                if js::type_of(js::get(x, "then")) == "function" {
                    let thenable: &'static Thenable = unsafe { js::cast(x) };
                    let thenable_state = if is_suspense_exception {
                        get_thenable_state_after_suspending()
                    } else {
                        None
                    };
                    let new_task = spawn_new_suspended_replay_task(
                        request,
                        // $FlowFixMe[incompatible-type]: Refined.
                        task,
                        thenable_state,
                    );
                    let (resolve, reject) = ping_of(request, &new_task);
                    thenable.then(resolve, reject);

                    // Restore the context. We assume that this will be restored by the inner
                    // functions in case nothing throws so we don't use "finally" here.
                    task.format_context.set(previous_format_context);
                    if !DISABLE_LEGACY_CONTEXT {
                        task.legacy_context.set(previous_legacy_context);
                    }
                    task.context.replace(previous_context.clone());
                    task.key_path.replace(previous_key_path);
                    task.tree_context.replace(previous_tree_context);
                    task.component_stack.replace(previous_component_stack);
                    task.replay.replace(previous_replay_set);
                    if __DEV__ {
                        task.debug_task.set(previous_debug_task);
                    }
                    // Restore all active ReactContexts to what they were before.
                    switch_context(previous_context);
                    return;
                }
                if js::strict_equals(
                    js::get(x, "message"),
                    Some(js::unknown("Maximum call stack size exceeded")),
                ) {
                    // This was a stack overflow. We do a lot of recursion in React by default for
                    // performance but it can lead to stack overflows in extremely deep trees.
                    // We do have the ability to create a trampoile if this happens which makes
                    // this kind of zero-cost.
                    let thenable_state = if is_suspense_exception {
                        get_thenable_state_after_suspending()
                    } else {
                        None
                    };
                    let new_task = spawn_new_suspended_replay_task(
                        request,
                        // $FlowFixMe[incompatible-type]: Refined.
                        task,
                        thenable_state,
                    );

                    // Immediately schedule the task for retrying.
                    request.pinged_tasks.borrow_mut().push(new_task);

                    // Restore the context. We assume that this will be restored by the inner
                    // functions in case nothing throws so we don't use "finally" here.
                    task.format_context.set(previous_format_context);
                    if !DISABLE_LEGACY_CONTEXT {
                        task.legacy_context.set(previous_legacy_context);
                    }
                    task.context.replace(previous_context.clone());
                    task.key_path.replace(previous_key_path);
                    task.tree_context.replace(previous_tree_context);
                    task.component_stack.replace(previous_component_stack);
                    task.replay.replace(previous_replay_set);
                    if __DEV__ {
                        task.debug_task.set(previous_debug_task);
                    }
                    // Restore all active ReactContexts to what they were before.
                    switch_context(previous_context);
                    return;
                }
            }

            // TODO: Abort any undiscovered Suspense boundaries in the ReplayNode.
        }
        Some(segment) => {
            // Render
            let children_length = segment.children.borrow().len();
            let chunk_length = segment.chunks.borrow().len();
            let Err(thrown_value) = catch_unwind(AssertUnwindSafe(|| {
                render_node_destructive(request, task, node, child_index)
            })) else {
                return;
            };
            reset_hooks_state();

            // Reset the write pointers to where we started.
            segment.children.borrow_mut().truncate(children_length);
            segment.chunks.borrow_mut().truncate(chunk_length);

            let is_suspense_exception = is_suspense_exception(thrown_value_of(&*thrown_value));
            x = if is_suspense_exception {
                // This is a special type of exception used for Suspense. For historical
                // reasons, the rest of the Suspense implementation expects the thrown
                // value to be a thenable, because before `use` existed that was the
                // (unstable) API for suspending. This implementation detail can change
                // later, once we deprecate the old API in favor of `use`.
                Some(js::unknown(get_suspended_thenable()))
            } else {
                thrown_value_of(&*thrown_value)
            };
            thrown = thrown_value;

            if request.aborted.get() {
                // A postponed root cannot flush any shell and must be rendered again in
                // full, so let it unwind instead of preserving individual holes.
                if request.tracked_postpones.borrow().is_some()
                    && task.blocked_boundary.borrow().is_some()
                {
                    let thenable_state = if is_suspense_exception {
                        get_thenable_state_after_suspending()
                    } else {
                        None
                    };
                    postpone_new_suspended_render_task(
                        request,
                        // $FlowFixMe[incompatible-type]: Refined.
                        task,
                        thenable_state,
                    );

                    // Restore the context. We assume that this will be restored by the inner
                    // functions in case nothing throws so we don't use "finally" here.
                    task.format_context.set(previous_format_context);
                    if !DISABLE_LEGACY_CONTEXT {
                        task.legacy_context.set(previous_legacy_context);
                    }
                    task.context.replace(previous_context.clone());
                    task.key_path.replace(previous_key_path);
                    task.tree_context.replace(previous_tree_context);
                    task.component_stack.replace(previous_component_stack);
                    if __DEV__ {
                        task.debug_task.set(previous_debug_task);
                    }
                    // Restore all active ReactContexts to what they were before.
                    switch_context(previous_context);
                    return;
                }
            } else if js::type_of(x) == "object"
                // $FlowFixMe[invalid-compare]
                && !matches!(js::classify(x), Kind::Null)
            {
                // $FlowFixMe[method-unbinding]
                if js::type_of(js::get(x, "then")) == "function" {
                    let thenable: &'static Thenable = unsafe { js::cast(x) };
                    let thenable_state = if is_suspense_exception {
                        get_thenable_state_after_suspending()
                    } else {
                        None
                    };
                    let new_task = spawn_new_suspended_render_task(
                        request,
                        // $FlowFixMe[incompatible-type]: Refined.
                        task,
                        thenable_state,
                    );
                    let (resolve, reject) = ping_of(request, &new_task);
                    thenable.then(resolve, reject);

                    // Restore the context. We assume that this will be restored by the inner
                    // functions in case nothing throws so we don't use "finally" here.
                    task.format_context.set(previous_format_context);
                    if !DISABLE_LEGACY_CONTEXT {
                        task.legacy_context.set(previous_legacy_context);
                    }
                    task.context.replace(previous_context.clone());
                    task.key_path.replace(previous_key_path);
                    task.tree_context.replace(previous_tree_context);
                    task.component_stack.replace(previous_component_stack);
                    if __DEV__ {
                        task.debug_task.set(previous_debug_task);
                    }
                    // Restore all active ReactContexts to what they were before.
                    switch_context(previous_context);
                    return;
                }
                if js::strict_equals(
                    js::get(x, "message"),
                    Some(js::unknown("Maximum call stack size exceeded")),
                ) {
                    // This was a stack overflow. We do a lot of recursion in React by default for
                    // performance but it can lead to stack overflows in extremely deep trees.
                    // We do have the ability to create a trampoile if this happens which makes
                    // this kind of zero-cost.
                    let thenable_state = if is_suspense_exception {
                        get_thenable_state_after_suspending()
                    } else {
                        None
                    };
                    let new_task = spawn_new_suspended_render_task(
                        request,
                        // $FlowFixMe[incompatible-type]: Refined.
                        task,
                        thenable_state,
                    );

                    // Immediately schedule the task for retrying.
                    request.pinged_tasks.borrow_mut().push(new_task);

                    // Restore the context. We assume that this will be restored by the inner
                    // functions in case nothing throws so we don't use "finally" here.
                    task.format_context.set(previous_format_context);
                    if !DISABLE_LEGACY_CONTEXT {
                        task.legacy_context.set(previous_legacy_context);
                    }
                    task.context.replace(previous_context.clone());
                    task.key_path.replace(previous_key_path);
                    task.tree_context.replace(previous_tree_context);
                    task.component_stack.replace(previous_component_stack);
                    if __DEV__ {
                        task.debug_task.set(previous_debug_task);
                    }
                    // Restore all active ReactContexts to what they were before.
                    switch_context(previous_context);
                    return;
                }
            }
        }
    }

    // Restore the context. We assume that this will be restored by the inner
    // functions in case nothing throws so we don't use "finally" here.
    task.format_context.set(previous_format_context);
    if !DISABLE_LEGACY_CONTEXT {
        task.legacy_context.set(previous_legacy_context);
    }
    task.context.replace(previous_context.clone());
    task.key_path.replace(previous_key_path);
    task.tree_context.replace(previous_tree_context);
    // We intentionally do not restore the component stack on the error pathway
    // Whatever handles the error needs to use this stack which is the location of the
    // error. We must restore the stack wherever we handle this

    // Restore all active ReactContexts to what they were before.
    switch_context(previous_context);

    // What was thrown, or the thenable that suspended in its place.
    if js::strict_equals(x, thrown_value_of(&*thrown)) {
        resume_unwind(thrown);
    }
    js::throw(x);
}

fn is_suspense_exception(thrown_value: Option<&'static Unknown>) -> bool {
    SUSPENSE_EXCEPTION.with(|&suspense_exception| {
        js::strict_equals(thrown_value, Some(js::unknown(suspense_exception)))
    })
}

fn errored_replay(
    request: &Rc<Request>,
    boundary: Option<Rc<SuspenseBoundary>>,
    error: Option<&'static Unknown>,
    error_info: &'static ThrownInfo,
    replay_nodes: &ReplayNodes,
    resume_slots: &ResumeSlots,
    debug_task: Option<&'static ConsoleTask>,
) {
    // Erroring during a replay doesn't actually cause an error by itself because
    // that component has already rendered. What causes the error is the resumable
    // points that we did not yet finish which will be below the point of the reset.
    // For example, if we're replaying a path to a Suspense boundary that is not done
    // that doesn't error the parent Suspense boundary.
    // This might be a bit strange that the error in a parent gets thrown at a child.
    // We log it only once and reuse the digest.
    let error_digest = log_recoverable_error(request, error, error_info, debug_task);
    abort_remaining_replay_nodes(
        request,
        boundary,
        replay_nodes,
        resume_slots,
        error,
        error_digest,
        error_info,
        false,
    );
}

fn errored_task(
    request: &Rc<Request>,
    boundary: Option<&Rc<SuspenseBoundary>>,
    row: Option<&Rc<SuspenseListRow>>,
    error: Option<&'static Unknown>,
    error_info: &'static ThrownInfo,
    debug_task: Option<&'static ConsoleTask>,
) {
    if let Some(row) = row {
        row.pending_tasks.set(row.pending_tasks.get() - 1.0);
        if row.pending_tasks.get() == 0.0 {
            finish_suspense_list_row(request, row);
        }
    }

    request
        .all_pending_tasks
        .set(request.all_pending_tasks.get() - 1.0);

    // We don't handle halts here because we only halt when prerendering and
    // when prerendering we should be finishing tasks not erroring them when
    // they halt or postpone
    match boundary {
        None => {
            // Recoverables can remain silent when a Suspense boundary lets us emit a
            // shell and defer its content to a downstream renderer. At the root there
            // is no shell to stream, so this is a fatal error and must be reported like
            // any other root error.
            if is_recoverable_error(error) {
                // This recoverable reached the root without a Suspense boundary, so
                // report it using the fatal diagnostic while leaving the original intact.
                let fatal_recoverable_error =
                    Some(js::unknown(clone_recoverable_error_as_fatal(unsafe {
                        js::cast(error)
                    })));
                log_recoverable_error(request, fatal_recoverable_error, error_info, debug_task);
                fatal_error(request, fatal_recoverable_error, error_info, debug_task);
            } else {
                log_recoverable_error(request, error, error_info, debug_task);
                fatal_error(request, error, error_info, debug_task);
            }
            // The shell fatally errored, so the render can never complete. Return before
            // the completeAll check below so we don't fire onAllReady for a render that
            // produced nothing. This mirrors finishAbortedTask, which also returns after
            // a fatalError on the root.
            return;
        }
        Some(boundary) => {
            let error_digest = log_recoverable_error(request, error, error_info, debug_task);
            boundary
                .pending_tasks
                .set(boundary.pending_tasks.get() - 1.0);
            if boundary.status.get() != CLIENT_RENDERED {
                boundary.status.set(CLIENT_RENDERED);
                encode_error_for_boundary(boundary, error_digest, error, error_info, false);
                untrack_boundary(request, boundary);

                let boundary_row = boundary.row.borrow().clone();
                if let Some(boundary_row) = boundary_row {
                    // Unblock the SuspenseListRow that was blocked by this boundary.
                    // finishSuspenseListRow → unblockSuspenseListRow → finishedTask reenters
                    // and decrements allPendingTasks. Pin the counter above zero so those
                    // nested calls can't trip completeAll before this outer frame's own
                    // zero check at the end.
                    request
                        .all_pending_tasks
                        .set(request.all_pending_tasks.get() + 1.0);
                    boundary_row
                        .pending_tasks
                        .set(boundary_row.pending_tasks.get() - 1.0);
                    if boundary_row.pending_tasks.get() == 0.0 {
                        finish_suspense_list_row(request, &boundary_row);
                    }
                    request
                        .all_pending_tasks
                        .set(request.all_pending_tasks.get() - 1.0);
                }

                // Regardless of what happens next, this boundary won't be displayed,
                // so we can flush it, if the parent already flushed.
                if boundary.parent_flushed.get() {
                    // We don't have a preference where in the queue this goes since it's likely
                    // to error on the client anyway. However, intentionally client-rendered
                    // boundaries should be flushed earlier so that they can start on the client.
                    // We reuse the same queue for errors.
                    request
                        .client_rendered_boundaries
                        .borrow_mut()
                        .push(Rc::clone(boundary));
                }

                if request.pending_root_tasks.get() == 0.0
                    && request.tracked_postpones.borrow().is_none()
                    && boundary.preamble.is_some()
                {
                    // The root is complete and this boundary may contribute part of the preamble.
                    // We eagerly attempt to prepare the preamble here because we expect most requests
                    // to have few boundaries which contribute preambles and it allow us to do this
                    // preparation work during the work phase rather than the when flushing.
                    prepare_preamble(request);
                }
            }
        }
    }

    if request.all_pending_tasks.get() == 0.0 {
        complete_all(request);
    }
}

fn abort_task_soft(request: &Rc<Request>, task: &Rc<Task>) {
    // This aborts task without aborting the parent boundary that it blocks.
    // It's used for when we didn't need this task to complete the tree.
    // If task was needed, then it should use abortTask instead.
    let boundary = task.blocked_boundary.borrow().clone();
    let segment = task.blocked_segment.borrow().clone();
    if let Some(segment) = segment {
        segment.status.set(ABORTED);
        finished_task(
            request,
            boundary.as_ref(),
            task.row.borrow().as_ref(),
            Some(&segment),
        );
    }
}

fn abort_remaining_suspense_boundary(
    request: &Rc<Request>,
    root_segment_id: f64,
    error: Option<&'static Unknown>,
    error_digest: Option<String>,
    error_info: &ThrownInfo,
    was_aborted: bool,
) {
    let resumed_boundary = create_suspense_boundary(
        request,
        None,
        Rc::new(RefCell::new(HashSet::new())),
        None,
        false,
    );
    resumed_boundary.parent_flushed.set(true);
    // We restore the same id of this boundary as was used during prerender.
    resumed_boundary.root_segment_id.set(root_segment_id);

    resumed_boundary.status.set(CLIENT_RENDERED);
    encode_error_for_boundary(
        &resumed_boundary,
        error_digest,
        error,
        error_info,
        was_aborted,
    );

    if resumed_boundary.parent_flushed.get() {
        request
            .client_rendered_boundaries
            .borrow_mut()
            .push(resumed_boundary);
    }
}

fn abort_remaining_replay_nodes(
    request: &Rc<Request>,
    boundary: Option<Rc<SuspenseBoundary>>,
    nodes: &ReplayNodes,
    slots: &ResumeSlots,
    error: Option<&'static Unknown>,
    error_digest: Option<String>,
    error_info: &'static ThrownInfo,
    aborted: bool,
) {
    for node in nodes.borrow().iter() {
        match &*node.borrow() {
            ReplayNode::Element(ReplayElement(_, _, child_nodes, child_slots)) => {
                abort_remaining_replay_nodes(
                    request,
                    boundary.clone(),
                    child_nodes,
                    child_slots,
                    error,
                    error_digest.clone(),
                    error_info,
                    aborted,
                );
            }
            ReplayNode::Boundary(boundary_node) => {
                let root_segment_id = boundary_node.5;
                abort_remaining_suspense_boundary(
                    request,
                    root_segment_id,
                    error,
                    error_digest.clone(),
                    error_info,
                    aborted,
                );
            }
        }
    }
    // Empty the set, since we've cleared it now.
    nodes.borrow_mut().clear();

    if !matches!(slots, ResumeSlots::None) {
        // We had something still to resume in the parent boundary. We must trigger
        // the error on the parent boundary since it's not able to complete.
        match &boundary {
            None => {
                panic!(concat!(
                    "We should not have any resumable nodes in the shell. ",
                    "This is a bug in React."
                ));
            }
            Some(boundary) if boundary.status.get() != CLIENT_RENDERED => {
                boundary.status.set(CLIENT_RENDERED);
                encode_error_for_boundary(boundary, error_digest, error, error_info, aborted);
                if boundary.parent_flushed.get() {
                    request
                        .client_rendered_boundaries
                        .borrow_mut()
                        .push(Rc::clone(boundary));
                }
            }
            Some(_) => {}
        }
        // Empty the set
        if let ResumeSlots::Indices(slots) = slots {
            for index in js::keys_in(js::unknown(*slots)) {
                js::delete(*slots, &index);
            }
        }
    }
}

fn is_current_task(request: &Request, task: &Rc<Task>) -> bool {
    request
        .current_task
        .borrow()
        .as_ref()
        .is_some_and(|current_task| Rc::ptr_eq(current_task, task))
}

fn abort_task(task: &Rc<Task>, request: &Rc<Request>) {
    // Mark pending tasks as aborted synchronously so any work that was already
    // scheduled cannot begin after abort is called.
    if is_current_task(request, task) {
        // This is a currently rendering Task. The render itself will abort the task.
        return;
    }
    let boundary = task.blocked_boundary.borrow().clone();
    let segment = task.blocked_segment.borrow().clone();
    if let Some(segment) = segment {
        segment.status.set(ABORTED);
    }

    if __DEV__ {
        // Capture async debug information at the point abort begins. The task may
        // receive more data before finishAbort runs and no longer suspend at the
        // call site we need to report.
        let mut node = task.node.get();
        if js::type_of(node) == "object" && !matches!(js::classify(node), Kind::Null) {
            let mut debug_info = js::get(node, "_debugInfo");
            while js::type_of(node) == "object"
                && !matches!(js::classify(node), Kind::Null)
                && SymbolFor::from_value(js::get(node, "$$typeof")) == Some(REACT_LAZY_TYPE)
            {
                let payload = js::get(node, "_payload");
                if js::strict_equals(js::get(payload, "status"), Some(js::unknown("fulfilled"))) {
                    node = js::get(payload, "value");
                    continue;
                }
                break;
            }
            if js::type_of(node) == "object"
                && !matches!(js::classify(node), Kind::Null)
                && (is_array(node)
                    || js::type_of(
                        ASYNC_ITERATOR.with(|&async_iterator| js::get(node, async_iterator)),
                    ) == "function"
                    // $FlowFixMe[invalid-compare]
                    || SymbolFor::from_value(js::get(node, "$$typeof")) == Some(REACT_ELEMENT_TYPE)
                    // $FlowFixMe[invalid-compare]
                    || SymbolFor::from_value(js::get(node, "$$typeof")) == Some(REACT_LAZY_TYPE))
                && is_array(js::get(node, "_debugInfo"))
            {
                debug_info = js::get(node, "_debugInfo");
            }
            push_halted_await_on_component_stack(task, debug_info);
            if task.thenable_state.borrow().is_some() {
                push_suspended_call_site_on_component_stack(request, task);
            }
        }
    }

    if let Some(boundary) = boundary {
        for fallback_task in boundary.fallback_abortable_tasks.borrow().iter() {
            abort_task(fallback_task, request);
        }
    }
}

fn finish_aborted_task(task: &Rc<Task>, request: &Rc<Request>, error: Option<&'static Unknown>) {
    // Report and complete a task that was synchronously claimed by abortTask.
    // A currently rendering task remains responsible for unwinding itself.
    if is_current_task(request, task) {
        return;
    }
    let boundary = task.blocked_boundary.borrow().clone();
    let segment = task.blocked_segment.borrow().clone();
    if let Some(segment) = &segment
        && segment.status.get() != ABORTED
    {
        return;
    }

    let error_info = get_thrown_info(task.component_stack.borrow().clone());
    // Only errors materialized by use() or abort() carry this internal brand.
    // Throwing the browser() token directly is still an application error.
    let is_recoverable_reason = is_recoverable_error(error);

    match &boundary {
        None => {
            let replay = task.replay.borrow().clone();
            let Some(replay) = replay else {
                // We didn't complete the root so we have nothing to show. We can close
                // the request;
                let tracked_postpones = request.tracked_postpones.borrow().clone();
                if !is_recoverable_reason
                    && let Some(tracked_postpones) = tracked_postpones
                    && let Some(segment) = &segment
                {
                    // We are aborting a prerender and must treat the shell as halted
                    // We log the error but we still resolve the prerender
                    log_recoverable_error(request, error, error_info, task.debug_task.get());
                    track_postpone(request, &tracked_postpones, task, segment);
                    finished_task(request, None, task.row.borrow().as_ref(), Some(segment));
                } else if is_recoverable_reason {
                    // This root task cannot recover from the abort. Report a fatal clone,
                    // but keep the original branded reason on the request for other tasks.
                    let fatal_recoverable_error =
                        Some(js::unknown(clone_recoverable_error_as_fatal(unsafe {
                            js::cast(error)
                        })));
                    log_recoverable_error(
                        request,
                        fatal_recoverable_error,
                        error_info,
                        task.debug_task.get(),
                    );
                    if request.status.get() != CLOSING && request.status.get() != CLOSED {
                        fatal_error(
                            request,
                            fatal_recoverable_error,
                            error_info,
                            task.debug_task.get(),
                        );
                    }
                } else {
                    log_recoverable_error(request, error, error_info, task.debug_task.get());
                    if request.status.get() != CLOSING && request.status.get() != CLOSED {
                        fatal_error(request, error, error_info, task.debug_task.get());
                    }
                }
                return;
            };
            if request.status.get() != CLOSING && request.status.get() != CLOSED {
                // If the shell aborts during a replay, that's not a fatal error. Instead
                // we should be able to recover by client rendering all the root boundaries in
                // the ReplaySet.
                replay.pending_tasks.set(replay.pending_tasks.get() - 1.0);
                if replay.pending_tasks.get() == 0.0 && !replay.nodes.borrow().is_empty() {
                    let error_digest = log_recoverable_error(request, error, error_info, None);
                    abort_remaining_replay_nodes(
                        request,
                        None,
                        &replay.nodes,
                        &replay.slots.borrow(),
                        error,
                        error_digest,
                        error_info,
                        true,
                    );
                }
                request
                    .pending_root_tasks
                    .set(request.pending_root_tasks.get() - 1.0);
                if request.pending_root_tasks.get() == 0.0 {
                    complete_shell(request);
                }
            }
        }
        Some(boundary) => {
            // We construct an errorInfo from the boundary's componentStack so the error in dev will indicate which
            // boundary the message is referring to
            let tracked_postpones = request.tracked_postpones.borrow().clone();
            if boundary.status.get() != CLIENT_RENDERED {
                if !is_recoverable_reason
                    && let Some(tracked_postpones) = &tracked_postpones
                    && let Some(segment) = &segment
                {
                    // We are aborting a prerender and must halt this boundary.
                    // We treat this like other postpones during prerendering
                    log_recoverable_error(request, error, error_info, task.debug_task.get());
                    track_postpone(request, tracked_postpones, task, segment);
                    // If this boundary was still pending then we haven't already cancelled its fallbacks.
                    // We'll need to abort the fallbacks, which will also error that parent boundary.
                    for fallback_task in boundary.fallback_abortable_tasks.borrow().iter() {
                        finish_aborted_task(fallback_task, request, error);
                    }
                    boundary.fallback_abortable_tasks.borrow_mut().clear();
                    return finished_task(
                        request,
                        Some(boundary),
                        task.row.borrow().as_ref(),
                        Some(segment),
                    );
                }
                boundary.status.set(CLIENT_RENDERED);
                // We are aborting a render or resume which should put boundaries
                // into an explicitly client rendered state
                let error_digest =
                    log_recoverable_error(request, error, error_info, task.debug_task.get());
                encode_error_for_boundary(boundary, error_digest, error, error_info, true);

                untrack_boundary(request, boundary);

                if boundary.parent_flushed.get() {
                    request
                        .client_rendered_boundaries
                        .borrow_mut()
                        .push(Rc::clone(boundary));
                }
            }

            boundary
                .pending_tasks
                .set(boundary.pending_tasks.get() - 1.0);

            let boundary_row = boundary.row.borrow().clone();
            if let Some(boundary_row) = boundary_row {
                // Unblock the SuspenseListRow that was blocked by this boundary.
                boundary_row
                    .pending_tasks
                    .set(boundary_row.pending_tasks.get() - 1.0);
                if boundary_row.pending_tasks.get() == 0.0 {
                    finish_suspense_list_row(request, &boundary_row);
                }
            }

            // If this boundary was still pending then we haven't already cancelled its fallbacks.
            // We'll need to abort the fallbacks, which will also error that parent boundary.
            for fallback_task in boundary.fallback_abortable_tasks.borrow().iter() {
                finish_aborted_task(fallback_task, request, error);
            }
            boundary.fallback_abortable_tasks.borrow_mut().clear();
        }
    }

    let row = task.row.borrow().clone();
    if let Some(row) = row {
        row.pending_tasks.set(row.pending_tasks.get() - 1.0);
        if row.pending_tasks.get() == 0.0 {
            finish_suspense_list_row(request, &row);
        }
    }

    request
        .all_pending_tasks
        .set(request.all_pending_tasks.get() - 1.0);
    if request.all_pending_tasks.get() == 0.0 {
        complete_all(request);
    }
}

#[cfg_attr(rust_js, rust_js::name = "finishAbortedTaskDEV")]
fn finish_aborted_task_dev(
    task: &Rc<Task>,
    request: &Rc<Request>,
    error: Option<&'static Unknown>,
) {
    if __DEV__ {
        let prev_task_in_dev = CURRENT_TASK_IN_DEV.with_borrow(Clone::clone);
        let prev_get_current_stack_impl = REACT_SHARED_INTERNALS.get_current_stack();
        set_current_task_in_dev(Some(Rc::clone(task)));
        REACT_SHARED_INTERNALS.set_get_current_stack(Some(&get_current_stack_in_dev));
        let result = catch_unwind(AssertUnwindSafe(|| {
            finish_aborted_task(task, request, error);
        }));
        set_current_task_in_dev(prev_task_in_dev);
        REACT_SHARED_INTERNALS.set_get_current_stack(prev_get_current_stack_impl);
        if let Err(thrown_value) = result {
            resume_unwind(thrown_value);
        }
    } else {
        // These errors should never make it into a build so we don't need to encode them in codes.json
        // eslint-disable-next-line react-internal/prod-error-codes
        panic!(
            "finishAbortedTaskDEV should never be called in production mode. This is a bug in React."
        );
    }
}

#[cfg_attr(rust_js, rust_js::name = "abortTaskDEV")]
fn abort_task_dev(task: &Rc<Task>, request: &Rc<Request>) {
    if __DEV__ {
        let prev_task_in_dev = CURRENT_TASK_IN_DEV.with_borrow(Clone::clone);
        let prev_get_current_stack_impl = REACT_SHARED_INTERNALS.get_current_stack();
        set_current_task_in_dev(Some(Rc::clone(task)));
        REACT_SHARED_INTERNALS.set_get_current_stack(Some(&get_current_stack_in_dev));
        let result = catch_unwind(AssertUnwindSafe(|| {
            abort_task(task, request);
        }));
        set_current_task_in_dev(prev_task_in_dev);
        REACT_SHARED_INTERNALS.set_get_current_stack(prev_get_current_stack_impl);
        if let Err(thrown_value) = result {
            resume_unwind(thrown_value);
        }
    } else {
        // These errors should never make it into a build so we don't need to encode them in codes.json
        // eslint-disable-next-line react-internal/prod-error-codes
        panic!("abortTaskDEV should never be called in production mode. This is a bug in React.");
    }
}

fn abort_unwound_task(task: &Rc<Task>, request: &Rc<Request>) {
    // This task was rendering when abort began, so the synchronous abort sweep
    // left it alone. It has now unwound from user code and can be completed
    // through the normal abort path.
    if __DEV__ {
        abort_task_dev(task, request);
    } else {
        abort_task(task, request);
    }
    let abort_set = Rc::clone(&task.abort_set.borrow());
    abort_set.borrow_mut().remove(task);
    if __DEV__ {
        finish_aborted_task_dev(task, request, request.fatal_error.get());
    } else {
        finish_aborted_task(task, request, request.fatal_error.get());
    }
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#emitEarlyPreloads"
)]
fn emit_early_preloads(
    render_state: &RenderState,
    resumable_state: &ResumableState,
    shell_complete: bool,
) {
    let _ = (render_state, resumable_state, shell_complete);
    unreachable!()
}

fn safely_emit_early_preloads(request: &Rc<Request>, shell_complete: bool) {
    if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
        emit_early_preloads(
            request.render_state,
            request.resumable_state,
            shell_complete,
        );
    })) {
        // We assume preloads are optimistic and thus non-fatal if errored.
        let error_info: &'static ThrownInfo =
            unsafe { js::cast(Some(js::unknown(js::object::spread(None::<&Unknown>)))) };
        log_recoverable_error(request, thrown_value_of(&*error), error_info, None);
    }
}

// I extracted this function out because we want to ensure we consistently emit preloads before
// transitioning to the next request stage and this transition can happen in multiple places in this
// implementation.
fn complete_shell(request: &Rc<Request>) {
    if request.tracked_postpones.borrow().is_none() {
        // We only emit early preloads on shell completion for renders. For prerenders
        // we wait for the entire Request to finish because we are not responding to a
        // live request and can wait for as much data as possible.

        // we should only be calling completeShell when the shell is complete so we
        // just use a literal here
        let shell_complete = true;
        safely_emit_early_preloads(request, shell_complete);
    }
    if request.tracked_postpones.borrow().is_none() {
        // When the shell is complete it will be possible to flush. We attempt to prepre
        // the Preamble here in case it is ready for flushing.
        // We exclude prerenders because these cannot flush until after completeAll has been called
        prepare_preamble(request);
    }

    let on_shell_ready = &request.on_shell_ready;
    on_shell_ready();
}

// I extracted this function out because we want to ensure we consistently emit preloads before
// transitioning to the next request stage and this transition can happen in multiple places in this
// implementation.
fn complete_all(request: &Rc<Request>) {
    // During a render the shell must be complete if the entire request is finished
    // however during a Prerender it is possible that the shell is incomplete because
    // it postponed. We cannot use rootPendingTasks in the prerender case because
    // those hit zero even when the shell postpones. Instead we look at the completedRootSegment
    let shell_complete = if request.tracked_postpones.borrow().is_none() {
        // Render, we assume it is completed
        true
    } else {
        // Prerender Request, we use the state of the root segment
        request
            .completed_root_segment
            .borrow()
            .as_ref()
            .is_none_or(|completed_root_segment| completed_root_segment.status.get() != POSTPONED)
    };
    safely_emit_early_preloads(request, shell_complete);

    // When the shell is complete it will be possible to flush. We attempt to prepre
    // the Preamble here in case it is ready for flushing
    prepare_preamble(request);

    let on_all_ready = &request.on_all_ready;
    on_all_ready();
}

fn queue_completed_segment(boundary: &SuspenseBoundary, segment: &Rc<Segment>) {
    let only_child = {
        let children = segment.children.borrow();
        (segment.chunks.borrow().is_empty()
            && children.len() == 1
            && children[0].boundary.borrow().is_none()
            && children[0].id.get() == -1.0)
            .then(|| Rc::clone(&children[0]))
    };
    if let Some(child_segment) = only_child {
        // This is an empty segment. There's nothing to write, so we can instead transfer the ID
        // to the child. That way any existing references point to the child.
        child_segment.id.set(segment.id.get());
        child_segment.parent_flushed.set(true);
        if child_segment.status.get() == COMPLETED
            || child_segment.status.get() == ABORTED
            || child_segment.status.get() == ERRORED
        {
            queue_completed_segment(boundary, &child_segment);
        }
    } else {
        let completed_segments = &boundary.completed_segments;
        completed_segments.borrow_mut().push(Rc::clone(segment));
    }
}

fn finished_segment(request: &Request, boundary: Option<&Rc<SuspenseBoundary>>, segment: &Segment) {
    // $FlowFixMe[invalid-compare]
    if let Some(byte_length_of_chunk) = BYTE_LENGTH_OF_CHUNK {
        // Count the bytes of all the chunks of this segment.
        let chunks = &segment.chunks;
        let mut segment_byte_size = 0.0;
        for &chunk in chunks.borrow().iter() {
            segment_byte_size += byte_length_of_chunk(chunk);
        }
        // Accumulate on the parent boundary to power heuristics.
        match boundary {
            None => request
                .byte_size
                .set(request.byte_size.get() + segment_byte_size),
            Some(boundary) => boundary
                .byte_size
                .set(boundary.byte_size.get() + segment_byte_size),
        }
    }
}

fn finished_task(
    request: &Rc<Request>,
    boundary: Option<&Rc<SuspenseBoundary>>,
    row: Option<&Rc<SuspenseListRow>>,
    segment: Option<&Rc<Segment>>,
) {
    if let Some(row) = row {
        row.pending_tasks.set(row.pending_tasks.get() - 1.0);
        if row.pending_tasks.get() == 0.0 {
            finish_suspense_list_row(request, row);
        } else if row.together.get() {
            try_to_resolve_together_row(request, row);
        }
    }
    request
        .all_pending_tasks
        .set(request.all_pending_tasks.get() - 1.0);
    match boundary {
        None => {
            if let Some(segment) = segment
                && segment.parent_flushed.get()
            {
                if request.completed_root_segment.borrow().is_some() {
                    panic!("There can only be one root segment. This is a bug in React.");
                }

                request
                    .completed_root_segment
                    .replace(Some(Rc::clone(segment)));
            }
            request
                .pending_root_tasks
                .set(request.pending_root_tasks.get() - 1.0);
            if request.pending_root_tasks.get() == 0.0 {
                complete_shell(request);
            }
        }
        Some(boundary) => {
            boundary
                .pending_tasks
                .set(boundary.pending_tasks.get() - 1.0);
            if boundary.status.get() == CLIENT_RENDERED {
                // This already errored.
            } else if boundary.pending_tasks.get() == 0.0 {
                if boundary.status.get() == PENDING {
                    boundary.status.set(COMPLETED);
                }
                // This must have been the last segment we were waiting on. This boundary is now complete.
                if let Some(segment) = segment
                    && segment.parent_flushed.get()
                {
                    // Our parent segment already flushed, so we need to schedule this segment to be emitted.
                    // If it is a segment that was aborted, we'll write other content instead so we don't need
                    // to emit it.
                    if segment.status.get() == COMPLETED || segment.status.get() == ABORTED {
                        queue_completed_segment(boundary, segment);
                    }
                }
                if boundary.parent_flushed.get() {
                    // The segment might be part of a segment that didn't flush yet, but if the boundary's
                    // parent flushed, we need to schedule the boundary to be emitted.
                    if boundary.status.get() == POSTPONED {
                        // All tasks finished, but postponed holes still block revealing this boundary.
                        request
                            .partial_boundaries
                            .borrow_mut()
                            .push(Rc::clone(boundary));
                    } else {
                        request
                            .completed_boundaries
                            .borrow_mut()
                            .push(Rc::clone(boundary));
                    }
                }

                // We can now cancel any pending task on the fallback since we won't need to show it anymore.
                // This needs to happen after we read the parentFlushed flags because aborting can finish
                // work which can trigger user code, which can start flushing, which can change those flags.
                // If the boundary was POSTPONED, we still need to finish the fallback first.
                // If the boundary is eligible to be outlined during flushing we can't cancel the fallback
                // since we might need it when it's being outlined.
                if boundary.status.get() == COMPLETED {
                    let boundary_row = boundary.row.borrow().clone();
                    if let Some(boundary_row) = &boundary_row {
                        // Hoist the HoistableState from the boundary to the row so that the next rows
                        // can depend on the same dependencies.
                        hoist_hoistables(boundary_row.hoistables, boundary.content_state);
                    }
                    if !is_eligible_for_outlining(request, boundary) {
                        // abortTaskSoft (below) and finishSuspenseListRow → unblockSuspenseListRow
                        // → finishedTask (further below) both reenter finishedTask and decrement
                        // allPendingTasks. Pin the counter above zero for the duration of these
                        // fan-outs so a nested finishedTask can't observe 0 and call completeAll
                        // before this outer call reaches its own zero check.
                        request
                            .all_pending_tasks
                            .set(request.all_pending_tasks.get() + 1.0);
                        for fallback_task in boundary.fallback_abortable_tasks.borrow().iter() {
                            abort_task_soft(request, fallback_task);
                        }
                        boundary.fallback_abortable_tasks.borrow_mut().clear();
                        if let Some(boundary_row) = &boundary_row {
                            // If we aren't eligible for outlining, we don't have to wait until we flush it.
                            boundary_row
                                .pending_tasks
                                .set(boundary_row.pending_tasks.get() - 1.0);
                            if boundary_row.pending_tasks.get() == 0.0 {
                                finish_suspense_list_row(request, boundary_row);
                            }
                        }
                        request
                            .all_pending_tasks
                            .set(request.all_pending_tasks.get() - 1.0);
                    }

                    if request.pending_root_tasks.get() == 0.0
                        && request.tracked_postpones.borrow().is_none()
                        && boundary.preamble.is_some()
                    {
                        // The root is complete and this boundary may contribute part of the preamble.
                        // We eagerly attempt to prepare the preamble here because we expect most requests
                        // to have few boundaries which contribute preambles and it allow us to do this
                        // preparation work during the work phase rather than the when flushing.
                        prepare_preamble(request);
                    }
                } else if boundary.status.get() == POSTPONED {
                    let boundary_row = boundary.row.borrow().clone();
                    if let Some(boundary_row) = boundary_row {
                        let tracked_postpones = request.tracked_postpones.borrow().clone();
                        if let Some(tracked_postpones) = tracked_postpones {
                            // If this boundary is postponed, then we need to also postpone any blocked boundaries
                            // in the next row.
                            track_postponed_suspense_list_row(
                                request,
                                &tracked_postpones,
                                boundary_row.next.borrow().as_ref(),
                            );
                        }
                        // finishSuspenseListRow → unblockSuspenseListRow → finishedTask reenters
                        // and decrements allPendingTasks. Pin the counter above zero so those
                        // nested calls can't trip completeAll before this outer frame's own
                        // zero check at the end.
                        request
                            .all_pending_tasks
                            .set(request.all_pending_tasks.get() + 1.0);
                        boundary_row
                            .pending_tasks
                            .set(boundary_row.pending_tasks.get() - 1.0);
                        if boundary_row.pending_tasks.get() == 0.0 {
                            // This is really unnecessary since we've already postponed the boundaries but
                            // for pairity with other track+finish paths. We might end up using the hoisting.
                            finish_suspense_list_row(request, &boundary_row);
                        }
                        request
                            .all_pending_tasks
                            .set(request.all_pending_tasks.get() - 1.0);
                    }
                }
            } else {
                if let Some(segment) = segment
                    && segment.parent_flushed.get()
                {
                    // Our parent already flushed, so we need to schedule this segment to be emitted.
                    // If it is a segment that was aborted, we'll write other content instead so we don't need
                    // to emit it.
                    if segment.status.get() == COMPLETED || segment.status.get() == ABORTED {
                        queue_completed_segment(boundary, segment);
                        let completed_segments = &boundary.completed_segments;
                        if completed_segments.borrow().len() == 1 {
                            // This is the first time since we last flushed that we completed anything.
                            // We can schedule this boundary to emit its partially completed segments early
                            // in case the parent has already been flushed.
                            if boundary.parent_flushed.get() {
                                request
                                    .partial_boundaries
                                    .borrow_mut()
                                    .push(Rc::clone(boundary));
                            }
                        }
                    }
                }
                let boundary_row = boundary.row.borrow().clone();
                if let Some(boundary_row) = boundary_row
                    && boundary_row.together.get()
                {
                    try_to_resolve_together_row(request, &boundary_row);
                }
            }
        }
    }

    if request.all_pending_tasks.get() == 0.0 {
        complete_all(request);
    }
}

fn retry_task(request: &Rc<Request>, task: &Rc<Task>) {
    let segment = task.blocked_segment.borrow().clone();
    match segment {
        None => {
            retry_replay_task(
                request, // $FlowFixMe[incompatible-type]: Refined.
                task,
            );
        }
        Some(segment) => {
            retry_render_task(
                request, // $FlowFixMe[incompatible-type]: Refined.
                task, &segment,
            );
        }
    }
}

fn retry_render_task(request: &Rc<Request>, task: &Rc<Task>, segment: &Rc<Segment>) {
    if segment.status.get() != PENDING {
        // We completed this by other means before we had a chance to retry it.
        return;
    }

    let prev_task = request.current_task.replace(Some(Rc::clone(task)));

    // We restore the context to what it was when we suspended.
    // We don't restore it after we leave because it's likely that we'll end up
    // needing a very similar context soon again.
    switch_context(task.context.borrow().clone());
    let mut prev_task_in_dev = None;
    if __DEV__ {
        prev_task_in_dev = CURRENT_TASK_IN_DEV.with_borrow(Clone::clone);
        set_current_task_in_dev(Some(Rc::clone(task)));
    }

    let children_length = segment.children.borrow().len();
    let chunk_length = segment.chunks.borrow().len();
    // Used to detect forward progress if we hit a stack overflow below.
    let start_node = task.node.get();
    let result = catch_unwind(AssertUnwindSafe(|| {
        // We call the destructive form that mutates this task. That way if something
        // suspends again, we can reuse the same task instead of spawning a new one.

        retry_node(request, task);
        push_segment_finale(
            &segment.chunks,
            request.render_state,
            segment.last_pushed_text.get(),
            segment.text_embedded.get(),
        );

        task.abort_set.borrow().borrow_mut().remove(task);
        segment.status.set(COMPLETED);
        finished_segment(request, task.blocked_boundary.borrow().as_ref(), segment);
        finished_task(
            request,
            task.blocked_boundary.borrow().as_ref(),
            task.row.borrow().as_ref(),
            Some(segment),
        );
    }));
    let result = match result {
        Ok(()) => Ok(()),
        Err(thrown_value) => catch_unwind(AssertUnwindSafe(|| {
            reset_hooks_state();

            // Reset the write pointers to where we started.
            segment.children.borrow_mut().truncate(children_length);
            segment.chunks.borrow_mut().truncate(chunk_length);

            let is_suspense_exception = is_suspense_exception(thrown_value_of(&*thrown_value));
            let x = if is_suspense_exception {
                // This is a special type of exception used for Suspense. For historical
                // reasons, the rest of the Suspense implementation expects the thrown
                // value to be a thenable, because before `use` existed that was the
                // (unstable) API for suspending. This implementation detail can change
                // later, once we deprecate the old API in favor of `use`.
                Some(js::unknown(get_suspended_thenable()))
            } else {
                thrown_value_of(&*thrown_value)
            };

            if request.aborted.get() {
                if is_suspense_exception {
                    // This task was rendering when abort() was called, so it never took
                    // the normal suspension path below that stores the thenable state.
                    // Preserve it before finishing the abort so DEV can replay the task
                    // and include this suspended use() call site in the owner stack.
                    task.thenable_state
                        .replace(get_thenable_state_after_suspending());
                }
                // The task has unwound from user code, so it must no longer appear to
                // be the currently rendering task while we synchronously finish it.
                // Restore the parent instead of clearing this field because finishing
                // can reenter Fizz and abort an outer render that is still on the stack.
                request.current_task.replace(prev_task.clone());
                abort_unwound_task(task, request);
                return;
            }

            if js::type_of(x) == "object" && !matches!(js::classify(x), Kind::Null) {
                // $FlowFixMe[method-unbinding]
                if js::type_of(js::get(x, "then")) == "function" {
                    // Something suspended again, let's pick it back up later.
                    segment.status.set(PENDING);
                    task.thenable_state.replace(if is_suspense_exception {
                        get_thenable_state_after_suspending()
                    } else {
                        None
                    });
                    // We've asserted that x is a thenable above
                    let thenable: &'static Thenable = unsafe { js::cast(x) };
                    let (resolve, reject) = ping_of(request, task);
                    thenable.then(resolve, reject);
                    return;
                }
                if js::strict_equals(
                    js::get(x, "message"),
                    Some(js::unknown("Maximum call stack size exceeded")),
                ) && !js::strict_equals(task.node.get(), start_node)
                {
                    segment.status.set(PENDING);
                    task.thenable_state.replace(None);
                    // Immediately schedule the task for retrying.
                    request.pinged_tasks.borrow_mut().push(Rc::clone(task));
                    return;
                }
            }

            let error_info = get_thrown_info(task.component_stack.borrow().clone());
            task.abort_set.borrow().borrow_mut().remove(task);

            segment.status.set(ERRORED);
            errored_task(
                request,
                task.blocked_boundary.borrow().as_ref(),
                task.row.borrow().as_ref(),
                x,
                error_info,
                if __DEV__ { task.debug_task.get() } else { None },
            );
        })),
    };
    request.current_task.replace(prev_task);
    if __DEV__ {
        set_current_task_in_dev(prev_task_in_dev);
    }
    if let Err(thrown_value) = result {
        resume_unwind(thrown_value);
    }
}

fn retry_replay_task(request: &Rc<Request>, task: &Rc<Task>) {
    let replay = task.replay.borrow().clone().unwrap();
    if replay.pending_tasks.get() == 0.0 {
        // There are no pending tasks working on this set, so we must have aborted.
        return;
    }

    let prev_task = request.current_task.replace(Some(Rc::clone(task)));

    // We restore the context to what it was when we suspended.
    // We don't restore it after we leave because it's likely that we'll end up
    // needing a very similar context soon again.
    switch_context(task.context.borrow().clone());
    let mut prev_task_in_dev = None;
    if __DEV__ {
        prev_task_in_dev = CURRENT_TASK_IN_DEV.with_borrow(Clone::clone);
        set_current_task_in_dev(Some(Rc::clone(task)));
    }

    // Used to detect forward progress if we hit a stack overflow below.
    let start_node = task.node.get();
    let result = catch_unwind(AssertUnwindSafe(|| {
        // We call the destructive form that mutates this task. That way if something
        // suspends again, we can reuse the same task instead of spawning a new one.
        let slots = replay.slots.borrow().clone();
        if let ResumeSlots::Root(resume_segment_id) = slots {
            resume_node(
                request,
                task,
                resume_segment_id,
                task.node.get(),
                task.child_index.get(),
            );
        } else {
            retry_node(request, task);
        }

        if replay.pending_tasks.get() == 1.0 && !replay.nodes.borrow().is_empty() {
            panic!(concat!(
                "Couldn't find all resumable slots by key/index during replaying. ",
                "The tree doesn't match so React will fallback to client rendering."
            ));
        }
        replay.pending_tasks.set(replay.pending_tasks.get() - 1.0);

        task.abort_set.borrow().borrow_mut().remove(task);
        finished_task(
            request,
            task.blocked_boundary.borrow().as_ref(),
            task.row.borrow().as_ref(),
            None,
        );
    }));
    let result = match result {
        Ok(()) => Ok(()),
        Err(thrown_value) => catch_unwind(AssertUnwindSafe(|| {
            reset_hooks_state();

            let is_suspense_exception = is_suspense_exception(thrown_value_of(&*thrown_value));
            let x = if is_suspense_exception {
                // This is a special type of exception used for Suspense. For historical
                // reasons, the rest of the Suspense implementation expects the thrown
                // value to be a thenable, because before `use` existed that was the
                // (unstable) API for suspending. This implementation detail can change
                // later, once we deprecate the old API in favor of `use`.
                Some(js::unknown(get_suspended_thenable()))
            } else {
                thrown_value_of(&*thrown_value)
            };

            if request.aborted.get() {
                if is_suspense_exception {
                    // This task was rendering when abort() was called, so it never took
                    // the normal suspension path below that stores the thenable state.
                    // Preserve it before finishing the abort so DEV can replay the task
                    // and include this suspended use() call site in the owner stack.
                    task.thenable_state
                        .replace(get_thenable_state_after_suspending());
                }
                // The task has unwound from user code, so it must no longer appear to
                // be the currently rendering task while we synchronously finish it.
                // Restore the parent instead of clearing this field because finishing
                // can reenter Fizz and abort an outer render that is still on the stack.
                request.current_task.replace(prev_task.clone());
                abort_unwound_task(task, request);
                return;
            }

            // $FlowFixMe[invalid-compare]
            if js::type_of(x) == "object" && !matches!(js::classify(x), Kind::Null) {
                // $FlowFixMe[method-unbinding]
                if js::type_of(js::get(x, "then")) == "function" {
                    // Something suspended again, let's pick it back up later.
                    let thenable: &'static Thenable = unsafe { js::cast(x) };
                    let (resolve, reject) = ping_of(request, task);
                    thenable.then(resolve, reject);
                    task.thenable_state.replace(if is_suspense_exception {
                        get_thenable_state_after_suspending()
                    } else {
                        None
                    });
                    return;
                }
                if js::strict_equals(
                    js::get(x, "message"),
                    Some(js::unknown("Maximum call stack size exceeded")),
                ) && !js::strict_equals(task.node.get(), start_node)
                {
                    // Stack overflow after making forward progress. Retry from a fresh stack.
                    // No progress (e.g. overflow inside the component itself) falls through.
                    task.thenable_state.replace(None);
                    // Immediately schedule the task for retrying.
                    request.pinged_tasks.borrow_mut().push(Rc::clone(task));
                    return;
                }
            }
            replay.pending_tasks.set(replay.pending_tasks.get() - 1.0);
            task.abort_set.borrow().borrow_mut().remove(task);
            let error_info = get_thrown_info(task.component_stack.borrow().clone());
            errored_replay(
                request,
                task.blocked_boundary.borrow().clone(),
                if request.aborted.get() {
                    request.fatal_error.get()
                } else {
                    x
                },
                error_info,
                &replay.nodes,
                &replay.slots.borrow(),
                if __DEV__ { task.debug_task.get() } else { None },
            );
            request
                .pending_root_tasks
                .set(request.pending_root_tasks.get() - 1.0);
            if request.pending_root_tasks.get() == 0.0 {
                complete_shell(request);
            }
            request
                .all_pending_tasks
                .set(request.all_pending_tasks.get() - 1.0);
            if request.all_pending_tasks.get() == 0.0 {
                complete_all(request);
            }
        })),
    };
    request.current_task.replace(prev_task);
    if __DEV__ {
        set_current_task_in_dev(prev_task_in_dev);
    }
    if let Err(thrown_value) = result {
        resume_unwind(thrown_value);
    }
}

pub fn perform_work(request: &Rc<Request>) {
    if request.aborted.get() || request.status.get() > OPEN {
        return;
    }
    let prev_context = get_active_context();
    let prev_dispatcher = REACT_SHARED_INTERNALS.h();
    HOOKS_DISPATCHER.with(|hooks_dispatcher| REACT_SHARED_INTERNALS.set_h(Some(hooks_dispatcher)));
    let prev_async_dispatcher = REACT_SHARED_INTERNALS.a();
    DEFAULT_ASYNC_DISPATCHER.with(|default_async_dispatcher| {
        REACT_SHARED_INTERNALS.set_a(Some(default_async_dispatcher))
    });

    let prev_request = CURRENT_REQUEST.replace(Some(Rc::clone(request)));

    let mut prev_get_current_stack_impl = None;
    if __DEV__ {
        prev_get_current_stack_impl = REACT_SHARED_INTERNALS.get_current_stack();
        REACT_SHARED_INTERNALS.set_get_current_stack(Some(&get_current_stack_in_dev));
    }
    let prev_resumable_state = CURRENT_RESUMABLE_STATE.get();
    set_current_resumable_state(Some(request.resumable_state));
    let result = catch_unwind(AssertUnwindSafe(|| {
        let pinged_tasks = &request.pinged_tasks;
        let mut i = 0;
        while i < pinged_tasks.borrow().len() {
            let task = Rc::clone(&pinged_tasks.borrow()[i]);
            retry_task(request, &task);
            i += 1;
        }
        pinged_tasks.borrow_mut().drain(0..i);
        if let Some(destination) = request.destination.get() {
            flush_completed_queues(request, destination);
        }
    }));
    let result = match result {
        Ok(()) => Ok(()),
        Err(error) => catch_unwind(AssertUnwindSafe(|| {
            let error_info: &'static ThrownInfo =
                unsafe { js::cast(Some(js::unknown(js::object::spread(None::<&Unknown>)))) };
            log_recoverable_error(request, thrown_value_of(&*error), error_info, None);
            fatal_error(request, thrown_value_of(&*error), error_info, None);
        })),
    };
    set_current_resumable_state(prev_resumable_state);

    REACT_SHARED_INTERNALS.set_h(prev_dispatcher);
    REACT_SHARED_INTERNALS.set_a(prev_async_dispatcher);

    if __DEV__ {
        REACT_SHARED_INTERNALS.set_get_current_stack(prev_get_current_stack_impl);
    }
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
    if let Err(thrown_value) = result {
        resume_unwind(thrown_value);
    }
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#writePlaceholder"
)]
fn write_placeholder(destination: &Destination, render_state: &RenderState, id: f64) -> bool {
    let _ = (destination, render_state, id);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#writeStartCompletedSuspenseBoundary"
)]
fn write_start_completed_suspense_boundary(
    destination: &Destination,
    render_state: &RenderState,
) -> bool {
    let _ = (destination, render_state);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#writeStartPendingSuspenseBoundary"
)]
fn write_start_pending_suspense_boundary(
    destination: &Destination,
    render_state: &RenderState,
    id: Option<f64>,
) -> bool {
    let _ = (destination, render_state, id);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#writeStartClientRenderedSuspenseBoundary"
)]
fn write_start_client_rendered_suspense_boundary(
    destination: &Destination,
    render_state: &RenderState,
    error_digest: Option<&str>,
    error_message: Option<&str>,
    error_stack: Option<&str>,
    error_component_stack: Option<&str>,
) -> bool {
    let _ = (
        destination,
        render_state,
        error_digest,
        error_message,
        error_stack,
        error_component_stack,
    );
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#writeEndCompletedSuspenseBoundary"
)]
fn write_end_completed_suspense_boundary(
    destination: &Destination,
    render_state: &RenderState,
) -> bool {
    let _ = (destination, render_state);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#writeEndPendingSuspenseBoundary"
)]
fn write_end_pending_suspense_boundary(
    destination: &Destination,
    render_state: &RenderState,
) -> bool {
    let _ = (destination, render_state);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#writeEndClientRenderedSuspenseBoundary"
)]
fn write_end_client_rendered_suspense_boundary(
    destination: &Destination,
    render_state: &RenderState,
) -> bool {
    let _ = (destination, render_state);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#writeStartSegment"
)]
fn write_start_segment(
    destination: &Destination,
    render_state: &RenderState,
    format_context: &FormatContext,
    id: f64,
) -> bool {
    let _ = (destination, render_state, format_context, id);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#writeEndSegment"
)]
fn write_end_segment(destination: &Destination, format_context: &FormatContext) -> bool {
    let _ = (destination, format_context);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#writeCompletedSegmentInstruction"
)]
fn write_completed_segment_instruction(
    destination: &Destination,
    resumable_state: &ResumableState,
    render_state: &RenderState,
    content_segment_id: f64,
) -> bool {
    let _ = (
        destination,
        resumable_state,
        render_state,
        content_segment_id,
    );
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#writeCompletedBoundaryInstruction"
)]
fn write_completed_boundary_instruction(
    destination: &Destination,
    resumable_state: &ResumableState,
    render_state: &RenderState,
    id: f64,
    hoistable_state: &HoistableState,
) -> bool {
    let _ = (
        destination,
        resumable_state,
        render_state,
        id,
        hoistable_state,
    );
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#writeClientRenderBoundaryInstruction"
)]
fn write_client_render_boundary_instruction(
    destination: &Destination,
    resumable_state: &ResumableState,
    render_state: &RenderState,
    id: f64,
    error_digest: Option<&str>,
    error_message: Option<&str>,
    error_stack: Option<&str>,
    error_component_stack: Option<&str>,
) -> bool {
    let _ = (
        destination,
        resumable_state,
        render_state,
        id,
        error_digest,
        error_message,
        error_stack,
        error_component_stack,
    );
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#writeHoistablesForBoundary"
)]
fn write_hoistables_for_boundary(
    destination: &Destination,
    hoistable_state: &HoistableState,
    render_state: &RenderState,
) -> bool {
    let _ = (destination, hoistable_state, render_state);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#writePreambleStart"
)]
fn write_preamble_start(
    destination: &Destination,
    resumable_state: &ResumableState,
    render_state: &RenderState,
    skip_blocking_shell: bool,
) {
    let _ = (
        destination,
        resumable_state,
        render_state,
        skip_blocking_shell,
    );
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#writePreambleEnd"
)]
fn write_preamble_end(destination: &Destination, render_state: &RenderState) {
    let _ = (destination, render_state);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#hoistPreambleState"
)]
fn hoist_preamble_state(render_state: &RenderState, preamble_state: &PreambleState) {
    let _ = (render_state, preamble_state);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#isPreambleReady"
)]
fn is_preamble_ready(render_state: &RenderState, has_pending_preambles: bool) -> bool {
    let _ = (render_state, has_pending_preambles);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#writeCompletedRoot"
)]
fn write_completed_root(
    destination: &Destination,
    resumable_state: &ResumableState,
    render_state: &RenderState,
    is_complete: bool,
) -> bool {
    let _ = (destination, resumable_state, render_state, is_complete);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#writeHoistables"
)]
fn write_hoistables(
    destination: &Destination,
    resumable_state: &ResumableState,
    render_state: &RenderState,
) {
    let _ = (destination, resumable_state, render_state);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#writePostamble"
)]
fn write_postamble(destination: &Destination, resumable_state: &ResumableState) {
    let _ = (destination, resumable_state);
    unreachable!()
}

fn prepare_preamble_from_subtree(
    request: &Request,
    segment: &Segment,
    collected_preamble_segments: &mut PreambleSegments,
) -> bool {
    if !segment.preamble_children.borrow().is_empty() {
        collected_preamble_segments.push(Rc::clone(&segment.preamble_children));
    }
    let mut pending_preambles = false;
    for next_segment in segment.children.borrow().iter() {
        pending_preambles =
            prepare_preamble_from_segment(request, next_segment, collected_preamble_segments)
                || pending_preambles;
    }
    pending_preambles
}

fn prepare_preamble_from_segment(
    request: &Request,
    segment: &Segment,
    collected_preamble_segments: &mut PreambleSegments,
) -> bool {
    let Some(boundary) = segment.boundary.borrow().clone() else {
        // This segment is not a boundary, let's check it's children
        return prepare_preamble_from_subtree(request, segment, collected_preamble_segments);
    };

    let Some(preamble) = &boundary.preamble else {
        // This boundary cannot have a preamble so it can't block the flushing of
        // the preamble.
        return false;
    };

    let status = boundary.status.get();

    match status {
        COMPLETED => {
            // This boundary is complete. It might have inner boundaries which are pending
            // and able to provide a preamble so we have to check it's children
            hoist_preamble_state(request.render_state, preamble.content);
            // We track this boundary's byteSize on the request since it will always flush with
            // the request since it may contribute to the preamble
            request
                .byte_size
                .set(request.byte_size.get() + boundary.byte_size.get());
            let Some(boundary_root_segment) = boundary.completed_segments.borrow().first().cloned()
            else {
                // Using the same error from flushSegment to avoid making a new one since conceptually the problem is still the same
                panic!(
                    "A previously unvisited boundary must have exactly one root segment. This is a bug in React."
                );
            };
            prepare_preamble_from_subtree(
                request,
                &boundary_root_segment,
                collected_preamble_segments,
            )
        }
        _ => {
            if status == POSTPONED {
                // This segment is postponed. When prerendering we consider this pending still because
                // it can resume. If we're rendering then this is equivalent to errored.
                if request.tracked_postpones.borrow().is_some() {
                    // This boundary won't contribute a preamble to the current prerender
                    return true;
                }
                // Expected fallthrough
            }
            if (status == POSTPONED || status == CLIENT_RENDERED)
                && segment.status.get() == COMPLETED
            {
                // This boundary is errored so if it contains a preamble we should include it
                hoist_preamble_state(request.render_state, preamble.fallback);
                return prepare_preamble_from_subtree(
                    request,
                    segment,
                    collected_preamble_segments,
                );
            }
            // Expected fallthrough
            // This boundary is still pending and might contain a preamble
            true
        }
    }
}

fn prepare_preamble(request: &Request) {
    let completed_root_segment = request.completed_root_segment.borrow().clone();
    if let Some(completed_root_segment) = completed_root_segment
        && request.completed_preamble_segments.borrow().is_none()
    {
        let mut collected_preamble_segments: PreambleSegments = vec![];
        let original_request_byte_size = request.byte_size.get();
        let has_pending_preambles = prepare_preamble_from_segment(
            request,
            &completed_root_segment,
            &mut collected_preamble_segments,
        );
        if is_preamble_ready(request.render_state, has_pending_preambles) {
            request
                .completed_preamble_segments
                .replace(Some(Rc::new(collected_preamble_segments)));
        } else {
            // We restore the original size since the preamble is not ready
            // and we will prepare it again.
            request.byte_size.set(original_request_byte_size);
        }
    }
}

fn flush_preamble(
    request: &Rc<Request>,
    destination: &Destination,
    _root_segment: &Segment,
    preamble_segments: &PreambleSegments,
    skip_blocking_shell: bool,
) {
    // The preamble is ready.
    write_preamble_start(
        destination,
        request.resumable_state,
        request.render_state,
        skip_blocking_shell,
    );
    for segments in preamble_segments {
        let mut j = 0;
        while j < segments.borrow().len() {
            let segment = Rc::clone(&segments.borrow()[j]);
            flush_segment(request, destination, &segment, None);
            j += 1;
        }
    }
    write_preamble_end(destination, request.render_state);
}

fn flush_subtree(
    request: &Rc<Request>,
    destination: &Destination,
    segment: &Segment,
    hoistable_state: Option<&'static HoistableState>,
) -> bool {
    segment.parent_flushed.set(true);
    match segment.status.get() {
        status @ (PENDING | POSTPONED) => {
            if status == PENDING {
                // We're emitting a placeholder for this segment to be filled in later.
                // Therefore we'll need to assign it an ID - to refer to it by.
                segment.id.set(next_segment_id(request));
                // Fallthrough
            }
            let segment_id = segment.id.get();
            // When this segment finally completes it won't be embedded in text since it will flush separately
            segment.last_pushed_text.set(false);
            segment.text_embedded.set(false);
            write_placeholder(destination, request.render_state, segment_id)
        }
        COMPLETED => {
            segment.status.set(FLUSHED);
            let mut r = true;
            let chunks = segment.chunks.borrow();
            let mut chunk_idx = 0;
            let children = segment.children.borrow();

            for next_child in children.iter() {
                // Write all the chunks up until the next child.
                while (chunk_idx as f64) < next_child.index {
                    write_chunk(destination, chunks[chunk_idx]);
                    chunk_idx += 1;
                }
                r = flush_segment(request, destination, next_child, hoistable_state);
            }
            // Finally just write all the remaining chunks
            while chunk_idx + 1 < chunks.len() {
                write_chunk(destination, chunks[chunk_idx]);
                chunk_idx += 1;
            }
            if chunk_idx < chunks.len() {
                r = write_chunk_and_return(destination, chunks[chunk_idx]);
            }
            r
        }
        ABORTED => true,
        _ => {
            panic!(
                "Aborted, errored or already flushed boundaries should not be flushed again. This is a bug in React."
            );
        }
    }
}

thread_local! {
    // Running count for how much bytes of boundaries have flushed inlined into the currently
    // flushing root or completed boundary.
    static FLUSHED_BYTE_SIZE: Cell<f64> = const { Cell::new(0.0) };
}

fn flush_segment(
    request: &Rc<Request>,
    destination: &Destination,
    segment: &Segment,
    hoistable_state: Option<&'static HoistableState>,
) -> bool {
    let Some(boundary) = segment.boundary.borrow().clone() else {
        // Not a suspense boundary.
        return flush_subtree(request, destination, segment, hoistable_state);
    };

    // We're going to write the boundary. We don't need to maintain this reference since
    // we might reflush this segment at a later time (if it aborts and we inlined) but
    // we don't want to reflush the boundary
    segment.boundary.replace(None);
    boundary.parent_flushed.set(true);
    // This segment is a Suspense boundary. We need to decide whether to
    // emit the content or the fallback now.
    if boundary.status.get() == CLIENT_RENDERED {
        // Emit a client rendered suspense boundary wrapper.
        // We never queue the inner boundary so we'll never emit its content or partial segments.

        let row = boundary.row.borrow().clone();
        if let Some(row) = row {
            // Since this boundary end up client rendered, we can unblock future suspense list rows.
            // This means that they may appear out of order if the future rows succeed but this is
            // a client rendered row.
            row.pending_tasks.set(row.pending_tasks.get() - 1.0);
            if row.pending_tasks.get() == 0.0 {
                finish_suspense_list_row(request, &row);
            }
        }

        // Each read before the config's JS runs (ADR 0863).
        let error_digest = boundary.error_digest.borrow().clone();
        if __DEV__ {
            let error_message = boundary.error_message.borrow().clone();
            let error_stack = boundary.error_stack.borrow().clone();
            let error_component_stack = boundary.error_component_stack.borrow().clone();
            write_start_client_rendered_suspense_boundary(
                destination,
                request.render_state,
                error_digest.as_deref(),
                error_message.as_deref(),
                error_stack.as_deref(),
                error_component_stack.as_deref(),
            );
        } else {
            write_start_client_rendered_suspense_boundary(
                destination,
                request.render_state,
                error_digest.as_deref(),
                js::null(),
                js::null(),
                js::null(),
            );
        }
        // Flush the fallback.
        flush_subtree(request, destination, segment, hoistable_state);

        write_end_client_rendered_suspense_boundary(destination, request.render_state)
    } else if boundary.status.get() != COMPLETED {
        if boundary.status.get() == PENDING {
            // For pending boundaries we lazily assign an ID to the boundary
            // and root segment.
            boundary.root_segment_id.set(next_segment_id(request));
        }

        if !boundary.completed_segments.borrow().is_empty() {
            // If this is at least partially complete, we can queue it to be partially emitted early.
            request
                .partial_boundaries
                .borrow_mut()
                .push(Rc::clone(&boundary));
        }

        // This boundary is still loading. Emit a pending suspense boundary wrapper.

        let id = boundary.root_segment_id.get();
        write_start_pending_suspense_boundary(destination, request.render_state, Some(id));

        if let Some(hoistable_state) = hoistable_state {
            hoist_hoistables(hoistable_state, boundary.fallback_state);
        }
        // Flush the fallback.
        flush_subtree(request, destination, segment, hoistable_state);

        write_end_pending_suspense_boundary(destination, request.render_state)
    } else if
    // We don't outline when we're emitting partially completed boundaries optimistically
    // because it doesn't make sense to outline something if its parent is going to be
    // blocked on something later in the stream anyway.
    !FLUSHING_PARTIAL_BOUNDARIES.get()
        && is_eligible_for_outlining(request, &boundary)
        && (FLUSHED_BYTE_SIZE.get() + boundary.byte_size.get() > request.progressive_chunk_size
            || has_suspensey_content(boundary.content_state, FLUSHING_SHELL.get())
            || boundary.defer)
    {
        // Inlining this boundary would make the current sequence being written too large
        // and block the parent for too long. Instead, it will be emitted separately so that we
        // can progressively show other content.
        // We add it to the queue during the flush because we have to ensure that
        // the parent flushes first so that there's something to inject it into.
        // We also have to make sure that it's emitted into the queue in a deterministic slot.
        // I.e. we can't insert it here when it completes.

        // Assign an ID to refer to the future content by.
        boundary.root_segment_id.set(next_segment_id(request));

        request
            .completed_boundaries
            .borrow_mut()
            .push(Rc::clone(&boundary));
        // Emit a pending rendered suspense boundary wrapper.
        write_start_pending_suspense_boundary(
            destination,
            request.render_state,
            Some(boundary.root_segment_id.get()),
        );

        // While we are going to flush the fallback we are going to follow it up with
        // the completed boundary immediately so we make the choice to omit fallback
        // boundary state from the parent since it will be replaced when the boundary
        // flushes later in this pass or in a future flush

        // Flush the fallback.
        flush_subtree(request, destination, segment, hoistable_state);

        write_end_pending_suspense_boundary(destination, request.render_state)
    } else {
        // We're inlining this boundary so its bytes get counted to the current running count.
        FLUSHED_BYTE_SIZE.set(FLUSHED_BYTE_SIZE.get() + boundary.byte_size.get());
        if let Some(hoistable_state) = hoistable_state {
            hoist_hoistables(hoistable_state, boundary.content_state);
        }

        let row = boundary.row.borrow().clone();
        if let Some(row) = row
            && is_eligible_for_outlining(request, &boundary)
        {
            // Once we have written the boundary, we can unblock the row and let future
            // rows be written. This may schedule new completed boundaries.
            row.pending_tasks.set(row.pending_tasks.get() - 1.0);
            if row.pending_tasks.get() == 0.0 {
                finish_suspense_list_row(request, &row);
            }
        }

        // We can inline this boundary's content as a complete boundary.
        write_start_completed_suspense_boundary(destination, request.render_state);

        let completed_segments = &boundary.completed_segments;

        if completed_segments.borrow().len() != 1 {
            panic!(
                "A previously unvisited boundary must have exactly one root segment. This is a bug in React."
            );
        }

        let content_segment = Rc::clone(&completed_segments.borrow()[0]);
        flush_segment(request, destination, &content_segment, hoistable_state);

        write_end_completed_suspense_boundary(destination, request.render_state)
    }
}

fn flush_client_rendered_boundary(
    request: &Request,
    destination: &Destination,
    boundary: &SuspenseBoundary,
) -> bool {
    // Each read before the config's JS runs (ADR 0863).
    let error_digest = boundary.error_digest.borrow().clone();
    if __DEV__ {
        let error_message = boundary.error_message.borrow().clone();
        let error_stack = boundary.error_stack.borrow().clone();
        let error_component_stack = boundary.error_component_stack.borrow().clone();
        write_client_render_boundary_instruction(
            destination,
            request.resumable_state,
            request.render_state,
            boundary.root_segment_id.get(),
            error_digest.as_deref(),
            error_message.as_deref(),
            error_stack.as_deref(),
            error_component_stack.as_deref(),
        )
    } else {
        write_client_render_boundary_instruction(
            destination,
            request.resumable_state,
            request.render_state,
            boundary.root_segment_id.get(),
            error_digest.as_deref(),
            js::null(),
            js::null(),
            js::null(),
        )
    }
}

fn flush_segment_container(
    request: &Rc<Request>,
    destination: &Destination,
    segment: &Segment,
    hoistable_state: &'static HoistableState,
) -> bool {
    write_start_segment(
        destination,
        request.render_state,
        segment.parent_format_context,
        segment.id.get(),
    );
    flush_segment(request, destination, segment, Some(hoistable_state));
    write_end_segment(destination, segment.parent_format_context)
}

fn flush_completed_boundary(
    request: &Rc<Request>,
    destination: &Destination,
    boundary: &SuspenseBoundary,
) -> bool {
    FLUSHED_BYTE_SIZE.set(boundary.byte_size.get()); // Start counting bytes
    let completed_segments = &boundary.completed_segments;
    let mut i = 0;
    while i < completed_segments.borrow().len() {
        let segment = Rc::clone(&completed_segments.borrow()[i]);
        flush_partially_completed_segment(request, destination, boundary, &segment);
        i += 1;
    }
    completed_segments.borrow_mut().clear();

    let row = boundary.row.borrow().clone();
    if let Some(row) = row
        && is_eligible_for_outlining(request, boundary)
    {
        // Once we have written the boundary, we can unblock the row and let future
        // rows be written. This may schedule new completed boundaries.
        row.pending_tasks.set(row.pending_tasks.get() - 1.0);
        if row.pending_tasks.get() == 0.0 {
            finish_suspense_list_row(request, &row);
        }
    }

    write_hoistables_for_boundary(destination, boundary.content_state, request.render_state);
    write_completed_boundary_instruction(
        destination,
        request.resumable_state,
        request.render_state,
        boundary.root_segment_id.get(),
        boundary.content_state,
    )
}

fn flush_partial_boundary(
    request: &Rc<Request>,
    destination: &Destination,
    boundary: &SuspenseBoundary,
) -> bool {
    FLUSHED_BYTE_SIZE.set(boundary.byte_size.get()); // Start counting bytes
    let completed_segments = &boundary.completed_segments;
    let mut i = 0;
    while i < completed_segments.borrow().len() {
        let segment = Rc::clone(&completed_segments.borrow()[i]);
        if !flush_partially_completed_segment(request, destination, boundary, &segment) {
            i += 1;
            completed_segments.borrow_mut().drain(0..i);
            // Only write as much as the buffer wants. Something higher priority
            // might want to write later.
            return false;
        }
        i += 1;
    }
    completed_segments.borrow_mut().drain(0..i);

    let row = boundary.row.borrow().clone();
    if let Some(row) = row
        && row.together.get()
        && boundary.pending_tasks.get() == 1.0
    {
        // "together" rows are blocked on their own boundaries.
        // We have now flushed all the boundary's segments as partials.
        // We can now unblock it from blocking the row that will eventually
        // unblock the boundary itself which can issue its complete instruction.
        // TODO: Ideally the complete instruction would be in a single <script> tag.
        if row.pending_tasks.get() == 1.0 {
            unblock_suspense_list_row(request, Some(Rc::clone(&row)), Some(row.hoistables));
        } else {
            row.pending_tasks.set(row.pending_tasks.get() - 1.0);
        }
    }

    write_hoistables_for_boundary(destination, boundary.content_state, request.render_state)
}

fn flush_partially_completed_segment(
    request: &Rc<Request>,
    destination: &Destination,
    boundary: &SuspenseBoundary,
    segment: &Segment,
) -> bool {
    if segment.status.get() == FLUSHED {
        // We've already flushed this inline
        return true;
    }

    let hoistable_state = boundary.content_state;

    let segment_id = segment.id.get();
    if segment_id == -1.0 {
        // This segment wasn't previously referred to. This happens at the root of
        // a boundary. We make kind of a leap here and assume this is the root.
        let root_segment_id = boundary.root_segment_id.get();
        segment.id.set(root_segment_id);

        if root_segment_id == -1.0 {
            panic!("A root segment ID must have been assigned by now. This is a bug in React.");
        }

        flush_segment_container(request, destination, segment, hoistable_state)
    } else if segment_id == boundary.root_segment_id.get() {
        // When we emit postponed boundaries, we might have assigned the ID already
        // but it's still the root segment so we can't inject it into the parent yet.
        flush_segment_container(request, destination, segment, hoistable_state)
    } else {
        flush_segment_container(request, destination, segment, hoistable_state);
        write_completed_segment_instruction(
            destination,
            request.resumable_state,
            request.render_state,
            segment_id,
        )
    }
}

thread_local! {
    static FLUSHING_PARTIAL_BOUNDARIES: Cell<bool> = const { Cell::new(false) };
    static FLUSHING_SHELL: Cell<bool> = const { Cell::new(false) };
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#resetResumableState"
)]
fn reset_resumable_state(resumable_state: &ResumableState, render_state: &RenderState) {
    let _ = (resumable_state, render_state);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFizzConfig.js#completeResumableState"
)]
fn complete_resumable_state(resumable_state: &ResumableState) {
    let _ = resumable_state;
    unreachable!()
}

fn new_thrown_info() -> &'static ThrownInfo {
    unsafe { js::cast(Some(js::unknown(js::object::spread(None::<&Unknown>)))) }
}

fn flush_completed_queues(request: &Rc<Request>, destination: &'static Destination) {
    begin_writing(destination);
    let result = catch_unwind(AssertUnwindSafe(|| {
        // The structure of this is to go through each queue one by one and write
        // until the sink tells us to stop. When we should stop, we still finish writing
        // that item fully and then yield. At that point we remove the already completed
        // items up until the point we completed them.

        if request.pending_root_tasks.get() > 0.0 {
            // When there are pending root tasks we don't want to flush anything
            return;
        }

        let completed_root_segment = request.completed_root_segment.borrow().clone();
        if let Some(completed_root_segment) = completed_root_segment {
            if completed_root_segment.status.get() == POSTPONED {
                return;
            }

            let Some(completed_preamble_segments) =
                request.completed_preamble_segments.borrow().clone()
            else {
                // The preamble isn't ready yet even though the root is so we omit flushing
                return;
            };

            FLUSHED_BYTE_SIZE.set(request.byte_size.get()); // Start counting bytes
            // TODO: Count the size of the preamble chunks too.
            let mut skip_blocking_shell = false;
            if ENABLE_FIZZ_BLOCKING_RENDER.with(|&enabled| enabled) {
                let blocking_render_max_size = get_blocking_render_max_size(request);
                if FLUSHED_BYTE_SIZE.get() > blocking_render_max_size {
                    skip_blocking_shell = true;
                    let max_size_kb = js::math::round(blocking_render_max_size / 1000.0);
                    let error = js::error::new(
                        Some(&format!(
                            "This rendered a large document (>{max_size_kb} kB) without any Suspense boundaries around most of it. That can delay initial paint longer than necessary. To improve load performance, add a <Suspense> or <SuspenseList> around the content you expect to be below the header or below the fold. In the meantime, the content will deopt to paint arbitrary incomplete pieces of HTML."
                        )),
                        None,
                    );
                    let error_info = new_thrown_info();
                    log_recoverable_error(request, Some(js::unknown(error)), error_info, None);
                }
            }
            flush_preamble(
                request,
                destination,
                &completed_root_segment,
                &completed_preamble_segments,
                skip_blocking_shell,
            );
            FLUSHING_SHELL.set(true);
            flush_segment(request, destination, &completed_root_segment, None);
            FLUSHING_SHELL.set(false);
            request.completed_root_segment.replace(None);
            let is_complete = request.all_pending_tasks.get() == 0.0
                && request.client_rendered_boundaries.borrow().is_empty()
                && request.completed_boundaries.borrow().is_empty()
                && request
                    .tracked_postpones
                    .borrow()
                    .as_ref()
                    .is_none_or(|tracked_postpones| {
                        tracked_postpones.root_nodes.borrow().is_empty()
                            && matches!(*tracked_postpones.root_slots.borrow(), ResumeSlots::None)
                    });
            write_completed_root(
                destination,
                request.resumable_state,
                request.render_state,
                is_complete,
            );
        }

        write_hoistables(destination, request.resumable_state, request.render_state);
        // We emit client rendering instructions for already emitted boundaries first.
        // This is so that we can signal to the client to start client rendering them as
        // soon as possible.
        let client_rendered_boundaries = &request.client_rendered_boundaries;
        let mut i = 0;
        while i < client_rendered_boundaries.borrow().len() {
            let boundary = Rc::clone(&client_rendered_boundaries.borrow()[i]);
            if !flush_client_rendered_boundary(request, destination, &boundary) {
                request.destination.set(None);
                i += 1;
                client_rendered_boundaries.borrow_mut().drain(0..i);
                return;
            }
            i += 1;
        }
        client_rendered_boundaries.borrow_mut().drain(0..i);

        // Next we emit any complete boundaries. It's better to favor boundaries
        // that are completely done since we can actually show them, than it is to emit
        // any individual segments from a partially complete boundary.
        let completed_boundaries = &request.completed_boundaries;
        i = 0;
        while i < completed_boundaries.borrow().len() {
            let boundary = Rc::clone(&completed_boundaries.borrow()[i]);
            if !flush_completed_boundary(request, destination, &boundary) {
                request.destination.set(None);
                i += 1;
                completed_boundaries.borrow_mut().drain(0..i);
                return;
            }
            i += 1;
        }
        completed_boundaries.borrow_mut().drain(0..i);

        // Allow anything written so far to flush to the underlying sink before
        // we continue with lower priorities.
        complete_writing(destination);
        begin_writing(destination);

        // TODO: Here we'll emit data used by hydration.

        // Next we emit any segments of any boundaries that are partially complete
        // but not deeply complete.
        FLUSHING_PARTIAL_BOUNDARIES.set(true);
        let partial_boundaries = &request.partial_boundaries;
        i = 0;
        while i < partial_boundaries.borrow().len() {
            let boundary = Rc::clone(&partial_boundaries.borrow()[i]);
            if !flush_partial_boundary(request, destination, &boundary) {
                request.destination.set(None);
                i += 1;
                partial_boundaries.borrow_mut().drain(0..i);
                return;
            }
            i += 1;
        }
        partial_boundaries.borrow_mut().drain(0..i);
        FLUSHING_PARTIAL_BOUNDARIES.set(false);

        // Next we check the completed boundaries again. This may have had
        // boundaries added to it in case they were too larged to be inlined.
        // SuspenseListRows might have been unblocked as well.
        // New ones might be added in this loop.
        let large_boundaries = &request.completed_boundaries;
        i = 0;
        while i < large_boundaries.borrow().len() {
            let boundary = Rc::clone(&large_boundaries.borrow()[i]);
            if !flush_completed_boundary(request, destination, &boundary) {
                request.destination.set(None);
                i += 1;
                large_boundaries.borrow_mut().drain(0..i);
                return;
            }
            i += 1;
        }
        large_boundaries.borrow_mut().drain(0..i);
    }));
    FLUSHING_PARTIAL_BOUNDARIES.set(false);
    let postponed_state = request.postponed_state.borrow().clone();
    if let Some(postponed_state) = postponed_state {
        // The shell flush above may have outlined completed boundaries, advancing
        // nextSegmentId past the value getPostponedState snapshotted before the
        // flush. Re-sync the postponed state to the post-flush high-water mark so
        // the resume allocates segment ids strictly above the shell's and can't
        // emit duplicate B:/S: ids. nextSegmentId only increases, so later flush
        // passes refine this monotonically.
        // TODO: Could be too late if the postponed state was already serialized
        // by API consumers. Accessing postponed state before the prelude has flushed
        // should be forbidden in the API.
        postponed_state
            .next_segment_id
            .set(request.next_segment_id.get());
    }
    if request.all_pending_tasks.get() == 0.0
        && request.client_rendered_boundaries.borrow().is_empty()
        && request.completed_boundaries.borrow().is_empty()
        && request.partial_boundaries.borrow().is_empty()
    {
        request.flush_scheduled.set(false);
        // We write the trailing tags but only if don't have any data to resume.
        // If we need to resume we'll write the postamble in the resume instead.
        write_postamble(destination, request.resumable_state);
        complete_writing(destination);
        flush_buffered(destination);
        if __DEV__ && !request.abortable_tasks.borrow().is_empty() {
            console::error((
                "There was still abortable task at the root when we closed. This is a bug in React.",
            ));
        }
        // We're done.
        end_render_lifetime(request);
        request.status.set(CLOSED);
        close(destination);
        // We need to stop flowing now because we do not want any async contexts which might call
        // float methods to initiate any flushes after this point
        stop_flowing(request);
    } else {
        complete_writing(destination);
        flush_buffered(destination);
    }
    if let Err(thrown_value) = result {
        resume_unwind(thrown_value);
    }
}

pub fn start_work(request: &Rc<Request>) {
    // $FlowFixMe[constant-condition]
    if IS_WORK_LOOP_EXTERNALLY_DRIVEN {
        return;
    }
    request
        .flush_scheduled
        .set(request.destination.get().is_some());
    // When prerendering we use microtasks for pinging work
    // $FlowFixMe[constant-condition]
    if SUPPORTS_REQUEST_STORAGE {
        let request = Rc::clone(request);
        schedule_microtask(move || {
            REQUEST_STORAGE.run(request_store(&request), || perform_work(&request))
        });
    } else {
        let request = Rc::clone(request);
        schedule_microtask(move || perform_work(&request));
    }
    let request = Rc::clone(request);
    schedule_work(move || {
        if request.status.get() == OPENING {
            request.status.set(OPEN);
        }

        if request.tracked_postpones.borrow().is_none() {
            // this is either a regular render or a resume. For regular render we want
            // to call emitEarlyPreloads after the first performWork because we want
            // are responding to a live request and need to balance sending something early
            // (i.e. don't want for the shell to finish) but we need something to send.
            // The only implementation of this is for DOM at the moment and during resumes nothing
            // actually emits but the code paths here are the same.
            // During a prerender we don't want to be too aggressive in emitting early preloads
            // because we aren't responding to a live request and we can wait for the prerender to
            // postpone before we emit anything.
            // $FlowFixMe[constant-condition]
            if SUPPORTS_REQUEST_STORAGE {
                REQUEST_STORAGE.run(request_store(&request), || {
                    enqueue_early_preloads_after_initial_work(&request)
                });
            } else {
                enqueue_early_preloads_after_initial_work(&request);
            }
        }
    });
}

fn enqueue_early_preloads_after_initial_work(request: &Rc<Request>) {
    let shell_complete = request.pending_root_tasks.get() == 0.0;
    safely_emit_early_preloads(request, shell_complete);
}

fn enqueue_flush(request: &Rc<Request>) {
    if !request.flush_scheduled.get()
        // If there are pinged tasks we are going to flush anyway after work completes
        && request.pinged_tasks.borrow().is_empty()
        // If there is no destination there is nothing we can flush to. A flush will
        // happen when we start flowing again
        && request.destination.get().is_some()
    {
        request.flush_scheduled.set(true);
        let request = Rc::clone(request);
        schedule_work(move || {
            // We need to existence check destination again here because it might go away
            // in between the enqueueFlush call and the work execution
            let destination = request.destination.get();
            if let Some(destination) = destination {
                flush_completed_queues(&request, destination);
            } else {
                request.flush_scheduled.set(false);
            }
        });
    }
}

// This function is intented to only be called during the pipe function for the Node builds.
// The reason we need this is because `renderToPipeableStream` is the only API which allows
// you to start flowing before the shell is complete and we've had a chance to emit early
// preloads already. This is really just defensive programming to ensure that we give hosts an
// opportunity to flush early preloads before streaming begins in case they are in an environment
// that only supports a single call to emitEarlyPreloads like the DOM renderers. It's unfortunate
// to put this Node only function directly in ReactFizzServer but it'd be more ackward to factor it
// by moving the implementation into ReactServerStreamConfigNode and even then we may not be able to
// eliminate all the wasted branching.
pub fn prepare_for_start_flowing_if_before_all_ready(request: &Rc<Request>) {
    let shell_complete = if request.tracked_postpones.borrow().is_none() {
        // Render Request, we define shell complete by the pending root tasks
        request.pending_root_tasks.get() == 0.0
    } else {
        // Prerender Request, we define shell complete by completedRootSegemtn
        match request.completed_root_segment.borrow().as_ref() {
            None => request.pending_root_tasks.get() == 0.0,
            Some(completed_root_segment) => completed_root_segment.status.get() != POSTPONED,
        }
    };
    safely_emit_early_preloads(request, shell_complete);
}

pub fn start_flowing(request: &Rc<Request>, destination: &'static Destination) {
    if request.status.get() == CLOSING {
        request.status.set(CLOSED);
        let mut error = request.fatal_error.get();
        if is_recoverable_error(error) {
            // An aborted request keeps its original branded reason while tasks
            // unwind. Convert it only now that the stream must receive a fatal.
            error = Some(js::unknown(clone_recoverable_error_as_fatal(unsafe {
                js::cast(error)
            })));
        }
        close_with_error(destination, error);
        return;
    }
    if request.status.get() == CLOSED {
        return;
    }
    if request.destination.get().is_some() {
        // We're already flowing.
        return;
    }
    request.destination.set(Some(destination));

    if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
        flush_completed_queues(request, destination);
    })) {
        let error_info = new_thrown_info();
        log_recoverable_error(request, thrown_value_of(&*error), error_info, None);
        fatal_error(request, thrown_value_of(&*error), error_info, None);
    }
}

pub fn stop_flowing(request: &Request) {
    request.destination.set(None);
}

fn finish_abort(request: &Rc<Request>, abortable_tasks: &AbortSet) {
    if let Err(error) = catch_unwind(AssertUnwindSafe(|| {
        if !abortable_tasks.borrow().is_empty() {
            let error = request.fatal_error.get();
            if __DEV__ {
                for task in abortable_tasks.borrow().iter() {
                    finish_aborted_task_dev(task, request, error);
                }
            } else {
                for task in abortable_tasks.borrow().iter() {
                    finish_aborted_task(task, request, error);
                }
            }
            abortable_tasks.borrow_mut().clear();
        }
        if let Some(destination) = request.destination.get() {
            flush_completed_queues(request, destination);
        }
    })) {
        let error_info = new_thrown_info();
        log_recoverable_error(request, thrown_value_of(&*error), error_info, None);
        fatal_error(request, thrown_value_of(&*error), error_info, None);
    }
}

fn end_render_lifetime(request: &Request) {
    let render_lifetime_controller = request.render_lifetime_controller.get();
    if let Some(render_lifetime_controller) = render_lifetime_controller {
        render_lifetime_controller.abort(Some(&RENDER_ENDED));
    }
}

// Aborts the request when the caller's signal aborts. The render lifetime
// bounds the listener, so the runtime removes the listener as soon as the
// render ends. From that point on abort() returns early, so the listener has
// nothing left to do.
//
// The listener has to be removed, because it would otherwise keep the whole
// Request reachable for as long as the caller's signal lives. A composite
// signal from AbortSignal.any() is itself retained by the runtime while it has
// any abort listener attached.
//
// A request whose stream is neither consumed nor cancelled never ends, so its
// listener stays attached for as long as the caller's signal lives.
pub fn attach_abort_signal(request: &Rc<Request>, signal: &'static AbortSignal) {
    if signal.aborted() {
        abort(request, signal.reason());
        return;
    }
    let render_lifetime_controller = webapi::abort_controller::new();
    request
        .render_lifetime_controller
        .set(Some(render_lifetime_controller));
    let request = Rc::clone(request);
    signal.add_event_listener(
        webapi::events::Abort,
        move |_: &Event| {
            abort(&request, signal.reason());
        },
        Some(&AddEventListenerOptions {
            capture: None,
            passive: None,
            once: None,
            signal: Some(render_lifetime_controller.signal()),
        }),
    );
}

// This is called to early terminate a request. It puts all pending boundaries in client rendered state.
pub fn abort(request: &Rc<Request>, reason: Option<&'static Unknown>) {
    if request.aborted.get() || (request.status.get() != OPEN && request.status.get() != OPENING) {
        // Only requests that are not already complete or in the process of aborting
        // can be aborted. in practice this makes abort callable at most once per render.
        return;
    }
    end_render_lifetime(request);
    let is_recoverable_reason = js::type_of(reason) == "object"
        && !matches!(js::classify(reason), Kind::Null)
        // $FlowFixMe[prop-missing]
        && SymbolFor::from_value(js::get(reason, "$$typeof")) == Some(REACT_RECOVERABLE_TYPE);
    // Mark the request before initializing a recoverable reason so an initializer
    // cannot reenter abort().
    request.aborted.set(true);
    let error = if is_recoverable_reason {
        Some(js::unknown(create_recoverable_error(unsafe {
            js::cast(reason)
        })))
    } else if matches!(js::classify(reason), Kind::Undefined) {
        Some(js::unknown(js::error::new(
            Some("The render was aborted by the server without a reason."),
            None,
        )))
    } else if js::type_of(reason) == "object"
        && !matches!(js::classify(reason), Kind::Null)
        && js::type_of(js::get(reason, "then")) == "function"
    {
        Some(js::unknown(js::error::new(
            Some("The render was aborted by the server with a promise."),
            None,
        )))
    } else {
        reason
    };
    // This error isn't necessarily fatal in this case but we need to stash it
    // so we can use it to abort any pending work.
    request.fatal_error.set(error);
    let abortable_tasks = Rc::clone(&request.abortable_tasks);
    if __DEV__ {
        for task in abortable_tasks.borrow().iter() {
            abort_task_dev(task, request);
        }
    } else {
        for task in abortable_tasks.borrow().iter() {
            abort_task(task, request);
        }
    }
    // Even though this looks async some renderers schedule work sync
    // So it is important that the finish step does not assume the stack
    // has unwinded yet
    let request = Rc::clone(request);
    schedule_work(move || finish_abort(&request, &abortable_tasks));
}

pub fn flush_resources(request: &Rc<Request>) {
    enqueue_flush(request);
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_form_state(request: &Request) -> Option<&'static FormState> {
    request.form_state
}

pub fn get_resumable_state(request: &Request) -> &'static ResumableState {
    request.resumable_state
}

pub fn get_render_state(request: &Request) -> &'static RenderState {
    request.render_state
}

fn add_to_replay_parent(
    node: &Rc<RefCell<ReplayNode>>,
    parent_key_path: Option<&Rc<KeyNode>>,
    tracked_postpones: &PostponedHoles,
) {
    match parent_key_path {
        None => {
            tracked_postpones
                .root_nodes
                .borrow_mut()
                .push(Rc::clone(node));
        }
        Some(parent_key_path) => {
            let working_map = &tracked_postpones.working_map;
            let parent_node = working_map.borrow().get(parent_key_path).cloned();
            let parent_node = match parent_node {
                Some(parent_node) => parent_node,
                None => {
                    let parent_node = Rc::new(RefCell::new(ReplayNode::Element(ReplayElement(
                        parent_key_path.1.clone(),
                        parent_key_path.2.clone(),
                        Rc::new(RefCell::new(vec![])),
                        ResumeSlots::None,
                    ))));
                    working_map
                        .borrow_mut()
                        .insert(Rc::clone(parent_key_path), Rc::clone(&parent_node));
                    add_to_replay_parent(
                        &parent_node,
                        parent_key_path.0.as_ref(),
                        tracked_postpones,
                    );
                    parent_node
                }
            };
            let (ReplayNode::Element(ReplayElement(_, _, children, _))
            | ReplayNode::Boundary(ReplaySuspenseBoundary(_, _, children, ..))) =
                &*parent_node.borrow();
            children.borrow_mut().push(Rc::clone(node));
        }
    }
}

pub struct PostponedState {
    next_segment_id: Cell<f64>,
    root_format_context: &'static FormatContext,
    progressive_chunk_size: f64,
    pub resumable_state: &'static ResumableState,
    replay_nodes: ReplayNodes,
    replay_slots: RefCell<ResumeSlots>,
}

// Returns the state of a postponed request or null if nothing was postponed.
#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_postponed_state(request: &Request) -> Option<Rc<PostponedState>> {
    let tracked_postpones = request.tracked_postpones.borrow().clone();
    let Some(tracked_postpones) = tracked_postpones.filter(|tracked_postpones| {
        !(tracked_postpones.root_nodes.borrow().is_empty()
            && matches!(*tracked_postpones.root_slots.borrow(), ResumeSlots::None))
    }) else {
        // Reset. Let the flushing behave as if we completed the whole document.
        request.tracked_postpones.replace(None);
        return None;
    };
    // True only when we postponed with a real shell (as opposed to postponing the
    // root itself). Only in that case does the prelude flush outline completed
    // boundaries and advance nextSegmentId past the value we capture here.
    let has_flushable_shell = match request.completed_root_segment.borrow().as_ref() {
        None => true,
        // The Root did not postpone
        Some(completed_root_segment) => {
            completed_root_segment.status.get() != POSTPONED
                // the Preamble was available
                && request.completed_preamble_segments.borrow().is_some()
        }
    };
    let (replay_slots, next_segment_id) = if !has_flushable_shell {
        // We need to ensure that on resume we retry the root. We use a number
        // type for the replaySlots to signify this (see resumeRequest).
        // The value -1 represents an unassigned ID but is not functionally meaningful
        // for resuming at the root.
        // We either postponed the root or we did not have a preamble to flush
        reset_resumable_state(request.resumable_state, request.render_state);
        (ResumeSlots::Root(-1.0), 0.0)
    } else {
        complete_resumable_state(request.resumable_state);
        (
            tracked_postpones.root_slots.borrow().clone(),
            request.next_segment_id.get(),
        )
    };
    let postponed_state = Rc::new(PostponedState {
        next_segment_id: Cell::new(next_segment_id),
        root_format_context: request.root_format_context,
        progressive_chunk_size: request.progressive_chunk_size,
        resumable_state: request.resumable_state,
        replay_nodes: Rc::clone(&tracked_postpones.root_nodes),
        replay_slots: RefCell::new(replay_slots),
    });
    if has_flushable_shell {
        // The prelude hasn't flushed yet — it's pull-driven and runs after this
        // (see completeAll -> onAllReady). Flushing the shell outlines completed
        // boundaries, bumping request.nextSegmentId beyond the snapshot above. Hold
        // a reference so flushCompletedQueues can finalize nextSegmentId to the
        // post-flush high-water mark. Otherwise the resume reuses the shell's
        // segment ids and the concatenated document has duplicate B:/S: ids, which
        // cross-wires React's boundary-completion scripts ($RC).
        request
            .postponed_state
            .replace(Some(Rc::clone(&postponed_state)));
    }
    Some(postponed_state)
}
