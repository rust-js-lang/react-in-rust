// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use js::{JsObject, Kind, PropertyDescriptor, SymbolFor, Unknown, WeakMap, object};
use react_reconciler::react_fiber_error_logger::{
    default_on_caught_error, default_on_recoverable_error, default_on_uncaught_error,
};
use react_reconciler::react_fiber_reconciler::ReactFiberReconciler;
use react_reconciler::react_fiber_tree_reflection::find_current_fiber_using_slow_path;
use react_reconciler::react_internal_types::Fiber;
use react_reconciler::react_root_tags::{CONCURRENT_ROOT, LEGACY_ROOT};
use react_reconciler::react_work_tags::{
    CLASS_COMPONENT, CONTEXT_CONSUMER, CONTEXT_PROVIDER, FORWARD_REF, FRAGMENT, FUNCTION_COMPONENT,
    HOST_COMPONENT, HOST_HOISTABLE, HOST_PORTAL, HOST_ROOT, HOST_SINGLETON, HOST_TEXT,
    INCOMPLETE_CLASS_COMPONENT, MEMO_COMPONENT, MODE, PROFILER, SCOPE_COMPONENT,
    SIMPLE_MEMO_COMPONENT, WorkTag,
};
use shared::check_string_coercion::check_prop_string_coercion;
use shared::environment::__DEV__;
use shared::get_component_name_from_type::get_component_name_from_type;
use shared::react_feature_flags::{DISABLE_LEGACY_MODE, ENABLE_REACT_TEST_RENDERER_WARNING};
use shared::react_types::ReactNodeList;
use webapi::console;

use crate::react_fiber_config_test_host::{
    Child, Container, ContainerTag, NodeMockElement, TextInstance, get_public_instance,
};

unsafe extern "Rust" {
    #[link_name = "scheduler/unstable_mock#* as Scheduler"]
    pub safe static SCHEDULER: &'static JsObject;
    // This is only in the development export.
    #[link_name = "react#*.act"]
    safe static REACT_ACT: Option<&'static Unknown>;
    // What a React Native test environment sets.
    #[link_name = "global.IS_REACT_NATIVE_TEST_ENVIRONMENT"]
    safe static IS_REACT_NATIVE_TEST_ENVIRONMENT: Option<&'static Unknown>;
}

fn default_on_default_transition_indicator() -> Option<&'static dyn Fn()> {
    None
}

thread_local! {
    pub static ACT: Option<&'static Unknown> = REACT_ACT;
}

// TODO: Remove from public bundle

/// The options `create` takes.
pub struct TestRendererOptions {
    // A function, `(element) => any`.
    pub create_node_mock: Option<&'static Unknown>,
    #[cfg_attr(rust_js, rust_js::name = "unstable_isConcurrent")]
    pub unstable_is_concurrent: Option<bool>,
    #[cfg_attr(rust_js, rust_js::name = "unstable_strictMode")]
    pub unstable_strict_mode: Option<&'static Unknown>,
}

unsafe impl js::Defined for TestRendererOptions {}

/// What `toJSON` gives of a host instance: its type, its props but
/// `children`, and its rendered children's.
struct ReactTestRendererJSON {
    pub r#type: &'static str,
    pub props: &'static JsObject,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub children: Option<Vec<&'static Unknown>>,
}

unsafe impl js::Defined for ReactTestRendererJSON {}

/// `findAll`'s options.
pub struct FindOptions {
    // performs a "greedy" search: if a matching node is found, will continue
    // to search within the matching node's children. (default: true)
    pub deep: Option<bool>,
}

unsafe impl js::Defined for FindOptions {}

pub type Predicate = dyn Fn(&Rc<ReactTestInstance>) -> Option<bool>;

struct DefaultTestOptions {
    create_node_mock: fn(&NodeMockElement) -> Option<&'static Unknown>,
}

#[cfg_attr(rust_js, rust_js::name = "defaultTestOptions")]
static DEFAULT_TEST_OPTIONS: DefaultTestOptions = DefaultTestOptions {
    create_node_mock: |_element| js::null(),
};

#[cfg_attr(rust_js, rust_js::name = "toJSON", rust_js::nullable)]
fn to_json(inst: &Child) -> Option<&'static Unknown> {
    if is_hidden(inst) {
        // Omit timed out children from output entirely. This seems like the least
        // surprising behavior. We could perhaps add a separate API that includes
        // them, if it turns out people need it.
        return None;
    }
    match inst {
        Child::Text(inst) => Some(js::unknown(inst.text.get())),
        Child::Instance(inst) => {
            // We don't include the `children` prop in JSON.
            // Instead, we will include the actual rendered children.
            let props = object::spread(Some(inst.props.get()));
            js::delete(props, "children");
            let children = inst.children.borrow().clone();
            let mut rendered_children: Option<Vec<&'static Unknown>> = None;
            for child in &children {
                if let Some(rendered_child) = to_json(child) {
                    match &mut rendered_children {
                        None => rendered_children = Some(vec![rendered_child]),
                        Some(rendered_children) => rendered_children.push(rendered_child),
                    }
                }
            }
            let json = js::unknown_of(ReactTestRendererJSON {
                r#type: inst.r#type.get(),
                props,
                children: rendered_children,
            });
            object::define_property(
                json,
                "$$typeof",
                &PropertyDescriptor {
                    value: Some(&SymbolFor("react.test.json")),
                    ..Default::default()
                },
            );
            Some(json)
        }
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn children_to_tree(node: Option<Rc<Fiber>>) -> Option<&'static Unknown> {
    let node = node?;
    let children = node_and_siblings_array(Some(node));
    if children.is_empty() {
        return None;
    } else if children.len() == 1 {
        return to_tree(children.first().cloned());
    }
    Some(js::unknown_of(flatten(
        &children
            .into_iter()
            .map(|child| to_tree(Some(child)))
            .collect::<Vec<_>>(),
    )))
}

fn node_and_siblings_array(node_with_sibling: Option<Rc<Fiber>>) -> Vec<Rc<Fiber>> {
    let mut array = Vec::new();
    let mut node = node_with_sibling;
    while let Some(current) = node {
        node = current.sibling.borrow().clone();
        array.push(current);
    }
    array
}

/// Where `flatten` is in one of the arrays it flattens.
struct Frame<'a> {
    i: usize,
    array: &'a [Option<&'static Unknown>],
}

fn flatten(arr: &[Option<&'static Unknown>]) -> Vec<Option<&'static Unknown>> {
    let mut result = Vec::new();
    let mut stack = vec![Frame { i: 0, array: arr }];
    while let Some(mut n) = stack.pop() {
        while n.i < n.array.len() {
            let el = n.array[n.i];
            n.i += 1;
            if let Kind::Array(el) = js::classify(el) {
                stack.push(n);
                stack.push(Frame { i: 0, array: el });
                break;
            }
            result.push(el);
        }
    }
    result
}

/// What `toTree` gives of a component or a host component.
struct TreeNode {
    pub node_type: &'static str,
    pub r#type: Option<&'static Unknown>,
    pub props: &'static JsObject,
    pub instance: Option<&'static Unknown>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub rendered: Option<&'static Unknown>,
}

unsafe impl js::Defined for TreeNode {}

#[cfg_attr(rust_js, rust_js::nullable)]
fn to_tree(node: Option<Rc<Fiber>>) -> Option<&'static Unknown> {
    let node = node?;
    match node.tag.get() {
        HOST_ROOT | HOST_PORTAL => children_to_tree(node.child.borrow().clone()),
        CLASS_COMPONENT => Some(js::unknown_of(TreeNode {
            node_type: "component",
            r#type: node.r#type.get(),
            props: object::spread(node.memoized_props.get()),
            instance: node.state_node.get(),
            rendered: children_to_tree(node.child.borrow().clone()),
        })),
        FUNCTION_COMPONENT | SIMPLE_MEMO_COMPONENT => Some(js::unknown_of(TreeNode {
            node_type: "component",
            r#type: node.r#type.get(),
            props: object::spread(node.memoized_props.get()),
            instance: js::null(),
            rendered: children_to_tree(node.child.borrow().clone()),
        })),
        HOST_HOISTABLE | HOST_SINGLETON | HOST_COMPONENT => Some(js::unknown_of(TreeNode {
            node_type: "host",
            r#type: node.r#type.get(),
            props: object::spread(node.memoized_props.get()),
            instance: js::null(), // TODO: use createNodeMock here somehow?
            rendered: Some(js::unknown_of(flatten(
                &node_and_siblings_array(node.child.borrow().clone())
                    .into_iter()
                    .map(|child| to_tree(Some(child)))
                    .collect::<Vec<_>>(),
            ))),
        })),
        HOST_TEXT => Some(js::unknown(
            unsafe { js::cast::<&TextInstance>(node.state_node.get()) }
                .text
                .get(),
        )),
        FRAGMENT
        | CONTEXT_PROVIDER
        | CONTEXT_CONSUMER
        | MODE
        | PROFILER
        | FORWARD_REF
        | MEMO_COMPONENT
        | INCOMPLETE_CLASS_COMPONENT
        | SCOPE_COMPONENT => children_to_tree(node.child.borrow().clone()),
        _ => panic!(
            "toTree() does not yet know how to handle nodes with tag={}",
            node.tag.get()
        ),
    }
}

thread_local! {
    static VALID_WRAPPER_TYPES: HashSet<WorkTag> = HashSet::from([
        FUNCTION_COMPONENT,
        CLASS_COMPONENT,
        HOST_COMPONENT,
        FORWARD_REF,
        MEMO_COMPONENT,
        SIMPLE_MEMO_COMPONENT,
        // Normally skipped, but used when there's more than one root child.
        HOST_ROOT,
    ]);
}

/// A child of a ReactTestInstance: another instance, or text.
#[derive(Clone)]
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum TestChild {
    Instance(Rc<ReactTestInstance>),
    Text(String),
}

unsafe impl js::Defined for TestChild {}

fn get_children(parent: &Rc<Fiber>) -> Vec<TestChild> {
    let mut children = Vec::new();
    let starting_node = parent;
    let Some(first) = starting_node.child.borrow().clone() else {
        return children;
    };
    first.r#return.replace(Some(Rc::clone(starting_node)));
    let mut node = first;
    'outer: loop {
        let mut descend = false;
        if VALID_WRAPPER_TYPES.with(|types| types.contains(&node.tag.get())) {
            children.push(TestChild::Instance(wrap_fiber(&node)));
        } else if node.tag.get() == HOST_TEXT {
            if __DEV__ {
                check_prop_string_coercion(node.memoized_props.get(), "memoizedProps");
            }
            children.push(TestChild::Text(
                js::concat_text(node.memoized_props.get()).unwrap(),
            ));
        } else {
            descend = true;
        }
        let child = node.child.borrow().clone();
        if descend && let Some(child) = child {
            child.r#return.replace(Some(Rc::clone(&node)));
            node = child;
            continue;
        }
        while node.sibling.borrow().is_none() {
            let parent = node.r#return.borrow().clone().unwrap();
            if Rc::ptr_eq(&parent, starting_node) {
                break 'outer;
            }
            node = parent;
        }
        let sibling = node.sibling.borrow().clone().unwrap();
        let return_fiber = node.r#return.borrow().clone();
        sibling.r#return.replace(return_fiber);
        node = sibling;
    }
    children
}

#[cfg_attr(rust_js, rust_js::class)]
pub struct ReactTestInstance {
    #[cfg_attr(rust_js, rust_js::name = "_fiber")]
    fiber: Rc<Fiber>,
}

unsafe impl js::Defined for ReactTestInstance {}

#[cfg_attr(rust_js, rust_js::class)]
impl ReactTestInstance {
    #[cfg_attr(rust_js, rust_js::name = "_currentFiber")]
    fn current_fiber(&self) -> Rc<Fiber> {
        // Throws if this component has been unmounted.
        let fiber = find_current_fiber_using_slow_path(&self.fiber);

        let Some(fiber) = fiber else {
            panic!(
                "Can't read from currently-mounting component. This error is likely caused by a bug in React. Please file an issue."
            );
        };

        fiber
    }

    fn new(fiber: Rc<Fiber>) -> Self {
        if !VALID_WRAPPER_TYPES.with(|types| types.contains(&fiber.tag.get())) {
            panic!(
                "Unexpected object passed to ReactTestInstance constructor (tag: {}). This is probably a bug in React.",
                fiber.tag.get()
            );
        }

        ReactTestInstance { fiber }
    }

    #[cfg_attr(rust_js, rust_js::getter)]
    pub fn instance(&self) -> Option<&'static Unknown> {
        let tag = self.fiber.tag.get();
        if tag == HOST_COMPONENT || tag == HOST_HOISTABLE || tag == HOST_SINGLETON {
            get_public_instance(unsafe { js::cast(self.fiber.state_node.get()) })
        } else {
            self.fiber.state_node.get()
        }
    }

    #[cfg_attr(rust_js, rust_js::getter)]
    pub fn r#type(&self) -> Option<&'static Unknown> {
        self.fiber.r#type.get()
    }

    #[cfg_attr(rust_js, rust_js::getter)]
    pub fn props(&self) -> Option<&'static Unknown> {
        self.current_fiber().memoized_props.get()
    }

    #[cfg_attr(rust_js, rust_js::getter, rust_js::nullable)]
    pub fn parent(&self) -> Option<Rc<ReactTestInstance>> {
        let mut parent = self.fiber.r#return.borrow().clone();
        while let Some(current) = parent {
            if VALID_WRAPPER_TYPES.with(|types| types.contains(&current.tag.get())) {
                if current.tag.get() == HOST_ROOT {
                    // Special case: we only "materialize" instances for roots
                    // if they have more than a single child. So we'll check that now.
                    if get_children(&current).len() < 2 {
                        return None;
                    }
                }
                return Some(wrap_fiber(&current));
            }
            parent = current.r#return.borrow().clone();
        }
        None
    }

    #[cfg_attr(rust_js, rust_js::getter)]
    pub fn children(&self) -> Vec<TestChild> {
        get_children(&self.current_fiber())
    }

    // Custom search functions
    pub fn find(self: &Rc<Self>, predicate: &Predicate) -> Rc<ReactTestInstance> {
        expect_one(
            self.find_all(predicate, Some(&FindOptions { deep: Some(false) })),
            &format!(
                "matching custom predicate: {}",
                js::shown(js::unknown(predicate))
            ),
        )
    }

    pub fn find_by_type(
        self: &Rc<Self>,
        r#type: Option<&'static Unknown>,
    ) -> Rc<ReactTestInstance> {
        expect_one(
            self.find_all_by_type(r#type, Some(&FindOptions { deep: Some(false) })),
            &format!(
                "with node type: \"{}\"",
                js::or(
                    get_component_name_from_type(r#type)
                        .unwrap_or_default()
                        .as_str(),
                    "Unknown"
                )
            ),
        )
    }

    pub fn find_by_props(self: &Rc<Self>, props: &'static Unknown) -> Rc<ReactTestInstance> {
        expect_one(
            self.find_all_by_props(props, Some(&FindOptions { deep: Some(false) })),
            &format!(
                "with props: {}",
                js::shown(
                    js::json::stringify(props)
                        .unwrap()
                        .as_deref()
                        .map(js::unknown)
                )
            ),
        )
    }

    pub fn find_all(
        self: &Rc<Self>,
        predicate: &Predicate,
        options: Option<&FindOptions>,
    ) -> Vec<Rc<ReactTestInstance>> {
        find_all(self, predicate, options)
    }

    pub fn find_all_by_type(
        self: &Rc<Self>,
        r#type: Option<&'static Unknown>,
        options: Option<&FindOptions>,
    ) -> Vec<Rc<ReactTestInstance>> {
        find_all(
            self,
            &move |node| Some(js::strict_equals(node.r#type(), r#type)),
            options,
        )
    }

    pub fn find_all_by_props(
        self: &Rc<Self>,
        props: &'static Unknown,
        options: Option<&FindOptions>,
    ) -> Vec<Rc<ReactTestInstance>> {
        find_all(
            self,
            &move |node| {
                Some(
                    node.props()
                        .is_some_and(|node_props| props_match(node_props, props)),
                )
            },
            options,
        )
    }
}

fn find_all(
    root: &Rc<ReactTestInstance>,
    predicate: &Predicate,
    options: Option<&FindOptions>,
) -> Vec<Rc<ReactTestInstance>> {
    let deep = match options {
        Some(options) => options.deep,
        None => Some(true),
    };
    let mut results = Vec::new();

    if predicate(root).unwrap_or(false) {
        results.push(Rc::clone(root));
        if !deep.unwrap_or(false) {
            return results;
        }
    }

    for child in root.children() {
        let TestChild::Instance(child) = child else {
            continue;
        };
        results.extend(find_all(&child, predicate, options));
    }

    results
}

fn expect_one(all: Vec<Rc<ReactTestInstance>>, message: &str) -> Rc<ReactTestInstance> {
    if all.len() == 1 {
        return Rc::clone(&all[0]);
    }

    let prefix = if all.is_empty() {
        "No instances found ".to_owned()
    } else {
        format!("Expected 1 but found {} instances ", all.len())
    };

    panic!("{}", prefix + message);
}

fn props_match(props: &Unknown, filter: &Unknown) -> bool {
    for key in js::keys_in(filter) {
        if !js::strict_equals(js::get(props, key.as_str()), js::get(filter, key.as_str())) {
            return false;
        }
    }
    true
}

type FlushSync = dyn Fn(Option<&dyn Fn() -> Option<&'static Unknown>>) -> Option<&'static Unknown>;

/// What `create` gives: the renderer of one element.
pub struct TestRenderer {
    #[cfg_attr(rust_js, rust_js::name = "_Scheduler")]
    pub scheduler: &'static JsObject,
    pub root: Option<&'static Unknown>,
    #[cfg_attr(rust_js, rust_js::name = "toJSON")]
    pub to_json: Box<dyn Fn() -> Option<&'static Unknown>>,
    pub to_tree: Box<dyn Fn() -> Option<&'static Unknown>>,
    pub update: Box<dyn Fn(Option<&'static ReactNodeList>)>,
    pub unmount: Box<dyn Fn()>,
    pub get_instance: Box<dyn Fn() -> Option<&'static Unknown>>,
    #[cfg_attr(rust_js, rust_js::name = "unstable_flushSync")]
    pub unstable_flush_sync: Box<FlushSync>,
}

unsafe impl js::Defined for TestRenderer {}

fn is_hidden(child: &Child) -> bool {
    match child {
        Child::Instance(instance) => instance.is_hidden.get(),
        Child::Text(text_instance) => text_instance.is_hidden.get(),
    }
}

pub fn create(
    element: Option<&'static ReactNodeList>,
    options: Option<&'static TestRendererOptions>,
) -> &'static TestRenderer {
    if __DEV__
        && ENABLE_REACT_TEST_RENDERER_WARNING
        && !js::strict_equals(IS_REACT_NATIVE_TEST_ENVIRONMENT, true)
    {
        console::error((
            "react-test-renderer is deprecated. See https://react.dev/warnings/react-test-renderer",
        ));
    }

    let mut create_node_mock: &'static dyn Fn(&NodeMockElement) -> Option<&'static Unknown> =
        &DEFAULT_TEST_OPTIONS.create_node_mock;
    let is_concurrent_only =
        DISABLE_LEGACY_MODE && !js::strict_equals(IS_REACT_NATIVE_TEST_ENVIRONMENT, true);
    let mut is_concurrent = Some(is_concurrent_only);
    let mut is_strict_mode = false;
    if js::type_of(options.map(js::unknown)) == "object"
        && let Some(options) = options
    {
        if let Kind::Function(_) = js::classify(options.create_node_mock) {
            create_node_mock = unsafe { js::cast(options.create_node_mock) };
        }
        if !is_concurrent_only {
            is_concurrent = options.unstable_is_concurrent;
        }
        if js::strict_equals(options.unstable_strict_mode, true) {
            is_strict_mode = true;
        }
    }
    let container = Rc::new(Container {
        children: Rc::default(),
        create_node_mock,
        tag: ContainerTag::Container,
    });
    let root = ReactFiberReconciler.create_container(
        js::unknown_of(Rc::clone(&container)),
        if is_concurrent.unwrap_or(false) {
            CONCURRENT_ROOT
        } else {
            LEGACY_ROOT
        },
        None,
        is_strict_mode,
        Some(false),
        String::new(),
        Rc::new(default_on_uncaught_error),
        Rc::new(default_on_caught_error),
        Rc::new(default_on_recoverable_error),
        &default_on_default_transition_indicator,
        None,
    );

    ReactFiberReconciler.update_container(element, &root, None, None);
    let container = Rc::new(RefCell::new(Some(container)));
    let root = Rc::new(RefCell::new(Some(root)));

    let entry = js::unknown_of(TestRenderer {
        scheduler: SCHEDULER,

        root: None, // makes flow happy
        // we define a 'getter' for 'root' below using 'Object.defineProperty'
        to_json: Box::new({
            let root = Rc::clone(&root);
            let container = Rc::clone(&container);
            move || {
                let (Some(_), Some(container)) =
                    (root.borrow().clone(), container.borrow().clone())
                else {
                    return js::null();
                };
                let children = container.children.borrow().clone();
                if children.is_empty() {
                    return js::null();
                }
                if children.len() == 1 {
                    return to_json(&children[0]);
                }
                if children.len() == 2 && is_hidden(&children[0]) && !is_hidden(&children[1]) {
                    // Omit timed out children from output entirely, including the fact that we
                    // temporarily wrap fallback and timed out children in an array.
                    return to_json(&children[1]);
                }
                let mut rendered_children: Option<Vec<&'static Unknown>> = None;
                for child in &children {
                    if let Some(rendered_child) = to_json(child) {
                        match &mut rendered_children {
                            None => rendered_children = Some(vec![rendered_child]),
                            Some(rendered_children) => rendered_children.push(rendered_child),
                        }
                    }
                }
                match rendered_children {
                    Some(rendered_children) => Some(js::unknown_of(rendered_children)),
                    None => js::null(),
                }
            }
        }),
        to_tree: Box::new({
            let root = Rc::clone(&root);
            move || {
                let Some(root) = root.borrow().clone() else {
                    return js::null();
                };
                let current = root.current.borrow().clone();
                to_tree(Some(current))
            }
        }),
        update: Box::new({
            let root = Rc::clone(&root);
            move |new_element| {
                let Some(root) = root.borrow().clone() else {
                    return;
                };
                ReactFiberReconciler.update_container(new_element, &root, None, None);
            }
        }),
        unmount: Box::new({
            let root = Rc::clone(&root);
            let container = Rc::clone(&container);
            move || {
                let Some(current) = root.borrow().clone() else {
                    return;
                };
                ReactFiberReconciler.update_container(js::null(), &current, None, None);
                container.replace(js::null());
                root.replace(js::null());
            }
        }),
        get_instance: Box::new({
            let root = Rc::clone(&root);
            move || {
                let Some(root) = root.borrow().clone() else {
                    return js::null();
                };
                ReactFiberReconciler.get_public_root_instance(&root)
            }
        }),

        unstable_flush_sync: Box::new(|r#fn| {
            ReactFiberReconciler
                .flush_sync_from_reconciler(r#fn)
                .flatten()
        }),
    });

    object::define_property(
        entry,
        "root",
        &PropertyDescriptor {
            configurable: Some(true),
            enumerable: Some(true),
            get: Some(&move || {
                let Some(fiber_root) = root.borrow().clone() else {
                    panic!("Can't access .root on unmounted test renderer");
                };
                let current = fiber_root.current.borrow().clone();
                let children = get_children(&current);
                if children.is_empty() {
                    panic!("Can't access .root on unmounted test renderer");
                } else if children.len() == 1 {
                    // Normally, we skip the root and just give you the child.
                    Some(js::unknown_of(children[0].clone()))
                } else {
                    // However, we give you the root if there's more than one root child.
                    // We could make this the behavior for all cases but it would be a breaking change.
                    Some(js::unknown_of(wrap_fiber(&current)))
                }
            }),
            ..Default::default()
        },
    );

    unsafe { js::cast(Some(entry)) }
}

thread_local! {
    static FIBER_TO_WRAPPER: &'static WeakMap<Fiber, Rc<ReactTestInstance>> =
        js::weak_map::new();
}

fn wrap_fiber(fiber: &Rc<Fiber>) -> Rc<ReactTestInstance> {
    FIBER_TO_WRAPPER.with(|fiber_to_wrapper| {
        let mut wrapper = fiber_to_wrapper.get(fiber);
        if wrapper.is_none()
            && let Some(alternate) = &*fiber.alternate.borrow()
        {
            wrapper = fiber_to_wrapper.get(alternate);
        }
        match wrapper {
            Some(wrapper) => wrapper,
            None => {
                let wrapper = Rc::new(ReactTestInstance::new(Rc::clone(fiber)));
                fiber_to_wrapper.set(fiber, Rc::clone(&wrapper));
                wrapper
            }
        }
    })
}

js::on_load! {
    // Enable ReactTestRenderer to be used to test DevTools integration.
    ReactFiberReconciler.inject_into_dev_tools();
}

pub use self::SCHEDULER as _Scheduler;
#[cfg_attr(rust_js, rust_js::name = "unstable_batchedUpdates")]
pub fn unstable_batched_updates<A, R>(r#fn: impl FnOnce(A) -> R, a: A) -> R {
    ReactFiberReconciler.batched_updates(r#fn, a)
}
#[cfg_attr(rust_js, rust_js::name = "version")]
pub use shared::react_version::REACT_VERSION as VERSION;
