// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js::{JsError, RegExp, Unknown, WeakMap};
use node::CallSite;
use node::js_error::{PrepareStackTrace, prepare_stack_trace, set_prepare_stack_trace};
use shared::environment::__DEV__;
use shared::react_types::ReactStackTrace;

thread_local! {
    static FRAMES_TO_SKIP: Cell<usize> = const { Cell::new(0) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static COLLECTED_STACK_TRACE: RefCell<Option<Rc<ReactStackTrace>>> = const { RefCell::new(None) };

    static IDENTIFIER_REG_EXP: &'static RegExp = js::reg_exp::new("^[a-zA-Z_$][0-9a-zA-Z_$]*$", "");
}

fn get_method_call_name(call_site: &CallSite) -> String {
    let type_name = call_site.get_type_name();
    let method_name = call_site.get_method_name();
    let function_name = call_site.get_function_name();
    let mut result = String::new();
    if let Some(function_name) = function_name.filter(|name| !name.is_empty()) {
        if let Some(type_name) = type_name.as_deref().filter(|name| !name.is_empty())
            && IDENTIFIER_REG_EXP.with(|identifier_reg_exp| identifier_reg_exp.test(&function_name))
            && function_name != type_name
        {
            result += type_name;
            result += ".";
        }
        result += &function_name;
        if let Some(method_name) = method_name.filter(|name| !name.is_empty())
            && function_name != method_name
            && !function_name.ends_with(&(".".to_owned() + &method_name))
            && !function_name.ends_with(&(" ".to_owned() + &method_name))
        {
            result += " [as ";
            result += &method_name;
            result += "]";
        }
    } else {
        if let Some(type_name) = type_name.filter(|name| !name.is_empty()) {
            result += &type_name;
            result += ".";
        }
        if let Some(method_name) = method_name.filter(|name| !name.is_empty()) {
            result += &method_name;
        } else {
            result += "<anonymous>";
        }
    }
    result
}

fn collect_stack_trace_private(_error: &JsError, structured_stack_trace: &[&CallSite]) -> String {
    let mut result: ReactStackTrace = Vec::new();
    // Collect structured stack traces from the callsites.
    // We mirror how V8 serializes stack frames and how we later parse them.
    for call_site in structured_stack_trace.iter().skip(FRAMES_TO_SKIP.get()) {
        let mut name = call_site
            .get_function_name()
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "<anonymous>".to_owned());
        if name.contains("react_stack_bottom_frame") {
            // Skip everything after the bottom frame since it'll be internals.
            break;
        } else if call_site.is_native() {
            // $FlowFixMe[prop-missing]
            let is_async = call_site.is_async();
            result.push((name, String::new(), 0.0, 0.0, 0.0, 0.0, is_async));
        } else {
            // We encode complex function calls as if they're part of the function
            // name since we cannot simulate the complex ones and they look the same
            // as function names in UIs on the client as well as stacks.
            if call_site.is_constructor() {
                name = "new ".to_owned() + &name;
            } else if !call_site.is_toplevel() {
                name = get_method_call_name(call_site);
            }
            if name == "<anonymous>" {
                name = String::new();
            }
            let mut filename = call_site
                .get_script_name_or_source_url()
                .filter(|filename| !filename.is_empty())
                .unwrap_or_else(|| "<anonymous>".to_owned());
            if filename == "<anonymous>" {
                filename = String::new();
                if call_site.is_eval()
                    && let Some(origin) = call_site
                        .get_eval_origin()
                        .filter(|origin| !origin.is_empty())
                {
                    filename = origin + ", <anonymous>";
                }
            }
            let line = call_site.get_line_number().unwrap_or(0);
            let col = call_site.get_column_number().unwrap_or(0);
            let enclosing_line =
                if js::is_function((*call_site, CallSite::get_enclosing_line_number)) {
                    // $FlowFixMe[prop-missing]
                    call_site.get_enclosing_line_number().unwrap_or(0)
                } else {
                    0
                };
            let enclosing_col =
                if js::is_function((*call_site, CallSite::get_enclosing_column_number)) {
                    // $FlowFixMe[prop-missing]
                    call_site.get_enclosing_column_number().unwrap_or(0)
                } else {
                    0
                };
            // $FlowFixMe[prop-missing]
            let is_async = call_site.is_async();
            result.push((
                name,
                filename,
                f64::from(line),
                f64::from(col),
                f64::from(enclosing_line),
                f64::from(enclosing_col),
                is_async,
            ));
        }
    }
    COLLECTED_STACK_TRACE.set(Some(Rc::new(result)));
    String::new()
}

fn collect_stack_trace(error: &JsError, structured_stack_trace: &[&CallSite]) -> String {
    collect_stack_trace_private(error, structured_stack_trace);
    // At the same time we generate a string stack trace just in case someone
    // else reads it. Ideally, we'd call the previous prepareStackTrace to
    // ensure it's in the expected format but it's common for that to be
    // source mapped and since we do a lot of eager parsing of errors, it
    // would be slow in those environments. We could maybe just rely on those
    // environments having to disable source mapping globally to speed things up.
    // For now, we just generate a default V8 formatted stack trace without
    // source mapping as a fallback.
    let name = Some(error.name())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Error".to_owned());
    let message = error.message();
    let mut stack = name + ": " + &message;
    for call_site in structured_stack_trace {
        stack += "\n    at ";
        stack += &call_site.to_string();
    }
    stack
}

/// Puts back the `Error.prepareStackTrace` there was, as reading a stack
/// returns or throws.
struct RestorePrepare(Option<&'static PrepareStackTrace<Option<&'static Unknown>>>);

impl Drop for RestorePrepare {
    fn drop(&mut self) {
        set_prepare_stack_trace(self.0);
    }
}

thread_local! {
    // This matches either of these V8 formats.
    //     at name (filename:0:0)
    //     at filename:0:0
    //     at async filename:0:0
    static FRAME_REG_EXP: &'static RegExp = js::reg_exp::new(
        r"^ {3} at (?:(.+) \((?:(.+):(\d+):(\d+)|\<anonymous\>)\)|(?:async )?(.+):(\d+):(\d+)|\<anonymous\>)$",
        "",
    );

    // DEV-only cache of parsed and filtered stack frames.
    #[cfg_attr(rust_js, rust_js::nullable)]
    static STACK_TRACE_CACHE: Option<&'static WeakMap<JsError, Rc<ReactStackTrace>>> =
        if __DEV__ { Some(js::weak_map::new()) } else { None };
}

// This version is only used when React fully owns the Error object and there's no risk of it having
// been already initialized and no risky that anyone else will initialize it later.
#[cfg_attr(rust_js, rust_js::nullable)]
pub fn parse_stack_trace_private(
    error: &JsError,
    skip_frames: usize,
) -> Option<Rc<ReactStackTrace>> {
    COLLECTED_STACK_TRACE.set(None);
    FRAMES_TO_SKIP.set(skip_frames);
    {
        let previous_prepare = prepare_stack_trace();
        set_prepare_stack_trace(Some(&collect_stack_trace_private));
        let _restore = RestorePrepare(previous_prepare);
        if error.stack().as_deref() != Some("") {
            return None;
        }
    }
    COLLECTED_STACK_TRACE.with_borrow(Clone::clone)
}

pub fn parse_stack_trace(error: &'static JsError, skip_frames: usize) -> Rc<ReactStackTrace> {
    // We can only get structured data out of error objects once. So we cache the information
    // so we can get it again each time. It also helps performance when the same error is
    // referenced more than once.
    let existing = STACK_TRACE_CACHE
        .with(|stack_trace_cache| stack_trace_cache.and_then(|cache| cache.get(error)));
    if let Some(existing) = existing {
        return existing;
    }
    // We override Error.prepareStackTrace with our own version that collects
    // the structured data. We need more information than the raw stack gives us
    // and we need to ensure that we don't get the source mapped version.
    COLLECTED_STACK_TRACE.set(None);
    FRAMES_TO_SKIP.set(skip_frames);
    let mut stack = {
        let previous_prepare = prepare_stack_trace();
        set_prepare_stack_trace(Some(&collect_stack_trace));
        let _restore = RestorePrepare(previous_prepare);
        // eslint-disable-next-line react-internal/safe-string-coercion
        js::string(error.stack().as_deref().map(js::unknown))
    };

    if let Some(result) = COLLECTED_STACK_TRACE.with_borrow(Clone::clone) {
        STACK_TRACE_CACHE.with(|stack_trace_cache| {
            if let Some(stack_trace_cache) = stack_trace_cache {
                stack_trace_cache.set(error, Rc::clone(&result));
            }
        });
        return result;
    }

    // If the stack has already been read, or this is not actually a V8 compatible
    // engine then we might not get a normalized stack and it might still have been
    // source mapped. Regardless we try our best to parse it. This works best if the
    // environment just uses default V8 formatting and no source mapping.

    if stack.starts_with("Error: react-stack-top-frame\n") {
        // V8's default formatting prefixes with the error message which we
        // don't want/need.
        stack = js::string::slice(&stack, 29.0, None);
    }
    let mut idx = js::string::index_of(&stack, "react_stack_bottom_frame", None);
    if idx != -1.0 {
        idx = js::string::last_index_of(&stack, "\n", Some(idx));
    }
    if idx != -1.0 {
        // Cut off everything after the bottom frame since it'll be internals.
        stack = js::string::slice(&stack, 0.0, Some(idx));
    }
    let frames: Vec<&str> = stack.split('\n').collect();
    let mut parsed_frames: ReactStackTrace = Vec::new();
    // We skip top frames here since they may or may not be parseable but we
    // want to skip the same number of frames regardless. I.e. we can't do it
    // in the caller.
    for frame in frames.iter().skip(skip_frames) {
        let Some(parsed) = FRAME_REG_EXP.with(|frame_reg_exp| frame_reg_exp.exec(frame)) else {
            continue;
        };
        let mut name = parsed.get(1).unwrap_or_default();
        let mut is_async = parsed.get(8).as_deref() == Some("async ");
        if name == "<anonymous>" {
            name = String::new();
        } else if name.starts_with("async ") {
            name = js::string::slice(&name, 6.0, None);
            is_async = true;
        }
        let mut filename = parsed
            .get(2)
            .filter(|filename| !filename.is_empty())
            .or_else(|| parsed.get(5))
            .unwrap_or_default();
        if filename == "<anonymous>" {
            filename = String::new();
        }
        let line = js::to_number(
            &parsed
                .get(3)
                .filter(|line| !line.is_empty())
                .or_else(|| parsed.get(6)),
        );
        let col = js::to_number(
            &parsed
                .get(4)
                .filter(|col| !col.is_empty())
                .or_else(|| parsed.get(7)),
        );
        parsed_frames.push((name, filename, line, col, 0.0, 0.0, is_async));
    }
    let parsed_frames = Rc::new(parsed_frames);
    STACK_TRACE_CACHE.with(|stack_trace_cache| {
        if let Some(stack_trace_cache) = stack_trace_cache {
            stack_trace_cache.set(error, Rc::clone(&parsed_frames));
        }
    });
    parsed_frames
}
