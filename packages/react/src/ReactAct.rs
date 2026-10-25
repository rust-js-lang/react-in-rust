// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::panic::{self, AssertUnwindSafe};
use std::rc::Rc;

use js::{JsError, Unknown, aggregate_error};
use shared::enqueue_task::enqueue_task as queue_macrotask;
use shared::environment::__DEV__;
use shared::react_feature_flags::DISABLE_LEGACY_MODE;
use webapi::console;

use crate::react_lazy::Thenable;
use crate::react_shared_internals_client::{ActQueue, REACT_SHARED_INTERNALS, RendererTask};

// `act` calls can be nested, so we track the depth. This represents the
// number of `act` scopes on the stack.
thread_local! {
    static ACT_SCOPE_DEPTH: Cell<f64> = const { Cell::new(0.0) };
}

// We only warn the first time you neglect to await an async `act` scope.
thread_local! {
    static DID_WARN_NO_AWAIT_ACT: Cell<bool> = const { Cell::new(false) };
}

fn aggregate_errors(errors: &[Option<&'static Unknown>]) -> Option<&'static Unknown> {
    if errors.len() > 1 && js::is_defined(aggregate_error::new) {
        return aggregate_error::new(errors, None).as_unknown();
    }
    errors.first().copied().flatten()
}

/// What a thenable's `then` is given: what it calls with the value it
/// resolves to, or with what it rejects with.
type Settle = dyn Fn(Option<&'static Unknown>);

/// The thenable `act` gives, awaited to finish its scope.
pub struct ActThenable {
    pub then: Box<dyn Fn(&'static Settle, &'static Settle)>,
}

pub fn act(callback: &dyn Fn() -> Option<&'static Unknown>) -> ActThenable {
    if __DEV__ {
        // When ReactSharedInternals.actQueue is not null, it signals to React that
        // we're currently inside an `act` scope. React will push all its tasks to
        // this queue instead of scheduling them with platform APIs.
        //
        // We set this to an empty array when we first enter an `act` scope, and
        // only unset it once we've left the outermost `act` scope — remember that
        // `act` calls can be nested.
        //
        // If we're already inside an `act` scope, reuse the existing queue.
        let prev_is_batching_legacy = if !DISABLE_LEGACY_MODE {
            REACT_SHARED_INTERNALS.with(|react_shared_internals| {
                react_shared_internals.is_batching_legacy.get().unwrap()
            })
        } else {
            false
        };
        let prev_act_queue = REACT_SHARED_INTERNALS
            .with(|react_shared_internals| react_shared_internals.act_queue.borrow().clone());
        let prev_act_scope_depth = ACT_SCOPE_DEPTH.get();
        ACT_SCOPE_DEPTH.set(ACT_SCOPE_DEPTH.get() + 1.0);
        let queue = prev_act_queue
            .clone()
            .unwrap_or_else(|| Rc::new(RefCell::new(Vec::new())));
        REACT_SHARED_INTERNALS.with(|react_shared_internals| {
            react_shared_internals
                .act_queue
                .replace(Some(Rc::clone(&queue)));
        });
        // Used to reproduce behavior of `batchedUpdates` in legacy mode. Only
        // set to `true` while the given callback is executed, not for updates
        // triggered during an async event, because this is how the legacy
        // implementation of `act` behaved.
        if !DISABLE_LEGACY_MODE {
            REACT_SHARED_INTERNALS.with(|react_shared_internals| {
                react_shared_internals.is_batching_legacy.set(Some(true));
            });
        }

        let mut result = None;
        // This tracks whether the `act` call is awaited. In certain cases, not
        // awaiting it is a mistake, so we will detect that and warn.
        let did_await_act_call = Rc::new(Cell::new(false));
        if let Err(error) = panic::catch_unwind(AssertUnwindSafe(|| {
            // Reset this to `false` right before entering the React work loop. The
            // only place we ever read this fields is just below, right after running
            // the callback. So we don't need to reset after the callback runs.
            if !DISABLE_LEGACY_MODE {
                REACT_SHARED_INTERNALS.with(|react_shared_internals| {
                    react_shared_internals
                        .did_schedule_legacy_update
                        .set(Some(false));
                });
            }
            result = callback();
            let did_schedule_legacy_update = if !DISABLE_LEGACY_MODE {
                REACT_SHARED_INTERNALS.with(|react_shared_internals| {
                    react_shared_internals
                        .did_schedule_legacy_update
                        .get()
                        .unwrap()
                })
            } else {
                false
            };

            // Replicate behavior of original `act` implementation in legacy mode,
            // which flushed updates immediately after the scope function exits, even
            // if it's an async function.
            if !prev_is_batching_legacy && did_schedule_legacy_update {
                flush_act_queue(&queue);
            }
            // `isBatchingLegacy` gets reset using the regular stack, not the async
            // one used to track `act` scopes. Why, you may be wondering? Because
            // that's how it worked before version 18. Yes, it's confusing! We should
            // delete legacy mode!!
            if !DISABLE_LEGACY_MODE {
                REACT_SHARED_INTERNALS.with(|react_shared_internals| {
                    react_shared_internals
                        .is_batching_legacy
                        .set(Some(prev_is_batching_legacy));
                });
            }
        })) {
            // `isBatchingLegacy` gets reset using the regular stack, not the async
            // one used to track `act` scopes. Why, you may be wondering? Because
            // that's how it worked before version 18. Yes, it's confusing! We should
            // delete legacy mode!!
            REACT_SHARED_INTERNALS.with(|react_shared_internals| {
                react_shared_internals
                    .thrown_errors
                    .borrow_mut()
                    .as_mut()
                    .unwrap()
                    .push(
                        error
                            .downcast_ref::<&'static JsError>()
                            .and_then(|error| error.as_unknown()),
                    );
            });
        }
        if REACT_SHARED_INTERNALS.with(|react_shared_internals| {
            react_shared_internals
                .thrown_errors
                .borrow()
                .as_ref()
                .unwrap()
                .len()
        }) > 0
        {
            if !DISABLE_LEGACY_MODE {
                REACT_SHARED_INTERNALS.with(|react_shared_internals| {
                    react_shared_internals
                        .is_batching_legacy
                        .set(Some(prev_is_batching_legacy));
                });
            }
            pop_act_scope(prev_act_queue.as_ref(), prev_act_scope_depth);
            let thrown_error = REACT_SHARED_INTERNALS.with(|react_shared_internals| {
                aggregate_errors(
                    react_shared_internals
                        .thrown_errors
                        .borrow()
                        .as_ref()
                        .unwrap(),
                )
            });
            REACT_SHARED_INTERNALS.with(|react_shared_internals| {
                react_shared_internals
                    .thrown_errors
                    .borrow_mut()
                    .as_mut()
                    .unwrap()
                    .clear();
            });
            js::throw(thrown_error);
        }

        if
        // $FlowFixMe[invalid-compare]
        let Some(result) = result
            && js::type_of(Some(result)) == "object"
            // $FlowFixMe[method-unbinding]
            && js::type_of(js::get(result, "then")) == "function"
        {
            // A promise/thenable was returned from the callback. Wait for it to
            // resolve before flushing the queue.
            //
            // If `act` were implemented as an async function, this whole block could
            // be a single `await` call. That's really the only difference between
            // this branch and the next one.
            let thenable: &'static Thenable = unsafe { js::cast(Some(result)) };

            // Warn if the an `act` call with an async scope is not awaited. In a
            // future release, consider making this an error.
            let did_await = Rc::clone(&did_await_act_call);
            queue_several_microtasks(Box::new(move || {
                if !did_await.get() && !DID_WARN_NO_AWAIT_ACT.get() {
                    DID_WARN_NO_AWAIT_ACT.set(true);
                    console::error((concat!(
                        "You called act(async () => ...) without await. ",
                        "This could lead to unexpected testing behaviour, ",
                        "interleaving multiple act calls and mixing their ",
                        "scopes. ",
                        "You should - await act(async () => ...);"
                    ),));
                }
            }));

            ActThenable {
                then: Box::new(move |resolve, reject| {
                    did_await_act_call.set(true);
                    let prev_act_queue = prev_act_queue.clone();
                    let queue = Rc::clone(&queue);
                    let rejected_prev_act_queue = prev_act_queue.clone();
                    thenable.then(
                        move |return_value| {
                            pop_act_scope(prev_act_queue.as_ref(), prev_act_scope_depth);
                            if prev_act_scope_depth == 0.0 {
                                // We're exiting the outermost `act` scope. Flush the queue.
                                if let Err(error) = panic::catch_unwind(AssertUnwindSafe(|| {
                                    flush_act_queue(&queue);
                                    queue_macrotask(Box::new(move || {
                                        // Recursively flush tasks scheduled by a microtask.
                                        recursively_flush_async_act_work(
                                            return_value,
                                            resolve,
                                            reject,
                                        )
                                    }));
                                })) {
                                    // `thenable` might not be a real promise, and `flushActQueue`
                                    // might throw, so we need to wrap `flushActQueue` in a
                                    // try/catch.
                                    REACT_SHARED_INTERNALS.with(|react_shared_internals| {
                                        react_shared_internals
                                            .thrown_errors
                                            .borrow_mut()
                                            .as_mut()
                                            .unwrap()
                                            .push(
                                                error
                                                    .downcast_ref::<&'static JsError>()
                                                    .and_then(|error| error.as_unknown()),
                                            );
                                    });
                                }
                                if REACT_SHARED_INTERNALS.with(|react_shared_internals| {
                                    react_shared_internals
                                        .thrown_errors
                                        .borrow()
                                        .as_ref()
                                        .unwrap()
                                        .len()
                                }) > 0
                                {
                                    let thrown_error =
                                        REACT_SHARED_INTERNALS.with(|react_shared_internals| {
                                            aggregate_errors(
                                                react_shared_internals
                                                    .thrown_errors
                                                    .borrow()
                                                    .as_ref()
                                                    .unwrap(),
                                            )
                                        });
                                    REACT_SHARED_INTERNALS.with(|react_shared_internals| {
                                        react_shared_internals
                                            .thrown_errors
                                            .borrow_mut()
                                            .as_mut()
                                            .unwrap()
                                            .clear();
                                    });
                                    reject(thrown_error);
                                }
                            } else {
                                resolve(return_value);
                            }
                        },
                        move |error| {
                            pop_act_scope(rejected_prev_act_queue.as_ref(), prev_act_scope_depth);
                            if REACT_SHARED_INTERNALS.with(|react_shared_internals| {
                                react_shared_internals
                                    .thrown_errors
                                    .borrow()
                                    .as_ref()
                                    .unwrap()
                                    .len()
                            }) > 0
                            {
                                let thrown_error =
                                    REACT_SHARED_INTERNALS.with(|react_shared_internals| {
                                        aggregate_errors(
                                            react_shared_internals
                                                .thrown_errors
                                                .borrow()
                                                .as_ref()
                                                .unwrap(),
                                        )
                                    });
                                REACT_SHARED_INTERNALS.with(|react_shared_internals| {
                                    react_shared_internals
                                        .thrown_errors
                                        .borrow_mut()
                                        .as_mut()
                                        .unwrap()
                                        .clear();
                                });
                                reject(thrown_error);
                            } else {
                                reject(error);
                            }
                        },
                    );
                }),
            }
        } else {
            let return_value = result;
            // The callback is not an async function. Exit the current
            // scope immediately.
            pop_act_scope(prev_act_queue.as_ref(), prev_act_scope_depth);
            if prev_act_scope_depth == 0.0 {
                // We're exiting the outermost `act` scope. Flush the queue.
                flush_act_queue(&queue);

                // If the queue is not empty, it implies that we intentionally yielded
                // to the main thread, because something suspended. We will continue
                // in an asynchronous task.
                //
                // Warn if something suspends but the `act` call is not awaited.
                // In a future release, consider making this an error.
                if !queue.borrow().is_empty() {
                    let did_await = Rc::clone(&did_await_act_call);
                    queue_several_microtasks(Box::new(move || {
                        if !did_await.get() && !DID_WARN_NO_AWAIT_ACT.get() {
                            DID_WARN_NO_AWAIT_ACT.set(true);
                            console::error((concat!(
                                "A component suspended inside an `act` scope, but the ",
                                "`act` call was not awaited. When testing React ",
                                "components that depend on asynchronous data, you must ",
                                "await the result:\n\n",
                                "await act(() => ...)"
                            ),));
                        }
                    }));
                }

                // Like many things in this module, this is next part is confusing.
                //
                // We do not currently require every `act` call that is passed a
                // callback to be awaited, through arguably we should. Since this
                // callback was synchronous, we need to exit the current scope before
                // returning.
                //
                // However, if thenable we're about to return *is* awaited, we'll
                // immediately restore the current scope. So it shouldn't observable.
                //
                // This doesn't affect the case where the scope callback is async,
                // because we always require those calls to be awaited.
                //
                // TODO: In a future version, consider always requiring all `act` calls
                // to be awaited, regardless of whether the callback is sync or async.
                REACT_SHARED_INTERNALS.with(|react_shared_internals| {
                    react_shared_internals.act_queue.replace(None);
                });
            }

            if REACT_SHARED_INTERNALS.with(|react_shared_internals| {
                react_shared_internals
                    .thrown_errors
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .len()
            }) > 0
            {
                let thrown_error = REACT_SHARED_INTERNALS.with(|react_shared_internals| {
                    aggregate_errors(
                        react_shared_internals
                            .thrown_errors
                            .borrow()
                            .as_ref()
                            .unwrap(),
                    )
                });
                REACT_SHARED_INTERNALS.with(|react_shared_internals| {
                    react_shared_internals
                        .thrown_errors
                        .borrow_mut()
                        .as_mut()
                        .unwrap()
                        .clear();
                });
                js::throw(thrown_error);
            }

            ActThenable {
                then: Box::new(move |resolve, reject| {
                    did_await_act_call.set(true);
                    if prev_act_scope_depth == 0.0 {
                        // If the `act` call is awaited, restore the queue we were
                        // using before (see long comment above) so we can flush it.
                        REACT_SHARED_INTERNALS.with(|react_shared_internals| {
                            react_shared_internals
                                .act_queue
                                .replace(Some(Rc::clone(&queue)));
                        });
                        queue_macrotask(Box::new(move || {
                            // Recursively flush tasks scheduled by a microtask.
                            recursively_flush_async_act_work(return_value, resolve, reject)
                        }));
                    } else {
                        resolve(return_value);
                    }
                }),
            }
        }
    } else {
        panic!("act(...) is not supported in production builds of React.");
    }
}

fn pop_act_scope(_prev_act_queue: Option<&ActQueue>, prev_act_scope_depth: f64) {
    if __DEV__ {
        if prev_act_scope_depth != ACT_SCOPE_DEPTH.get() - 1.0 {
            console::error((concat!(
                "You seem to have overlapping act() calls, this is not supported. ",
                "Be sure to await previous act() calls before making a new one. "
            ),));
        }
        ACT_SCOPE_DEPTH.set(prev_act_scope_depth);
    }
}

fn recursively_flush_async_act_work(
    return_value: Option<&'static Unknown>,
    resolve: &'static Settle,
    reject: &'static Settle,
) {
    if __DEV__ {
        // Check if any tasks were scheduled asynchronously.
        let queue = REACT_SHARED_INTERNALS
            .with(|react_shared_internals| react_shared_internals.act_queue.borrow().clone());
        if let Some(queue) = queue {
            if !queue.borrow().is_empty() {
                // Async tasks were scheduled, mostly likely in a microtask.
                // Keep flushing until there are no more.
                match panic::catch_unwind(AssertUnwindSafe(|| {
                    flush_act_queue(&queue);
                    // The work we just performed may have schedule additional async
                    // tasks. Wait a macrotask and check again.
                    queue_macrotask(Box::new(move || {
                        recursively_flush_async_act_work(return_value, resolve, reject)
                    }));
                })) {
                    Ok(()) => return,
                    Err(error) => {
                        // Leave remaining tasks on the queue if something throws.
                        REACT_SHARED_INTERNALS.with(|react_shared_internals| {
                            react_shared_internals
                                .thrown_errors
                                .borrow_mut()
                                .as_mut()
                                .unwrap()
                                .push(
                                    error
                                        .downcast_ref::<&'static JsError>()
                                        .and_then(|error| error.as_unknown()),
                                );
                        });
                    }
                }
            } else {
                // The queue is empty. We can finish.
                REACT_SHARED_INTERNALS.with(|react_shared_internals| {
                    react_shared_internals.act_queue.replace(None);
                });
            }
        }
        if REACT_SHARED_INTERNALS.with(|react_shared_internals| {
            react_shared_internals
                .thrown_errors
                .borrow()
                .as_ref()
                .unwrap()
                .len()
        }) > 0
        {
            let thrown_error = REACT_SHARED_INTERNALS.with(|react_shared_internals| {
                aggregate_errors(
                    react_shared_internals
                        .thrown_errors
                        .borrow()
                        .as_ref()
                        .unwrap(),
                )
            });
            REACT_SHARED_INTERNALS.with(|react_shared_internals| {
                react_shared_internals
                    .thrown_errors
                    .borrow_mut()
                    .as_mut()
                    .unwrap()
                    .clear();
            });
            reject(thrown_error);
        } else {
            resolve(return_value);
        }
    }
}

thread_local! {
    static IS_FLUSHING: Cell<bool> = const { Cell::new(false) };
}

/// What `flush_act_queue`'s `finally` does.
struct FinishFlushing;

impl Drop for FinishFlushing {
    fn drop(&mut self) {
        IS_FLUSHING.set(false);
    }
}

fn flush_act_queue(queue: &RefCell<Vec<RendererTask>>) {
    if __DEV__ && !IS_FLUSHING.get() {
        // Prevent re-entrance.
        IS_FLUSHING.set(true);
        let mut i = 0;
        let _finish = FinishFlushing;
        if let Err(error) = panic::catch_unwind(AssertUnwindSafe(|| {
            while i < queue.borrow().len() {
                let mut callback = queue.borrow()[i].clone();
                loop {
                    REACT_SHARED_INTERNALS.with(|react_shared_internals| {
                        react_shared_internals.did_use_promise.set(Some(false));
                    });
                    let continuation = (callback.0)(false);
                    if let Some(continuation) = continuation {
                        if REACT_SHARED_INTERNALS.with(|react_shared_internals| {
                            react_shared_internals.did_use_promise.get().unwrap()
                        }) {
                            // The component just suspended. Yield to the main thread in
                            // case the promise is already resolved. If so, it will ping in
                            // a microtask and we can resume without unwinding the stack.
                            queue.borrow_mut()[i] = callback;
                            queue.borrow_mut().drain(0..i);
                            return;
                        }
                        callback = continuation;
                    } else {
                        break;
                    }
                }
                i += 1;
            }
            // We flushed the entire queue.
            queue.borrow_mut().clear();
        })) {
            // If something throws, leave the remaining callbacks on the queue.
            queue.borrow_mut().drain(0..i + 1);
            REACT_SHARED_INTERNALS.with(|react_shared_internals| {
                react_shared_internals
                    .thrown_errors
                    .borrow_mut()
                    .as_mut()
                    .unwrap()
                    .push(
                        error
                            .downcast_ref::<&'static JsError>()
                            .and_then(|error| error.as_unknown()),
                    );
            });
        }
    }
}

// Some of our warnings attempt to detect if the `act` call is awaited by
// checking in an asynchronous task. Wait a few microtasks before checking. The
// only reason one isn't sufficient is we want to accommodate the case where an
// `act` call is returned from an async function without first being awaited,
// since that's a somewhat common pattern. If you do this too many times in a
// nested sequence, you might get a warning, but you can always fix by awaiting
// the call.
//
// A macrotask would also work (and is the fallback) but depending on the test
// environment it may cause the warning to fire too late.
thread_local! {
    static QUEUE_SEVERAL_MICROTASKS: fn(Box<dyn Fn()>) =
        if js::is_defined(webapi::global::queue_microtask) {
            |callback| {
                webapi::global::queue_microtask(Box::new(move || {
                    webapi::global::queue_microtask(callback)
                }));
            }
        } else {
            queue_macrotask
        };
}

fn queue_several_microtasks(callback: Box<dyn Fn()>) {
    QUEUE_SEVERAL_MICROTASKS.with(|&queue_several_microtasks| queue_several_microtasks)(callback);
}
