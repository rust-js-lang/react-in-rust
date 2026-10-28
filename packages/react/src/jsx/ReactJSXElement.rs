// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;
use std::panic;

use js::{Dict, Error, Kind, PropertyDescriptor, SymbolFor, Unknown, object};
use node::js_error;
use shared::assign::assign;
use shared::check_string_coercion::check_key_string_coercion;
use shared::environment::__DEV__;
use shared::get_component_name_from_type::get_component_name_from_type;
use shared::has_own_property::HAS_OWN_PROPERTY;
use shared::is_array::is_array;
use shared::react_feature_flags::{ENABLE_OPTIMISTIC_KEY, OWNER_STACK_LIMIT};
use shared::react_shared_internals::REACT_SHARED_INTERNALS;
use shared::react_symbols::{
    REACT_ELEMENT_TYPE, REACT_FRAGMENT_TYPE, REACT_LAZY_TYPE, REACT_OPTIMISTIC_KEY,
};
use webapi::{ConsoleTask, console};

/// `shared/ReactTypes`' `ReactKey`: an element's key, text, or the
/// optimistic key a renderer gives it later.
#[cfg_attr(rust_js, rust_js::untagged)]
#[derive(Clone, PartialEq)]
pub enum ReactKey {
    Text(String),
    Optimistic(SymbolFor<'static>),
}

unsafe impl js::Defined for ReactKey {}

/// `shared/ReactElementType`'s `ReactElement`.
pub struct ReactElement {
    #[cfg_attr(rust_js, rust_js::name = "$$typeof")]
    pub r#typeof: SymbolFor<'static>,
    pub r#type: Option<&'static Unknown>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub key: Option<ReactKey>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub r#ref: Option<&'static Unknown>,
    pub props: &'static Unknown,
    // __DEV__ or for string refs
    #[cfg_attr(rust_js, rust_js::name = "_owner")]
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub owner: Option<&'static Unknown>,

    // __DEV__
    #[cfg_attr(rust_js, rust_js::name = "_store")]
    #[cfg_attr(rust_js, rust_js::nullable, rust_js::optional)]
    pub store: Cell<Option<&'static Dict<f64>>>, // 0: not validated, 1: validated, 2: force fail
}

unsafe impl js::Defined for ReactElement {}

/// What `console.createTask` is: given a name, the task DevTools shows the
/// stack of where it was made, or `None` where it makes none.
type CreateTask = dyn Fn(&str) -> Option<&'static ConsoleTask>;

#[cfg_attr(rust_js, rust_js::nullable)]
fn no_task(_name: &str) -> Option<&'static ConsoleTask> {
    None
}

thread_local! {
    static CREATE_TASK: Box<CreateTask> =
        // eslint-disable-next-line react-internal/no-production-logging
        if __DEV__ && let Some(create_task) = console::create_task() {
            // eslint-disable-next-line react-internal/no-production-logging
            Box::new(move |name| Some(create_task(name)))
        } else {
            Box::new(no_task)
        };
}

fn get_task_name(r#type: Option<&Unknown>) -> String {
    if SymbolFor::from_value(r#type) == Some(REACT_FRAGMENT_TYPE) {
        return "<>".to_string();
    }
    if js::type_of(r#type) == "object"
        && let Some(r#type) = r#type
        && SymbolFor::from_value(js::get(r#type, "$$typeof")) == Some(REACT_LAZY_TYPE)
    {
        // We don't want to eagerly initialize the initializer in DEV mode so we can't
        // call it to extract the type so we don't know the type of this component.
        return "<...>".to_string();
    }
    panic::catch_unwind(|| {
        let name = get_component_name_from_type(r#type);
        if let Some(name) = name
            && !name.is_empty()
        {
            "<".to_string() + &name + ">"
        } else {
            "<...>".to_string()
        }
    })
    .unwrap_or_else(|_| "<...>".to_string())
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn get_owner() -> Option<&'static Unknown> {
    if __DEV__ {
        let dispatcher = REACT_SHARED_INTERNALS.a()?;
        return dispatcher.get_owner();
    }
    None
}

// v8 (Chromium, Node.js) defaults to 10
// SpiderMonkey (Firefox) does not support Error.stackTraceLimit
// JSC (Safari) defaults to 100
// The lower the limit, the more likely we'll not reach react_stack_bottom_frame
// The higher the limit, the slower Error() is when not inspecting with a debugger.
// When inspecting with a debugger, Error.stackTraceLimit has no impact on Error() performance (in v8).
#[cfg_attr(rust_js, rust_js::name = "ownerStackTraceLimit")]
const OWNER_STACK_TRACE_LIMIT: f64 = 10.0;

struct FakeCallStack {
    #[cfg_attr(rust_js, rust_js::name = "react_stack_bottom_frame")]
    react_stack_bottom_frame: fn(fn() -> &'static Error) -> &'static Error,
}

#[inline(never)]
#[cfg_attr(rust_js, rust_js::name = "UnknownOwner")]
fn unknown_owner() -> &'static Error {
    #[inline(never)]
    fn react_stack_top_frame() -> &'static Error {
        js::error::new("react-stack-top-frame")
    }
    react_stack_top_frame()
}
#[cfg_attr(rust_js, rust_js::name = "createFakeCallStack")]
const CREATE_FAKE_CALL_STACK: FakeCallStack = FakeCallStack {
    react_stack_bottom_frame: |call_stack_for_error| call_stack_for_error(),
};

thread_local! {
    static SPECIAL_PROP_KEY_WARNING_SHOWN: Cell<bool> = const { Cell::new(false) };
    static DID_WARN_ABOUT_ELEMENT_REF: &'static Dict<bool> = js::dict::new();
    static DID_WARN_ABOUT_OLD_JSX_RUNTIME: Cell<bool> = const { Cell::new(false) };
    static UNKNOWN_OWNER_DEBUG_STACK: Cell<Option<&'static Error>> = const { Cell::new(None) };
    static UNKNOWN_OWNER_DEBUG_TASK: Cell<Option<&'static ConsoleTask>> = const { Cell::new(None) };
}

js::on_load! {
    if __DEV__ {
        // We use this technique to trick minifiers to preserve the function name.
        UNKNOWN_OWNER_DEBUG_STACK.set(Some((CREATE_FAKE_CALL_STACK.react_stack_bottom_frame)(
            unknown_owner,
        )));
        UNKNOWN_OWNER_DEBUG_TASK.set(CREATE_TASK.with(|create_task| {
            create_task(&get_task_name(Some(js::unknown(&unknown_owner))))
        }));
    }
}

fn has_valid_ref(config: &Unknown) -> bool {
    if __DEV__ && HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(config, "ref")) {
        let getter = object::get_own_property_descriptor(Some(config), "ref")
            .unwrap()
            .get;
        if let Some(getter) = getter
            && js::truthy(js::get(js::unknown(getter), "isReactWarning"))
        {
            return false;
        }
    }
    !js::strict_equals(js::get(config, "ref"), None::<&Unknown>)
}

fn has_valid_key(config: &Unknown) -> bool {
    if __DEV__ && HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(config, "key")) {
        let getter = object::get_own_property_descriptor(Some(config), "key")
            .unwrap()
            .get;
        if let Some(getter) = getter
            && js::truthy(js::get(js::unknown(getter), "isReactWarning"))
        {
            return false;
        }
    }
    !js::strict_equals(js::get(config, "key"), None::<&Unknown>)
}

fn define_key_prop_warning_getter(props: &Unknown, display_name: Option<&'static Unknown>) {
    if __DEV__ {
        let warn_about_accessing_key = move || {
            if !SPECIAL_PROP_KEY_WARNING_SHOWN.get() {
                SPECIAL_PROP_KEY_WARNING_SHOWN.set(true);
                console::error((
                    concat!(
                        "%s: `key` is not a prop. Trying to access it will result ",
                        "in `undefined` being returned. If you need to access the same ",
                        "value within the child component, you should pass it as a different ",
                        "prop. (https://react.dev/link/special-props)"
                    ),
                    display_name,
                ));
            }
            None
        };
        js::set(
            js::unknown(&warn_about_accessing_key),
            "isReactWarning",
            true,
        );
        object::define_property(
            props,
            "key",
            &PropertyDescriptor {
                get: Some(&warn_about_accessing_key),
                configurable: Some(true),
                ..Default::default()
            },
        );
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn element_ref_getter_with_deprecation_warning(this: &ReactElement) -> Option<&'static Unknown> {
    if __DEV__ {
        let component_name = get_component_name_from_type(this.r#type);
        let key = component_name.as_deref().unwrap_or("null");
        if !DID_WARN_ABOUT_ELEMENT_REF.with(|did_warn_about_element_ref| {
            js::dict::get(did_warn_about_element_ref, key).is_some_and(|&warned| warned)
        }) {
            DID_WARN_ABOUT_ELEMENT_REF.with(|did_warn_about_element_ref| {
                js::dict::set(did_warn_about_element_ref, key, true);
            });
            console::error((concat!(
                "Accessing element.ref was removed in React 19. ref is now a ",
                "regular prop. It will be removed from the JSX Element ",
                "type in a future release."
            ),));
        }

        // An undefined `element.ref` is coerced to `null` for
        // backwards compatibility.
        let ref_prop = js::get(this.props, "ref");
        return if !js::strict_equals(ref_prop, None::<&Unknown>) {
            ref_prop
        } else {
            None
        };
    }
    None
}

/// Factory method to create a new React element. This no longer adheres to
/// the class pattern, so do not use new to call it. Also, instanceof check
/// will not work. Instead test $$typeof field against Symbol.for('react.transitional.element') to check
/// if something is a React Element.
///
/// @internal
#[cfg_attr(rust_js, rust_js::name = "ReactElement")]
fn react_element(
    r#type: Option<&'static Unknown>,
    key: Option<ReactKey>,
    props: &'static Unknown,
    owner: Option<&'static Unknown>,
    debug_stack: Option<&'static Error>,
    debug_task: Option<&'static ConsoleTask>,
) -> ReactElement {
    // Ignore whatever was passed as the ref argument and treat `props.ref` as
    // the source of truth. The only thing we use this for is `element.ref`,
    // which will log a deprecation warning on access. In the next release, we
    // can remove `element.ref` as well as the `ref` argument.
    let ref_prop = js::get(props, "ref");

    // An undefined `element.ref` is coerced to `null` for
    // backwards compatibility.
    let r#ref = if !js::strict_equals(ref_prop, None::<&Unknown>) {
        ref_prop
    } else {
        None
    };

    let element;
    if __DEV__ {
        // In dev, make `ref` a non-enumerable property with a warning. It's non-
        // enumerable so that test matchers and serializers don't access it and
        // trigger the warning.
        //
        // `ref` will be removed from the element completely in a future release.
        element = ReactElement {
            // This tag allows us to uniquely identify this as a React Element
            r#typeof: REACT_ELEMENT_TYPE,

            // Built-in properties that belong on the element
            r#type,
            key,

            props,

            // Record the component responsible for creating this element.
            owner,
            r#ref: None,
            store: Cell::new(None),
        };
        if r#ref.is_some() {
            object::define_property(
                &element,
                "ref",
                &PropertyDescriptor {
                    enumerable: Some(false),
                    get: Some(&|| element_ref_getter_with_deprecation_warning(&element)),
                    ..Default::default()
                },
            );
        } else {
            // Don't warn on access if a ref is not given. This reduces false
            // positives in cases where a test serializer uses
            // getOwnPropertyDescriptors to compare objects, like Jest does, which is
            // a problem because it bypasses non-enumerability.
            //
            // So unfortunately this will trigger a false positive warning in Jest
            // when the diff is printed:
            //
            //   expect(<div ref={ref} />).toEqual(<span ref={ref} />);
            //
            // A bit sketchy, but this is what we've done for the `props.key` and
            // `props.ref` accessors for years, which implies it will be good enough
            // for `element.ref`, too. Let's see if anyone complains.
            object::define_property(
                &element,
                "ref",
                &PropertyDescriptor {
                    enumerable: Some(false),
                    value: Some(&Kind::Null),
                    ..Default::default()
                },
            );
        }
    } else {
        // In prod, `ref` is a regular property and _owner doesn't exist.
        element = ReactElement {
            // This tag allows us to uniquely identify this as a React Element
            r#typeof: REACT_ELEMENT_TYPE,

            // Built-in properties that belong on the element
            r#type,
            key,
            r#ref,

            props,
            owner: None,
            store: Cell::new(None),
        };
    }

    if __DEV__ {
        // The validation flag is currently mutative. We put it on
        // an external backing store so that we can freeze the whole object.
        // This can be replaced with a WeakMap once they are implemented in
        // commonly used development environments.
        element.store.set(Some(js::dict::new()));

        // To make comparing ReactElements easier for testing purposes, we make
        // the validation flag non-enumerable (where possible, which should
        // include every environment we run tests in), so the test framework
        // ignores it.
        object::define_property(
            &element.store.get(),
            "validated",
            &PropertyDescriptor {
                configurable: Some(false),
                enumerable: Some(false),
                writable: Some(true),
                value: Some(&0.0),
                ..Default::default()
            },
        );
        // debugInfo contains Server Component debug information.
        object::define_property(
            &element,
            "_debugInfo",
            &PropertyDescriptor {
                configurable: Some(false),
                enumerable: Some(false),
                writable: Some(true),
                value: Some(&Kind::Null),
                ..Default::default()
            },
        );
        object::define_property(
            &element,
            "_debugStack",
            &PropertyDescriptor {
                configurable: Some(false),
                enumerable: Some(false),
                writable: Some(true),
                value: Some(&debug_stack),
                ..Default::default()
            },
        );
        object::define_property(
            &element,
            "_debugTask",
            &PropertyDescriptor {
                configurable: Some(false),
                enumerable: Some(false),
                writable: Some(true),
                value: Some(&debug_task),
                ..Default::default()
            },
        );
        if js::is_defined(object::freeze::<Unknown>) {
            object::freeze(element.props);
            object::freeze(&element);
        }
    }

    element
}

/// https://github.com/reactjs/rfcs/pull/107
/// @param {*} type
/// @param {object} props
/// @param {string} key
pub fn jsx_prod(
    r#type: Option<&'static Unknown>,
    config: &'static Unknown,
    maybe_key: Option<&'static Unknown>,
) -> ReactElement {
    let mut key = None;

    // Currently, key can be spread in as a prop. This causes a potential
    // issue if key is also explicitly declared (ie. <div {...props} key="Hi" />
    // or <div key="Hi" {...props} /> ). We want to deprecate key spread,
    // but as an intermediary step, we will use jsxDEV for everything except
    // <div {...props} key="Hi" />, because we aren't currently able to tell if
    // key is explicitly declared to be undefined or not.
    if !js::strict_equals(maybe_key, None::<&Unknown>) {
        if ENABLE_OPTIMISTIC_KEY.with(|&enabled| enabled)
            && SymbolFor::from_value(maybe_key) == Some(REACT_OPTIMISTIC_KEY)
        {
            key = Some(ReactKey::Optimistic(REACT_OPTIMISTIC_KEY));
        } else {
            if __DEV__ {
                check_key_string_coercion(maybe_key);
            }
            key = Some(ReactKey::Text(js::concat_text(maybe_key).unwrap()));
        }
    }

    if has_valid_key(config) {
        if ENABLE_OPTIMISTIC_KEY.with(|&enabled| enabled)
            && SymbolFor::from_value(maybe_key) == Some(REACT_OPTIMISTIC_KEY)
        {
            key = Some(ReactKey::Optimistic(REACT_OPTIMISTIC_KEY));
        } else {
            if __DEV__ {
                check_key_string_coercion(js::get(config, "key"));
            }
            key = Some(ReactKey::Text(
                js::concat_text(js::get(config, "key")).unwrap(),
            ));
        }
    }

    let props;
    if !js::has(config, "key") {
        // If key was not spread in, we can reuse the original props object. This
        // only works for `jsx`, not `createElement`, because `jsx` is a compiler
        // target and the compiler always passes a new object. For `createElement`,
        // we can't assume a new object is passed every time because it can be
        // called manually.
        //
        // Spreading key is a warning in dev. In a future release, we will not
        // remove a spread key from the props object. (But we'll still warn.) We'll
        // always pass the object straight through.
        props = config;
    } else {
        // We need to remove reserved props (key, prop, ref). Create a fresh props
        // object and copy over all the non-reserved props. We don't use `delete`
        // because in V8 it will deopt the object to dictionary mode.
        props = js::unknown(js::dict::new::<Option<&Unknown>>());
        for prop_name in js::keys_in(config) {
            // Skip over reserved prop names
            if prop_name != "key" {
                js::set(props, &prop_name, js::get(config, &prop_name));
            }
        }
    }

    react_element(r#type, key, props, get_owner(), None, None)
}

// While `jsxDEV` should never be called when running in production, we do
// support `jsx` and `jsxs` when running in development. This supports the case
// where a third-party dependency ships code that was compiled for production;
// we want to still provide warnings in development.
//
// So these functions are the _dev_ implementations of the _production_
// API signatures.
//
// Since these functions are dev-only, it's ok to add an indirection here. They
// only exist to provide different versions of `isStaticChildren`. (We shouldn't
// use this pattern for the prod versions, though, because it will add an call
// frame.)
pub fn jsx_prod_signature_running_in_dev_with_dynamic_children(
    r#type: Option<&'static Unknown>,
    config: &'static Unknown,
    maybe_key: Option<&'static Unknown>,
) -> Option<ReactElement> {
    if __DEV__ {
        let is_static_children = false;
        let track_actual_owner = __DEV__ && {
            let recently_created_owner_stacks =
                REACT_SHARED_INTERNALS.recently_created_owner_stacks();
            REACT_SHARED_INTERNALS
                .set_recently_created_owner_stacks(recently_created_owner_stacks + 1.0);
            recently_created_owner_stacks < OWNER_STACK_LIMIT
        };
        let mut debug_stack_dev = None;
        if __DEV__ {
            if track_actual_owner {
                let previous_stack_trace_limit = js_error::stack_trace_limit();
                js_error::set_stack_trace_limit(OWNER_STACK_TRACE_LIMIT);
                debug_stack_dev = Some(js::error::new("react-stack-top-frame"));
                js_error::set_stack_trace_limit(previous_stack_trace_limit);
            } else {
                debug_stack_dev = UNKNOWN_OWNER_DEBUG_STACK.get();
            }
        }

        return jsx_dev_impl(
            r#type,
            config,
            maybe_key,
            is_static_children,
            debug_stack_dev,
            if __DEV__ {
                if track_actual_owner {
                    CREATE_TASK.with(|create_task| create_task(&get_task_name(r#type)))
                } else {
                    UNKNOWN_OWNER_DEBUG_TASK.get()
                }
            } else {
                None
            },
        );
    }
    None
}

pub fn jsx_prod_signature_running_in_dev_with_static_children(
    r#type: Option<&'static Unknown>,
    config: &'static Unknown,
    maybe_key: Option<&'static Unknown>,
) -> Option<ReactElement> {
    if __DEV__ {
        let is_static_children = true;
        let track_actual_owner = __DEV__ && {
            let recently_created_owner_stacks =
                REACT_SHARED_INTERNALS.recently_created_owner_stacks();
            REACT_SHARED_INTERNALS
                .set_recently_created_owner_stacks(recently_created_owner_stacks + 1.0);
            recently_created_owner_stacks < OWNER_STACK_LIMIT
        };
        let mut debug_stack_dev = None;
        if __DEV__ {
            if track_actual_owner {
                let previous_stack_trace_limit = js_error::stack_trace_limit();
                js_error::set_stack_trace_limit(OWNER_STACK_TRACE_LIMIT);
                debug_stack_dev = Some(js::error::new("react-stack-top-frame"));
                js_error::set_stack_trace_limit(previous_stack_trace_limit);
            } else {
                debug_stack_dev = UNKNOWN_OWNER_DEBUG_STACK.get();
            }
        }
        return jsx_dev_impl(
            r#type,
            config,
            maybe_key,
            is_static_children,
            debug_stack_dev,
            if __DEV__ {
                if track_actual_owner {
                    CREATE_TASK.with(|create_task| create_task(&get_task_name(r#type)))
                } else {
                    UNKNOWN_OWNER_DEBUG_TASK.get()
                }
            } else {
                None
            },
        );
    }
    None
}

thread_local! {
    static DID_WARN_ABOUT_KEY_SPREAD: &'static Dict<bool> = js::dict::new();
}

/// https://github.com/reactjs/rfcs/pull/107
/// @param {*} type
/// @param {object} props
/// @param {string} key
#[cfg_attr(rust_js, rust_js::name = "jsxDEV")]
pub fn jsx_dev(
    r#type: Option<&'static Unknown>,
    config: &'static Unknown,
    maybe_key: Option<&'static Unknown>,
    is_static_children: bool,
) -> Option<ReactElement> {
    let track_actual_owner = __DEV__ && {
        let recently_created_owner_stacks = REACT_SHARED_INTERNALS.recently_created_owner_stacks();
        REACT_SHARED_INTERNALS
            .set_recently_created_owner_stacks(recently_created_owner_stacks + 1.0);
        recently_created_owner_stacks < OWNER_STACK_LIMIT
    };
    let mut debug_stack_dev = None;
    if __DEV__ {
        if track_actual_owner {
            let previous_stack_trace_limit = js_error::stack_trace_limit();
            js_error::set_stack_trace_limit(OWNER_STACK_TRACE_LIMIT);
            debug_stack_dev = Some(js::error::new("react-stack-top-frame"));
            js_error::set_stack_trace_limit(previous_stack_trace_limit);
        } else {
            debug_stack_dev = UNKNOWN_OWNER_DEBUG_STACK.get();
        }
    }
    jsx_dev_impl(
        r#type,
        config,
        maybe_key,
        is_static_children,
        debug_stack_dev,
        if __DEV__ {
            if track_actual_owner {
                CREATE_TASK.with(|create_task| create_task(&get_task_name(r#type)))
            } else {
                UNKNOWN_OWNER_DEBUG_TASK.get()
            }
        } else {
            None
        },
    )
}

#[cfg_attr(rust_js, rust_js::name = "jsxDEVImpl")]
fn jsx_dev_impl(
    r#type: Option<&'static Unknown>,
    config: &'static Unknown,
    maybe_key: Option<&'static Unknown>,
    is_static_children: bool,
    debug_stack: Option<&'static Error>,
    debug_task: Option<&'static ConsoleTask>,
) -> Option<ReactElement> {
    if __DEV__ {
        // We don't warn for invalid element type here because with owner stacks,
        // we error in the renderer. The renderer is the only one that knows what
        // types are valid for this particular renderer so we let it error there.

        // Skip key warning if the type isn't valid since our key validation logic
        // doesn't expect a non-string/function type and can throw confusing
        // errors. We don't want exception behavior to differ between dev and
        // prod. (Rendering will throw with a helpful message and as soon as the
        // type is fixed, the key warnings will appear.)
        // With owner stacks, we no longer need the type here so this comment is
        // no longer true. Which is why we can run this even for invalid types.
        let children = js::get(config, "children");
        if !js::strict_equals(children, None::<&Unknown>) {
            if is_static_children {
                if is_array(children) {
                    let children: &[Option<&'static Unknown>] = unsafe { js::cast(children) };
                    for &child in children {
                        validate_child_keys(child);
                    }

                    if js::is_defined(object::freeze::<Unknown>) {
                        object::freeze(children);
                    }
                } else {
                    console::error((concat!(
                        "React.jsx: Static children should always be an array. ",
                        "You are likely explicitly calling React.jsxs or React.jsxDEV. ",
                        "Use the Babel transform instead."
                    ),));
                }
            } else {
                validate_child_keys(children);
            }
        }

        // Warn about key spread regardless of whether the type is valid.
        if HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(config, "key")) {
            let component_name = get_component_name_from_type(r#type);
            let keys: Vec<String> = object::keys(config)
                .into_iter()
                .filter(|k| k != "key")
                .collect();
            let before_example = if !keys.is_empty() {
                "{key: someKey, ".to_string() + &keys.join(": ..., ") + ": ...}"
            } else {
                "{key: someKey}".to_string()
            };
            let warning_key =
                component_name.as_deref().unwrap_or("null").to_string() + &before_example;
            if !DID_WARN_ABOUT_KEY_SPREAD.with(|did_warn_about_key_spread| {
                js::dict::get(did_warn_about_key_spread, &warning_key).is_some_and(|&warned| warned)
            }) {
                let after_example = if !keys.is_empty() {
                    "{".to_string() + &keys.join(": ..., ") + ": ...}"
                } else {
                    "{}".to_string()
                };
                console::error((
                    concat!(
                        "A props object containing a \"key\" prop is being spread into JSX:\n",
                        "  let props = %s;\n",
                        "  <%s {...props} />\n",
                        "React keys must be passed directly to JSX without using spread:\n",
                        "  let props = %s;\n",
                        "  <%s key={someKey} {...props} />"
                    ),
                    before_example,
                    component_name.clone(),
                    after_example,
                    component_name,
                ));
                DID_WARN_ABOUT_KEY_SPREAD.with(|did_warn_about_key_spread| {
                    js::dict::set(did_warn_about_key_spread, &warning_key, true);
                });
            }
        }

        let mut key = None;

        // Currently, key can be spread in as a prop. This causes a potential
        // issue if key is also explicitly declared (ie. <div {...props} key="Hi" />
        // or <div key="Hi" {...props} /> ). We want to deprecate key spread,
        // but as an intermediary step, we will use jsxDEV for everything except
        // <div {...props} key="Hi" />, because we aren't currently able to tell if
        // key is explicitly declared to be undefined or not.
        if !js::strict_equals(maybe_key, None::<&Unknown>) {
            if ENABLE_OPTIMISTIC_KEY.with(|&enabled| enabled)
                && SymbolFor::from_value(maybe_key) == Some(REACT_OPTIMISTIC_KEY)
            {
                key = Some(ReactKey::Optimistic(REACT_OPTIMISTIC_KEY));
            } else {
                if __DEV__ {
                    check_key_string_coercion(maybe_key);
                }
                key = Some(ReactKey::Text(js::concat_text(maybe_key).unwrap()));
            }
        }

        if has_valid_key(config) {
            if ENABLE_OPTIMISTIC_KEY.with(|&enabled| enabled)
                && SymbolFor::from_value(js::get(config, "key")) == Some(REACT_OPTIMISTIC_KEY)
            {
                key = Some(ReactKey::Optimistic(REACT_OPTIMISTIC_KEY));
            } else {
                if __DEV__ {
                    check_key_string_coercion(js::get(config, "key"));
                }
                key = Some(ReactKey::Text(
                    js::concat_text(js::get(config, "key")).unwrap(),
                ));
            }
        }

        let props;
        if !js::has(config, "key") {
            // If key was not spread in, we can reuse the original props object. This
            // only works for `jsx`, not `createElement`, because `jsx` is a compiler
            // target and the compiler always passes a new object. For `createElement`,
            // we can't assume a new object is passed every time because it can be
            // called manually.
            //
            // Spreading key is a warning in dev. In a future release, we will not
            // remove a spread key from the props object. (But we'll still warn.) We'll
            // always pass the object straight through.
            props = config;
        } else {
            // We need to remove reserved props (key, prop, ref). Create a fresh props
            // object and copy over all the non-reserved props. We don't use `delete`
            // because in V8 it will deopt the object to dictionary mode.
            props = js::unknown(js::dict::new::<Option<&Unknown>>());
            for prop_name in js::keys_in(config) {
                // Skip over reserved prop names
                if prop_name != "key" {
                    js::set(props, &prop_name, js::get(config, &prop_name));
                }
            }
        }

        if js::truthy(key.as_ref()) {
            let display_name = if js::type_of(r#type) == "function" {
                js::get(r#type, "displayName")
                    .filter(|name| js::truthy(Some(*name)))
                    .or_else(|| js::get(r#type, "name"))
                    .filter(|name| js::truthy(Some(*name)))
                    .or_else(|| Some(js::unknown("Unknown")))
            } else {
                r#type
            };
            define_key_prop_warning_getter(props, display_name);
        }

        return Some(react_element(
            r#type,
            key,
            props,
            get_owner(),
            debug_stack,
            debug_task,
        ));
    }
    None
}

/// Create and return a new ReactElement of the given type.
/// See https://reactjs.org/docs/react-api.html#createelement
pub fn create_element(
    r#type: Option<&'static Unknown>,
    config: Option<&'static Unknown>,
    #[cfg_attr(rust_js, rust_js::rest)] children: &[Option<&'static Unknown>],
) -> ReactElement {
    if __DEV__ {
        // We don't warn for invalid element type here because with owner stacks,
        // we error in the renderer. The renderer is the only one that knows what
        // types are valid for this particular renderer so we let it error there.

        // Skip key warning if the type isn't valid since our key validation logic
        // doesn't expect a non-string/function type and can throw confusing
        // errors. We don't want exception behavior to differ between dev and
        // prod. (Rendering will throw with a helpful message and as soon as the
        // type is fixed, the key warnings will appear.)
        for &child in children {
            validate_child_keys(child);
        }

        // Unlike the jsx() runtime, createElement() doesn't warn about key spread.
    }

    // Reserved names are extracted
    let props = js::unknown(js::dict::new::<Option<&Unknown>>());

    let mut key = None;

    if let Some(config) = config {
        if __DEV__
            && !DID_WARN_ABOUT_OLD_JSX_RUNTIME.get()
            && js::has(config, "__self")
            // Do not assume this is the result of an oudated JSX transform if key
            // is present, because the modern JSX transform sometimes outputs
            // createElement to preserve precedence between a static key and a
            // spread key. To avoid false positive warnings, we never warn if
            // there's a key.
            && !js::has(config, "key")
        {
            DID_WARN_ABOUT_OLD_JSX_RUNTIME.set(true);
            console::warn((concat!(
                "Your app (or one of its dependencies) is using an outdated JSX ",
                "transform. Update to the modern JSX transform for ",
                "faster performance: https://react.dev/link/new-jsx-transform"
            ),));
        }

        if has_valid_key(config) {
            if ENABLE_OPTIMISTIC_KEY.with(|&enabled| enabled)
                && SymbolFor::from_value(js::get(config, "key")) == Some(REACT_OPTIMISTIC_KEY)
            {
                key = Some(ReactKey::Optimistic(REACT_OPTIMISTIC_KEY));
            } else {
                if __DEV__ {
                    check_key_string_coercion(js::get(config, "key"));
                }
                key = Some(ReactKey::Text(
                    js::concat_text(js::get(config, "key")).unwrap(),
                ));
            }
        }

        // Remaining properties are added to a new props object
        for prop_name in js::keys_in(config) {
            if HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(config, &prop_name))
                // Skip over reserved prop names
                && prop_name != "key"
                // Even though we don't use these anymore in the runtime, we don't want
                // them to appear as props, so in createElement we filter them out.
                // We don't have to do this in the jsx() runtime because the jsx()
                // transform never passed these as props; it used separate arguments.
                && prop_name != "__self"
                && prop_name != "__source"
            {
                js::set(props, &prop_name, js::get(config, &prop_name));
            }
        }
    }

    // Children can be more than one argument, and those are transferred onto
    // the newly allocated props object.
    let children_length = children.len();
    if children_length == 1 {
        js::set(props, "children", children[0]);
    } else if children_length > 1 {
        let child_array = children.to_vec();
        if __DEV__ && js::is_defined(object::freeze::<Unknown>) {
            object::freeze(&child_array);
        }
        js::set(props, "children", child_array);
    }

    // Resolve default props
    if js::truthy(r#type) && js::truthy(js::get(r#type, "defaultProps")) {
        let default_props: &Unknown = unsafe { js::cast(js::get(r#type, "defaultProps")) };
        for prop_name in js::keys_in(default_props) {
            if js::strict_equals(js::get(props, &prop_name), None::<&Unknown>) {
                js::set(props, &prop_name, js::get(default_props, &prop_name));
            }
        }
    }
    if __DEV__ && js::truthy(key.as_ref()) {
        let display_name = if js::type_of(r#type) == "function" {
            js::get(r#type, "displayName")
                .filter(|name| js::truthy(Some(*name)))
                .or_else(|| js::get(r#type, "name"))
                .filter(|name| js::truthy(Some(*name)))
                .or_else(|| Some(js::unknown("Unknown")))
        } else {
            r#type
        };
        define_key_prop_warning_getter(props, display_name);
    }
    let track_actual_owner = __DEV__ && {
        let recently_created_owner_stacks = REACT_SHARED_INTERNALS.recently_created_owner_stacks();
        REACT_SHARED_INTERNALS
            .set_recently_created_owner_stacks(recently_created_owner_stacks + 1.0);
        recently_created_owner_stacks < OWNER_STACK_LIMIT
    };
    let mut debug_stack_dev = None;
    if __DEV__ {
        if track_actual_owner {
            let previous_stack_trace_limit = js_error::stack_trace_limit();
            js_error::set_stack_trace_limit(OWNER_STACK_TRACE_LIMIT);
            debug_stack_dev = Some(js::error::new("react-stack-top-frame"));
            js_error::set_stack_trace_limit(previous_stack_trace_limit);
        } else {
            debug_stack_dev = UNKNOWN_OWNER_DEBUG_STACK.get();
        }
    }
    react_element(
        r#type,
        key,
        props,
        get_owner(),
        debug_stack_dev,
        if __DEV__ {
            if track_actual_owner {
                CREATE_TASK.with(|create_task| create_task(&get_task_name(r#type)))
            } else {
                UNKNOWN_OWNER_DEBUG_TASK.get()
            }
        } else {
            None
        },
    )
}

pub fn clone_and_replace_key(
    old_element: &ReactElement,
    new_key: Option<ReactKey>,
) -> ReactElement {
    let cloned_element = react_element(
        old_element.r#type,
        new_key,
        old_element.props,
        if !__DEV__ { None } else { old_element.owner },
        if __DEV__ {
            unsafe { js::cast(js::get(old_element, "_debugStack")) }
        } else {
            None
        },
        if __DEV__ {
            unsafe { js::cast(js::get(old_element, "_debugTask")) }
        } else {
            None
        },
    );
    if __DEV__ {
        // The cloned element should inherit the original element's key validation.
        if js::truthy(old_element.store.get()) {
            js::set(
                cloned_element.store.get(),
                "validated",
                js::get(old_element.store.get(), "validated"),
            );
        }
    }
    cloned_element
}

/// Clone and return a new ReactElement using element as the starting point.
/// See https://reactjs.org/docs/react-api.html#cloneelement
pub fn clone_element(
    element: Option<&'static Unknown>,
    config: Option<&'static Unknown>,
    #[cfg_attr(rust_js, rust_js::rest)] children: &[Option<&'static Unknown>],
) -> ReactElement {
    let Some(element) = element else {
        panic!(
            "The argument must be a React element, but you passed {}.",
            js::shown(element)
        );
    };
    let element: &ReactElement = unsafe { js::cast(Some(element)) };

    // Original props are copied
    let props = assign(
        js::unknown(js::dict::new::<Option<&Unknown>>()),
        (element.props,),
    );

    // Reserved names are extracted
    let mut key = element.key.clone();

    // Owner will be preserved, unless ref is overridden
    let mut owner = if !__DEV__ { None } else { element.owner };

    if let Some(config) = config {
        if has_valid_ref(config) {
            owner = if __DEV__ { get_owner() } else { None };
        }
        if has_valid_key(config) {
            if ENABLE_OPTIMISTIC_KEY.with(|&enabled| enabled)
                && SymbolFor::from_value(js::get(config, "key")) == Some(REACT_OPTIMISTIC_KEY)
            {
                key = Some(ReactKey::Optimistic(REACT_OPTIMISTIC_KEY));
            } else {
                if __DEV__ {
                    check_key_string_coercion(js::get(config, "key"));
                }
                key = Some(ReactKey::Text(
                    js::concat_text(js::get(config, "key")).unwrap(),
                ));
            }
        }

        // Remaining properties override existing props
        for prop_name in js::keys_in(config) {
            if HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(config, &prop_name))
                // Skip over reserved prop names
                && prop_name != "key"
                // ...and maybe these, too, though we currently rely on them for
                // warnings and debug information in dev. Need to decide if we're OK
                // with dropping them. In the jsx() runtime it's not an issue because
                // the data gets passed as separate arguments instead of props, but
                // it would be nice to stop relying on them entirely so we can drop
                // them from the internal Fiber field.
                && prop_name != "__self"
                && prop_name != "__source"
                // Undefined `ref` is ignored by cloneElement. We treat it the same as
                // if the property were missing. This is mostly for
                // backwards compatibility.
                && (prop_name != "ref"
                    || !js::strict_equals(js::get(config, "ref"), None::<&Unknown>))
            {
                js::set(props, &prop_name, js::get(config, &prop_name));
            }
        }
    }

    // Children can be more than one argument, and those are transferred onto
    // the newly allocated props object.
    let children_length = children.len();
    if children_length == 1 {
        js::set(props, "children", children[0]);
    } else if children_length > 1 {
        let child_array = children.to_vec();
        js::set(props, "children", child_array);
    }

    let cloned_element = react_element(
        element.r#type,
        key,
        props,
        owner,
        if __DEV__ {
            unsafe { js::cast(js::get(element, "_debugStack")) }
        } else {
            None
        },
        if __DEV__ {
            unsafe { js::cast(js::get(element, "_debugTask")) }
        } else {
            None
        },
    );

    for &child in children {
        validate_child_keys(child);
    }

    cloned_element
}

/// Ensure that every element either is passed in a static location, in an
/// array with an explicit keys property defined, or in an object literal
/// with valid key property.
///
/// @internal
/// @param {ReactNode} node Statically passed child of any type.
fn validate_child_keys(node: Option<&'static Unknown>) {
    if __DEV__ {
        // Mark elements as being in a valid static child position so they
        // don't need keys.
        if is_valid_element(node) {
            if js::truthy(js::get(node, "_store")) {
                js::set(js::get(node, "_store"), "validated", 1.0);
            }
        } else if is_lazy_type(node) {
            if js::strict_equals(js::get(js::get(node, "_payload"), "status"), "fulfilled") {
                if is_valid_element(js::get(js::get(node, "_payload"), "value"))
                    && js::truthy(js::get(
                        js::get(js::get(node, "_payload"), "value"),
                        "_store",
                    ))
                {
                    js::set(
                        js::get(js::get(js::get(node, "_payload"), "value"), "_store"),
                        "validated",
                        1.0,
                    );
                }
            } else if js::truthy(js::get(node, "_store")) {
                js::set(js::get(node, "_store"), "validated", 1.0);
            }
        }
    }
}

/// Verifies the object is a ReactElement.
/// See https://reactjs.org/docs/react-api.html#isvalidelement
/// @param {?object} object
/// @return {boolean} True if `object` is a ReactElement.
/// @final
pub fn is_valid_element(object: Option<&Unknown>) -> bool {
    js::type_of(object) == "object"
        && object.is_some_and(|object| {
            SymbolFor::from_value(js::get(object, "$$typeof")) == Some(REACT_ELEMENT_TYPE)
        })
}

pub fn is_lazy_type(object: Option<&Unknown>) -> bool {
    js::type_of(object) == "object"
        && object.is_some_and(|object| {
            SymbolFor::from_value(js::get(object, "$$typeof")) == Some(REACT_LAZY_TYPE)
        })
}
