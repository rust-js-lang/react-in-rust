// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js::{Error, JsObject, Kind, Map, Unknown};
use shared::environment::__DEV__;
use shared::react_types::ReactStackTrace;
use webapi::performance;

use crate::react_flight_async_sequence::{
    AWAIT_NODE, AsyncSequence, IO_NODE, PROMISE_NODE, UNRESOLVED_AWAIT_NODE,
    UNRESOLVED_PROMISE_NODE,
};
use crate::react_flight_current_owner::resolve_owner;
use crate::react_flight_hooks::{Request, resolve_request};

use node::async_hooks::{self, HookCallbacks, async_resource, execution_async_id};

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightServer.js#isAwaitInUserspace"
)]
fn is_await_in_userspace(request: &Request, stack: &ReactStackTrace) -> bool {
    let _ = (request, stack);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "./src/ReactFlightServerConfig.js#parseStackTracePrivate"
)]
#[cfg_attr(rust_js, rust_js::nullable)]
fn parse_stack_trace_private(error: &Error, skip_frames: usize) -> Option<Rc<ReactStackTrace>> {
    let _ = (error, skip_frames);
    unreachable!()
}

thread_local! {
    static PENDING_OPERATIONS: &'static Map<f64, Rc<AsyncSequence>> = js::map::new(None);

    // Keep the last resolved await as a workaround for async functions missing data.
    #[cfg_attr(rust_js, rust_js::nullable)]
    static LAST_RAN_AWAIT: RefCell<Option<Rc<AsyncSequence>>> = const { RefCell::new(None) };
}

fn resolve_promise_or_await_node(
    unresolved_node: &Rc<AsyncSequence>,
    end_time: f64,
) -> Rc<AsyncSequence> {
    let resolved_node = unresolved_node;
    resolved_node
        .tag
        .set(if unresolved_node.tag.get() == UNRESOLVED_PROMISE_NODE {
            PROMISE_NODE
        } else {
            AWAIT_NODE
        });
    resolved_node.end.set(end_time);
    Rc::clone(resolved_node)
}

thread_local! {
    static EMPTY_STACK: Rc<ReactStackTrace> = Rc::new(Vec::new());
}

// Initialize the tracing of async operations.
// We do this globally since the async work can potentially eagerly
// start before the first request and once requests start they can interleave.
// In theory we could enable and disable using a ref count of active requests
// but given that typically this is just a live server, it doesn't really matter.
pub fn init_async_debug_info() {
    if __DEV__ {
        async_hooks::create_hook(&HookCallbacks {
            init: Some(&|async_id: f64,
                         r#type: &str,
                         trigger_async_id: f64,
                         resource: &'static JsObject| {
                let trigger = PENDING_OPERATIONS
                    .with(|pending_operations| pending_operations.get(&trigger_async_id));
                let node: Rc<AsyncSequence>;
                if r#type == "PROMISE" {
                    let current_async_id = execution_async_id();
                    if current_async_id != trigger_async_id {
                        // When you call .then() on a native Promise, or await/Promise.all() a thenable,
                        // then this intermediate Promise is created. We use this as our await point
                        let Some(trigger) = trigger else {
                            // We don't track awaits on things that started outside our tracked scope.
                            return;
                        };
                        // If the thing we're waiting on is another Await we still track that sequence
                        // so that we can later pick the best stack trace in user space.
                        let mut stack = js::null();
                        let promise_ref;
                        if trigger.stack.is_some()
                            && (trigger.tag.get() == AWAIT_NODE
                                || trigger.tag.get() == UNRESOLVED_AWAIT_NODE)
                        {
                            // We already had a stack for an await. In a chain of awaits we'll only need one good stack.
                            // We mark it with an empty stack to signal to any await on this await that we have a stack.
                            stack = Some(EMPTY_STACK.with(Rc::clone));
                            if !matches!(
                                js::classify(js::get(resource, "_debugInfo")),
                                Kind::Undefined
                            ) {
                                // We may need to forward this debug info at the end so we need to retain this promise.
                                promise_ref =
                                    Some(js::weak_ref::new(unsafe { js::cast(Some(&**resource)) }));
                            } else {
                                // Otherwise, we can just refer to the inner one since that's the one we'll log anyway.
                                promise_ref = trigger.promise;
                            }
                        } else {
                            promise_ref =
                                Some(js::weak_ref::new(unsafe { js::cast(Some(&**resource)) }));
                            let request = resolve_request();
                            match request {
                                None => {
                                    // We don't collect stacks for awaits that weren't in the scope of a specific render.
                                }
                                Some(request) => {
                                    stack =
                                        parse_stack_trace_private(js::error::new(None, None), 5);
                                    if let Some(trace) = &stack
                                        && !is_await_in_userspace(request, trace)
                                    {
                                        // If this await was not done directly in user space, then clear the stack. We won't use it
                                        // anyway. This lets future awaits on this await know that we still need to get their stacks
                                        // until we find one in user space.
                                        stack = js::null();
                                    }
                                }
                            }
                        }
                        let current = PENDING_OPERATIONS
                            .with(|pending_operations| pending_operations.get(&current_async_id));
                        node = Rc::new(AsyncSequence {
                            tag: Cell::new(UNRESOLVED_AWAIT_NODE),
                            owner: resolve_owner(),
                            stack,
                            start: Cell::new(performance.now()),
                            end: Cell::new(-1.1), // set when resolved.
                            promise: promise_ref,
                            // GAP rd567: a field's comment is lost where its value is a `let`-`else`'s binding.
                            awaited: RefCell::new(Some(trigger)), // The thing we're awaiting on. Might get overrriden when we resolve.
                            previous: RefCell::new(current),      // The path that led us here.
                        });
                    } else {
                        let owner = resolve_owner();
                        node = Rc::new(AsyncSequence {
                            tag: Cell::new(UNRESOLVED_PROMISE_NODE),
                            owner,
                            stack: if owner.is_none() {
                                None
                            } else {
                                parse_stack_trace_private(js::error::new(None, None), 5)
                            },
                            start: Cell::new(performance.now()),
                            end: Cell::new(-1.1), // Set when we resolve.
                            promise: Some(js::weak_ref::new(unsafe {
                                js::cast(Some(&**resource))
                            })),
                            awaited: RefCell::new(trigger), // It might get overridden when we resolve.
                            previous: RefCell::new(None),
                        });
                    }
                } else if
                // bound-anonymous-fn is the default name for snapshots and .bind() without a name.
                // This isn't I/O by itself but likely just a continuation. If the bound function
                // has a name, we might treat it as I/O but we can't tell the difference.
                r#type == "bound-anonymous-fn"
                // queueMicroTask, process.nextTick and setImmediate aren't considered new I/O
                // for our purposes but just continuation of existing I/O.
                || r#type == "Microtask"
                || r#type == "TickObject"
                || r#type == "Immediate"
                {
                    // Treat the trigger as the node to carry along the sequence.
                    // For "bound-anonymous-fn" this will be the callsite of the .bind() which may not
                    // be the best if the callsite of the .run() call is within I/O which should be
                    // tracked. It might be better to track the execution context of "before()" as the
                    // execution context for anything spawned from within the run(). Basically as if
                    // it wasn't an AsyncResource at all.
                    let Some(trigger) = trigger else {
                        return;
                    };
                    node = trigger;
                } else {
                    // New I/O
                    match trigger {
                        None => {
                            // We have begun a new I/O sequence.
                            let owner = resolve_owner();
                            node = Rc::new(AsyncSequence {
                                tag: Cell::new(IO_NODE),
                                owner,
                                stack: if owner.is_none() {
                                    parse_stack_trace_private(js::error::new(None, None), 3)
                                } else {
                                    None
                                },
                                start: Cell::new(performance.now()),
                                end: Cell::new(-1.1), // Only set when pinged.
                                promise: None,
                                awaited: RefCell::new(None),
                                previous: RefCell::new(None),
                            });
                        }
                        Some(trigger) => {
                            if trigger.tag.get() == AWAIT_NODE
                                || trigger.tag.get() == UNRESOLVED_AWAIT_NODE
                            {
                                // We have begun a new I/O sequence after the await.
                                let owner = resolve_owner();
                                node = Rc::new(AsyncSequence {
                                    tag: Cell::new(IO_NODE),
                                    owner,
                                    stack: if owner.is_none() {
                                        parse_stack_trace_private(js::error::new(None, None), 3)
                                    } else {
                                        None
                                    },
                                    start: Cell::new(performance.now()),
                                    end: Cell::new(-1.1), // Only set when pinged.
                                    promise: None,
                                    awaited: RefCell::new(None),
                                    previous: RefCell::new(Some(trigger)),
                                });
                            } else {
                                // Otherwise, this is just a continuation of the same I/O sequence.
                                node = trigger;
                            }
                        }
                    }
                }
                PENDING_OPERATIONS
                    .with(|pending_operations| pending_operations.set(&async_id, node));
            }),
            before: Some(&|async_id: f64| {
                let node =
                    PENDING_OPERATIONS.with(|pending_operations| pending_operations.get(&async_id));
                if let Some(node) = node {
                    match node.tag.get() {
                        IO_NODE => {
                            LAST_RAN_AWAIT.set(None);
                            // Log the end time when we resolved the I/O.
                            let io_node = &node;
                            if io_node.end.get() < 0.0 {
                                io_node.end.set(performance.now());
                            } else {
                                // This can happen more than once if it's a recurring resource like a connection.
                                // Even for single events like setTimeout, this can happen three times due to ticks
                                // and microtasks each running its own scope.
                                // To preserve each operation's separate end time, we create a clone of the IO node.
                                // Any pre-existing reference will refer to the first resolution and any new resolutions
                                // will refer to the new node.
                                let cloned_node = Rc::new(AsyncSequence {
                                    tag: Cell::new(IO_NODE),
                                    owner: io_node.owner,
                                    stack: io_node.stack.clone(),
                                    start: io_node.start.clone(),
                                    end: Cell::new(performance.now()),
                                    promise: io_node.promise,
                                    // GAP rd565: a `RefCell` field's `clone()`, a moment's
                                    // borrow, is counted, `$withBorrow($fieldCell(..))`,
                                    // where upstream reads `ioNode.awaited`.
                                    awaited: io_node.awaited.clone(),
                                    previous: io_node.previous.clone(),
                                });
                                PENDING_OPERATIONS.with(|pending_operations| {
                                    pending_operations.set(&async_id, cloned_node)
                                });
                            }
                        }
                        UNRESOLVED_AWAIT_NODE => {
                            // If we begin before we resolve, that means that this is actually already resolved but
                            // the promiseResolve hook is called at the end of the execution. So we track the time
                            // in the before call instead.
                            // $FlowFixMe[incompatible-type]
                            LAST_RAN_AWAIT.set(Some(resolve_promise_or_await_node(
                                &node,
                                performance.now(),
                            )));
                        }
                        AWAIT_NODE => {
                            LAST_RAN_AWAIT.set(Some(node));
                        }
                        UNRESOLVED_PROMISE_NODE => {
                            // We typically don't expected Promises to have an execution scope since only the awaits
                            // have a then() callback. However, this can happen for native async functions. The last
                            // piece of code that executes the return after the last await has the execution context
                            // of the Promise.
                            let resolved_node =
                                resolve_promise_or_await_node(&node, performance.now());
                            // We are missing information about what this was unblocked by but we can guess that it
                            // was whatever await we ran last since this will continue in a microtask after that.
                            // This is not perfect because there could potentially be other microtasks getting in
                            // between.
                            // GAP rd566: a field given a cell's `take()` is `const old =
                            // lastRanAwait; lastRanAwait = null; resolvedNode.previous = old`,
                            // where upstream assigns it and then clears it.
                            *resolved_node.previous.borrow_mut() = LAST_RAN_AWAIT.take();
                        }
                        _ => {
                            LAST_RAN_AWAIT.set(None);
                        }
                    }
                }
            }),
            promise_resolve: Some(&|async_id: f64| {
                let node =
                    PENDING_OPERATIONS.with(|pending_operations| pending_operations.get(&async_id));
                if let Some(node) = node {
                    let resolved_node = match node.tag.get() {
                        UNRESOLVED_AWAIT_NODE | UNRESOLVED_PROMISE_NODE => {
                            resolve_promise_or_await_node(&node, performance.now())
                        }
                        AWAIT_NODE | PROMISE_NODE => {
                            // We already resolved this in the before hook.
                            node
                        }
                        _ => {
                            // eslint-disable-next-line react-internal/prod-error-codes
                            panic!("A Promise should never be an IO_NODE. This is a bug in React.")
                        }
                    };
                    let current_async_id = execution_async_id();
                    if async_id != current_async_id {
                        // If the promise was not resolved by itself, then that means that
                        // the trigger that we originally stored wasn't actually the dependency.
                        // Instead, the current execution context is what ultimately unblocked it.
                        let awaited = PENDING_OPERATIONS
                            .with(|pending_operations| pending_operations.get(&current_async_id));
                        if resolved_node.tag.get() == PROMISE_NODE {
                            // For a Promise we just override the await. We're not interested in
                            // what created the Promise itself.
                            *resolved_node.awaited.borrow_mut() = awaited;
                        } else {
                            // For an await, there's really two things awaited here. It's the trigger
                            // that .then() was called on but there seems to also be something else
                            // in the .then() callback that blocked the returned Promise from resolving
                            // immediately. We create a fork node which essentially represents an await
                            // of the Promise returned from the .then() callback. That Promise was blocked
                            // on the original awaited thing which we stored as "previous".
                            if let Some(awaited) = awaited {
                                let cloned_node = Rc::new(AsyncSequence {
                                    tag: Cell::new(AWAIT_NODE),
                                    owner: resolved_node.owner,
                                    stack: resolved_node.stack.clone(),
                                    start: resolved_node.start.clone(),
                                    end: resolved_node.end.clone(),
                                    promise: resolved_node.promise,
                                    // GAP rd565: as before's.
                                    awaited: resolved_node.awaited.clone(),
                                    previous: resolved_node.previous.clone(),
                                });
                                // We started awaiting on the callback when the original .then() resolved.
                                resolved_node.start.set(resolved_node.end.get());
                                // It resolved now. We could use the end time of "awaited" maybe.
                                resolved_node.end.set(performance.now());
                                *resolved_node.previous.borrow_mut() = Some(cloned_node);
                                *resolved_node.awaited.borrow_mut() = Some(awaited);
                            }
                        }
                    }
                }
            }),
            destroy: Some(&|async_id: f64| {
                // If we needed the meta data from this operation we should have already
                // extracted it or it should be part of a chain of triggers.
                PENDING_OPERATIONS.with(|pending_operations| pending_operations.delete(&async_id));
            }),
            ..Default::default()
        })
        .enable();
    }
}

pub fn mark_async_sequence_root_task() {
    if __DEV__ {
        // Whatever Task we're running now is spawned by React itself to perform render work.
        // Don't track any cause beyond this task. We may still track I/O that was started outside
        // React but just not the cause of entering the render.
        PENDING_OPERATIONS
            .with(|pending_operations| pending_operations.delete(&execution_async_id()));
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_current_async_sequence() -> Option<Rc<AsyncSequence>> {
    if !__DEV__ {
        return None;
    }
    let current_node =
        PENDING_OPERATIONS.with(|pending_operations| pending_operations.get(&execution_async_id()));
    let Some(current_node) = current_node else {
        // Nothing that we tracked led to the resolution of this execution context.
        return None;
    };
    Some(current_node)
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_async_sequence_from_promise(promise: &'static Unknown) -> Option<Rc<AsyncSequence>> {
    if !__DEV__ {
        return None;
    }
    // A Promise is conceptually an AsyncResource but doesn't have its own methods.
    // We use this hack to extract the internal asyncId off the Promise.
    // Ignore errors extracting the ID. We treat it as missing.
    // This could happen if our hack stops working or in the case where this is
    // a Proxy that throws such as our own ClientReference proxies.
    // GAP rd568: upstream reads `AsyncResource.prototype.asyncId` once, `const
    // getAsyncId`, and calls `getAsyncId.call(promise)`: the binding is the
    // whole path, read at each call.
    // GAP rd569: the `Result` of a throwing call made its default is
    // `$try(() => ..)` and its `TAG`, where a `match` of it, which clippy
    // asks to be this, is upstream's `try { .. } catch (x) { .. }`.
    let async_id = async_resource::async_id_of(promise).unwrap_or_default()?;
    let node = PENDING_OPERATIONS.with(|pending_operations| pending_operations.get(&async_id))?;
    Some(node)
}
