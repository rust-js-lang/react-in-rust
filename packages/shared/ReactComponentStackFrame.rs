// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};

use js::{
    Class, JsError, JsObject, Kind, Promise, PropertyDescriptor, Unknown, WeakMap, object, reflect,
};
use node::js_error::{self, PrepareStackTrace};

use crate::console_patching_dev::{disable_logs, reenable_logs};
use crate::default_prepare_stack_trace::DEFAULT_PREPARE_STACK_TRACE;
use crate::environment::__DEV__;
use crate::react_owner_stack_frames::format_owner_stack;
use crate::react_shared_internals::{Dispatcher, REACT_SHARED_INTERNALS};

thread_local! {
    static PREFIX: RefCell<Option<String>> = const { RefCell::new(None) };
    static SUFFIX: RefCell<String> = const { RefCell::new(String::new()) };
}

pub fn describe_built_in_component_frame(name: &str) -> String {
    if PREFIX.with_borrow(Option::is_none) {
        // Extract the VM specific prefix used by each line.
        let Err(x) = catch_unwind(|| panic!(""));
        let stack = x
            .downcast_ref::<&'static JsError>()
            .and_then(|x| x.stack())
            .unwrap_or_default();
        let r#match = js::reg_exp::new(r"\n( *(at )?)", "").exec(&js::string::trim(&stack));
        PREFIX.set(Some(r#match.and_then(|m| m.get(1)).unwrap_or_default()));
        SUFFIX.set(
            if stack.contains("\n    at") {
                // V8
                " (<anonymous>)"
            } else if stack.contains('@') {
                // JSC/Spidermonkey
                "@unknown:0:0"
            } else {
                // Other
                ""
            }
            .to_string(),
        );
    }
    // We use the prefix to ensure our stacks line up with native stack frames.
    let prefix = PREFIX.with_borrow(|prefix| prefix.clone().unwrap_or_default());
    let suffix = SUFFIX.with_borrow(String::clone);
    format!("\n{prefix}{name}{suffix}")
}

pub fn describe_debug_info_frame(
    name: &str,
    env: Option<&str>,
    location: Option<&JsError>,
) -> String {
    if let Some(location) = location {
        // If we have a location, it's the child's owner stack. Treat the bottom most frame as
        // the location of this function.
        let child_stack = format_owner_stack(location);
        let last_line = match child_stack.rfind('\n') {
            None => &child_stack,
            Some(idx) => &child_stack[idx + 1..],
        };
        if last_line.contains(name) {
            // For async stacks it's possible we don't have the owner on it. As a precaution only
            // use this frame if it has the name of the function in it.
            return format!("\n{last_line}");
        }
    }

    describe_built_in_component_frame(&match env {
        Some(env) if !env.is_empty() => format!("{name} [{env}]"),
        _ => name.to_string(),
    })
}

thread_local! {
    static REENTRY: Cell<bool> = const { Cell::new(false) };
    static COMPONENT_FRAME_CACHE: Option<&'static WeakMap<dyn Fn() -> Option<&'static Unknown>, String>> =
        if __DEV__ {
            Some(if js::is_defined(js::weak_map::new::<JsObject, String>) {
                js::weak_map::new()
            } else {
                js::map::new(None)
            })
        } else {
            None
        };
}

/// What `describe_native_component_frame`'s `finally` puts back.
struct Restore {
    previous_prepare_stack_trace: Option<&'static PrepareStackTrace<Option<&'static Unknown>>>,
    previous_dispatcher: Option<&'static Dispatcher>,
}

impl Drop for Restore {
    fn drop(&mut self) {
        REENTRY.set(false);
        if __DEV__ {
            REACT_SHARED_INTERNALS.set_h(self.previous_dispatcher);
            reenable_logs();
        }
        js_error::set_prepare_stack_trace(self.previous_prepare_stack_trace);
    }
}

/// What the `finally` of the fallback without `Reflect.construct` puts back:
/// `fn.prototype.props` as it was.
struct RestoreProps {
    prototype: &'static Unknown,
    prev_props: Option<PropertyDescriptor<'static>>,
}

impl Drop for RestoreProps {
    fn drop(&mut self) {
        match &self.prev_props {
            Some(prev_props) => {
                object::define_property(self.prototype, "props", prev_props);
            }
            None => {
                js::delete(self.prototype, "props");
            }
        }
    }
}

/// Finding a common stack frame between sample and control errors can be
/// tricky given the different types and levels of stack trace truncation from
/// different JS VMs. So instead we'll attempt to control what that common
/// frame should be through this object method:
/// Having both the sample and control errors be in the function under the
/// `DescribeNativeComponentFrameRoot` property, + setting the `name` and
/// `displayName` properties of the function ensures that a stack
/// frame exists that has the method name `DescribeNativeComponentFrameRoot` in
/// it for both control and sample stacks.
#[cfg_attr(rust_js, rust_js::name = "DetermineComponentFrameRoot")]
fn determine_component_frame_root(
    r#fn: &dyn Fn() -> Option<&'static Unknown>,
    construct: bool,
) -> (Option<String>, Option<String>) {
    let mut control: Option<&'static JsError> = None;
    if let Err(sample) = catch_unwind(AssertUnwindSafe(|| {
        // This should throw.
        if construct {
            // Something should be setting the props in the constructor.
            #[cfg_attr(rust_js, rust_js::name = "Fake")]
            fn fake() -> Option<&'static Unknown> {
                panic!("");
            }
            // $FlowFixMe[prop-missing]
            object::define_property(
                Class::of(&fake).prototype(),
                "props",
                &PropertyDescriptor {
                    set: Some(&|_| {
                        // We use a throwing setter instead of frozen or non-writable props
                        // because that won't throw in a non-strict mode function.
                        panic!("");
                    }),
                    ..Default::default()
                },
            );
            if js::is_defined(reflect::construct) {
                // We construct a different control for this case to include any extra
                // frames added by the construct call.
                if let Err(x) = catch_unwind(|| reflect::construct(&fake, &[], None)) {
                    control = x.downcast_ref::<&'static JsError>().copied();
                }
                reflect::construct(r#fn, &[], Some(&fake));
            } else {
                if let Err(x) = catch_unwind(fake) {
                    control = x.downcast_ref::<&'static JsError>().copied();
                }

                let prototype = Class::of(r#fn).prototype();
                let prev_props = object::get_own_property_descriptor(Some(prototype), "props");
                object::define_property(
                    prototype,
                    "props",
                    &PropertyDescriptor {
                        configurable: Some(true),
                        set: Some(&|_| panic!("")),
                        ..Default::default()
                    },
                );
                let _restore_props = RestoreProps {
                    prototype,
                    prev_props,
                };

                js::new(Class::of(r#fn), ());
            }
        } else {
            let Err(x) = catch_unwind(|| panic!(""));
            control = x.downcast_ref::<&'static JsError>().copied();
            // TODO(luna): This will currently only throw if the function component
            // tries to access React/ReactDOM/props. We should probably make this throw
            // in simple components too
            let maybe_promise = r#fn();

            // If the function component returns a promise, it's likely an async
            // component, which we don't yet support. Attach a noop catch handler to
            // silence the error.
            // TODO: Implement component stacks for async client components?
            if let Some(maybe_promise) = maybe_promise
                && let Kind::Function(_) = js::classify(js::get(maybe_promise, "catch"))
            {
                unsafe { js::cast::<Promise<Option<&'static Unknown>>>(Some(maybe_promise)) }
                    .catch(|_| {});
            }
        }
    }))
    // This is inlined manually because closure doesn't do it for us.
        && let Some(sample) = sample.downcast_ref::<&'static JsError>()
        && let Some(control) = control
        && let Some(sample_stack) = sample.stack()
    {
        return (Some(sample_stack), control.stack());
    }
    (None, None)
}

/// Leverages native browser/VM stack frames to get proper details (e.g.
/// filename, line + col number) for a single component in a component stack. We
/// do this by:
///   (1) throwing and catching an error in the function - this will be our
///       control error.
///   (2) calling the component which will eventually throw an error that we'll
///       catch - this will be our sample error.
///   (3) diffing the control and sample error stacks to find the stack frame
///       which represents our component.
pub fn describe_native_component_frame(
    r#fn: &dyn Fn() -> Option<&'static Unknown>,
    construct: bool,
) -> String {
    // If something asked for a stack inside a fake render, it should get ignored.
    if REENTRY.get() {
        return String::new();
    }

    if __DEV__
        && let Some(frame) = COMPONENT_FRAME_CACHE
            .with(|cache| *cache)
            .and_then(|cache| cache.get(r#fn))
    {
        return frame;
    }

    {
        REENTRY.set(true);
        let previous_prepare_stack_trace = js_error::prepare_stack_trace();
        js_error::set_prepare_stack_trace(DEFAULT_PREPARE_STACK_TRACE);
        let mut previous_dispatcher = None;

        if __DEV__ {
            previous_dispatcher = REACT_SHARED_INTERNALS.h();
            // Set the dispatcher in DEV because this might be call in the render function
            // for warnings.
            REACT_SHARED_INTERNALS.set_h::<Dispatcher>(None);
            disable_logs();
        }
        let _restore = Restore {
            previous_prepare_stack_trace,
            previous_dispatcher,
        };

        // $FlowFixMe[prop-missing]
        js::set(
            js::unknown(&determine_component_frame_root),
            "displayName",
            "DetermineComponentFrameRoot",
        );
        let name_prop_descriptor = object::get_own_property_descriptor(
            Some(js::unknown(&determine_component_frame_root)),
            "name",
        );
        // Before ES6, the `name` property was not configurable.
        if let Some(name_prop_descriptor) = name_prop_descriptor
            && name_prop_descriptor.configurable == Some(true)
        {
            // V8 utilizes a function's `name` property when generating a stack trace.
            object::define_property(
                js::unknown(&determine_component_frame_root),
                // Configurable properties can be updated even if its writable descriptor
                // is set to `false`.
                // $FlowFixMe[cannot-write]
                "name",
                &PropertyDescriptor {
                    value: Some(&"DetermineComponentFrameRoot"),
                    ..Default::default()
                },
            );
        }

        let (sample_stack, control_stack) = determine_component_frame_root(r#fn, construct);
        if let Some(sample_stack) = sample_stack
            && let Some(control_stack) = control_stack
        {
            // This extracts the first frame from the sample that isn't also in the control.
            // Skipping one frame that we assume is the frame that calls the two.
            let sample_lines: Vec<&str> = sample_stack.split('\n').collect();
            let control_lines: Vec<&str> = control_stack.split('\n').collect();
            let mut s: isize = 0;
            let mut c: isize = 0;
            while s < sample_lines.len() as isize
                && !sample_lines[s as usize].contains("DetermineComponentFrameRoot")
            {
                s += 1;
            }
            while c < control_lines.len() as isize
                && !control_lines[c as usize].contains("DetermineComponentFrameRoot")
            {
                c += 1;
            }
            // We couldn't find our intentionally injected common root frame, attempt
            // to find another common root frame by search from the bottom of the
            // control stack...
            if s == sample_lines.len() as isize || c == control_lines.len() as isize {
                s = sample_lines.len() as isize - 1;
                c = control_lines.len() as isize - 1;
                while s >= 1 && c >= 0 && sample_lines[s as usize] != control_lines[c as usize] {
                    // We expect at least one stack frame to be shared.
                    // Typically this will be the root most one. However, stack frames may be
                    // cut off due to maximum stack limits. In this case, one maybe cut off
                    // earlier than the other. We assume that the sample is longer or the same
                    // and there for cut off earlier. So we should find the root most frame in
                    // the sample somewhere in the control.
                    c -= 1;
                }
            }
            while s >= 1 && c >= 0 {
                // Next we find the first one that isn't the same which should be the
                // frame that called our sample function and the control.
                if sample_lines[s as usize] != control_lines[c as usize] {
                    // In V8, the first line is describing the message but other VMs don't.
                    // If we're about to return the first line, and the control is also on the same
                    // line, that's a pretty good indicator that our sample threw at same line as
                    // the control. I.e. before we entered the sample frame. So we ignore this result.
                    // This can happen if you passed a class to function component, or non-function.
                    if s != 1 || c != 1 {
                        loop {
                            s -= 1;
                            c -= 1;
                            // We may still have similar intermediate frames from the construct call.
                            // The next one that isn't the same should be our match though.
                            if c < 0 || sample_lines[s as usize] != control_lines[c as usize] {
                                // V8 adds a "new" prefix for native classes. Let's remove it to make it prettier.
                                let mut frame = format!(
                                    "\n{}",
                                    sample_lines[s as usize].replacen(" at new ", " at ", 1)
                                );

                                // If our component frame is labeled "<anonymous>"
                                // but we have a user-provided "displayName"
                                // splice it in to make the stack more readable.
                                if js::truthy(js::get(js::unknown(r#fn), "displayName"))
                                    && frame.contains("<anonymous>")
                                {
                                    frame = js::string::replace(&frame, "<anonymous>", unsafe {
                                        js::cast(js::get(js::unknown(r#fn), "displayName"))
                                    });
                                }

                                if __DEV__
                                    && let Some(cache) = COMPONENT_FRAME_CACHE.with(|cache| *cache)
                                {
                                    cache.set(r#fn, frame.clone());
                                }
                                // Return the line we found.
                                return frame;
                            }
                            if s < 1 || c < 0 {
                                break;
                            }
                        }
                    }
                    break;
                }
                s -= 1;
                c -= 1;
            }
        }
    }
    // Fallback to just using the name if we couldn't make it throw.
    let name: Option<&str> = unsafe {
        js::cast(
            js::get(js::unknown(r#fn), "displayName")
                .filter(|name| js::truthy(Some(*name)))
                .or_else(|| js::get(js::unknown(r#fn), "name")),
        )
    };
    let synthetic_frame = match name {
        Some(name) if !name.is_empty() => describe_built_in_component_frame(name),
        _ => String::new(),
    };
    if __DEV__ && let Some(cache) = COMPONENT_FRAME_CACHE.with(|cache| *cache) {
        cache.set(r#fn, synthetic_frame.clone());
    }
    synthetic_frame
}

pub fn describe_class_component_frame(ctor: &dyn Fn() -> Option<&'static Unknown>) -> String {
    describe_native_component_frame(ctor, true)
}

pub fn describe_function_component_frame(r#fn: &dyn Fn() -> Option<&'static Unknown>) -> String {
    describe_native_component_frame(r#fn, false)
}
