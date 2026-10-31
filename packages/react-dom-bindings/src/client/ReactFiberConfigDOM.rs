// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use js::{Class, Dict, ErrorOptions, JsError, JsObject, Kind, TimeoutId, Unknown, dict, function};
use react_isomorphic::react_transition_type::TransitionTypes;
use react_reconciler::react_current_fiber::run_with_fiber_in_dev;
use react_reconciler::react_fiber_tree_reflection::{
    fiber_is_portaled_into_host, get_fragment_instance_or_text_instance_siblings,
    get_fragment_parent_instance_or_container_fiber, get_fragment_portal_container_info,
    get_fragment_root_container_info, get_instance_from_host_fiber, is_fiber_contained_by_fragment,
    is_fiber_following, is_fiber_preceding, is_fragment_contained_by_fiber,
    traverse_fragment_instances_and_text_instances,
    traverse_fragment_instances_and_text_instances_deeply,
};
use react_reconciler::react_internal_types::Fiber;
use react_reconciler::react_test_selectors::BoundingRect;
use react_reconciler::react_work_tags::{
    HOST_COMPONENT, HOST_HOISTABLE, HOST_SINGLETON, HOST_TEXT,
};
use shared::check_string_coercion::check_attribute_string_coercion;
use shared::environment::__DEV__;
use shared::has_own_property::HAS_OWN_PROPERTY;
use shared::react_feature_flags::{
    DISABLE_COMMENTS_AS_DOM_CONTAINERS, ENABLE_CREATE_EVENT_HANDLE_API,
    ENABLE_HYDRATION_CHANGE_EVENT, ENABLE_MOVE_BEFORE, ENABLE_PROFILER_TIMER, ENABLE_SCOPE_API,
    ENABLE_SRC_OBJECT,
};
use webapi::global;
use webapi::global::btoa;
use webapi::window_global::get_computed_style;
use webapi::{
    AbortSignal, AddEventListenerOptions, AddEventListenerOptionsOrBool, Animation,
    AnimationTimeline, CSSNumberish, CSSStyleDeclaration, CSSStyleProperties, Comment, DOMRect,
    DOMRectList, Document, Element, ElementCreationOptions, Event,
    EventListenerOrEventListenerObject, EventTargetExt, FocusOptions, GetAnimationsOptions,
    GetRootNodeOptions, HTMLElement, IntersectionObserver, IntoAddEventListenerOptionsOrBool,
    IntoBoolOrScrollIntoViewOptions, IntoEventListenerOptionsOrBool, KeyframeAnimationOptions,
    Node, NumberOrCSSNumericValueOrStr, ResizeObserver, StartViewTransitionOptions, Text,
    TimelineRangeOffsetOrCSSNumericValueOrCSSKeywordValueOrStr, ViewTransition, Window, console,
    css_unit_value, document, html_element, html_image_element, html_link_element,
    html_select_element, keyframe_effect, navigator, node, window, window_global,
};

use crate::estimate_bandwidth::estimate_bandwidth;
use crate::react_dom_component_tree::is_owned_instance;
use crate::react_dom_form_actions::{FormStatus, NOT_PENDING};
use crate::react_dom_resource_validation::validate_link_props_for_style_resource;
use js::Promise;
use react_isomorphic::react_context::ReactContext;
use react_reconciler::react_test_selectors::{Intersection, ObserveVisibleRectsCallback};
use shared::noop::noop;
use shared::react_feature_flags::{ENABLE_SUSPENSEY_IMAGES, ENABLE_VIEW_TRANSITION};
use shared::react_symbols::REACT_CONTEXT_TYPE;
use webapi::window_global::match_media;
use webapi::{
    HTMLImageElement, IntersectionObserverEntry, IntersectionObserverInit, intersection_observer,
};

pub use crate::react_dom_update_priority::{
    get_current_update_priority, resolve_update_priority, set_current_update_priority,
};

use crate::dom_accessibility_roles::has_role;
use crate::dom_namespaces::{MATH_NAMESPACE, SVG_NAMESPACE};
use crate::dom_plugin_event_system::listen_to_all_supported_events;
use crate::html_node_type::{
    COMMENT_NODE, DOCUMENT_FRAGMENT_NODE, DOCUMENT_NODE, DOCUMENT_TYPE_NODE, ELEMENT_NODE,
    TEXT_NODE,
};
use crate::react_dom_component::{
    clear_click_listener, clear_singleton_properties, diff_hydrated_properties, diff_hydrated_text,
    get_props_from_element, hydrate_properties, hydrate_text, set_initial_properties,
    trap_click_on_non_interactive_element, update_properties,
};
#[cfg_attr(rust_js, rust_js::name = "getInstanceFromNodeDOMTree")]
use crate::react_dom_component_tree::get_instance_from_node as get_instance_from_node_dom_tree;
use crate::react_dom_component_tree::{
    get_closest_instance_from_node, get_fiber_current_props_from_node,
    get_fiber_from_scope_instance, is_container_marked_as_root, is_marked_hoistable,
    mark_node_as_hoistable, precache_fiber_node, update_fiber_props,
};
#[cfg_attr(rust_js, rust_js::name = "ReactBrowserEventEmitterIsEnabled")]
use crate::react_dom_event_listener::is_enabled as react_browser_event_emitter_is_enabled;
#[cfg_attr(rust_js, rust_js::name = "ReactBrowserEventEmitterSetEnabled")]
use crate::react_dom_event_listener::set_enabled as react_browser_event_emitter_set_enabled;
use crate::react_dom_event_replaying::{flush_event_replaying, retry_if_blocked_on};
use crate::react_dom_input::hydrate_input;
use crate::react_dom_select::hydrate_select;
use crate::react_dom_src_object::set_src_object;
use crate::react_dom_textarea::hydrate_textarea;
use crate::react_input_selection::{
    SelectionInformation, get_selection_information, restore_selection,
};
use crate::set_text_content::set_text_content;
use crate::validate_dom_nesting::{
    AncestorInfoDev, updated_ancestor_info_dev, validate_dom_nesting, validate_text_nesting,
};

pub use crate::react_dom_component_tree::detach_deleted_instance;

#[cfg_attr(rust_js, rust_js::name = "rendererVersion")]
pub use shared::react_version::REACT_VERSION as RENDERER_VERSION;

#[cfg_attr(rust_js, rust_js::name = "rendererPackageName")]
pub const RENDERER_PACKAGE_NAME: &str = "react-dom";
#[cfg_attr(rust_js, rust_js::name = "extraDevToolsConfig", rust_js::nullable)]
pub const EXTRA_DEV_TOOLS_CONFIG: Option<&Unknown> = None;

/// The props of a host component, as React DOM reads them. Each is what a
/// program gave, so one upstream types `string` but tests by `typeof` is
/// any value here.
pub struct Props {
    pub auto_focus: Option<&'static Unknown>,
    pub children: Option<&'static Unknown>,
    #[cfg_attr(rust_js, rust_js::name = "dangerouslySetInnerHTML")]
    pub dangerously_set_inner_html: Option<&'static Unknown>,
    pub is: Option<&'static Unknown>,
    pub multiple: Option<&'static Unknown>,
    pub size: Option<&'static Unknown>,
    pub r#type: Option<&'static Unknown>,
    pub value: Option<&'static Unknown>,
    pub default_value: Option<&'static Unknown>,
    pub checked: Option<&'static Unknown>,
    pub default_checked: Option<&'static Unknown>,
    pub src: Option<&'static Unknown>,
    pub src_set: Option<&'static Unknown>,
    pub loading: Option<&'static Unknown>,
    pub on_load: Option<&'static Unknown>,
}

unsafe impl js::Defined for Props {}

/// The `style` prop, as React DOM reads its members here: each is what a
/// program gave.
pub struct StyleProp {
    pub display: Option<&'static Unknown>,
    pub view_transition_name: Option<&'static Unknown>,
    #[cfg_attr(rust_js, rust_js::name = "view-transition-name")]
    pub view_transition_name_dashed: Option<&'static Unknown>,
    pub view_transition_class: Option<&'static Unknown>,
    #[cfg_attr(rust_js, rust_js::name = "view-transition-class")]
    pub view_transition_class_dashed: Option<&'static Unknown>,
    pub margin: Option<&'static Unknown>,
    pub margin_top: Option<&'static Unknown>,
    #[cfg_attr(rust_js, rust_js::name = "margin-top")]
    pub margin_top_dashed: Option<&'static Unknown>,
    pub margin_bottom: Option<&'static Unknown>,
    #[cfg_attr(rust_js, rust_js::name = "margin-bottom")]
    pub margin_bottom_dashed: Option<&'static Unknown>,
}

unsafe impl js::Defined for StyleProp {}

/// What a root renders into: an element, a document or a fragment.
pub type Container = Node;
pub type Instance = Element;
pub type TextInstance = Text;

pub type ActivityInstance = Comment;
/// A dehydrated Suspense boundary's start comment, which a retry is held on
/// as `_reactRetry`.
pub type SuspenseInstance = Comment;

/// A host context in DEV: the namespace, and the ancestors validateDOMNesting
/// checks a child against. In production a host context is its namespace.
pub struct HostContextDev {
    pub context: HostContextNamespace,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub ancestor_info: Option<AncestorInfoDev>,
}

unsafe impl js::Defined for HostContextDev {}

/// `HostContextDev` in DEV, a `HostContextNamespace` in production.
pub type HostContext = Unknown;

const SUPPRESS_HYDRATION_WARNING: &str = "suppressHydrationWarning";

const ACTIVITY_START_DATA: &str = "&";
const ACTIVITY_END_DATA: &str = "/&";
const SUSPENSE_START_DATA: &str = "$";
const SUSPENSE_END_DATA: &str = "/$";
const SUSPENSE_PENDING_START_DATA: &str = "$?";
const SUSPENSE_QUEUED_START_DATA: &str = "$~";
const SUSPENSE_FALLBACK_START_DATA: &str = "$!";
const PREAMBLE_CONTRIBUTION_HTML: &str = "html";
const PREAMBLE_CONTRIBUTION_BODY: &str = "body";
const PREAMBLE_CONTRIBUTION_HEAD: &str = "head";
const FORM_STATE_IS_MATCHING: &str = "F!";
const FORM_STATE_IS_NOT_MATCHING: &str = "F";

const DOCUMENT_READY_STATE_LOADING: &str = "loading";

const STYLE: &str = "style";

pub type HostContextNamespace = u8;
#[cfg_attr(rust_js, rust_js::name = "HostContextNamespaceNone")]
pub const HOST_CONTEXT_NAMESPACE_NONE: HostContextNamespace = 0;
#[cfg_attr(rust_js, rust_js::name = "HostContextNamespaceSvg")]
const HOST_CONTEXT_NAMESPACE_SVG: HostContextNamespace = 1;
#[cfg_attr(rust_js, rust_js::name = "HostContextNamespaceMath")]
const HOST_CONTEXT_NAMESPACE_MATH: HostContextNamespace = 2;

pub use react_reconciler::react_fiber_config_with_no_persistence::*;

thread_local! {
    #[cfg_attr(rust_js, rust_js::nullable)]
    static EVENTS_ENABLED: Cell<Option<bool>> = const { Cell::new(None) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static SELECTION_INFORMATION: RefCell<Option<Rc<SelectionInformation>>> = const { RefCell::new(None) };
}

fn get_owner_document_from_root_container(root_container_element: &Container) -> &'static Document {
    if root_container_element.node_type() == DOCUMENT_NODE {
        webapi::document::unchecked_from(root_container_element)
    } else {
        root_container_element.owner_document().unwrap()
    }
}

pub fn get_root_host_context(root_container_instance: &'static Container) -> &'static HostContext {
    let r#type;
    let context;
    let node_type = root_container_instance.node_type();
    match node_type {
        DOCUMENT_NODE | DOCUMENT_FRAGMENT_NODE => {
            r#type = if node_type == DOCUMENT_NODE {
                "#document".to_owned()
            } else {
                "#fragment".to_owned()
            };
            let root: Option<&Element> =
                unsafe { js::cast(js::get(root_container_instance, "documentElement")) };
            context = if let Some(root) = root {
                let namespace_uri = root.namespace_uri();
                match namespace_uri {
                    Some(namespace_uri) if !namespace_uri.is_empty() => {
                        get_own_host_context(&namespace_uri)
                    }
                    _ => HOST_CONTEXT_NAMESPACE_NONE,
                }
            } else {
                HOST_CONTEXT_NAMESPACE_NONE
            };
        }
        _ => {
            let container = if !DISABLE_COMMENTS_AS_DOM_CONTAINERS && node_type == COMMENT_NODE {
                root_container_instance.parent_node().unwrap()
            } else {
                root_container_instance
            };
            let container = webapi::element::unchecked_from(container);
            r#type = container.tag_name();
            let namespace_uri = container.namespace_uri();
            match namespace_uri {
                Some(namespace_uri) if !namespace_uri.is_empty() => {
                    let own_context = get_own_host_context(&namespace_uri);
                    context = get_child_host_context_prod(own_context, &r#type);
                }
                _ => {
                    context = match r#type.as_str() {
                        "svg" => HOST_CONTEXT_NAMESPACE_SVG,
                        "math" => HOST_CONTEXT_NAMESPACE_MATH,
                        _ => HOST_CONTEXT_NAMESPACE_NONE,
                    };
                }
            }
        }
    }
    if __DEV__ {
        let validated_tag = r#type.to_lowercase();
        let ancestor_info = updated_ancestor_info_dev(None, &validated_tag);
        return js::unknown_of(HostContextDev {
            context,
            ancestor_info,
        });
    }
    js::unknown_of(context)
}

fn get_own_host_context(namespace_uri: &str) -> HostContextNamespace {
    match namespace_uri {
        SVG_NAMESPACE => HOST_CONTEXT_NAMESPACE_SVG,
        MATH_NAMESPACE => HOST_CONTEXT_NAMESPACE_MATH,
        _ => HOST_CONTEXT_NAMESPACE_NONE,
    }
}

fn get_child_host_context_prod(
    parent_namespace: HostContextNamespace,
    r#type: &str,
) -> HostContextNamespace {
    if parent_namespace == HOST_CONTEXT_NAMESPACE_NONE {
        // No (or default) parent namespace: potential entry point.
        return match r#type {
            "svg" => HOST_CONTEXT_NAMESPACE_SVG,
            "math" => HOST_CONTEXT_NAMESPACE_MATH,
            _ => HOST_CONTEXT_NAMESPACE_NONE,
        };
    }
    if parent_namespace == HOST_CONTEXT_NAMESPACE_SVG && r#type == "foreignObject" {
        // We're leaving SVG.
        return HOST_CONTEXT_NAMESPACE_NONE;
    }
    // By default, pass namespace below.
    parent_namespace
}

pub fn get_child_host_context(
    parent_host_context: &'static HostContext,
    r#type: &str,
) -> &'static HostContext {
    if __DEV__ {
        let parent_host_context_dev: &HostContextDev =
            unsafe { js::cast(Some(parent_host_context)) };
        let context = get_child_host_context_prod(parent_host_context_dev.context, r#type);
        let ancestor_info =
            updated_ancestor_info_dev(parent_host_context_dev.ancestor_info.as_ref(), r#type);
        return js::unknown_of(HostContextDev {
            context,
            ancestor_info,
        });
    }
    let parent_namespace: HostContextNamespace = unsafe { js::cast(Some(parent_host_context)) };
    js::unknown_of(get_child_host_context_prod(parent_namespace, r#type))
}

pub fn get_public_instance(instance: &'static Instance) -> &'static Instance {
    instance
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn prepare_for_commit(container_info: &'static Container) -> Option<Rc<Fiber>> {
    EVENTS_ENABLED.set(Some(react_browser_event_emitter_is_enabled()));
    let selection_information = Rc::new(get_selection_information(Some(container_info)));
    SELECTION_INFORMATION.set(Some(Rc::clone(&selection_information)));
    let mut active_instance = js::null();
    if ENABLE_CREATE_EVENT_HANDLE_API && let Some(focused_elem) = selection_information.focused_elem
    {
        active_instance = get_closest_instance_from_node(focused_elem);
    }
    react_browser_event_emitter_set_enabled(Some(false));
    active_instance
}

pub fn before_active_instance_blur(internal_instance_handle: &Rc<Fiber>) {
    if ENABLE_CREATE_EVENT_HANDLE_API {
        react_browser_event_emitter_set_enabled(Some(true));
        dispatch_before_detached_blur(
            SELECTION_INFORMATION
                .with_borrow(|selection_information| {
                    selection_information.as_ref().unwrap().focused_elem
                })
                .unwrap(),
            internal_instance_handle,
        );
        react_browser_event_emitter_set_enabled(Some(false));
    }
}

pub fn after_active_instance_blur() {
    if ENABLE_CREATE_EVENT_HANDLE_API {
        react_browser_event_emitter_set_enabled(Some(true));
        dispatch_after_detached_blur(
            SELECTION_INFORMATION
                .with_borrow(|selection_information| {
                    selection_information.as_ref().unwrap().focused_elem
                })
                .unwrap(),
        );
        react_browser_event_emitter_set_enabled(Some(false));
    }
}

pub fn reset_after_commit(container_info: &'static Container) {
    let selection_information = SELECTION_INFORMATION.with_borrow(Clone::clone);
    restore_selection(&selection_information.unwrap(), Some(container_info));
    react_browser_event_emitter_set_enabled(EVENTS_ENABLED.get());
    EVENTS_ENABLED.set(js::null());
    SELECTION_INFORMATION.set(js::null());
}

pub fn create_hoistable_instance(
    r#type: &str,
    props: &'static Props,
    root_container_instance: &'static Container,
    internal_instance_handle: &Rc<Fiber>,
) -> &'static Instance {
    let owner_document = get_owner_document_from_root_container(root_container_instance);

    let dom_element = owner_document.create_element_named(r#type, None);
    precache_fiber_node(internal_instance_handle, js::unknown(dom_element));
    update_fiber_props(dom_element, js::unknown(props));
    set_initial_properties(dom_element, r#type, js::unknown(props));
    mark_node_as_hoistable(dom_element);
    dom_element
}

thread_local! {
    static DID_WARN_SCRIPT_TAGS: Cell<bool> = const { Cell::new(false) };
}

fn is_script_data_block(props: &Props) -> bool {
    let script_type = props.r#type;
    let Kind::String(script_type) = js::classify(script_type) else {
        return false;
    };
    if script_type.is_empty() {
        return false;
    }
    let lower = script_type.to_lowercase();
    // Special non-MIME keywords recognized by the HTML spec
    // TODO: May be fine to also not warn about having these types be parsed as "parser-inserted"
    if lower == "module" || lower == "importmap" || lower == "speculationrules" {
        return false;
    }
    // JavaScript MIME types per https://mimesniff.spec.whatwg.org/#javascript-mime-type
    if let "application/ecmascript"
    | "application/javascript"
    | "application/x-ecmascript"
    | "application/x-javascript"
    | "text/ecmascript"
    | "text/javascript"
    | "text/javascript1.0"
    | "text/javascript1.1"
    | "text/javascript1.2"
    | "text/javascript1.3"
    | "text/javascript1.4"
    | "text/javascript1.5"
    | "text/jscript"
    | "text/livescript"
    | "text/x-ecmascript"
    | "text/x-javascript" = lower.as_str()
    {
        return false;
    }
    // Any other non-empty type value means this is a data block
    true
}

thread_local! {
    static WARNED_UNKNOWN_TAGS: &'static Dict<bool> = {
        let warned_unknown_tags = dict::new();
        // There are working polyfills for <dialog>. Let people use it.
        dict::set(warned_unknown_tags, "dialog", true);
        // Electron ships a custom <webview> tag to display external web content in
        // an isolated frame and process.
        // This tag is not present in non Electron environments such as JSDom which
        // is often used for testing purposes.
        // @see https://electronjs.org/docs/api/webview-tag
        dict::set(warned_unknown_tags, "webview", true);
        warned_unknown_tags
    };
}

pub fn create_instance(
    r#type: &str,
    props: &'static Props,
    root_container_instance: &'static Container,
    host_context: &'static HostContext,
    internal_instance_handle: &Rc<Fiber>,
) -> &'static Instance {
    let host_context_prod: HostContextNamespace = if __DEV__ {
        // TODO: take namespace into account when validating.
        let host_context_dev: &HostContextDev = unsafe { js::cast(Some(host_context)) };
        validate_dom_nesting(r#type, host_context_dev.ancestor_info.as_ref());
        host_context_dev.context
    } else {
        unsafe { js::cast(Some(host_context)) }
    };

    let owner_document = get_owner_document_from_root_container(root_container_instance);

    let dom_element: &Instance;
    match host_context_prod {
        HOST_CONTEXT_NAMESPACE_SVG => {
            dom_element = owner_document.create_element_ns_named(Some(SVG_NAMESPACE), r#type, None);
        }
        HOST_CONTEXT_NAMESPACE_MATH => {
            dom_element =
                owner_document.create_element_ns_named(Some(MATH_NAMESPACE), r#type, None);
        }
        _ => match r#type {
            "svg" => {
                dom_element =
                    owner_document.create_element_ns_named(Some(SVG_NAMESPACE), r#type, None);
            }
            "math" => {
                dom_element =
                    owner_document.create_element_ns_named(Some(MATH_NAMESPACE), r#type, None);
            }
            "script" => {
                // Create the script via .innerHTML so its "parser-inserted" flag is
                // set to true and it does not execute
                let div = owner_document.create_element_named("div", None);
                if __DEV__
                    && !DID_WARN_SCRIPT_TAGS.get()
                    // Data block scripts are not executed by UAs anyway so
                    // we don't need to warn: https://html.spec.whatwg.org/multipage/scripting.html#attr-script-type
                    && !is_script_data_block(props)
                {
                    console::error((concat!(
                        "Encountered a script tag while rendering React component. ",
                        "Scripts inside React components are never executed when rendering ",
                        "on the client. Consider using template tag instead ",
                        "(https://developer.mozilla.org/en-US/docs/Web/HTML/Element/template)."
                    ),));
                    DID_WARN_SCRIPT_TAGS.set(true);
                }
                div.set_inner_html(&("<script><".to_owned() + "/script>"));
                // This is guaranteed to yield a script element.
                let first_child = div.first_child().unwrap();
                dom_element = webapi::element::unchecked_from(div.remove_child(first_child));
            }
            "select" => {
                if let Kind::String(is) = js::classify(props.is) {
                    dom_element = owner_document.create_element_named(
                        "select",
                        Some(&ElementCreationOptions {
                            is: Some(is),
                            ..Default::default()
                        }),
                    );
                } else {
                    // Separate else branch instead of using `props.is || undefined` above because of a Firefox bug.
                    // See discussion in https://github.com/react/react/pull/6896
                    // and discussion in https://bugzilla.mozilla.org/show_bug.cgi?id=1276240
                    dom_element = owner_document.create_element_named("select", None);
                }
                if js::truthy(props.multiple) {
                    html_select_element::unchecked_from(dom_element).set_multiple(true);
                } else if js::truthy(props.size) {
                    // Setting a size greater than 1 causes a select to behave like `multiple=true`, where
                    // it is possible that no option is selected.
                    //
                    // This is only necessary when a select in "single selection mode".
                    html_select_element::unchecked_from(dom_element).set_size(props.size);
                }
            }
            _ => {
                if let Kind::String(is) = js::classify(props.is) {
                    dom_element = owner_document.create_element_named(
                        r#type,
                        Some(&ElementCreationOptions {
                            is: Some(is),
                            ..Default::default()
                        }),
                    );
                } else {
                    // Separate else branch instead of using `props.is || undefined` above because of a Firefox bug.
                    // See discussion in https://github.com/react/react/pull/6896
                    // and discussion in https://bugzilla.mozilla.org/show_bug.cgi?id=1276240
                    dom_element = owner_document.create_element_named(r#type, None);
                }

                // We're not SVG/MathML and we don't have a dash, so we're not a custom element
                // Even if you use `is`, these should be of known type and lower case.
                if __DEV__ && !r#type.contains('-') {
                    if r#type != r#type.to_lowercase() {
                        console::error((
                            concat!(
                                "<%s /> is using incorrect casing. ",
                                "Use PascalCase for React components, ",
                                "or lowercase for HTML elements."
                            ),
                            r#type,
                        ));
                    }
                    if js::object::to_string(Some(js::unknown(dom_element))).unwrap()
                        == "[object HTMLUnknownElement]"
                        && !WARNED_UNKNOWN_TAGS.with(|warned_unknown_tags| {
                            HAS_OWN_PROPERTY.with(|has_own_property| {
                                has_own_property.call(js::unknown(*warned_unknown_tags), r#type)
                            })
                        })
                    {
                        WARNED_UNKNOWN_TAGS.with(|warned_unknown_tags| {
                            dict::set(warned_unknown_tags, r#type, true)
                        });
                        console::error((
                            concat!(
                                "The tag <%s> is unrecognized in this browser. ",
                                "If you meant to render a React component, start its name with ",
                                "an uppercase letter."
                            ),
                            r#type,
                        ));
                    }
                }
            }
        },
    }
    precache_fiber_node(internal_instance_handle, js::unknown(dom_element));
    update_fiber_props(dom_element, js::unknown(props));
    dom_element
}

thread_local! {
    static DID_WARN_FOR_CLONE: Cell<bool> = const { Cell::new(false) };
}

pub fn clone_mutable_instance(instance: &Instance, keep_children: bool) -> &'static Instance {
    if __DEV__ {
        // Warn for problematic
        let tag_name = instance.tag_name();
        if let "VIDEO" | "IFRAME" = tag_name.as_str()
            && !DID_WARN_FOR_CLONE.get()
        {
            DID_WARN_FOR_CLONE.set(true);
            // TODO: Once we have the ability to avoid cloning the root, suggest an absolutely
            // positioned ViewTransition instead as the solution.
            console::warn((
                concat!(
                    "startGestureTransition() required cloning a <%s> element since it exists in ",
                    "both states of the gesture. This can be problematic since it will load it twice ",
                    "Try removing or hiding it with <Activity mode=\"offscreen\"> in the optimistic state."
                ),
                tag_name.to_lowercase(),
            ));
        }
    }
    webapi::element::unchecked_from(instance.clone_node(Some(keep_children)))
}

pub fn append_initial_child(parent_instance: &Instance, child: &Node) {
    // Note: This should not use moveBefore() because initial are appended while disconnected.
    parent_instance.append_child(child);
}

pub fn finalize_initial_children(
    dom_element: &'static Instance,
    r#type: &str,
    props: &'static Props,
    _host_context: &HostContext,
) -> bool {
    set_initial_properties(dom_element, r#type, js::unknown(props));
    match r#type {
        "button" | "input" | "select" | "textarea" => js::truthy(props.auto_focus),
        "img" => true,
        _ => false,
    }
}

pub fn finalize_hydrated_children(
    _dom_element: &Instance,
    r#type: &str,
    _props: &Props,
    _host_context: &HostContext,
) -> bool {
    // TOOD: Consider unifying this with hydrateInstance.
    if !ENABLE_HYDRATION_CHANGE_EVENT
        .with(|enable_hydration_change_event| *enable_hydration_change_event)
    {
        return false;
    }
    matches!(r#type, "input" | "select" | "textarea" | "img")
}

pub fn should_set_text_content(r#type: &str, props: &Props) -> bool {
    r#type == "textarea"
        || r#type == "noscript"
        || matches!(
            js::classify(props.children),
            Kind::String(_) | Kind::Number(_) | Kind::BigInt(_)
        )
        || (matches!(
            js::classify(props.dangerously_set_inner_html),
            Kind::Object(_) | Kind::Array(_)
        ) && js::get(props.dangerously_set_inner_html, "__html").is_some())
}

pub fn create_text_instance(
    text: &str,
    root_container_instance: &'static Container,
    host_context: &'static HostContext,
    internal_instance_handle: &Rc<Fiber>,
) -> &'static TextInstance {
    if __DEV__ {
        let host_context_dev: &HostContextDev = unsafe { js::cast(Some(host_context)) };
        let ancestor_info = host_context_dev.ancestor_info.as_ref().unwrap();
        if let Some(ancestor) = &ancestor_info.current {
            validate_text_nesting(text, &ancestor.tag, ancestor_info.implicit_root_scope);
        }
    }
    let text_node =
        get_owner_document_from_root_container(root_container_instance).create_text_node(text);
    precache_fiber_node(internal_instance_handle, js::unknown(text_node));
    text_node
}

pub fn clone_mutable_text_instance(text_instance: &TextInstance) -> &'static TextInstance {
    webapi::text::unchecked_from(text_instance.clone_node(Some(false)))
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::nullable)]
    static CURRENT_POPSTATE_TRANSITION_EVENT: Cell<Option<&'static Event>> = const { Cell::new(None) };
}

pub fn should_attempt_eager_transition() -> bool {
    let event = window.event();
    if let Some(event) = event
        && event.r#type() == "popstate"
    {
        // This is a popstate event. Attempt to render any transition during this
        // event synchronously. Unless we already attempted during this event.
        if Some(event) == CURRENT_POPSTATE_TRANSITION_EVENT.get() {
            // We already attempted to render this popstate transition synchronously.
            // Any subsequent attempts must have happened as the result of a derived
            // update, like startTransition inside useEffect, or useDV. Switch back to
            // the default behavior for all remaining transitions during the current
            // popstate event.
            return false;
        } else {
            // Cache the current event in case a derived transition is scheduled.
            // (Refer to previous branch.)
            CURRENT_POPSTATE_TRANSITION_EVENT.set(Some(event));
            return true;
        }
    }
    // We're not inside a popstate event.
    CURRENT_POPSTATE_TRANSITION_EVENT.set(js::null());
    false
}

thread_local! {
    static SCHEDULER_EVENT: Cell<Option<&'static Event>> = const { Cell::new(None) };
}

pub fn track_scheduler_event() {
    SCHEDULER_EVENT.set(window.event());
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn resolve_event_type() -> Option<String> {
    let event = window.event();
    match event {
        Some(event) if Some(event) != SCHEDULER_EVENT.get() => Some(event.r#type()),
        _ => None,
    }
}

pub fn resolve_event_time_stamp() -> f64 {
    let event = window.event();
    match event {
        Some(event) if Some(event) != SCHEDULER_EVENT.get() => event.time_stamp(),
        _ => -1.1,
    }
}

#[cfg_attr(rust_js, rust_js::name = "isPrimaryRenderer")]
pub const IS_PRIMARY_RENDERER: bool = true;
#[cfg_attr(rust_js, rust_js::name = "warnsIfNotActing")]
pub const WARNS_IF_NOT_ACTING: bool = true;
pub type TimeoutHandle = TimeoutId;
type ScheduleTimeout = fn(Box<dyn FnOnce()>, Option<f64>) -> &'static TimeoutHandle;
type CancelTimeout = fn(Option<&TimeoutHandle>);

// This initialization code may run even on server environments
// if a component just imports ReactDOM (e.g. for findDOMNode).
// Some environments might not have setTimeout or clearTimeout.
thread_local! {
    pub static SCHEDULE_TIMEOUT: Option<ScheduleTimeout> =
        if js::is_function(js::set_timeout::<Box<dyn FnOnce()>>) { Some(js::set_timeout) } else { None };
    pub static CANCEL_TIMEOUT: Option<CancelTimeout> =
        if js::is_function(js::clear_timeout) { Some(js::clear_timeout) } else { None };
}
#[cfg_attr(rust_js, rust_js::name = "noTimeout")]
pub const NO_TIMEOUT: i32 = -1;
type RequestAnimationFrame = fn(Box<dyn FnOnce(Option<f64>)>);
thread_local! {
    static LOCAL_PROMISE: Option<&'static Class> =
        if js::is_function(js::class::<Promise<()>>) { Some(js::class::<Promise<()>>()) } else { None };
    static LOCAL_REQUEST_ANIMATION_FRAME: Option<RequestAnimationFrame> =
        if js::is_function(window_global::request_animation_frame) {
            Some(|callback| {
                window_global::request_animation_frame(Box::new(move |time| callback(Some(time))));
            })
        } else if SCHEDULE_TIMEOUT.with(Option::is_some) {
            Some(|callback| {
                SCHEDULE_TIMEOUT.with(|schedule_timeout| schedule_timeout.unwrap())(Box::new(move || callback(None)), None);
            })
        } else {
            None
        };
}

pub use crate::react_dom_component_tree::get_closest_instance_from_node as get_instance_from_node;

pub fn prepare_portal_mount(portal_instance: &'static Instance) {
    listen_to_all_supported_events(portal_instance);
}

pub fn prepare_scope_update(scope_instance: &Unknown, internal_instance_handle: &Rc<Fiber>) {
    if ENABLE_SCOPE_API {
        precache_fiber_node(internal_instance_handle, scope_instance);
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_instance_from_scope(scope_instance: &Unknown) -> Option<Rc<Fiber>> {
    if ENABLE_SCOPE_API {
        return get_fiber_from_scope_instance(scope_instance);
    }
    js::null()
}

// -------------------
//     Microtasks
// -------------------
#[cfg_attr(rust_js, rust_js::name = "supportsMicrotasks")]
pub const SUPPORTS_MICROTASKS: bool = true;
type ScheduleMicrotask = fn(Box<dyn FnOnce()>);
thread_local! {
    pub static SCHEDULE_MICROTASK: ScheduleMicrotask = if js::is_function(global::queue_microtask) {
        global::queue_microtask
    } else if LOCAL_PROMISE.with(|local_promise| local_promise.is_some()) {
        |callback| {
            LOCAL_PROMISE.with(|local_promise| {
                local_promise
                    .unwrap()
                    .call_static(js::promise::resolve, (js::null::<&Unknown>(),))
                    .then::<(), _>(move |_| callback())
                    .catch(handle_error_in_next_tick);
            });
        }
    } else {
        |callback| {
            js::set_timeout(callback, None);
        }
    }; // TODO: Determine the best fallback here.
}

fn handle_error_in_next_tick(error: &'static JsError) {
    js::set_timeout(move || js::throw(error.as_unknown()), None);
}

// -------------------
//     Mutation
// -------------------

#[cfg_attr(rust_js, rust_js::name = "supportsMutation")]
pub const SUPPORTS_MUTATION: bool = true;

pub fn commit_mount(
    dom_element: &'static Instance,
    r#type: &str,
    new_props: &'static Props,
    _internal_instance_handle: &Rc<Fiber>,
) {
    // Despite the naming that might imply otherwise, this method only
    // fires if there is an `Update` effect scheduled during mounting.
    // This happens if `finalizeInitialChildren` returns `true` (which it
    // does to implement the `autoFocus` attribute on the client). But
    // there are also other cases when this might happen (such as patching
    // up text content during hydration mismatch). So we'll check this again.
    match r#type {
        "button" | "input" | "select" | "textarea" => {
            if js::truthy(new_props.auto_focus) {
                html_element::unchecked_from(dom_element).focus(None);
            }
        }
        "img" => {
            // The technique here is to assign the src or srcSet property to cause the browser
            // to issue a new load event. If it hasn't loaded yet it'll fire whenever the load actually completes.
            // If it has already loaded we missed it so the second load will still be the first one that executes
            // any associated onLoad props.
            // Even if we have srcSet we prefer to reassign src. The reason is that Firefox does not trigger a new
            // load event when only srcSet is assigned. Chrome will trigger a load event if either is assigned so we
            // only need to assign one. And Safari just never triggers a new load event which means this technique
            // is already a noop regardless of which properties are assigned. We should revisit if browsers update
            // this heuristic in the future.
            if js::truthy(new_props.src) {
                let src = new_props.src.unwrap();
                if ENABLE_SRC_OBJECT.with(|enable_src_object| *enable_src_object)
                    && matches!(js::classify(Some(src)), Kind::Object(_) | Kind::Array(_))
                {
                    // For object src, we can't just set the src again to the same blob URL because it might have
                    // already revoked if it loaded before this. However, we can create a new blob URL and set that.
                    // This is relatively cheap since the blob is already in memory but this might cause some
                    // duplicated work.
                    // TODO: We could maybe detect if load hasn't fired yet and if so reuse the URL.
                    match catch_unwind(AssertUnwindSafe(|| {
                        set_src_object(dom_element, r#type, src)
                    })) {
                        Ok(()) => return,
                        Err(_) => {
                            // If URL.createObjectURL() errors, it was probably some other object type
                            // that should be toString:ed instead, so we just fall-through to the normal
                            // path.
                        }
                    }
                }
                html_image_element::unchecked_from(dom_element).set_src(src);
            } else if js::truthy(new_props.src_set) {
                html_image_element::unchecked_from(dom_element).set_srcset(new_props.src_set);
            }
        }
        _ => {}
    }
}

pub fn commit_hydrated_instance(
    dom_element: &'static Instance,
    r#type: &str,
    props: &'static Props,
    _internal_instance_handle: &Rc<Fiber>,
) {
    if !ENABLE_HYDRATION_CHANGE_EVENT
        .with(|enable_hydration_change_event| *enable_hydration_change_event)
    {
        return;
    }
    // This fires in the commit phase if a hydrated instance needs to do further
    // work in the commit phase. Similar to commitMount. However, this should not
    // do things that would've already happened such as set auto focus since that
    // would steal focus. It's only scheduled if finalizeHydratedChildren returns
    // true.
    match r#type {
        "input" => {
            hydrate_input(
                dom_element,
                props.value,
                props.default_value,
                props.checked,
                props.default_checked,
            );
        }
        "select" => {
            hydrate_select(
                dom_element,
                props.value,
                props.default_value,
                props.multiple,
            );
        }
        "textarea" => hydrate_textarea(dom_element, props.value, props.default_value),
        "img" => {
            // TODO: Should we replay onLoad events?
        }
        _ => {}
    }
}

pub fn commit_update(
    dom_element: &'static Instance,
    r#type: &str,
    old_props: &'static Props,
    new_props: &'static Props,
    _internal_instance_handle: &Rc<Fiber>,
) {
    // Diff and update the properties.
    update_properties(
        dom_element,
        r#type,
        js::unknown(old_props),
        js::unknown(new_props),
    );

    // Update the props handle so that we know which props are the ones with
    // with current event handlers.
    update_fiber_props(dom_element, js::unknown(new_props));
}

pub fn reset_text_content(dom_element: &Instance) {
    set_text_content(dom_element, "");
}

pub fn commit_text_update(text_instance: &TextInstance, _old_text: &str, new_text: &str) {
    text_instance.set_node_value(Some(new_text));
}

thread_local! {
    // $FlowFixMe[prop-missing]: We're doing the feature detection here.
    #[cfg_attr(rust_js, rust_js::name = "supportsMoveBefore")]
    static SUPPORTS_MOVE_BEFORE: bool =
        ENABLE_MOVE_BEFORE
            && js::is_defined(window)
            && matches!(
                js::classify(js::get(js::get(js::get(window, "Element"), "prototype"), "moveBefore")),
                Kind::Function(_)
            );
}

pub fn append_child(parent_instance: &Instance, child: &Node) {
    if SUPPORTS_MOVE_BEFORE.with(|supports_move_before| *supports_move_before)
        && child.parent_node().is_some()
    {
        // $FlowFixMe[prop-missing]: We've checked this with supportsMoveBefore.
        parent_instance.move_before(child, None);
    } else {
        parent_instance.append_child(child);
    }
}

fn warn_for_react_children_conflict(container: &'static Container) {
    if __DEV__ {
        if js::truthy(js::get(container, "__reactWarnedAboutChildrenConflict")) {
            return;
        }
        let props = get_fiber_current_props_from_node(js::unknown(container));
        if let Some(props) = props {
            let props: &Props = unsafe { js::cast(Some(props)) };
            let fiber = get_instance_from_node_dom_tree(container);
            if fiber.is_some() {
                if matches!(
                    js::classify(props.children),
                    Kind::String(_) | Kind::Number(_)
                ) {
                    js::set(container, "__reactWarnedAboutChildrenConflict", true);
                    // Run the warning with the Fiber of the container for context of where the children are specified.
                    // We could also maybe use the Portal. The current execution context is the child being added.
                    run_with_fiber_in_dev(
                        fiber,
                        |_: (), _: (), _: (), _: (), _: ()| {
                            console::error((concat!(
                                "Cannot use a ref on a React element as a container to `createRoot` or `createPortal` ",
                                "if that element also sets \"children\" text content using React. It should be a leaf with no children. ",
                                "Otherwise it's ambiguous which children should be used."
                            ),));
                        },
                        (),
                        (),
                        (),
                        (),
                        (),
                    );
                } else if props.dangerously_set_inner_html.is_some() {
                    js::set(container, "__reactWarnedAboutChildrenConflict", true);
                    run_with_fiber_in_dev(
                        fiber,
                        |_: (), _: (), _: (), _: (), _: ()| {
                            console::error((concat!(
                                "Cannot use a ref on a React element as a container to `createRoot` or `createPortal` ",
                                "if that element also sets \"dangerouslySetInnerHTML\" using React. It should be a leaf with no children. ",
                                "Otherwise it's ambiguous which children should be used."
                            ),));
                        },
                        (),
                        (),
                        (),
                        (),
                        (),
                    );
                }
            }
        }
    }
}

pub fn append_child_to_container(container: &'static Container, child: &Node) {
    if __DEV__ {
        warn_for_react_children_conflict(container);
    }
    let parent_node: &Element = if container.node_type() == DOCUMENT_NODE {
        webapi::document::unchecked_from(container).body().unwrap()
    } else if !DISABLE_COMMENTS_AS_DOM_CONTAINERS && container.node_type() == COMMENT_NODE {
        let parent_node = webapi::element::unchecked_from(container.parent_node().unwrap());
        if SUPPORTS_MOVE_BEFORE.with(|supports_move_before| *supports_move_before)
            && child.parent_node().is_some()
        {
            // $FlowFixMe[prop-missing]: We've checked this with supportsMoveBefore.
            parent_node.move_before(child, Some(container));
        } else {
            parent_node.insert_before(child, Some(container));
        }
        return;
    } else if container.node_name() == "HTML" {
        container.owner_document().unwrap().body().unwrap()
    } else {
        webapi::element::unchecked_from(container)
    };
    if SUPPORTS_MOVE_BEFORE.with(|supports_move_before| *supports_move_before)
        && child.parent_node().is_some()
    {
        // $FlowFixMe[prop-missing]: We've checked this with supportsMoveBefore.
        parent_node.move_before(child, None);
    } else {
        parent_node.append_child(child);
    }
    // This container might be used for a portal.
    // If something inside a portal is clicked, that click should bubble
    // through the React tree. However, on Mobile Safari the click would
    // never bubble through the *DOM* tree unless an ancestor with onclick
    // event exists. So we wouldn't see it and dispatch it.
    // This is why we ensure that non React root containers have inline onclick
    // defined.
    // https://github.com/react/react/issues/11918
    let react_root_container = js::get(container, "_reactRootContainer");
    if react_root_container.is_none()
        && matches!(js::classify(js::get(parent_node, "onclick")), Kind::Null)
    {
        // TODO: This cast may not be sound for SVG, MathML or custom elements.
        trap_click_on_non_interactive_element(html_element::unchecked_from(parent_node));
    }
}

pub fn insert_before(parent_instance: &Instance, child: &Node, before_child: &Node) {
    if SUPPORTS_MOVE_BEFORE.with(|supports_move_before| *supports_move_before)
        && child.parent_node().is_some()
    {
        // $FlowFixMe[prop-missing]: We've checked this with supportsMoveBefore.
        parent_instance.move_before(child, Some(before_child));
    } else {
        parent_instance.insert_before(child, Some(before_child));
    }
}

pub fn insert_in_container_before(
    container: &'static Container,
    child: &Node,
    before_child: &Node,
) {
    if __DEV__ {
        warn_for_react_children_conflict(container);
    }
    let parent_node: &Element = if container.node_type() == DOCUMENT_NODE {
        webapi::document::unchecked_from(container).body().unwrap()
    } else if !DISABLE_COMMENTS_AS_DOM_CONTAINERS && container.node_type() == COMMENT_NODE {
        webapi::element::unchecked_from(container.parent_node().unwrap())
    } else if container.node_name() == "HTML" {
        container.owner_document().unwrap().body().unwrap()
    } else {
        webapi::element::unchecked_from(container)
    };
    if SUPPORTS_MOVE_BEFORE.with(|supports_move_before| *supports_move_before)
        && child.parent_node().is_some()
    {
        // $FlowFixMe[prop-missing]: We've checked this with supportsMoveBefore.
        parent_node.move_before(child, Some(before_child));
    } else {
        parent_node.insert_before(child, Some(before_child));
    }
}

pub fn is_singleton_scope(r#type: &str) -> bool {
    r#type == "head"
}

fn create_event(r#type: &str, bubbles: bool) -> &'static Event {
    let event = document.create_event("Event");
    event.init_event(r#type, Some(bubbles), Some(false));
    event
}

fn dispatch_before_detached_blur(target: &Element, internal_instance_handle: &Rc<Fiber>) {
    if ENABLE_CREATE_EVENT_HANDLE_API {
        let event = create_event("beforeblur", true);
        // Dispatch "beforeblur" directly on the target,
        // so it gets picked up by the event system and
        // can propagate through the React internal tree.
        // $FlowFixMe[prop-missing]: internal field
        js::set(
            event,
            "_detachedInterceptFiber",
            Rc::clone(internal_instance_handle),
        );
        target.dispatch_event(event);
    }
}

fn dispatch_after_detached_blur(target: &'static Element) {
    if ENABLE_CREATE_EVENT_HANDLE_API {
        let event = create_event("afterblur", false);
        // So we know what was detached, make the relatedTarget the
        // detached target on the "afterblur" event.
        js::set(event, "relatedTarget", target);
        // Dispatch the event on the document.
        document.dispatch_event(event);
    }
}

pub fn remove_child(parent_instance: &Instance, child: &Node) {
    parent_instance.remove_child(child);
}

pub fn remove_child_from_container(container: &Container, child: &Node) {
    let parent_node: &Element = if container.node_type() == DOCUMENT_NODE {
        webapi::document::unchecked_from(container).body().unwrap()
    } else if !DISABLE_COMMENTS_AS_DOM_CONTAINERS && container.node_type() == COMMENT_NODE {
        webapi::element::unchecked_from(container.parent_node().unwrap())
    } else if container.node_name() == "HTML" {
        container.owner_document().unwrap().body().unwrap()
    } else {
        webapi::element::unchecked_from(container)
    };
    parent_node.remove_child(child);
}

fn clear_hydration_boundary(parent_instance: &Instance, hydration_instance: &'static Comment) {
    let mut node: &Node = hydration_instance;
    // Delete all nodes within this suspense boundary.
    // There might be nested nodes so we need to keep track of how
    // deep we are and only break out when we're back on top.
    let mut depth = 0;
    loop {
        let next_node = node.next_sibling();
        parent_instance.remove_child(node);
        if let Some(next_node) = next_node
            && next_node.node_type() == COMMENT_NODE
        {
            let data = webapi::comment::unchecked_from(next_node).data();
            if data == SUSPENSE_END_DATA || data == ACTIVITY_END_DATA {
                if depth == 0 {
                    parent_instance.remove_child(next_node);
                    // Retry if any event replaying was blocked on this.
                    retry_if_blocked_on(js::unknown(hydration_instance));
                    return;
                } else {
                    depth -= 1;
                }
            } else if data == SUSPENSE_START_DATA
                || data == SUSPENSE_PENDING_START_DATA
                || data == SUSPENSE_QUEUED_START_DATA
                || data == SUSPENSE_FALLBACK_START_DATA
                || data == ACTIVITY_START_DATA
            {
                depth += 1;
            } else if data == PREAMBLE_CONTRIBUTION_HTML {
                // If a preamble contribution marker is found within the bounds of this boundary,
                // then it contributed to the html tag and we need to reset it.
                let owner_document = parent_instance.owner_document();
                let document_element: &Element = owner_document.document_element().unwrap();
                clear_singleton_preamble_contribution(document_element);
            } else if data == PREAMBLE_CONTRIBUTION_HEAD {
                let owner_document = parent_instance.owner_document();
                let head: &Element = owner_document.head().unwrap();
                clear_singleton_preamble_contribution(head);
                // We need to clear the head because this is the only singleton that can have children that
                // were part of this boundary but are not inside this boundary.
                clear_head(head);
            } else if data == PREAMBLE_CONTRIBUTION_BODY {
                let owner_document = parent_instance.owner_document();
                let body: &Element = owner_document.body().unwrap();
                clear_singleton_preamble_contribution(body);
            }
        }
        // $FlowFixMe[incompatible-type] we bail out when we get a null
        match next_node {
            Some(next_node) => node = next_node,
            None => break,
        }
    }
    // TODO: Warn, we didn't find the end comment boundary.
    // Retry if any event replaying was blocked on this.
    retry_if_blocked_on(js::unknown(hydration_instance));
}

pub fn clear_activity_boundary(
    parent_instance: &Instance,
    activity_instance: &'static ActivityInstance,
) {
    clear_hydration_boundary(parent_instance, activity_instance);
}

pub fn clear_suspense_boundary(
    parent_instance: &Instance,
    suspense_instance: &'static SuspenseInstance,
) {
    clear_hydration_boundary(parent_instance, suspense_instance);
}

fn clear_hydration_boundary_from_container(
    container: &'static Container,
    hydration_instance: &'static Comment,
) {
    let parent_node: &Element = if container.node_type() == DOCUMENT_NODE {
        webapi::document::unchecked_from(container).body().unwrap()
    } else if !DISABLE_COMMENTS_AS_DOM_CONTAINERS && container.node_type() == COMMENT_NODE {
        webapi::element::unchecked_from(container.parent_node().unwrap())
    } else if container.node_name() == "HTML" {
        container.owner_document().unwrap().body().unwrap()
    } else {
        webapi::element::unchecked_from(container)
    };
    clear_hydration_boundary(parent_node, hydration_instance);
    // Retry if any event replaying was blocked on this.
    retry_if_blocked_on(js::unknown(container));
}

pub fn clear_activity_boundary_from_container(
    container: &'static Container,
    activity_instance: &'static ActivityInstance,
) {
    clear_hydration_boundary_from_container(container, activity_instance);
}

pub fn clear_suspense_boundary_from_container(
    container: &'static Container,
    suspense_instance: &'static SuspenseInstance,
) {
    clear_hydration_boundary_from_container(container, suspense_instance);
}

fn hide_or_unhide_dehydrated_boundary(suspense_instance: &'static Comment, is_hidden: bool) {
    let mut node: &Node = suspense_instance;
    // Unhide all nodes within this suspense boundary.
    let mut depth = 0;
    loop {
        let next_node = node.next_sibling();
        if node.node_type() == ELEMENT_NODE {
            let instance = html_element::unchecked_from(node);
            if is_hidden {
                js::set(instance, "_stashedDisplay", instance.style().display());
                instance.style().set_display("none");
            } else {
                let stashed_display: Option<String> =
                    unsafe { js::cast(js::get(instance, "_stashedDisplay")) };
                instance
                    .style()
                    .set_display(stashed_display.unwrap_or_default());
                if instance.get_attribute("style").as_deref() == Some("") {
                    instance.remove_attribute("style");
                }
            }
        } else if node.node_type() == TEXT_NODE {
            let text_node = webapi::text::unchecked_from(node);
            if is_hidden {
                js::set(text_node, "_stashedText", text_node.node_value());
                text_node.set_node_value("");
            } else {
                let stashed_text: Option<String> =
                    unsafe { js::cast(js::get(text_node, "_stashedText")) };
                text_node.set_node_value(stashed_text.as_deref().unwrap_or(""));
            }
        }
        if let Some(next_node) = next_node
            && next_node.node_type() == COMMENT_NODE
        {
            let data = webapi::comment::unchecked_from(next_node).data();
            if data == SUSPENSE_END_DATA || data == ACTIVITY_END_DATA {
                if depth == 0 {
                    return;
                } else {
                    depth -= 1;
                }
            } else if data == SUSPENSE_START_DATA
                || data == SUSPENSE_PENDING_START_DATA
                || data == SUSPENSE_QUEUED_START_DATA
                || data == SUSPENSE_FALLBACK_START_DATA
                || data == ACTIVITY_START_DATA
            {
                depth += 1;
            }
            // TODO: Should we hide preamble contribution in this case?
        }
        // $FlowFixMe[incompatible-type] we bail out when we get a null
        match next_node {
            Some(next_node) => node = next_node,
            None => break,
        }
    }
}

pub fn hide_dehydrated_boundary(suspense_instance: &'static SuspenseInstance) {
    hide_or_unhide_dehydrated_boundary(suspense_instance, true);
}

pub fn hide_instance(instance: &Instance) {
    // TODO: Does this work for all element types? What about MathML? Should we
    // pass host context to this method?
    let instance = html_element::unchecked_from(instance);
    let style = instance.style();
    // $FlowFixMe[method-unbinding]
    if js::is_function((
        style,
        CSSStyleDeclaration::set_property as fn(_, _, &'static str, _),
    )) {
        style.set_property("display", "none", Some("important"));
    } else {
        style.set_display("none");
    }
}

pub fn hide_text_instance(text_instance: &TextInstance) {
    text_instance.set_node_value("");
}

pub fn unhide_dehydrated_boundary(dehydrated_instance: &'static Comment) {
    hide_or_unhide_dehydrated_boundary(dehydrated_instance, false);
}

pub fn unhide_instance(instance: &Instance, props: &Props) {
    let instance = html_element::unchecked_from(instance);
    let style_prop = js::get(props, STYLE);
    let display = match style_prop {
        Some(style_prop) if style_prop.has_own_property("display") => {
            js::get(style_prop, "display")
        }
        _ => None,
    };
    instance.style().set_display(&if display.is_none()
        || matches!(js::classify(display), Kind::Bool(_))
    {
        String::new()
    } else {
        // The value would've errored already if it wasn't safe.
        // eslint-disable-next-line react-internal/safe-string-coercion
        js::string::trim(&js::concat_text(display).unwrap())
    });
}

pub fn unhide_text_instance(text_instance: &TextInstance, text: &str) {
    text_instance.set_node_value(text);
}

fn warn_for_block_inside_inline(instance: &'static HTMLElement) {
    if __DEV__ {
        let mut next_node = instance.first_child();
        while let Some(first) = next_node {
            let mut node: &Node = first;
            if node.node_type() == ELEMENT_NODE
                && get_computed_style(webapi::element::unchecked_from(node), None).display()
                    == "block"
            {
                let fiber = get_instance_from_node_dom_tree(node)
                    .or_else(|| get_instance_from_node_dom_tree(instance));
                run_with_fiber_in_dev(
                    fiber,
                    |parent_tag: String, child_tag: String, _: (), _: (), _: ()| {
                        console::error((
                            concat!(
                                "You're about to start a <ViewTransition> around a display: inline ",
                                "element <%s>, which itself has a display: block element <%s> inside it. ",
                                "This might trigger a bug in Safari which causes the View Transition to ",
                                "be skipped with a duplicate name error.\n",
                                "https://bugs.webkit.org/show_bug.cgi?id=290923"
                            ),
                            js::string::to_locale_lower_case(&parent_tag, None),
                            js::string::to_locale_lower_case(&child_tag, None),
                        ));
                    },
                    instance.tag_name(),
                    webapi::element::unchecked_from(node).tag_name(),
                    (),
                    (),
                    (),
                );
                break;
            }
            if let Some(first_child) = node.first_child() {
                next_node = Some(first_child);
                continue;
            }
            if js::strict_equals(node, instance) {
                break;
            }
            while node.next_sibling().is_none() {
                match node.parent_node() {
                    Some(parent_node) if !js::strict_equals(parent_node, instance) => {
                        node = parent_node
                    }
                    _ => break,
                }
            }
            next_node = node.next_sibling();
        }
    }
}

fn count_client_rects(rects: &DOMRectList) -> u32 {
    if rects.length() == 1.0 {
        return 1;
    }
    // Count non-zero rects.
    let mut count = 0;
    for i in 0..rects.length() as usize {
        let rect = &rects[i];
        if rect.width() > 0.0 && rect.height() > 0.0 {
            count += 1;
        }
    }
    count
}

pub fn apply_view_transition_name(instance: &Instance, name: &str, class_name: Option<&str>) {
    let instance = html_element::unchecked_from(instance);
    // If the name isn't valid CSS identifier, base64 encode the name instead.
    // This doesn't let you select it in custom CSS selectors but it does work in current
    // browsers.
    let escaped_name = if webapi::css::escape(name) != name {
        "r-".to_owned() + &btoa(name).replace('=', "")
    } else {
        name.to_owned()
    };
    // $FlowFixMe[prop-missing]
    instance.style().set_view_transition_name(&escaped_name);
    if let Some(class_name) = class_name {
        // $FlowFixMe[prop-missing]
        instance.style().set_view_transition_class(class_name);
    }
    let computed_style = get_computed_style(instance, None);
    if computed_style.display() == "inline" {
        // WebKit has a bug where assigning a name to display: inline elements errors
        // if they have display: block children. We try to work around this bug in the
        // simple case by converting it automatically to display: inline-block.
        // https://bugs.webkit.org/show_bug.cgi?id=290923
        let rects = instance.get_client_rects();
        if count_client_rects(rects) == 1 {
            // If the instance has a single client rect, that means that it can be
            // expressed as a display: inline-block or block.
            // This will cause layout thrash but we live with it since inline view transitions
            // are unusual.
            let style = instance.style();
            // If there's literally only one rect, then it's likely on a single line like an
            // inline-block. If it's multiple rects but all but one of them are empty it's
            // likely because it's a single block that caused a line break.
            style.set_display(if rects.length() == 1.0 {
                "inline-block"
            } else {
                "block"
            });
            // Margin doesn't apply to inline so should be zero. However, padding top/bottom
            // applies to inline-block positioning which we can offset by setting the margin
            // to the negative padding to get it back into original position.
            style.set_margin_top(&("-".to_owned() + &computed_style.padding_top()));
            style.set_margin_bottom(&("-".to_owned() + &computed_style.padding_bottom()));
        } else {
            // This case cannot be easily fixed if it has blocks but it's also fine if
            // it doesn't have blocks. So we only warn in DEV about this being an issue.
            warn_for_block_inside_inline(instance);
        }
    }
}

pub fn restore_view_transition_name(instance: &Instance, props: &Props) {
    let instance = html_element::unchecked_from(instance);
    let style = instance.style();
    let style_prop: Option<&StyleProp> = unsafe { js::cast(js::get(props, STYLE)) };
    let view_transition_name = match style_prop {
        Some(style_prop) => {
            if js::unknown(style_prop).has_own_property("viewTransitionName") {
                style_prop.view_transition_name
            } else if js::unknown(style_prop).has_own_property("view-transition-name") {
                style_prop.view_transition_name_dashed
            } else {
                None
            }
        }
        None => None,
    };
    // $FlowFixMe[prop-missing]
    style.set_view_transition_name(&if view_transition_name.is_none()
        || matches!(js::classify(view_transition_name), Kind::Bool(_))
    {
        String::new()
    } else {
        // The value would've errored already if it wasn't safe.
        // eslint-disable-next-line react-internal/safe-string-coercion
        js::string::trim(&js::concat_text(view_transition_name).unwrap())
    });
    let view_transition_class = match style_prop {
        Some(style_prop) => {
            if js::unknown(style_prop).has_own_property("viewTransitionClass") {
                style_prop.view_transition_class
            } else if js::unknown(style_prop).has_own_property("view-transition-class") {
                style_prop.view_transition_class_dashed
            } else {
                None
            }
        }
        None => None,
    };
    // $FlowFixMe[prop-missing]
    style.set_view_transition_class(&if view_transition_class.is_none()
        || matches!(js::classify(view_transition_class), Kind::Bool(_))
    {
        String::new()
    } else {
        // The value would've errored already if it wasn't safe.
        // eslint-disable-next-line react-internal/safe-string-coercion
        js::string::trim(&js::concat_text(view_transition_class).unwrap())
    });
    if style.display() == "inline-block" {
        // We might have overridden the style. Reset it to what it should be.
        match style_prop {
            None => {
                style.set_margin("");
                style.set_display("");
            }
            Some(style_prop) => {
                let display = style_prop.display;
                style.set_display(
                    if display.is_none() || matches!(js::classify(display), Kind::Bool(_)) {
                        Some(js::unknown(""))
                    } else {
                        display
                    },
                );
                let margin = style_prop.margin;
                if margin.is_some() {
                    style.set_margin(margin);
                } else {
                    let margin_top = if js::unknown(style_prop).has_own_property("marginTop") {
                        style_prop.margin_top
                    } else {
                        style_prop.margin_top_dashed
                    };
                    style.set_margin_top(
                        if margin_top.is_none() || matches!(js::classify(margin_top), Kind::Bool(_))
                        {
                            Some(js::unknown(""))
                        } else {
                            margin_top
                        },
                    );
                    let margin_bottom = if js::unknown(style_prop).has_own_property("marginBottom")
                    {
                        style_prop.margin_bottom
                    } else {
                        style_prop.margin_bottom_dashed
                    };
                    style.set_margin_bottom(
                        if margin_bottom.is_none()
                            || matches!(js::classify(margin_bottom), Kind::Bool(_))
                        {
                            Some(js::unknown(""))
                        } else {
                            margin_bottom
                        },
                    );
                }
            }
        }
    }
}

pub fn cancel_view_transition_name(instance: &Instance, old_name: &str, props: &Props) {
    // To cancel the "new" state and paint this instance as part of the parent, all we have to do
    // is remove the view-transition-name before we exit startViewTransition.
    restore_view_transition_name(instance, props);
    // There isn't a way to cancel an "old" state but what we can do is hide it by animating it.
    // Since it is already removed from the old state of the parent, this technique only works
    // if the parent also isn't transitioning. Therefore we should only cancel the root most
    // ViewTransitions.
    let document_element = instance.owner_document().document_element();
    if let Some(document_element) = document_element {
        document_element.animate(
            Some(&HiddenKeyframes {
                opacity: [0.0, 0.0],
                pointer_events: ["none", "none"],
            }),
            // $FlowFixMe[incompatible-type]
            Some(&KeyframeAnimationOptions {
                duration: Some(NumberOrCSSNumericValueOrStr::Number(0.0)),
                fill: Some("forwards"),
                pseudo_element: Some(&("::view-transition-group(".to_owned() + old_name + ")")),
                ..Default::default()
            }),
        );
    }
}

/// The keyframes of an animation that hides what it animates and lets
/// clicks through.
struct HiddenKeyframes {
    opacity: [f64; 2],
    pointer_events: [&'static str; 2],
}

unsafe impl js::Defined for HiddenKeyframes {}

/// The keyframes of an animation that sizes what it animates to nothing.
struct EmptyKeyframes {
    width: [f64; 2],
    height: [f64; 2],
}

unsafe impl js::Defined for EmptyKeyframes {}

pub fn cancel_root_view_transition_name(root_container: &'static Container) {
    let document_element: Option<&Element> = if root_container.node_type() == DOCUMENT_NODE {
        webapi::document::unchecked_from(root_container).document_element()
    } else {
        root_container.owner_document().unwrap().document_element()
    };

    if !DISABLE_COMMENTS_AS_DOM_CONTAINERS && root_container.node_type() == COMMENT_NODE {
        if __DEV__ {
            console::warn((
                "Cannot cancel root view transition on a comment node. All view transitions will be globally scoped.",
            ));
        }
        return;
    }

    if let Some(document_element) = document_element
        // $FlowFixMe[prop-missing]
        && html_element::unchecked_from(document_element).style().view_transition_name() == ""
    {
        let document_element = html_element::unchecked_from(document_element);
        // $FlowFixMe[prop-missing]
        document_element.style().set_view_transition_name("none");
        document_element.animate(
            Some(&HiddenKeyframes {
                opacity: [0.0, 0.0],
                pointer_events: ["none", "none"],
            }),
            // $FlowFixMe[incompatible-type]
            Some(&KeyframeAnimationOptions {
                duration: Some(NumberOrCSSNumericValueOrStr::Number(0.0)),
                fill: Some("forwards"),
                pseudo_element: Some("::view-transition-group(root)"),
                ..Default::default()
            }),
        );
        // By default the root ::view-transition selector captures all pointer events,
        // which means nothing gets interactive. We want to let whatever is not animating
        // remain interactive during the transition. To do that, we set the size to nothing
        // so that the transition doesn't capture any clicks. We don't set pointer-events
        // on this one as that would apply to all running transitions. This lets animations
        // that are running to block clicks so that they don't end up incorrectly hitting
        // whatever is below the animation.
        document_element.animate(
            Some(&EmptyKeyframes {
                width: [0.0, 0.0],
                height: [0.0, 0.0],
            }),
            // $FlowFixMe[incompatible-call]
            // $FlowFixMe[incompatible-type]
            Some(&KeyframeAnimationOptions {
                duration: Some(NumberOrCSSNumericValueOrStr::Number(0.0)),
                fill: Some("forwards"),
                pseudo_element: Some("::view-transition"),
                ..Default::default()
            }),
        );
    }
}

pub fn restore_root_view_transition_name(root_container: &'static Container) {
    let container_instance: &Instance = if root_container.node_type() == DOCUMENT_NODE {
        webapi::document::unchecked_from(root_container)
            .body()
            .unwrap()
    } else if root_container.node_name() == "HTML" {
        root_container.owner_document().unwrap().body().unwrap()
    } else {
        // If the container is not the whole document, then we ideally should probably
        // clone the whole document outside of the React too.
        webapi::element::unchecked_from(root_container)
    };
    if !DISABLE_COMMENTS_AS_DOM_CONTAINERS && container_instance.node_type() == COMMENT_NODE {
        return;
    }
    let container_instance = html_element::unchecked_from(container_instance);
    if
    // $FlowFixMe[prop-missing]
    container_instance.style().view_transition_name() == "root" {
        // If we moved the root view transition name to the container in a gesture
        // we need to restore it now.
        container_instance.style().set_view_transition_name("");
    }
    let document_element: Option<&Element> = container_instance.owner_document().document_element();
    if let Some(document_element) = document_element
        // $FlowFixMe[prop-missing]
        && html_element::unchecked_from(document_element).style().view_transition_name() == "none"
    {
        // $FlowFixMe[prop-missing]
        html_element::unchecked_from(document_element)
            .style()
            .set_view_transition_name("");
    }
}

fn get_computed_transform(style: &CSSStyleProperties) -> String {
    // Gets the merged transform of all the short hands.
    let mut transform = style.transform();
    if transform == "none" {
        transform = String::new();
    }
    let scale = style.scale();
    if scale != "none" && !scale.is_empty() {
        let parts: Vec<&str> = scale.split(' ').collect();
        transform = (if parts.len() == 3 { "scale3d" } else { "scale" }).to_owned()
            + "("
            + &parts.join(", ")
            + ") "
            + &transform;
    }
    let rotate = style.rotate();
    if rotate != "none" && !rotate.is_empty() {
        let parts: Vec<&str> = rotate.split(' ').collect();
        if parts.len() == 1 {
            transform = "rotate(".to_owned() + parts[0] + ") " + &transform;
        } else if parts.len() == 2 {
            transform =
                "rotate".to_owned() + &parts[0].to_uppercase() + "(" + parts[1] + ") " + &transform;
        } else {
            transform = "rotate3d(".to_owned() + &parts.join(", ") + ") " + &transform;
        }
    }
    let translate = style.translate();
    if translate != "none" && !translate.is_empty() {
        let parts: Vec<&str> = translate.split(' ').collect();
        transform = (if parts.len() == 3 {
            "translate3d"
        } else {
            "translate"
        })
        .to_owned()
            + "("
            + &parts.join(", ")
            + ") "
            + &transform;
    }
    transform
}

fn move_out_of_viewport(original_style: &CSSStyleProperties, element: &HTMLElement) {
    // Apply a transform that safely puts the whole element outside the viewport
    // while still letting it paint its "old" state to a snapshot.
    let transform = get_computed_transform(original_style);
    // Clear the long form properties.
    // $FlowFixMe[prop-missing]
    element.style().set_translate("none");
    // $FlowFixMe[prop-missing]
    element.style().set_scale("none");
    // $FlowFixMe[prop-missing]
    element.style().set_rotate("none");
    // Apply a translate to move it way out of the viewport. This is applied first
    // so that it is in the coordinate space of the parent and not after applying
    // other transforms. That's why we need to merge the long form properties.
    // TODO: Ideally we'd adjust for the parent's rotate/scale. Otherwise when
    // we move back the ::view-transition-group we might overshoot or undershoot.
    element
        .style()
        .set_transform(&("translate(-20000px, -20000px) ".to_owned() + &transform));
}

fn move_old_frame_into_viewport(keyframe: &'static Unknown) {
    // In the resulting View Transition Animation, the first frame will be offset.
    let computed_transform: Option<String> = unsafe { js::cast(js::get(keyframe, "transform")) };
    if let Some(computed_transform) = computed_transform {
        let mut transform = if computed_transform == "none" {
            String::new()
        } else {
            computed_transform
        };
        transform = "translate(20000px, 20000px) ".to_owned() + &transform;
        js::set(keyframe, "transform", transform);
    }
}

pub fn clone_root_view_transition_container(
    root_container: &'static Container,
) -> &'static Instance {
    // This implies that we're not going to animate the root document but instead
    // the clone so we first clear the name of the root container.
    let document_element: Option<&Element> = if root_container.node_type() == DOCUMENT_NODE {
        webapi::document::unchecked_from(root_container).document_element()
    } else {
        root_container.owner_document().unwrap().document_element()
    };
    if let Some(document_element) = document_element
        // $FlowFixMe[prop-missing]
        && html_element::unchecked_from(document_element).style().view_transition_name() == ""
    {
        // $FlowFixMe[prop-missing]
        html_element::unchecked_from(document_element)
            .style()
            .set_view_transition_name("none");
    }

    let container_instance: &HTMLElement = if root_container.node_type() == DOCUMENT_NODE {
        webapi::document::unchecked_from(root_container)
            .body()
            .unwrap()
    } else if root_container.node_name() == "HTML" {
        root_container.owner_document().unwrap().body().unwrap()
    } else if !DISABLE_COMMENTS_AS_DOM_CONTAINERS && root_container.node_type() == COMMENT_NODE {
        panic!("Cannot use a startGestureTransition() with a comment node root.");
    } else {
        // If the container is not the whole document, then we ideally should probably
        // clone the whole document outside of the React too.
        html_element::unchecked_from(root_container)
    };

    let Some(container_parent) = container_instance.parent_node() else {
        panic!("Cannot use a startGestureTransition() on a detached root.");
    };

    let clone = html_element::unchecked_from(container_instance.clone_node(Some(false)));

    let computed_style = get_computed_style(container_instance, None);

    if computed_style.position() == "absolute" || computed_style.position() == "fixed" {
        // If the style is already absolute, we don't have to do anything because it'll appear
        // in the same place.
    } else {
        // Otherwise we need to absolutely position the clone in the same location as the original.
        let mut positioned_ancestor = html_element::unchecked_from(container_parent);
        while let Some(parent_node) = positioned_ancestor.parent_node()
            && parent_node.node_type() != DOCUMENT_NODE
        {
            if get_computed_style(positioned_ancestor, None).position() != "static" {
                break;
            }
            // $FlowFixMe[incompatible-type]: This is refined.
            positioned_ancestor = html_element::unchecked_from(parent_node);
        }

        let positioned_ancestor_style = positioned_ancestor.style();
        let container_instance_style = container_instance.style();
        // Clear the transform while we're measuring since it affects the bounding client rect.
        let prev_ancestor_translate = positioned_ancestor_style.translate();
        let prev_ancestor_scale = positioned_ancestor_style.scale();
        let prev_ancestor_rotate = positioned_ancestor_style.rotate();
        let prev_ancestor_transform = positioned_ancestor_style.transform();
        let prev_translate = container_instance_style.translate();
        let prev_scale = container_instance_style.scale();
        let prev_rotate = container_instance_style.rotate();
        let prev_transform = container_instance_style.transform();
        positioned_ancestor_style.set_translate("none");
        positioned_ancestor_style.set_scale("none");
        positioned_ancestor_style.set_rotate("none");
        positioned_ancestor_style.set_transform("none");
        container_instance_style.set_translate("none");
        container_instance_style.set_scale("none");
        container_instance_style.set_rotate("none");
        container_instance_style.set_transform("none");

        let ancestor_rect = positioned_ancestor.get_bounding_client_rect();
        let rect = container_instance.get_bounding_client_rect();

        let clone_style = clone.style();
        clone_style.set_position("absolute");
        clone_style.set_top(&(js::concat_text(rect.top() - ancestor_rect.top()) + "px"));
        clone_style.set_left(&(js::concat_text(rect.left() - ancestor_rect.left()) + "px"));
        clone_style.set_width(&(js::concat_text(rect.width()) + "px"));
        clone_style.set_height(&(js::concat_text(rect.height()) + "px"));
        clone_style.set_margin("0px");
        clone_style.set_box_sizing("border-box");

        positioned_ancestor_style.set_translate(&prev_ancestor_translate);
        positioned_ancestor_style.set_scale(&prev_ancestor_scale);
        positioned_ancestor_style.set_rotate(&prev_ancestor_rotate);
        positioned_ancestor_style.set_transform(&prev_ancestor_transform);
        container_instance_style.set_translate(&prev_translate);
        container_instance_style.set_scale(&prev_scale);
        container_instance_style.set_rotate(&prev_rotate);
        container_instance_style.set_transform(&prev_transform);
    }

    // For this transition the container will act as the root. Nothing outside of it should
    // be affected anyway. This lets us transition from the cloned container to the original.
    // $FlowFixMe[prop-missing]
    clone.style().set_view_transition_name("root");

    // Move out of the viewport so that it's still painted for the snapshot but is not visible
    // for the frame where the snapshot happens.
    move_out_of_viewport(computed_style, clone);

    // Insert the clone after the root container as a sibling. This may inject a body
    // as the next sibling of an existing body. document.body will still point to the
    // first one and any id selectors will still find the first one. That's why it's
    // important that it's after the existing node.
    container_instance
        .parent_node()
        .unwrap()
        .insert_before(clone, container_instance.next_sibling());

    clone
}

pub fn remove_root_view_transition_clone(root_container: &'static Container, clone: &Instance) {
    let container_instance: &Instance = if root_container.node_type() == DOCUMENT_NODE {
        webapi::document::unchecked_from(root_container)
            .body()
            .unwrap()
    } else if root_container.node_name() == "HTML" {
        root_container.owner_document().unwrap().body().unwrap()
    } else {
        // If the container is not the whole document, then we ideally should probably
        // clone the whole document outside of the React too.
        webapi::element::unchecked_from(root_container)
    };
    let Some(container_parent) = container_instance.parent_node() else {
        panic!("Cannot use a startGestureTransition() on a detached root.");
    };
    // We assume that the clone is still within the same parent.
    container_parent.remove_child(clone);

    // Now the root is on the containerInstance itself until we call restoreRootViewTransitionName.
    html_element::unchecked_from(container_instance)
        .style()
        .set_view_transition_name("root");
}

/// Where an instance was laid out, and what of its style a View Transition
/// asks of it.
pub struct InstanceMeasurement {
    pub rect: &'static DOMRect,
    pub abs: bool,  // is absolutely positioned
    pub clip: bool, // is a clipping parent
    pub view: bool, // is in viewport bounds
}

unsafe impl js::Defined for InstanceMeasurement {}

fn create_measurement(
    rect: &'static DOMRect,
    computed_style: &CSSStyleProperties,
    element: &Element,
) -> InstanceMeasurement {
    let owner_window = element.owner_document().default_view().unwrap();
    InstanceMeasurement {
        rect,
        abs:
            // Absolutely positioned instances don't contribute their size to the parent.
            computed_style.position() == "absolute" || computed_style.position() == "fixed",
        clip:
            // If a ViewTransition boundary acts as a clipping parent group we should
            // always mark it to animate if its children do so that we can clip them.
            // This doesn't actually have any effect yet until browsers implement
            // layered capture and nested view transitions.
            computed_style.clip_path() != "none"
                || computed_style.overflow() != "visible"
                || computed_style.filter() != "none"
                || computed_style.mask() != "none"
                || computed_style.mask() != "none"
                || computed_style.border_radius() != "0px",
        view:
            // If the instance was within the bounds of the viewport. We don't care as
            // much about if it was fully occluded because then it can still pop out.
            rect.bottom() >= 0.0
                && rect.right() >= 0.0
                && rect.top() <= owner_window.inner_height()
                && rect.left() <= owner_window.inner_width(),
    }
}

pub fn measure_instance(instance: &Instance) -> InstanceMeasurement {
    let rect = instance.get_bounding_client_rect();
    let computed_style = get_computed_style(instance, None);
    create_measurement(rect, computed_style, instance)
}

pub fn measure_cloned_instance(instance: &Instance) -> InstanceMeasurement {
    let measured_rect = instance.get_bounding_client_rect();
    // Adjust the DOMRect based on the translate that put it outside the viewport.
    // TODO: This might not be completely correct if the parent also has a transform.
    let rect = webapi::dom_rect::new(
        Some(measured_rect.x() + 20000.0),
        Some(measured_rect.y() + 20000.0),
        Some(measured_rect.width()),
        Some(measured_rect.height()),
    );
    let computed_style = get_computed_style(instance, None);
    create_measurement(rect, computed_style, instance)
}

pub fn was_instance_in_viewport(measurement: &InstanceMeasurement) -> bool {
    measurement.view
}

pub fn has_instance_changed(
    old_measurement: &InstanceMeasurement,
    new_measurement: &InstanceMeasurement,
) -> bool {
    // Note: This is not guaranteed from the same instance in the case that the Instance of the
    // ViewTransition swaps out but it's still the same ViewTransition instance.
    if new_measurement.clip {
        // If we're a clipping parent, we always animate if any of our children do so that we can clip
        // them. This doesn't yet until browsers implement layered capture and nested view transitions.
        return true;
    }
    let old_rect = old_measurement.rect;
    let new_rect = new_measurement.rect;
    old_rect.y() != new_rect.y()
        || old_rect.x() != new_rect.x()
        || old_rect.height() != new_rect.height()
        || old_rect.width() != new_rect.width()
}

pub fn has_instance_affected_parent(
    old_measurement: &InstanceMeasurement,
    new_measurement: &InstanceMeasurement,
) -> bool {
    // Note: This is not guaranteed from the same instance in the case that the Instance of the
    // ViewTransition swaps out but it's still the same ViewTransition instance.
    // If the instance has resized, it might have affected the parent layout.
    if new_measurement.abs {
        // Absolutely positioned elements don't affect the parent layout, unless they
        // previously were not absolutely positioned.
        return !old_measurement.abs;
    }
    let old_rect = old_measurement.rect;
    let new_rect = new_measurement.rect;
    old_rect.height() != new_rect.height() || old_rect.width() != new_rect.width()
}

// How long to wait for new fonts to load before just committing anyway.
// This freezes the screen. It needs to be short enough that it doesn't cause too much of
// an issue when it's a new load and slow, yet long enough that you have a chance to load
// it. Otherwise we wait for no reason. The assumption here is that you likely have
// either cached the font or preloaded it earlier.
// This timeout is also used for Suspensey Images when they're blocking a View Transition.
const SUSPENSEY_FONT_AND_IMAGE_TIMEOUT: f64 = 500.0;

fn customize_view_transition_error(
    error: &'static JsError,
    ignore_abort: bool,
) -> Option<&'static Unknown> {
    if matches!(
        js::classify(error.as_unknown()),
        Kind::Object(_) | Kind::Array(_)
    ) {
        match error.name().as_str() {
            "TimeoutError" => {
                // We assume that the only reason a Timeout can happen is because the Navigation
                // promise. We expect any other work to either be fast or have a timeout (fonts).
                if __DEV__ {
                    // eslint-disable-next-line react-internal/prod-error-codes
                    return js::error::new(
                        concat!(
                            "A ViewTransition timed out because a Navigation stalled. ",
                            "This can happen if a Navigation is blocked on React itself. ",
                            "Such as if it's resolved inside useEffect. ",
                            "This can be solved by moving the resolution to useLayoutEffect."
                        ),
                        Some(&ErrorOptions {
                            cause: Some(&error),
                        }),
                    )
                    .as_unknown();
                }
            }
            "AbortError" => {
                if ignore_abort {
                    return js::null();
                }
                if __DEV__ {
                    // eslint-disable-next-line react-internal/prod-error-codes
                    return js::error::new(
                        concat!(
                            "A ViewTransition was aborted early. This might be because you have ",
                            "other View Transition libraries on the page and only one can run at ",
                            "a time. To avoid this, use only React's built-in <ViewTransition> ",
                            "to coordinate."
                        ),
                        Some(&ErrorOptions {
                            cause: Some(&error),
                        }),
                    )
                    .as_unknown();
                }
            }
            "InvalidStateError"
                if error.message()
                    == "View transition was skipped because document visibility state is hidden."
                    || error.message()
                        == "Skipping view transition because document visibility state has become hidden."
                    || error.message() == "Skipping view transition because viewport size changed."
                    // Chrome uses a generic error message instead of specific reasons. It will log a
                    // more specific reason in the console but the user might not look there.
                    // Some of these errors are important to surface like duplicate name errors but
                    // it's too noisy for unactionable cases like the document was hidden. Therefore,
                    // we hide all of them and hopefully it surfaces in another browser.
                    || error.message() == "Transition was aborted because of invalid state" =>
            {
                // Skip logging this. This is not considered an error.
                return js::null();
            }
            _ => {}
        }
    }
    error.as_unknown()
}

/** @noinline */
fn force_layout(owner_document: &Document) -> f64 {
    // This function exists to trick minifiers to not remove this unused member expression.
    owner_document.document_element().unwrap().client_height()
}

fn wait_for_image_to_load(
    image: &'static HTMLImageElement,
    resolve: &'static dyn Fn(Option<&'static Unknown>),
) {
    // TODO: Use decode() instead of the load event here once the fix in
    // https://issues.chromium.org/issues/420748301 has propagated fully.
    image.add_event_listener_named("load", move |_: &'static Event| resolve(None), None);
    image.add_event_listener_named("error", move |_: &'static Event| resolve(None), None);
}

/// What a View Transition waits on before it animates: a promise of any value.
type BlockingPromise = Promise<Option<&'static Unknown>>;

pub fn start_view_transition(
    suspended_state: Option<&'static SuspendedState>,
    root_container: &'static Container,
    transition_types: Option<Rc<RefCell<TransitionTypes>>>,
    mutation_callback: &'static dyn Fn(),
    layout_callback: &'static dyn Fn(),
    after_mutation_callback: &'static dyn Fn(),
    spawned_work_callback: &'static dyn Fn(),
    passive_callback: &'static dyn Fn() -> Option<&'static Unknown>,
    error_callback: &'static dyn Fn(Option<&'static Unknown>),
    blocked_callback: &'static dyn Fn(&str), // Profiling-only
    finished_animation: &'static dyn Fn(),   // Profiling-only
) -> Option<&'static RunningViewTransition> {
    let owner_document: &'static Document = if root_container.node_type() == DOCUMENT_NODE {
        webapi::document::unchecked_from(root_container)
    } else {
        root_container.owner_document().unwrap()
    };
    match catch_unwind(AssertUnwindSafe(|| {
        let types: Option<Vec<String>> = transition_types
            .as_ref()
            .map(|types| types.borrow().clone());
        // $FlowFixMe[prop-missing]
        let transition =
            owner_document.start_view_transition(Some(&StartViewTransitionOptions {
                update: Some(&move || -> Option<BlockingPromise> {
                    // Note: We read the existence of a pending navigation before we apply the
                    // mutations. That way we're not waiting on a navigation that we spawned
                    // from this update. Only navigations that started before this commit.
                    let owner_window = owner_document.default_view().unwrap();
                    let pending_navigation = if js::is_defined((owner_window, Window::navigation)) {
                        owner_window.navigation().transition()
                    } else {
                        None
                    };
                    // $FlowFixMe[prop-missing]
                    let previous_font_loading_status = owner_document.fonts().status();
                    mutation_callback();
                    let mut blocking_promises: Vec<BlockingPromise> = Vec::new();
                    if previous_font_loading_status == "loaded" {
                        // Force layout calculation to trigger font loading.
                        force_layout(owner_document);
                        if
                        // $FlowFixMe[prop-missing]
                        owner_document.fonts().status() == "loading" {
                            // The mutation lead to new fonts being loaded. We should wait on them before continuing.
                            // This avoids waiting for potentially unrelated fonts that were already loading before.
                            // Either in an earlier transition or as part of a sync optimistic state. This doesn't
                            // include preloads that happened earlier.
                            blocking_promises.push(owner_document.fonts().ready().into_unknown());
                        }
                    }
                    let blocking_index_snapshot = blocking_promises.len();
                    if let Some(suspended_state) = suspended_state {
                        // Suspend on any images that still haven't loaded and are in the viewport.
                        let suspensey_images = &suspended_state.suspensey_images;
                        let mut img_bytes = 0.0;
                        let mut i = 0;
                        while i < suspensey_images.borrow().len() {
                            let suspensey_image = suspensey_images.borrow()[i];
                            if !suspensey_image.complete() {
                                let rect = suspensey_image.get_bounding_client_rect();
                                let in_viewport = rect.bottom() > 0.0
                                    && rect.right() > 0.0
                                    && rect.top() < owner_window.inner_height()
                                    && rect.left() < owner_window.inner_width();
                                if in_viewport {
                                    img_bytes += estimate_image_bytes(suspensey_image);
                                    if img_bytes > ESTIMATED_BYTES_WITHIN_LIMIT.get() {
                                        // We don't think we'll be able to download all the images within
                                        // the timeout. Give up. Rewind to only block on fonts, if any.
                                        blocking_promises.truncate(blocking_index_snapshot);
                                        break;
                                    }
                                    let loading_image = js::promise::new(|resolve, _| {
                                        wait_for_image_to_load(suspensey_image, resolve)
                                    });
                                    blocking_promises.push(loading_image);
                                }
                            }
                            i += 1;
                        }
                    }
                    if !blocking_promises.is_empty() {
                        if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
                            let blocked_reason = if blocking_index_snapshot > 0 {
                                if blocking_promises.len() > blocking_index_snapshot {
                                    "Waiting on Fonts and Images"
                                } else {
                                    "Waiting on Fonts"
                                }
                            } else {
                                "Waiting on Images"
                            };
                            blocked_callback(blocked_reason);
                        }
                        let blocking_ready: BlockingPromise = js::promise::race(vec![
                            js::promise::all(blocking_promises).into_unknown(),
                            js::promise::new(
                                |resolve: &'static dyn Fn(Option<&'static Unknown>), _| {
                                    js::set_timeout(
                                        move || resolve(None),
                                        Some(SUSPENSEY_FONT_AND_IMAGE_TIMEOUT),
                                    );
                                },
                            ),
                        ])
                        .then_with(|_| layout_callback(), |_| layout_callback());
                        let all_ready: BlockingPromise =
                            if let Some(pending_navigation) = pending_navigation {
                                js::promise::all_settled(vec![
                                    pending_navigation.finished().into_unknown(),
                                    blocking_ready,
                                ])
                                .into_unknown()
                            } else {
                                blocking_ready
                            };
                        return Some(all_ready.then_with(
                            |_| after_mutation_callback(),
                            |_| after_mutation_callback(),
                        ));
                    }
                    layout_callback();
                    if let Some(pending_navigation) = pending_navigation {
                        Some(pending_navigation.finished().then_with(
                            |_| after_mutation_callback(),
                            |_| after_mutation_callback(),
                        ))
                    } else {
                        after_mutation_callback();
                        None
                    }
                }),
                types: types.as_deref(),
            }));
        // $FlowFixMe[prop-missing]
        js::set(owner_document, "__reactViewTransition", transition);

        let view_transition_animations: Rc<RefCell<Vec<&'static Animation>>> =
            Rc::new(RefCell::new(Vec::new()));

        let ready_callback = {
            let view_transition_animations = Rc::clone(&view_transition_animations);
            move || {
                let document_element: &Element = owner_document.document_element().unwrap();
                // Loop through all View Transition Animations.
                // $FlowFixMe[prop-missing]
                // $FlowFixMe[incompatible-type]
                let animations = document_element.get_animations(Some(GetAnimationsOptions {
                    subtree: Some(true),
                    ..Default::default()
                }));
                for animation in animations {
                    let effect = keyframe_effect::unchecked_from(animation.effect().unwrap());
                    // $FlowFixMe[prop-missing]
                    let pseudo_element: Option<String> = effect.pseudo_element();
                    if let Some(pseudo_element) = pseudo_element
                        && pseudo_element.starts_with("::view-transition")
                    {
                        view_transition_animations.borrow_mut().push(animation);
                        let keyframes = effect.get_keyframes();
                        // Next, we're going to try to optimize this animation in case the auto-generated
                        // width/height keyframes are unnecessary.
                        let mut width: Option<&'static Unknown> = None;
                        let mut height: Option<&'static Unknown> = None;
                        let mut unchanged_dimensions = true;
                        for &keyframe in &keyframes {
                            let w = js::get(keyframe, "width");
                            if width.is_none() {
                                width = w;
                            } else if !js::strict_equals(width, w) {
                                unchanged_dimensions = false;
                                break;
                            }
                            let h = js::get(keyframe, "height");
                            if height.is_none() {
                                height = h;
                            } else if !js::strict_equals(height, h) {
                                unchanged_dimensions = false;
                                break;
                            }
                            // We're clearing the keyframes in case we are going to apply the optimization.
                            js::delete(keyframe, "width");
                            js::delete(keyframe, "height");
                            if js::strict_equals(js::get(keyframe, "transform"), "none") {
                                js::delete(keyframe, "transform");
                            }
                        }
                        if unchanged_dimensions
                            && let Some(width) = width
                            && let Some(height) = height
                        {
                            // Replace the keyframes with ones that don't animate the width/height.
                            // $FlowFixMe[incompatible-type]
                            effect.set_keyframes(Some(&keyframes));
                            // Read back the new animation to see what the underlying width/height of the pseudo-element was.
                            let computed_style = get_computed_style(
                                // $FlowFixMe[incompatible-type]
                                effect.target().unwrap(),
                                // $FlowFixMe[prop-missing]
                                effect.pseudo_element().as_deref(),
                            );
                            if !js::strict_equals(computed_style.width(), width)
                                || !js::strict_equals(computed_style.height(), height)
                            {
                                // Oops. Turns out that the pseudo-element had a different width/height so we need to let it
                                // be overridden. Add it back.
                                let first = keyframes[0];
                                js::set(first, "width", width);
                                js::set(first, "height", height);
                                let last = keyframes[keyframes.len() - 1];
                                js::set(last, "width", width);
                                js::set(last, "height", height);
                                // $FlowFixMe[incompatible-type]
                                effect.set_keyframes(Some(&keyframes));
                            }
                        }
                    }
                }
                spawned_work_callback();
            }
        };
        let handle_error = move |error: &'static JsError| {
            // $FlowFixMe[prop-missing]
            if js::strict_equals(js::get(owner_document, "__reactViewTransition"), transition) {
                // $FlowFixMe[prop-missing]
                js::set(
                    owner_document,
                    "__reactViewTransition",
                    js::null::<&Unknown>(),
                );
            }
            let _finish = FinishViewTransition {
                mutation_callback,
                layout_callback,
                spawned_work_callback,
                finished_animation,
            };
            let error = customize_view_transition_error(error, false);
            if error.is_some() {
                error_callback(error);
            }
        };
        transition
            .ready()
            .then_with::<(), _, _>(move |()| ready_callback(), handle_error);
        transition.finished().finally(move || {
            let mut i = 0;
            while i < view_transition_animations.borrow().len() {
                // In Safari, we need to manually cancel all manually started animations
                // or it'll block or interfer with future transitions.
                // We can't use getAnimations() due to #35336 so we collect them in an array.
                let animation = view_transition_animations.borrow()[i];
                animation.cancel();
                i += 1;
            }
            // $FlowFixMe[prop-missing]
            if js::strict_equals(js::get(owner_document, "__reactViewTransition"), transition) {
                // $FlowFixMe[prop-missing]
                js::set(
                    owner_document,
                    "__reactViewTransition",
                    js::null::<&Unknown>(),
                );
            }
            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
                finished_animation();
            }
            passive_callback();
        });
        transition
    })) {
        Ok(transition) => Some(transition),
        Err(_) => {
            // We use the error as feature detection.
            // The only thing that should throw is if startViewTransition is missing
            // or if it doesn't accept the object form. Other errors are async.
            // I.e. it's before the View Transitions v2 spec. We only support View
            // Transitions v2 otherwise we fallback to not animating to ensure that
            // we're not animating with the wrong animation mapped.
            // Flush remaining work synchronously.
            mutation_callback();
            layout_callback();
            // Skip afterMutationCallback(). We don't need it since we're not animating.
            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
                finished_animation();
            }
            spawned_work_callback();
            // Skip passiveCallback(). Spawned work will schedule a task.
            None
        }
    }
}

/// What a View Transition's error handler finishes, however the error is
/// reported.
struct FinishViewTransition {
    mutation_callback: &'static dyn Fn(),
    layout_callback: &'static dyn Fn(),
    spawned_work_callback: &'static dyn Fn(),
    finished_animation: &'static dyn Fn(),
}

impl Drop for FinishViewTransition {
    fn drop(&mut self) {
        // Continue the reset of the work.
        // If the error happened in the snapshot phase before the update callback
        // was invoked, then we need to first finish the mutation and layout phases.
        // If they're already invoked it's still safe to call them due the status check.
        (self.mutation_callback)();
        (self.layout_callback)();
        // Skip afterMutationCallback() since we're not animating.
        (self.spawned_work_callback)();
        if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
            (self.finished_animation)();
        }
    }
}

/// What runs a View Transition: React skips it, or waits for it to finish.
pub type RunningViewTransition = ViewTransition;

fn merge_translate(translate_a: Option<&str>, translate_b: Option<&str>) -> String {
    let translate_a = match translate_a {
        Some(translate_a) if !translate_a.is_empty() && translate_a != "none" => translate_a,
        _ => return translate_b.unwrap_or_default().to_owned(),
    };
    let translate_b = match translate_b {
        Some(translate_b) if !translate_b.is_empty() && translate_b != "none" => translate_b,
        // $FlowFixMe[constant-condition]
        _ => return translate_a.to_owned(),
    };
    let parts_a: Vec<&str> = translate_a.split(' ').collect();
    let parts_b: Vec<&str> = translate_b.split(' ').collect();
    let mut i = 0;
    let mut result = String::new();
    while i < parts_a.len() && i < parts_b.len() {
        if i > 0 {
            result += " ";
        }
        result += &("calc(".to_owned() + parts_a[i] + " + " + parts_b[i] + ")");
        i += 1;
    }
    while i < parts_a.len() {
        result += &(" ".to_owned() + parts_a[i]);
        i += 1;
    }
    while i < parts_b.len() {
        result += &(" ".to_owned() + parts_b[i]);
        i += 1;
    }
    result
}

fn animate_gesture(
    keyframes: Vec<&'static JsObject>,
    target_element: &Element,
    pseudo_element: &str,
    timeline: &'static GestureTimeline,
    view_transition_animations: &RefCell<Vec<&'static Animation>>,
    custom_timeline_cleanup: &RefCell<Vec<&'static dyn Fn()>>,
    range_start: f64,
    range_end: f64,
    move_first_frame_into_viewport: bool,
    move_all_frames_into_viewport: bool,
) {
    let mut width: Option<&'static Unknown> = None;
    let mut height: Option<&'static Unknown> = None;
    let mut unchanged_dimensions = true;
    for &keyframe in &keyframes {
        // Delete any easing since we always apply linear easing to gestures.
        js::delete(keyframe, "easing");
        js::delete(keyframe, "computedOffset");
        let w = js::get(keyframe, "width");
        if width.is_none() {
            width = w;
        } else if !js::strict_equals(width, w) {
            unchanged_dimensions = false;
        }
        let h = js::get(keyframe, "height");
        if height.is_none() {
            height = h;
        } else if !js::strict_equals(height, h) {
            unchanged_dimensions = false;
        }
        // Chrome returns "auto" for width/height which is not a valid value to
        // animate to. Similarly, transform: "none" is actually lack of transform.
        if js::strict_equals(js::get(keyframe, "width"), "auto") {
            js::delete(keyframe, "width");
        }
        if js::strict_equals(js::get(keyframe, "height"), "auto") {
            js::delete(keyframe, "height");
        }
        if js::strict_equals(js::get(keyframe, "transform"), "none") {
            js::delete(keyframe, "transform");
        }
        if move_all_frames_into_viewport && js::get(keyframe, "transform").is_none() {
            // If a transform is not explicitly specified to override the auto
            // generated one on the pseudo element, then we need to adjust it to
            // put it back into the viewport. We don't know the offset relative to
            // the screen so instead we use the translate prop to do a relative
            // adjustment.
            // TODO: If the "transform" was manually overridden on the pseudo
            // element itself and no longer the auto generated one, then we shouldn't
            // adjust it. I'm not sure how to detect this.
            let translate: Option<String> = unsafe { js::cast(js::get(keyframe, "translate")) };
            match translate {
                Some(translate) if !translate.is_empty() => {
                    js::set(
                        keyframe,
                        "translate",
                        merge_translate(Some(&translate), Some("20000px 20000px")),
                    );
                }
                _ => {
                    // TODO: If there's a CSS rule targeting translate on the pseudo element
                    // already we need to merge it.
                    let element_translate: String =
                        get_computed_style(target_element, Some(pseudo_element)).translate();
                    js::set(
                        keyframe,
                        "translate",
                        merge_translate(Some(&element_translate), Some("20000px 20000px")),
                    );
                }
            }
        }
    }
    if move_first_frame_into_viewport {
        // If this is the generated animation that does a FLIP matrix translation
        // from the old position, we need to adjust it from the out of viewport
        // position. If this is going from old to new it only applies to first
        // keyframe. Otherwise it applies to every keyframe.
        move_old_frame_into_viewport(keyframes[0]);
    }
    if unchanged_dimensions
        && let Some(width) = width
        && let Some(height) = height
    {
        // Read the underlying width/height of the pseudo-element. The previous animation
        // should have already been cancelled so we should observe the underlying element.
        let computed_style = get_computed_style(target_element, Some(pseudo_element));
        if js::strict_equals(computed_style.width(), width)
            && js::strict_equals(computed_style.height(), height)
        {
            for &keyframe in &keyframes {
                js::delete(keyframe, "width");
                js::delete(keyframe, "height");
            }
        }
    }

    // TODO: Reverse the reverse if the original direction is reverse.
    let reverse = range_start > range_end;
    if js::instance_of(timeline, js::class::<AnimationTimeline>()) {
        // Native Timeline
        // $FlowFixMe[incompatible-type]
        let animation = target_element.animate(
            Some(&keyframes),
            Some(&KeyframeAnimationOptions {
                pseudo_element: Some(pseudo_element),
                // Set the timeline to the current gesture timeline to drive the updates.
                timeline: Some(unsafe { js::cast(Some(timeline)) }),
                // We reset all easing functions to linear so that it feels like you
                // have direct impact on the transition and to avoid double bouncing
                // from scroll bouncing.
                easing: Some("linear"),
                // We fill in both direction for overscroll.
                fill: Some("both"), // TODO: Should we preserve the fill instead?
                // We play all gestures in reverse, except if we're in reverse direction
                // in which case we need to play it in reverse of the reverse.
                direction: Some(if reverse { "normal" } else { "reverse" }),
                // Range start needs to be higher than range end. If it goes in reverse
                // we reverse the whole animation below.
                range_start: Some(
                    TimelineRangeOffsetOrCSSNumericValueOrCSSKeywordValueOrStr::Str(
                        &(js::concat_text(if reverse { range_end } else { range_start }) + "%"),
                    ),
                ),
                range_end: Some(
                    TimelineRangeOffsetOrCSSNumericValueOrCSSKeywordValueOrStr::Str(
                        &(js::concat_text(if reverse { range_start } else { range_end }) + "%"),
                    ),
                ),
                ..Default::default()
            }),
        );
        view_transition_animations.borrow_mut().push(animation);
    } else {
        // Custom Timeline
        // $FlowFixMe[incompatible-type]
        let animation = target_element.animate(
            Some(&keyframes),
            Some(&KeyframeAnimationOptions {
                pseudo_element: Some(pseudo_element),
                // We reset all easing functions to linear so that it feels like you
                // have direct impact on the transition and to avoid double bouncing
                // from scroll bouncing.
                easing: Some("linear"),
                // We fill in both direction for overscroll.
                fill: Some("both"), // TODO: Should we preserve the fill instead?
                // We play all gestures in reverse, except if we're in reverse direction
                // in which case we need to play it in reverse of the reverse.
                direction: Some(if reverse { "normal" } else { "reverse" }),
                // We set the delay and duration to represent the span of the range.
                delay: Some(if reverse { range_end } else { range_start }),
                duration: Some(NumberOrCSSNumericValueOrStr::Number(if reverse {
                    range_start - range_end
                } else {
                    range_end - range_start
                })),
                ..Default::default()
            }),
        );
        view_transition_animations.borrow_mut().push(animation);
        // Let the custom timeline take control of driving the animation.
        let timeline: &CustomTimeline = unsafe { js::cast(Some(timeline)) };
        if let Some(cleanup) = timeline.animate(animation) {
            custom_timeline_cleanup.borrow_mut().push(cleanup);
        }
    }
}

pub fn start_gesture_transition(
    _suspended_state: Option<&'static SuspendedState>,
    root_container: &'static Container,
    timeline: &'static GestureTimeline,
    range_start: f64,
    range_end: f64,
    transition_types: Option<Rc<RefCell<TransitionTypes>>>,
    mutation_callback: &'static dyn Fn(),
    animate_callback: &'static dyn Fn(),
    error_callback: &'static dyn Fn(Option<&'static Unknown>),
    finished_animation: &'static dyn Fn(), // Profiling-only
) -> Option<&'static RunningViewTransition> {
    let owner_document: &'static Document = if root_container.node_type() == DOCUMENT_NODE {
        webapi::document::unchecked_from(root_container)
    } else {
        root_container.owner_document().unwrap()
    };
    match catch_unwind(AssertUnwindSafe(|| {
        // Force layout before we start the Transition. This works around a bug in Safari
        // if one of the clones end up being a stylesheet that isn't loaded or uncached.
        // https://bugs.webkit.org/show_bug.cgi?id=290146
        force_layout(owner_document);
        let types: Option<Vec<String>> = transition_types
            .as_ref()
            .map(|types| types.borrow().clone());
        // $FlowFixMe[prop-missing]
        let transition = owner_document.start_view_transition(Some(&StartViewTransitionOptions {
            update: Some(&mutation_callback),
            types: types.as_deref(),
        }));
        // $FlowFixMe[prop-missing]
        js::set(owner_document, "__reactViewTransition", transition);
        let custom_timeline_cleanup: Rc<RefCell<Vec<&'static dyn Fn()>>> =
            Rc::new(RefCell::new(Vec::new())); // Cleanup Animations started in a CustomTimeline
        let view_transition_animations: Rc<RefCell<Vec<&'static Animation>>> =
            Rc::new(RefCell::new(Vec::new()));
        let ready_callback = {
            let custom_timeline_cleanup = Rc::clone(&custom_timeline_cleanup);
            let view_transition_animations = Rc::clone(&view_transition_animations);
            move || {
                let document_element: &Element = owner_document.document_element().unwrap();
                // Loop through all View Transition Animations.
                // $FlowFixMe[prop-missing]
                // $FlowFixMe[incompatible-type]
                let animations = document_element.get_animations(Some(GetAnimationsOptions {
                    subtree: Some(true),
                    ..Default::default()
                }));
                // First do a pass to collect all known group and new items so we can look
                // up if they exist later.
                let found_groups: &js::Set<str> = js::set::new(None);
                let found_news: &js::Set<str> = js::set::new(None);
                // Collect the longest duration of any view-transition animation including delay.
                let mut longest_duration = 0.0;
                for animation in &animations {
                    let effect = keyframe_effect::unchecked_from(animation.effect().unwrap());
                    // $FlowFixMe[prop-missing]
                    let pseudo_element: Option<String> = effect.pseudo_element();
                    match pseudo_element {
                        None => {}
                        Some(pseudo_element)
                            if pseudo_element.starts_with("::view-transition")
                                && effect.target() == Some(document_element) =>
                        {
                            let timing = effect.get_timing();
                            let duration =
                                // $FlowFixMe[prop-missing]
                                if let Some(NumberOrCSSNumericValueOrStr::Number(duration)) = timing.duration {
                                    duration
                                } else {
                                    0.0
                                };
                            // TODO: Consider interation count higher than 1.
                            // $FlowFixMe[prop-missing]
                            // $FlowFixMe[unsafe-addition]
                            let duration_with_delay = timing.delay.unwrap() + duration;
                            if duration_with_delay > longest_duration {
                                longest_duration = duration_with_delay;
                            }
                            if pseudo_element.starts_with("::view-transition-group") {
                                found_groups.add(&js::string::slice(&pseudo_element, 23.0, None));
                            } else if pseudo_element.starts_with("::view-transition-new") {
                                // TODO: This is not really a sufficient detection because if the new
                                // pseudo element might exist but have animations disabled on it.
                                found_news.add(&js::string::slice(&pseudo_element, 21.0, None));
                            }
                        }
                        Some(_) => {}
                    }
                }
                let duration_to_range_multipler = (range_end - range_start) / longest_duration;
                for anim in &animations {
                    if anim.play_state() != "running" {
                        continue;
                    }
                    let effect = keyframe_effect::unchecked_from(anim.effect().unwrap());
                    // $FlowFixMe[prop-missing]
                    let pseudo_element: Option<String> = effect.pseudo_element();
                    if let Some(pseudo_element) = pseudo_element
                        && pseudo_element.starts_with("::view-transition")
                        && effect.target() == Some(document_element)
                    {
                        // Ideally we could mutate the existing animation but unfortunately
                        // the mutable APIs seem less tested and therefore are lacking or buggy.
                        // Therefore we create a new animation instead.
                        anim.cancel();
                        let mut is_generated_group_anim = false;
                        let mut is_exit_group_anim = false;
                        if pseudo_element.starts_with("::view-transition-group") {
                            let group_name = &js::string::slice(&pseudo_element, 23.0, None);
                            if found_news.has(group_name) {
                                // If this has both "new" and "old" state we expect this to be an auto-generated
                                // animation that started outside the viewport. We need to adjust this first frame
                                // to be inside the viewport.
                                // $FlowFixMe[prop-missing]
                                let animation_name: Option<String> =
                                    unsafe { js::cast(js::get(anim, "animationName")) };
                                is_generated_group_anim =
                                    animation_name.is_some_and(|animation_name| {
                                        // $FlowFixMe[prop-missing]
                                        animation_name
                                            .starts_with("-ua-view-transition-group-anim-")
                                    });
                            } else {
                                // If this has only an "old" state then the pseudo element will be outside
                                // the viewport. If any keyframes don't override "transform" we need to
                                // adjust them.
                                is_exit_group_anim = true;
                            }
                            // TODO: If this has only an old state and no new state,
                        }
                        // Adjust the range based on how long the animation would've ran as time based.
                        // Since we're running animations in reverse from how they normally would run,
                        // therefore the timing is from the rangeEnd to the start.
                        let timing = effect.get_timing();
                        let duration =
                            // $FlowFixMe[prop-missing]
                            if let Some(NumberOrCSSNumericValueOrStr::Number(duration)) = timing.duration {
                                duration
                            } else {
                                0.0
                            };
                        let mut adjusted_range_start =
                            // $FlowFixMe[unsafe-addition]
                            // $FlowFixMe[prop-missing]
                            range_end - (duration + timing.delay.unwrap()) * duration_to_range_multipler;
                        let mut adjusted_range_end = range_end
                            // $FlowFixMe[prop-missing]
                            // $FlowFixMe[unsafe-arithmetic]
                            - timing.delay.unwrap() * duration_to_range_multipler;
                        if timing.direction.as_deref() == Some("reverse")
                            || timing.direction.as_deref() == Some("alternate-reverse")
                        {
                            // This animation was originally in reverse so we have to play it in flipped range.
                            std::mem::swap(&mut adjusted_range_start, &mut adjusted_range_end);
                        }
                        animate_gesture(
                            effect.get_keyframes(),
                            // $FlowFixMe[incompatible-type]: Always documentElement atm.
                            effect.target().unwrap(),
                            &pseudo_element,
                            timeline,
                            &view_transition_animations,
                            &custom_timeline_cleanup,
                            adjusted_range_start,
                            adjusted_range_end,
                            is_generated_group_anim,
                            is_exit_group_anim,
                        );
                        if pseudo_element.starts_with("::view-transition-old") {
                            let group_name = &js::string::slice(&pseudo_element, 21.0, None);
                            if !found_groups.has(group_name) && !found_news.has(group_name) {
                                found_groups.add(group_name);
                                // We haven't seen any group animation with this name. Since the old
                                // state was outside the viewport we need to put it back. Since we
                                // can't programmatically target the element itself, we use an
                                // animation to adjust it.
                                // This usually happens for exit animations where the element has
                                // the old position.
                                // If we also have a "new" state then we skip this because it means
                                // someone manually disabled the auto-generated animation. We need to
                                // treat the old state as having the position of the "new" state which
                                // will happen by default.
                                let pseudo_element_name =
                                    "::view-transition-group".to_owned() + group_name;
                                animate_gesture(
                                    vec![js::object::spread(None), js::object::spread(None)],
                                    // $FlowFixMe[incompatible-type]: Always documentElement atm.
                                    effect.target().unwrap(),
                                    &pseudo_element_name,
                                    timeline,
                                    &view_transition_animations,
                                    &custom_timeline_cleanup,
                                    range_start,
                                    range_end,
                                    false,
                                    true, // We let the helper apply the translate
                                );
                            }
                        }
                    }
                }
                // View Transitions with ScrollTimeline has a quirk where they end if the
                // ScrollTimeline ever reaches 100% but that doesn't mean we're done because
                // you can swipe back again. We can prevent this by adding a paused Animation
                // that never stops. This seems to keep all running Animations alive until
                // we explicitly abort (or something forces the View Transition to cancel).
                // $FlowFixMe[incompatible-call]
                // $FlowFixMe[incompatible-type]
                let blocking_anim = document_element.animate(
                    Some(&[EmptyKeyframe {}, EmptyKeyframe {}]),
                    Some(&KeyframeAnimationOptions {
                        pseudo_element: Some("::view-transition"),
                        duration: Some(NumberOrCSSNumericValueOrStr::Number(1.0)),
                        ..Default::default()
                    }),
                );
                blocking_anim.pause();
                view_transition_animations.borrow_mut().push(blocking_anim);
                animate_callback();
            }
        };
        // In Chrome, "new" animations are not ready in the ready callback. We have to wait
        // until requestAnimationFrame before we can observe them through getAnimations().
        // However, in Safari, that would cause a flicker because we're applying them late.
        // TODO: Think of a feature detection for this instead.
        let ready_callback: Rc<dyn Fn()> = Rc::new(ready_callback);
        let ready_for_animations: Box<dyn FnOnce(())> = if navigator.user_agent().contains("Chrome")
        {
            Box::new(move |()| {
                window_global::request_animation_frame(Box::new(move |_| ready_callback()));
            })
        } else {
            Box::new(move |()| ready_callback())
        };
        let handle_error = move |error: &'static JsError| {
            // $FlowFixMe[prop-missing]
            if js::strict_equals(js::get(owner_document, "__reactViewTransition"), transition) {
                // $FlowFixMe[prop-missing]
                js::set(
                    owner_document,
                    "__reactViewTransition",
                    js::null::<&Unknown>(),
                );
            }
            let _finish = FinishGestureTransition {
                mutation_callback,
                animate_callback,
                finished_animation,
            };
            let error = customize_view_transition_error(error, true);
            if error.is_some() {
                error_callback(error);
            }
        };
        transition
            .ready()
            .then_with::<(), _, _>(ready_for_animations, handle_error);
        transition.finished().finally(move || {
            let mut i = 0;
            while i < view_transition_animations.borrow().len() {
                // In Safari, we need to manually cancel all manually started animations
                // or it'll block or interfer with future transitions.
                // We can't use getAnimations() due to #35336 so we collect them in an array.
                let animation = view_transition_animations.borrow()[i];
                animation.cancel();
                i += 1;
            }
            let mut i = 0;
            while i < custom_timeline_cleanup.borrow().len() {
                let cleanup = custom_timeline_cleanup.borrow()[i];
                cleanup();
                i += 1;
            }
            // $FlowFixMe[prop-missing]
            if js::strict_equals(js::get(owner_document, "__reactViewTransition"), transition) {
                // $FlowFixMe[prop-missing]
                js::set(
                    owner_document,
                    "__reactViewTransition",
                    js::null::<&Unknown>(),
                );
            }
            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
                // Signal that the Transition was unable to continue. We do that here
                // instead of when we stop the running View Transition to ensure that
                // we cover cases when something else stops it early.
                finished_animation();
            }
        });
        transition
    })) {
        Ok(transition) => Some(transition),
        Err(_) => {
            // We use the error as feature detection.
            // The only thing that should throw is if startViewTransition is missing
            // or if it doesn't accept the object form. Other errors are async.
            // I.e. it's before the View Transitions v2 spec. We only support View
            // Transitions v2 otherwise we fallback to not animating to ensure that
            // we're not animating with the wrong animation mapped.
            // Run through the sequence to put state back into a consistent state.
            mutation_callback();
            animate_callback();
            if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
                finished_animation();
            }
            None
        }
    }
}

/// What a gesture's View Transition's error handler finishes, however the
/// error is reported.
struct FinishGestureTransition {
    mutation_callback: &'static dyn Fn(),
    animate_callback: &'static dyn Fn(),
    finished_animation: &'static dyn Fn(),
}

impl Drop for FinishGestureTransition {
    fn drop(&mut self) {
        // Continue the reset of the work.
        // If the error happened in the snapshot phase before the update callback
        // was invoked, then we need to first finish the mutation and layout phases.
        // If they're already invoked it's still safe to call them due the status check.
        (self.mutation_callback)();
        // Skip readyCallback() and go straight to animateCallbck() since we're not animating.
        // animateCallback() is still required to restore states.
        (self.animate_callback)();
        if ENABLE_PROFILER_TIMER.with(|&enabled| enabled) {
            (self.finished_animation)();
        }
    }
}

/// An empty keyframe, `{}`.
struct EmptyKeyframe {}

unsafe impl js::Defined for EmptyKeyframe {}

pub fn stop_view_transition(transition: &RunningViewTransition) {
    transition.skip_transition();
}

pub fn add_view_transition_finished_listener(
    transition: &RunningViewTransition,
    callback: &'static dyn Fn(),
) {
    transition.finished().finally(callback);
}

/// The pseudo-elements of one View Transition name, as one of
/// `::view-transition-group(name)` and its kin: animated, and asked for
/// its animations and style, as an element is.
#[cfg_attr(rust_js, rust_js::class)]
pub struct ViewTransitionPseudoElement {
    #[cfg_attr(rust_js, rust_js::name = "_scope")]
    scope: &'static Element,
    #[cfg_attr(rust_js, rust_js::name = "_selector")]
    selector: String,
}

unsafe impl js::Defined for ViewTransitionPseudoElement {}

/// A `duration` alone, the options a number of milliseconds is.
struct DurationOptions {
    duration: f64,
}

unsafe impl js::Defined for DurationOptions {}

#[cfg_attr(rust_js, rust_js::class)]
impl ViewTransitionPseudoElement {
    fn new(pseudo: &str, name: &str) -> Self {
        ViewTransitionPseudoElement {
            // TODO: Get the owner document from the root container.
            scope: document.document_element().unwrap(),
            selector: "::view-transition-".to_owned() + pseudo + "(" + name + ")",
        }
    }

    pub fn animate(
        &self,
        keyframes: Option<&'static Unknown>,
        options: Option<&'static Unknown>,
    ) -> &'static Animation {
        let opts = if let Kind::Number(options) = js::classify(options) {
            js::unknown_of(DurationOptions { duration: options })
        } else {
            js::object::assign(
                // $FlowFixMe[prop-missing]
                // $FlowFixMe[incompatible-type]
                js::unknown(dict::new::<&Unknown>()),
                (options,),
            )
        };
        js::set(opts, "pseudoElement", &self.selector);
        // TODO: Handle multiple child instances.
        self.scope.animate(
            keyframes.map(|keyframes| keyframes as &dyn Any),
            Some(unsafe { js::cast::<&KeyframeAnimationOptions>(Some(opts)) }),
        )
    }

    pub fn get_animations(
        &self,
        _options: Option<&GetAnimationsOptions>,
    ) -> Vec<&'static Animation> {
        let scope = self.scope;
        let selector = &self.selector;
        let animations = scope.get_animations(
            // $FlowFixMe[prop-missing]
            // $FlowFixMe[incompatible-type]
            Some(GetAnimationsOptions {
                subtree: Some(true),
                ..Default::default()
            }),
        );
        let mut result = Vec::new();
        for animation in animations {
            let effect = animation.effect().map(keyframe_effect::unchecked_from);
            // TODO: Handle multiple child instances.
            if let Some(effect) = effect
                && effect.target() == Some(scope)
                && effect.pseudo_element().as_ref() == Some(selector)
            {
                result.push(animation);
            }
        }
        result
    }

    pub fn get_computed_style(&self) -> &'static CSSStyleProperties {
        let scope = self.scope;
        let selector = &self.selector;
        get_computed_style(scope, Some(selector))
    }
}

/// The pseudo-elements of a View Transition name, each animated as an
/// element is.
pub struct ViewTransitionInstance {
    pub name: String,
    pub group: ViewTransitionPseudoElement,
    pub image_pair: ViewTransitionPseudoElement,
    pub old: ViewTransitionPseudoElement,
    pub new: ViewTransitionPseudoElement,
}

unsafe impl js::Defined for ViewTransitionInstance {}

pub fn create_view_transition_instance(name: &str) -> ViewTransitionInstance {
    ViewTransitionInstance {
        name: name.to_owned(),
        group: ViewTransitionPseudoElement::new("group", name),
        image_pair: ViewTransitionPseudoElement::new("image-pair", name),
        old: ViewTransitionPseudoElement::new("old", name),
        new: ViewTransitionPseudoElement::new("new", name),
    }
}

/// A timeline a program drives a gesture by, as upstream's `interface
/// CustomTimeline`: its current time, and what plays an Animation on it.
pub struct CustomTimeline(PhantomData<js::JsObject>);

impl CustomTimeline {
    #[cfg_attr(rust_js, rust_js::link_name = "get currentTime")]
    pub fn current_time(&self) -> f64 {
        unreachable!()
    }

    /// What the program plays `animation` by; what it gives cleans up after.
    #[cfg_attr(rust_js, rust_js::link_name = "animate")]
    pub fn animate(&self, animation: &Animation) -> Option<&'static dyn Fn()> {
        let _ = animation;
        unreachable!()
    }
}

/// A gesture's timeline: the host's `AnimationTimeline`, or a program's
/// `CustomTimeline`.
pub type GestureTimeline = Unknown;

pub fn get_current_gesture_offset(timeline: &'static GestureTimeline) -> f64 {
    // A CustomTimeline's currentTime is a number, as a CSSNumberish is.
    let timeline: &AnimationTimeline = unsafe { js::cast(Some(timeline)) };
    let Some(time) = timeline.current_time() else {
        panic!("Cannot start a gesture with a disconnected AnimationTimeline.");
    };
    match time {
        CSSNumberish::Number(time) => time,
        CSSNumberish::CSSNumericValue(time) => css_unit_value::unchecked_from(time).value(),
    }
}
/// `boolean | AddEventListenerOptions`, a listener's options as a program
/// gave them.
pub type EventListenerOptionsOrUseCapture = AddEventListenerOptionsOrBool<'static>;

/// A listener a Fragment holds, attached to each of its host children.
pub struct StoredEventListener {
    pub r#type: String,
    pub listener: EventListenerOrEventListenerObject,
    pub options_or_use_capture: Option<EventListenerOptionsOrUseCapture>,
    // When once:true, a wrapper that removes the fragment listener after the
    // first fire. Otherwise the same as listener.
    pub attached_listener: EventListenerOrEventListenerObject,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub cleanup: Option<Box<dyn Fn()>>,
}

unsafe impl js::Defined for StoredEventListener {}

/// A Fragment's listeners: one array, which its methods read and change
/// while others hold it.
pub type EventListeners = Rc<RefCell<Vec<Rc<StoredEventListener>>>>;

/// A Fragment's ref: the host children it holds, as one element-like
/// handle that is focused, observed and measured as they are.
#[cfg_attr(rust_js, rust_js::class)]
pub struct FragmentInstance {
    #[cfg_attr(rust_js, rust_js::name = "_fragmentFiber")]
    pub fragment_fiber: RefCell<Rc<Fiber>>,
    #[cfg_attr(rust_js, rust_js::name = "_eventListeners", rust_js::nullable)]
    pub event_listeners: RefCell<Option<EventListeners>>,
    #[cfg_attr(rust_js, rust_js::name = "_observers", rust_js::nullable)]
    pub observers: Cell<Option<&'static js::Set<Observer>>>,
}

unsafe impl js::Defined for FragmentInstance {}

/// An IntersectionObserver or a ResizeObserver, as a fragment observes its
/// children with either.
#[cfg_attr(rust_js, rust_js::untagged)]
#[derive(Clone, Copy)]
pub enum Observer {
    Intersection(&'static IntersectionObserver),
    Resize(&'static ResizeObserver),
}

unsafe impl js::Defined for Observer {}

#[cfg_attr(rust_js, rust_js::class)]
impl FragmentInstance {
    fn new(fragment_fiber: Rc<Fiber>) -> Self {
        FragmentInstance {
            fragment_fiber: RefCell::new(fragment_fiber),
            event_listeners: RefCell::new(js::null()),
            observers: Cell::new(js::null()),
        }
    }

    pub fn add_event_listener(
        &'static self,
        r#type: &str,
        listener: EventListenerOrEventListenerObject,
        options_or_use_capture: Option<EventListenerOptionsOrUseCapture>,
    ) {
        let mut signal: Option<&'static AbortSignal> = js::null();
        let mut cleanup: Option<Box<dyn Fn()>> = js::null();
        if let Some(AddEventListenerOptionsOrBool::AddEventListenerOptions(options)) =
            &options_or_use_capture
        {
            signal = options.signal;
            if let Some(signal) = signal
                && signal.aborted()
            {
                return;
            }
        }

        let listeners = Rc::clone(
            self.event_listeners
                .borrow_mut()
                .get_or_insert_with(Default::default),
        );
        // Element.addEventListener will only apply uniquely new event listeners by default. Since we
        // need to collect the listeners to apply to appended children, we track them ourselves and use
        // custom equality check for the options.
        let is_new_event_listener = index_of_event_listener(
            &listeners.borrow(),
            r#type,
            listener,
            options_or_use_capture.as_ref(),
        )
        .is_none();
        if is_new_event_listener {
            let fragment_instance = self;
            let mut attached_listener = listener;
            if is_once_option(options_or_use_capture.as_ref()) {
                // once is fragment-scoped: the first fire on any child removes this
                // listener from the fragment and every host child.
                let r#type = r#type.to_owned();
                attached_listener = EventListenerOrEventListenerObject::EventListener(
                    function::with_this(move |this, (event,): (&'static Event,)| {
                        fragment_instance.remove_event_listener(
                            &r#type,
                            listener,
                            options_or_use_capture,
                        );
                        match listener {
                            EventListenerOrEventListenerObject::EventListener(listener) => {
                                function::call(
                                    listener,
                                    this.map(|this| this as &dyn js::IntoUnknown),
                                    (event,),
                                );
                            }
                            EventListenerOrEventListenerObject::EventListenerObject(listener) => {
                                listener.handle_event(event);
                            }
                        }
                    }),
                );
            }
            if let Some(signal) = signal {
                let r#type = r#type.to_owned();
                let on_abort: &'static dyn Fn(&Event) = webapi::listener(move |_: &Event| {
                    fragment_instance.remove_event_listener(
                        &r#type,
                        listener,
                        options_or_use_capture,
                    );
                });
                signal.add_event_listener_named(
                    "abort",
                    on_abort,
                    Some(&AddEventListenerOptions {
                        once: Some(true),
                        ..Default::default()
                    }),
                );
                // $FlowFixMe[method-unbinding]
                cleanup = Some(Box::new(move || {
                    signal.remove_event_listener_named("abort", on_abort, None);
                }));
            }
            let attach_options = get_attach_options(options_or_use_capture);
            listeners.borrow_mut().push(Rc::new(StoredEventListener {
                r#type: r#type.to_owned(),
                listener,
                options_or_use_capture,
                attached_listener,
                cleanup,
            }));
            let fragment_fiber = Rc::clone(&self.fragment_fiber.borrow());
            traverse_fragment_instances_and_text_instances(
                &fragment_fiber,
                &add_event_listener_to_child,
                &r#type,
                &attached_listener,
                &attach_options,
            );
        }
        self.event_listeners.replace(Some(listeners));
    }

    pub fn remove_event_listener(
        &self,
        r#type: &str,
        listener: EventListenerOrEventListenerObject,
        options_or_use_capture: Option<EventListenerOptionsOrUseCapture>,
    ) {
        let Some(listeners) = self.event_listeners.borrow().clone() else {
            return;
        };
        let Some(index) = index_of_event_listener(
            &listeners.borrow(),
            r#type,
            listener,
            options_or_use_capture.as_ref(),
        ) else {
            return;
        };
        let stored = Rc::clone(&listeners.borrow()[index]);
        let StoredEventListener {
            attached_listener,
            options_or_use_capture: stored_options,
            cleanup,
            ..
        } = &*stored;
        let attach_options = get_attach_options(*stored_options);
        let fragment_fiber = Rc::clone(&self.fragment_fiber.borrow());
        traverse_fragment_instances_and_text_instances(
            &fragment_fiber,
            &remove_event_listener_from_child,
            &r#type,
            attached_listener,
            &attach_options,
        );
        listeners.borrow_mut().remove(index);
        if let Some(cleanup) = cleanup {
            cleanup();
        }
    }

    pub fn dispatch_event(&self, event: &'static Event) -> bool {
        let fragment_fiber = Rc::clone(&self.fragment_fiber.borrow());
        let Some(parent_host_fiber) =
            get_fragment_parent_instance_or_container_fiber(&fragment_fiber)
        else {
            return true;
        };
        let parent_host_instance: &Node =
            unsafe { js::cast(Some(get_instance_from_host_fiber(&parent_host_fiber))) };
        let event_listeners = self.event_listeners.borrow().clone();
        if event_listeners
            .as_ref()
            .is_some_and(|event_listeners| !event_listeners.borrow().is_empty())
            || !event.bubbles()
        {
            // The temporary node stands in for the fragment's position so that its own
            // listeners fire before the event propagates to the parent. A Document can
            // only hold comments and processing instructions alongside its
            // documentElement, so a Text node would be an invalid child there.
            let temp: &Node = if parent_host_instance.node_type() == DOCUMENT_NODE {
                webapi::document::unchecked_from(parent_host_instance).create_comment("")
            } else {
                document.create_text_node("")
            };
            if let Some(event_listeners) = &event_listeners {
                let mut i = 0;
                while i < event_listeners.borrow().len() {
                    let stored = Rc::clone(&event_listeners.borrow()[i]);
                    let StoredEventListener {
                        r#type,
                        attached_listener,
                        options_or_use_capture,
                        ..
                    } = &*stored;
                    temp.add_event_listener_named(
                        r#type,
                        *attached_listener,
                        get_attach_options(*options_or_use_capture)
                            .as_ref()
                            .map(|options| options as &dyn IntoAddEventListenerOptionsOrBool),
                    );
                    i += 1;
                }
            }
            parent_host_instance.append_child(temp);
            let cancelable = temp.dispatch_event(event);
            if let Some(event_listeners) = &event_listeners {
                let mut i = 0;
                while i < event_listeners.borrow().len() {
                    let stored = Rc::clone(&event_listeners.borrow()[i]);
                    let StoredEventListener {
                        r#type,
                        attached_listener,
                        options_or_use_capture,
                        ..
                    } = &*stored;
                    temp.remove_event_listener_named(
                        r#type,
                        *attached_listener,
                        get_attach_options(*options_or_use_capture)
                            .as_ref()
                            .map(|options| options as &dyn IntoEventListenerOptionsOrBool),
                    );
                    i += 1;
                }
            }
            parent_host_instance.remove_child(temp);
            cancelable
        } else {
            parent_host_instance.dispatch_event(event)
        }
    }

    pub fn focus(&self, focus_options: Option<&'static FocusOptions>) {
        let fragment_fiber = Rc::clone(&self.fragment_fiber.borrow());
        traverse_fragment_instances_and_text_instances_deeply(
            &fragment_fiber,
            &set_focus_on_fiber_if_focusable,
            &focus_options,
            &(),
            &(),
        );
    }

    pub fn focus_last(&self, focus_options: Option<&'static FocusOptions>) {
        let children: RefCell<Vec<Rc<Fiber>>> = RefCell::new(Vec::new());
        let fragment_fiber = Rc::clone(&self.fragment_fiber.borrow());
        traverse_fragment_instances_and_text_instances_deeply(
            &fragment_fiber,
            &collect_children,
            &children,
            &(),
            &(),
        );
        for child in children.borrow().iter().rev() {
            if set_focus_on_fiber_if_focusable(child, &focus_options, &(), &()) {
                break;
            }
        }
    }

    pub fn blur(&self) {
        let fragment_fiber = Rc::clone(&self.fragment_fiber.borrow());
        let Some(parent_host_fiber) =
            get_fragment_parent_instance_or_container_fiber(&fragment_fiber)
        else {
            return;
        };
        let parent_instance_or_container: &Container =
            unsafe { js::cast(Some(get_instance_from_host_fiber(&parent_host_fiber))) };
        // Instance is included in the Container type for DOM.
        let owner_document = get_owner_document_from_root_container(parent_instance_or_container);
        let Some(active_element) = owner_document.active_element() else {
            return;
        };
        traverse_fragment_instances_and_text_instances(
            &fragment_fiber,
            &blur_active_element_within_fragment,
            &active_element,
            &(),
            &(),
        );
    }

    pub fn observe_using(&'static self, observer: Observer) {
        let fragment_fiber = Rc::clone(&self.fragment_fiber.borrow());
        if __DEV__ {
            let has_text = Cell::new(false);
            let has_element = Cell::new(false);
            traverse_fragment_instances_and_text_instances(
                &fragment_fiber,
                &|child: &Rc<Fiber>, _: &(), _: &(), _: &()| {
                    if child.tag.get() == HOST_TEXT {
                        has_text.set(true);
                    } else {
                        // Stop traversal, found element
                        has_element.set(true);
                        return true;
                    }
                    false
                },
                &(),
                &(),
                &(),
            );
            if has_text.get() && !has_element.get() {
                console::error((concat!(
                    "observeUsing() was called on a FragmentInstance with only text children. ",
                    "Observers do not work on text nodes."
                ),));
            }
        }
        if self.observers.get().is_none() {
            self.observers.set(Some(js::set::new(None)));
        }
        self.observers.get().unwrap().add(&observer);
        traverse_fragment_instances_and_text_instances(
            &fragment_fiber,
            &observe_child,
            &observer,
            &(),
            &(),
        );
    }

    pub fn unobserve_using(&'static self, observer: Observer) {
        let fragment_fiber = Rc::clone(&self.fragment_fiber.borrow());
        match self.observers.get() {
            Some(observers) if observers.has(&observer) => {
                observers.delete(&observer);
                traverse_fragment_instances_and_text_instances(
                    &fragment_fiber,
                    &unobserve_child,
                    &observer,
                    &(),
                    &(),
                );
                unobserve_pending_children(self, observer);
            }
            _ => {
                if __DEV__ {
                    console::error((concat!(
                        "You are calling unobserveUsing() with an observer that is not being observed with this fragment ",
                        "instance. First attach the observer with observeUsing()"
                    ),));
                }
            }
        }
    }

    pub fn get_client_rects(&self) -> Vec<&'static DOMRect> {
        let rects: RefCell<Vec<&'static DOMRect>> = RefCell::new(Vec::new());
        let fragment_fiber = Rc::clone(&self.fragment_fiber.borrow());
        traverse_fragment_instances_and_text_instances(
            &fragment_fiber,
            &collect_client_rects,
            &rects,
            &(),
            &(),
        );
        rects.into_inner()
    }

    pub fn get_root_node(
        &'static self,
        get_root_node_options: Option<GetRootNodeOptions>,
    ) -> &'static Unknown {
        let fragment_fiber = Rc::clone(&self.fragment_fiber.borrow());
        let Some(parent_host_fiber) =
            get_fragment_parent_instance_or_container_fiber(&fragment_fiber)
        else {
            return js::unknown(self);
        };
        let parent_host_instance: &Node =
            unsafe { js::cast(Some(get_instance_from_host_fiber(&parent_host_fiber))) };
        // $FlowFixMe[incompatible-type] Flow expects Node
        let root_node = parent_host_instance.get_root_node(get_root_node_options);
        js::unknown(root_node)
    }

    pub fn compare_document_position(&self, other_node: &'static Instance) -> f64 {
        let fragment_fiber = Rc::clone(&self.fragment_fiber.borrow());
        let Some(parent_host_fiber) =
            get_fragment_parent_instance_or_container_fiber(&fragment_fiber)
        else {
            return node::DOCUMENT_POSITION_DISCONNECTED;
        };
        let children: RefCell<Vec<Rc<Fiber>>> = RefCell::new(Vec::new());
        traverse_fragment_instances_and_text_instances(
            &fragment_fiber,
            &collect_children,
            &children,
            &(),
            &(),
        );
        let children = children.into_inner();
        let parent_host_instance: &'static Node =
            unsafe { js::cast(Some(get_instance_from_host_fiber(&parent_host_fiber))) };

        if children.is_empty() {
            // Match non-empty CDP: when portaled, position against the portal
            // container rather than the React host parent.
            let mut empty_parent_host_instance = parent_host_instance;
            if fiber_is_portaled_into_host(&fragment_fiber)
                && let Some(portal_container) = get_fragment_portal_container_info(&fragment_fiber)
            {
                empty_parent_host_instance = unsafe { js::cast(Some(portal_container)) };
            }
            return compare_document_position_for_empty_fragment(
                &fragment_fiber,
                empty_parent_host_instance,
                other_node,
                &get_instance_from_host_fiber,
            );
        }

        let first_node: &Node =
            unsafe { js::cast(Some(get_instance_from_host_fiber(&children[0]))) };
        let last_node: &Node = unsafe {
            js::cast(Some(get_instance_from_host_fiber(
                &children[children.len() - 1],
            )))
        };

        // If the fragment has been portaled into another host instance, we need to
        // our best guess is to use the parent of the child instance, rather than
        // the fiber tree host parent.
        let parent_host_instance_from_dom: Option<&Node> =
            if fiber_is_portaled_into_host(&fragment_fiber) {
                first_node
                    .parent_element()
                    .map(|parent_element| -> &Node { parent_element })
            } else {
                Some(parent_host_instance)
            };

        let Some(parent_host_instance_from_dom) = parent_host_instance_from_dom else {
            return node::DOCUMENT_POSITION_DISCONNECTED;
        };

        // Check if first and last node are actually in the expected document position
        // before relying on them as source of truth for other contained nodes
        let first_node_is_contained = (parent_host_instance_from_dom
            .compare_document_position(first_node) as u32
            & node::DOCUMENT_POSITION_CONTAINED_BY as u32)
            != 0;
        let last_node_is_contained = (parent_host_instance_from_dom
            .compare_document_position(last_node) as u32
            & node::DOCUMENT_POSITION_CONTAINED_BY as u32)
            != 0;
        let first_result = first_node.compare_document_position(other_node) as u32;
        let last_result = last_node.compare_document_position(other_node) as u32;

        let other_node_is_first_or_last_child = (first_node_is_contained
            && js::strict_equals(first_node, other_node))
            || (last_node_is_contained && js::strict_equals(last_node, other_node));
        let other_node_is_first_or_last_child_disconnected = (!first_node_is_contained
            && js::strict_equals(first_node, other_node))
            || (!last_node_is_contained && js::strict_equals(last_node, other_node));
        let other_node_is_within_first_or_last_child =
            first_result & node::DOCUMENT_POSITION_CONTAINED_BY as u32 != 0
                || last_result & node::DOCUMENT_POSITION_CONTAINED_BY as u32 != 0;
        let other_node_is_between_first_and_last_children = first_node_is_contained
            && last_node_is_contained
            && first_result & node::DOCUMENT_POSITION_FOLLOWING as u32 != 0
            && last_result & node::DOCUMENT_POSITION_PRECEDING as u32 != 0;

        let result = if other_node_is_first_or_last_child
            || other_node_is_within_first_or_last_child
            || other_node_is_between_first_and_last_children
        {
            node::DOCUMENT_POSITION_CONTAINED_BY as u32
        } else if other_node_is_first_or_last_child_disconnected {
            // otherNode has been portaled into another container
            node::DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC as u32
        } else {
            first_result
        };

        if result & node::DOCUMENT_POSITION_DISCONNECTED as u32 != 0
            || result & node::DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC as u32 != 0
        {
            return f64::from(result);
        }

        // Now that we have the result from the DOM API, we double check it matches
        // the state of the React tree. If it doesn't, we have a case of portaled or
        // otherwise injected elements and we return DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC.
        let document_position_matches_fiber_position = validate_document_position_with_fiber_tree(
            result,
            &fragment_fiber,
            &children[0],
            &children[children.len() - 1],
            other_node,
        );
        if document_position_matches_fiber_position {
            return f64::from(result);
        }
        node::DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC
    }

    pub fn scroll_into_view(&self, align_to_top: Option<&'static Unknown>) {
        if matches!(
            js::classify(align_to_top),
            Kind::Object(_) | Kind::Array(_) | Kind::Null
        ) {
            panic!(concat!(
                "FragmentInstance.scrollIntoView() does not support ",
                "scrollIntoViewOptions. Use the alignToTop boolean instead."
            ));
        }
        // First, get the children nodes
        let children: RefCell<Vec<Rc<Fiber>>> = RefCell::new(Vec::new());
        let fragment_fiber = Rc::clone(&self.fragment_fiber.borrow());
        traverse_fragment_instances_and_text_instances(
            &fragment_fiber,
            &collect_children,
            &children,
            &(),
            &(),
        );
        let children = children.into_inner();

        let resolved_align_to_top = !js::strict_equals(align_to_top, false);

        // `element.scrollIntoView(alignToTop)` of what JS gave, which WebIDL makes a boolean.
        let align: Option<bool> = align_to_top.map(|align_to_top| js::truthy(Some(align_to_top)));

        // If there are no children, we can use the parent and siblings to determine a position
        if children.is_empty() {
            let host_siblings = get_fragment_instance_or_text_instance_siblings(&fragment_fiber);
            let target_fiber = if resolved_align_to_top {
                host_siblings[1]
                    .clone()
                    .or_else(|| host_siblings[0].clone())
                    .or_else(|| get_fragment_parent_instance_or_container_fiber(&fragment_fiber))
            } else {
                host_siblings[0]
                    .clone()
                    .or_else(|| host_siblings[1].clone())
            };

            let Some(target_fiber) = target_fiber else {
                return;
            };
            // For text node siblings, use Range API to scroll to their position
            if target_fiber.tag.get() == HOST_TEXT {
                let text_node: &TextInstance =
                    unsafe { js::cast(Some(get_instance_from_host_fiber(&target_fiber))) };
                scroll_text_node_into_view(text_node, resolved_align_to_top);
                return;
            }
            let target: &Node =
                unsafe { js::cast(Some(get_instance_from_host_fiber(&target_fiber))) };
            // If the parent host fiber is a HostRoot, the target is a Container
            // which is not necessarily an Element with a scrollIntoView method.
            if target.node_type() == DOCUMENT_NODE {
                // A Document is always in view.
            } else if target.node_type() == DOCUMENT_FRAGMENT_NODE {
                let fragment = target;
                // ShadowRoot always has a host: https://dom.spec.whatwg.org/#ref-for-concept-documentfragment-host%E2%91%A5
                // A generic DocumentFragment doesn't implement this property but conceptually
                // host is a nullable Element: https://dom.spec.whatwg.org/#concept-documentfragment-host
                let host = if js::has(fragment, "host") {
                    Some(webapi::shadow_root::unchecked_from(fragment).host())
                } else {
                    None
                };
                if let Some(host) = host {
                    // The ShadowRoot's host element marks the position where the
                    // fragment's content would appear.
                    host.scroll_into_view(
                        align
                            .as_ref()
                            .map(|align| align as &dyn IntoBoolOrScrollIntoViewOptions),
                    );
                } else if __DEV__ {
                    console::warn((concat!(
                        "You are attempting to scroll a FragmentInstance that is only ",
                        "mounted inside a detached DocumentFragment. No scroll was ",
                        "performed."
                    ),));
                }
                return;
            } else {
                // Narrowed down to Element by nodeType check above, but Flow doesn't know that.
                let element: &Element = webapi::element::unchecked_from(target);
                element.scroll_into_view(
                    align
                        .as_ref()
                        .map(|align| align as &dyn IntoBoolOrScrollIntoViewOptions),
                );
            }
        }

        let mut i: isize = if resolved_align_to_top {
            children.len() as isize - 1
        } else {
            0
        };
        while i
            != (if resolved_align_to_top {
                -1
            } else {
                children.len() as isize
            })
        {
            let child = &children[i as usize];
            // For text nodes, use Range API to scroll to their position
            if child.tag.get() == HOST_TEXT {
                let text_node: &TextInstance =
                    unsafe { js::cast(Some(get_instance_from_host_fiber(child))) };
                scroll_text_node_into_view(text_node, resolved_align_to_top);
                i += if resolved_align_to_top { -1 } else { 1 };
                continue;
            }
            let instance: &Instance =
                unsafe { js::cast(Some(get_instance_from_host_fiber(child))) };
            instance.scroll_into_view(
                align
                    .as_ref()
                    .map(|align| align as &dyn IntoBoolOrScrollIntoViewOptions),
            );
            i += if resolved_align_to_top { -1 } else { 1 };
        }
    }
}

fn add_event_listener_to_child(
    child: &Rc<Fiber>,
    r#type: &&str,
    listener: &EventListenerOrEventListenerObject,
    options_or_use_capture: &Option<EventListenerOptionsOrUseCapture>,
) -> bool {
    let instance: &Node = unsafe { js::cast(Some(get_instance_from_host_fiber(child))) };
    instance.add_event_listener_named(
        r#type,
        *listener,
        options_or_use_capture
            .as_ref()
            .map(|options| options as &dyn IntoAddEventListenerOptionsOrBool),
    );
    false
}

fn remove_event_listener_from_child(
    child: &Rc<Fiber>,
    r#type: &&str,
    listener: &EventListenerOrEventListenerObject,
    options_or_use_capture: &Option<EventListenerOptionsOrUseCapture>,
) -> bool {
    let instance: &Node = unsafe { js::cast(Some(get_instance_from_host_fiber(child))) };
    instance.remove_event_listener_named(
        r#type,
        *listener,
        options_or_use_capture
            .as_ref()
            .map(|options| options as &dyn IntoEventListenerOptionsOrBool),
    );
    false
}

fn is_once_option(opts: Option<&EventListenerOptionsOrUseCapture>) -> bool {
    matches!(opts, Some(AddEventListenerOptionsOrBool::AddEventListenerOptions(opts)) if opts.once == Some(true))
}

fn get_attach_options(
    opts: Option<EventListenerOptionsOrUseCapture>,
) -> Option<EventListenerOptionsOrUseCapture> {
    // Strip once and signal when attaching to host children; Fragment owns once and signal semantics.
    match opts {
        None | Some(AddEventListenerOptionsOrBool::Bool(_)) => opts,
        Some(AddEventListenerOptionsOrBool::AddEventListenerOptions(options))
            if options.once != Some(true)
                && !js::instance_of(options.signal, js::class::<AbortSignal>()) =>
        {
            opts
        }
        Some(AddEventListenerOptionsOrBool::AddEventListenerOptions(opts)) => Some(
            AddEventListenerOptionsOrBool::AddEventListenerOptions(AddEventListenerOptions {
                capture: opts.capture,
                passive: opts.passive,
                ..Default::default()
            }),
        ),
    }
}

fn normalize_listener_options(opts: Option<&EventListenerOptionsOrUseCapture>) -> String {
    match opts {
        None => "c=0".to_owned(),
        Some(AddEventListenerOptionsOrBool::Bool(opts)) => {
            format!("c={}", if *opts { "1" } else { "0" })
        }
        Some(AddEventListenerOptionsOrBool::AddEventListenerOptions(opts)) => {
            format!("c={}", if opts.capture == Some(true) { "1" } else { "0" })
        }
    }
}

fn index_of_event_listener(
    event_listeners: &[Rc<StoredEventListener>],
    r#type: &str,
    listener: EventListenerOrEventListenerObject,
    options_or_use_capture: Option<&EventListenerOptionsOrUseCapture>,
) -> Option<usize> {
    if event_listeners.is_empty() {
        return None;
    }
    let normalized_options = normalize_listener_options(options_or_use_capture);
    event_listeners.iter().position(|item| {
        item.r#type == r#type
            && item.listener == listener
            && normalize_listener_options(item.options_or_use_capture.as_ref())
                == normalized_options
    })
}

fn set_focus_on_fiber_if_focusable(
    fiber: &Rc<Fiber>,
    focus_options: &Option<&'static FocusOptions>,
    _: &(),
    _: &(),
) -> bool {
    // Skip text nodes - they are not focusable
    if fiber.tag.get() == HOST_TEXT {
        return false;
    }
    let instance: &Instance = unsafe { js::cast(Some(get_instance_from_host_fiber(fiber))) };
    set_focus_if_focusable(instance, *focus_options)
}

fn collect_children(
    child: &Rc<Fiber>,
    collection: &RefCell<Vec<Rc<Fiber>>>,
    _: &(),
    _: &(),
) -> bool {
    collection.borrow_mut().push(Rc::clone(child));
    false
}

fn blur_active_element_within_fragment(
    child: &Rc<Fiber>,
    active_element: &&'static Element,
    _: &(),
    _: &(),
) -> bool {
    // Skip text nodes - they can't be focused
    if child.tag.get() == HOST_TEXT {
        return false;
    }
    let instance: &Instance = unsafe { js::cast(Some(get_instance_from_host_fiber(child))) };
    if instance == *active_element || instance.contains(Some(active_element)) {
        // $FlowFixMe[prop-missing]
        html_element::unchecked_from(active_element).blur();
        return true;
    }
    false
}

fn observe_child(child: &Rc<Fiber>, observer: &Observer, _: &(), _: &()) -> bool {
    // Skip text nodes - observers don't work on them
    if child.tag.get() == HOST_TEXT {
        return false;
    }
    let instance: &Instance = unsafe { js::cast(Some(get_instance_from_host_fiber(child))) };
    match observer {
        Observer::Intersection(observer) => observer.observe(instance),
        Observer::Resize(observer) => observer.observe(instance, None),
    }
    false
}

fn unobserve_child(child: &Rc<Fiber>, observer: &Observer, _: &(), _: &()) -> bool {
    // Skip text nodes - they were never observed
    if child.tag.get() == HOST_TEXT {
        return false;
    }
    let instance: &Instance = unsafe { js::cast(Some(get_instance_from_host_fiber(child))) };
    match observer {
        Observer::Intersection(observer) => observer.unobserve(instance),
        Observer::Resize(observer) => observer.unobserve(instance),
    }
    false
}

/// An IntersectionObserver's unobserve of a deleted child, held until after
/// paint.
struct PendingIntersectionUnobserve {
    fragment_instance: &'static FragmentInstance,
    observer: &'static IntersectionObserver,
    instance: &'static Instance,
}

unsafe impl js::Defined for PendingIntersectionUnobserve {}

thread_local! {
    static PENDING_INTERSECTION_UNOBSERVES: RefCell<Vec<PendingIntersectionUnobserve>> =
        const { RefCell::new(Vec::new()) };
    static INTERSECTION_UNOBSERVE_SCHEDULED: Cell<bool> = const { Cell::new(false) };
}

fn is_intersection_observer(observer: Observer) -> bool {
    // IntersectionObserver has rootMargin; ResizeObserver does not. Avoid
    // instanceof so jsdom mocks and cross-realm observers still match.
    matches!(
        js::classify(js::get(observer, "rootMargin")),
        Kind::String(_)
    )
}

// A later commit can reinsert the same node before the post-paint unobserve
// runs (Activity hidden → visible). Cancel so the flush does not drop it.
fn cancel_pending_intersection_unobserve(
    fragment_instance: &FragmentInstance,
    observer: Observer,
    instance: &Instance,
) {
    PENDING_INTERSECTION_UNOBSERVES.with_borrow_mut(|pending_intersection_unobserves| {
        pending_intersection_unobserves.retain(|pending| {
            !std::ptr::eq(pending.fragment_instance, fragment_instance)
                || js::unknown(pending.observer) != js::unknown(&observer)
                || pending.instance != instance
        });
    });
}

// unobserveUsing() only walks the children that are still mounted, so release
// the deleted ones that are waiting for their exit record.
fn unobserve_pending_children(fragment_instance: &FragmentInstance, observer: Observer) {
    PENDING_INTERSECTION_UNOBSERVES.with_borrow_mut(|pending_intersection_unobserves| {
        pending_intersection_unobserves.retain(|pending| {
            if std::ptr::eq(pending.fragment_instance, fragment_instance)
                && js::unknown(pending.observer) == js::unknown(&observer)
            {
                match observer {
                    Observer::Intersection(observer) => observer.unobserve(pending.instance),
                    Observer::Resize(observer) => observer.unobserve(pending.instance),
                }
                false
            } else {
                true
            }
        });
    });
}

fn schedule_pending_intersection_unobserve(
    fragment_instance: &'static FragmentInstance,
    observer: &'static IntersectionObserver,
    instance: &'static Instance,
) {
    PENDING_INTERSECTION_UNOBSERVES.with_borrow_mut(|pending_intersection_unobserves| {
        pending_intersection_unobserves.push(PendingIntersectionUnobserve {
            fragment_instance,
            observer,
            instance,
        });
    });
    if !INTERSECTION_UNOBSERVE_SCHEDULED.get() {
        INTERSECTION_UNOBSERVE_SCHEDULED.set(true);
        // Unobserving in this commit would cancel the record IntersectionObserver
        // delivers for the now disconnected target. Wait until after paint so the
        // exit fires first, then drop the observer's strong ref to the node.
        request_post_paint_callback(Box::new(|_| {
            INTERSECTION_UNOBSERVE_SCHEDULED.set(false);
            let pending = PENDING_INTERSECTION_UNOBSERVES.take();
            for item in &pending {
                item.observer.unobserve(item.instance);
            }
        }));
    }
}

fn collect_client_rects(
    child: &Rc<Fiber>,
    rects: &RefCell<Vec<&'static DOMRect>>,
    _: &(),
    _: &(),
) -> bool {
    if child.tag.get() == HOST_TEXT {
        let text_node: &Text = unsafe { js::cast(child.state_node.get()) };
        let range = text_node.owner_document().create_range();
        range.select_node_contents(text_node);
        // $FlowFixMe[method-unbinding]
        let client_rects = range.get_client_rects();
        rects
            .borrow_mut()
            .extend((0..client_rects.length() as usize).map(|i| &client_rects[i]));
    } else {
        let instance: &Instance = unsafe { js::cast(Some(get_instance_from_host_fiber(child))) };
        // $FlowFixMe[method-unbinding]
        let client_rects = instance.get_client_rects();
        rects
            .borrow_mut()
            .extend((0..client_rects.length() as usize).map(|i| &client_rects[i]));
    }
    false
}

fn validate_document_position_with_fiber_tree(
    document_position: u32,
    fragment_fiber: &Rc<Fiber>,
    preceding_boundary_fiber: &Rc<Fiber>,
    following_boundary_fiber: &Rc<Fiber>,
    other_node: &'static Instance,
) -> bool {
    let other_fiber = get_closest_instance_from_node(other_node);
    if document_position & node::DOCUMENT_POSITION_CONTAINED_BY as u32 != 0 {
        return other_fiber.is_some_and(|other_fiber| {
            is_fiber_contained_by_fragment(&other_fiber, fragment_fiber)
        });
    }
    if document_position & node::DOCUMENT_POSITION_CONTAINS as u32 != 0 {
        let Some(other_fiber) = other_fiber else {
            // otherFiber is null when otherNode is not part of a React tree. That
            // includes the document, documentElement and body, but also the root
            // container and any element above it. All of them contain the whole
            // React tree, so check containment of the root container rather than
            // enumerating the nodes above it.
            let root_container = get_fragment_root_container_info(fragment_fiber);
            return root_container.is_some_and(|root_container| {
                other_node.contains(Some(unsafe { js::cast::<&Node>(Some(root_container)) }))
            });
        };
        return is_fragment_contained_by_fiber(fragment_fiber, &other_fiber);
    }
    if document_position & node::DOCUMENT_POSITION_PRECEDING as u32 != 0 {
        return other_fiber.is_some_and(|other_fiber| {
            Rc::ptr_eq(&other_fiber, preceding_boundary_fiber)
                || is_fiber_preceding(preceding_boundary_fiber, &other_fiber)
        });
    }
    if document_position & node::DOCUMENT_POSITION_FOLLOWING as u32 != 0 {
        return other_fiber.is_some_and(|other_fiber| {
            Rc::ptr_eq(&other_fiber, following_boundary_fiber)
                || is_fiber_following(following_boundary_fiber, &other_fiber)
        });
    }

    false
}

fn scroll_text_node_into_view(text_node: &TextInstance, resolved_align_to_top: bool) {
    let range = text_node.owner_document().create_range();
    range.select_node_contents(text_node);
    let rect = range.get_bounding_client_rect();
    let scroll_y = if resolved_align_to_top {
        window.scroll_y() + rect.top()
    } else {
        window.scroll_y() + rect.bottom() - window.inner_height()
    };
    window.scroll_to_with_x_and_y(window.scroll_x() + rect.left(), scroll_y);
}

fn add_fragment_handle_to_fiber(
    child: &Rc<Fiber>,
    fragment_instance: &&'static FragmentInstance,
    _: &(),
    _: &(),
) -> bool {
    let instance = get_instance_from_host_fiber(child);
    add_fragment_handle_to_instance(instance, fragment_instance);
    false
}

fn add_fragment_handle_to_instance(
    instance: &'static Unknown,
    fragment_instance: &'static FragmentInstance,
) {
    if js::get(instance, "reactFragments").is_none() {
        js::set(
            instance,
            "reactFragments",
            js::set::new::<FragmentInstance>(None),
        );
    }
    let react_fragments: &js::Set<FragmentInstance> =
        unsafe { js::cast(js::get(instance, "reactFragments")) };
    react_fragments.add(fragment_instance);
}

pub fn create_fragment_instance(fragment_fiber: &Rc<Fiber>) -> &'static FragmentInstance {
    let fragment_instance: &'static FragmentInstance = unsafe {
        js::cast(Some(js::unknown_of(FragmentInstance::new(Rc::clone(
            fragment_fiber,
        )))))
    };
    traverse_fragment_instances_and_text_instances(
        fragment_fiber,
        &add_fragment_handle_to_fiber,
        &fragment_instance,
        &(),
        &(),
    );
    fragment_instance
}

pub fn update_fragment_instance_fiber(fragment_fiber: &Rc<Fiber>, instance: &FragmentInstance) {
    instance.fragment_fiber.replace(Rc::clone(fragment_fiber));
}

pub fn commit_new_child_to_fragment_instance(
    child_instance: &'static Node,
    fragment_instance: &'static FragmentInstance,
) {
    let event_listeners = fragment_instance.event_listeners.borrow().clone();
    if let Some(event_listeners) = &event_listeners {
        let mut i = 0;
        while i < event_listeners.borrow().len() {
            let stored = Rc::clone(&event_listeners.borrow()[i]);
            let StoredEventListener {
                r#type,
                attached_listener,
                options_or_use_capture,
                ..
            } = &*stored;
            child_instance.add_event_listener_named(
                r#type,
                *attached_listener,
                get_attach_options(*options_or_use_capture)
                    .as_ref()
                    .map(|options| options as &dyn IntoAddEventListenerOptionsOrBool),
            );
            i += 1;
        }
    }
    // Observers and fragment handles only apply to element children.
    if child_instance.node_type() == TEXT_NODE {
        return;
    }
    let instance: &'static Instance = webapi::element::unchecked_from(child_instance);
    if let Some(observers) = fragment_instance.observers.get() {
        observers.for_each(|observer| {
            cancel_pending_intersection_unobserve(fragment_instance, *observer, instance);
            match observer {
                Observer::Intersection(observer) => observer.observe(instance),
                Observer::Resize(observer) => observer.observe(instance, None),
            }
        });
    }
    add_fragment_handle_to_instance(js::unknown(instance), fragment_instance);
}

pub fn delete_child_from_fragment_instance(
    child_instance: &'static Node,
    fragment_instance: &'static FragmentInstance,
) {
    let event_listeners = fragment_instance.event_listeners.borrow().clone();
    if let Some(event_listeners) = &event_listeners {
        let mut i = 0;
        while i < event_listeners.borrow().len() {
            let stored = Rc::clone(&event_listeners.borrow()[i]);
            let StoredEventListener {
                r#type,
                attached_listener,
                options_or_use_capture,
                ..
            } = &*stored;
            child_instance.remove_event_listener_named(
                r#type,
                *attached_listener,
                get_attach_options(*options_or_use_capture)
                    .as_ref()
                    .map(|options| options as &dyn IntoEventListenerOptionsOrBool),
            );
            i += 1;
        }
    }
    if child_instance.node_type() == TEXT_NODE {
        return;
    }
    let instance: &'static Instance = webapi::element::unchecked_from(child_instance);
    if let Some(observers) = fragment_instance.observers.get() {
        observers.for_each(|observer| {
            if is_intersection_observer(*observer) {
                // Stay observed until the next IntersectionObserver delivery so a
                // disconnected target still gets an isIntersecting: false record.
                schedule_pending_intersection_unobserve(
                    fragment_instance,
                    unsafe { js::cast(Some(js::unknown(observer))) },
                    instance,
                );
            } else {
                match observer {
                    Observer::Intersection(observer) => observer.unobserve(instance),
                    Observer::Resize(observer) => observer.unobserve(instance),
                }
            }
        });
    }
    let react_fragments: Option<&js::Set<FragmentInstance>> =
        unsafe { js::cast(js::get(instance, "reactFragments")) };
    if let Some(react_fragments) = react_fragments {
        react_fragments.delete(fragment_instance);
    }
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "../shared/ReactDOMFragmentRefShared.js#compareDocumentPositionForEmptyFragment"
)]
fn compare_document_position_for_empty_fragment(
    fragment_fiber: &Fiber,
    parent_host_instance: &Node,
    other_node: &Node,
    get_public_instance: &dyn Fn(&Fiber) -> &'static Unknown,
) -> f64 {
    let _ = (
        fragment_fiber,
        parent_host_instance,
        other_node,
        get_public_instance,
    );
    unreachable!()
}

pub fn clear_container(container: &'static Container) {
    let node_type = container.node_type();
    if node_type == DOCUMENT_NODE {
        clear_container_sparingly(container);
    } else if node_type == ELEMENT_NODE {
        match container.node_name().as_str() {
            "HEAD" | "HTML" | "BODY" => {
                clear_container_sparingly(container);
            }
            _ => {
                container.set_text_content("");
            }
        }
    }
}

fn clear_container_sparingly(container: &Node) {
    let mut next_node: Option<&Node> = container.first_child();
    if let Some(first_child) = next_node
        && first_child.node_type() == DOCUMENT_TYPE_NODE
    {
        next_node = first_child.next_sibling();
    }
    while let Some(node) = next_node {
        next_node = node.next_sibling();
        match node.node_name().as_str() {
            "HTML" | "HEAD" | "BODY" => {
                let element: &Element = webapi::element::unchecked_from(node);
                clear_container_sparingly(element);
                // If these singleton instances had previously been rendered with React they
                // may still hold on to references to the previous fiber tree. We detatch them
                // prospectively to reset them to a baseline starting state since we cannot create
                // new instances.
                detach_deleted_instance(element);
                continue;
            }
            // Script tags are retained to avoid an edge case bug. Normally scripts will execute if they
            // are ever inserted into the DOM. However when streaming if a script tag is opened but not
            // yet closed some browsers create and insert the script DOM Node but the script cannot execute
            // yet until the closing tag is parsed. If something causes React to call clearContainer while
            // this DOM node is in the document but not yet executable the DOM node will be removed from the
            // document and when the script closing tag comes in the script will not end up running. This seems
            // to happen in Chrome/Firefox but not Safari at the moment though this is not necessarily specified
            // behavior so it could change in future versions of browsers. While leaving all scripts is broader
            // than strictly necessary this is the least amount of additional code to avoid this breaking
            // edge case.
            //
            // Style tags are retained because they may likely come from 3rd party scripts and extensions
            "SCRIPT" | "STYLE" => {
                continue;
            }
            // Stylesheet tags are retained because they may likely come from 3rd party scripts and extensions
            "LINK"
                if html_link_element::unchecked_from(node).rel().to_lowercase() == "stylesheet" =>
            {
                continue;
            }
            _ => {}
        }
        container.remove_child(node);
    }
}

fn clear_head(head: &Element) {
    let mut node = head.first_child();
    while let Some(current) = node {
        let next_node = current.next_sibling();
        let node_name = current.node_name();
        if is_marked_hoistable(current)
            || node_name == "SCRIPT"
            || node_name == "STYLE"
            || (node_name == "LINK"
                && html_link_element::unchecked_from(current)
                    .rel()
                    .to_lowercase()
                    == "stylesheet")
        {
            // retain these nodes
        } else {
            head.remove_child(current);
        }
        node = next_node;
    }
}

// Making this so we can eventually move all of the instance caching to the commit phase.
// Currently this is only used to associate fiber and props to instances for hydrating
// HostSingletons. The reason we need it here is we only want to make this binding on commit
// because only one fiber can own the instance at a time and render can fail/restart
pub fn bind_instance(
    instance: &'static Instance,
    props: &'static Props,
    internal_instance_handle: &Rc<Fiber>,
) {
    precache_fiber_node(internal_instance_handle, js::unknown(instance));
    update_fiber_props(instance, js::unknown(props));
}

// -------------------
//     Hydration
// -------------------

#[cfg_attr(rust_js, rust_js::name = "supportsHydration")]
pub const SUPPORTS_HYDRATION: bool = true;

/// What can be hydrated: an element, a text node, or the comment that
/// starts an Activity or Suspense boundary, or marks a form's state.
pub type HydratableInstance = Node;

/// The comment Fizz writes where a form's state is, matching or not.
pub type FormStateMarkerInstance = Comment;

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn can_hydrate_instance(
    mut instance: &'static HydratableInstance,
    r#type: &str,
    props: &'static Props,
    in_root_or_singleton: bool,
) -> Option<&'static Instance> {
    while instance.node_type() == ELEMENT_NODE {
        let element: &Element = webapi::element::unchecked_from(instance);
        let any_props = props;
        if element.node_name().to_lowercase() != r#type.to_lowercase() {
            if !in_root_or_singleton {
                // Usually we error for mismatched tags.
                if element.node_name() == "INPUT"
                    && js::strict_equals(js::get(element, "type"), "hidden")
                {
                    // If we have extra hidden inputs, we don't mismatch. This allows us to embed
                    // extra form data in the original form.
                } else {
                    return js::null();
                }
            }
            // In root or singleton parents we skip past mismatched instances.
        } else if !in_root_or_singleton {
            // Match
            if r#type == "input" && js::strict_equals(js::get(element, "type"), "hidden") {
                if __DEV__ {
                    check_attribute_string_coercion(js::get(any_props, "name"), "name");
                }
                let name =
                    js::get(any_props, "name").map(|name| js::concat_text(Some(name)).unwrap());
                if !js::strict_equals(js::get(any_props, "type"), "hidden")
                    || element.get_attribute("name") != name
                {
                    // Skip past hidden inputs unless that's what we're looking for. This allows us
                    // embed extra form data in the original form.
                } else {
                    return Some(element);
                }
            } else {
                return Some(element);
            }
        } else if is_marked_hoistable(element) {
            // We've already claimed this as a hoistable which isn't hydrated this way so we skip past it.
        } else {
            // We have an Element with the right type.

            // We are going to try to exclude it if we can definitely identify it as a hoisted Node or if
            // we can guess that the node is likely hoisted or was inserted by a 3rd party script or browser extension
            // using high entropy attributes for certain types. This technique will fail for strange insertions like
            // extension prepending <div> in the <body> but that already breaks before and that is an edge case.
            match r#type {
                // case 'title':
                //We assume all titles are matchable. You should only have one in the Document, at least in a hoistable scope
                // and if you are a HostComponent with type title we must either be in an <svg> context or this title must have an `itemProp` prop.
                "meta" => {
                    // The only way to opt out of hoisting meta tags is to give it an itemprop attribute. We assume there will be
                    // not 3rd party meta tags that are prepended, accepting the cases where this isn't true because meta tags
                    // are usually only functional for SSR so even in a rare case where we did bind to an injected tag the runtime
                    // implications are minimal
                    if element.has_attribute("itemprop") {
                        return Some(element);
                    }
                    // This is a Hoistable
                }
                "link" => {
                    // Links come in many forms and we do expect 3rd parties to inject them into <head> / <body>. We exclude known resources
                    // and then use high-entroy attributes like href which are almost always used and almost always unique to filter out unlikely
                    // matches.
                    let rel = element.get_attribute("rel");
                    if rel.as_deref() == Some("stylesheet")
                        && element.has_attribute("data-precedence")
                    {
                        // This is a stylesheet resource
                    } else if !js::strict_equals(rel, js::get(any_props, "rel"))
                        || !js::strict_equals(
                            element.get_attribute("href"),
                            if js::get(any_props, "href").is_none()
                                || js::strict_equals(js::get(any_props, "href"), "")
                            {
                                js::null()
                            } else {
                                js::get(any_props, "href")
                            },
                        )
                        || !js::strict_equals(
                            element.get_attribute("crossorigin"),
                            js::get(any_props, "crossOrigin").or(js::null()),
                        )
                        || !js::strict_equals(
                            element.get_attribute("title"),
                            js::get(any_props, "title").or(js::null()),
                        )
                    {
                        // rel + href should usually be enough to uniquely identify a link however crossOrigin can vary for rel preconnect
                        // and title could vary for rel alternate
                    } else {
                        return Some(element);
                    }
                }
                "style" => {
                    // Styles are hard to match correctly. We can exclude known resources but otherwise we accept the fact that a non-hoisted style tags
                    // in <head> or <body> are likely never going to be unmounted given their position in the document and the fact they likely hold global styles
                    if !element.has_attribute("data-precedence") {
                        return Some(element);
                    }
                    // This is a style resource
                }
                "script" => {
                    // Scripts are a little tricky, we exclude known resources and then similar to links try to use high-entropy attributes
                    // to reject poor matches. One challenge with scripts are inline scripts. We don't attempt to check text content which could
                    // in theory lead to a hydration error later if a 3rd party injected an inline script before the React rendered nodes.
                    // Falling back to client rendering if this happens should be seemless though so we will try this hueristic and revisit later
                    // if we learn it is problematic
                    let src_attr = element.get_attribute("src");
                    if (!js::strict_equals(
                        src_attr.as_deref(),
                        js::get(any_props, "src").or(js::null()),
                    ) || !js::strict_equals(
                        element.get_attribute("type"),
                        js::get(any_props, "type").or(js::null()),
                    ) || !js::strict_equals(
                        element.get_attribute("crossorigin"),
                        js::get(any_props, "crossOrigin").or(js::null()),
                    ))
                        // This script is for a different src/type/crossOrigin. It may be a script resource
                        // or it may just be a mistmatch
                        && src_attr.is_some_and(|src_attr| !src_attr.is_empty())
                        && element.has_attribute("async")
                        && !element.has_attribute("itemprop")
                    {
                        // This is an async script resource
                    } else {
                        return Some(element);
                    }
                }
                _ => {
                    // We have excluded the most likely cases of mismatch between hoistable tags, 3rd party script inserted tags,
                    // and browser extension inserted tags. While it is possible this is not the right match it is a decent hueristic
                    // that should work in the vast majority of cases.
                    return Some(element);
                }
            }
        }
        let Some(next_instance) = get_next_hydratable_sibling(element) else {
            break;
        };
        instance = next_instance;
    }
    // This is a suspense boundary or Text node or we got the end.
    // Suspense Boundaries are never expected to be injected by 3rd parties. If we see one it should be matched
    // and this is a hydration error.
    // Text Nodes are also not expected to be injected by 3rd parties. This is less of a guarantee for <body>
    // but it seems reasonable and conservative to reject this as a hydration error as well
    js::null()
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn can_hydrate_text_instance(
    mut instance: &'static HydratableInstance,
    text: &str,
    in_root_or_singleton: bool,
) -> Option<&'static TextInstance> {
    // Empty strings are not parsed by HTML so there won't be a correct match here.
    if text.is_empty() {
        return js::null();
    }

    while instance.node_type() != TEXT_NODE {
        if instance.node_type() == ELEMENT_NODE
            && instance.node_name() == "INPUT"
            && js::strict_equals(js::get(instance, "type"), "hidden")
        {
            // If we have extra hidden inputs, we don't mismatch. This allows us to
            // embed extra form data in the original form.
        } else if !in_root_or_singleton {
            return js::null();
        }
        let Some(next_instance) = get_next_hydratable_sibling(instance) else {
            return js::null();
        };
        instance = next_instance;
    }
    // This has now been refined to a text node.
    Some(webapi::text::unchecked_from(instance))
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn can_hydrate_hydration_boundary(
    mut instance: &'static HydratableInstance,
    in_root_or_singleton: bool,
) -> Option<&'static Comment> {
    while instance.node_type() != COMMENT_NODE {
        if instance.node_type() == ELEMENT_NODE
            && instance.node_name() == "INPUT"
            && js::strict_equals(js::get(instance, "type"), "hidden")
        {
            // If we have extra hidden inputs, we don't mismatch. This allows us to
            // embed extra form data in the original form.
        } else if !in_root_or_singleton {
            return js::null();
        }
        let Some(next_instance) = get_next_hydratable_sibling(instance) else {
            return js::null();
        };
        instance = next_instance;
    }
    // This has now been refined to a hydration boundary node.
    Some(webapi::comment::unchecked_from(instance))
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn can_hydrate_activity_instance(
    instance: &'static HydratableInstance,
    in_root_or_singleton: bool,
) -> Option<&'static ActivityInstance> {
    let hydratable_instance = can_hydrate_hydration_boundary(instance, in_root_or_singleton);
    if let Some(hydratable_instance) = hydratable_instance
        && hydratable_instance.data() == ACTIVITY_START_DATA
    {
        return Some(hydratable_instance);
    }
    js::null()
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn can_hydrate_suspense_instance(
    instance: &'static HydratableInstance,
    in_root_or_singleton: bool,
) -> Option<&'static SuspenseInstance> {
    let hydratable_instance = can_hydrate_hydration_boundary(instance, in_root_or_singleton);
    if let Some(hydratable_instance) = hydratable_instance
        && hydratable_instance.data() != ACTIVITY_START_DATA
    {
        return Some(hydratable_instance);
    }
    js::null()
}

pub fn is_suspense_instance_pending(instance: &SuspenseInstance) -> bool {
    instance.data() == SUSPENSE_PENDING_START_DATA || instance.data() == SUSPENSE_QUEUED_START_DATA
}

pub fn is_suspense_instance_fallback(instance: &SuspenseInstance) -> bool {
    instance.data() == SUSPENSE_FALLBACK_START_DATA
        || (instance.data() == SUSPENSE_PENDING_START_DATA
            && instance.owner_document().ready_state() != DOCUMENT_READY_STATE_LOADING)
}

/// What Fizz wrote of a boundary's error: its digest, and in DEV its
/// message and stacks.
pub struct SuspenseInstanceFallbackErrorDetails {
    pub digest: Option<&'static Unknown>,
    pub message: Option<&'static Unknown>,
    pub stack: Option<&'static Unknown>,
    pub component_stack: Option<&'static Unknown>,
}

unsafe impl js::Defined for SuspenseInstanceFallbackErrorDetails {}

pub fn get_suspense_instance_fallback_error_details(
    instance: &SuspenseInstance,
) -> SuspenseInstanceFallbackErrorDetails {
    let dataset = instance
        .next_sibling()
        .map(|next_sibling| html_element::unchecked_from(next_sibling).dataset());
    let (mut digest, mut message, mut stack, mut component_stack) = (None, None, None, None);
    if let Some(dataset) = dataset {
        digest = js::get(dataset, "dgst");
        if __DEV__ {
            message = js::get(dataset, "msg");
            stack = js::get(dataset, "stck");
            component_stack = js::get(dataset, "cstck");
        }
    }
    if __DEV__ {
        SuspenseInstanceFallbackErrorDetails {
            message,
            digest,
            stack,
            component_stack,
        }
    } else {
        // Object gets DCE'd if constructed in tail position and matches callsite destructuring
        SuspenseInstanceFallbackErrorDetails {
            digest,
            message: None,
            stack: None,
            component_stack: None,
        }
    }
}

pub fn register_suspense_instance_retry(
    instance: &'static SuspenseInstance,
    callback: &'static dyn Fn(),
) {
    let owner_document = instance.owner_document();
    if instance.data() == SUSPENSE_QUEUED_START_DATA {
        // The Fizz runtime has already queued this boundary for reveal. We wait for it
        // to be revealed and then retries.
        js::set(instance, "_reactRetry", callback);
    } else if
    // The Fizz runtime must have put this boundary into client render or complete
    // state after the render finished but before it committed. We need to call the
    // callback now rather than wait
    instance.data() != SUSPENSE_PENDING_START_DATA
        // The boundary is still in pending status but the document has finished loading
        // before we could register the event handler that would have scheduled the retry
        // on load so we call the callback now.
        || owner_document.ready_state() != DOCUMENT_READY_STATE_LOADING
    {
        callback();
    } else {
        // We're still in pending status and the document is still loading so we attach
        // a listener to the document load even and expose the retry on the instance for
        // the Fizz runtime to trigger if it ends up resolving this boundary
        let listener_cell = Rc::new(Cell::new(None));
        let listener = webapi::listener({
            let listener_cell = Rc::clone(&listener_cell);
            move |_: &Event| {
                callback();
                if let Some(listener) = listener_cell.get() {
                    owner_document.remove_event_listener_named("DOMContentLoaded", listener, None);
                }
            }
        });
        listener_cell.set(Some(listener));
        owner_document.add_event_listener_named("DOMContentLoaded", listener, None);
        js::set(instance, "_reactRetry", listener);
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn can_hydrate_form_state_marker(
    mut instance: &'static HydratableInstance,
    in_root_or_singleton: bool,
) -> Option<&'static FormStateMarkerInstance> {
    while instance.node_type() != COMMENT_NODE {
        if !in_root_or_singleton {
            return js::null();
        }
        let Some(next_instance) = get_next_hydratable_sibling(instance) else {
            return js::null();
        };
        instance = next_instance;
    }
    let node_data = webapi::comment::unchecked_from(instance).data();
    if node_data == FORM_STATE_IS_MATCHING || node_data == FORM_STATE_IS_NOT_MATCHING {
        let marker_instance: &FormStateMarkerInstance = webapi::comment::unchecked_from(instance);
        return Some(marker_instance);
    }
    js::null()
}

pub fn is_form_state_marker_matching(marker_instance: &FormStateMarkerInstance) -> bool {
    marker_instance.data() == FORM_STATE_IS_MATCHING
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn get_next_hydratable(mut node: Option<&'static Node>) -> Option<&'static HydratableInstance> {
    // Skip non-hydratable nodes.
    while let Some(current) = node {
        let node_type = current.node_type();
        if node_type == ELEMENT_NODE || node_type == TEXT_NODE {
            break;
        }
        if node_type == COMMENT_NODE {
            let data = webapi::comment::unchecked_from(current).data();
            if data == SUSPENSE_START_DATA
                || data == SUSPENSE_FALLBACK_START_DATA
                || data == SUSPENSE_PENDING_START_DATA
                || data == SUSPENSE_QUEUED_START_DATA
                || data == ACTIVITY_START_DATA
                || data == FORM_STATE_IS_MATCHING
                || data == FORM_STATE_IS_NOT_MATCHING
            {
                break;
            }
            if data == SUSPENSE_END_DATA || data == ACTIVITY_END_DATA {
                return js::null();
            }
        }
        node = current.next_sibling();
    }
    node
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_next_hydratable_sibling(
    instance: &HydratableInstance,
) -> Option<&'static HydratableInstance> {
    get_next_hydratable(instance.next_sibling())
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_first_hydratable_child(
    parent_instance: &Instance,
) -> Option<&'static HydratableInstance> {
    get_next_hydratable(parent_instance.first_child())
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_first_hydratable_child_within_container(
    parent_container: &'static Container,
) -> Option<&'static HydratableInstance> {
    let parent_element: &Element;
    match parent_container.node_type() {
        DOCUMENT_NODE => {
            parent_element = webapi::document::unchecked_from(parent_container)
                .body()
                .unwrap();
        }
        _ => {
            if parent_container.node_name() == "HTML" {
                parent_element = parent_container.owner_document().unwrap().body().unwrap();
            } else {
                parent_element = webapi::element::unchecked_from(parent_container);
            }
        }
    }
    get_next_hydratable(parent_element.first_child())
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_first_hydratable_child_within_activity_instance(
    parent_instance: &ActivityInstance,
) -> Option<&'static HydratableInstance> {
    get_next_hydratable(parent_instance.next_sibling())
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_first_hydratable_child_within_suspense_instance(
    parent_instance: &SuspenseInstance,
) -> Option<&'static HydratableInstance> {
    get_next_hydratable(parent_instance.next_sibling())
}

thread_local! {
    // If it were possible to have more than one scope singleton in a DOM tree
    // we would need to model this as a stack but since you can only have one <head>
    // and head is the only singleton that is a scope in DOM we can get away with
    // tracking this as a single value.
    #[cfg_attr(rust_js, rust_js::nullable)]
    static PREVIOUS_HYDRATABLE_ON_ENTERING_SCOPED_SINGLETON: Cell<Option<&'static HydratableInstance>> =
        const { Cell::new(None) };
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_first_hydratable_child_within_singleton(
    r#type: &str,
    singleton_instance: &Instance,
    current_hydratable_instance: Option<&'static HydratableInstance>,
) -> Option<&'static HydratableInstance> {
    if is_singleton_scope(r#type) {
        PREVIOUS_HYDRATABLE_ON_ENTERING_SCOPED_SINGLETON.set(current_hydratable_instance);
        get_next_hydratable(singleton_instance.first_child())
    } else {
        current_hydratable_instance
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_next_hydratable_sibling_after_singleton(
    r#type: &str,
    current_hydratable_instance: Option<&'static HydratableInstance>,
) -> Option<&'static HydratableInstance> {
    if is_singleton_scope(r#type) {
        let previous_hydratable_instance = PREVIOUS_HYDRATABLE_ON_ENTERING_SCOPED_SINGLETON.get();
        PREVIOUS_HYDRATABLE_ON_ENTERING_SCOPED_SINGLETON.set(js::null());
        previous_hydratable_instance
    } else {
        current_hydratable_instance
    }
}

/// A pseudo React element of a hydratable instance, its tag and props,
/// as a DEV warning prints it.
pub struct DescribedInstance {
    pub r#type: String,
    pub props: &'static Unknown,
}

unsafe impl js::Defined for DescribedInstance {}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn describe_hydratable_instance_for_dev_warnings(
    instance: &'static HydratableInstance,
) -> Option<&'static Unknown> {
    // Reverse engineer a pseudo react-element from hydratable instance
    if instance.node_type() == ELEMENT_NODE {
        // Reverse engineer a set of props that can print for dev warnings
        Some(js::unknown_of(DescribedInstance {
            r#type: instance.node_name().to_lowercase(),
            props: js::unknown(get_props_from_element(webapi::element::unchecked_from(
                instance,
            ))),
        }))
    } else if instance.node_type() == COMMENT_NODE {
        if webapi::comment::unchecked_from(instance).data() == ACTIVITY_START_DATA {
            return Some(js::unknown_of(DescribedInstance {
                r#type: "Activity".to_owned(),
                props: js::unknown(dict::new::<&Unknown>()),
            }));
        }
        Some(js::unknown_of(DescribedInstance {
            r#type: "Suspense".to_owned(),
            props: js::unknown(dict::new::<&Unknown>()),
        }))
    } else {
        instance.node_value().map(js::unknown_of)
    }
}

pub fn validate_hydratable_instance(
    r#type: &str,
    _props: &Props,
    host_context: &'static HostContext,
) -> bool {
    if __DEV__ {
        // TODO: take namespace into account when validating.
        let host_context_dev: &HostContextDev = unsafe { js::cast(Some(host_context)) };
        return validate_dom_nesting(r#type, host_context_dev.ancestor_info.as_ref());
    }
    true
}

pub fn hydrate_instance(
    instance: &'static Instance,
    r#type: &str,
    props: &'static Props,
    host_context: &'static HostContext,
    internal_instance_handle: &Rc<Fiber>,
) -> bool {
    precache_fiber_node(internal_instance_handle, js::unknown(instance));
    // TODO: Possibly defer this until the commit phase where all the events
    // get attached.
    update_fiber_props(instance, js::unknown(props));

    hydrate_properties(instance, r#type, js::unknown(props), host_context)
}

// Returns a Map of properties that were different on the server.
#[cfg_attr(rust_js, rust_js::nullable)]
pub fn diff_hydrated_props_for_dev_warnings(
    instance: &'static Instance,
    r#type: &str,
    props: &'static Props,
    host_context: &'static HostContext,
) -> Option<&'static Dict<Option<&'static Unknown>>> {
    diff_hydrated_properties(instance, r#type, js::unknown(props), host_context)
}

pub fn validate_hydratable_text_instance(text: &str, host_context: &'static HostContext) -> bool {
    if __DEV__ {
        let host_context_dev: &HostContextDev = unsafe { js::cast(Some(host_context)) };
        let ancestor = &host_context_dev.ancestor_info.as_ref().unwrap().current;
        if let Some(ancestor) = ancestor {
            return validate_text_nesting(
                text,
                &ancestor.tag,
                host_context_dev
                    .ancestor_info
                    .as_ref()
                    .unwrap()
                    .implicit_root_scope,
            );
        }
    }
    true
}

pub fn hydrate_text_instance(
    text_instance: &'static TextInstance,
    text: &str,
    internal_instance_handle: &Rc<Fiber>,
    parent_instance_props: Option<&'static Props>,
) -> bool {
    precache_fiber_node(internal_instance_handle, js::unknown(text_instance));

    hydrate_text(text_instance, text, parent_instance_props.map(js::unknown))
}

// Returns the server text if it differs from the client.
#[cfg_attr(rust_js, rust_js::nullable)]
pub fn diff_hydrated_text_for_dev_warnings(
    text_instance: &TextInstance,
    text: &str,
    parent_props: Option<&'static Props>,
) -> Option<String> {
    if parent_props.is_none_or(|parent_props| {
        !js::strict_equals(js::get(parent_props, SUPPRESS_HYDRATION_WARNING), true)
    }) {
        return diff_hydrated_text(text_instance, text);
    }
    js::null()
}

pub fn hydrate_activity_instance(
    activity_instance: &'static ActivityInstance,
    internal_instance_handle: &Rc<Fiber>,
) {
    precache_fiber_node(internal_instance_handle, js::unknown(activity_instance));
}

pub fn hydrate_suspense_instance(
    suspense_instance: &'static SuspenseInstance,
    internal_instance_handle: &Rc<Fiber>,
) {
    precache_fiber_node(internal_instance_handle, js::unknown(suspense_instance));
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn get_next_hydratable_instance_after_hydration_boundary(
    hydration_instance: &Comment,
) -> Option<&'static HydratableInstance> {
    let mut node = hydration_instance.next_sibling();
    // Skip past all nodes within this suspense boundary.
    // There might be nested nodes so we need to keep track of how
    // deep we are and only break out when we're back on top.
    let mut depth = 0;
    while let Some(current) = node {
        if current.node_type() == COMMENT_NODE {
            let data = webapi::comment::unchecked_from(current).data();
            if data == SUSPENSE_END_DATA || data == ACTIVITY_END_DATA {
                if depth == 0 {
                    return get_next_hydratable_sibling(current);
                } else {
                    depth -= 1;
                }
            } else if data == SUSPENSE_START_DATA
                || data == SUSPENSE_FALLBACK_START_DATA
                || data == SUSPENSE_PENDING_START_DATA
                || data == SUSPENSE_QUEUED_START_DATA
                || data == ACTIVITY_START_DATA
            {
                depth += 1;
            }
        }
        node = current.next_sibling();
    }
    // TODO: Warn, we didn't find the end comment boundary.
    js::null()
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_next_hydratable_instance_after_activity_instance(
    activity_instance: &ActivityInstance,
) -> Option<&'static HydratableInstance> {
    get_next_hydratable_instance_after_hydration_boundary(activity_instance)
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_next_hydratable_instance_after_suspense_instance(
    suspense_instance: &SuspenseInstance,
) -> Option<&'static HydratableInstance> {
    get_next_hydratable_instance_after_hydration_boundary(suspense_instance)
}

// Returns the SuspenseInstance if this node is a direct child of a
// SuspenseInstance. I.e. if its previous sibling is a Comment with
// SUSPENSE_x_START_DATA. Otherwise, null.
#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_parent_hydration_boundary(target_instance: &Node) -> Option<&'static Comment> {
    let mut node = target_instance.previous_sibling();
    // Skip past all nodes within this suspense boundary.
    // There might be nested nodes so we need to keep track of how
    // deep we are and only break out when we're back on top.
    let mut depth = 0;
    while let Some(current) = node {
        if current.node_type() == COMMENT_NODE {
            let data = webapi::comment::unchecked_from(current).data();
            if data == SUSPENSE_START_DATA
                || data == SUSPENSE_FALLBACK_START_DATA
                || data == SUSPENSE_PENDING_START_DATA
                || data == SUSPENSE_QUEUED_START_DATA
                || data == ACTIVITY_START_DATA
            {
                if depth == 0 {
                    return Some(webapi::comment::unchecked_from(current));
                } else {
                    depth -= 1;
                }
            } else if data == SUSPENSE_END_DATA || data == ACTIVITY_END_DATA {
                depth += 1;
            }
        }
        node = current.previous_sibling();
    }
    js::null()
}

pub fn commit_hydrated_container(container: &'static Container) {
    // Retry if any event replaying was blocked on this.
    retry_if_blocked_on(js::unknown(container));
}

pub fn commit_hydrated_activity_instance(activity_instance: &'static ActivityInstance) {
    // Retry if any event replaying was blocked on this.
    retry_if_blocked_on(js::unknown(activity_instance));
}

pub fn commit_hydrated_suspense_instance(suspense_instance: &'static SuspenseInstance) {
    // Retry if any event replaying was blocked on this.
    retry_if_blocked_on(js::unknown(suspense_instance));
}

pub fn flush_hydration_events() {
    if ENABLE_HYDRATION_CHANGE_EVENT
        .with(|enable_hydration_change_event| *enable_hydration_change_event)
    {
        flush_event_replaying();
    }
}

pub fn should_delete_unhydrated_tail_instances(parent_type: &str) -> bool {
    parent_type != "form" && parent_type != "button"
}

// -------------------
//     Test Selectors
// -------------------

#[cfg_attr(rust_js, rust_js::name = "supportsTestSelectors")]
pub const SUPPORTS_TEST_SELECTORS: bool = true;

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn find_fiber_root(node: &'static Instance) -> Option<Rc<Fiber>> {
    let mut stack = vec![node];
    let mut index = 0;
    while index < stack.len() {
        let current = stack[index];
        index += 1;
        if is_container_marked_as_root(js::unknown(current)) {
            return get_instance_from_node_dom_tree(current);
        }
        stack.extend(webapi::html_collection::iter(current.children()));
    }
    js::null()
}

pub fn get_bounding_rect(node: &Instance) -> BoundingRect {
    let rect = node.get_bounding_client_rect();
    BoundingRect {
        x: Cell::new(rect.left()),
        y: Cell::new(rect.top()),
        width: Cell::new(rect.width()),
        height: Cell::new(rect.height()),
    }
}

pub fn match_accessibility_role(node: &Instance, role: &str) -> bool {
    if has_role(node, role) {
        return true;
    }

    false
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_text_content(fiber: &Fiber) -> Option<String> {
    match fiber.tag.get() {
        HOST_HOISTABLE | HOST_SINGLETON | HOST_COMPONENT => {
            let mut text_content = String::new();
            let state_node: &Node = unsafe { js::cast(fiber.state_node.get()) };
            let child_nodes = state_node.child_nodes();
            for i in 0..child_nodes.length() as usize {
                let child_node = &child_nodes[i];
                if child_node.node_type() == TEXT_NODE {
                    text_content += &child_node.text_content().unwrap_or_default();
                }
            }
            Some(text_content)
        }
        HOST_TEXT => {
            let state_node: &Node = unsafe { js::cast(fiber.state_node.get()) };
            state_node.text_content()
        }
        _ => js::null(),
    }
}

pub fn is_hidden_subtree(fiber: &Fiber) -> bool {
    fiber.tag.get() == HOST_COMPONENT
        && js::strict_equals(js::get(fiber.memoized_props.get(), "hidden"), true)
}

/// What removes the document's capture listener for `focus`, however
/// setFocusIfFocusable's focus ends.
struct RemoveFocusListener {
    owner_document: &'static Document,
    handle_focus: &'static dyn Fn(&Event),
}

impl Drop for RemoveFocusListener {
    fn drop(&mut self) {
        self.owner_document
            .remove_event_listener_named("focus", self.handle_focus, Some(&true));
    }
}

pub fn set_focus_if_focusable(
    node: &'static Instance,
    focus_options: Option<&'static FocusOptions>,
) -> bool {
    // The logic for determining if an element is focusable is kind of complex,
    // and since we want to actually change focus anyway- we can just skip it.
    // Instead we'll just listen for a "focus" event to verify that focus was set.
    //
    // We could compare the node to document.activeElement after focus,
    // but this would not handle the case where application code managed focus to automatically blur.
    let element = html_element::unchecked_from(node);

    // If this element is already the active element, it's focusable and already
    // focused. Calling .focus() on it would be a no-op (no focus event fires),
    // so we short-circuit here.
    if element.owner_document().active_element() == Some(node) {
        return true;
    }

    let did_focus = Rc::new(Cell::new(false));
    let handle_focus = webapi::listener({
        let did_focus = Rc::clone(&did_focus);
        move |_: &Event| did_focus.set(true)
    });

    // Listen on the document in the capture phase so we detect focus even when
    // it lands on a different element than the one we called .focus() on. This
    // happens with <label> elements (focus delegates to the associated input)
    // and shadow hosts with delegatesFocus.
    element
        .owner_document()
        .add_event_listener_named("focus", handle_focus, Some(&true));
    let _remove_focus_listener = RemoveFocusListener {
        owner_document: element.owner_document(),
        handle_focus,
    };
    // $FlowFixMe[method-unbinding]
    function::call(
        js::or(
            js::method((element, HTMLElement::focus)),
            js::method((js::class::<HTMLElement>().prototype(), HTMLElement::focus)),
        ),
        Some(element),
        (focus_options,),
    );

    did_focus.get()
}

/// What setupIntersectionObserver gives: what stops the host observer, and
/// what changes what it observes.
pub struct IntersectionObserverHandle {
    pub disconnect: Box<dyn Fn()>,
    pub observe: Box<dyn Fn(&'static Instance)>,
    pub unobserve: Box<dyn Fn(&'static Instance)>,
}

pub fn setup_intersection_observer(
    targets: &[&'static Instance],
    callback: &'static ObserveVisibleRectsCallback,
    options: Option<IntersectionObserverInit<'_>>,
) -> IntersectionObserverHandle {
    let rect_ratio_cache: &'static Map<Instance, Intersection> = js::map::new(None);
    targets.iter().for_each(|target| {
        rect_ratio_cache.set(
            target,
            Intersection {
                rect: get_bounding_rect(target),
                ratio: 0.0,
            },
        );
    });

    let handle_intersection = move |entries: &[&IntersectionObserverEntry],
                                    _: &IntersectionObserver| {
        entries.iter().for_each(|entry| {
            let bounding_client_rect = entry.bounding_client_rect();
            let intersection_ratio = entry.intersection_ratio();
            let target = entry.target();
            rect_ratio_cache.set(
                target,
                Intersection {
                    rect: BoundingRect {
                        x: Cell::new(bounding_client_rect.left()),
                        y: Cell::new(bounding_client_rect.top()),
                        width: Cell::new(bounding_client_rect.width()),
                        height: Cell::new(bounding_client_rect.height()),
                    },
                    ratio: intersection_ratio,
                },
            );
        });

        callback(rect_ratio_cache.values().collect());
    };

    let observer = intersection_observer::new(Box::new(handle_intersection), options);
    targets.iter().for_each(|target| {
        observer.observe(target);
    });

    IntersectionObserverHandle {
        disconnect: Box::new(move || observer.disconnect()),
        observe: Box::new(move |target| {
            rect_ratio_cache.set(
                target,
                Intersection {
                    rect: get_bounding_rect(target),
                    ratio: 0.0,
                },
            );
            observer.observe(target);
        }),
        unobserve: Box::new(move |target| {
            rect_ratio_cache.delete(target);
            observer.unobserve(target);
        }),
    }
}

pub fn request_post_paint_callback(callback: Box<dyn FnOnce(Option<f64>)>) {
    LOCAL_REQUEST_ANIMATION_FRAME.with(|frame| frame.unwrap())(Box::new(move |_| {
        LOCAL_REQUEST_ANIMATION_FRAME.with(|frame| frame.unwrap())(callback);
    }));
}

// -------------------
//     Singletons
// -------------------

#[cfg_attr(rust_js, rust_js::name = "supportsSingletons")]
pub const SUPPORTS_SINGLETONS: bool = true;

pub fn is_host_singleton_type(r#type: &str) -> bool {
    r#type == "html" || r#type == "head" || r#type == "body"
}

pub fn resolve_singleton_instance(
    r#type: &str,
    _props: &Props,
    root_container_instance: &'static Container,
    host_context: &'static HostContext,
    validate_dom_nesting_dev: bool,
) -> &'static Instance {
    if __DEV__ {
        let host_context_dev: &HostContextDev = unsafe { js::cast(Some(host_context)) };
        if validate_dom_nesting_dev {
            validate_dom_nesting(r#type, host_context_dev.ancestor_info.as_ref());
        }
    }
    let owner_document = get_owner_document_from_root_container(root_container_instance);
    match r#type {
        "html" => {
            let Some(document_element) = owner_document.document_element() else {
                panic!(concat!(
                    "React expected an <html> element (document.documentElement) to exist in the Document but one was",
                    " not found. React never removes the documentElement for any Document it renders into so",
                    " the cause is likely in some other script running on this page."
                ));
            };
            document_element
        }
        "head" => {
            let Some(head) = owner_document.head() else {
                panic!(concat!(
                    "React expected a <head> element (document.head) to exist in the Document but one was",
                    " not found. React never removes the head for any Document it renders into so",
                    " the cause is likely in some other script running on this page."
                ));
            };
            head
        }
        "body" => {
            let Some(body) = owner_document.body() else {
                panic!(concat!(
                    "React expected a <body> element (document.body) to exist in the Document but one was",
                    " not found. React never removes the body for any Document it renders into so",
                    " the cause is likely in some other script running on this page."
                ));
            };
            body
        }
        _ => {
            panic!(
                "resolveSingletonInstance was called with an element type that is not supported. This is a bug in React."
            );
        }
    }
}

pub fn acquire_singleton_instance(
    r#type: &str,
    props: &'static Props,
    instance: &'static Instance,
    internal_instance_handle: &Rc<Fiber>,
) {
    if __DEV__ {
        if
        // If this instance is the container then it is invalid to acquire it as a singleton however
        // the DOM nesting validation will already warn for this and the message below isn't semantically
        // aligned with the actual fix you need to make so we omit the warning in this case
        !is_container_marked_as_root(js::unknown(instance))
            // If this instance isn't the root but is currently owned by a different HostSingleton instance then
            // we we need to warn that you are rendering more than one singleton at a time.
            && get_instance_from_node_dom_tree(instance).is_some()
        {
            let tag_name = instance.tag_name().to_lowercase();
            console::error((
                concat!(
                    "You are mounting a new %s component when a previous one has not first unmounted. It is an",
                    " error to render more than one %s component at a time and attributes and children of these",
                    " components will likely fail in unpredictable ways. Please only render a single instance of",
                    " <%s> and if you need to mount a new one, ensure any previous ones have unmounted first."
                ),
                &tag_name,
                &tag_name,
                &tag_name,
            ));
        }
        match r#type {
            "html" | "head" | "body" => {}
            _ => {
                console::error((
                    "acquireSingletonInstance was called with an element type that is not supported. This is a bug in React.",
                ));
            }
        }
    }

    let attributes = instance.attributes();
    while attributes.length() != 0.0 {
        instance.remove_attribute_node(&attributes[0]);
    }

    set_initial_properties(instance, r#type, js::unknown(props));
    precache_fiber_node(internal_instance_handle, js::unknown(instance));
    update_fiber_props(instance, js::unknown(props));
}

pub fn release_singleton_instance(
    instance: &'static Instance,
    r#type: &str,
    props: &'static Props,
) {
    // Remove the attributes and property-backed state owned by this Fiber.
    clear_singleton_properties(instance, r#type, js::unknown(props));

    // These properties aren't cleared by updateProperties when their next
    // value is null. Normally that is handled by replacing/removing the host
    // instance, but a singleton cannot be removed.
    // TODO: HostSingleton updates do not currently schedule ContentReset when
    // dangerouslySetInnerHTML becomes undefined, so an ordinary update can leave
    // the previous HTML in place. This only handles the release path.
    if props.dangerously_set_inner_html.is_some() {
        instance.set_text_content("");
    }
    clear_click_listener(html_element::unchecked_from(instance));

    // Only remove state that was represented by this Fiber's props. Attributes
    // added imperatively while React owned the singleton must be preserved.
    detach_deleted_instance(instance);
}

fn clear_singleton_preamble_contribution(instance: &Instance) {
    // This path is only used when clearing a dehydrated boundary that contains a
    // Fizz preamble contribution marker. The marker tells us which singleton the
    // boundary contributed to, but it does not include the contributed props and
    // there is no HostSingleton Fiber to provide them. We therefore cannot tell
    // which attributes came from React and which were added imperatively by a
    // script or third party. For now, clearing every attribute is an accepted
    // edge case.
    // TODO: Include the contributed properties in the marker so this cleanup can
    // remove only the attributes owned by the boundary.
    let attributes = instance.attributes();
    while attributes.length() != 0.0 {
        instance.remove_attribute_node(&attributes[0]);
    }
    detach_deleted_instance(instance);
}
use js::Map;
use react_reconciler::react_fiber_hooks::request_form_reset as request_form_reset_on_fiber;
use react_reconciler::react_fiber_host_context::get_current_root_host_container;
use react_reconciler::react_fiber_work_loop::flush_sync_work as flush_sync_work_on_all_roots;
use shared::react_dom_shared_internals::{HostDispatcher, REACT_DOM_SHARED_INTERNALS};
use shared::react_feature_flags::DISABLE_LEGACY_MODE;
use webapi::{DocumentOrShadowRoot, HTMLFormElement, HTMLLinkElement, document_or_shadow_root};

use crate::escape_selector_attribute_value_inside_double_quotes::escape_selector_attribute_value_inside_double_quotes;
use crate::react_dom_component_tree::{
    clear_pending_load_on_node, get_resources_from_root, is_node_pending_load,
    mark_node_as_pending_load,
};

// -------------------
//     Resources
// -------------------

#[cfg_attr(rust_js, rust_js::name = "supportsResources")]
pub const SUPPORTS_RESOURCES: bool = true;

/// A Resource's loading state, LoadingState's bits.
type LoadingState = i32;
#[cfg_attr(rust_js, rust_js::name = "NotLoaded")]
const NOT_LOADED: LoadingState = /*       */ 0b000;
#[cfg_attr(rust_js, rust_js::name = "Loaded")]
const LOADED: LoadingState = /*          */ 0b001;
#[cfg_attr(rust_js, rust_js::name = "Errored")]
const ERRORED: LoadingState = /*         */ 0b010;
#[cfg_attr(rust_js, rust_js::name = "Settled")]
const SETTLED: LoadingState = /*         */ 0b011;
#[cfg_attr(rust_js, rust_js::name = "Inserted")]
const INSERTED: LoadingState = /*        */ 0b100;

/// A stylesheet Resource's state: how far its loading is, and the preload
/// link it adopted.
pub struct StylesheetState {
    pub loading: Cell<LoadingState>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub preload: Cell<Option<&'static HTMLLinkElement>>,
}

unsafe impl js::Defined for StylesheetState {}

/// A hoistable stylesheet, style tag, script or void element, and how many
/// of the tree hold it.
pub struct Resource {
    pub r#type: &'static str,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub instance: Cell<Option<&'static Instance>>,
    pub count: Cell<f64>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub state: Option<Rc<StylesheetState>>,
}

unsafe impl js::Defined for Resource {}

/// A root's hoistable styles and scripts, each by its key.
pub struct RootResources {
    pub hoistable_styles: &'static Map<String, Rc<Resource>>,
    pub hoistable_scripts: &'static Map<String, Rc<Resource>>,
}

pub fn prepare_to_commit_hoistables() {
    TAG_CACHES.set(js::null());
}

thread_local! {
    // global collections of Resources
    static PRELOAD_PROPS_MAP: &'static Map<String, &'static Unknown> = js::map::new(None);
    static PRECONNECTS_SET: &'static js::Set<str> = js::set::new(None);
}

/// A Document or a ShadowRoot, where hoistables go.
pub type HoistableRoot = DocumentOrShadowRoot;

// getRootNode is missing from IE and old jsdom versions
pub fn get_hoistable_root(container: &'static Container) -> &'static HoistableRoot {
    // $FlowFixMe[method-unbinding]
    if js::is_function((container, Node::get_root_node)) {
        let root_node = container.get_root_node(None);
        if root_node.node_type() == DOCUMENT_NODE {
            return document_or_shadow_root::unchecked_from(root_node);
        }
        if root_node.node_type() == DOCUMENT_FRAGMENT_NODE {
            return document_or_shadow_root::unchecked_from(root_node);
        }
    }
    if container.node_type() == DOCUMENT_NODE {
        document_or_shadow_root::unchecked_from(container)
    } else {
        DocumentOrShadowRoot::of(container.owner_document().unwrap())
    }
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn get_current_resource_root() -> Option<&'static HoistableRoot> {
    let current_container = get_current_root_host_container();
    current_container
        .map(|current_container| get_hoistable_root(unsafe { js::cast(Some(current_container)) }))
}

fn get_document_from_root(root: &'static HoistableRoot) -> &'static Document {
    root.owner_document()
        .unwrap_or_else(|| webapi::document::unchecked_from(root))
}

thread_local! {
    static PREVIOUS_DISPATCHER: &'static HostDispatcher =
        REACT_DOM_SHARED_INTERNALS.d(); /* ReactDOMCurrentDispatcher */
}

js::on_load! {
    REACT_DOM_SHARED_INTERNALS /* ReactDOMCurrentDispatcher */
        .set_d(HostDispatcher {
            f /* flushSyncWork */: if DISABLE_LEGACY_MODE {
                &flush_sync_work
            } else {
                PREVIOUS_DISPATCHER.with(|previous_dispatcher| previous_dispatcher.f)
            },
            r: &request_form_reset,
            prefetch_dns: &prefetch_dns,
            preconnect: &preconnect,
            preload: &preload,
            m: &preload_module,
            preinit_script: &preinit_script,
            preinit_style: &preinit_style,
            preinit_module_script: &preinit_module_script,
        });
}

fn flush_sync_work() -> Option<bool> {
    if DISABLE_LEGACY_MODE {
        let previous_was_rendering =
            PREVIOUS_DISPATCHER.with(|previous_dispatcher| (previous_dispatcher.f)()); /* flushSyncWork */
        let was_rendering = flush_sync_work_on_all_roots();
        // Since multiple dispatchers can flush sync work during a single flushSync call
        // we need to return true if any of them were rendering.
        Some(previous_was_rendering == Some(true) || was_rendering)
    } else {
        panic!(
            "flushSyncWork should not be called from builds that support legacy mode. This is a bug in React."
        );
    }
}

fn request_form_reset(form: &HTMLFormElement) {
    let form_inst = get_instance_from_node_dom_tree(form);
    if let Some(form_inst) = form_inst
        && form_inst.tag.get() == HOST_COMPONENT
        && js::strict_equals(form_inst.r#type.get(), "form")
    {
        request_form_reset_on_fiber(&form_inst);
    } else {
        // This form was either not rendered by this React renderer (or it's an
        // invalid type). Try the next one.
        //
        // The last implementation in the sequence will throw an error.
        PREVIOUS_DISPATCHER
            .with(|previous_dispatcher| (previous_dispatcher.r)(/* requestFormReset */ form));
    }
}

thread_local! {
    // We expect this to get inlined. It is a function mostly to communicate the special nature of
    // how we resolve the HoistableRoot for ReactDOM.pre*() methods. Because we support calling
    // these methods outside of render there is no way to know which Document or ShadowRoot is 'scoped'
    // and so we have to fall back to something universal. Currently we just refer to the global document.
    // This is notable because nowhere else in ReactDOM do we actually reference the global document or window
    // because we may be rendering inside an iframe.
    #[cfg_attr(rust_js, rust_js::nullable)]
    static GLOBAL_DOCUMENT: Option<&'static Document> =
        if js::is_defined(document) { Some(document) } else { None };
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn get_global_document() -> Option<&'static Document> {
    GLOBAL_DOCUMENT.with(|global_document| *global_document)
}

/// The props of a preconnect or dns-prefetch link.
struct PreconnectProps {
    rel: String,
    cross_origin: Option<&'static Unknown>,
    href: String,
}

unsafe impl js::Defined for PreconnectProps {}

fn preconnect_as(rel: &str, href: &str, cross_origin: Option<&'static Unknown>) {
    let owner_document = get_global_document();
    if let Some(owner_document) = owner_document
        && !href.is_empty()
    {
        let limited_escaped_href = escape_selector_attribute_value_inside_double_quotes(href);
        let mut key = format!("link[rel=\"{rel}\"][href=\"{limited_escaped_href}\"]");
        if let Kind::String(cross_origin) = js::classify(cross_origin) {
            key += &format!("[crossorigin=\"{cross_origin}\"]");
        }
        if !PRECONNECTS_SET.with(|preconnects_set| preconnects_set.has(&key)) {
            PRECONNECTS_SET.with(|preconnects_set| preconnects_set.add(&key));

            let preconnect_props = js::unknown_of(PreconnectProps {
                rel: rel.to_owned(),
                cross_origin,
                href: href.to_owned(),
            });
            if owner_document.query_selector(&key).is_none() {
                let instance = owner_document.create_element_named("link", None);
                set_initial_properties(instance, "link", preconnect_props);
                mark_node_as_hoistable(instance);
                owner_document.head().unwrap().append_child(instance);
            }
        }
    }
}

#[cfg_attr(rust_js, rust_js::name = "prefetchDNS")]
fn prefetch_dns(href: &str) {
    PREVIOUS_DISPATCHER.with(|previous_dispatcher| {
        (previous_dispatcher.prefetch_dns)(/* prefetchDNS */ href)
    });
    preconnect_as("dns-prefetch", href, js::null());
}

fn preconnect(href: &str, cross_origin: Option<&'static Unknown>) {
    PREVIOUS_DISPATCHER.with(|previous_dispatcher| {
        (previous_dispatcher.preconnect)(/* preconnect */ href, cross_origin)
    });
    preconnect_as("preconnect", href, cross_origin);
}

/// The props a preload link starts from, before the caller's options.
struct PreloadLiteral {
    rel: &'static str,
    // There is a bug in Safari where imageSrcSet is not respected on preload links
    // so we omit the href here if we have imageSrcSet b/c safari will load the wrong image.
    // This harms older browers that do not support imageSrcSet by making their preloads not work
    // but this population is shrinking fast and is already small so we accept this tradeoff.
    href: Option<String>,
    r#as: String,
}

unsafe impl js::Defined for PreloadLiteral {}

fn preload(href: &str, r#as: &str, options: Option<&'static Unknown>) {
    PREVIOUS_DISPATCHER.with(|previous_dispatcher| {
        (previous_dispatcher.preload)(/* preload */ href, r#as, options)
    });
    let owner_document = get_global_document();
    if let Some(owner_document) = owner_document
        && !href.is_empty()
        && !r#as.is_empty()
    {
        let mut preload_selector = format!(
            "link[rel=\"preload\"][as=\"{}\"]",
            escape_selector_attribute_value_inside_double_quotes(r#as)
        );
        if r#as == "image" {
            if js::truthy(options) && js::truthy(js::get(options, "imageSrcSet")) {
                preload_selector += &format!(
                    "[imagesrcset=\"{}\"]",
                    escape_selector_attribute_value_inside_double_quotes(&js::string(js::get(
                        options,
                        "imageSrcSet"
                    )))
                );
                if let Kind::String(image_sizes) = js::classify(js::get(options, "imageSizes")) {
                    preload_selector += &format!(
                        "[imagesizes=\"{}\"]",
                        escape_selector_attribute_value_inside_double_quotes(image_sizes)
                    );
                }
            } else {
                preload_selector += &format!(
                    "[href=\"{}\"]",
                    escape_selector_attribute_value_inside_double_quotes(href)
                );
            }
        } else {
            preload_selector += &format!(
                "[href=\"{}\"]",
                escape_selector_attribute_value_inside_double_quotes(href)
            );
        }
        // Some preloads are keyed under their selector. This happens when the preload is for
        // an arbitrary type. Other preloads are keyed under the resource key they represent a preload for.
        // Here we figure out which key to use to determine if we have a preload already.
        let mut key = preload_selector.clone();
        match r#as {
            "style" => key = get_style_key(href),
            "script" => key = get_script_key(href),
            _ => {}
        }
        if !PRELOAD_PROPS_MAP.with(|preload_props_map| preload_props_map.has(&key)) {
            let preload_props = js::object::assign(
                js::unknown_of(PreloadLiteral {
                    rel: "preload",
                    href: if r#as == "image"
                        && js::truthy(options)
                        && js::truthy(js::get(options, "imageSrcSet"))
                    {
                        None
                    } else {
                        Some(href.to_owned())
                    },
                    r#as: r#as.to_owned(),
                }),
                (options,),
            );
            PRELOAD_PROPS_MAP.with(|preload_props_map| preload_props_map.set(&key, preload_props));

            if owner_document.query_selector(&preload_selector).is_none() {
                if r#as == "style"
                    && owner_document
                        .query_selector(&get_stylesheet_selector_from_key(&key))
                        .is_some()
                {
                    // We already have a stylesheet for this key. We don't need to preload it.
                    return;
                } else if r#as == "script"
                    && owner_document
                        .query_selector(&get_script_selector_from_key(&key))
                        .is_some()
                {
                    // We already have a script for this key. We don't need to preload it.
                    return;
                }
                let instance = owner_document.create_element_named("link", None);
                set_initial_properties(instance, "link", preload_props);
                if r#as == "style" {
                    // Stash a loading state on the preload link. it will clean itself up once settled
                    mark_node_as_pending_load(instance);
                    let clear =
                        webapi::listener(move |_: &Event| clear_pending_load_on_node(instance));
                    html_element::unchecked_from(instance).set_onerror(Some(&clear));
                    html_element::unchecked_from(instance).set_onload(Some(&clear));
                }
                mark_node_as_hoistable(instance);
                owner_document.head().unwrap().append_child(instance);
            }
        }
    }
}

/// The props a modulepreload link starts from, before the caller's options.
struct PreloadModuleLiteral {
    rel: &'static str,
    href: String,
}

unsafe impl js::Defined for PreloadModuleLiteral {}

fn preload_module(href: &str, options: Option<&'static Unknown>) {
    PREVIOUS_DISPATCHER.with(|previous_dispatcher| {
        (previous_dispatcher.m)(/* preloadModule */ href, options)
    });
    let owner_document = get_global_document();
    if let Some(owner_document) = owner_document
        && !href.is_empty()
    {
        let r#as = if js::truthy(options)
            && let Kind::String(r#as) = js::classify(js::get(options, "as"))
        {
            r#as.to_owned()
        } else {
            "script".to_owned()
        };
        let preload_selector = format!(
            "link[rel=\"modulepreload\"][as=\"{}\"][href=\"{}\"]",
            escape_selector_attribute_value_inside_double_quotes(&r#as),
            escape_selector_attribute_value_inside_double_quotes(href)
        );
        // Some preloads are keyed under their selector. This happens when the preload is for
        // an arbitrary type. Other preloads are keyed under the resource key they represent a preload for.
        // Here we figure out which key to use to determine if we have a preload already.
        let mut key = preload_selector.clone();
        if let "audioworklet" | "paintworklet" | "serviceworker" | "sharedworker" | "worker"
        | "script" = r#as.as_str()
        {
            key = get_script_key(href);
        }

        if !PRELOAD_PROPS_MAP.with(|preload_props_map| preload_props_map.has(&key)) {
            let props = js::object::assign(
                js::unknown_of(PreloadModuleLiteral {
                    rel: "modulepreload",
                    href: href.to_owned(),
                }),
                (options,),
            );
            PRELOAD_PROPS_MAP.with(|preload_props_map| preload_props_map.set(&key, props));

            if owner_document.query_selector(&preload_selector).is_none() {
                if let "audioworklet" | "paintworklet" | "serviceworker" | "sharedworker" | "worker"
                | "script" = r#as.as_str()
                    && owner_document
                        .query_selector(&get_script_selector_from_key(&key))
                        .is_some()
                {
                    return;
                }
                let instance = owner_document.create_element_named("link", None);
                set_initial_properties(instance, "link", props);
                mark_node_as_hoistable(instance);
                owner_document.head().unwrap().append_child(instance);
            }
        }
    }
}

/// The props a preinitialized stylesheet starts from, before the caller's options.
struct StylesheetLiteral {
    rel: &'static str,
    href: String,
    #[cfg_attr(rust_js, rust_js::name = "data-precedence")]
    data_precedence: String,
}

unsafe impl js::Defined for StylesheetLiteral {}

fn preinit_style(
    href: &str,
    precedence: Option<&'static Unknown>,
    options: Option<&'static Unknown>,
) {
    PREVIOUS_DISPATCHER.with(|previous_dispatcher| {
        (previous_dispatcher.preinit_style)(/* preinitStyle */ href, precedence, options)
    });

    let owner_document = get_global_document();
    if let Some(owner_document) = owner_document
        && !href.is_empty()
    {
        let styles = get_resources_from_root(js::unknown(owner_document)).hoistable_styles;

        let key = get_style_key(href);
        let precedence = if js::truthy(precedence) {
            js::string(precedence)
        } else {
            "default".to_owned()
        };

        // Check if this resource already exists
        let resource = styles.get(&key);
        if resource.is_some() {
            // We can early return. The resource exists and there is nothing
            // more to do
            return;
        }

        let state = Rc::new(StylesheetState {
            loading: Cell::new(NOT_LOADED),
            preload: Cell::new(None),
        });

        // Attempt to hydrate instance from DOM
        let mut instance = owner_document.query_selector(&get_stylesheet_selector_from_key(&key));
        if instance.is_some() {
            state.loading.set(LOADED | INSERTED);
        } else {
            // Construct a new instance and insert it
            let stylesheet_props = js::object::assign(
                js::unknown_of(StylesheetLiteral {
                    rel: "stylesheet",
                    href: href.to_owned(),
                    data_precedence: precedence.clone(),
                }),
                (options,),
            );
            let preload_props =
                PRELOAD_PROPS_MAP.with(|preload_props_map| preload_props_map.get(&key));
            if let Some(preload_props) = preload_props {
                adopt_preload_props_for_stylesheet(stylesheet_props, preload_props);
            }
            let link = owner_document.create_element_named("link", None);
            instance = Some(link);
            mark_node_as_hoistable(link);
            set_initial_properties(link, "link", stylesheet_props);

            js::set(
                link,
                "_p",
                js::promise::new(|resolve: &'static dyn Fn(&'static Event), reject| {
                    html_element::unchecked_from(link).set_onload(Some(&resolve));
                    html_element::unchecked_from(link).set_onerror(Some(&reject));
                }),
            );
            link.add_event_listener_named(
                "load",
                {
                    let state = Rc::clone(&state);
                    move |_: &Event| state.loading.set(state.loading.get() | LOADED)
                },
                None,
            );
            link.add_event_listener_named(
                "error",
                {
                    let state = Rc::clone(&state);
                    move |_: &Event| state.loading.set(state.loading.get() | ERRORED)
                },
                None,
            );

            state.loading.set(state.loading.get() | INSERTED);
            insert_stylesheet(link, &precedence, DocumentOrShadowRoot::of(owner_document));
        }

        // Construct a Resource and cache it
        // $FlowFixMe[incompatible-type]
        let resource = Rc::new(Resource {
            r#type: "stylesheet",
            instance: Cell::new(instance),
            count: Cell::new(1.0),
            state: Some(state),
        });
        // $FlowFixMe[incompatible-type]
        styles.set(&key, resource);
    }
}

/// The props a preinitialized script starts from, before the caller's options.
struct ScriptLiteral {
    src: String,
    r#async: bool,
    r#type: Option<&'static str>,
}

unsafe impl js::Defined for ScriptLiteral {}

fn preinit_script(src: &str, options: Option<&'static Unknown>) {
    PREVIOUS_DISPATCHER.with(|previous_dispatcher| {
        (previous_dispatcher.preinit_script)(/* preinitScript */ src, options)
    });

    let owner_document = get_global_document();
    if let Some(owner_document) = owner_document
        && !src.is_empty()
    {
        let scripts = get_resources_from_root(js::unknown(owner_document)).hoistable_scripts;

        let key = get_script_key(src);

        // Check if this resource already exists
        let resource = scripts.get(&key);
        if resource.is_some() {
            // We can early return. The resource exists and there is nothing
            // more to do
            return;
        }

        // Attempt to hydrate instance from DOM
        let mut instance = owner_document.query_selector(&get_script_selector_from_key(&key));
        if instance.is_none() {
            // Construct a new instance and insert it
            let script_props = js::object::assign(
                js::unknown_of(ScriptLiteral {
                    src: src.to_owned(),
                    r#async: true,
                    r#type: None,
                }),
                (options,),
            );
            // Adopt certain preload props
            let preload_props =
                PRELOAD_PROPS_MAP.with(|preload_props_map| preload_props_map.get(&key));
            if let Some(preload_props) = preload_props {
                adopt_preload_props_for_script(script_props, preload_props);
            }
            let script = owner_document.create_element_named("script", None);
            instance = Some(script);
            mark_node_as_hoistable(script);
            set_initial_properties(script, "link", script_props);
            owner_document.head().unwrap().append_child(script);
        }

        // Construct a Resource and cache it
        let resource = Rc::new(Resource {
            r#type: "script",
            instance: Cell::new(instance),
            count: Cell::new(1.0),
            state: None,
        });
        scripts.set(&key, resource);
    }
}

fn preinit_module_script(src: &str, options: Option<&'static Unknown>) {
    PREVIOUS_DISPATCHER.with(|previous_dispatcher| {
        (previous_dispatcher.preinit_module_script)(/* preinitModuleScript */ src, options)
    });

    let owner_document = get_global_document();
    if let Some(owner_document) = owner_document
        && !src.is_empty()
    {
        let scripts = get_resources_from_root(js::unknown(owner_document)).hoistable_scripts;

        let key = get_script_key(src);

        // Check if this resource already exists
        let resource = scripts.get(&key);
        if resource.is_some() {
            // We can early return. The resource exists and there is nothing
            // more to do
            return;
        }

        // Attempt to hydrate instance from DOM
        let mut instance = owner_document.query_selector(&get_script_selector_from_key(&key));
        if instance.is_none() {
            // Construct a new instance and insert it
            let script_props = js::object::assign(
                js::unknown_of(ScriptLiteral {
                    src: src.to_owned(),
                    r#async: true,
                    r#type: Some("module"),
                }),
                (options,),
            );
            // Adopt certain preload props
            let preload_props =
                PRELOAD_PROPS_MAP.with(|preload_props_map| preload_props_map.get(&key));
            if let Some(preload_props) = preload_props {
                adopt_preload_props_for_script(script_props, preload_props);
            }
            let script = owner_document.create_element_named("script", None);
            instance = Some(script);
            mark_node_as_hoistable(script);
            set_initial_properties(script, "link", script_props);
            owner_document.head().unwrap().append_child(script);
        }

        // Construct a Resource and cache it
        let resource = Rc::new(Resource {
            r#type: "script",
            instance: Cell::new(instance),
            count: Cell::new(1.0),
            state: None,
        });
        scripts.set(&key, resource);
    }
}

/// The props a `<style>` hoists by: its href and precedence, both text.
pub struct StyleTagQualifyingProps {
    pub href: String,
    pub precedence: String,
}

unsafe impl js::Defined for StyleTagQualifyingProps {}

/// The props a stylesheet `<link>` hoists by: its rel, href and precedence.
pub struct StylesheetQualifyingProps {
    pub rel: String,
    pub href: String,
    pub precedence: String,
}

unsafe impl js::Defined for StylesheetQualifyingProps {}

/// The props of an async script, as `getResource` qualifies them.
pub struct ScriptProps {
    pub src: String,
    pub r#async: bool,
}

unsafe impl js::Defined for ScriptProps {}

// This function is called in begin work and we should always have a currentDocument set
#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_resource(
    r#type: &str,
    current_props: Option<&'static Unknown>,
    pending_props: &'static Unknown,
    current_resource: Option<&Resource>,
) -> Option<Rc<Resource>> {
    let Some(resource_root) = get_current_resource_root() else {
        panic!("\"resourceRoot\" was expected to exist. This is a bug in React.");
    };
    match r#type {
        "meta" | "title" => js::null(),
        "style" => {
            if let (Kind::String(_), Kind::String(href)) = (
                js::classify(js::get(pending_props, "precedence")),
                js::classify(js::get(pending_props, "href")),
            ) {
                let key = get_style_key(href);
                let styles = get_resources_from_root(js::unknown(resource_root)).hoistable_styles;
                let resource = match styles.get(&key) {
                    Some(resource) => resource,
                    None => {
                        let resource = Rc::new(Resource {
                            r#type: "style",
                            instance: Cell::new(None),
                            count: Cell::new(0.0),
                            state: None,
                        });
                        styles.set(&key, Rc::clone(&resource));
                        resource
                    }
                };
                return Some(resource);
            }
            Some(Rc::new(Resource {
                r#type: "void",
                instance: Cell::new(None),
                count: Cell::new(0.0),
                state: None,
            }))
        }
        "link" => {
            if js::strict_equals(js::get(pending_props, "rel"), "stylesheet")
                && let Kind::String(href) = js::classify(js::get(pending_props, "href"))
                && matches!(
                    js::classify(js::get(pending_props, "precedence")),
                    Kind::String(_)
                )
            {
                let qualified_props = pending_props;
                let key = get_style_key(href);

                let styles = get_resources_from_root(js::unknown(resource_root)).hoistable_styles;

                let resource = match styles.get(&key) {
                    Some(resource) => resource,
                    None => {
                        // We asserted this above but Flow can't figure out that the type satisfies
                        let owner_document = get_document_from_root(resource_root);
                        let state = Rc::new(StylesheetState {
                            loading: Cell::new(NOT_LOADED),
                            preload: Cell::new(None),
                        });
                        let resource = Rc::new(Resource {
                            r#type: "stylesheet",
                            instance: Cell::new(None),
                            count: Cell::new(0.0),
                            state: Some(Rc::clone(&state)),
                        });
                        styles.set(&key, Rc::clone(&resource));
                        let instance =
                            owner_document.query_selector(&get_stylesheet_selector_from_key(&key));
                        if let Some(instance) = instance {
                            let loading_state = js::get(instance, "_p");
                            if js::truthy(loading_state) {
                                // This instance is inserted as part of a boundary reveal and is not yet
                                // loaded
                            } else {
                                // This instance is already loaded
                                resource.instance.set(Some(instance));
                                state.loading.set(LOADED | INSERTED);
                            }
                        } else {
                            // We don't have an instance we need to preload it instead
                            // $FlowFixMe[incompatible-type] -- the key we use here can only match non module preloads
                            let preload_props = match PRELOAD_PROPS_MAP
                                .with(|preload_props_map| preload_props_map.get(&key))
                            {
                                Some(preload_props) => preload_props,
                                None => {
                                    let preload_props =
                                        preload_props_from_stylesheet(qualified_props);
                                    PRELOAD_PROPS_MAP.with(|preload_props_map| {
                                        preload_props_map.set(&key, preload_props)
                                    });
                                    preload_props
                                }
                            };

                            preload_stylesheet(owner_document, &key, preload_props, &state);
                        }
                        resource
                    }
                };
                if js::truthy(current_props) && current_resource.is_none() {
                    // This node was previously an Instance type and is becoming a Resource type
                    // For now we error because we don't support flavor changes
                    let mut diff = String::new();
                    if __DEV__ {
                        diff = format!(
                            "\n\n  - {}\n  + {}",
                            describe_link_for_resource_error_dev(current_props.unwrap()),
                            describe_link_for_resource_error_dev(pending_props)
                        );
                    }
                    panic!(
                        "{}{}",
                        concat!(
                            "Expected <link> not to update to be updated to a stylesheet with precedence.",
                            " Check the `rel`, `href`, and `precedence` props of this component.",
                            " Alternatively, check whether two different <link> components render in the same slot or share the same key."
                        ),
                        diff
                    );
                }
                Some(resource)
            } else {
                if js::truthy(current_props) && current_resource.is_some() {
                    // This node was previously a Resource type and is becoming an Instance type
                    // For now we error because we don't support flavor changes
                    let mut diff = String::new();
                    if __DEV__ {
                        diff = format!(
                            "\n\n  - {}\n  + {}",
                            describe_link_for_resource_error_dev(current_props.unwrap()),
                            describe_link_for_resource_error_dev(pending_props)
                        );
                    }
                    panic!(
                        "{}{}",
                        concat!(
                            "Expected stylesheet with precedence to not be updated to a different kind of <link>.",
                            " Check the `rel`, `href`, and `precedence` props of this component.",
                            " Alternatively, check whether two different <link> components render in the same slot or share the same key."
                        ),
                        diff
                    );
                }
                js::null()
            }
        }
        "script" => {
            let r#async = js::get(pending_props, "async");
            let src = js::get(pending_props, "src");
            if let Kind::String(src) = js::classify(src)
                && js::truthy(r#async)
                && !matches!(js::classify(r#async), Kind::Function(_) | Kind::Symbol(_))
            {
                let key = get_script_key(src);
                let scripts = get_resources_from_root(js::unknown(resource_root)).hoistable_scripts;

                let resource = match scripts.get(&key) {
                    Some(resource) => resource,
                    None => {
                        let resource = Rc::new(Resource {
                            r#type: "script",
                            instance: Cell::new(None),
                            count: Cell::new(0.0),
                            state: None,
                        });
                        scripts.set(&key, Rc::clone(&resource));
                        resource
                    }
                };
                return Some(resource);
            }
            Some(Rc::new(Resource {
                r#type: "void",
                instance: Cell::new(None),
                count: Cell::new(0.0),
                state: None,
            }))
        }
        _ => {
            panic!(
                "getResource encountered a type it did not expect: \"{type}\". this is a bug in React."
            );
        }
    }
}

#[cfg_attr(rust_js, rust_js::name = "describeLinkForResourceErrorDEV")]
fn describe_link_for_resource_error_dev(props: &'static Unknown) -> String {
    if __DEV__ {
        let mut described_props = 0;

        let mut description = "<link".to_owned();
        if let Kind::String(rel) = js::classify(js::get(props, "rel")) {
            described_props += 1;
            description += &format!(" rel=\"{rel}\"");
        } else if HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(props, "rel")) {
            described_props += 1;
            description += &format!(
                " rel=\"{}\"",
                if matches!(js::classify(js::get(props, "rel")), Kind::Null) {
                    "null".to_owned()
                } else {
                    "invalid type ".to_owned() + &js::type_of(js::get(props, "rel"))
                }
            );
        }
        if let Kind::String(href) = js::classify(js::get(props, "href")) {
            described_props += 1;
            description += &format!(" href=\"{href}\"");
        } else if HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(props, "href")) {
            described_props += 1;
            description += &format!(
                " href=\"{}\"",
                if matches!(js::classify(js::get(props, "href")), Kind::Null) {
                    "null".to_owned()
                } else {
                    "invalid type ".to_owned() + &js::type_of(js::get(props, "href"))
                }
            );
        }
        if let Kind::String(precedence) = js::classify(js::get(props, "precedence")) {
            described_props += 1;
            description += &format!(" precedence=\"{precedence}\"");
        } else if HAS_OWN_PROPERTY
            .with(|has_own_property| has_own_property.call(props, "precedence"))
        {
            described_props += 1;
            description += &format!(
                " precedence={{{}}}",
                if matches!(js::classify(js::get(props, "precedence")), Kind::Null) {
                    "null".to_owned()
                } else {
                    "invalid type ".to_owned() + &js::type_of(js::get(props, "precedence"))
                }
            );
        }
        if js::object::get_own_property_names(Some(props)).len() > described_props {
            description += " ...";
        }
        description += " />";
        return description;
    }
    String::new()
}

fn style_tag_props_from_raw_props(raw_props: &'static StyleTagQualifyingProps) -> &'static Unknown {
    let style_tag_props = js::object::spread(Some(js::unknown(raw_props)));
    js::set(style_tag_props, "data-href", &raw_props.href);
    js::set(style_tag_props, "data-precedence", &raw_props.precedence);
    js::set(style_tag_props, "href", js::null::<&Unknown>());
    js::set(style_tag_props, "precedence", js::null::<&Unknown>());
    js::unknown(style_tag_props)
}

fn get_style_key(href: &str) -> String {
    let limited_escaped_href = escape_selector_attribute_value_inside_double_quotes(href);
    format!("href=\"{limited_escaped_href}\"")
}

fn get_style_tag_selector(href: &str) -> String {
    let limited_escaped_href = escape_selector_attribute_value_inside_double_quotes(href);
    format!("style[data-href~=\"{limited_escaped_href}\"]")
}

fn get_stylesheet_selector_from_key(key: &str) -> String {
    format!("link[rel=\"stylesheet\"][{key}]")
}

fn get_preload_stylesheet_selector_from_key(key: &str) -> String {
    format!("link[rel=\"preload\"][as=\"style\"][{key}]")
}

fn stylesheet_props_from_raw_props(
    raw_props: &'static StylesheetQualifyingProps,
) -> &'static Unknown {
    let stylesheet_props = js::object::spread(Some(js::unknown(raw_props)));
    js::set(stylesheet_props, "data-precedence", &raw_props.precedence);
    js::set(stylesheet_props, "precedence", js::null::<&Unknown>());
    js::unknown(stylesheet_props)
}

fn preload_stylesheet(
    owner_document: &'static Document,
    key: &str,
    preload_props: &'static Unknown,
    state: &Rc<StylesheetState>,
) {
    let mut instance =
        owner_document.query_selector(&get_preload_stylesheet_selector_from_key(key));
    if let Some(found) = instance {
        if !is_node_pending_load(found) {
            // If we find a preload already it was SSR'd and we won't have an actual
            // loading state to track. For now we will just assume it is loaded
            state.loading.set(LOADED);
            return;
        } else {
            // fall through and attach loading listeners
        }
    } else {
        let link = owner_document.create_element_named("link", None);
        instance = Some(link);
        mark_node_as_pending_load(link);
        let clear = webapi::listener(move |_: &Event| clear_pending_load_on_node(link));
        html_element::unchecked_from(link).set_onerror(Some(&clear));
        html_element::unchecked_from(link).set_onload(Some(&clear));
        set_initial_properties(link, "link", preload_props);
        mark_node_as_hoistable(link);
        owner_document.head().unwrap().append_child(link);
    }
    let instance = instance.unwrap();
    // $FlowFixMe[incompatible-type] -- if instance is an Element it will also be an HTMLLinkElement
    state
        .preload
        .set(Some(html_link_element::unchecked_from(instance)));
    instance.add_event_listener_named(
        "load",
        {
            let state = Rc::clone(state);
            move |_: &Event| state.loading.set(state.loading.get() | LOADED)
        },
        None,
    );
    instance.add_event_listener_named(
        "error",
        {
            let state = Rc::clone(state);
            move |_: &Event| state.loading.set(state.loading.get() | ERRORED)
        },
        None,
    );
}

/// The preload props a stylesheet gets from its own.
struct PreloadPropsOfStylesheet {
    rel: &'static str,
    r#as: &'static str,
    href: Option<&'static Unknown>,
    cross_origin: Option<&'static Unknown>,
    integrity: Option<&'static Unknown>,
    media: Option<&'static Unknown>,
    href_lang: Option<&'static Unknown>,
    referrer_policy: Option<&'static Unknown>,
}

unsafe impl js::Defined for PreloadPropsOfStylesheet {}

fn preload_props_from_stylesheet(props: &'static Unknown) -> &'static Unknown {
    js::unknown_of(PreloadPropsOfStylesheet {
        rel: "preload",
        r#as: "style",
        href: js::get(props, "href"),
        cross_origin: js::get(props, "crossOrigin"),
        integrity: js::get(props, "integrity"),
        media: js::get(props, "media"),
        href_lang: js::get(props, "hrefLang"),
        referrer_policy: js::get(props, "referrerPolicy"),
    })
}

fn get_script_key(src: &str) -> String {
    let limited_escaped_src = escape_selector_attribute_value_inside_double_quotes(src);
    format!("[src=\"{limited_escaped_src}\"]")
}

fn get_script_selector_from_key(key: &str) -> String {
    "script[async]".to_owned() + key
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn acquire_resource(
    hoistable_root: &'static HoistableRoot,
    resource: &Resource,
    props: &'static Unknown,
) -> Option<&'static Instance> {
    resource.count.set(resource.count.get() + 1.0);
    if resource.instance.get().is_none() {
        match resource.r#type {
            "style" => {
                let qualified_props: &StyleTagQualifyingProps = unsafe { js::cast(Some(props)) };

                // Attempt to hydrate instance from DOM
                let instance =
                    hoistable_root.query_selector(&get_style_tag_selector(&qualified_props.href));
                if let Some(instance) = instance {
                    resource.instance.set(Some(instance));
                    mark_node_as_hoistable(instance);
                    return Some(instance);
                }

                let style_props = style_tag_props_from_raw_props(unsafe { js::cast(Some(props)) });
                let owner_document = get_document_from_root(hoistable_root);
                let instance = owner_document.create_element_named("style", None);

                mark_node_as_hoistable(instance);
                set_initial_properties(instance, "style", style_props);

                // TODO: `style` does not have loading state for tracking insertions. I
                // guess because these aren't suspensey? Not sure whether this is a
                // factoring smell.
                // resource.state.loading |= Inserted;
                insert_stylesheet(instance, &qualified_props.precedence, hoistable_root);
                resource.instance.set(Some(instance));

                Some(instance)
            }
            "stylesheet" => {
                // This typing is enforce by `getResource`. If we change the logic
                // there for what qualifies as a stylesheet resource we need to ensure
                // this cast still makes sense;
                let qualified_props: &StylesheetQualifyingProps = unsafe { js::cast(Some(props)) };
                let key = get_style_key(&qualified_props.href);

                // Attempt to hydrate instance from DOM
                let instance =
                    hoistable_root.query_selector(&get_stylesheet_selector_from_key(&key));
                if let Some(instance) = instance {
                    let state = resource.state.as_ref().unwrap();
                    state.loading.set(state.loading.get() | INSERTED);
                    resource.instance.set(Some(instance));
                    mark_node_as_hoistable(instance);
                    return Some(instance);
                }

                let stylesheet_props =
                    stylesheet_props_from_raw_props(unsafe { js::cast(Some(props)) });
                let preload_props =
                    PRELOAD_PROPS_MAP.with(|preload_props_map| preload_props_map.get(&key));
                if let Some(preload_props) = preload_props {
                    adopt_preload_props_for_stylesheet(stylesheet_props, preload_props);
                }

                // Construct and insert a new instance
                let owner_document = get_document_from_root(hoistable_root);
                let instance = owner_document.create_element_named("link", None);
                mark_node_as_hoistable(instance);
                let link_instance: &HTMLLinkElement = html_link_element::unchecked_from(instance);
                js::set(
                    link_instance,
                    "_p",
                    js::promise::new(|resolve: &'static dyn Fn(&'static Event), reject| {
                        link_instance.set_onload(Some(&resolve));
                        link_instance.set_onerror(Some(&reject));
                    }),
                );
                set_initial_properties(instance, "link", stylesheet_props);
                let state = resource.state.as_ref().unwrap();
                state.loading.set(state.loading.get() | INSERTED);
                insert_stylesheet(instance, &qualified_props.precedence, hoistable_root);
                resource.instance.set(Some(instance));

                Some(instance)
            }
            "script" => {
                // This typing is enforce by `getResource`. If we change the logic
                // there for what qualifies as a stylesheet resource we need to ensure
                // this cast still makes sense;
                let borrowed_script_props: &ScriptProps = unsafe { js::cast(Some(props)) };
                let key = get_script_key(&borrowed_script_props.src);

                // Attempt to hydrate instance from DOM
                let instance = hoistable_root.query_selector(&get_script_selector_from_key(&key));
                if let Some(instance) = instance {
                    resource.instance.set(Some(instance));
                    mark_node_as_hoistable(instance);
                    return Some(instance);
                }

                let mut script_props = js::unknown(borrowed_script_props);
                let preload_props =
                    PRELOAD_PROPS_MAP.with(|preload_props_map| preload_props_map.get(&key));
                if let Some(preload_props) = preload_props {
                    script_props =
                        js::unknown(js::object::spread(Some(js::unknown(borrowed_script_props))));
                    adopt_preload_props_for_script(script_props, preload_props);
                }

                // Construct and insert a new instance
                let owner_document = get_document_from_root(hoistable_root);
                let instance = owner_document.create_element_named("script", None);
                mark_node_as_hoistable(instance);
                set_initial_properties(instance, "link", script_props);
                owner_document.head().unwrap().append_child(instance);
                resource.instance.set(Some(instance));

                Some(instance)
            }
            "void" => None,
            _ => {
                panic!(
                    "acquireResource encountered a resource type it did not expect: \"{}\". this is a bug in React.",
                    resource.r#type
                );
            }
        }
    } else {
        // In the case of stylesheets, they might have already been assigned an
        // instance during `suspendResource`. But that doesn't mean they were
        // inserted, because the commit might have been interrupted. So we need to
        // check now.
        //
        // The other resource types are unaffected because they are not
        // yet suspensey.
        //
        // TODO: This is a bit of a code smell. Consider refactoring how
        // `suspendResource` and `acquireResource` work together. The idea is that
        // `suspendResource` does all the same stuff as `acquireResource` except
        // for the insertion.
        if resource.r#type == "stylesheet"
            && (resource.state.as_ref().unwrap().loading.get() & INSERTED) == NOT_LOADED
        {
            let qualified_props: &StylesheetQualifyingProps = unsafe { js::cast(Some(props)) };
            let instance = resource.instance.get().unwrap();
            let state = resource.state.as_ref().unwrap();
            state.loading.set(state.loading.get() | INSERTED);
            insert_stylesheet(instance, &qualified_props.precedence, hoistable_root);
        }
        resource.instance.get()
    }
}

pub fn release_resource(resource: &Resource) {
    resource.count.set(resource.count.get() - 1.0);
}

fn insert_stylesheet(instance: &Element, precedence: &str, root: &'static HoistableRoot) {
    let nodes =
        root.query_selector_all("link[rel=\"stylesheet\"][data-precedence],style[data-precedence]");
    let last = if nodes.length() != 0.0 {
        Some(&nodes[nodes.length() as usize - 1])
    } else {
        None
    };
    let mut prior = last;
    for i in 0..nodes.length() as usize {
        let node = &nodes[i];
        // $FlowFixMe[prop-missing]
        let node_precedence = js::get(html_element::unchecked_from(node).dataset(), "precedence");
        if js::strict_equals(node_precedence, precedence) {
            prior = Some(node);
        } else if prior != last {
            break;
        }
    }

    if let Some(prior) = prior {
        // We get the prior from the document so we know it is in the tree.
        // We also know that links can't be the topmost Node so the parentNode
        // must exist.
        prior
            .parent_node()
            .unwrap()
            .insert_before(instance, prior.next_sibling());
    } else {
        let parent: &Node = if root.node_type() == DOCUMENT_NODE {
            webapi::document::unchecked_from(root).head().unwrap()
        } else {
            root
        };
        parent.insert_before(instance, parent.first_child());
    }
}

fn adopt_preload_props_for_stylesheet(
    stylesheet_props: &'static Unknown,
    preload_props: &'static Unknown,
) {
    if js::get(stylesheet_props, "crossOrigin").is_none() {
        js::set(
            stylesheet_props,
            "crossOrigin",
            js::get(preload_props, "crossOrigin"),
        );
    }
    if js::get(stylesheet_props, "referrerPolicy").is_none() {
        js::set(
            stylesheet_props,
            "referrerPolicy",
            js::get(preload_props, "referrerPolicy"),
        );
    }
    if js::get(stylesheet_props, "title").is_none() {
        js::set(stylesheet_props, "title", js::get(preload_props, "title"));
    }
}

fn adopt_preload_props_for_script(script_props: &'static Unknown, preload_props: &'static Unknown) {
    if js::get(script_props, "crossOrigin").is_none() {
        js::set(
            script_props,
            "crossOrigin",
            js::get(preload_props, "crossOrigin"),
        );
    }
    if js::get(script_props, "referrerPolicy").is_none() {
        js::set(
            script_props,
            "referrerPolicy",
            js::get(preload_props, "referrerPolicy"),
        );
    }
    if js::get(script_props, "integrity").is_none() {
        js::set(
            script_props,
            "integrity",
            js::get(preload_props, "integrity"),
        );
    }
}

/// A document's hoistable tags of one type, by type and key; each type's own key holds null
/// once the cache is seeded with that type.
type KeyedTagCache = Map<str, Option<Rc<RefCell<Vec<&'static Element>>>>>;
type DocumentTagCaches = Map<Document, &'static KeyedTagCache>;
thread_local! {
    #[cfg_attr(rust_js, rust_js::nullable)]
    static TAG_CACHES: Cell<Option<&'static DocumentTagCaches>> = const { Cell::new(None) };
}

pub fn hydrate_hoistable(
    hoistable_root: &'static HoistableRoot,
    r#type: &str,
    props: &'static Unknown,
    internal_instance_handle: &Rc<Fiber>,
) -> &'static Instance {
    let owner_document = get_document_from_root(hoistable_root);

    let instance: &'static Instance;
    'get_instance: {
        match r#type {
            "title" => {
                let instance = match owner_document.get_elements_by_tag_name("title").item(0) {
                    Some(instance)
                        if !(is_owned_instance(instance)
                            || instance.namespace_uri().as_deref() == Some(SVG_NAMESPACE)
                            || instance.has_attribute("itemprop")) =>
                    {
                        instance
                    }
                    _ => {
                        let instance = owner_document.create_element_named(r#type, None);
                        owner_document.head().unwrap().insert_before(
                            instance,
                            owner_document
                                .query_selector("head > title")
                                .map(|title| -> &Node { title }),
                        );
                        instance
                    }
                };
                set_initial_properties(instance, r#type, props);
                precache_fiber_node(internal_instance_handle, js::unknown(instance));
                mark_node_as_hoistable(instance);
                return instance;
            }
            "link" => {
                let cache = get_hydratable_hoistable_cache("link", "href", owner_document);
                let key = r#type.to_owned()
                    + &(if js::truthy(js::get(props, "href")) {
                        js::concat_text(js::get(props, "href")).unwrap()
                    } else {
                        String::new()
                    });
                let maybe_nodes = cache.get(&key).flatten();
                if let Some(maybe_nodes) = maybe_nodes {
                    let mut nodes = maybe_nodes.borrow_mut();
                    for i in 0..nodes.len() {
                        let node = nodes[i];
                        if !js::strict_equals(
                            node.get_attribute("href"),
                            if js::get(props, "href").is_none()
                                || js::strict_equals(js::get(props, "href"), "")
                            {
                                js::null()
                            } else {
                                js::get(props, "href")
                            },
                        ) || !js::strict_equals(
                            node.get_attribute("rel"),
                            if js::get(props, "rel").is_none() {
                                js::null()
                            } else {
                                js::get(props, "rel")
                            },
                        ) || !js::strict_equals(
                            node.get_attribute("title"),
                            if js::get(props, "title").is_none() {
                                js::null()
                            } else {
                                js::get(props, "title")
                            },
                        ) || !js::strict_equals(
                            node.get_attribute("crossorigin"),
                            if js::get(props, "crossOrigin").is_none() {
                                js::null()
                            } else {
                                js::get(props, "crossOrigin")
                            },
                        ) {
                            // mismatch, try the next node;
                            continue;
                        }
                        instance = node;
                        nodes.remove(i);
                        break 'get_instance;
                    }
                }
                instance = owner_document.create_element_named(r#type, None);
                set_initial_properties(instance, r#type, props);
                owner_document.head().unwrap().append_child(instance);
            }
            "meta" => {
                let cache = get_hydratable_hoistable_cache("meta", "content", owner_document);
                let key = r#type.to_owned()
                    + &(if js::truthy(js::get(props, "content")) {
                        js::concat_text(js::get(props, "content")).unwrap()
                    } else {
                        String::new()
                    });
                let maybe_nodes = cache.get(&key).flatten();
                if let Some(maybe_nodes) = maybe_nodes {
                    let mut nodes = maybe_nodes.borrow_mut();
                    for i in 0..nodes.len() {
                        let node = nodes[i];

                        // We coerce content to string because it is the most likely one to
                        // use a `toString` capable value. For the rest we just do identity match
                        // passing non-strings here is not really valid anyway.
                        if __DEV__ {
                            check_attribute_string_coercion(js::get(props, "content"), "content");
                        }
                        if node.get_attribute("content")
                            != (if js::get(props, "content").is_none() {
                                None
                            } else {
                                Some(js::concat_text(js::get(props, "content")).unwrap())
                            })
                            || !js::strict_equals(
                                node.get_attribute("name"),
                                if js::get(props, "name").is_none() {
                                    js::null()
                                } else {
                                    js::get(props, "name")
                                },
                            )
                            || !js::strict_equals(
                                node.get_attribute("property"),
                                if js::get(props, "property").is_none() {
                                    js::null()
                                } else {
                                    js::get(props, "property")
                                },
                            )
                            || !js::strict_equals(
                                node.get_attribute("http-equiv"),
                                if js::get(props, "httpEquiv").is_none() {
                                    js::null()
                                } else {
                                    js::get(props, "httpEquiv")
                                },
                            )
                            || !js::strict_equals(
                                node.get_attribute("charset"),
                                if js::get(props, "charSet").is_none() {
                                    js::null()
                                } else {
                                    js::get(props, "charSet")
                                },
                            )
                        {
                            // mismatch, try the next node;
                            continue;
                        }
                        instance = node;
                        nodes.remove(i);
                        break 'get_instance;
                    }
                }
                instance = owner_document.create_element_named(r#type, None);
                set_initial_properties(instance, r#type, props);
                owner_document.head().unwrap().append_child(instance);
            }
            _ => {
                panic!(
                    "getNodesForType encountered a type it did not expect: \"{type}\". This is a bug in React."
                );
            }
        }
    }

    // This node is a match
    precache_fiber_node(internal_instance_handle, js::unknown(instance));
    mark_node_as_hoistable(instance);
    instance
}

fn get_hydratable_hoistable_cache(
    r#type: &str,
    key_attribute: &str,
    owner_document: &'static Document,
) -> &'static KeyedTagCache {
    let cache: &'static KeyedTagCache;
    let caches: &'static DocumentTagCaches;
    match TAG_CACHES.get() {
        None => {
            cache = js::map::new(None);
            caches = js::map::new(None);
            TAG_CACHES.set(Some(caches));
            caches.set(owner_document, cache);
        }
        Some(tag_caches) => {
            caches = tag_caches;
            let maybe_cache = caches.get(owner_document);
            match maybe_cache {
                None => {
                    cache = js::map::new(None);
                    caches.set(owner_document, cache);
                }
                Some(maybe_cache) => cache = maybe_cache,
            }
        }
    }

    if cache.has(r#type) {
        // We use type as a special key that signals that this cache has been seeded for this type
        return cache;
    }

    // Mark this cache as seeded for this type
    cache.set(r#type, None);

    let nodes = owner_document.get_elements_by_tag_name(r#type);
    for i in 0..nodes.length() as usize {
        let node = &nodes[i];
        if !is_owned_instance(node)
            && (r#type != "link" || node.get_attribute("rel").as_deref() != Some("stylesheet"))
            && node.namespace_uri().as_deref() != Some(SVG_NAMESPACE)
        {
            let node_key = node.get_attribute(key_attribute).unwrap_or_default();
            let key = r#type.to_owned() + &node_key;
            let existing = cache.get(&key).flatten();
            if let Some(existing) = existing {
                existing.borrow_mut().push(node);
            } else {
                cache.set(&key, Some(Rc::new(RefCell::new(vec![node]))));
            }
        }
    }

    cache
}

pub fn mount_hoistable(hoistable_root: &'static HoistableRoot, r#type: &str, instance: &Instance) {
    let owner_document = get_document_from_root(hoistable_root);
    owner_document.head().unwrap().insert_before(
        instance,
        if r#type == "title" {
            owner_document
                .query_selector("head > title")
                .map(|title| -> &Node { title })
        } else {
            None
        },
    );
}

pub fn unmount_hoistable(instance: &Instance) {
    instance.parent_node().unwrap().remove_child(instance);
}

pub fn is_host_hoistable_type(
    r#type: &str,
    props: &'static Unknown,
    host_context: &'static HostContext,
) -> bool {
    let mut outside_host_container_context = false;
    let host_context_prod: HostContextNamespace;
    if __DEV__ {
        let host_context_dev: &HostContextDev = unsafe { js::cast(Some(host_context)) };
        // We can only render resources when we are not within the host container context
        outside_host_container_context = host_context_dev
            .ancestor_info
            .as_ref()
            .unwrap()
            .container_tag_in_scope
            .is_none();
        host_context_prod = host_context_dev.context;
    } else {
        host_context_prod = unsafe { js::cast(Some(host_context)) };
    }

    // Global opt out of hoisting for anything in SVG Namespace or anything with an itemProp inside an itemScope
    if host_context_prod == HOST_CONTEXT_NAMESPACE_SVG || js::get(props, "itemProp").is_some() {
        if __DEV__
            && outside_host_container_context
            && js::get(props, "itemProp").is_some()
            && (r#type == "meta"
                || r#type == "title"
                || r#type == "style"
                || r#type == "link"
                || r#type == "script")
        {
            console::error((
                concat!(
                    "Cannot render a <%s> outside the main document if it has an `itemProp` prop. `itemProp` suggests the tag belongs to an",
                    " `itemScope` which can appear anywhere in the DOM. If you were intending for React to hoist this <%s> remove the `itemProp` prop.",
                    " Otherwise, try moving this tag into the <head> or <body> of the Document."
                ),
                r#type,
                r#type,
            ));
        }
        return false;
    }

    match r#type {
        "meta" | "title" => true,
        "style" => {
            if !matches!(js::classify(js::get(props, "precedence")), Kind::String(_))
                || !matches!(js::classify(js::get(props, "href")), Kind::String(_))
                || js::strict_equals(js::get(props, "href"), "")
            {
                if __DEV__ && outside_host_container_context {
                    console::error((concat!(
                        "Cannot render a <style> outside the main document without knowing its precedence and a unique href key.",
                        " React can hoist and deduplicate <style> tags if you provide a `precedence` prop along with an `href` prop that",
                        " does not conflict with the `href` values used in any other hoisted <style> or <link rel=\"stylesheet\" ...> tags. ",
                        " Note that hoisting <style> tags is considered an advanced feature that most will not use directly.",
                        " Consider moving the <style> tag to the <head> or consider adding a `precedence=\"default\"` and `href=\"some unique resource identifier\"`."
                    ),));
                }
                return false;
            }
            true
        }
        "link" => {
            if !matches!(js::classify(js::get(props, "rel")), Kind::String(_))
                || !matches!(js::classify(js::get(props, "href")), Kind::String(_))
                || js::strict_equals(js::get(props, "href"), "")
                || js::truthy(js::get(props, "onLoad"))
                || js::truthy(js::get(props, "onError"))
            {
                if __DEV__ {
                    if js::strict_equals(js::get(props, "rel"), "stylesheet")
                        && matches!(js::classify(js::get(props, "precedence")), Kind::String(_))
                    {
                        validate_link_props_for_style_resource(props);
                    }
                    if outside_host_container_context {
                        if !matches!(js::classify(js::get(props, "rel")), Kind::String(_))
                            || !matches!(js::classify(js::get(props, "href")), Kind::String(_))
                            || js::strict_equals(js::get(props, "href"), "")
                        {
                            console::error((concat!(
                                "Cannot render a <link> outside the main document without a `rel` and `href` prop.",
                                " Try adding a `rel` and/or `href` prop to this <link> or moving the link into the <head> tag"
                            ),));
                        } else if js::truthy(js::get(props, "onError"))
                            || js::truthy(js::get(props, "onLoad"))
                        {
                            console::error((concat!(
                                "Cannot render a <link> with onLoad or onError listeners outside the main document.",
                                " Try removing onLoad={...} and onError={...} or moving it into the root <head> tag or",
                                " somewhere in the <body>."
                            ),));
                        }
                    }
                }
                return false;
            }
            match js::classify(js::get(props, "rel")) {
                Kind::String("stylesheet") => {
                    let precedence = js::get(props, "precedence");
                    let disabled = js::get(props, "disabled");
                    if __DEV__
                        && !matches!(js::classify(precedence), Kind::String(_))
                        && outside_host_container_context
                    {
                        console::error((concat!(
                            "Cannot render a <link rel=\"stylesheet\" /> outside the main document without knowing its precedence.",
                            " Consider adding precedence=\"default\" or moving it into the root <head> tag."
                        ),));
                    }
                    matches!(js::classify(precedence), Kind::String(_)) && disabled.is_none()
                }
                _ => true,
            }
        }
        "script" => {
            let is_async = js::truthy(js::get(props, "async"))
                && !matches!(js::classify(js::get(props, "async")), Kind::Function(_))
                && !matches!(js::classify(js::get(props, "async")), Kind::Symbol(_));
            if !is_async
                || js::truthy(js::get(props, "onLoad"))
                || js::truthy(js::get(props, "onError"))
                || !js::truthy(js::get(props, "src"))
                || !matches!(js::classify(js::get(props, "src")), Kind::String(_))
            {
                if __DEV__ && outside_host_container_context {
                    if !is_async {
                        console::error((concat!(
                            "Cannot render a sync or defer <script> outside the main document without knowing its order.",
                            " Try adding async=\"\" or moving it into the root <head> tag."
                        ),));
                    } else if js::truthy(js::get(props, "onLoad"))
                        || js::truthy(js::get(props, "onError"))
                    {
                        console::error((concat!(
                            "Cannot render a <script> with onLoad or onError listeners outside the main document.",
                            " Try removing onLoad={...} and onError={...} or moving it into the root <head> tag or",
                            " somewhere in the <body>."
                        ),));
                    } else {
                        console::error((concat!(
                            "Cannot render a <script> outside the main document without `async={true}` and a non-empty `src` prop.",
                            " Ensure there is a valid `src` and either make the script async or move it into the root <head> tag or",
                            " somewhere in the <body>."
                        ),));
                    }
                }
                return false;
            }
            true
        }
        "noscript" | "template" => {
            if __DEV__ && outside_host_container_context {
                console::error((
                    "Cannot render <%s> outside the main document. Try moving it into the root <head> tag.",
                    r#type,
                ));
            }
            false
        }
        _ => false,
    }
}

pub fn may_suspend_commit(r#type: &str, props: &Props) -> bool {
    if !ENABLE_SUSPENSEY_IMAGES && !ENABLE_VIEW_TRANSITION {
        return false;
    }
    // Suspensey images are the default, unless you opt-out of with either
    // loading="lazy" or onLoad={...} which implies you're ok waiting.
    r#type == "img"
        && props.src.is_some()
        && !js::strict_equals(props.src, "")
        && props.on_load.is_none()
        && !js::strict_equals(props.loading, "lazy")
}

pub fn may_suspend_commit_on_update(r#type: &str, old_props: &Props, new_props: &Props) -> bool {
    may_suspend_commit(r#type, new_props)
        && (!js::strict_equals(new_props.src, old_props.src)
            || !js::strict_equals(new_props.src_set, old_props.src_set))
}

pub fn may_suspend_commit_in_sync_render(_type: &str, _props: &Props) -> bool {
    // TODO: Allow sync lanes to suspend too with an opt-in.
    false
}

pub fn may_resource_suspend_commit(resource: &Resource) -> bool {
    resource.r#type == "stylesheet"
        && (resource.state.as_ref().unwrap().loading.get() & INSERTED) == NOT_LOADED
}

pub fn preload_instance(instance: &'static Instance, _type: &str, _props: &Props) -> bool {
    // We don't need to preload Suspensey images because the browser will
    // load them early once we set the src.
    // If we return true here, we'll still get a suspendInstance call in the
    // pre-commit phase to determine if we still need to decode the image or
    // if was dropped from cache. This just avoids rendering Suspense fallback.
    js::truthy(js::get(instance, "complete"))
}

pub fn preload_resource(resource: &Resource) -> bool {
    if resource.r#type == "stylesheet"
        && (resource.state.as_ref().unwrap().loading.get() & SETTLED) == NOT_LOADED
    {
        // Return false to indicate this resource should suspend
        return false;
    }

    // Return true to indicate this resource should not suspend
    true
}

/// What a commit waits for before it starts: stylesheets, images and a
/// running view transition, and what to call once none is left.
pub struct SuspendedState {
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub stylesheets: Cell<Option<&'static Map<Resource, &'static HoistableRoot>>>,
    pub count: Cell<f64>,     // suspensey css and active view transitions
    pub img_count: Cell<f64>, // suspensey images pending to load
    pub img_bytes: Cell<f64>, // number of bytes we estimate needing to download
    pub suspensey_images: RefCell<Vec<&'static HTMLImageElement>>, // instances of suspensey images (whether loaded or not)
    pub waiting_for_images: Cell<bool>, // false when we're no longer blocking on images
    pub waiting_for_view_transition: Cell<bool>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub unsuspend: RefCell<Option<Box<dyn Fn()>>>,
}

unsafe impl js::Defined for SuspendedState {}

pub fn start_suspending_commit() -> Rc<SuspendedState> {
    Rc::new(SuspendedState {
        stylesheets: Cell::new(None),
        count: Cell::new(0.0),
        img_count: Cell::new(0.0),
        img_bytes: Cell::new(0.0),
        suspensey_images: RefCell::new(Vec::new()),
        waiting_for_images: Cell::new(true),
        waiting_for_view_transition: Cell::new(false),
        // We use a noop function when we begin suspending because if possible we want the
        // waitfor step to finish synchronously. If it doesn't we'll return a function to
        // provide the actual unsuspend function and that will get completed when the count
        // hits zero or it will get cancelled if the root starts new work.
        unsuspend: RefCell::new(Some(Box::new(noop))),
    })
}

fn estimate_image_bytes(instance: &HTMLImageElement) -> f64 {
    let width: f64 = if instance.width() != 0.0 {
        instance.width()
    } else {
        100.0
    };
    let height: f64 = if instance.height() != 0.0 {
        instance.height()
    } else {
        100.0
    };
    let pixel_ratio: f64 = if js::is_number(webapi::window_global::device_pixel_ratio) {
        webapi::window_global::device_pixel_ratio()
    } else {
        1.0
    };
    let pixels_to_download = width * height * pixel_ratio;
    const AVERAGE_BYTE_PER_PIXEL: f64 = 0.25;
    pixels_to_download * AVERAGE_BYTE_PER_PIXEL
}

pub fn suspend_instance(
    state: &Rc<SuspendedState>,
    instance: &'static Instance,
    _type: &str,
    _props: &Props,
) {
    if !ENABLE_SUSPENSEY_IMAGES && !ENABLE_VIEW_TRANSITION {
        return;
    }
    if
    // $FlowFixMe[prop-missing]
    js::is_function((
        html_image_element::unchecked_from(instance),
        HTMLImageElement::decode,
    )) {
        // If this browser supports decode() API, we use it to suspend waiting on the image.
        // The loading should have already started at this point, so it should be enough to
        // just call decode() which should also wait for the data to finish loading.
        state.img_count.set(state.img_count.get() + 1.0);
        // Estimate the byte size that we're about to download based on the width/height
        // specified in the props. This is best practice to know ahead of time but if it's
        // unspecified we'll fallback to a guess of 100x100 pixels.
        if !js::truthy(js::get(instance, "complete")) {
            state.img_bytes.set(
                state.img_bytes.get()
                    + estimate_image_bytes(html_image_element::unchecked_from(instance)),
            );
            state
                .suspensey_images
                .borrow_mut()
                .push(html_image_element::unchecked_from(instance));
        }
        let ping = function::bind(ON_UNSUSPEND_IMG.with(|f| *f), Some(state), ());
        // $FlowFixMe[prop-missing]
        html_image_element::unchecked_from(instance)
            .decode()
            .then_with::<(), _, _>(move |_| ping(), move |_| ping());
    }
}

pub fn suspend_resource(
    state: &Rc<SuspendedState>,
    hoistable_root: &'static HoistableRoot,
    resource: &Resource,
    props: &'static Unknown,
) {
    if resource.r#type == "stylesheet" {
        if let Kind::String(media) = js::classify(js::get(props, "media")) {
            // If we don't currently match media we avoid suspending on this resource
            // and let it insert on the mutation path
            if !match_media(media).matches() {
                return;
            }
        }
        if (resource.state.as_ref().unwrap().loading.get() & INSERTED) == NOT_LOADED {
            if resource.instance.get().is_none() {
                let qualified_props: &StylesheetQualifyingProps = unsafe { js::cast(Some(props)) };
                let key = get_style_key(&qualified_props.href);

                // Attempt to hydrate instance from DOM
                let instance =
                    hoistable_root.query_selector(&get_stylesheet_selector_from_key(&key));
                if let Some(instance) = instance {
                    // If this instance has a loading state it came from the Fizz runtime.
                    // If there is not loading state it is assumed to have been server rendered
                    // as part of the preamble and therefore synchronously loaded. It could have
                    // errored however which we still do not yet have a means to detect. For now
                    // we assume it is loaded.
                    let maybe_loading_state = js::get(instance, "_p");
                    if matches!(js::classify(maybe_loading_state), Kind::Object(_) | Kind::Array(_))
                        // $FlowFixMe[method-unbinding]
                        && matches!(js::classify(js::get(maybe_loading_state, "then")), Kind::Function(_))
                    {
                        let loading_state: Promise<Option<&'static Unknown>> =
                            unsafe { js::cast(maybe_loading_state) };
                        state.count.set(state.count.get() + 1.0);
                        let ping = function::bind(ON_UNSUSPEND.with(|f| *f), Some(state), ());
                        loading_state.then_with::<(), _, _>(move |_| ping(), move |_| ping());
                    }
                    let resource_state = resource.state.as_ref().unwrap();
                    resource_state
                        .loading
                        .set(resource_state.loading.get() | INSERTED);
                    resource.instance.set(Some(instance));
                    mark_node_as_hoistable(instance);
                    return;
                }

                let owner_document = get_document_from_root(hoistable_root);

                let stylesheet_props = stylesheet_props_from_raw_props(qualified_props);
                let preload_props =
                    PRELOAD_PROPS_MAP.with(|preload_props_map| preload_props_map.get(&key));
                if let Some(preload_props) = preload_props {
                    adopt_preload_props_for_stylesheet(stylesheet_props, preload_props);
                }

                // Construct and insert a new instance
                let instance = owner_document.create_element_named("link", None);
                mark_node_as_hoistable(instance);
                // This Promise is a loading state used by the Fizz runtime. We need this incase there is a race
                // between this resource being rendered on the client and being rendered with a late completed boundary.
                let link_instance: &HTMLLinkElement = html_link_element::unchecked_from(instance);
                js::set(
                    link_instance,
                    "_p",
                    js::promise::new(|resolve: &'static dyn Fn(&'static Event), reject| {
                        link_instance.set_onload(Some(&resolve));
                        link_instance.set_onerror(Some(&reject));
                    }),
                );
                set_initial_properties(instance, "link", stylesheet_props);
                resource.instance.set(Some(instance));
            }

            if state.stylesheets.get().is_none() {
                state.stylesheets.set(Some(js::map::new(None)));
            }
            state
                .stylesheets
                .get()
                .unwrap()
                .set(resource, hoistable_root);

            let preload_el = resource.state.as_ref().unwrap().preload.get();
            if let Some(preload_el) = preload_el
                && (resource.state.as_ref().unwrap().loading.get() & SETTLED) == NOT_LOADED
            {
                state.count.set(state.count.get() + 1.0);
                let ping = function::bind(ON_UNSUSPEND.with(|f| *f), Some(state), ());
                preload_el.add_event_listener_named("load", move |_: &Event| ping(), None);
                preload_el.add_event_listener_named("error", move |_: &Event| ping(), None);
            }
        }
    }
}

pub fn suspend_on_active_view_transition(
    state: &Rc<SuspendedState>,
    root_container: &'static Container,
) {
    let owner_document: &Document = if root_container.node_type() == DOCUMENT_NODE {
        webapi::document::unchecked_from(root_container)
    } else {
        root_container.owner_document().unwrap()
    };
    // $FlowFixMe[prop-missing]
    let active_view_transition: Option<&'static ViewTransition> =
        unsafe { js::cast(js::get(owner_document, "__reactViewTransition")) };
    let Some(active_view_transition) = active_view_transition else {
        return;
    };
    state.count.set(state.count.get() + 1.0);
    state.waiting_for_view_transition.set(true);
    let ping = function::bind(ON_UNSUSPEND.with(|f| *f), Some(state), ());
    active_view_transition
        .finished()
        .then_with::<(), _, _>(move |_| ping(), move |_| ping());
}

const SUSPENSEY_STYLESHEET_TIMEOUT: f64 = 60000.0;

const SUSPENSEY_IMAGE_TIMEOUT: f64 = 800.0;

const SUSPENSEY_IMAGE_TIME_ESTIMATE: f64 = 500.0;

thread_local! {
    static ESTIMATED_BYTES_WITHIN_LIMIT: Cell<f64> = const { Cell::new(0.0) };
}

/// What the reconciler calls with the commit to run once the commit may
/// start, and which gives what cancels it.
pub type WaitForCommit = Box<dyn Fn(Box<dyn Fn()>) -> Box<dyn Fn()>>;

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn wait_for_commit_to_be_ready(
    state: &Rc<SuspendedState>,
    timeout_offset: f64,
) -> Option<WaitForCommit> {
    if let Some(stylesheets) = state.stylesheets.get()
        && state.count.get() == 0.0
    {
        // We are not currently blocked but we have not inserted all stylesheets.
        // If this insertion happens and loads or errors synchronously then we can
        // avoid suspending the commit. To do this we check the count again immediately after
        insert_suspended_stylesheets(state, stylesheets);
    }

    // We need to check the count again because the inserted stylesheets may have led to new
    // tasks to wait on.
    if state.count.get() > 0.0 || state.img_count.get() > 0.0 {
        let state = Rc::clone(state);
        return Some(Box::new(move |commit| {
            // We almost never want to show content before its styles have loaded. But
            // eventually we will give up and allow unstyled content. So this number is
            // somewhat arbitrary — big enough that you'd only reach it under
            // extreme circumstances.
            // TODO: Figure out what the browser engines do during initial page load and
            // consider aligning our behavior with that.
            let stylesheet_timer = js::set_timeout(
                {
                    let state = Rc::clone(&state);
                    move || {
                        if let Some(stylesheets) = state.stylesheets.get() {
                            insert_suspended_stylesheets(&state, stylesheets);
                        }
                        if let Some(unsuspend) = state.unsuspend.take() {
                            unsuspend();
                        }
                    }
                },
                Some(SUSPENSEY_STYLESHEET_TIMEOUT + timeout_offset),
            );

            if state.img_bytes.get() > 0.0 && ESTIMATED_BYTES_WITHIN_LIMIT.get() == 0.0 {
                // Estimate how many bytes we can download in 500ms.
                let mbps = estimate_bandwidth();
                ESTIMATED_BYTES_WITHIN_LIMIT.set(mbps * 125.0 * SUSPENSEY_IMAGE_TIME_ESTIMATE);
            }
            // If we have more images to download than we expect to fit in the timeout, then
            // don't wait for images longer than 50ms. The 50ms lets us still do decoding and
            // hitting caches if it turns out that they're already in the HTTP cache.
            let img_timeout = if state.img_bytes.get() > ESTIMATED_BYTES_WITHIN_LIMIT.get() {
                50.0
            } else {
                SUSPENSEY_IMAGE_TIMEOUT
            };
            let img_timer = js::set_timeout(
                {
                    let state = Rc::clone(&state);
                    move || {
                        // We're no longer blocked on images. If CSS resolves after this we can commit.
                        state.waiting_for_images.set(false);
                        if state.count.get() == 0.0 {
                            if let Some(stylesheets) = state.stylesheets.get() {
                                insert_suspended_stylesheets(&state, stylesheets);
                            }
                            if let Some(unsuspend) = state.unsuspend.take() {
                                unsuspend();
                            }
                        }
                    }
                },
                Some(img_timeout + timeout_offset),
            );

            *state.unsuspend.borrow_mut() = Some(commit);

            let state = Rc::clone(&state);
            Box::new(move || {
                *state.unsuspend.borrow_mut() = None;
                js::clear_timeout(Some(stylesheet_timer));
                js::clear_timeout(Some(img_timer));
            })
        }));
    }
    None
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_suspended_commit_reason(
    state: &SuspendedState,
    _root_container: &Container,
) -> Option<&'static str> {
    if state.waiting_for_view_transition.get() {
        return Some("Waiting for the previous Animation");
    }
    if state.count.get() > 0.0 {
        if state.img_count.get() > 0.0 {
            return Some("Suspended on CSS and Images");
        }
        return Some("Suspended on CSS");
    }
    if state.img_count.get() == 1.0 {
        return Some("Suspended on an Image");
    }
    if state.img_count.get() > 0.0 {
        return Some("Suspended on Images");
    }
    None
}

fn check_if_fully_unsuspended(state: &Rc<SuspendedState>) {
    if state.count.get() == 0.0 && (state.img_count.get() == 0.0 || !state.waiting_for_images.get())
    {
        if let Some(stylesheets) = state.stylesheets.get() {
            // If we haven't actually inserted the stylesheets yet we need to do so now before starting the commit.
            // The reason we do this after everything else has finished is because we want to have all the stylesheets
            // load synchronously right before mutating. Ideally the new styles will cause a single recalc only on the
            // new tree. When we filled up stylesheets we only inlcuded stylesheets with matching media attributes so we
            // wait for them to load before actually continuing. We expect this to increase the count above zero
            insert_suspended_stylesheets(state, stylesheets);
        } else if let Some(unsuspend) = state.unsuspend.take() {
            unsuspend();
        }
    }
}

thread_local! {
    static ON_UNSUSPEND: &'static dyn Fn() = function::with_this(|this, ()| {
        let state: &Rc<SuspendedState> = unsafe { js::cast(this) };
        state.count.set(state.count.get() - 1.0);
        check_if_fully_unsuspended(state);
    });
    static ON_UNSUSPEND_IMG: &'static dyn Fn() = function::with_this(|this, ()| {
        let state: &Rc<SuspendedState> = unsafe { js::cast(this) };
        state.img_count.set(state.img_count.get() - 1.0);
        check_if_fully_unsuspended(state);
    });
}

// We use a value that is type distinct from precedence to track which one is last.
// This ensures there is no collision with user defined precedences. Normally we would
// just track this in module scope but since the precedences are tracked per HoistableRoot
// we need to associate it to something other than a global scope hence why we try to
// colocate it with the map of precedences in the first place
const LAST_PRECEDENCE: Option<&str> = js::null();

/// A root's last stylesheet of each precedence, and its last of all by
/// `LAST_PRECEDENCE`.
type Precedences = Map<Option<&'static str>, &'static Instance>;

thread_local! {
    // This is typecast to non-null because it will always be set before read.
    // it is important that this not be used except when the stack guarantees it exists.
    // Currentlyt his is only during insertSuspendedStylesheet.
    #[cfg_attr(rust_js, rust_js::nullable)]
    static PRECEDENCES_BY_ROOT: Cell<Option<&'static Map<HoistableRoot, &'static Precedences>>> =
        const { Cell::new(None) };
}

fn insert_suspended_stylesheets(
    state: &Rc<SuspendedState>,
    resources: &'static Map<Resource, &'static HoistableRoot>,
) {
    // We need to clear this out so we don't try to reinsert after the stylesheets have loaded
    state.stylesheets.set(None);

    if state.unsuspend.borrow().is_none() {
        // The suspended commit was cancelled. We don't need to insert any stylesheets.
        return;
    }

    // Temporarily increment count. we don't want any synchronously loaded stylesheets to try to unsuspend
    // before we finish inserting all stylesheets.
    state.count.set(state.count.get() + 1.0);

    PRECEDENCES_BY_ROOT.set(Some(js::map::new(None)));
    resources
        .for_each(|root, resource, map| insert_stylesheet_into_root(state, root, resource, map));
    PRECEDENCES_BY_ROOT.set(None);

    // We can remove our temporary count and if we're still at zero we can unsuspend.
    // If we are in the synchronous phase before deciding if the commit should suspend and this
    // ends up hitting the unsuspend path it will just invoke the noop unsuspend.
    function::call(ON_UNSUSPEND.with(|f| *f), Some(state), ());
}

fn insert_stylesheet_into_root(
    state: &Rc<SuspendedState>,
    root: &'static HoistableRoot,
    resource: &'static Resource,
    _map: &Map<Resource, &'static HoistableRoot>,
) {
    if (resource.state.as_ref().unwrap().loading.get() & INSERTED) != 0 {
        // This resource was inserted by another root committing. we don't need to insert it again
        return;
    }

    let mut last: Option<&'static Element> = None;
    let precedences_by_root = PRECEDENCES_BY_ROOT.get().unwrap();
    let precedences = match precedences_by_root.get(root) {
        None => {
            let precedences = js::map::new(None);
            precedences_by_root.set(root, precedences);
            let nodes = root.query_selector_all("link[data-precedence],style[data-precedence]");
            for i in 0..nodes.length() as usize {
                let node = &nodes[i];
                if node.node_name() == "LINK"
                    // We omit style tags with media="not all" because they are not in the right position
                    // and will be hoisted by the Fizz runtime imminently.
                    || node.get_attribute("media").as_deref() != Some("not all")
                {
                    precedences.set(
                        // $FlowFixMe[prop-missing]
                        &unsafe {
                            js::cast(js::get(
                                html_element::unchecked_from(node).dataset(),
                                "precedence",
                            ))
                        },
                        node,
                    );
                    last = Some(node);
                }
            }
            if let Some(last) = last {
                precedences.set(&LAST_PRECEDENCE, last);
            }
            precedences
        }
        Some(precedences) => {
            last = precedences.get(&LAST_PRECEDENCE);
            precedences
        }
    };

    // We only call this after we have constructed an instance so we assume it here
    let instance = resource.instance.get().unwrap();
    // We will always have a precedence for stylesheet instances
    let precedence = instance.get_attribute("data-precedence").unwrap();

    let prior = precedences.get(&Some(precedence.as_str())).or(last);
    if prior == last {
        precedences.set(&LAST_PRECEDENCE, instance);
    }
    precedences.set(&Some(precedence.as_str()), instance);

    state.count.set(state.count.get() + 1.0);
    let on_complete = function::bind(ON_UNSUSPEND.with(|f| *f), Some(state), ());
    instance.add_event_listener_named("load", move |_: &Event| on_complete(), None);
    instance.add_event_listener_named("error", move |_: &Event| on_complete(), None);

    if let Some(prior) = prior {
        prior
            .parent_node()
            .unwrap()
            .insert_before(instance, prior.next_sibling());
    } else {
        let parent: &Node = if root.node_type() == DOCUMENT_NODE {
            webapi::document::unchecked_from(root).head().unwrap()
        } else {
            root
        };
        parent.insert_before(instance, parent.first_child());
    }
    let resource_state = resource.state.as_ref().unwrap();
    resource_state
        .loading
        .set(resource_state.loading.get() | INSERTED);
}

pub type TransitionStatus = FormStatus;

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "NotPendingTransition")]
    pub static NOT_PENDING_TRANSITION: &'static TransitionStatus = NOT_PENDING.with(|not_pending| *not_pending);
    #[cfg_attr(rust_js, rust_js::name = "HostTransitionContext")]
    pub static HOST_TRANSITION_CONTEXT: ReactContext<&'static TransitionStatus> = ReactContext {
        r#typeof: REACT_CONTEXT_TYPE,
        provider: Cell::new(std::rc::Weak::new()),
        consumer: Cell::new(js::null()),
        current_value: RefCell::new(NOT_PENDING_TRANSITION.with(|not_pending_transition| *not_pending_transition)),
        current_value2: RefCell::new(NOT_PENDING_TRANSITION.with(|not_pending_transition| *not_pending_transition)),
        thread_count: Cell::new(0.0),
        current_renderer: Cell::new(None),
        current_renderer2: Cell::new(None),
    };
}

pub type FormInstance = HTMLFormElement;
pub fn reset_form_instance(form: &FormInstance) {
    react_browser_event_emitter_set_enabled(Some(true));
    form.reset();
    react_browser_event_emitter_set_enabled(Some(false));
}
