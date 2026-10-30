// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// Shared implementation and constants between the inline script and external
// runtime instruction sets.

use std::cell::{Cell, RefCell};
use std::panic::{self, AssertUnwindSafe};

use js::{Kind, Unknown, function};
use webapi::events::{Error, Load, Submit};
use webapi::window_global::request_animation_frame;
use webapi::{
    Element, Event, EventTargetExt, Node, StartViewTransitionOptions, SubmitEvent, ViewTransition,
    comment, css, document, element, form_data, global, html_form_element, html_image_element,
    node_list, performance, tags, window, window_global,
};

const ELEMENT_NODE: f64 = 1.0;
const COMMENT_NODE: f64 = 8.0;
const ACTIVITY_START_DATA: &str = "&";
const ACTIVITY_END_DATA: &str = "/&";
const SUSPENSE_START_DATA: &str = "$";
const SUSPENSE_END_DATA: &str = "/$";
const SUSPENSE_PENDING_START_DATA: &str = "$?";
const SUSPENSE_QUEUED_START_DATA: &str = "$~";
const SUSPENSE_FALLBACK_START_DATA: &str = "$!";

const FALLBACK_THROTTLE_MS: f64 = 300.0;

const SUSPENSEY_FONT_AND_IMAGE_TIMEOUT: f64 = 500.0;

// If you have a target goal in mind for a metric to hit, you don't want the
// only reason you miss it by a little bit to be throttling heuristics.
// This tries to avoid throttling if avoiding it would let you hit this metric.
// This is derived from trying to hit an LCP of 2.5 seconds with some head room.
const TARGET_VANITY_METRIC: f64 = 2300.0;

/// The boundaries completed and not yet revealed: each one's first node and
/// its content's, one after the other.
pub type Batch = RefCell<Vec<&'static Element>>;

/// What a completed boundary is given, its first node's id and its content's.
pub type CompleteBoundary = dyn Fn(&str, &str);
/// What a boundary rendered on the client is given, its first node's id and its error's.
pub type ClientRenderBoundary =
    dyn Fn(&str, Option<&str>, Option<&str>, Option<&str>, Option<&str>);
/// What a boundary waiting on its stylesheets is given, and their descriptors.
pub type CompleteBoundaryWithStyles = dyn Fn(&str, &str, &[Vec<String>]);
/// What reveals a batch of completed boundaries.
pub type RevealBoundaries = dyn Fn(&'static Batch);

// TODO: Symbols that are referenced outside this module use dynamic accessor
// notation instead of dot notation to prevent Closure's advanced compilation
// mode from renaming. We could use extern files instead, but I couldn't get it
// working. Closure converts it to a dot access anyway, though, so it's not an
// urgent issue.
unsafe extern "Rust" {
    /// When the shell painted.
    #[link_name = "window.$RT"]
    pub(crate) safe static RT: Option<&'static Unknown>;
    /// The boundaries completed and not yet revealed.
    #[link_name = "window.$RB"]
    pub(crate) safe static RB: &'static Batch;
    #[link_name = "window.$RV"]
    pub(crate) safe static RV: &'static RevealBoundaries;
    /// Each stylesheet's element, by its href.
    #[link_name = "window.$RM"]
    pub(crate) safe static RM: &'static js::Map<Option<String>, &'static Element>;
    #[link_name = "window.$RC"]
    pub(crate) safe static RC: &'static CompleteBoundary;
    #[link_name = "window.$RX"]
    pub(crate) safe static RX: &'static ClientRenderBoundary;
    #[link_name = "window.$RR"]
    pub(crate) safe static RR: &'static CompleteBoundaryWithStyles;
    #[link_name = "window.$RS"]
    pub(crate) safe static RS: &'static CompleteBoundary;
}

#[cfg_attr(rust_js, rust_js::link_name = "set window.$RT")]
pub(crate) fn set_rt(time: f64) {
    let _ = time;
    unreachable!()
}

#[cfg_attr(rust_js, rust_js::link_name = "set window.$RB")]
pub(crate) fn set_rb(batch: Batch) {
    let _ = batch;
    unreachable!()
}

#[cfg_attr(rust_js, rust_js::link_name = "set window.$RV")]
pub(crate) fn set_rv(reveal: &'static RevealBoundaries) {
    let _ = reveal;
    unreachable!()
}

#[cfg_attr(rust_js, rust_js::link_name = "set window.$RM")]
pub(crate) fn set_rm(resources: &'static js::Map<Option<String>, &'static Element>) {
    let _ = resources;
    unreachable!()
}

#[cfg_attr(rust_js, rust_js::link_name = "set window.$RC")]
pub(crate) fn set_rc(complete: &'static CompleteBoundary) {
    let _ = complete;
    unreachable!()
}

#[cfg_attr(rust_js, rust_js::link_name = "set window.$RX")]
pub(crate) fn set_rx(client_render: &'static ClientRenderBoundary) {
    let _ = client_render;
    unreachable!()
}

#[cfg_attr(rust_js, rust_js::link_name = "set window.$RR")]
pub(crate) fn set_rr(complete: &'static CompleteBoundaryWithStyles) {
    let _ = complete;
    unreachable!()
}

#[cfg_attr(rust_js, rust_js::link_name = "set window.$RS")]
pub(crate) fn set_rs(complete: &'static CompleteBoundary) {
    let _ = complete;
    unreachable!()
}

pub fn reveal_completed_boundaries(batch: &Batch) {
    set_rt(performance.now());
    for i in (0..).step_by(2) {
        let Some(&suspense_id_node) = batch.borrow().get(i) else {
            break;
        };
        let content_node = batch.borrow()[i + 1];
        if let Some(parent_node) = content_node.parent_node() {
            // We can detach the content now.
            // Completions of boundaries within this contentNode will now find the boundary
            // in its designated place.
            parent_node.remove_child(content_node);
        } else {
            // If the client has failed hydration we may have already deleted the streaming
            // segments. The server may also have emitted a complete instruction but cancelled
            // the segment. Regardless we can ignore this case.
        }
        // Clear all the existing children. This is complicated because
        // there can be embedded Suspense boundaries in the fallback.
        // This is similar to clearSuspenseBoundary in ReactFiberConfigDOM.
        // TODO: We could avoid this if we never emitted suspense boundaries in fallback trees.
        // They never hydrate anyway. However, currently we support incrementally loading the fallback.
        let Some(parent_instance) = suspense_id_node.parent_node() else {
            // We may have client-rendered this boundary already. Skip it.
            continue;
        };

        // Find the boundary around the fallback. This is always the previous node.
        let suspense_node = suspense_id_node.previous_sibling();

        let mut node: Option<&Node> = Some(suspense_id_node);
        let mut depth = 0;
        loop {
            if let Some(current) = node
                && current.node_type() == COMMENT_NODE
            {
                let data = comment::unchecked_from(current).data();
                if data == SUSPENSE_END_DATA || data == ACTIVITY_END_DATA {
                    if depth == 0 {
                        break;
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
            }

            let Some(current) = node else {
                break;
            };
            let next_node = current.next_sibling();
            parent_instance.remove_child(current);
            node = next_node;
            if node.is_none() {
                break;
            }
        }

        let end_of_boundary = node;

        // Insert all the children from the contentNode between the start and end of suspense boundary.
        while let Some(first_child) = content_node.first_child() {
            parent_instance.insert_before(first_child, end_of_boundary);
        }

        let suspense_node = suspense_node.expect("a boundary's comment is before its first node");
        comment::unchecked_from(suspense_node).set_data(SUSPENSE_START_DATA);
        if let Some(retry) = js::get(suspense_node, "_reactRetry")
            && js::truthy(Some(retry))
        {
            let retry: &'static dyn Fn(f64) = unsafe { js::cast(Some(retry)) };
            request_animation_frame(Box::new(retry));
        }
    }
    batch.borrow_mut().clear();
}

pub fn reveal_completed_boundaries_with_view_transitions(
    reveal_boundaries: &'static RevealBoundaries,
    batch: &'static Batch,
) {
    let should_start_view_transition = Cell::new(false);
    let auto_name_idx = Cell::new(0.0);
    let restore_queue = RefCell::new(Vec::new());
    let apply_view_transition_name = |element: &'static Element, class_attribute_name: &str| {
        let Some(class_name) = element
            .get_attribute(class_attribute_name)
            .filter(|name| !name.is_empty())
        else {
            return;
        };
        // Add any elements we apply a name to a queue to be reverted when we start.
        let element_style = webapi::html_element::unchecked_from(element).style();
        restore_queue.borrow_mut().push((
            element,
            element_style.view_transition_name(),
            element_style.view_transition_class(),
        ));
        if class_name != "auto" {
            element_style.set_view_transition_class(&class_name);
        }
        let name = match element
            .get_attribute("vt-name")
            .filter(|name| !name.is_empty())
        {
            Some(name) => name,
            None => {
                // Auto-generate a name for this one.
                // TODO: We don't have a prefix to pick from here but maybe we don't need it
                // since it's only applicable temporarily during this specific animation.
                let id_prefix = "";
                let index = auto_name_idx.get();
                auto_name_idx.set(index + 1.0);
                format!("_{id_prefix}T_{index}_")
            }
        };
        // If the name isn't valid CSS identifier, base64 encode the name instead.
        // This doesn't let you select it in custom CSS selectors but it does work in current
        // browsers.
        let escaped_name = if css::escape(&name) != name {
            format!("r-{}", global::btoa(&name).replace('=', ""))
        } else {
            name
        };
        element_style.set_view_transition_name(&escaped_name);
        should_start_view_transition.set(true);
    };
    let started = panic::catch_unwind(AssertUnwindSafe(|| {
        if let Some(existing_transition) = js::get(document, "__reactViewTransition")
            && js::truthy(Some(existing_transition))
        {
            // Retry after the previous ViewTransition finishes.
            let existing_transition: &ViewTransition =
                unsafe { js::cast(Some(existing_transition)) };
            existing_transition
                .finished()
                .finally(function::bind(RV, Some(&Kind::Null), (batch,)));
            return true;
        }
        // First collect all entering names that might form pairs exiting names.
        let appearing_view_transitions = js::map::new::<Option<String>, Option<&Element>>(None);
        for i in (1..).step_by(2) {
            let Some(&content_node) = batch.borrow().get(i) else {
                break;
            };
            let appearing_elements = content_node.query_selector_all("[vt-share]");
            for appearing_element in node_list::iter(appearing_elements) {
                appearing_view_transitions.set(
                    &appearing_element.get_attribute("vt-name"),
                    Some(appearing_element),
                );
            }
        }
        let mut suspensey_images = Vec::new();
        // Next we'll find the nodes that we're going to animate and apply names to them..
        for i in (0..).step_by(2) {
            let Some(&suspense_id_node) = batch.borrow().get(i) else {
                break;
            };
            let Some(parent_instance) = suspense_id_node.parent_node() else {
                // We may have client-rendered this boundary already. Skip it.
                continue;
            };
            let parent_rect = element::unchecked_from(parent_instance).get_bounding_client_rect();
            if !js::truthy(Some(&parent_rect.left()))
                && !js::truthy(Some(&parent_rect.top()))
                && !js::truthy(Some(&parent_rect.width()))
                && !js::truthy(Some(&parent_rect.height()))
            {
                // If the parent instance is display: none then we don't animate this boundary.
                // This can happen when this boundary is actually a child of a different boundary that
                // isn't yet revealed or is about to be revealed, but in that case that boundary
                // should do the exit/enter and not this one. Conveniently this also lets us skip
                // this if it's just in a hidden tree in general.
                // TODO: Should we skip it if it's out of viewport? It's possible that it gets
                // brought into the viewport by changing size.
                // TODO: There's a another case where an inner boundary is inside a fallback that
                // is about to be deleted. In that case we should not run exit animations on the inner.
                continue;
            }

            // Apply exit animations to the immediate elements inside the fallback.
            let mut node: Option<&Node> = Some(suspense_id_node);
            let mut depth = 0;
            while let Some(current) = node {
                if current.node_type() == COMMENT_NODE {
                    let data = comment::unchecked_from(current).data();
                    if data == SUSPENSE_END_DATA {
                        if depth == 0 {
                            break;
                        } else {
                            depth -= 1;
                        }
                    } else if data == SUSPENSE_START_DATA
                        || data == SUSPENSE_PENDING_START_DATA
                        || data == SUSPENSE_QUEUED_START_DATA
                        || data == SUSPENSE_FALLBACK_START_DATA
                    {
                        depth += 1;
                    }
                } else if current.node_type() == ELEMENT_NODE {
                    let exit_element = element::unchecked_from(current);
                    let exit_name = exit_element.get_attribute("vt-name");
                    let paired_element = appearing_view_transitions.get(&exit_name).flatten();
                    apply_view_transition_name(
                        exit_element,
                        if paired_element.is_some() {
                            "vt-share"
                        } else {
                            "vt-exit"
                        },
                    );
                    if let Some(paired_element) = paired_element {
                        // Activate the other side as well.
                        apply_view_transition_name(paired_element, "vt-share");
                        appearing_view_transitions.set(&exit_name, None); // mark claimed
                    }
                    // Next we'll look inside this element for pairs to trigger "share".
                    let disappearing_elements = exit_element.query_selector_all("[vt-share]");
                    for disappearing_element in node_list::iter(disappearing_elements) {
                        let name = disappearing_element.get_attribute("vt-name");
                        if let Some(appearing_element) =
                            appearing_view_transitions.get(&name).flatten()
                        {
                            apply_view_transition_name(disappearing_element, "vt-share");
                            apply_view_transition_name(appearing_element, "vt-share");
                            appearing_view_transitions.set(&name, None); // mark claimed
                        }
                    }
                    // Relay the exit to nested ViewTransitions that opted in
                    let relay_exit_elements = exit_element.query_selector_all("[vt-parent-exit]");
                    for relay_exit_element in node_list::iter(relay_exit_elements) {
                        apply_view_transition_name(relay_exit_element, "vt-parent-exit");
                    }
                }
                node = current.next_sibling();
            }

            // Apply enter animations to the new nodes about to be inserted.
            let content_node = batch.borrow()[i + 1];
            let mut enter_element = content_node.first_element_child();
            while let Some(current) = enter_element {
                let paired =
                    appearing_view_transitions.get(&current.get_attribute("vt-name")) == Some(None);
                if !paired {
                    apply_view_transition_name(current, "vt-enter");
                }
                // Relay the enter to nested ViewTransitions that opted in
                let relay_enter_elements = current.query_selector_all("[vt-parent-enter]");
                for relay_enter_element in node_list::iter(relay_enter_elements) {
                    apply_view_transition_name(relay_enter_element, "vt-parent-enter");
                }
                enter_element = current.next_element_sibling();
            }

            // Apply update animations to any parents and siblings that might be affected.
            let mut ancestor_element = element::unchecked_from(parent_instance);
            loop {
                let mut child_element = ancestor_element.first_element_child();
                while let Some(current) = child_element {
                    // TODO: Bail out if we can
                    let update_class_name = current.get_attribute("vt-update");
                    if update_class_name
                        .as_deref()
                        .is_some_and(|name| !name.is_empty() && name != "none")
                        && !restore_queue
                            .borrow()
                            .iter()
                            .any(|(element, ..)| *element == current)
                    {
                        // If we have already handled this element as part of another exit/enter/share, don't override.
                        apply_view_transition_name(current, "vt-update");
                    }
                    child_element = current.next_element_sibling();
                }
                match ancestor_element.parent_node() {
                    Some(parent)
                        if parent.node_type() == ELEMENT_NODE
                            && element::unchecked_from(parent)
                                .get_attribute("vt-update")
                                .as_deref()
                                != Some("none") =>
                    {
                        ancestor_element = element::unchecked_from(parent);
                    }
                    _ => break,
                }
            }

            // Find the appearing Suspensey Images inside the new content.
            let appearing_images =
                content_node.query_selector_all("img[src]:not([loading=\"lazy\"])");
            // TODO: Consider marking shouldStartViewTransition if we found any images.
            // But only once we can disable the root animation for that case.
            suspensey_images.extend(node_list::iter(appearing_images));
        }
        if should_start_view_transition.get() {
            let update = function::with_this(move |_, ()| {
                reveal_boundaries(batch);
                let mut blocking_promises = vec![
                    // Force layout to trigger font loading, we stash the actual value to trick minifiers.
                    js::promise::resolve(
                        document
                            .document_element()
                            .expect("a document has its element")
                            .client_height(),
                    )
                    .into_unknown(),
                    // Block on fonts finishing loading before revealing these boundaries.
                    document.fonts().ready().into_unknown(),
                ];
                for &suspensey_image in &suspensey_images {
                    if !html_image_element::unchecked_from(suspensey_image).complete() {
                        let rect = suspensey_image.get_bounding_client_rect();
                        let in_viewport = rect.bottom() > 0.0
                            && rect.right() > 0.0
                            && rect.top() < window.inner_height()
                            && rect.left() < window.inner_width();
                        if in_viewport {
                            // TODO: Use decode() instead of the load event here once the fix in
                            // https://issues.chromium.org/issues/420748301 has propagated fully.
                            let loading_image = js::promise::new::<&'static Event>(|resolve, _| {
                                suspensey_image.add_event_listener(Load, resolve, None);
                                suspensey_image.add_event_listener(Error, resolve, None);
                            });
                            blocking_promises.push(loading_image.into_unknown());
                        }
                    }
                }
                js::promise::race(vec![
                    js::promise::all(blocking_promises).into_unknown(),
                    js::promise::new::<()>(|resolve, _| {
                        let current_time = performance.now();
                        let ms_until_timeout =
                            // If the throttle would make us miss the target metric, then shorten the throttle.
                            // performance.now()'s zero value is assumed to be the start time of the metric.
                            if current_time < TARGET_VANITY_METRIC
                                && current_time > TARGET_VANITY_METRIC - FALLBACK_THROTTLE_MS
                            {
                                TARGET_VANITY_METRIC - current_time
                            } else {
                                // Otherwise it's throttled starting from last commit time.
                                SUSPENSEY_FONT_AND_IMAGE_TIMEOUT
                            };
                        js::set_timeout(move || resolve(()), Some(ms_until_timeout));
                    })
                    .into_unknown(),
                ])
            });
            let transition = document.start_view_transition(Some(&StartViewTransitionOptions {
                update: Some(&update),
                types: Some(&[]), // TODO: Add a hard coded type for Suspense reveals.
            }));
            js::set(document, "__reactViewTransition", transition);
            let restore_queue = restore_queue.take();
            transition.ready().finally(move || {
                // Restore all the names/classes that we applied to what they were before.
                // We do it in reverse order in case there were duplicates so the first one wins.
                for (element, previous_name, _) in restore_queue.iter().rev() {
                    let element_style = webapi::html_element::unchecked_from(element).style();
                    element_style.set_view_transition_name(previous_name);
                    // Upstream restores the class from the name it kept too.
                    let previous_class_name = previous_name;
                    element_style.set_view_transition_class(previous_class_name);
                    if element.get_attribute("style").as_deref() == Some("") {
                        element.remove_attribute("style");
                    }
                }
            });
            transition.finished().finally(move || {
                if js::get(document, "__reactViewTransition") == Some(js::unknown(transition)) {
                    js::set(document, "__reactViewTransition", Kind::Null);
                }
            });
            // Queue any future completions into its own batch since they won't have been
            // snapshotted by this one.
            set_rb(RefCell::new(Vec::new()));
            return true;
        }
        // Fall through to reveal.
        false
    }));
    // Fall through to reveal.
    if let Ok(true) = started {
        return;
    }
    // ViewTransitions v2 not supported or no ViewTransitions found. Reveal immediately.
    reveal_boundaries(batch);
}

pub fn client_render_boundary(
    suspense_boundary_id: &str,
    error_digest: Option<&str>,
    error_msg: Option<&str>,
    error_stack: Option<&str>,
    error_component_stack: Option<&str>,
) {
    // Find the fallback's first element.
    let Some(suspense_id_node) = document.get_element_by_id(suspense_boundary_id) else {
        // The user must have already navigated away from this tree.
        // E.g. because the parent was hydrated.
        return;
    };
    // Find the boundary around the fallback. This is always the previous node.
    let suspense_node = suspense_id_node
        .previous_sibling()
        .expect("a boundary's comment is before its first node");
    // Tag it to be client rendered.
    comment::unchecked_from(suspense_node).set_data(SUSPENSE_FALLBACK_START_DATA);
    // assign error metadata to first sibling
    let dataset = webapi::html_element::unchecked_from(suspense_id_node).dataset();
    if let Some(error_digest) = error_digest {
        dataset.set("dgst", error_digest);
    }
    if let Some(error_msg) = error_msg.filter(|msg| !msg.is_empty()) {
        dataset.set("msg", error_msg);
    }
    if let Some(error_stack) = error_stack.filter(|stack| !stack.is_empty()) {
        dataset.set("stck", error_stack);
    }
    if let Some(error_component_stack) = error_component_stack.filter(|stack| !stack.is_empty()) {
        dataset.set("cstck", error_component_stack);
    }
    // Tell React to retry it if the parent already hydrated.
    if let Some(retry) = js::get(suspense_node, "_reactRetry")
        && js::truthy(Some(retry))
    {
        let retry: &'static dyn Fn() = unsafe { js::cast(Some(retry)) };
        retry();
    }
}

pub fn complete_boundary(suspense_boundary_id: &str, content_id: &str) {
    let Some(content_node_outer) = document.get_element_by_id(content_id) else {
        // If the client has failed hydration we may have already deleted the streaming
        // segments. The server may also have emitted a complete instruction but cancelled
        // the segment. Regardless we can ignore this case.
        return;
    };

    // Find the fallback's first element.
    let Some(suspense_id_node_outer) = document.get_element_by_id(suspense_boundary_id) else {
        // We'll never reveal this boundary so we can remove its content immediately.
        // Otherwise we'll leave it in until we reveal it.
        // This is important in case this specific boundary contains other boundaries
        // that may get completed before we reveal this one.
        content_node_outer
            .parent_node()
            .expect("a boundary's content is in the document")
            .remove_child(content_node_outer);

        // The user must have already navigated away from this tree.
        // E.g. because the parent was hydrated. That's fine there's nothing to do
        // but we have to make sure that we already deleted the container node.
        return;
    };

    // Mark this Suspense boundary as queued so we know not to client render it
    // at the end of document load.
    let suspense_node_outer = suspense_id_node_outer
        .previous_sibling()
        .expect("a boundary's comment is before its first node");
    comment::unchecked_from(suspense_node_outer).set_data(SUSPENSE_QUEUED_START_DATA);
    // Queue this boundary for the next batch
    RB.borrow_mut()
        .extend([suspense_id_node_outer, content_node_outer]);

    if RB.borrow().len() == 2 {
        // This is the first time we've pushed to the batch. We need to schedule a callback
        // to flush the batch. This is delayed by the throttle heuristic.
        if let Kind::Number(shell_time) = js::classify(RT) {
            let current_time = performance.now();
            let ms_until_timeout =
                // If the throttle would make us miss the target metric, then shorten the throttle.
                // performance.now()'s zero value is assumed to be the start time of the metric.
                if current_time < TARGET_VANITY_METRIC
                    && current_time > TARGET_VANITY_METRIC - FALLBACK_THROTTLE_MS
                {
                    TARGET_VANITY_METRIC - current_time
                } else {
                    // Otherwise it's throttled starting from last commit time.
                    shell_time + FALLBACK_THROTTLE_MS - current_time
                };
            // We always schedule the flush in a timer even if it's very low or negative to allow
            // for multiple completeBoundary calls that are already queued to have a chance to
            // make the batch.
            let reveal = function::bind(RV, Some(&Kind::Null), (RB,));
            js::set_timeout(reveal, Some(ms_until_timeout));
        } else {
            // If we haven't had our rAF callback yet, schedule everything for the first paint.
            let reveal = function::bind(RV, Some(&Kind::Null), (RB,));
            request_animation_frame(Box::new(move |_| reveal()));
        }
    }
}

pub fn complete_segment(container_id: &str, placeholder_id: &str) {
    // We always expect both nodes to exist here because, while we might
    // have navigated away from the main tree, we still expect the detached
    // tree to exist.
    let segment_container = document
        .get_element_by_id(container_id)
        .expect("a segment's container is in the document");
    let placeholder_node = document
        .get_element_by_id(placeholder_id)
        .expect("a segment's placeholder is in the document");
    let parent = |node: &Element| {
        node.parent_node()
            .expect("a segment's nodes are in the document")
    };
    parent(segment_container).remove_child(segment_container);
    while let Some(first_child) = segment_container.first_child() {
        parent(placeholder_node).insert_before(first_child, Some(placeholder_node));
    }
    parent(placeholder_node).remove_child(placeholder_node);
}

// This is the exact URL string we expect that Fizz renders if we provide a function action.
// We use this for hydration warnings. It needs to be in sync with Fizz. Maybe makes sense
// as a shared module for that reason.
const EXPECTED_FORM_ACTION_URL: &str =
    "javascript:throw new Error('React form unexpectedly submitted.')";

pub fn listen_to_form_submissions_for_replaying() {
    // A global replay queue ensures actions are replayed in order.
    // This event listener should be above the React one. That way when
    // we preventDefault in React's handling we also prevent this event
    // from queing it. Since React listens to the root and the top most
    // container you can use is the document, the window is fine.
    window_global::add_event_listener(
        Submit,
        |event: &SubmitEvent| {
            if event.default_prevented() {
                // We let earlier events to prevent the action from submitting.
                return;
            }
            let form = html_form_element::unchecked_from(
                event.target().expect("a submit event has a form"),
            );
            let submitter = event.submitter();
            let mut action = form.action();
            let mut form_data_submitter = submitter;
            if let Some(submitter) = submitter
                && let Some(submitter_action) = submitter.get_attribute("formAction")
            {
                // The submitter overrides the action.
                action = submitter_action;
                // If the submitter overrides the action, and it passes the test below,
                // that means that it was a function action which conceptually has no name.
                // Therefore, we exclude the submitter from the formdata.
                form_data_submitter = None;
            }
            if action != EXPECTED_FORM_ACTION_URL {
                // The form is a regular form action, we can bail.
                return;
            }

            // Prevent native navigation.
            // This will also prevent other React's on the same page from listening.
            event.prevent_default();

            // Take a snapshot of the FormData at the time of the event.
            let form_data = form_data::new(Some(form), form_data_submitter);

            // Queue for replaying later. This field could potentially be shared with multiple
            // Reacts on the same page since each one will preventDefault for the next one.
            // This means that this protocol is shared with any React version that shares the same
            // javascript: URL placeholder value. So we might not be the first to declare it.
            // We attach it to the form's root node, which is the shared environment context
            // where we preserve sequencing and where we'll pick it up from during hydration.
            // If there's no ownerDocument, then this is the document.
            let root = form.owner_document();
            let queue = js::or(
                js::get(root, "$$reactFormReplay"),
                Some(js::unknown_of(Vec::<Option<&Unknown>>::new())),
            );
            js::set(root, "$$reactFormReplay", queue);
            let queue: &RefCell<Vec<Option<&Unknown>>> = unsafe { js::cast(queue) };
            queue.borrow_mut().extend([
                Some(js::unknown(form)),
                submitter.map(js::unknown),
                Some(js::unknown(form_data)),
            ]);
        },
        None,
    );
}

pub fn complete_boundary_with_styles(
    suspense_boundary_id: &str,
    content_id: &str,
    stylesheet_descriptors: &[Vec<String>],
) {
    let precedences = js::map::new::<Option<String>, &Element>(None);
    let this_document = document;
    let mut last_resource: Option<&Element> = None;

    // Seed the precedence list with existing resources and collect hoistable style tags
    let nodes = this_document.query_selector_all("link[data-precedence],style[data-precedence]");
    let mut style_tags_to_hoist = Vec::new();
    for node in webapi::node_list::iter(nodes) {
        if node.get_attribute("media").as_deref() == Some("not all") {
            style_tags_to_hoist.push(node);
        } else {
            if node.tag_name() == "LINK" {
                RM.set(&node.get_attribute("href"), node);
            }
            last_resource = Some(node);
            precedences.set(
                &webapi::html_element::unchecked_from(node)
                    .dataset()
                    .get("precedence"),
                node,
            );
        }
    }

    let mut dependencies = Vec::new();

    let cleanup_with = function::with_this(
        |this, (cb, _event): (&'static dyn Fn(Option<&'static Unknown>), &'static Event)| {
            js::set(this, "_p", Kind::Null);
            cb(None);
        },
    );

    // Sheets Mode
    for stylesheet_descriptor in stylesheet_descriptors {
        // Sheet Mode iterates over the stylesheet arguments and constructs them if new or checks them for
        // dependency if they already existed
        let mut descriptor = stylesheet_descriptor.iter();
        let href = descriptor
            .next()
            .expect("a stylesheet's descriptor starts with its href");
        let precedence;
        let resource_el;
        let loading_state;
        let avoid_insert;

        if let Some(existing) = RM.get(&Some(href.clone())) {
            // We have an already inserted stylesheet.
            resource_el = existing;
            loading_state = js::get(resource_el, "_p");
            avoid_insert = true;
            precedence = None;
        } else {
            // We haven't already processed this href so we need to construct a stylesheet and hoist it
            // We construct it here and attach a loadingState. We also check whether it matches
            // media before we include it in the dependency array.
            let link = this_document.create_element(tags::Link);
            link.set_href(href);
            link.set_rel("stylesheet");
            precedence = descriptor.next().cloned();
            if let Some(precedence) = &precedence {
                link.dataset().set("precedence", precedence);
            }
            while let Some(attr) = descriptor.next().filter(|attr| !attr.is_empty()) {
                link.set_attribute(attr, descriptor.next().map(String::as_str));
            }
            let promise = js::promise::new(|resolve, reject| {
                link.set_onload(Some(&function::bind(cleanup_with, Some(link), (resolve,))));
                let reject: &dyn Fn(Option<&'static Unknown>) = reject;
                link.set_onerror(Some(&function::bind(cleanup_with, Some(link), (reject,))));
            });
            js::set(link, "_p", &promise);
            loading_state = js::get(link, "_p");
            resource_el = link;
            avoid_insert = false;
            // Save this resource element so we can bailout if it is used again
            RM.set(&Some(href.clone()), resource_el);
        }
        let media = resource_el.get_attribute("media");
        if js::truthy(loading_state)
            && media
                .as_deref()
                .is_none_or(|media| media.is_empty() || window.match_media(media).matches())
        {
            dependencies.push(unsafe { js::cast::<js::Promise<Option<&Unknown>>>(loading_state) });
        }
        if avoid_insert {
            // We have a link that is already in the document. We don't want to fall through to the insert path
            continue;
        }
        last_resource = insert_resource(precedences, last_resource, precedence, resource_el);
    }

    // <style> mode iterates over not-yet-hoisted <style> tags with data-precedence and hoists them.
    for resource_el in style_tags_to_hoist {
        let precedence = resource_el.get_attribute("data-precedence");
        resource_el.remove_attribute("media");
        last_resource = insert_resource(precedences, last_resource, precedence, resource_el);
    }

    if let Some(suspense_id_node_outer) = document.get_element_by_id(suspense_boundary_id) {
        // Mark this Suspense boundary as queued so we know not to client render it
        // at the end of document load.
        let suspense_node_outer = suspense_id_node_outer
            .previous_sibling()
            .expect("a boundary's comment is before its first node");
        comment::unchecked_from(suspense_node_outer).set_data(SUSPENSE_QUEUED_START_DATA);
    }

    let (complete, client_render) = (RC, RX);
    let boundary = suspense_boundary_id.to_string();
    let (rendered, content) = (boundary.clone(), content_id.to_string());
    js::promise::all(dependencies).then_with::<(), _, _>(
        move |_| complete(&boundary, &content),
        move |_| client_render(&rendered, Some("CSS failed to load"), None, None, None),
    );
}

/// `resource_el`, either a newly constructed `<link rel="stylesheet" ...>` or a `<style>` tag
/// requiring hoisting, inserted after the last resource of its precedence: the last resource
/// now.
fn insert_resource(
    precedences: &js::Map<Option<String>, &'static Element>,
    mut last_resource: Option<&'static Element>,
    precedence: Option<String>,
    resource_el: &'static Element,
) -> Option<&'static Element> {
    let prior = precedences.get(&precedence).or(last_resource);
    if prior == last_resource {
        last_resource = Some(resource_el);
    }
    precedences.set(&precedence, resource_el);

    // Finally, we insert the newly constructed instance at an appropriate location
    // in the Document.
    if let Some(prior) = prior {
        prior
            .parent_node()
            .expect("a resource is in the document")
            .insert_before(resource_el, prior.next_sibling());
    } else {
        let head = document.head().expect("a document has a head");
        head.insert_before(resource_el, head.first_child());
    }
    last_resource
}
