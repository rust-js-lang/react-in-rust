// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use js::{
    Error, ErrorOptions, IntoUnknown, JsObject, Kind, Map, PropertyDescriptor, Rest, SymbolFor,
    Unknown, function,
};
use react_isomorphic::react_hooks::{BasicStateAction, InitialState};
use react_reconciler::react_fiber_hooks::{
    Action, ActionDispatch, ActionStateAndDispatch, CacheRefresh, Deps, Dispatch, EventFunction,
    EventFunctionImpl, GetSnapshot, MemoCreate, Reducer, StartTransition, StateAndDispatch,
    Subscribe,
};
use react_reconciler::react_internal_types::{Dispatcher, MemoCacheArray};
use shared::check_string_coercion::check_attribute_string_coercion;
use shared::environment::__DEV__;
use shared::has_own_property::HAS_OWN_PROPERTY;
use shared::noop::noop;
use shared::object_is::OBJECT_IS;
use shared::react_symbols::{
    REACT_CONTEXT_TYPE, REACT_MEMO_CACHE_SENTINEL, REACT_RECOVERABLE_TYPE,
};
use shared::react_types::{
    ReactCustomFormAction, ReactFormState, ReactRecoverable, ReactRecoverableReason, RefObject,
    Thenable,
};
use webapi::console;

use crate::react_fizz_current_task::{KeyNode, Request, Task};
use crate::react_fizz_new_context::{Context, read_context as read_context_impl};
use crate::react_fizz_thenable::{
    ThenableState, create_thenable_state, read_previous_thenable, track_used_thenable,
};
use crate::react_fizz_tree_context::get_tree_id;
use crate::react_fizz_view_transition_component::{ResumableState, make_id};

unsafe extern "Rust" {
    #[link_name = "./src/ReactFizzConfig.js#NotPendingTransition"]
    safe static NOT_PENDING_TRANSITION: Option<&'static Unknown>;
    #[link_name = "./src/ReactFizzConfig.js#supportsClientAPIs"]
    safe static SUPPORTS_CLIENT_APIS: bool;
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactServerStreamConfig.js#createFastHash"
)]
fn create_fast_hash(input: &str) -> String {
    let _ = input;
    unreachable!()
}

#[cfg_attr(rust_js, rust_js::link_name = "./src/ReactFizzServer.js#getFormState")]
#[cfg_attr(rust_js, rust_js::nullable)]
fn get_form_state(
    request: &Request,
) -> Option<&'static ReactFormState<Option<&'static Unknown>, Option<&'static Unknown>>> {
    let _ = request;
    unreachable!()
}

struct Update {
    action: Option<&'static Unknown>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    next: RefCell<Option<Rc<Update>>>,
}

struct UpdateQueue {
    #[cfg_attr(rust_js, rust_js::nullable)]
    last: RefCell<Option<Rc<Update>>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    dispatch: Cell<Option<&'static Dispatch>>,
}

struct Hook {
    memoized_state: Cell<Option<&'static Unknown>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    queue: RefCell<Option<Rc<UpdateQueue>>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    next: RefCell<Option<Rc<Hook>>>,
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::nullable)]
    static CURRENTLY_RENDERING_COMPONENT: Cell<Option<&'static Unknown>> = const { Cell::new(None) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static CURRENTLY_RENDERING_TASK: Cell<Option<&'static Task>> = const { Cell::new(None) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static CURRENTLY_RENDERING_REQUEST: Cell<Option<&'static Request>> = const { Cell::new(None) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static CURRENTLY_RENDERING_KEY_PATH: Cell<Option<&'static KeyNode>> = const { Cell::new(None) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static FIRST_WORK_IN_PROGRESS_HOOK: RefCell<Option<Rc<Hook>>> = const { RefCell::new(None) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static WORK_IN_PROGRESS_HOOK: RefCell<Option<Rc<Hook>>> = const { RefCell::new(None) };
    // Whether the work-in-progress hook is a re-rendered hook
    static IS_RE_RENDER: Cell<bool> = const { Cell::new(false) };
    // Whether an update was scheduled during the currently executing render pass.
    static DID_SCHEDULE_RENDER_PHASE_UPDATE: Cell<bool> = const { Cell::new(false) };
    // Counts the number of useId hooks in this component
    static LOCAL_ID_COUNTER: Cell<f64> = const { Cell::new(0.0) };
    // Chunks that should be pushed to the stream once the component
    // finishes rendering.
    // Counts the number of useActionState calls in this component
    static ACTION_STATE_COUNTER: Cell<f64> = const { Cell::new(0.0) };
    // The index of the useActionState hook that matches the one passed in at the
    // root during an MPA navigation, if any.
    static ACTION_STATE_MATCHING_INDEX: Cell<f64> = const { Cell::new(-1.0) };
    // Counts the number of use(thenable) calls in this component
    static THENABLE_INDEX_COUNTER: Cell<usize> = const { Cell::new(0) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static THENABLE_STATE: RefCell<Option<Rc<ThenableState>>> = const { RefCell::new(None) };
}

#[cfg_attr(rust_js, rust_js::name = "browserReasonInitializationFallback")]
const BROWSER_REASON_INITIALIZATION_FALLBACK: &str = concat!(
    "The reason for browser-only rendering could not be determined because its ",
    "initializer threw."
);

pub fn create_recoverable_error(recoverable: &ReactRecoverable) -> &'static Error {
    let reason = &recoverable.reason;
    let initialized_reason = match reason {
        Some(ReactRecoverableReason::Lazy(reason)) => {
            match catch_unwind(AssertUnwindSafe(reason)) {
                Ok(initialized_reason) => initialized_reason,
                Err(_) => {
                    // A reason is only diagnostic metadata. Its initializer must not affect
                    // whether the renderer can defer this subtree to the browser.
                    Some(js::unknown(BROWSER_REASON_INITIALIZATION_FALLBACK))
                }
            }
        }
        Some(ReactRecoverableReason::Message(reason)) => Some(js::unknown(reason.as_str())),
        None => None,
    };
    // Always create the recoverable at the consumption point so its stack
    // identifies the relevant use() or abort() call. A lazy reason is diagnostic
    // metadata and can be any value supported by Error.cause.
    let options = reason.as_ref().map(|_| ErrorOptions {
        cause: Some(&initialized_reason),
    });
    let error = js::error::new(
        Some("Browser-only rendering was requested by `browser()`."),
        options.as_ref(),
    );
    js::object::define_property(
        error,
        &REACT_RECOVERABLE_TYPE,
        &PropertyDescriptor {
            value: Some(&true),
            ..Default::default()
        },
    );
    error
}

pub fn is_recoverable_error(error: Option<&'static Unknown>) -> bool {
    if js::type_of(error) != "object" || matches!(js::classify(error), Kind::Null) {
        return false;
    }
    js::strict_equals(
        js::get(error, &REACT_RECOVERABLE_TYPE),
        Some(js::unknown(&true)),
    )
}

pub fn clone_recoverable_error_as_fatal(recoverable_error: &Error) -> &'static Error {
    // Create a separate diagnostic for fatal reporting without changing the
    // branded recoverable error that other tasks may still need to observe.
    let cause;
    let options = if HAS_OWN_PROPERTY
        .with(|has_own_property| has_own_property.call(js::unknown(recoverable_error), "cause"))
    {
        cause = recoverable_error.cause();
        Some(ErrorOptions {
            cause: Some(&cause),
        })
    } else {
        None
    };
    let fatal_recoverable_error = js::error::new(
        Some(concat!(
            "The server render could not complete because client rendering was ",
            "requested outside a Suspense boundary. See this error's cause for ",
            "additional details."
        )),
        options.as_ref(),
    );
    // Keep the frames captured where the recoverable was consumed, but replace
    // the first line with the fatal error's message.
    match recoverable_error.stack() {
        Some(stack) => {
            let frame_start = stack.find('\n');
            fatal_recoverable_error.set_stack(Some(
                &(fatal_recoverable_error.name()
                    + ": "
                    + &fatal_recoverable_error.message()
                    + match frame_start {
                        None => "",
                        Some(frame_start) => &stack[frame_start..],
                    }),
            ));
        }
        None => fatal_recoverable_error.set_stack(None),
    }
    fatal_recoverable_error
}

thread_local! {
    // Lazily created map of render-phase updates
    #[cfg_attr(rust_js, rust_js::nullable)]
    static RENDER_PHASE_UPDATES: Cell<Option<&'static Map<UpdateQueue, Rc<Update>>>> =
        const { Cell::new(None) };
    // Counter to prevent infinite loops.
    static NUMBER_OF_RE_RENDERS: Cell<u32> = const { Cell::new(0) };
}

const RE_RENDER_LIMIT: u32 = 25;

thread_local! {
    static IS_IN_HOOK_USER_CODE_IN_DEV: Cell<bool> = const { Cell::new(false) };

    // In DEV, this is the name of the currently executing primitive hook
    static CURRENT_HOOK_NAME_IN_DEV: Cell<Option<&'static str>> = const { Cell::new(None) };
}

fn resolve_currently_rendering_component() -> &'static Unknown {
    if CURRENTLY_RENDERING_COMPONENT.get().is_none() {
        panic!(
            "{}",
            concat!(
                "Invalid hook call. Hooks can only be called inside of the body of a function component. This could happen for",
                " one of the following reasons:\n",
                "1. You might have mismatching versions of React and the renderer (such as React DOM)\n",
                "2. You might be breaking the Rules of Hooks\n",
                "3. You might have more than one copy of React in the same app\n",
                "See https://react.dev/link/invalid-hook-call for tips about how to debug and fix this problem."
            )
        );
    }

    if __DEV__ && IS_IN_HOOK_USER_CODE_IN_DEV.get() {
        console::error((concat!(
            "Do not call Hooks inside useEffect(...), useMemo(...), or other built-in Hooks. ",
            "You can only call Hooks at the top level of your React function. ",
            "For more information, see ",
            "https://react.dev/link/rules-of-hooks"
        ),));
    }
    CURRENTLY_RENDERING_COMPONENT.get().unwrap()
}

fn are_hook_inputs_equal(
    next_deps: &[Option<&'static Unknown>],
    prev_deps: Option<&'static Unknown>,
) -> bool {
    if matches!(js::classify(prev_deps), Kind::Null) {
        if __DEV__ {
            console::error((
                concat!(
                    "%s received a final argument during this render, but not during ",
                    "the previous render. Even though the final argument is optional, ",
                    "its type cannot change between renders."
                ),
                CURRENT_HOOK_NAME_IN_DEV.get(),
            ));
        }
        return false;
    }
    let prev_deps: &[Option<&'static Unknown>] = unsafe { js::cast(prev_deps) };

    // Don't bother comparing lengths in prod because these arrays should be
    // passed inline.
    if __DEV__ && next_deps.len() != prev_deps.len() {
        console::error((
            concat!(
                "The final argument passed to %s changed size between renders. The ",
                "order and size of this array must remain constant.\n\n",
                "Previous: %s\n",
                "Incoming: %s"
            ),
            CURRENT_HOOK_NAME_IN_DEV.get(),
            &format!("[{}]", js::array::join(next_deps, ", ")),
            &format!("[{}]", js::array::join(prev_deps, ", ")),
        ));
    }
    // $FlowFixMe[incompatible-use] found when upgrading Flow
    let mut i = 0;
    while i < prev_deps.len() && i < next_deps.len() {
        // $FlowFixMe[incompatible-use] found when upgrading Flow
        if OBJECT_IS.with(|is| is(&next_deps[i], &prev_deps[i])) {
            i += 1;
            continue;
        }
        return false;
    }
    true
}

fn create_hook() -> Rc<Hook> {
    if NUMBER_OF_RE_RENDERS.get() > 0 {
        panic!("Rendered more hooks than during the previous render");
    }
    Rc::new(Hook {
        memoized_state: Cell::new(js::null()),
        queue: RefCell::new(None),
        next: RefCell::new(None),
    })
}

fn create_work_in_progress_hook() -> Rc<Hook> {
    let work_in_progress_hook = WORK_IN_PROGRESS_HOOK.with_borrow(Clone::clone);
    let hook = match work_in_progress_hook {
        None => {
            // This is the first hook in the list
            match FIRST_WORK_IN_PROGRESS_HOOK.with_borrow(Clone::clone) {
                None => {
                    IS_RE_RENDER.set(false);
                    let hook = create_hook();
                    FIRST_WORK_IN_PROGRESS_HOOK.set(Some(Rc::clone(&hook)));
                    hook
                }
                Some(first_work_in_progress_hook) => {
                    // There's already a work-in-progress. Reuse it.
                    IS_RE_RENDER.set(true);
                    first_work_in_progress_hook
                }
            }
        }
        Some(work_in_progress_hook) => {
            let next = work_in_progress_hook.next.borrow().clone();
            match next {
                None => {
                    IS_RE_RENDER.set(false);
                    // Append to the end of the list
                    let hook = create_hook();
                    *work_in_progress_hook.next.borrow_mut() = Some(Rc::clone(&hook));
                    hook
                }
                Some(next) => {
                    // There's already a work-in-progress. Reuse it.
                    IS_RE_RENDER.set(true);
                    next
                }
            }
        }
    };
    WORK_IN_PROGRESS_HOOK.set(Some(Rc::clone(&hook)));
    hook
}

#[cfg_attr(rust_js, rust_js::nullable(key_path, prev_thenable_state))]
pub fn prepare_to_use_hooks(
    request: &'static Request,
    task: &'static Task,
    key_path: Option<&'static KeyNode>,
    component_identity: &'static Unknown,
    prev_thenable_state: Option<Rc<ThenableState>>,
) {
    CURRENTLY_RENDERING_COMPONENT.set(Some(component_identity));
    CURRENTLY_RENDERING_TASK.set(Some(task));
    CURRENTLY_RENDERING_REQUEST.set(Some(request));
    CURRENTLY_RENDERING_KEY_PATH.set(key_path);
    if __DEV__ {
        IS_IN_HOOK_USER_CODE_IN_DEV.set(false);
    }

    // The following should have already been reset
    // didScheduleRenderPhaseUpdate = false;
    // firstWorkInProgressHook = null;
    // numberOfReRenders = 0;
    // renderPhaseUpdates = null;
    // workInProgressHook = null;

    LOCAL_ID_COUNTER.set(0.0);
    ACTION_STATE_COUNTER.set(0.0);
    ACTION_STATE_MATCHING_INDEX.set(-1.0);
    THENABLE_INDEX_COUNTER.set(0);
    THENABLE_STATE.set(prev_thenable_state);
}

#[cfg_attr(rust_js, rust_js::nullable(prev_thenable_state))]
pub fn prepare_to_use_thenable_state(prev_thenable_state: Option<Rc<ThenableState>>) {
    THENABLE_INDEX_COUNTER.set(0);
    THENABLE_STATE.set(prev_thenable_state);
}

/// A component Fizz renders with hooks: of its props, and its ref or its
/// legacy context, what it renders.
pub type Component =
    dyn Fn(Option<&'static Unknown>, Option<&'static Unknown>) -> Option<&'static Unknown>;

pub fn finish_hooks(
    component: &Component,
    props: Option<&'static Unknown>,
    mut children: Option<&'static Unknown>,
    ref_or_context: Option<&'static Unknown>,
) -> Option<&'static Unknown> {
    // This must be called after every function component to prevent hooks from
    // being used in classes.

    while DID_SCHEDULE_RENDER_PHASE_UPDATE.get() {
        // Updates were scheduled during the render phase. They are stored in
        // the `renderPhaseUpdates` map. Call the component again, reusing the
        // work-in-progress hooks and applying the additional updates on top. Keep
        // restarting until no more updates are scheduled.
        DID_SCHEDULE_RENDER_PHASE_UPDATE.set(false);
        LOCAL_ID_COUNTER.set(0.0);
        ACTION_STATE_COUNTER.set(0.0);
        ACTION_STATE_MATCHING_INDEX.set(-1.0);
        THENABLE_INDEX_COUNTER.set(0);
        NUMBER_OF_RE_RENDERS.set(NUMBER_OF_RE_RENDERS.get() + 1);

        // Start over from the beginning of the list
        WORK_IN_PROGRESS_HOOK.set(None);

        children = component(props, ref_or_context);
    }

    reset_hooks_state();
    children
}

pub fn get_thenable_state_after_suspending() -> Option<Rc<ThenableState>> {
    THENABLE_STATE.take()
}

pub fn check_did_render_id_hook() -> bool {
    // This should be called immediately after every finishHooks call.
    // Conceptually, it's part of the return value of finishHooks; it's only a
    // separate function to avoid using an array tuple.
    LOCAL_ID_COUNTER.get() != 0.0
}

pub fn get_action_state_count() -> f64 {
    // This should be called immediately after every finishHooks call.
    // Conceptually, it's part of the return value of finishHooks; it's only a
    // separate function to avoid using an array tuple.
    ACTION_STATE_COUNTER.get()
}
pub fn get_action_state_matching_index() -> f64 {
    // This should be called immediately after every finishHooks call.
    // Conceptually, it's part of the return value of finishHooks; it's only a
    // separate function to avoid using an array tuple.
    ACTION_STATE_MATCHING_INDEX.get()
}

// Reset the internal hooks state if an error occurs while rendering a component
pub fn reset_hooks_state() {
    if __DEV__ {
        IS_IN_HOOK_USER_CODE_IN_DEV.set(false);
    }

    CURRENTLY_RENDERING_COMPONENT.set(None);
    CURRENTLY_RENDERING_TASK.set(None);
    CURRENTLY_RENDERING_REQUEST.set(None);
    CURRENTLY_RENDERING_KEY_PATH.set(None);
    DID_SCHEDULE_RENDER_PHASE_UPDATE.set(false);
    FIRST_WORK_IN_PROGRESS_HOOK.set(None);
    NUMBER_OF_RE_RENDERS.set(0);
    RENDER_PHASE_UPDATES.set(None);
    WORK_IN_PROGRESS_HOOK.set(None);
}

fn read_context(context: &Rc<Context>) -> Option<&'static Unknown> {
    if __DEV__ && IS_IN_HOOK_USER_CODE_IN_DEV.get() {
        console::error((concat!(
            "Context can only be read while React is rendering. ",
            "In classes, you can read it in the render method or getDerivedStateFromProps. ",
            "In function components, you can read it directly in the function body, but not ",
            "inside Hooks like useReducer() or useMemo()."
        ),));
    }
    read_context_impl(context)
}

fn use_context(context: &Rc<Context>) -> Option<&'static Unknown> {
    if __DEV__ {
        CURRENT_HOOK_NAME_IN_DEV.set(Some("useContext"));
    }
    resolve_currently_rendering_component();
    read_context_impl(context)
}

fn basic_state_reducer(
    state: Option<&'static Unknown>,
    action: Option<&'static Unknown>,
) -> Option<&'static Unknown> {
    let action: BasicStateAction<Option<&'static Unknown>> = unsafe { js::cast(action) };
    // $FlowFixMe[incompatible-use]: Flow doesn't like mixed types
    match action {
        BasicStateAction::Update(action) => action(state),
        BasicStateAction::State(action) => action,
    }
}

pub fn use_state(initial_state: Option<&'static Unknown>) -> StateAndDispatch {
    if __DEV__ {
        CURRENT_HOOK_NAME_IN_DEV.set(Some("useState"));
    }
    use_reducer(
        &basic_state_reducer,
        // useReducer has a special case to support lazy useState initializers
        initial_state,
        None,
    )
}

pub fn use_reducer(
    reducer: &'static Reducer,
    initial_arg: Option<&'static Unknown>,
    init: Option<&'static Unknown>,
) -> StateAndDispatch {
    if __DEV__ {
        // $FlowFixMe[invalid-compare]
        if !std::ptr::addr_eq(reducer, &basic_state_reducer as &Reducer) {
            CURRENT_HOOK_NAME_IN_DEV.set(Some("useReducer"));
        }
    }
    CURRENTLY_RENDERING_COMPONENT.set(Some(resolve_currently_rendering_component()));
    let work_in_progress_hook = create_work_in_progress_hook();
    if IS_RE_RENDER.get() {
        // This is a re-render. Apply the new render phase updates to the previous
        // current hook.
        let queue = work_in_progress_hook.queue.borrow().clone().unwrap();
        let dispatch = queue.dispatch.get();
        if let Some(render_phase_updates) = RENDER_PHASE_UPDATES.get() {
            // Render phase updates are stored in a map of queue -> linked list
            let first_render_phase_update = render_phase_updates.get(&queue);
            if let Some(first_render_phase_update) = first_render_phase_update {
                // $FlowFixMe[incompatible-use] found when upgrading Flow
                render_phase_updates.delete(&queue);
                // $FlowFixMe[incompatible-use] found when upgrading Flow
                let mut new_state = work_in_progress_hook.memoized_state.get();
                let mut update = Some(first_render_phase_update);
                while let Some(current) = update {
                    // Process this render phase update. We don't have to check the
                    // priority because it will always be the same as the current
                    // render's.
                    let action = current.action;
                    if __DEV__ {
                        IS_IN_HOOK_USER_CODE_IN_DEV.set(true);
                    }
                    new_state = reducer(new_state, action);
                    if __DEV__ {
                        IS_IN_HOOK_USER_CODE_IN_DEV.set(false);
                    }
                    // $FlowFixMe[incompatible-type] we bail out when we get a null
                    update = current.next.borrow().clone();
                }

                // $FlowFixMe[incompatible-use] found when upgrading Flow
                work_in_progress_hook.memoized_state.set(new_state);

                return (new_state, dispatch);
            }
        }
        // $FlowFixMe[incompatible-use] found when upgrading Flow
        (work_in_progress_hook.memoized_state.get(), dispatch)
    } else {
        if __DEV__ {
            IS_IN_HOOK_USER_CODE_IN_DEV.set(true);
        }
        // $FlowFixMe[invalid-compare]
        let initial_state = if std::ptr::addr_eq(reducer, &basic_state_reducer as &Reducer) {
            // Special case for `useState`.
            let initial_arg: InitialState<Option<&'static Unknown>> =
                unsafe { js::cast(initial_arg) };
            match initial_arg {
                InitialState::Init(init) => init(),
                InitialState::State(state) => state,
            }
        } else if !matches!(js::classify(init), Kind::Undefined) {
            let init: &dyn Fn(Option<&'static Unknown>) -> Option<&'static Unknown> =
                unsafe { js::cast(init) };
            init(initial_arg)
        } else {
            initial_arg
        };
        if __DEV__ {
            IS_IN_HOOK_USER_CODE_IN_DEV.set(false);
        }
        // $FlowFixMe[incompatible-use] found when upgrading Flow
        work_in_progress_hook.memoized_state.set(initial_state);
        // $FlowFixMe[incompatible-use] found when upgrading Flow
        let queue = Rc::new(UpdateQueue {
            last: RefCell::new(None),
            dispatch: Cell::new(None),
        });
        *work_in_progress_hook.queue.borrow_mut() = Some(Rc::clone(&queue));
        let dispatch: &'static Dispatch = function::bind(
            dispatch_action,
            Some(&Kind::Null),
            (CURRENTLY_RENDERING_COMPONENT.get(), Rc::clone(&queue)),
        );
        queue.dispatch.set(Some(dispatch));
        // $FlowFixMe[incompatible-use] found when upgrading Flow
        (work_in_progress_hook.memoized_state.get(), Some(dispatch))
    }
}

fn use_memo(next_create: &MemoCreate, deps: Option<Deps>) -> Option<&'static Unknown> {
    CURRENTLY_RENDERING_COMPONENT.set(Some(resolve_currently_rendering_component()));
    let work_in_progress_hook = create_work_in_progress_hook();

    let next_deps = match deps {
        None => js::null(),
        deps => deps,
    };

    let prev_state = work_in_progress_hook.memoized_state.get();
    if !matches!(js::classify(prev_state), Kind::Null)
        && let Some(next_deps) = &next_deps
    {
        let prev_deps = js::get(prev_state, &1usize);
        if are_hook_inputs_equal(next_deps, prev_deps) {
            return js::get(prev_state, &0usize);
        }
    }

    if __DEV__ {
        IS_IN_HOOK_USER_CODE_IN_DEV.set(true);
    }
    let next_value = next_create();
    if __DEV__ {
        IS_IN_HOOK_USER_CODE_IN_DEV.set(false);
    }
    // $FlowFixMe[incompatible-use] found when upgrading Flow
    work_in_progress_hook
        .memoized_state
        .set(Some(js::unknown_of((next_value, next_deps))));
    next_value
}

fn use_ref(initial_value: Option<&'static Unknown>) -> Rc<RefObject> {
    CURRENTLY_RENDERING_COMPONENT.set(Some(resolve_currently_rendering_component()));
    let work_in_progress_hook = create_work_in_progress_hook();
    let previous_ref = work_in_progress_hook.memoized_state.get();
    if matches!(js::classify(previous_ref), Kind::Null) {
        let r#ref = Rc::new(RefObject {
            current: Cell::new(initial_value),
        });
        if __DEV__ {
            js::object::seal(&*r#ref);
        }
        // $FlowFixMe[incompatible-use] found when upgrading Flow
        work_in_progress_hook
            .memoized_state
            .set(Some(js::unknown_of(Rc::clone(&r#ref))));
        r#ref
    } else {
        unsafe { js::cast(previous_ref) }
    }
}

fn dispatch_action(
    component_identity: Option<&'static Unknown>,
    queue: &Rc<UpdateQueue>,
    action: Option<&'static Unknown>,
) {
    if NUMBER_OF_RE_RENDERS.get() >= RE_RENDER_LIMIT {
        panic!(
            "{}",
            concat!(
                "Too many re-renders. React limits the number of renders to prevent ",
                "an infinite loop."
            )
        );
    }

    if js::strict_equals(component_identity, CURRENTLY_RENDERING_COMPONENT.get()) {
        // This is a render phase update. Stash it in a lazily-created map of
        // queue -> linked list of updates. After this render pass, we'll restart
        // and apply the stashed updates on top of the work-in-progress hook.
        DID_SCHEDULE_RENDER_PHASE_UPDATE.set(true);
        let update = Rc::new(Update {
            action,
            next: RefCell::new(None),
        });
        let render_phase_updates = match RENDER_PHASE_UPDATES.get() {
            Some(render_phase_updates) => render_phase_updates,
            None => {
                let render_phase_updates = js::map::new(None);
                RENDER_PHASE_UPDATES.set(Some(render_phase_updates));
                render_phase_updates
            }
        };
        let first_render_phase_update = render_phase_updates.get(queue);
        match first_render_phase_update {
            None => {
                // $FlowFixMe[incompatible-use] found when upgrading Flow
                render_phase_updates.set(queue, update);
            }
            Some(first_render_phase_update) => {
                // Append the update to the end of the list.
                let mut last_render_phase_update = first_render_phase_update;
                loop {
                    let next = last_render_phase_update.next.borrow().clone();
                    let Some(next) = next else { break };
                    last_render_phase_update = next;
                }
                *last_render_phase_update.next.borrow_mut() = Some(update);
            }
        }
    } else {
        // This means an update has happened after the function component has
        // returned. On the server this is a no-op. In React Fiber, the update
        // would be scheduled for a future render.
    }
}

pub fn use_callback(
    callback: Option<&'static Unknown>,
    deps: Option<Deps>,
) -> Option<&'static Unknown> {
    use_memo(&move || callback, deps)
}

thread_local! {
    static THROW_ON_USE_EFFECT_EVENT_CALL: Rc<EventFunction> =
        Rc::new(|_: &dyn Rest<dyn IntoUnknown>| -> Option<&'static Unknown> {
            panic!("A function wrapped in useEffectEvent can't be called during rendering.");
        });
}

pub fn use_effect_event(_callback: &'static EventFunctionImpl) -> Rc<EventFunction> {
    // $FlowFixMe[incompatible-type]
    THROW_ON_USE_EFFECT_EVENT_CALL.with(Rc::clone)
}

fn use_sync_external_store(
    _subscribe: &'static Subscribe,
    _get_snapshot: &'static GetSnapshot,
    get_server_snapshot: Option<&'static Unknown>,
) -> Option<&'static Unknown> {
    if matches!(js::classify(get_server_snapshot), Kind::Undefined) {
        panic!(
            "{}",
            concat!(
                "Missing getServerSnapshot, which is required for ",
                "server-rendered content. Will revert to client rendering."
            )
        );
    }
    let get_server_snapshot: &GetSnapshot = unsafe { js::cast(get_server_snapshot) };
    get_server_snapshot()
}

fn use_deferred_value(
    value: Option<&'static Unknown>,
    initial_value: Option<&'static Unknown>,
) -> Option<&'static Unknown> {
    resolve_currently_rendering_component();
    if !matches!(js::classify(initial_value), Kind::Undefined) {
        initial_value
    } else {
        value
    }
}

fn unsupported_start_transition(
    _callback: &dyn Fn() -> Option<&'static Unknown>,
    _options: Option<&shared::react_types::StartTransitionOptions>,
) {
    panic!("startTransition cannot be called during server rendering.");
}

fn use_transition() -> (bool, Rc<StartTransition>) {
    resolve_currently_rendering_component();
    (false, Rc::new(unsupported_start_transition))
}

fn use_host_transition_status() -> Option<&'static Unknown> {
    resolve_currently_rendering_component();
    NOT_PENDING_TRANSITION
}

fn unsupported_set_optimistic_state(_action: Option<&'static Unknown>) {
    panic!("Cannot update optimistic state while rendering.");
}

fn use_optimistic(
    passthrough: Option<&'static Unknown>,
    _reducer: Option<&'static Unknown>,
) -> StateAndDispatch {
    resolve_currently_rendering_component();
    (passthrough, Some(&unsupported_set_optimistic_state))
}

fn create_postback_action_state_key(
    permalink: Option<&str>,
    component_key_path: Option<&'static KeyNode>,
    hook_index: f64,
) -> String {
    match permalink {
        Some(permalink) => {
            // Don't bother to hash a permalink-based key since it's already short.
            "p".to_owned() + permalink
        }
        None => {
            // Append a node to the key path that represents the form state hook.
            let key_path = (component_key_path, js::null::<&str>(), hook_index);
            // Key paths are hashed to reduce the size. It does not need to be secure,
            // and it's more important that it's fast than that it's completely
            // collision-free.
            let key_path_hash = create_fast_hash(&js::json::stringify(&key_path));
            "k".to_owned() + &key_path_hash
        }
    }
}

/// A server action, which has additional features to enable MPA-style form
/// submissions with progressive enhancement.
struct ServerAction(PhantomData<JsObject>);

impl ServerAction {
    #[cfg_attr(rust_js, rust_js::link_name = "$$FORM_ACTION")]
    fn form_action(&self, prefix: &str) -> &'static ReactCustomFormAction {
        let _ = prefix;
        unreachable!()
    }
}

/// An action, its state bound: of the payload, what the action gives.
type BoundAction = dyn Fn(Option<&'static Unknown>) -> Option<&'static Unknown>;

/// What a server action says of whether a postback's action is it.
type IsSignatureEqual = dyn Fn(Option<&'static Unknown>, f64) -> bool;

fn use_action_state(
    action: &'static Action,
    initial_state: Option<&'static Unknown>,
    permalink: Option<String>,
) -> ActionStateAndDispatch {
    resolve_currently_rendering_component();

    // Count the number of useActionState hooks per component. We also use this to
    // track the position of this useActionState hook relative to the other ones in
    // this component, so we can generate a unique key for each one.
    let action_state_hook_index = ACTION_STATE_COUNTER.get();
    ACTION_STATE_COUNTER.set(action_state_hook_index + 1.0);
    let request = CURRENTLY_RENDERING_REQUEST.get().unwrap();

    // $FlowFixMe[prop-missing]
    let form_action = js::get(js::unknown(action), "$$FORM_ACTION");
    if js::type_of(form_action) == "function" {
        // This is a server action. These have additional features to enable
        // MPA-style form submissions with progressive enhancement.

        // TODO: If the same permalink is passed to multiple useActionStates, and
        // they all have the same action signature, Fizz will pass the postback
        // state to all of them. We should probably only pass it to the first one,
        // and/or warn.

        // The key is lazily generated and deduped so the that the keypath doesn't
        // get JSON.stringify-ed unnecessarily, and at most once.
        let next_postback_state_key: Rc<RefCell<Option<String>>> =
            Rc::new(RefCell::new(js::null()));

        // Determine the current form state. If we received state during an MPA form
        // submission, then we will reuse that, if the action identity matches.
        // Otherwise, we'll use the initial state argument. We will emit a comment
        // marker into the stream that indicates whether the state was reused.
        let mut state = initial_state;
        let component_key_path = CURRENTLY_RENDERING_KEY_PATH.get();
        let postback_action_state = get_form_state(request);
        // $FlowFixMe[prop-missing]
        let is_signature_equal = js::get(js::unknown(action), "$$IS_SIGNATURE_EQUAL");
        if let Some(postback_action_state) = postback_action_state
            && js::type_of(is_signature_equal) == "function"
        {
            let is_signature_equal: &IsSignatureEqual = unsafe { js::cast(is_signature_equal) };
            let postback_key = &postback_action_state.1;
            let postback_reference_id = postback_action_state.2;
            let postback_bound_arity = postback_action_state.3;
            if js::truthy(function::call(
                is_signature_equal,
                Some(&js::unknown(action)),
                (postback_reference_id, postback_bound_arity),
            )) {
                next_postback_state_key.replace(Some(create_postback_action_state_key(
                    permalink.as_deref(),
                    component_key_path,
                    action_state_hook_index,
                )));
                if Some(postback_key.as_str()) == next_postback_state_key.borrow().as_deref() {
                    // This was a match
                    ACTION_STATE_MATCHING_INDEX.set(action_state_hook_index);
                    // Reuse the state that was submitted by the form.
                    state = postback_action_state.0;
                }
            }
        }

        // Bind the state to the first argument of the action.
        let bound_action: &'static BoundAction =
            function::bind(action, Some(&Kind::Null), (state,));

        // Wrap the action so the return value is void.
        let dispatch: &'static ActionDispatch =
            function::with_this(move |_, (payload,): (Option<&'static Unknown>,)| {
                bound_action(payload);
            });

        // $FlowIgnore[prop-missing]
        if js::type_of(js::get(js::unknown(bound_action), "$$FORM_ACTION")) == "function" {
            // $FlowFixMe[prop-missing]
            js::set(
                js::unknown(dispatch),
                "$$FORM_ACTION",
                move |prefix: &str| -> &'static ReactCustomFormAction {
                    let bound_action: &ServerAction =
                        unsafe { js::cast(Some(js::unknown(bound_action))) };
                    let metadata = bound_action.form_action(prefix);

                    // Override the action URL
                    if let Some(permalink) = &permalink {
                        if __DEV__ {
                            check_attribute_string_coercion(
                                Some(js::unknown(permalink.as_str())),
                                "target",
                            );
                        }
                        metadata
                            .action
                            .replace(Some(js::concat_text(permalink.as_str())));
                    }

                    if let Some(form_data) = metadata.data {
                        if next_postback_state_key.borrow().is_none() {
                            next_postback_state_key.replace(Some(
                                create_postback_action_state_key(
                                    permalink.as_deref(),
                                    component_key_path,
                                    action_state_hook_index,
                                ),
                            ));
                        }
                        form_data.append(
                            "$ACTION_KEY",
                            next_postback_state_key.borrow().as_deref().unwrap(),
                        );
                    }
                    metadata
                },
            );
        }

        (state, dispatch, false)
    } else {
        // This is not a server action, so the implementation is much simpler.

        // Bind the state to the first argument of the action.
        let bound_action: &'static BoundAction =
            function::bind(action, Some(&Kind::Null), (initial_state,));
        // Wrap the action so the return value is void.
        let dispatch: &'static ActionDispatch =
            function::with_this(move |_, (payload,): (Option<&'static Unknown>,)| {
                bound_action(payload);
            });
        (initial_state, dispatch, false)
    }
}

fn use_id() -> String {
    let task = CURRENTLY_RENDERING_TASK.get().unwrap();
    let tree_id = get_tree_id(task.tree_context());

    let Some(resumable_state) = CURRENT_RESUMABLE_STATE.get() else {
        panic!(
            "Invalid hook call. Hooks can only be called inside of the body of a function component."
        );
    };

    let local_id = LOCAL_ID_COUNTER.get();
    LOCAL_ID_COUNTER.set(local_id + 1.0);
    make_id(resumable_state, &tree_id, local_id)
}

fn r#use(usable: Option<&'static Unknown>) -> Option<&'static Unknown> {
    // $FlowFixMe[invalid-compare]
    if let Some(usable) = usable
        && js::type_of(Some(usable)) == "object"
    {
        // $FlowFixMe[method-unbinding]
        if js::type_of(js::get(usable, "then")) == "function" {
            // This is a thenable.
            let thenable: &'static Thenable = unsafe { js::cast(Some(usable)) };
            return unwrap_thenable(thenable);
        } else if SymbolFor::from_value(js::get(usable, "$$typeof")) == Some(REACT_RECOVERABLE_TYPE)
        {
            // Create the recoverable error here so its stack captures the component
            // that passed this value to use(). The internal brand lets the renderer
            // distinguish it from an Error thrown by application code.
            let recoverable: &ReactRecoverable = unsafe { js::cast(Some(usable)) };
            js::throw(create_recoverable_error(recoverable).as_unknown());
        } else if SymbolFor::from_value(js::get(usable, "$$typeof")) == Some(REACT_CONTEXT_TYPE) {
            let context: Rc<Context> = unsafe { js::cast(Some(usable)) };
            return read_context(&context);
        }
    }

    // eslint-disable-next-line react-internal/safe-string-coercion
    panic!(
        "An unsupported type was passed to use(): {}",
        js::string(usable)
    );
}

pub fn unwrap_thenable(thenable: &'static Thenable) -> Option<&'static Unknown> {
    let index = THENABLE_INDEX_COUNTER.get();
    THENABLE_INDEX_COUNTER.set(index + 1);
    let thenable_state = THENABLE_STATE.with_borrow_mut(|thenable_state| {
        Rc::clone(thenable_state.get_or_insert_with(|| Rc::new(create_thenable_state())))
    });
    track_used_thenable(&thenable_state, thenable, index)
}

pub fn read_previous_thenable_from_state() -> Option<&'static Unknown> {
    let index = THENABLE_INDEX_COUNTER.get();
    THENABLE_INDEX_COUNTER.set(index + 1);
    let thenable_state = THENABLE_STATE.with_borrow(Clone::clone)?;
    read_previous_thenable(&thenable_state, index)
}

fn unsupported_refresh(_fetch: Option<&'static Unknown>, _seed_key: Option<&'static Unknown>) {
    panic!("Cache cannot be refreshed during server rendering.");
}

fn use_cache_refresh() -> &'static CacheRefresh {
    &unsupported_refresh
}

fn use_memo_cache(size: usize) -> MemoCacheArray {
    Rc::new(RefCell::new(vec![
        Some(js::unknown(
            &REACT_MEMO_CACHE_SENTINEL
        ));
        size
    ]))
}

fn client_hook_not_supported() -> ! {
    panic!(
        "{}",
        concat!(
            "Cannot use state or effect Hooks in renderToHTML because ",
            "this component will never be hydrated."
        )
    );
}

thread_local! {
    // $FlowFixMe[constant-condition]
    #[cfg_attr(rust_js, rust_js::name = "HooksDispatcher")]
    pub static HOOKS_DISPATCHER: Dispatcher = if SUPPORTS_CLIENT_APIS {
        Dispatcher {
            read_context,
            r#use,
            use_context,
            use_memo,
            use_reducer,
            use_ref,
            use_state,
            use_insertion_effect: |_, _| noop(),
            use_layout_effect: |_, _| noop(),
            use_callback,
            // useImperativeHandle is not run in the server environment
            use_imperative_handle: |_, _, _| noop(),
            // Effects are not run in the server environment.
            use_effect: |_, _| noop(),
            // Debugging effect
            use_debug_value: |_, _| noop(),
            use_deferred_value,
            use_transition,
            use_id,
            // Subscriptions are not setup in a server environment.
            use_sync_external_store,
            use_optimistic,
            use_action_state,
            use_form_state: use_action_state,
            use_host_transition_status,
            use_memo_cache,
            use_cache_refresh,
            use_effect_event,
        }
    } else {
        Dispatcher {
            read_context,
            r#use,
            use_callback,
            use_context,
            use_effect: |_, _| client_hook_not_supported(),
            use_imperative_handle: |_, _, _| client_hook_not_supported(),
            use_insertion_effect: |_, _| client_hook_not_supported(),
            use_layout_effect: |_, _| client_hook_not_supported(),
            use_memo,
            use_reducer: |_, _, _| client_hook_not_supported(),
            use_ref: |_| client_hook_not_supported(),
            use_state: |_| client_hook_not_supported(),
            use_debug_value: |_, _| noop(),
            use_deferred_value: |_, _| client_hook_not_supported(),
            use_transition: || client_hook_not_supported(),
            use_sync_external_store: |_, _, _| client_hook_not_supported(),
            use_id,
            use_host_transition_status,
            use_form_state: use_action_state,
            use_action_state,
            use_optimistic,
            use_memo_cache,
            use_cache_refresh,
            use_effect_event,
        }
    };
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub static CURRENT_RESUMABLE_STATE: Cell<Option<&'static ResumableState>> = const { Cell::new(None) };
}

#[cfg_attr(rust_js, rust_js::nullable(resumable_state))]
pub fn set_current_resumable_state(resumable_state: Option<&'static ResumableState>) {
    CURRENT_RESUMABLE_STATE.set(resumable_state);
}
