// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

//! This is a renderer of React that doesn't have a render target output.
//! It is useful to demonstrate the internals of the reconciler in isolation
//! and for testing semantics of reconciliation separate from the host
//! environment.

use std::cell::{Cell, RefCell};
use std::marker::PhantomData;

use js::{JsObject, Map, Uint8Array, Unknown};
use node::BufferEncoding;
use node::buffer::Buffer;

pub struct Instance<'a> {
    r#type: &'a str,
    pub children: [&'static Node; 0],
    pub prop: Option<&'static Unknown>,
    pub hidden: bool,
}

// Safety: its type, its empty children, its `prop`, a test's text or number, and whether it's hidden, JSON's: what JSON.stringify throws of another `prop` is thrown as React's is.
unsafe impl js::JsonText for Instance<'_> {}

pub struct TextInstance<'a> {
    pub text: &'a str,
    pub hidden: bool,
}

// Safety: its text and whether it's hidden, JSON's.
unsafe impl js::JsonText for TextInstance<'_> {}

pub struct ActivityInstance {
    pub children: [&'static Node; 0],
}

// Safety: its empty children, JSON's.
unsafe impl js::JsonText for ActivityInstance {}

/// An instance, a text instance, a boundary or a segment: a JS object of
/// the chunk it was made of, or made here.
pub struct Node(PhantomData<JsObject>);

unsafe impl js::Defined for Node {}

// Safety: what JSON.parse made of a chunk, JSON's.
unsafe impl js::JsonText for Node {}

/// What a render's instances hold, an instance's, a boundary's and a
/// segment's: its children, each a JS object of the chunk it was made of.
pub struct Parent {
    pub children: RefCell<Vec<&'static Node>>,
}

unsafe impl js::Defined for Parent {}

pub struct SuspenseInstance {
    pub state: Cell<&'static str>,
    pub children: RefCell<Vec<&'static Node>>,
}

unsafe impl js::Defined for SuspenseInstance {}

pub struct Placeholder {
    pub parent: &'static Parent,
    pub index: usize,
}

unsafe impl js::Defined for Placeholder {}

pub struct Segment {
    pub children: RefCell<Vec<&'static Node>>,
}

unsafe impl js::Defined for Segment {}

pub struct Destination {
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub root: Cell<Option<&'static Unknown>>,
    pub placeholders: &'static Map<f64, Placeholder>,
    pub segments: &'static Map<f64, &'static Segment>,
    pub stack: RefCell<Vec<&'static Parent>>,
    pub abort: Box<dyn Fn()>,
}

unsafe impl js::Defined for Destination {}

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "POP")]
    static POP: &'static Buffer = Buffer::from_with_str("/", Some(BufferEncoding::Utf8));
}

fn write(destination: &Destination, buffer: &'static Uint8Array) {
    let stack = &destination.stack;
    if POP.with(|pop| std::ptr::eq::<Uint8Array>(buffer, &***pop)) {
        stack.borrow_mut().pop();
        return;
    }
    // We assume one chunk is one instance.
    let instance: &'static Parent = unsafe {
        js::cast(
            js::json::parse(&Buffer::from(buffer).to_string(
                Some(BufferEncoding::Utf8),
                None,
                None,
            ))
            .unwrap(),
        )
    };
    if stack.borrow().is_empty() {
        destination.root.set(Some(js::unknown(instance)));
    } else {
        let parent = stack.borrow()[stack.borrow().len() - 1];
        parent
            .children
            .borrow_mut()
            .push(unsafe { js::cast(Some(js::unknown(instance))) });
    }
    stack.borrow_mut().push(instance);
}

/// The Fizz server React's noop renderer is: of its host config.
pub struct ReactNoopServer(PhantomData<JsObject>);

unsafe impl js::Defined for ReactNoopServer {}

/// Fizz's request, of the noop renderer.
pub struct Request(PhantomData<JsObject>);

unsafe impl js::Defined for Request {}

impl ReactNoopServer {
    #[cfg_attr(rust_js, rust_js::link_name = "createRequest")]
    fn create_request(
        &self,
        children: Option<&'static Unknown>,
        resumable_state: Option<&'static Unknown>,
        render_state: Option<&'static Unknown>,
        root_format_context: Option<&'static Unknown>,
        progressive_chunk_size: Option<f64>,
        on_error: Option<&'static dyn Fn(Option<&'static Unknown>) -> Option<String>>,
        on_browser_bailout: Option<&'static dyn Fn(Option<&'static Unknown>)>,
        on_all_ready: Option<&'static dyn Fn()>,
        on_shell_ready: Option<&'static dyn Fn()>,
    ) -> &'static Request {
        let _ = (
            children,
            resumable_state,
            render_state,
            root_format_context,
            progressive_chunk_size,
            on_error,
            on_browser_bailout,
            on_all_ready,
            on_shell_ready,
        );
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "startWork")]
    fn start_work(&self, request: &Request) {
        let _ = request;
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "startFlowing")]
    fn start_flowing(&self, request: &Request, destination: &Destination) {
        let _ = (request, destination);
        unreachable!()
    }

    #[cfg_attr(rust_js, rust_js::link_name = "abort")]
    fn abort(&self, request: &Request) {
        let _ = request;
        unreachable!()
    }
}

/// The chunks a segment is written to.
type Target = RefCell<Vec<&'static Buffer>>;

/// The noop renderer's Fizz host config: what Fizz asks of it, each its
/// function.
struct HostConfig {
    schedule_microtask: fn(&dyn Fn()),
    schedule_work: fn(&dyn Fn()),
    begin_writing: fn(&Destination),
    write_chunk: fn(&Destination, &'static Uint8Array),
    write_chunk_and_return: fn(&Destination, &'static Uint8Array) -> bool,
    complete_writing: fn(&Destination),
    close: fn(&Destination),
    close_with_error: fn(&Destination, Option<&'static Unknown>),
    flush_buffered: fn(&Destination),

    #[cfg_attr(rust_js, rust_js::nullable)]
    byte_length_of_chunk: Option<fn(&Uint8Array) -> f64>,

    get_child_format_context: fn() -> Option<&'static Unknown>,
    get_suspense_fallback_format_context: fn() -> Option<&'static Unknown>,
    get_suspense_content_format_context: fn() -> Option<&'static Unknown>,

    get_view_transition_format_context: fn() -> Option<&'static Unknown>,

    reset_resumable_state: fn(),
    complete_resumable_state: fn(),

    push_text_instance: fn(&Target, &str, Option<&'static Unknown>, bool) -> bool,
    push_start_instance: fn(&Target, &str, &'static Unknown) -> Option<&'static Unknown>,

    push_end_instance: fn(&Target, &str, &'static Unknown),

    // This is a noop in ReactNoop
    push_segment_finale: fn(&Target, Option<&'static Unknown>, bool, bool),

    write_completed_root:
        fn(&Destination, Option<&'static Unknown>, Option<&'static Unknown>, bool) -> bool,

    write_placeholder: fn(&Destination, Option<&'static Unknown>, f64),

    push_start_activity_boundary: fn(&Target, Option<&'static Unknown>),

    push_end_activity_boundary: fn(&Target, Option<&'static Unknown>),

    write_start_completed_suspense_boundary: fn(&Destination, Option<&'static Unknown>) -> bool,
    write_start_pending_suspense_boundary: fn(&Destination, Option<&'static Unknown>) -> bool,
    write_start_client_rendered_suspense_boundary:
        fn(&Destination, Option<&'static Unknown>) -> bool,
    write_end_completed_suspense_boundary: fn(&Destination) -> bool,
    write_end_pending_suspense_boundary: fn(&Destination) -> bool,
    write_end_client_rendered_suspense_boundary: fn(&Destination) -> bool,

    write_start_segment:
        fn(&Destination, Option<&'static Unknown>, Option<&'static Unknown>, f64) -> bool,
    write_end_segment: fn(&Destination, Option<&'static Unknown>) -> bool,

    write_completed_segment_instruction: fn(&Destination, Option<&'static Unknown>, f64) -> bool,

    write_completed_boundary_instruction:
        fn(&Destination, Option<&'static Unknown>, &SuspenseInstance, f64) -> bool,

    write_client_render_boundary_instruction:
        fn(&Destination, Option<&'static Unknown>, &'static SuspenseInstance) -> bool,

    write_preamble_start: fn(),
    write_preamble_end: fn(),
    write_hoistables: fn(),
    write_hoistables_for_boundary: fn(),
    write_postamble: fn(),
    hoist_hoistables: fn(Option<&'static Unknown>, Option<&'static Unknown>),
    has_suspensey_content: fn(Option<&'static Unknown>, bool) -> bool,
    create_hoistable_state: fn() -> Option<&'static Unknown>,
    emit_early_preloads: fn(),
    create_preamble_state: fn() -> Option<&'static Unknown>,
    can_have_preamble: fn() -> bool,
    hoist_preamble_state: fn(),
    is_preamble_ready: fn() -> bool,
    is_preamble_context: fn() -> bool,
}

unsafe impl js::Defined for HostConfig {}

#[cfg_attr(rust_js, rust_js::link_name = "react-server#default")]
#[cfg_attr(rust_js, rust_js::name = "ReactFizzServer")]
fn react_fizz_server(config: &HostConfig) -> &'static ReactNoopServer {
    let _ = config;
    unreachable!()
}

thread_local! {
    // $FlowFixMe[prop-missing]
    // $FlowFixMe[incompatible-type]
    #[cfg_attr(rust_js, rust_js::name = "ReactNoopServer")]
    static REACT_NOOP_SERVER: &'static ReactNoopServer = react_fizz_server(&HostConfig {
        schedule_microtask: |callback| {
            callback();
        },
        schedule_work: |callback| {
            callback();
        },
        begin_writing: |_destination| {},
        write_chunk: |destination, buffer| {
            write(destination, buffer);
        },
        write_chunk_and_return: |destination, buffer| {
            write(destination, buffer);
            true
        },
        complete_writing: |_destination| {},
        close: |_destination| {},
        close_with_error: |_destination, _error| {},
        flush_buffered: |_destination| {},

        byte_length_of_chunk: None,

        get_child_format_context: || js::null(),
        get_suspense_fallback_format_context: || js::null(),
        get_suspense_content_format_context: || js::null(),

        get_view_transition_format_context: || js::null(),

        reset_resumable_state: || {},
        complete_resumable_state: || {},

        push_text_instance: |target, text, _render_state, _text_embedded| {
            let text_instance = TextInstance { text, hidden: false };
            target.borrow_mut().push(Buffer::from_with_str(&js::json::stringify(&text_instance), Some(BufferEncoding::Utf8)));
            POP.with(|pop| target.borrow_mut().push(pop));
            false
        },
        push_start_instance: |target, r#type, props| {
            let instance = Instance {
                r#type,
                children: [],
                prop: js::get(props, "prop"),
                hidden: false,
            };
            target.borrow_mut().push(Buffer::from_with_str(&js::json::stringify(&instance), Some(BufferEncoding::Utf8)));
            js::get(props, "children")
        },

        push_end_instance: |target, _type, _props| {
            POP.with(|pop| target.borrow_mut().push(pop));
        },

        // This is a noop in ReactNoop
        push_segment_finale: |_target, _render_state, _last_pushed_text, _text_embedded| {},

        write_completed_root: |_destination, _resumable_state, _render_state, _is_complete| true,

        write_placeholder: |destination, _render_state, id| {
            let parent = destination.stack.borrow()[destination.stack.borrow().len() - 1];
            destination.placeholders.set(
                &id,
                Placeholder {
                    parent,
                    index: parent.children.borrow().len(),
                },
            );
        },

        push_start_activity_boundary: |target, _render_state| {
            let activity_instance = ActivityInstance { children: [] };
            target.borrow_mut().push(Buffer::from_with_str(&js::json::stringify(&activity_instance), Some(BufferEncoding::Utf8)));
        },

        push_end_activity_boundary: |target, _render_state| {
            POP.with(|pop| target.borrow_mut().push(pop));
        },

        write_start_completed_suspense_boundary: |destination, _render_state| {
            let suspense_instance: &'static Parent = unsafe {
                js::cast(Some(js::unknown_of(SuspenseInstance {
                    state: Cell::new("complete"),
                    children: RefCell::new(Vec::new()),
                })))
            };
            let parent = destination.stack.borrow()[destination.stack.borrow().len() - 1];
            parent.children.borrow_mut().push(unsafe { js::cast(Some(js::unknown(suspense_instance))) });
            destination.stack.borrow_mut().push(suspense_instance);
            true
        },
        write_start_pending_suspense_boundary: |destination, _render_state| {
            let suspense_instance: &'static Parent = unsafe {
                js::cast(Some(js::unknown_of(SuspenseInstance {
                    state: Cell::new("pending"),
                    children: RefCell::new(Vec::new()),
                })))
            };
            let parent = destination.stack.borrow()[destination.stack.borrow().len() - 1];
            parent.children.borrow_mut().push(unsafe { js::cast(Some(js::unknown(suspense_instance))) });
            destination.stack.borrow_mut().push(suspense_instance);
            true
        },
        write_start_client_rendered_suspense_boundary: |destination, _render_state| {
            let suspense_instance: &'static Parent = unsafe {
                js::cast(Some(js::unknown_of(SuspenseInstance {
                    state: Cell::new("client-render"),
                    children: RefCell::new(Vec::new()),
                })))
            };
            let parent = destination.stack.borrow()[destination.stack.borrow().len() - 1];
            parent.children.borrow_mut().push(unsafe { js::cast(Some(js::unknown(suspense_instance))) });
            destination.stack.borrow_mut().push(suspense_instance);
            true
        },
        write_end_completed_suspense_boundary: |destination| {
            destination.stack.borrow_mut().pop();
            true
        },
        write_end_pending_suspense_boundary: |destination| {
            destination.stack.borrow_mut().pop();
            true
        },
        write_end_client_rendered_suspense_boundary: |destination| {
            destination.stack.borrow_mut().pop();
            true
        },

        write_start_segment: |destination, _render_state, _format_context, id| {
            let segment: &'static Segment = unsafe {
                js::cast(Some(js::unknown_of(Segment {
                    children: RefCell::new(Vec::new()),
                })))
            };
            destination.segments.set(&id, segment);
            if !destination.stack.borrow().is_empty() {
                panic!("Segments are only expected at the root of the stack.");
            }
            destination
                .stack
                .borrow_mut()
                .push(unsafe { js::cast(Some(js::unknown(segment))) });
            true
        },
        write_end_segment: |destination, _format_context| {
            destination.stack.borrow_mut().pop();
            true
        },

        write_completed_segment_instruction: |destination, _render_state, content_segment_id| {
            let Some(segment) = destination.segments.get(&content_segment_id) else {
                panic!("Missing segment.");
            };
            let Some(placeholder) = destination.placeholders.get(&content_segment_id) else {
                panic!("Missing placeholder.");
            };
            let children = segment.children.borrow().clone();
            placeholder
                .parent
                .children
                .borrow_mut()
                .splice(placeholder.index..placeholder.index, children);
            true
        },

        write_completed_boundary_instruction: |destination, _render_state, boundary, content_segment_id| {
            let Some(segment) = destination.segments.get(&content_segment_id) else {
                panic!("Missing segment.");
            };
            let children = segment.children.borrow().clone();
            boundary.children.replace(children);
            boundary.state.set("complete");
            true
        },

        write_client_render_boundary_instruction: |_destination, _render_state, boundary| {
            // $FlowFixMe[prop-missing]
            js::set(js::unknown(boundary), "status", "client-render");
            true
        },

        write_preamble_start: || {},
        write_preamble_end: || {},
        write_hoistables: || {},
        write_hoistables_for_boundary: || {},
        write_postamble: || {},
        hoist_hoistables: |_parent, _child| {},
        has_suspensey_content: |_hoistable_state, _flushing_in_shell| false,
        create_hoistable_state: || js::null(),
        emit_early_preloads: || {},
        create_preamble_state: || js::null(),
        can_have_preamble: || false,
        hoist_preamble_state: || {},
        is_preamble_ready: || true,
        is_preamble_context: || false,
    });
}

pub struct Options {
    pub progressive_chunk_size: Option<f64>,
    pub on_shell_ready: Option<&'static dyn Fn()>,
    pub on_all_ready: Option<&'static dyn Fn()>,
    pub on_error: Option<&'static dyn Fn(Option<&'static Unknown>) -> Option<String>>,
    pub on_browser_bailout: Option<&'static dyn Fn(Option<&'static Unknown>)>,
}

pub fn render(
    children: Option<&'static Unknown>,
    options: Option<&'static Options>,
) -> Destination {
    let request = REACT_NOOP_SERVER.with(|react_noop_server| {
        react_noop_server.create_request(
            // $FlowFixMe[incompatible-type]
            children,
            // $FlowFixMe[incompatible-type]
            js::null(),
            // $FlowFixMe[incompatible-type]
            js::null(),
            // $FlowFixMe[incompatible-type]
            js::null(),
            options.and_then(|options| options.progressive_chunk_size),
            options.and_then(|options| options.on_error),
            options.and_then(|options| options.on_browser_bailout),
            options.and_then(|options| options.on_all_ready),
            options.and_then(|options| options.on_shell_ready),
        )
    });
    // $FlowFixMe[prop-missing]
    // $FlowFixMe[incompatible-type]
    let destination = Destination {
        root: Cell::new(js::null()),
        placeholders: js::map::new(None),
        segments: js::map::new(None),
        stack: RefCell::new(Vec::new()),
        abort: Box::new(move || {
            REACT_NOOP_SERVER.with(|react_noop_server| react_noop_server.abort(request));
        }),
    };
    REACT_NOOP_SERVER.with(|react_noop_server| {
        react_noop_server.start_work(request);
        // $FlowFixMe[incompatible-type]
        react_noop_server.start_flowing(request, &destination);
    });
    destination
}
