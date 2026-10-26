// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use js::{Set, Unknown};
use shared::environment::__DEV__;
use webapi::console;

use crate::get_component_name_from_fiber::get_component_name_from_fiber;
use crate::react_current_fiber::run_with_fiber_in_dev;
use crate::react_internal_types::Fiber;
use crate::react_type_of_mode::{NO_MODE, STRICT_LEGACY_MODE};

type FiberArray = Vec<Rc<Fiber>>;
type FiberToFiberComponentsMap = HashMap<Rc<Fiber>, FiberArray>;

/// What records a warning about a fiber's component, given its instance.
pub type RecordWarning = fn(&Rc<Fiber>, Option<&'static Unknown>);

pub struct ReactStrictModeWarnings {
    pub record_unsafe_lifecycle_warnings: Cell<RecordWarning>,
    pub flush_pending_unsafe_lifecycle_warnings: Cell<fn()>,
    pub record_legacy_context_warning: Cell<RecordWarning>,
    pub flush_legacy_context_warning: Cell<fn()>,
    pub discard_pending_warnings: Cell<fn()>,
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "ReactStrictModeWarnings")]
    pub static REACT_STRICT_MODE_WARNINGS: ReactStrictModeWarnings = ReactStrictModeWarnings {
        record_unsafe_lifecycle_warnings: Cell::new(|_fiber, _instance| {}),
        flush_pending_unsafe_lifecycle_warnings: Cell::new(|| {}),
        record_legacy_context_warning: Cell::new(|_fiber, _instance| {}),
        flush_legacy_context_warning: Cell::new(|| {}),
        discard_pending_warnings: Cell::new(|| {}),
    };

    static PENDING_COMPONENT_WILL_MOUNT_WARNINGS: RefCell<FiberArray> =
        const { RefCell::new(Vec::new()) };
    #[cfg_attr(rust_js, rust_js::name = "pendingUNSAFE_ComponentWillMountWarnings")]
    static PENDING_UNSAFE_COMPONENT_WILL_MOUNT_WARNINGS: RefCell<FiberArray> =
        const { RefCell::new(Vec::new()) };
    static PENDING_COMPONENT_WILL_RECEIVE_PROPS_WARNINGS: RefCell<FiberArray> =
        const { RefCell::new(Vec::new()) };
    #[cfg_attr(rust_js, rust_js::name = "pendingUNSAFE_ComponentWillReceivePropsWarnings")]
    static PENDING_UNSAFE_COMPONENT_WILL_RECEIVE_PROPS_WARNINGS: RefCell<FiberArray> =
        const { RefCell::new(Vec::new()) };
    static PENDING_COMPONENT_WILL_UPDATE_WARNINGS: RefCell<FiberArray> =
        const { RefCell::new(Vec::new()) };
    #[cfg_attr(rust_js, rust_js::name = "pendingUNSAFE_ComponentWillUpdateWarnings")]
    static PENDING_UNSAFE_COMPONENT_WILL_UPDATE_WARNINGS: RefCell<FiberArray> =
        const { RefCell::new(Vec::new()) };

    // Tracks components we have already warned about.
    static DID_WARN_ABOUT_UNSAFE_LIFECYCLES: &'static Set<Option<&'static Unknown>> =
        js::set::new(None);

    static PENDING_LEGACY_CONTEXT_WARNING: RefCell<FiberToFiberComponentsMap> =
        RefCell::new(HashMap::new());

    // Tracks components we have already warned about.
    static DID_WARN_ABOUT_LEGACY_CONTEXT: &'static Set<Option<&'static Unknown>> =
        js::set::new(None);
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn find_strict_root(fiber: &Rc<Fiber>) -> Option<Rc<Fiber>> {
    let mut maybe_strict_root = None;

    let mut node = Some(Rc::clone(fiber));
    while let Some(current) = node {
        if (current.mode.get() & STRICT_LEGACY_MODE) != NO_MODE {
            maybe_strict_root = Some(Rc::clone(&current));
        }
        node = current.r#return.borrow().clone();
    }

    maybe_strict_root
}

fn set_to_sorted_string(set: &HashSet<String>) -> String {
    let mut array: Vec<&str> = Vec::new();
    for value in set {
        array.push(value);
    }
    array.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    array.join(", ")
}

js::on_load! {
    if __DEV__ {
        REACT_STRICT_MODE_WARNINGS.with(|react_strict_mode_warnings| {
            react_strict_mode_warnings.record_unsafe_lifecycle_warnings.set(|fiber, instance| {
                // Dedupe strategy: Warn once per component.
                if DID_WARN_ABOUT_UNSAFE_LIFECYCLES.with(|did_warn| did_warn.has(&fiber.r#type.get())) {
                    return;
                }

                if js::type_of(js::get(instance, "componentWillMount")) == "function"
                    // Don't warn about react-lifecycles-compat polyfilled components.
                    && !js::strict_equals(
                        js::get(js::get(instance, "componentWillMount"), "__suppressDeprecationWarning"),
                        true,
                    ) {
                    PENDING_COMPONENT_WILL_MOUNT_WARNINGS
                        .with_borrow_mut(|pending| pending.push(Rc::clone(fiber)));
                }

                if (fiber.mode.get() & STRICT_LEGACY_MODE) != NO_MODE
                    && js::type_of(js::get(instance, "UNSAFE_componentWillMount")) == "function"
                {
                    PENDING_UNSAFE_COMPONENT_WILL_MOUNT_WARNINGS
                        .with_borrow_mut(|pending| pending.push(Rc::clone(fiber)));
                }

                if js::type_of(js::get(instance, "componentWillReceiveProps")) == "function"
                    // Don't warn about react-lifecycles-compat polyfilled components.
                    && !js::strict_equals(
                        js::get(js::get(instance, "componentWillReceiveProps"), "__suppressDeprecationWarning"),
                        true,
                    ) {
                    PENDING_COMPONENT_WILL_RECEIVE_PROPS_WARNINGS
                        .with_borrow_mut(|pending| pending.push(Rc::clone(fiber)));
                }

                if (fiber.mode.get() & STRICT_LEGACY_MODE) != NO_MODE
                    && js::type_of(js::get(instance, "UNSAFE_componentWillReceiveProps"))
                        == "function"
                {
                    PENDING_UNSAFE_COMPONENT_WILL_RECEIVE_PROPS_WARNINGS
                        .with_borrow_mut(|pending| pending.push(Rc::clone(fiber)));
                }

                if js::type_of(js::get(instance, "componentWillUpdate")) == "function"
                    // Don't warn about react-lifecycles-compat polyfilled components.
                    && !js::strict_equals(
                        js::get(js::get(instance, "componentWillUpdate"), "__suppressDeprecationWarning"),
                        true,
                    ) {
                    PENDING_COMPONENT_WILL_UPDATE_WARNINGS
                        .with_borrow_mut(|pending| pending.push(Rc::clone(fiber)));
                }

                if (fiber.mode.get() & STRICT_LEGACY_MODE) != NO_MODE
                    && js::type_of(js::get(instance, "UNSAFE_componentWillUpdate")) == "function"
                {
                    PENDING_UNSAFE_COMPONENT_WILL_UPDATE_WARNINGS
                        .with_borrow_mut(|pending| pending.push(Rc::clone(fiber)));
                }
            });

            react_strict_mode_warnings.flush_pending_unsafe_lifecycle_warnings.set(|| {
                // We do an initial pass to gather component names
                let mut component_will_mount_unique_names = HashSet::new();
                if PENDING_COMPONENT_WILL_MOUNT_WARNINGS.with_borrow(|pending| !pending.is_empty()) {
                    PENDING_COMPONENT_WILL_MOUNT_WARNINGS.with_borrow(|pending| {
                        for fiber in pending {
                                component_will_mount_unique_names.insert(
                                    get_component_name_from_fiber(fiber)
                                        .filter(|name| !name.is_empty())
                                        .unwrap_or_else(|| "Component".to_string()),
                                );
                                DID_WARN_ABOUT_UNSAFE_LIFECYCLES
                                    .with(|did_warn| did_warn.add(&fiber.r#type.get()));
                        }
                    });
                    PENDING_COMPONENT_WILL_MOUNT_WARNINGS.set(Vec::new());
                }

                let mut unsafe_component_will_mount_unique_names = HashSet::new();
                if PENDING_UNSAFE_COMPONENT_WILL_MOUNT_WARNINGS.with_borrow(|pending| !pending.is_empty()) {
                    PENDING_UNSAFE_COMPONENT_WILL_MOUNT_WARNINGS.with_borrow(|pending| {
                        for fiber in pending {
                                unsafe_component_will_mount_unique_names.insert(
                                    get_component_name_from_fiber(fiber)
                                        .filter(|name| !name.is_empty())
                                        .unwrap_or_else(|| "Component".to_string()),
                                );
                                DID_WARN_ABOUT_UNSAFE_LIFECYCLES
                                    .with(|did_warn| did_warn.add(&fiber.r#type.get()));
                        }
                    });
                    PENDING_UNSAFE_COMPONENT_WILL_MOUNT_WARNINGS.set(Vec::new());
                }

                let mut component_will_receive_props_unique_names = HashSet::new();
                if PENDING_COMPONENT_WILL_RECEIVE_PROPS_WARNINGS.with_borrow(|pending| !pending.is_empty()) {
                    PENDING_COMPONENT_WILL_RECEIVE_PROPS_WARNINGS.with_borrow(|pending| {
                        for fiber in pending {
                                component_will_receive_props_unique_names.insert(
                                    get_component_name_from_fiber(fiber)
                                        .filter(|name| !name.is_empty())
                                        .unwrap_or_else(|| "Component".to_string()),
                                );
                                DID_WARN_ABOUT_UNSAFE_LIFECYCLES
                                    .with(|did_warn| did_warn.add(&fiber.r#type.get()));
                        }
                    });
                    PENDING_COMPONENT_WILL_RECEIVE_PROPS_WARNINGS.set(Vec::new());
                }

                let mut unsafe_component_will_receive_props_unique_names = HashSet::new();
                if PENDING_UNSAFE_COMPONENT_WILL_RECEIVE_PROPS_WARNINGS.with_borrow(|pending| !pending.is_empty()) {
                    PENDING_UNSAFE_COMPONENT_WILL_RECEIVE_PROPS_WARNINGS.with_borrow(|pending| {
                        for fiber in pending {
                                unsafe_component_will_receive_props_unique_names.insert(
                                    get_component_name_from_fiber(fiber)
                                        .filter(|name| !name.is_empty())
                                        .unwrap_or_else(|| "Component".to_string()),
                                );
                                DID_WARN_ABOUT_UNSAFE_LIFECYCLES
                                    .with(|did_warn| did_warn.add(&fiber.r#type.get()));
                        }
                    });
                    PENDING_UNSAFE_COMPONENT_WILL_RECEIVE_PROPS_WARNINGS.set(Vec::new());
                }

                let mut component_will_update_unique_names = HashSet::new();
                if PENDING_COMPONENT_WILL_UPDATE_WARNINGS.with_borrow(|pending| !pending.is_empty()) {
                    PENDING_COMPONENT_WILL_UPDATE_WARNINGS.with_borrow(|pending| {
                        for fiber in pending {
                                component_will_update_unique_names.insert(
                                    get_component_name_from_fiber(fiber)
                                        .filter(|name| !name.is_empty())
                                        .unwrap_or_else(|| "Component".to_string()),
                                );
                                DID_WARN_ABOUT_UNSAFE_LIFECYCLES
                                    .with(|did_warn| did_warn.add(&fiber.r#type.get()));
                        }
                    });
                    PENDING_COMPONENT_WILL_UPDATE_WARNINGS.set(Vec::new());
                }

                let mut unsafe_component_will_update_unique_names = HashSet::new();
                if PENDING_UNSAFE_COMPONENT_WILL_UPDATE_WARNINGS.with_borrow(|pending| !pending.is_empty()) {
                    PENDING_UNSAFE_COMPONENT_WILL_UPDATE_WARNINGS.with_borrow(|pending| {
                        for fiber in pending {
                                unsafe_component_will_update_unique_names.insert(
                                    get_component_name_from_fiber(fiber)
                                        .filter(|name| !name.is_empty())
                                        .unwrap_or_else(|| "Component".to_string()),
                                );
                                DID_WARN_ABOUT_UNSAFE_LIFECYCLES
                                    .with(|did_warn| did_warn.add(&fiber.r#type.get()));
                        }
                    });
                    PENDING_UNSAFE_COMPONENT_WILL_UPDATE_WARNINGS.set(Vec::new());
                }

                // Finally, we flush all the warnings
                // UNSAFE_ ones before the deprecated ones, since they'll be 'louder'
                if !unsafe_component_will_mount_unique_names.is_empty() {
                    let sorted_names =
                        set_to_sorted_string(&unsafe_component_will_mount_unique_names);
                    console::error((
                        concat!(
                            "Using UNSAFE_componentWillMount in strict mode is not recommended and may indicate bugs in your code. ",
                            "See https://react.dev/link/unsafe-component-lifecycles for details.\n\n",
                            "* Move code with side effects to componentDidMount, and set initial state in the constructor.\n",
                            "\nPlease update the following components: %s"
                        ),
                        sorted_names,
                    ));
                }

                if !unsafe_component_will_receive_props_unique_names.is_empty() {
                    let sorted_names =
                        set_to_sorted_string(&unsafe_component_will_receive_props_unique_names);
                    console::error((
                        concat!(
                            "Using UNSAFE_componentWillReceiveProps in strict mode is not recommended ",
                            "and may indicate bugs in your code. ",
                            "See https://react.dev/link/unsafe-component-lifecycles for details.\n\n",
                            "* Move data fetching code or side effects to componentDidUpdate.\n",
                            "* If you're updating state whenever props change, ",
                            "refactor your code to use memoization techniques or move it to ",
                            "static getDerivedStateFromProps. Learn more at: https://react.dev/link/derived-state\n",
                            "\nPlease update the following components: %s"
                        ),
                        sorted_names,
                    ));
                }

                if !unsafe_component_will_update_unique_names.is_empty() {
                    let sorted_names =
                        set_to_sorted_string(&unsafe_component_will_update_unique_names);
                    console::error((
                        concat!(
                            "Using UNSAFE_componentWillUpdate in strict mode is not recommended ",
                            "and may indicate bugs in your code. ",
                            "See https://react.dev/link/unsafe-component-lifecycles for details.\n\n",
                            "* Move data fetching code or side effects to componentDidUpdate.\n",
                            "\nPlease update the following components: %s"
                        ),
                        sorted_names,
                    ));
                }

                if !component_will_mount_unique_names.is_empty() {
                    let sorted_names = set_to_sorted_string(&component_will_mount_unique_names);

                    console::warn((
                        concat!(
                            "componentWillMount has been renamed, and is not recommended for use. ",
                            "See https://react.dev/link/unsafe-component-lifecycles for details.\n\n",
                            "* Move code with side effects to componentDidMount, and set initial state in the constructor.\n",
                            "* Rename componentWillMount to UNSAFE_componentWillMount to suppress ",
                            "this warning in non-strict mode. In React 18.x, only the UNSAFE_ name will work. ",
                            "To rename all deprecated lifecycles to their new names, you can run ",
                            "`npx react-codemod rename-unsafe-lifecycles` in your project source folder.\n",
                            "\nPlease update the following components: %s"
                        ),
                        sorted_names,
                    ));
                }

                if !component_will_receive_props_unique_names.is_empty() {
                    let sorted_names =
                        set_to_sorted_string(&component_will_receive_props_unique_names);

                    console::warn((
                        concat!(
                            "componentWillReceiveProps has been renamed, and is not recommended for use. ",
                            "See https://react.dev/link/unsafe-component-lifecycles for details.\n\n",
                            "* Move data fetching code or side effects to componentDidUpdate.\n",
                            "* If you're updating state whenever props change, refactor your ",
                            "code to use memoization techniques or move it to ",
                            "static getDerivedStateFromProps. Learn more at: https://react.dev/link/derived-state\n",
                            "* Rename componentWillReceiveProps to UNSAFE_componentWillReceiveProps to suppress ",
                            "this warning in non-strict mode. In React 18.x, only the UNSAFE_ name will work. ",
                            "To rename all deprecated lifecycles to their new names, you can run ",
                            "`npx react-codemod rename-unsafe-lifecycles` in your project source folder.\n",
                            "\nPlease update the following components: %s"
                        ),
                        sorted_names,
                    ));
                }

                if !component_will_update_unique_names.is_empty() {
                    let sorted_names = set_to_sorted_string(&component_will_update_unique_names);

                    console::warn((
                        concat!(
                            "componentWillUpdate has been renamed, and is not recommended for use. ",
                            "See https://react.dev/link/unsafe-component-lifecycles for details.\n\n",
                            "* Move data fetching code or side effects to componentDidUpdate.\n",
                            "* Rename componentWillUpdate to UNSAFE_componentWillUpdate to suppress ",
                            "this warning in non-strict mode. In React 18.x, only the UNSAFE_ name will work. ",
                            "To rename all deprecated lifecycles to their new names, you can run ",
                            "`npx react-codemod rename-unsafe-lifecycles` in your project source folder.\n",
                            "\nPlease update the following components: %s"
                        ),
                        sorted_names,
                    ));
                }
            });

            react_strict_mode_warnings.record_legacy_context_warning.set(|fiber, instance| {
                let strict_root = find_strict_root(fiber);
                let Some(strict_root) = strict_root else {
                    console::error((concat!(
                        "Expected to find a StrictMode component in a strict mode tree. ",
                        "This error is likely caused by a bug in React. Please file an issue."
                    ),));
                    return;
                };

                // Dedup strategy: Warn once per component.
                if DID_WARN_ABOUT_LEGACY_CONTEXT.with(|did_warn| did_warn.has(&fiber.r#type.get())) {
                    return;
                }

                if js::get(fiber.r#type.get(), "contextTypes").is_some()
                    || js::get(fiber.r#type.get(), "childContextTypes").is_some()
                    || (instance.is_some()
                        && js::type_of(js::get(instance, "getChildContext")) == "function")
                {
                    PENDING_LEGACY_CONTEXT_WARNING.with_borrow_mut(|pending_legacy_context_warning| {
                        pending_legacy_context_warning
                            .entry(strict_root)
                            .or_default()
                            .push(Rc::clone(fiber));
                    });
                }
            });

            react_strict_mode_warnings.flush_legacy_context_warning.set(|| {
                PENDING_LEGACY_CONTEXT_WARNING.with_borrow(|pending_legacy_context_warning| {
                    for fiber_array in pending_legacy_context_warning.values() {
                        if fiber_array.is_empty() {
                            continue;
                        }
                        let first_fiber = &fiber_array[0];

                        let mut unique_names = HashSet::new();
                        for fiber in fiber_array {
                            unique_names.insert(
                                get_component_name_from_fiber(fiber)
                                    .filter(|name| !name.is_empty())
                                    .unwrap_or_else(|| "Component".to_string()),
                            );
                            DID_WARN_ABOUT_LEGACY_CONTEXT
                                .with(|did_warn| did_warn.add(&fiber.r#type.get()));
                        }

                        let sorted_names = set_to_sorted_string(&unique_names);

                        run_with_fiber_in_dev(
                            Some(Rc::clone(first_fiber)),
                            |_: (), _: (), _: (), _: (), _: ()| {
                                console::error((
                                    concat!(
                                        "Legacy context API has been detected within a strict-mode tree.",
                                        "\n\nThe old API will be supported in all 16.x releases, but applications ",
                                        "using it should migrate to the new version.",
                                        "\n\nPlease update the following components: %s",
                                        "\n\nLearn more about this warning here: https://react.dev/link/legacy-context"
                                    ),
                                    &sorted_names,
                                ));
                            },
                            (),
                            (),
                            (),
                            (),
                            (),
                        );
                    }
                });
            });

            react_strict_mode_warnings.discard_pending_warnings.set(|| {
                PENDING_COMPONENT_WILL_MOUNT_WARNINGS.set(Vec::new());
                PENDING_UNSAFE_COMPONENT_WILL_MOUNT_WARNINGS.set(Vec::new());
                PENDING_COMPONENT_WILL_RECEIVE_PROPS_WARNINGS.set(Vec::new());
                PENDING_UNSAFE_COMPONENT_WILL_RECEIVE_PROPS_WARNINGS.set(Vec::new());
                PENDING_COMPONENT_WILL_UPDATE_WARNINGS.set(Vec::new());
                PENDING_UNSAFE_COMPONENT_WILL_UPDATE_WARNINGS.set(Vec::new());
                PENDING_LEGACY_CONTEXT_WARNING.set(HashMap::new());
            });
        });
    }
}

js::export_default!(REACT_STRICT_MODE_WARNINGS);
