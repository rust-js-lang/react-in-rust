// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js::{Class, Map, Unknown};
use react_reconciler::react_event_priorities::{EventPriority, is_higher_event_priority};
use react_reconciler::react_fiber_reconciler::{
    attempt_continuous_hydration, attempt_hydration_at_current_priority,
};
use react_reconciler::react_fiber_shell_hydration::is_root_dehydrated;
use react_reconciler::react_fiber_tree_reflection::{
    get_activity_instance_from_fiber, get_container_from_fiber, get_nearest_mounted_fiber,
    get_suspense_instance_from_fiber,
};
use react_reconciler::react_internal_types::FiberRoot;
use react_reconciler::react_work_tags::{ACTIVITY_COMPONENT, HOST_ROOT, SUSPENSE_COMPONENT};
use scheduler::{
    Callback, UNSTABLE_NORMAL_PRIORITY as NORMAL_PRIORITY,
    unstable_schedule_callback as schedule_callback,
};
use shared::react_feature_flags::ENABLE_HYDRATION_CHANGE_EVENT;
use webapi::{
    Event, EventInit, EventTarget, InputEventInit, Node, PointerEvent, element, event, focus_event,
    html_input_element, input_event, mouse_event, pointer_event,
};

use crate::current_replaying_event::{reset_replaying_event, set_replaying_event};
use crate::dom_event_names::DOMEventName;
use crate::event_system_flags::EventSystemFlags;
use crate::form_action_event_plugin::dispatch_replayed_form_action;
use crate::plugin_module_type::AnyNativeEvent;
use crate::react_dom_component_tree::{
    get_closest_instance_from_node, get_fiber_current_props_from_node, get_instance_from_node,
};
use crate::react_dom_event_listener::{
    find_instance_blocking_event, find_instance_blocking_target,
};
use crate::react_dom_update_priority::{
    resolve_update_priority, run_with_priority as attempt_hydration_at_priority,
};

// TODO: Upgrade this definition once we're on a newer version of Flow that
// has this definition built-in.
type PointerEventType = PointerEvent;

struct QueuedReplayableEvent {
    #[cfg_attr(rust_js, rust_js::nullable)]
    blocked_on: Cell<Option<&'static Unknown>>,
    dom_event_name: DOMEventName,
    event_system_flags: Cell<EventSystemFlags>,
    native_event: &'static AnyNativeEvent,
    target_containers: RefCell<Vec<&'static EventTarget>>,
}

thread_local! {
    static HAS_SCHEDULED_REPLAY_ATTEMPT: Cell<bool> = const { Cell::new(false) };

    // The last of each continuous event type. We only need to replay the last one
    // if the last target was dehydrated.
    #[cfg_attr(rust_js, rust_js::nullable)]
    static QUEUED_FOCUS: RefCell<Option<Rc<QueuedReplayableEvent>>> = const { RefCell::new(None) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static QUEUED_DRAG: RefCell<Option<Rc<QueuedReplayableEvent>>> = const { RefCell::new(None) };
    #[cfg_attr(rust_js, rust_js::nullable)]
    static QUEUED_MOUSE: RefCell<Option<Rc<QueuedReplayableEvent>>> = const { RefCell::new(None) };
    // For pointer events there can be one latest event per pointerId.
    static QUEUED_POINTERS: &'static Map<f64, Rc<QueuedReplayableEvent>> = js::map::new(None);
    static QUEUED_POINTER_CAPTURES: &'static Map<f64, Rc<QueuedReplayableEvent>> = js::map::new(None);
    // We could consider replaying selectionchange and touchmoves too.

    static QUEUED_CHANGE_EVENT_TARGETS: RefCell<Vec<&'static EventTarget>> = const { RefCell::new(Vec::new()) };
}

struct QueuedHydrationTarget {
    #[cfg_attr(rust_js, rust_js::nullable)]
    blocked_on: Cell<Option<&'static Unknown>>,
    target: &'static Node,
    priority: EventPriority,
}

thread_local! {
    static QUEUED_EXPLICIT_HYDRATION_TARGETS: RefCell<Vec<Rc<QueuedHydrationTarget>>> = const { RefCell::new(Vec::new()) };

    static DISCRETE_REPLAYABLE_EVENTS: Vec<&'static str> = vec![
        "mousedown",
        "mouseup",
        "touchcancel",
        "touchend",
        "touchstart",
        "auxclick",
        "dblclick",
        "pointercancel",
        "pointerdown",
        "pointerup",
        "dragend",
        "dragstart",
        "drop",
        "compositionend",
        "compositionstart",
        "keydown",
        "keypress",
        "keyup",
        "input",
        "textInput", // Intentionally camelCase
        "copy",
        "cut",
        "paste",
        "click",
        "change",
        "contextmenu",
        "reset",
        // 'submit', // stopPropagation blocks the replay mechanism
    ];
}

pub fn is_discrete_event_that_requires_hydration(event_type: &str) -> bool {
    DISCRETE_REPLAYABLE_EVENTS
        .with(|discrete_replayable_events| discrete_replayable_events.contains(&event_type))
}

#[cfg_attr(rust_js, rust_js::nullable(blocked_on))]
fn create_queued_replayable_event(
    blocked_on: Option<&'static Unknown>,
    dom_event_name: &str,
    event_system_flags: EventSystemFlags,
    target_container: &'static EventTarget,
    native_event: &'static AnyNativeEvent,
) -> Rc<QueuedReplayableEvent> {
    Rc::new(QueuedReplayableEvent {
        blocked_on: Cell::new(blocked_on),
        dom_event_name: dom_event_name.to_owned(),
        event_system_flags: Cell::new(event_system_flags),
        native_event,
        target_containers: RefCell::new(vec![target_container]),
    })
}

// Resets the replaying for this type of continuous event to no event.
pub fn clear_if_continuous_event(dom_event_name: &str, native_event: &AnyNativeEvent) {
    match dom_event_name {
        "focusin" | "focusout" => {
            QUEUED_FOCUS.set(js::null());
        }
        "dragenter" | "dragleave" => {
            QUEUED_DRAG.set(js::null());
        }
        "mouseover" | "mouseout" => {
            QUEUED_MOUSE.set(js::null());
        }
        "pointerover" | "pointerout" => {
            let pointer_id = pointer_event::unchecked_from(native_event).pointer_id();
            QUEUED_POINTERS.with(|queued_pointers| queued_pointers.delete(&pointer_id));
        }
        "gotpointercapture" | "lostpointercapture" => {
            let pointer_id = pointer_event::unchecked_from(native_event).pointer_id();
            QUEUED_POINTER_CAPTURES
                .with(|queued_pointer_captures| queued_pointer_captures.delete(&pointer_id));
        }
        _ => {}
    }
}

#[cfg_attr(rust_js, rust_js::nullable(existing_queued_event, blocked_on))]
fn accumulate_or_create_continuous_queued_replayable_event(
    existing_queued_event: Option<Rc<QueuedReplayableEvent>>,
    blocked_on: Option<&'static Unknown>,
    dom_event_name: &str,
    event_system_flags: EventSystemFlags,
    target_container: &'static EventTarget,
    native_event: &'static AnyNativeEvent,
) -> Rc<QueuedReplayableEvent> {
    if let Some(existing_queued_event) = existing_queued_event
        && existing_queued_event.native_event == native_event
    {
        // If we have already queued this exact event, then it's because
        // the different event systems have different DOM event listeners.
        // We can accumulate the flags, and the targetContainers, and
        // store a single event to be replayed.
        existing_queued_event
            .event_system_flags
            .set(existing_queued_event.event_system_flags.get() | event_system_flags);
        let mut target_containers = existing_queued_event.target_containers.borrow_mut();
        if !target_containers.contains(&target_container) {
            target_containers.push(target_container);
        }
        drop(target_containers);
        return existing_queued_event;
    }
    let queued_event = create_queued_replayable_event(
        blocked_on,
        dom_event_name,
        event_system_flags,
        target_container,
        native_event,
    );
    if let Some(blocked_on) = blocked_on {
        let fiber = get_instance_from_node(unsafe { js::cast(Some(blocked_on)) });
        if let Some(fiber) = fiber {
            // Attempt to increase the priority of this target.
            attempt_continuous_hydration(&fiber);
        }
    }
    queued_event
}

#[cfg_attr(rust_js, rust_js::nullable(blocked_on))]
pub fn queue_if_continuous_event(
    blocked_on: Option<&'static Unknown>,
    dom_event_name: &str,
    event_system_flags: EventSystemFlags,
    target_container: &'static EventTarget,
    native_event: &'static AnyNativeEvent,
) -> bool {
    // These set relatedTarget to null because the replayed event will be treated as if we
    // moved from outside the window (no target) onto the target once it hydrates.
    // Instead of mutating we could clone the event.
    match dom_event_name {
        "focusin" => {
            let focus_event = focus_event::unchecked_from(native_event);
            QUEUED_FOCUS.set(Some(
                accumulate_or_create_continuous_queued_replayable_event(
                    QUEUED_FOCUS.with_borrow(Clone::clone),
                    blocked_on,
                    dom_event_name,
                    event_system_flags,
                    target_container,
                    focus_event,
                ),
            ));
            true
        }
        "dragenter" => {
            let drag_event = webapi::drag_event::unchecked_from(native_event);
            QUEUED_DRAG.set(Some(
                accumulate_or_create_continuous_queued_replayable_event(
                    QUEUED_DRAG.with_borrow(Clone::clone),
                    blocked_on,
                    dom_event_name,
                    event_system_flags,
                    target_container,
                    drag_event,
                ),
            ));
            true
        }
        "mouseover" => {
            let mouse_event = mouse_event::unchecked_from(native_event);
            QUEUED_MOUSE.set(Some(
                accumulate_or_create_continuous_queued_replayable_event(
                    QUEUED_MOUSE.with_borrow(Clone::clone),
                    blocked_on,
                    dom_event_name,
                    event_system_flags,
                    target_container,
                    mouse_event,
                ),
            ));
            true
        }
        "pointerover" => {
            let pointer_event: &PointerEventType = pointer_event::unchecked_from(native_event);
            let pointer_id = pointer_event.pointer_id();
            QUEUED_POINTERS.with(|queued_pointers| {
                queued_pointers.set(
                    &pointer_id,
                    accumulate_or_create_continuous_queued_replayable_event(
                        queued_pointers.get(&pointer_id),
                        blocked_on,
                        dom_event_name,
                        event_system_flags,
                        target_container,
                        pointer_event,
                    ),
                );
            });
            true
        }
        "gotpointercapture" => {
            let pointer_event: &PointerEventType = pointer_event::unchecked_from(native_event);
            let pointer_id = pointer_event.pointer_id();
            QUEUED_POINTER_CAPTURES.with(|queued_pointer_captures| {
                queued_pointer_captures.set(
                    &pointer_id,
                    accumulate_or_create_continuous_queued_replayable_event(
                        queued_pointer_captures.get(&pointer_id),
                        blocked_on,
                        dom_event_name,
                        event_system_flags,
                        target_container,
                        pointer_event,
                    ),
                );
            });
            true
        }
        _ => false,
    }
}

// Check if this target is unblocked. Returns true if it's unblocked.
fn attempt_explicit_hydration_target(queued_target: &QueuedHydrationTarget) {
    // TODO: This function shares a lot of logic with findInstanceBlockingEvent.
    // Try to unify them. It's a bit tricky since it would require two return
    // values.
    let target_inst = get_closest_instance_from_node(queued_target.target);
    if let Some(target_inst) = target_inst {
        let nearest_mounted = get_nearest_mounted_fiber(&target_inst);
        if let Some(nearest_mounted) = nearest_mounted {
            let tag = nearest_mounted.tag.get();
            if tag == SUSPENSE_COMPONENT {
                let instance = get_suspense_instance_from_fiber(&nearest_mounted);
                if instance.is_some() {
                    // We're blocked on hydrating this boundary.
                    // Increase its priority.
                    queued_target.blocked_on.set(instance);
                    attempt_hydration_at_priority(queued_target.priority, || {
                        attempt_hydration_at_current_priority(&nearest_mounted);
                    });

                    return;
                }
            } else if tag == ACTIVITY_COMPONENT {
                let instance = get_activity_instance_from_fiber(&nearest_mounted);
                if instance.is_some() {
                    // We're blocked on hydrating this boundary.
                    // Increase its priority.
                    queued_target.blocked_on.set(instance);
                    attempt_hydration_at_priority(queued_target.priority, || {
                        attempt_hydration_at_current_priority(&nearest_mounted);
                    });

                    return;
                }
            } else if tag == HOST_ROOT {
                let root: &FiberRoot = unsafe { js::cast(nearest_mounted.state_node.get()) };
                if is_root_dehydrated(root) {
                    queued_target
                        .blocked_on
                        .set(get_container_from_fiber(&nearest_mounted));
                    // We don't currently have a way to increase the priority of
                    // a root other than sync.
                    return;
                }
            }
        }
    }
    queued_target.blocked_on.set(js::null());
}

pub fn queue_explicit_hydration_target(target: &'static Node) {
    let update_priority = resolve_update_priority();
    let queued_target = Rc::new(QueuedHydrationTarget {
        blocked_on: Cell::new(js::null()),
        target,
        priority: update_priority,
    });
    let mut i = 0;
    while let Some(queued_explicit_hydration_target) = QUEUED_EXPLICIT_HYDRATION_TARGETS
        .with_borrow(|queued_explicit_hydration_targets| {
            queued_explicit_hydration_targets.get(i).cloned()
        })
    {
        // Stop once we hit the first target with lower priority than
        if !is_higher_event_priority(update_priority, queued_explicit_hydration_target.priority) {
            break;
        }
        i += 1;
    }
    QUEUED_EXPLICIT_HYDRATION_TARGETS.with_borrow_mut(|queued_explicit_hydration_targets| {
        queued_explicit_hydration_targets.insert(i, Rc::clone(&queued_target));
    });
    if i == 0 {
        attempt_explicit_hydration_target(&queued_target);
    }
}

fn attempt_replay_continuous_queued_event(queued_event: &QueuedReplayableEvent) -> bool {
    if queued_event.blocked_on.get().is_some() {
        return false;
    }
    let target_containers = &queued_event.target_containers;
    while !target_containers.borrow().is_empty() {
        let next_blocked_on = find_instance_blocking_event(queued_event.native_event);
        if next_blocked_on.is_none() {
            let native_event = queued_event.native_event;
            let native_event_clone = js::new(
                unsafe { js::cast::<&Unknown>(js::get(native_event, "constructor")) },
                (native_event.r#type(), native_event),
            );
            set_replaying_event(unsafe { js::cast(Some(native_event_clone)) });
            native_event
                .target()
                .unwrap()
                .dispatch_event(unsafe { js::cast(Some(native_event_clone)) });
            reset_replaying_event();
        } else {
            // We're still blocked. Try again later.
            let fiber = get_instance_from_node(unsafe { js::cast(next_blocked_on) });
            if let Some(fiber) = fiber {
                attempt_continuous_hydration(&fiber);
            }
            queued_event.blocked_on.set(next_blocked_on);
            return false;
        }
        // This target container was successfully dispatched. Try the next.
        target_containers.borrow_mut().remove(0);
    }
    true
}

fn attempt_replay_continuous_queued_event_in_map(
    queued_event: Rc<QueuedReplayableEvent>,
    key: &f64,
    map: &Map<f64, Rc<QueuedReplayableEvent>>,
) {
    if attempt_replay_continuous_queued_event(&queued_event) {
        map.delete(key);
    }
}

fn replay_change_event(target: &'static EventTarget) {
    // Dispatch a fake "change" event for the input.
    let element = html_input_element::unchecked_from(target);
    if element.node_name() == "INPUT" {
        if element.r#type() == "checkbox" || element.r#type() == "radio" {
            // Checkboxes always fire a click event regardless of how the change was made.
            #[cfg_attr(rust_js, rust_js::name = "EventCtr")]
            let event_ctr: &Class = if js::is_function(pointer_event::new) {
                js::class::<PointerEvent>()
            } else {
                js::class::<Event>()
            };
            target.dispatch_event(unsafe {
                js::cast(Some(js::new(
                    event_ctr,
                    (
                        "click",
                        &EventInit {
                            bubbles: Some(true),
                            ..Default::default()
                        },
                    ),
                )))
            });
            // For checkboxes the input event uses the Event constructor instead of InputEvent.
            target.dispatch_event(event::new(
                "input",
                Some(EventInit {
                    bubbles: Some(true),
                    ..Default::default()
                }),
            ));
        } else if js::is_function(input_event::new) {
            target.dispatch_event(input_event::new(
                "input",
                Some(InputEventInit {
                    bubbles: Some(true),
                    ..Default::default()
                }),
            ));
        }
    } else if element.node_name() == "TEXTAREA" && js::is_function(input_event::new) {
        target.dispatch_event(input_event::new(
            "input",
            Some(InputEventInit {
                bubbles: Some(true),
                ..Default::default()
            }),
        ));
    }
    target.dispatch_event(event::new(
        "change",
        Some(EventInit {
            bubbles: Some(true),
            ..Default::default()
        }),
    ));
}

fn replay_unblocked_events() {
    HAS_SCHEDULED_REPLAY_ATTEMPT.set(false);
    // Replay any continuous events.
    if let Some(queued_focus) = QUEUED_FOCUS.with_borrow(Clone::clone)
        && attempt_replay_continuous_queued_event(&queued_focus)
    {
        QUEUED_FOCUS.set(js::null());
    }
    if let Some(queued_drag) = QUEUED_DRAG.with_borrow(Clone::clone)
        && attempt_replay_continuous_queued_event(&queued_drag)
    {
        QUEUED_DRAG.set(js::null());
    }
    if let Some(queued_mouse) = QUEUED_MOUSE.with_borrow(Clone::clone)
        && attempt_replay_continuous_queued_event(&queued_mouse)
    {
        QUEUED_MOUSE.set(js::null());
    }
    QUEUED_POINTERS.with(|queued_pointers| {
        queued_pointers.for_each(attempt_replay_continuous_queued_event_in_map)
    });
    QUEUED_POINTER_CAPTURES.with(|queued_pointer_captures| {
        queued_pointer_captures.for_each(attempt_replay_continuous_queued_event_in_map)
    });
    if ENABLE_HYDRATION_CHANGE_EVENT
        .with(|enable_hydration_change_event| *enable_hydration_change_event)
    {
        let mut i = 0;
        while let Some(target) = QUEUED_CHANGE_EVENT_TARGETS
            .with_borrow(|queued_change_event_targets| queued_change_event_targets.get(i).copied())
        {
            replay_change_event(target);
            i += 1;
        }
        QUEUED_CHANGE_EVENT_TARGETS.with_borrow_mut(Vec::clear);
    }
}

pub fn flush_event_replaying() {
    // Synchronously flush any event replaying so that it gets observed before
    // any new updates are applied.
    if HAS_SCHEDULED_REPLAY_ATTEMPT.get() {
        replay_unblocked_events();
    }
}

pub fn queue_change_event(target: &'static EventTarget) {
    if ENABLE_HYDRATION_CHANGE_EVENT
        .with(|enable_hydration_change_event| *enable_hydration_change_event)
    {
        QUEUED_CHANGE_EVENT_TARGETS.with_borrow_mut(|queued_change_event_targets| {
            queued_change_event_targets.push(target)
        });
        if !HAS_SCHEDULED_REPLAY_ATTEMPT.get() {
            HAS_SCHEDULED_REPLAY_ATTEMPT.set(true);
        }
    }
}

fn schedule_callback_if_unblocked(queued_event: &QueuedReplayableEvent, unblocked: &Unknown) {
    if js::strict_equals(queued_event.blocked_on.get(), unblocked) {
        queued_event.blocked_on.set(js::null());
        if !HAS_SCHEDULED_REPLAY_ATTEMPT.get() {
            HAS_SCHEDULED_REPLAY_ATTEMPT.set(true);
            if !ENABLE_HYDRATION_CHANGE_EVENT
                .with(|enable_hydration_change_event| *enable_hydration_change_event)
            {
                // Schedule a callback to attempt replaying as many events as are
                // now unblocked. This first might not actually be unblocked yet.
                // We could check it early to avoid scheduling an unnecessary callback.
                schedule_callback(
                    NORMAL_PRIORITY,
                    Callback(Box::new(|_| {
                        replay_unblocked_events();
                        None
                    })),
                    None,
                );
            }
        }
    }
}

thread_local! {
    // [form, submitter or action, formData...]
    #[cfg_attr(rust_js, rust_js::nullable)]
    static LAST_SCHEDULED_REPLAY_QUEUE: Cell<Option<&'static Unknown>> = const { Cell::new(None) };
}

fn replay_unblocked_form_actions(form_replaying_queue: &'static Unknown) {
    if js::strict_equals(LAST_SCHEDULED_REPLAY_QUEUE.get(), form_replaying_queue) {
        LAST_SCHEDULED_REPLAY_QUEUE.set(js::null());
    }
    let mut i = 0.0;
    while i < unsafe { js::cast::<f64>(js::get(form_replaying_queue, "length")) } {
        let form = js::get(form_replaying_queue, &i);
        let submitter_or_action = js::get(form_replaying_queue, &(i + 1.0));
        let form_data = js::get(form_replaying_queue, &(i + 2.0));
        if js::type_of(submitter_or_action) != "function" {
            // This action is not hydrated yet. This might be because it's blocked on
            // a different React instance or higher up our tree.
            let blocked_on = find_instance_blocking_target(unsafe {
                js::cast(if js::truthy(submitter_or_action) {
                    submitter_or_action
                } else {
                    form
                })
            });
            if blocked_on.is_none() {
                // We're not blocked but we don't have an action. This must mean that
                // this is in another React instance. We'll just skip past it.
                i += 3.0;
                continue;
            } else {
                // We're blocked on something in this React instance. We'll retry later.
                break;
            }
        }
        let form_inst = get_instance_from_node(unsafe { js::cast(form) });
        if let Some(form_inst) = form_inst {
            // This is part of our instance.
            // We're ready to replay this. Let's delete it from the queue.
            js::array::splice(form_replaying_queue, i, 3.0);
            dispatch_replayed_form_action(
                &form_inst,
                unsafe { js::cast(form) },
                unsafe { js::cast(submitter_or_action) },
                unsafe { js::cast(form_data) },
            );
            // Continue without incrementing the index.
            continue;
        }
        // This form must've been part of a different React instance.
        // If we want to preserve ordering between React instances on the same root
        // we'd need some way for the other instance to ping us when it's done.
        // We'll just skip this and let the other instance execute it.
        i += 3.0;
    }
}

fn schedule_replay_queue_if_needed(form_replaying_queue: &'static Unknown) {
    // Schedule a callback to execute any unblocked form actions in.
    // We only keep track of the last queue which means that if multiple React oscillate
    // commits, we could schedule more callbacks than necessary but it's not a big deal
    // and we only really except one instance.
    if !js::strict_equals(LAST_SCHEDULED_REPLAY_QUEUE.get(), form_replaying_queue) {
        LAST_SCHEDULED_REPLAY_QUEUE.set(Some(form_replaying_queue));
        schedule_callback(
            NORMAL_PRIORITY,
            Callback(Box::new(move |_| {
                replay_unblocked_form_actions(form_replaying_queue);
                None
            })),
            None,
        );
    }
}

pub fn retry_if_blocked_on(unblocked: &'static Unknown) {
    if let Some(queued_focus) = QUEUED_FOCUS.with_borrow(Clone::clone) {
        schedule_callback_if_unblocked(&queued_focus, unblocked);
    }
    if let Some(queued_drag) = QUEUED_DRAG.with_borrow(Clone::clone) {
        schedule_callback_if_unblocked(&queued_drag, unblocked);
    }
    if let Some(queued_mouse) = QUEUED_MOUSE.with_borrow(Clone::clone) {
        schedule_callback_if_unblocked(&queued_mouse, unblocked);
    }
    let unblock = |queued_event: Rc<QueuedReplayableEvent>,
                   _: &f64,
                   _: &Map<f64, Rc<QueuedReplayableEvent>>| {
        schedule_callback_if_unblocked(&queued_event, unblocked)
    };
    QUEUED_POINTERS.with(|queued_pointers| queued_pointers.for_each(unblock));
    QUEUED_POINTER_CAPTURES
        .with(|queued_pointer_captures| queued_pointer_captures.for_each(unblock));

    QUEUED_EXPLICIT_HYDRATION_TARGETS.with_borrow(|queued_explicit_hydration_targets| {
        for queued_target in queued_explicit_hydration_targets {
            if js::strict_equals(queued_target.blocked_on.get(), unblocked) {
                queued_target.blocked_on.set(js::null());
            }
        }
    });

    while let Some(next_explicit_target) =
        QUEUED_EXPLICIT_HYDRATION_TARGETS.with_borrow(|queued_explicit_hydration_targets| {
            queued_explicit_hydration_targets.first().cloned()
        })
    {
        if next_explicit_target.blocked_on.get().is_some() {
            // We're still blocked.
            break;
        } else {
            attempt_explicit_hydration_target(&next_explicit_target);
            if next_explicit_target.blocked_on.get().is_none() {
                // We're unblocked.
                QUEUED_EXPLICIT_HYDRATION_TARGETS.with_borrow_mut(
                    |queued_explicit_hydration_targets| queued_explicit_hydration_targets.remove(0),
                );
            }
        }
    }

    // Check the document if there are any queued form actions.
    // If there's no ownerDocument, then this is the document.
    let owner_document = js::get(unblocked, "ownerDocument");
    let root = if js::truthy(owner_document) {
        owner_document
    } else {
        Some(unblocked)
    };
    let form_replaying_queue = js::get(root, "$$reactFormReplay");
    if let Some(form_replaying_queue) = form_replaying_queue {
        let mut i = 0.0;
        while i < unsafe { js::cast::<f64>(js::get(form_replaying_queue, "length")) } {
            let form = js::get(form_replaying_queue, &i);
            let submitter_or_action = js::get(form_replaying_queue, &(i + 1.0));
            let form_props = get_fiber_current_props_from_node(unsafe { js::cast(form) });
            if js::type_of(submitter_or_action) == "function" {
                // This action has already resolved. We're just waiting to dispatch it.
                if form_props.is_none() {
                    // This was not part of this React instance. It might have been recently
                    // unblocking us from dispatching our events. So let's make sure we schedule
                    // a retry.
                    schedule_replay_queue_if_needed(form_replaying_queue);
                }
                i += 3.0;
                continue;
            }
            if let Some(form_props) = form_props {
                // This form belongs to this React instance but the submitter might
                // not be done yet.
                let mut action = js::null();
                let submitter = submitter_or_action;
                if let Some(submitter) = submitter
                    && element::unchecked_from(unsafe { js::cast(Some(submitter)) })
                        .has_attribute("formAction")
                {
                    // The submitter is the one that is responsible for the action.
                    let target: &'static Node = unsafe { js::cast(Some(submitter)) };
                    let submitter_props = get_fiber_current_props_from_node(submitter);
                    if let Some(submitter_props) = submitter_props {
                        // The submitter is part of this instance.
                        action = js::get(submitter_props, "formAction");
                    } else {
                        let blocked_on = find_instance_blocking_target(target);
                        if blocked_on.is_some() {
                            // The submitter is not hydrated yet. We'll wait for it.
                            i += 3.0;
                            continue;
                        }
                        // The submitter must have been a part of a different React instance.
                        // Except the form isn't. We don't dispatch actions in this scenario.
                    }
                } else {
                    action = js::get(form_props, "action");
                }
                if js::type_of(action) == "function" {
                    js::set(form_replaying_queue, &(i + 1.0), action);
                    i += 3.0;
                } else {
                    // Something went wrong so let's just delete this action.
                    js::array::splice(form_replaying_queue, i, 3.0);
                }
                // Schedule a replay in case this unblocked something.
                schedule_replay_queue_if_needed(form_replaying_queue);
                continue;
            }
            // Something above this target is still blocked so we can't continue yet.
            // We're not sure if this target is actually part of this React instance
            // yet. It could be a different React as a child but at least some parent is.
            // We must continue for any further queued actions.
            i += 3.0;
        }
    }
}
