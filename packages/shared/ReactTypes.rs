// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::rc::{Rc, Weak};

use js::{Dict, Error, JsError, JsObject, Promise, SymbolFor, Unknown};
use webapi::{ConsoleTask, FormData};

/// An element's key, text, or the optimistic key a renderer gives it later.
#[cfg_attr(rust_js, rust_js::untagged)]
#[derive(Clone, PartialEq)]
pub enum ReactKey {
    Text(String),
    Optimistic(SymbolFor<'static>),
}

unsafe impl js::Defined for ReactKey {}

/// An element, a portal, text, a fragment, a provider or a consumer.
pub type ReactNode = Unknown;

/// `null`, `undefined` or a boolean, which render nothing.
pub type ReactEmpty = Unknown;

/// Nothing, or an iterable of nodes.
pub type ReactFragment = Unknown;

/// Nothing, or a node.
pub type ReactNodeList = Unknown;

/// A string or a number.
pub type ReactText = Unknown;

/// A context's provider element.
pub struct ReactProvider<T: 'static> {
    #[cfg_attr(rust_js, rust_js::name = "$$typeof")]
    pub r#typeof: SymbolFor<'static>,
    pub r#type: Rc<ReactContext<T>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub key: Option<ReactKey>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub r#ref: Option<&'static Unknown>,
    pub props: ReactProviderProps<T>,
}

/// A provider element's props.
pub struct ReactProviderProps<T: 'static> {
    pub value: T,
    pub children: Option<&'static ReactNodeList>,
}

/// What reads a context's value.
pub struct ReactConsumerType<T: 'static> {
    #[cfg_attr(rust_js, rust_js::name = "$$typeof")]
    pub r#typeof: SymbolFor<'static>,
    #[cfg_attr(rust_js, rust_js::name = "_context")]
    pub context: Weak<ReactContext<T>>,
}

/// A context's consumer element.
pub struct ReactConsumer<T: 'static> {
    #[cfg_attr(rust_js, rust_js::name = "$$typeof")]
    pub r#typeof: SymbolFor<'static>,
    pub r#type: Rc<ReactConsumerType<T>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub key: Option<ReactKey>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub r#ref: Option<&'static Unknown>,
    pub props: ReactConsumerProps<T>,
}

/// A consumer element's props: what renders the context's value.
pub struct ReactConsumerProps<T: 'static> {
    pub children: &'static dyn Fn(T) -> Option<&'static ReactNodeList>,
}

/// A context: the value its provider gives what it renders, and the
/// consumer that reads it.
pub struct ReactContext<T: 'static> {
    #[cfg_attr(rust_js, rust_js::name = "$$typeof")]
    pub r#typeof: SymbolFor<'static>,
    #[cfg_attr(rust_js, rust_js::name = "_currentValue")]
    pub current_value: RefCell<T>,
    #[cfg_attr(rust_js, rust_js::name = "_currentValue2")]
    pub current_value2: RefCell<T>,
    #[cfg_attr(rust_js, rust_js::name = "_threadCount")]
    pub thread_count: Cell<f64>,
    #[cfg_attr(rust_js, rust_js::name = "Provider")]
    pub provider: Cell<Weak<ReactContext<T>>>,
    #[cfg_attr(rust_js, rust_js::name = "Consumer")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub consumer: Cell<Option<Rc<ReactConsumerType<T>>>>,
    // DEV only
    #[cfg_attr(rust_js, rust_js::name = "_currentRenderer")]
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub current_renderer: Cell<Option<&'static Unknown>>,
    #[cfg_attr(rust_js, rust_js::name = "_currentRenderer2")]
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub current_renderer2: Cell<Option<&'static Unknown>>,
}

/// A portal: children rendered into another container.
pub struct ReactPortal {
    // This tag allow us to uniquely identify this as a React Portal
    #[cfg_attr(rust_js, rust_js::name = "$$typeof")]
    pub r#typeof: SymbolFor<'static>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub key: Option<ReactKey>,
    pub children: Option<&'static ReactNodeList>,
    pub container_info: Option<&'static Unknown>,
    // TODO: figure out the API for cross-renderer implementation.
    pub implementation: Option<&'static Unknown>,
}

/// What a ref holds: `current`, the value it's set to, `null` until then
/// where `createRef` made it.
pub struct RefObject {
    pub current: Cell<Option<&'static Unknown>>,
}

unsafe impl js::Defined for RefObject {}

/// A Scope's type.
pub struct ReactScope {
    #[cfg_attr(rust_js, rust_js::name = "$$typeof")]
    pub r#typeof: SymbolFor<'static>,
}

/// Whether a host instance, of its type and props, is one the query wants.
pub type ReactScopeQuery = dyn Fn(
    Option<&'static Unknown>,
    Option<&'static Unknown>,
    &'static Unknown,
) -> Option<&'static Unknown>;

// `ReactScopeInstance` is react-reconciler's ReactFiberScope's, the object
// whose methods it implements.

// The subset of a Thenable required by things thrown by Suspense.
// This doesn't require a value to be passed to either handler.
/// What resolves, calling back what waits on it with nothing.
pub struct Wakeable(PhantomData<JsObject>);

unsafe impl js::Defined for Wakeable {}
// Safety: a wakeable is an object, a ping cache's key.
unsafe impl js::WeakKey for Wakeable {}

impl Wakeable {
    #[cfg_attr(rust_js, rust_js::link_name = "then")]
    pub fn then(&self, on_fulfill: &dyn Fn(), on_reject: &dyn Fn()) {
        let _ = (on_fulfill, on_reject);
        unreachable!()
    }
}

// The subset of a Promise that React APIs rely on. This resolves a value.
// This doesn't require a return value neither from the handler nor the
// then function.
/// A thenable: what a lazy component's loader gives, or `use` reads, which
/// React makes introspectable as it settles, its `status` `'pending'`,
/// `'pending_weak'`, `'fulfilled'` with a `value` or `'rejected'` with a
/// `reason`.
pub struct Thenable(PhantomData<JsObject>);

unsafe impl js::Defined for Thenable {}

impl Thenable {
    #[cfg_attr(rust_js, rust_js::link_name = "then")]
    pub fn then(
        &self,
        on_fulfill: impl FnOnce(Option<&'static Unknown>) + 'static,
        on_reject: impl FnOnce(Option<&'static Unknown>) + 'static,
    ) {
        let _ = (on_fulfill, on_reject);
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "get status")]
    pub fn status(&self) -> Option<String> {
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "set status")]
    pub fn set_status(&self, status: &str) {
        let _ = status;
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "get value")]
    pub fn value(&self) -> Option<&'static Unknown> {
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "set value")]
    pub fn set_value(&self, value: Option<&Unknown>) {
        let _ = value;
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "get reason")]
    pub fn reason(&self) -> Option<&'static Unknown> {
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "set reason")]
    pub fn set_reason(&self, reason: Option<&Unknown>) {
        let _ = reason;
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "get displayName")]
    pub fn display_name(&self) -> Option<&'static Unknown> {
        unreachable!()
    }
}

/// A thenable whose `status` is `'pending'`.
pub type PendingThenable = Thenable;
/// A thenable whose `status` is `'pending_weak'`.
pub type WeakPendingThenable = Thenable;
/// A thenable whose `status` is `'fulfilled'`, with its `value`.
pub type FulfilledThenable = Thenable;

/// A thenable React makes already rejected: its `then` does nothing.
pub struct RejectedThenable {
    pub then: fn(),
    pub status: &'static str,
    pub reason: Option<&'static Unknown>,
}

unsafe impl js::Defined for RejectedThenable {}

/// A message, or what gives one.
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum ReactRecoverableReason {
    Message(String),
    Lazy(&'static dyn Fn() -> Option<&'static Unknown>),
}

// A recoverable lets an intermediate renderer defer a subtree to a downstream
// renderer. It does not produce a value: a renderer either continues through
// it or interrupts the current render so that a later renderer can recover the
// subtree. The reason is initialized only by a renderer that defers the work.
pub struct ReactRecoverable {
    #[cfg_attr(rust_js, rust_js::name = "$$typeof")]
    pub r#typeof: SymbolFor<'static>,
    #[cfg_attr(rust_js, rust_js::name = "_reason")]
    pub reason: Option<ReactRecoverableReason>,
}

pub struct StartTransitionOptions {
    pub name: Option<String>,
}

/// What `use` reads: a thenable, a context or a recoverable.
pub trait Usable<T> {}
impl<T> Usable<T> for &ReactContext<T> {}
impl<T> Usable<T> for &Thenable {}
impl<T> Usable<T> for &ReactRecoverable {}

pub struct ReactCustomFormAction {
    pub name: Option<String>,
    pub action: Option<String>,
    pub enc_type: Option<String>,
    pub method: Option<String>,
    pub target: Option<String>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub data: Option<&'static FormData>,
}

// This is an opaque type returned by decodeFormState on the server, but it's
// defined in this shared file because the same type is used by React on
// the client.
/// The actual state value, the key path, the Server Reference ID and the
/// number of bound arguments.
pub type ReactFormState<S, ReferenceId> = (S, String, ReferenceId, f64);

// Intrinsic GestureProvider. This type varies by Environment whether a particular
// renderer supports it.
pub type GestureProvider = Unknown;

pub struct GestureOptions {
    pub range_start: Option<f64>,
    pub range_end: Option<f64>,
}

// `Awaited<T>`, what `await` unwraps a type to, is a type-level function
// that has no Rust form: Rust's is `IntoFuture::Output`.

/// The function name, the file name, the line and column numbers, the
/// enclosing line and column numbers, and whether it's an async resume.
// TODO: model nested eval locations as nested arrays
pub type ReactCallSite = (String, String, f64, f64, f64, f64, bool);

pub type ReactStackTrace = Vec<ReactCallSite>;

/// The function name, the file name, and the enclosing line and column
/// numbers.
// TODO: model nested eval locations as nested arrays
pub type ReactFunctionLocation = (String, String, f64, f64);

pub struct ReactComponentInfo {
    pub name: String,
    pub env: Option<String>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub key: Option<ReactKey>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub owner: Option<&'static ReactComponentInfo>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub stack: Option<ReactStackTrace>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub props: Option<&'static Dict<Option<&'static Unknown>>>,
    // Stashed Data for the Specific Execution Environment. Not part of the transport protocol
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub debug_stack: Option<&'static JsError>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub debug_task: Option<&'static ConsoleTask>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub debug_location: Option<&'static JsError>,
}

pub struct ReactEnvironmentInfo {
    pub env: String,
}

pub struct ReactErrorInfoProd {
    pub digest: String,
}

/// A string, boolean, number, `null`, or an object or array of them.
pub type JSONValue = Unknown;

pub struct ReactErrorInfoDev {
    pub digest: Option<String>,
    #[cfg_attr(rust_js, rust_js::test)]
    pub name: String,
    pub message: String,
    pub stack: ReactStackTrace,
    pub env: String,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub owner: Option<String>,
    pub cause: Option<&'static JSONValue>,
    pub errors: Option<&'static JSONValue>,
}

#[cfg_attr(rust_js, rust_js::untagged)]
pub enum ReactErrorInfo {
    Dev(ReactErrorInfoDev),
    #[cfg_attr(rust_js, rust_js::otherwise)]
    Prod(ReactErrorInfoProd),
}

// The point where the Async Info started which might not be the same place it was awaited.
pub struct ReactIOInfo {
    // the name of the async function being called (e.g. "fetch")
    pub name: Cell<&'static str>,
    // the start time
    pub start: Cell<f64>,
    // the end time (this might be different from the time the await was unblocked)
    pub end: Cell<f64>,
    // the byte size of this resource across the network. (should only be included if affecting the client.)
    pub byte_size: Option<f64>,
    // the Promise that was awaited if any, may be rejected
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub value: Cell<Option<Promise<Option<&'static Unknown>>>>,
    // the environment where this I/O was spawned.
    pub env: Option<String>,
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub owner: Option<&'static Unknown>,
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub stack: Option<ReactStackTrace>,
    // Stashed Data for the Specific Execution Environment. Not part of the transport protocol
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub debug_stack: Option<&'static Error>,
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub debug_task: Option<&'static ConsoleTask>,
}

pub struct ReactAsyncInfo {
    pub awaited: Rc<ReactIOInfo>,
    // the environment where this was awaited. This might not be the same as where it was spawned.
    pub env: Option<String>,
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub owner: Option<&'static ReactComponentInfo>,
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub stack: Option<ReactStackTrace>,
    // Stashed Data for the Specific Execution Environment. Not part of the transport protocol
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub debug_stack: Option<&'static Error>,
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub debug_task: Option<&'static ConsoleTask>,
}

pub struct ReactTimeInfo {
    // performance.now
    pub time: f64,
}

/// A `ReactComponentInfo`, `ReactEnvironmentInfo`, `ReactAsyncInfo` or
/// `ReactTimeInfo`, which React tells apart by their members, `name`, `env`,
/// `awaited` and `time`. It isn't an untagged enum of them: an object member,
/// `awaited`, tells no variant (rust-js's rd291), and an async entry has an
/// `env` too.
pub type ReactDebugInfoEntry = Unknown;

pub type ReactDebugInfo = Vec<&'static ReactDebugInfoEntry>;

// Intrinsic ViewTransitionInstance. This type varies by Environment whether a particular
// renderer supports it.
pub type ViewTransitionInstance = Unknown;

/// A class for each transition type, whose `default` is the class of a type
/// it has none for: `'none'`, `'auto'` or another.
pub type ViewTransitionClassPerType = Dict<String>;

/// A class name, `'none'` or `'auto'` among them, or one for each transition
/// type.
#[derive(Clone, Copy)]
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum ViewTransitionClass {
    Name(&'static str),
    #[cfg_attr(rust_js, rust_js::otherwise)]
    PerType(&'static ViewTransitionClassPerType),
}

pub struct GestureOptionsRequired {
    pub range_start: f64,
    pub range_end: f64,
}

/// What a `<ViewTransition>` calls as it animates, given its
/// ReactFiberConfig's `ViewTransitionInstance` and the transition's types:
/// what cleans up after it, if anything.
pub type ViewTransitionCallback =
    dyn Fn(&'static ViewTransitionInstance, &Rc<RefCell<Vec<String>>>) -> Option<&'static dyn Fn()>;

/// What a `<ViewTransition>` calls as a gesture animates it, given the
/// ReactFiberConfig's `GestureTimeline` first.
pub type ViewTransitionGestureCallback = dyn Fn(
    &'static GestureProvider,
    &GestureOptionsRequired,
    &'static ViewTransitionInstance,
    &Rc<RefCell<Vec<String>>>,
) -> Option<&'static dyn Fn()>;

pub struct ViewTransitionProps {
    pub name: Option<String>,
    pub children: Option<&'static ReactNodeList>,
    pub default: Option<ViewTransitionClass>,
    pub enter: Option<ViewTransitionClass>,
    pub exit: Option<ViewTransitionClass>,
    pub share: Option<ViewTransitionClass>,
    pub update: Option<ViewTransitionClass>,
    pub parent_enter: Option<ViewTransitionClass>,
    pub parent_exit: Option<ViewTransitionClass>,
    pub on_enter: Option<&'static ViewTransitionCallback>,
    pub on_exit: Option<&'static ViewTransitionCallback>,
    pub on_parent_enter: Option<&'static ViewTransitionCallback>,
    pub on_parent_exit: Option<&'static ViewTransitionCallback>,
    pub on_share: Option<&'static ViewTransitionCallback>,
    pub on_update: Option<&'static ViewTransitionCallback>,
    pub on_gesture_enter: Option<&'static ViewTransitionGestureCallback>,
    pub on_gesture_exit: Option<&'static ViewTransitionGestureCallback>,
    pub on_gesture_parent_enter: Option<&'static ViewTransitionGestureCallback>,
    pub on_gesture_parent_exit: Option<&'static ViewTransitionGestureCallback>,
    pub on_gesture_share: Option<&'static ViewTransitionGestureCallback>,
    pub on_gesture_update: Option<&'static ViewTransitionGestureCallback>,
}

unsafe impl js::Defined for ViewTransitionProps {}

// `ActivityProps` is react-reconciler's ReactFiberBeginWork's: its `mode`
// is the reconciler's `OffscreenMode`, which it passes on to the Offscreen
// fiber it makes, as Flow lets upstream's `'hidden' | 'visible'` flow into it.

pub struct SuspenseProps {
    pub children: Option<&'static ReactNodeList>,
    pub fallback: Option<&'static ReactNodeList>,

    // TODO: Add "unstable_" prefix?
    pub suspense_callback: Option<&'static dyn Fn(Option<&'static js::Set<Wakeable>>)>,

    #[cfg_attr(rust_js, rust_js::name = "unstable_avoidThisFallback")]
    pub unstable_avoid_this_fallback: Option<bool>,
    pub defer: Option<bool>,
    pub name: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SuspenseListRevealOrder {
    #[cfg_attr(rust_js, rust_js::name = "forwards")]
    Forwards,
    #[cfg_attr(rust_js, rust_js::name = "backwards")]
    Backwards,
    #[cfg_attr(rust_js, rust_js::name = "unstable_legacy-backwards")]
    UnstableLegacyBackwards,
    #[cfg_attr(rust_js, rust_js::name = "together")]
    Together,
    #[cfg_attr(rust_js, rust_js::name = "independent")]
    Independent,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SuspenseListTailMode {
    #[cfg_attr(rust_js, rust_js::name = "visible")]
    Visible,
    #[cfg_attr(rust_js, rust_js::name = "collapsed")]
    Collapsed,
    #[cfg_attr(rust_js, rust_js::name = "hidden")]
    Hidden,
}

// A SuspenseList row cannot include a nested Array since it's an easy mistake to not realize it
// is treated as a single row. A Fragment can be used to intentionally have multiple children as
// a single row.
/// A directional list's children are an iterable of rows, each a node that
/// isn't an iterable; a non-directional list's, `'independent'` or
/// `'together'`, any node, and its `tail` none.
pub struct SuspenseListProps {
    pub children: Option<&'static ReactNodeList>,
    pub reveal_order: Option<SuspenseListRevealOrder>,
    pub tail: Option<SuspenseListTailMode>,
}

pub struct TracingMarkerProps {
    pub name: String,
    pub children: Option<&'static ReactNodeList>,
}

pub struct CacheProps {
    pub children: Option<&'static ReactNodeList>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ProfilerPhase {
    #[cfg_attr(rust_js, rust_js::name = "mount")]
    Mount,
    #[cfg_attr(rust_js, rust_js::name = "update")]
    Update,
    #[cfg_attr(rust_js, rust_js::name = "nested-update")]
    NestedUpdate,
}

/// A `<Profiler>`'s `onRender`: its id, phase, actual and base durations,
/// start time and commit time.
pub type OnRender =
    dyn Fn(Option<&'static Unknown>, &str, Option<f64>, Option<f64>, Option<f64>, f64);
/// A `<Profiler>`'s `onCommit` and `onPostCommit`: its id, phase, effect
/// duration and commit time.
pub type OnCommit = dyn Fn(Option<&'static Unknown>, &str, f64, f64);

pub struct ProfilerProps {
    pub id: Option<String>,
    pub on_render: Option<&'static OnRender>,
    pub on_commit: Option<&'static OnCommit>,
    pub children: Option<&'static ReactNodeList>,
}
