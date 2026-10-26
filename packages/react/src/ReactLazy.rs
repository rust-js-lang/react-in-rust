// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, OnceCell, RefCell};
use std::marker::PhantomData;
use std::rc::Rc;

use js::{Error, JsObject, Kind, Promise, SymbolFor, Unknown};
use shared::environment::__DEV__;
use shared::noop::noop;
use shared::react_symbols::REACT_LAZY_TYPE;
use webapi::{ConsoleTask, console, performance};

#[derive(Clone, Copy, PartialEq)]
#[repr(i8)]
enum Status {
    Uninitialized = -1,
    Pending = 0,
    Resolved = 1,
    Rejected = 2,
}

/// A thenable, `shared/ReactTypes`' `Thenable`: what a lazy component's
/// loader gives, which React makes introspectable as it settles.
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

/// `shared/ReactTypes`' `ReactIOInfo`: the I/O a lazy component waits on.
pub struct ReactIOInfo {
    pub name: Cell<&'static str>,
    pub start: Cell<f64>,
    pub end: Cell<f64>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub value: Cell<Option<Promise<Option<&'static Unknown>>>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub owner: Option<&'static Unknown>,
    pub debug_stack: &'static Error,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub debug_task: Option<&'static ConsoleTask>,
}

/// `shared/ReactTypes`' `ReactAsyncInfo`: what a lazy component awaited.
pub struct ReactAsyncInfo {
    pub awaited: Rc<ReactIOInfo>,
}

/// Where a lazy component is in loading what it's of: its `_status`, and
/// its `_result`, the loader, the thenable it gave, the module it resolved
/// to or what it was rejected with.
pub struct Payload {
    #[cfg_attr(rust_js, rust_js::name = "_status")]
    status: Cell<Status>,
    #[cfg_attr(rust_js, rust_js::name = "_result")]
    result: Cell<Option<&'static Unknown>>,
    // DEV-only
    #[cfg_attr(rust_js, rust_js::name = "_ioInfo")]
    io_info: OnceCell<Rc<ReactIOInfo>>,
}

pub struct LazyComponent {
    #[cfg_attr(rust_js, rust_js::name = "$$typeof")]
    pub r#typeof: SymbolFor<'static>,
    #[cfg_attr(rust_js, rust_js::name = "_payload")]
    pub payload: Rc<Payload>,
    #[cfg_attr(rust_js, rust_js::name = "_init")]
    pub init: fn(&Rc<Payload>) -> Option<&'static Unknown>,

    // __DEV__
    #[cfg_attr(rust_js, rust_js::name = "_debugInfo")]
    pub debug_info: RefCell<Option<Vec<ReactAsyncInfo>>>,
}

fn lazy_initializer(payload: &Rc<Payload>) -> Option<&'static Unknown> {
    if payload.status.get() == Status::Uninitialized {
        let mut resolve_debug_value = None;
        let mut reject_debug_value = None;
        if __DEV__ && let Some(io_info) = payload.io_info.get() {
            // Mark when we first kicked off the lazy request.
            // $FlowFixMe[cannot-write]
            let now = performance.now();
            io_info.start.set(now);
            io_info.end.set(now);
            // Stash a Promise for introspection of the value later.
            // $FlowFixMe[cannot-write]
            io_info.value.set(Some(js::promise::new(|resolve, reject| {
                resolve_debug_value = Some(resolve);
                reject_debug_value = Some(reject);
            })));
        }
        let ctor: &dyn Fn() -> &'static Thenable = unsafe { js::cast(payload.result.get()) };
        let thenable = ctor();
        // Transition to the next state.
        // This might throw either because it's missing or throws. If so, we treat it
        // as still uninitialized and try again next time. Which is the same as what
        // happens if the ctor or any wrappers processing the ctor throws. This might
        // end up fixing it if the resolution was a concurrency bug.
        let resolved = Rc::clone(payload);
        let rejected = Rc::clone(payload);
        thenable.then(
            move |module_object| {
                if matches!(
                    resolved.status.get(),
                    Status::Pending | Status::Uninitialized
                ) {
                    // Transition to the next state.
                    resolved.status.set(Status::Resolved);
                    resolved.result.set(module_object);
                    if __DEV__ && let Some(io_info) = resolved.io_info.get() {
                        // Mark the end time of when we resolved.
                        // $FlowFixMe[cannot-write]
                        io_info.end.set(performance.now());
                        // Surface the default export as the resolved "value" for debug purposes.
                        let debug_value = module_object
                            .and_then(|module_object| js::get(module_object, "default"));
                        resolve_debug_value.unwrap()(debug_value);
                        // $FlowFixMe[incompatible-use]
                        // $FlowFixMe[prop-missing]
                        js::set(io_info.value.get(), "status", "fulfilled");
                        // $FlowFixMe[incompatible-use]
                        // $FlowFixMe[prop-missing]
                        js::set(io_info.value.get(), "value", debug_value);
                    }
                    // Make the thenable introspectable
                    // TODO we should move the lazy introspection into the resolveLazy
                    // impl or make suspendedThenable be able to be a lazy itself
                    if js::strict_equals(thenable.status(), None::<&Unknown>) {
                        thenable.set_status("fulfilled");
                        thenable.set_value(module_object);
                    }
                }
            },
            move |error| {
                if matches!(
                    rejected.status.get(),
                    Status::Pending | Status::Uninitialized
                ) {
                    // Transition to the next state.
                    rejected.status.set(Status::Rejected);
                    rejected.result.set(error);
                    if __DEV__ && let Some(io_info) = rejected.io_info.get() {
                        // Mark the end time of when we rejected.
                        // $FlowFixMe[cannot-write]
                        io_info.end.set(performance.now());
                        // Hide unhandled rejections.
                        // $FlowFixMe[incompatible-use]
                        let _: Promise<()> = io_info
                            .value
                            .get()
                            .unwrap()
                            .then_with(|_| noop(), |_| noop());
                        reject_debug_value.unwrap()(error);
                        // $FlowFixMe[incompatible-use]
                        // $FlowFixMe[prop-missing]
                        js::set(io_info.value.get(), "status", "rejected");
                        // $FlowFixMe[incompatible-use]
                        // $FlowFixMe[prop-missing]
                        js::set(io_info.value.get(), "reason", error);
                    }
                    // Make the thenable introspectable
                    // TODO we should move the lazy introspection into the resolveLazy
                    // impl or make suspendedThenable be able to be a lazy itself
                    if js::strict_equals(thenable.status(), None::<&Unknown>) {
                        thenable.set_status("rejected");
                        thenable.set_reason(error);
                    }
                }
            },
        );
        if __DEV__
            && let Some(io_info) = payload.io_info.get()
            && let Kind::String(display_name) = js::classify(thenable.display_name())
        {
            // $FlowFixMe[cannot-write]
            io_info.name.set(display_name);
        }
        if payload.status.get() == Status::Uninitialized {
            // In case, we're still uninitialized, then we're waiting for the thenable
            // to resolve. Set it as pending in the meantime.
            payload.status.set(Status::Pending);
            payload.result.set(Some(js::unknown(thenable)));
        }
    }
    if payload.status.get() == Status::Resolved {
        let module_object = payload.result.get();
        if __DEV__ && js::strict_equals(module_object, None::<&Unknown>) {
            console::error((
                concat!(
                    "lazy: Expected the result of a dynamic imp",
                    "ort() call. ",
                    "Instead received: %s\n\nYour code should look like: \n  ",
                    // Break up imports to avoid accidentally parsing them as dependencies.
                    "const MyComponent = lazy(() => imp",
                    "ort('./MyComponent'))\n\n",
                    "Did you accidentally put curly braces around the import?"
                ),
                module_object,
            ));
        }
        if __DEV__ && !js::has(module_object, "default") {
            console::error((
                concat!(
                    "lazy: Expected the result of a dynamic imp",
                    "ort() call. ",
                    "Instead received: %s\n\nYour code should look like: \n  ",
                    // Break up imports to avoid accidentally parsing them as dependencies.
                    "const MyComponent = lazy(() => imp",
                    "ort('./MyComponent'))"
                ),
                module_object,
            ));
        }
        js::get(module_object, "default")
    } else {
        js::throw(payload.result.get())
    }
}

pub fn lazy(ctor: &'static dyn Fn() -> &'static Thenable) -> LazyComponent {
    let payload = Rc::new(Payload {
        // We use these fields to store the result.
        status: Cell::new(Status::Uninitialized),
        result: Cell::new(Some(js::unknown(ctor))),
        io_info: OnceCell::new(),
    });

    let lazy_type = LazyComponent {
        r#typeof: REACT_LAZY_TYPE,
        payload: Rc::clone(&payload),
        init: lazy_initializer,
        debug_info: RefCell::new(None),
    };

    if __DEV__ {
        // TODO: We should really track the owner here but currently ReactIOInfo
        // can only contain ReactComponentInfo and not a Fiber. It's unusual to
        // create a lazy inside an owner though since they should be in module scope.
        let owner = None;
        let io_info = Rc::new(ReactIOInfo {
            name: Cell::new("lazy"),
            start: Cell::new(-1.0),
            end: Cell::new(-1.0),
            value: Cell::new(None),
            owner,
            debug_stack: js::error::new("react-stack-top-frame"),
            // eslint-disable-next-line react-internal/no-production-logging
            debug_task: console::create_task().map(|create_task| create_task("lazy()")),
        });
        let _ = payload.io_info.set(Rc::clone(&io_info));
        // Add debug info to the lazy, but this doesn't have an await stack yet.
        // That will be inferred by later usage.
        lazy_type
            .debug_info
            .replace(Some(vec![ReactAsyncInfo { awaited: io_info }]));
    }

    lazy_type
}
