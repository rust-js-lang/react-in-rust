// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::rc::Rc;

use js::{Class, Map, Set, TimeoutId, Unknown, WeakMap, math, number, string};
use react_reconciler::react_internal_types::Fiber;
use react_reconciler::react_work_tags::{
    ACTIVITY_COMPONENT, HOST_COMPONENT, HOST_HOISTABLE, HOST_ROOT, HOST_SINGLETON, HOST_TEXT,
    SUSPENSE_COMPONENT,
};
use shared::environment::__DEV__;
use shared::react_feature_flags::{ENABLE_INTERNAL_INSTANCE_MAP, ENABLE_SCOPE_API};
use webapi::{Element, EventTarget, Node};

use crate::react_dom_event_handle_types::{ReactDOMEventHandle, ReactDOMEventHandleListener};
use crate::react_fiber_config_dom::{RootResources, get_parent_hydration_boundary};

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "randomKey")]
    static RANDOM_KEY: String = string::slice(&number::to_string(math::random(), Some(36)), 2.0, None);
    #[cfg_attr(rust_js, rust_js::name = "internalInstanceKey")]
    static INTERNAL_INSTANCE_KEY: String = RANDOM_KEY.with(|random_key| "__reactFiber$".to_owned() + random_key);
    #[cfg_attr(rust_js, rust_js::name = "internalPropsKey")]
    static INTERNAL_PROPS_KEY: String = RANDOM_KEY.with(|random_key| "__reactProps$".to_owned() + random_key);
    #[cfg_attr(rust_js, rust_js::name = "internalContainerInstanceKey")]
    static INTERNAL_CONTAINER_INSTANCE_KEY: String = RANDOM_KEY.with(|random_key| "__reactContainer$".to_owned() + random_key);
    #[cfg_attr(rust_js, rust_js::name = "internalEventHandlersKey")]
    static INTERNAL_EVENT_HANDLERS_KEY: String = RANDOM_KEY.with(|random_key| "__reactEvents$".to_owned() + random_key);
    #[cfg_attr(rust_js, rust_js::name = "internalEventHandlerListenersKey")]
    static INTERNAL_EVENT_HANDLER_LISTENERS_KEY: String = RANDOM_KEY.with(|random_key| "__reactListeners$".to_owned() + random_key);
    #[cfg_attr(rust_js, rust_js::name = "internalEventHandlesSetKey")]
    static INTERNAL_EVENT_HANDLES_SET_KEY: String = RANDOM_KEY.with(|random_key| "__reactHandles$".to_owned() + random_key);
    #[cfg_attr(rust_js, rust_js::name = "internalRootNodeResourcesKey")]
    static INTERNAL_ROOT_NODE_RESOURCES_KEY: String = RANDOM_KEY.with(|random_key| "__reactResources$".to_owned() + random_key);
    #[cfg_attr(rust_js, rust_js::name = "internalHoistableMarker")]
    static INTERNAL_HOISTABLE_MARKER: String = RANDOM_KEY.with(|random_key| "__reactMarker$".to_owned() + random_key);
    #[cfg_attr(rust_js, rust_js::name = "internalScrollTimer")]
    static INTERNAL_SCROLL_TIMER: String = RANDOM_KEY.with(|random_key| "__reactScroll$".to_owned() + random_key);
    #[cfg_attr(rust_js, rust_js::name = "internalLoadPendingKey")]
    static INTERNAL_LOAD_PENDING_KEY: String = RANDOM_KEY.with(|random_key| "__reactLoad$".to_owned() + random_key);
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "PossiblyWeakMap")]
    static POSSIBLY_WEAK_MAP: &'static Class = if js::is_function(js::weak_map::new::<js::JsObject, ()>) {
        js::class::<WeakMap<js::JsObject, ()>>()
    } else {
        js::class::<Map<Unknown, ()>>()
    };
    static INTERNAL_INSTANCE_MAP: &'static Map<Unknown, Rc<Fiber>> =
        unsafe { js::cast(Some(js::new(POSSIBLY_WEAK_MAP.with(|c| *c), ()))) };
    static INTERNAL_PROPS_MAP: &'static Map<Unknown, &'static Unknown> =
        unsafe { js::cast(Some(js::new(POSSIBLY_WEAK_MAP.with(|c| *c), ()))) };
}

pub fn detach_deleted_instance(node: &Element) {
    // Don't delete the event listener set. The native event listeners it tracks
    // remain attached to the node, so this bookkeeping needs to last for the
    // lifetime of the node to prevent duplicate listeners if it is reused.
    if ENABLE_INTERNAL_INSTANCE_MAP {
        INTERNAL_INSTANCE_MAP.with(|map| map.delete(js::unknown(node)));
        INTERNAL_PROPS_MAP.with(|map| map.delete(js::unknown(node)));
        INTERNAL_EVENT_HANDLER_LISTENERS_KEY.with(|key| js::delete(node, key));
        INTERNAL_EVENT_HANDLES_SET_KEY.with(|key| js::delete(node, key));
        INTERNAL_ROOT_NODE_RESOURCES_KEY.with(|key| js::delete(node, key));
        if __DEV__ {
            INTERNAL_INSTANCE_KEY.with(|key| js::delete(node, key));
        }
        return;
    }
    // TODO: This function is only called on host components. I don't think all of
    // these fields are relevant.
    INTERNAL_INSTANCE_KEY.with(|key| js::delete(node, key));
    INTERNAL_PROPS_KEY.with(|key| js::delete(node, key));
    INTERNAL_EVENT_HANDLER_LISTENERS_KEY.with(|key| js::delete(node, key));
    INTERNAL_EVENT_HANDLES_SET_KEY.with(|key| js::delete(node, key));
}

pub fn precache_fiber_node(host_inst: &Rc<Fiber>, node: &Unknown) {
    if ENABLE_INTERNAL_INSTANCE_MAP {
        INTERNAL_INSTANCE_MAP.with(|map| map.set(node, Rc::clone(host_inst)));
        if __DEV__ {
            INTERNAL_INSTANCE_KEY.with(|key| js::set(node, key, host_inst));
        }
        return;
    }
    INTERNAL_INSTANCE_KEY.with(|key| js::set(node, key, host_inst));
}

pub fn mark_container_as_root(host_root: &Rc<Fiber>, node: &Unknown) {
    INTERNAL_CONTAINER_INSTANCE_KEY.with(|key| js::set(node, key, host_root));
}

pub fn unmark_container_as_root(node: &Unknown) {
    INTERNAL_CONTAINER_INSTANCE_KEY.with(|key| js::set(node, key, js::null::<&Unknown>()));
}

pub fn is_container_marked_as_root(node: &Unknown) -> bool {
    INTERNAL_CONTAINER_INSTANCE_KEY.with(|key| js::truthy(js::get(node, key)))
}

// Given a DOM node, return the closest HostComponent or HostText fiber ancestor.
// If the target node is part of a hydrated or not yet rendered subtree, then
// this may also return a SuspenseComponent, ActivityComponent or HostRoot to
// indicate that.
// Conceptually the HostRoot fiber is a child of the Container node. So if you
// pass the Container node as the targetNode, you will not actually get the
// HostRoot back. To get to the HostRoot, you need to pass a child of it.
// The same thing applies to Suspense and Activity boundaries.
#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_closest_instance_from_node(mut target_node: &'static EventTarget) -> Option<Rc<Fiber>> {
    let mut target_inst = if ENABLE_INTERNAL_INSTANCE_MAP {
        INTERNAL_INSTANCE_MAP.with(|map| map.get(js::unknown(target_node)))
    } else {
        INTERNAL_INSTANCE_KEY
            .with(|key| unsafe { js::cast(js::get(js::unknown(target_node), key)) })
    };
    if target_inst.is_some() {
        // Don't return HostRoot, SuspenseComponent or ActivityComponent here.
        return target_inst;
    }
    // If the direct event target isn't a React owned DOM node, we need to look
    // to see if one of its parents is a React owned DOM node.
    let mut parent_node: Option<&'static EventTarget> =
        unsafe { js::cast(js::get(target_node, "parentNode")) };
    while let Some(parent) = parent_node {
        // We'll check if this is a container root that could include
        // React nodes in the future. We need to check this first because
        // if we're a child of a dehydrated container, we need to first
        // find that inner container before moving on to finding the parent
        // instance. Note that we don't check this field on  the targetNode
        // itself because the fibers are conceptually between the container
        // node and the first child. It isn't surrounding the container node.
        // If it's not a container, we check if it's an instance.
        target_inst = if ENABLE_INTERNAL_INSTANCE_MAP {
            INTERNAL_CONTAINER_INSTANCE_KEY
                .with(|key| unsafe {
                    js::cast::<Option<Rc<Fiber>>>(js::get(js::unknown(parent), key))
                })
                .or_else(|| INTERNAL_INSTANCE_MAP.with(|map| map.get(js::unknown(parent))))
        } else {
            INTERNAL_CONTAINER_INSTANCE_KEY
                .with(|key| unsafe {
                    js::cast::<Option<Rc<Fiber>>>(js::get(js::unknown(parent), key))
                })
                .or_else(|| {
                    INTERNAL_INSTANCE_KEY
                        .with(|key| unsafe { js::cast(js::get(js::unknown(parent), key)) })
                })
        };
        if let Some(target_inst) = target_inst {
            // Since this wasn't the direct target of the event, we might have
            // stepped past dehydrated DOM nodes to get here. However they could
            // also have been non-React nodes. We need to answer which one.

            // If we the instance doesn't have any children, then there can't be
            // a nested suspense boundary within it. So we can use this as a fast
            // bailout. Most of the time, when people add non-React children to
            // the tree, it is using a ref to a child-less DOM node.
            // Normally we'd only need to check one of the fibers because if it
            // has ever gone from having children to deleting them or vice versa
            // it would have deleted the dehydrated boundary nested inside already.
            // However, since the HostRoot starts out with an alternate it might
            // have one on the alternate so we need to check in case this was a
            // root.
            let alternate = target_inst.alternate.borrow().clone();
            if target_inst.child.borrow().is_some()
                || alternate.is_some_and(|alternate| alternate.child.borrow().is_some())
            {
                // Next we need to figure out if the node that skipped past is
                // nested within a dehydrated boundary and if so, which one.
                let mut hydration_instance =
                    get_parent_hydration_boundary(webapi::node::unchecked_from(target_node));
                while let Some(instance) = hydration_instance {
                    // We found a suspense instance. That means that we haven't
                    // hydrated it yet. Even though we leave the comments in the
                    // DOM after hydrating, and there are boundaries in the DOM
                    // that could already be hydrated, we wouldn't have found them
                    // through this pass since if the target is hydrated it would
                    // have had an internalInstanceKey on it.
                    // Let's get the fiber associated with the SuspenseComponent
                    // as the deepest instance.
                    let target_fiber = if ENABLE_INTERNAL_INSTANCE_MAP {
                        INTERNAL_INSTANCE_MAP.with(|map| map.get(js::unknown(instance)))
                    } else {
                        INTERNAL_INSTANCE_KEY
                            .with(|key| unsafe { js::cast(js::get(instance, key)) })
                    };
                    if target_fiber.is_some() {
                        return target_fiber;
                    }
                    // If we don't find a Fiber on the comment, it might be because
                    // we haven't gotten to hydrate it yet. There might still be a
                    // parent boundary that hasn't above this one so we need to find
                    // the outer most that is known.
                    hydration_instance = get_parent_hydration_boundary(instance);
                    // If we don't find one, then that should mean that the parent
                    // host component also hasn't hydrated yet. We can return it
                    // below since it will bail out on the isMounted check later.
                }
            }
            return Some(target_inst);
        }
        target_node = parent;
        parent_node = unsafe { js::cast(js::get(target_node, "parentNode")) };
    }
    None
}

/// Given a DOM node, return the ReactDOMComponent or ReactDOMTextComponent
/// instance, or null if the node was not rendered by this React.
#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_instance_from_node(node: &Node) -> Option<Rc<Fiber>> {
    let inst = if ENABLE_INTERNAL_INSTANCE_MAP {
        INTERNAL_INSTANCE_MAP
            .with(|map| map.get(js::unknown(node)))
            .or_else(|| {
                INTERNAL_CONTAINER_INSTANCE_KEY
                    .with(|key| unsafe { js::cast(js::get(js::unknown(node), key)) })
            })
    } else {
        INTERNAL_INSTANCE_KEY
            .with(|key| unsafe { js::cast::<Option<Rc<Fiber>>>(js::get(js::unknown(node), key)) })
            .or_else(|| {
                INTERNAL_CONTAINER_INSTANCE_KEY
                    .with(|key| unsafe { js::cast(js::get(js::unknown(node), key)) })
            })
    };
    if let Some(inst) = inst {
        let tag = inst.tag.get();
        if tag == HOST_COMPONENT
            || tag == HOST_TEXT
            || tag == SUSPENSE_COMPONENT
            || tag == ACTIVITY_COMPONENT
            || tag == HOST_HOISTABLE
            || tag == HOST_SINGLETON
            || tag == HOST_ROOT
        {
            return Some(inst);
        } else {
            return None;
        }
    }
    None
}

/// Given a ReactDOMComponent or ReactDOMTextComponent, return the corresponding
/// DOM node.
pub fn get_node_from_instance(inst: &Fiber) -> Option<&'static Unknown> {
    let tag = inst.tag.get();
    if tag == HOST_COMPONENT || tag == HOST_HOISTABLE || tag == HOST_SINGLETON || tag == HOST_TEXT {
        // In Fiber this, is just the state node right now. We assume it will be
        // a host component or host text.
        return inst.state_node.get();
    }

    // Without this first invariant, passing a non-DOM-component triggers the next
    // invariant for a missing parent, which is super confusing.
    panic!("getNodeFromInstance: Invalid argument.");
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_fiber_current_props_from_node(node: &Unknown) -> Option<&'static Unknown> {
    if ENABLE_INTERNAL_INSTANCE_MAP {
        return INTERNAL_PROPS_MAP.with(|map| map.get(node));
    }
    INTERNAL_PROPS_KEY.with(|key| js::get(node, key))
}

pub fn update_fiber_props(node: &Element, props: &'static Unknown) {
    if ENABLE_INTERNAL_INSTANCE_MAP {
        INTERNAL_PROPS_MAP.with(|map| map.set(js::unknown(node), props));
        return;
    }
    INTERNAL_PROPS_KEY.with(|key| js::set(node, key, props));
}

pub fn get_event_listener_set(node: &EventTarget) -> &'static Set<String> {
    let element_listener_set: Option<&'static Set<String>> =
        INTERNAL_EVENT_HANDLERS_KEY.with(|key| unsafe { js::cast(js::get(node, key)) });
    if let Some(element_listener_set) = element_listener_set {
        return element_listener_set;
    }
    let element_listener_set = js::set::new(None);
    INTERNAL_EVENT_HANDLERS_KEY.with(|key| js::set(node, key, element_listener_set));
    element_listener_set
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_fiber_from_scope_instance(scope: &Unknown) -> Option<Rc<Fiber>> {
    if ENABLE_SCOPE_API {
        if ENABLE_INTERNAL_INSTANCE_MAP {
            return INTERNAL_INSTANCE_MAP.with(|map| map.get(scope));
        }
        return INTERNAL_INSTANCE_KEY.with(|key| unsafe { js::cast(js::get(scope, key)) });
    }
    None
}

pub fn set_event_handler_listeners(
    scope: &Unknown,
    listeners: &'static Set<ReactDOMEventHandleListener>,
) {
    INTERNAL_EVENT_HANDLER_LISTENERS_KEY.with(|key| js::set(scope, key, listeners));
}

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn get_event_handler_listeners(
    scope: &Unknown,
) -> Option<&'static Set<ReactDOMEventHandleListener>> {
    INTERNAL_EVENT_HANDLER_LISTENERS_KEY.with(|key| unsafe { js::cast(js::get(scope, key)) })
}

pub fn add_event_handle_to_target(target: &Unknown, event_handle: &ReactDOMEventHandle) {
    let event_handles: Option<&'static Set<ReactDOMEventHandle>> =
        INTERNAL_EVENT_HANDLES_SET_KEY.with(|key| unsafe { js::cast(js::get(target, key)) });
    let event_handles = match event_handles {
        Some(event_handles) => event_handles,
        None => {
            let event_handles = js::set::new(None);
            INTERNAL_EVENT_HANDLES_SET_KEY.with(|key| js::set(target, key, event_handles));
            event_handles
        }
    };
    event_handles.add(event_handle);
}

pub fn does_target_have_event_handle(target: &Unknown, event_handle: &ReactDOMEventHandle) -> bool {
    let event_handles: Option<&'static Set<ReactDOMEventHandle>> =
        INTERNAL_EVENT_HANDLES_SET_KEY.with(|key| unsafe { js::cast(js::get(target, key)) });
    let Some(event_handles) = event_handles else {
        return false;
    };
    event_handles.has(event_handle)
}

pub fn get_resources_from_root(root: &Unknown) -> Rc<RootResources> {
    let resources: Option<Rc<RootResources>> =
        INTERNAL_ROOT_NODE_RESOURCES_KEY.with(|key| unsafe { js::cast(js::get(root, key)) });
    if let Some(resources) = resources {
        return resources;
    }
    let resources = Rc::new(RootResources {
        hoistable_styles: js::map::new(None),
        hoistable_scripts: js::map::new(None),
    });
    INTERNAL_ROOT_NODE_RESOURCES_KEY.with(|key| js::set(root, key, &*resources));
    resources
}

pub fn is_marked_hoistable(node: &Node) -> bool {
    INTERNAL_HOISTABLE_MARKER.with(|key| js::truthy(js::get(node, key)))
}

pub fn mark_node_as_hoistable(node: &Node) {
    INTERNAL_HOISTABLE_MARKER.with(|key| js::set(node, key, true));
}

pub fn get_scroll_end_timer(node: &EventTarget) -> Option<&'static TimeoutId> {
    INTERNAL_SCROLL_TIMER.with(|key| unsafe { js::cast(js::get(node, key)) })
}

pub fn set_scroll_end_timer(node: &EventTarget, timer: &TimeoutId) {
    INTERNAL_SCROLL_TIMER.with(|key| js::set(node, key, timer));
}

pub fn clear_scroll_end_timer(node: &EventTarget) {
    INTERNAL_SCROLL_TIMER.with(|key| js::set(node, key, ()));
}

pub fn mark_node_as_pending_load(node: &Node) {
    INTERNAL_LOAD_PENDING_KEY.with(|key| js::set(node, key, true));
}

pub fn clear_pending_load_on_node(node: &Node) {
    INTERNAL_LOAD_PENDING_KEY.with(|key| js::set(node, key, ()));
}

pub fn is_node_pending_load(node: &Node) -> bool {
    INTERNAL_LOAD_PENDING_KEY.with(|key| js::strict_equals(js::get(node, key), true))
}

pub fn is_owned_instance(node: &Node) -> bool {
    if ENABLE_INTERNAL_INSTANCE_MAP {
        return INTERNAL_HOISTABLE_MARKER.with(|key| js::truthy(js::get(node, key)))
            || INTERNAL_INSTANCE_MAP.with(|map| map.has(js::unknown(node)));
    }
    INTERNAL_HOISTABLE_MARKER.with(|key| js::truthy(js::get(node, key)))
        || INTERNAL_INSTANCE_KEY.with(|key| js::truthy(js::get(node, key)))
}
