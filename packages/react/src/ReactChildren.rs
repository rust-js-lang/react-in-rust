// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;

use js::{RegExp, SymbolFor, Unknown, function};
use shared::check_string_coercion::check_key_string_coercion;
use shared::environment::__DEV__;
use shared::is_array::is_array;
use shared::noop::noop;
use shared::react_feature_flags::ENABLE_OPTIMISTIC_KEY;
use shared::react_symbols::{
    REACT_ELEMENT_TYPE, REACT_LAZY_TYPE, REACT_OPTIMISTIC_KEY, REACT_PORTAL_TYPE, get_iterator_fn,
};
use webapi::console;

use crate::react_jsx_element::{ReactKey, clone_and_replace_key, is_valid_element};
use crate::react_lazy::{LazyComponent, Thenable};

const SEPARATOR: &str = ".";
const SUBSEPARATOR: &str = ":";

/// Escape and wrap key so it is safe to use as a reactid
///
/// @param {string} key to be escaped.
/// @return {string} the escaped key.
fn escape(key: &str) -> String {
    let escape_regex = js::reg_exp::new("[=:]", "g");
    let escaped_string = js::reg_exp::replace(key, escape_regex, |r#match: &str| {
        // $FlowFixMe[invalid-computed-prop]
        match r#match {
            "=" => "=0",
            ":" => "=2",
            _ => unreachable!(),
        }
    });

    String::from("$") + &escaped_string
}

/*
 * TODO: Test that a single child and an array with one item have the same key
 * pattern.
 */

thread_local! {
    static DID_WARN_ABOUT_MAPS: Cell<bool> = const { Cell::new(false) };
}

thread_local! {
    static USER_PROVIDED_KEY_ESCAPE_REGEX: &'static RegExp = js::reg_exp::new("/+", "g");
}
fn escape_user_provided_key(text: &str) -> String {
    USER_PROVIDED_KEY_ESCAPE_REGEX.with(|&user_provided_key_escape_regex| {
        js::reg_exp::replace(text, user_provided_key_escape_regex, "$&/")
    })
}

/// Generate a key string that identifies a element within a set.
///
/// @param {*} element A element that could contain a manual key.
/// @param {number} index Index that is used if a manual key is not provided.
/// @return {string}
fn get_element_key(element: Option<&'static Unknown>, index: u32) -> String {
    // Do some typechecking here since we call this blindly. We want to ensure
    // that we don't block potential future ES APIs.
    if js::type_of(element) == "object"
        && let Some(element) = element
        && js::get(element, "key").is_some()
    {
        if ENABLE_OPTIMISTIC_KEY.with(|&enabled| enabled)
            && SymbolFor::from_value(js::get(element, "key")) == Some(REACT_OPTIMISTIC_KEY)
        {
            // For React.Children purposes this is treated as just null.
            if __DEV__ {
                console::error(("React.Children helpers don't support optimisticKey.",));
            }
            return js::number::to_string(f64::from(index), Some(36));
        }
        // Explicit key
        if __DEV__ {
            check_key_string_coercion(js::get(element, "key"));
        }
        return escape(&js::concat_text(js::get(element, "key")).unwrap());
    }
    // Implicit key determined by the index in the set
    js::number::to_string(f64::from(index), Some(36))
}

fn resolve_thenable(thenable: &'static Thenable) -> Option<&'static Unknown> {
    match thenable.status().as_deref() {
        Some("fulfilled") => return thenable.value(),
        Some("rejected") => js::throw(thenable.reason()),
        _ => {
            if js::type_of(js::get(thenable, "status")) == "string" {
                // Only instrument the thenable if the status if not defined. If
                // it's defined, but an unknown value, assume it's been instrumented by
                // some custom userspace implementation. We treat it as "pending".
                // Attach a dummy listener, to ensure that any lazy initialization can
                // happen. Flight lazily parses JSON when the value is actually awaited.
                thenable.then(|_| noop(), |_| noop());
            } else {
                // This is an uncached thenable that we haven't seen before.

                // TODO: Detect infinite ping loops caused by uncached promises.

                thenable.set_status("pending");
                thenable.then(
                    move |fulfilled_value| {
                        if thenable.status().as_deref() == Some("pending") {
                            thenable.set_status("fulfilled");
                            thenable.set_value(fulfilled_value);
                        }
                    },
                    move |error| {
                        if thenable.status().as_deref() == Some("pending") {
                            thenable.set_status("rejected");
                            thenable.set_reason(error);
                        }
                    },
                );
            }

            // Check one more time in case the thenable resolved synchronously.
            match thenable.status().as_deref() {
                Some("fulfilled") => return thenable.value(),
                Some("rejected") => js::throw(thenable.reason()),
                _ => {}
            }
        }
    }
    js::throw(Some(js::unknown(thenable)))
}

#[cfg_attr(rust_js, rust_js::nullable(children))]
fn map_into_array(
    mut children: Option<&'static Unknown>,
    array: &mut Vec<Option<&'static Unknown>>,
    escaped_prefix: &str,
    name_so_far: &str,
    callback: &mut dyn FnMut(Option<&'static Unknown>) -> Option<&'static Unknown>,
) -> u32 {
    let r#type = js::type_of(children);

    if r#type == "undefined" || r#type == "boolean" {
        // All of the above are perceived as null.
        children = None;
    }

    let mut invoke_callback = false;

    if children.is_none() {
        invoke_callback = true;
    } else {
        match r#type.as_str() {
            "bigint" | "string" | "number" => invoke_callback = true,
            "object" => match SymbolFor::from_value(js::get(children, "$$typeof")) {
                Some(REACT_ELEMENT_TYPE | REACT_PORTAL_TYPE) => invoke_callback = true,
                Some(REACT_LAZY_TYPE) => {
                    let lazy: &LazyComponent = unsafe { js::cast(children) };
                    let payload = &lazy.payload;
                    let init = lazy.init;
                    return map_into_array(
                        init(payload),
                        array,
                        escaped_prefix,
                        name_so_far,
                        callback,
                    );
                }
                _ => {}
            },
            _ => {}
        }
    }

    if invoke_callback {
        let child = children;
        let mut mapped_child = callback(child);
        // If it's the only child, treat the name as if it was wrapped in an array
        // so that it's consistent if the number of children grows:
        let child_key = if name_so_far.is_empty() {
            SEPARATOR.to_string() + &get_element_key(child, 0)
        } else {
            name_so_far.to_string()
        };
        if is_array(mapped_child) {
            let escaped_child_key = escape_user_provided_key(&child_key) + "/";
            map_into_array(mapped_child, array, &escaped_child_key, "", &mut |c| c);
        } else if mapped_child.is_some() {
            if is_valid_element(mapped_child) {
                // The `if` statement here prevents auto-disabling of the safe
                // coercion ESLint rule, so we must manually disable it below.
                // $FlowFixMe[incompatible-type] Flow incorrectly thinks React.Portal doesn't have a key
                if __DEV__
                    && js::get(mapped_child, "key").is_some()
                    && (!js::truthy(child)
                        || !js::strict_equals(js::get(child, "key"), js::get(mapped_child, "key")))
                {
                    check_key_string_coercion(js::get(mapped_child, "key"));
                }
                let new_child = clone_and_replace_key(
                    unsafe { js::cast(mapped_child) },
                    // Keep both the (mapped) and old keys if they differ, just as
                    // traverseAllChildren used to do for objects as children
                    Some(ReactKey::Text(
                        escaped_prefix.to_string()
                        // $FlowFixMe[incompatible-type] Flow incorrectly thinks React.Portal doesn't have a key
                        + &(if js::get(mapped_child, "key").is_some()
                            && (!js::truthy(child)
                                || !js::strict_equals(
                                    js::get(child, "key"),
                                    js::get(mapped_child, "key"),
                                )) {
                            escape_user_provided_key(
                                // $FlowFixMe[unsafe-addition]
                                &js::concat_text(js::get(mapped_child, "key")).unwrap(), // eslint-disable-line react-internal/safe-string-coercion
                            ) + "/"
                        } else {
                            String::new()
                        })
                        + &child_key,
                    )),
                );
                // If `child` was an element without a `key`, we need to validate if
                // it should have had a `key`, before assigning one to `mappedChild`.
                // $FlowFixMe[incompatible-type] Flow incorrectly thinks React.Portal doesn't have a key
                if __DEV__
                    && !name_so_far.is_empty()
                    && child.is_some()
                    && is_valid_element(child)
                    && js::get(child, "key").is_none()
                {
                    // We check truthiness of `child._store.validated` instead of being
                    // inequal to `1` to provide a bit of backward compatibility for any
                    // libraries (like `fbt`) which may be hacking this property.
                    if js::truthy(js::get(child, "_store"))
                        && !js::truthy(js::get(js::get(child, "_store"), "validated"))
                    {
                        // Mark this child as having failed validation, but let the actual
                        // renderer print the warning later.
                        js::set(new_child.store.get(), "validated", 2.0);
                    }
                }
                mapped_child = Some(js::unknown_of(new_child));
            }
            array.push(mapped_child);
        }
        return 1;
    }

    let mut subtree_count = 0; // Count of children found in the current subtree.
    let next_name_prefix = if name_so_far.is_empty() {
        SEPARATOR.to_string()
    } else {
        name_so_far.to_string() + SUBSEPARATOR
    };

    if is_array(children) {
        let children: &[Option<&'static Unknown>] = unsafe { js::cast(children) };
        for (i, &child) in children.iter().enumerate() {
            let next_name = next_name_prefix.clone() + &get_element_key(child, i as u32);
            subtree_count += map_into_array(child, array, escaped_prefix, &next_name, callback);
        }
    } else {
        let iterator_fn = get_iterator_fn(children);
        if let Some(iterator_fn) = iterator_fn
            && js::type_of(Some(iterator_fn)) == "function"
        {
            let iterable_children = children;

            if __DEV__ {
                // Warn about using Maps as children
                if js::strict_equals(iterator_fn, js::get(iterable_children, "entries")) {
                    if !DID_WARN_ABOUT_MAPS.get() {
                        console::warn((concat!(
                            "Using Maps as children is not supported. ",
                            "Use an array of keyed ReactElements instead."
                        ),));
                    }
                    DID_WARN_ABOUT_MAPS.set(true);
                }
            }

            let iterator_fn: &dyn Fn() -> Option<&'static Unknown> =
                unsafe { js::cast(Some(iterator_fn)) };
            let iterator: &js::Iterator<Option<&'static Unknown>> =
                unsafe { js::cast(function::call(iterator_fn, Some(&iterable_children), ())) };
            let mut ii = 0;
            // $FlowFixMe[incompatible-use] `iteratorFn` might return null according to typing.
            loop {
                let step = iterator.next();
                if step.done == Some(true) {
                    break;
                }
                let child = step.value;
                let next_name = next_name_prefix.clone() + &get_element_key(child, ii);
                ii += 1;
                subtree_count += map_into_array(child, array, escaped_prefix, &next_name, callback);
            }
        } else if r#type == "object" {
            if js::type_of(js::get(children, "then")) == "function" {
                return map_into_array(
                    resolve_thenable(unsafe { js::cast(children) }),
                    array,
                    escaped_prefix,
                    name_so_far,
                    callback,
                );
            }

            // eslint-disable-next-line react-internal/safe-string-coercion
            let children_string = js::string(children);

            panic!(
                concat!(
                    "Objects are not valid as a React child (found: {}). ",
                    "If you meant to render a collection of children, use an array ",
                    "instead."
                ),
                if children_string == "[object Object]" {
                    "object with keys {".to_string()
                        + &js::object::keys(unsafe { js::cast(children) }).join(", ")
                        + "}"
                } else {
                    children_string
                },
            );
        }
    }

    subtree_count
}

type MapFunc<'a> = dyn Fn(Option<&'static Unknown>, u32) -> Option<&'static Unknown> + 'a;

/// Maps children that are typically specified as `props.children`.
///
/// See https://reactjs.org/docs/react-api.html#reactchildrenmap
///
/// The provided mapFunction(child, index) will be called for each
/// leaf child.
///
/// @param {?*} children Children tree container.
/// @param {function(*, int)} func The map function.
/// @param {*} context Context for mapFunction.
/// @return {object} Object containing the ordered map of results.
#[cfg_attr(rust_js, rust_js::nullable)]
pub fn map_children(
    children: Option<&'static Unknown>,
    func: &MapFunc<'_>,
    context: Option<&'static Unknown>,
) -> Option<Vec<Option<&'static Unknown>>> {
    // $FlowFixMe[incompatible-type] limitation refining abstract types in Flow
    children?;
    let mut result = Vec::new();
    let mut count: u32 = 0;
    map_into_array(children, &mut result, "", "", &mut |child| {
        let index = count;
        count += 1;
        function::call(func, Some(&context), (child, index))
    });
    Some(result)
}

/// Count the number of children that are typically specified as
/// `props.children`.
///
/// See https://reactjs.org/docs/react-api.html#reactchildrencount
///
/// @param {?*} children Children tree container.
/// @return {number} The number of children.
pub fn count_children(children: Option<&'static Unknown>) -> u32 {
    let n = Cell::new(0);
    map_children(
        children,
        &|_, _| {
            n.set(n.get() + 1);
            // Don't return anything
            None
        },
        None,
    );
    n.get()
}

type ForEachFunc = dyn Fn(Option<&'static Unknown>);

/// Iterates through children that are typically specified as `props.children`.
///
/// See https://reactjs.org/docs/react-api.html#reactchildrenforeach
///
/// The provided forEachFunc(child, index) will be called for each
/// leaf child.
///
/// @param {?*} children Children tree container.
/// @param {function(*, int)} forEachFunc
/// @param {*} forEachContext Context for forEachContext.
pub fn for_each_children(
    children: Option<&'static Unknown>,
    for_each_func: &ForEachFunc,
    for_each_context: Option<&'static Unknown>,
) {
    map_children(
        children,
        // $FlowFixMe[missing-this-annot]
        &|child, index| {
            function::call(for_each_func, Some(&for_each_context), (child, index));
            // Don't return anything.
            None
        },
        for_each_context,
    );
}

/// Flatten a children object (typically specified as `props.children`) and
/// return an array with appropriately re-keyed children.
///
/// See https://reactjs.org/docs/react-api.html#reactchildrentoarray
pub fn to_array(children: Option<&'static Unknown>) -> Vec<Option<&'static Unknown>> {
    map_children(children, &|child, _| child, None).unwrap_or_default()
}

/// Returns the first child in a collection of children and verifies that there
/// is only one child in the collection.
///
/// See https://reactjs.org/docs/react-api.html#reactchildrenonly
///
/// The current implementation of this function assumes that a single child gets
/// passed without a wrapper, but the purpose of this helper function is to
/// abstract away the particular structure of children.
///
/// @param {?object} children Child collection structure.
/// @return {ReactElement} The first and only `ReactElement` contained in the
/// structure.
pub fn only_child(children: Option<&'static Unknown>) -> Option<&'static Unknown> {
    if !is_valid_element(children) {
        panic!("React.Children.only expected to receive a single React element child.");
    }

    children
}

pub use count_children as count;
pub use for_each_children as for_each;
pub use map_children as map;
pub use only_child as only;
