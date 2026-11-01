// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, OnceCell};
use std::rc::Rc;

use art::{FAST_NO_SIDE_EFFECTS, Node, Surface as ArtSurface, mode};
use js::array::SLICE as slice;
use js::{PropertyDescriptor, Unknown, function, object};
use react::webapi::Element;
use react::{CSSProperties, Component, HostType, JSX, jsx};
use react_reconciler::react_fiber_error_logger::{
    default_on_caught_error, default_on_recoverable_error, default_on_uncaught_error,
};
use react_reconciler::react_fiber_reconciler::ReactFiberReconciler;
use react_reconciler::react_internal_types::FiberRoot;
use react_reconciler::react_root_tags::{CONCURRENT_ROOT, LEGACY_ROOT};
use shared::react_feature_flags::DISABLE_LEGACY_MODE;

use crate::react_art_internals::{TYPES, children_as_string};

fn default_on_default_transition_indicator() -> Option<&'static dyn Fn()> {
    // Noop
    None
}

js::on_load! {
    mode::set_current(
        // Change to 'art/modes/dom' for easier debugging via SVG
        FAST_NO_SIDE_EFFECTS,
    );
}

/* Declarative fill-type objects; API design not finalized */

#[cfg_attr(rust_js, rust_js::class)]
pub struct LinearGradient {
    _args: Option<&'static Unknown>,
}

#[cfg_attr(rust_js, rust_js::class)]
impl LinearGradient {
    pub fn new(
        _stops: Option<&Unknown>,
        _x1: Option<&Unknown>,
        _y1: Option<&Unknown>,
        _x2: Option<&Unknown>,
        _y2: Option<&Unknown>,
    ) -> Self {
        LinearGradient {
            _args: function::call(slice, Some(js::arguments()), ()),
        }
    }

    pub fn apply_fill(&self, node: &Node) {
        function::apply(
            node.fill_linear(),
            Some(node),
            self._args.map(|args| args as &dyn js::IntoUnknown),
        );
    }
}

#[cfg_attr(rust_js, rust_js::class)]
pub struct RadialGradient {
    _args: Option<&'static Unknown>,
}

#[cfg_attr(rust_js, rust_js::class)]
impl RadialGradient {
    pub fn new(
        _stops: Option<&Unknown>,
        _fx: Option<&Unknown>,
        _fy: Option<&Unknown>,
        _rx: Option<&Unknown>,
        _ry: Option<&Unknown>,
        _cx: Option<&Unknown>,
        _cy: Option<&Unknown>,
    ) -> Self {
        RadialGradient {
            _args: function::call(slice, Some(js::arguments()), ()),
        }
    }

    pub fn apply_fill(&self, node: &Node) {
        function::apply(
            node.fill_radial(),
            Some(node),
            self._args.map(|args| args as &dyn js::IntoUnknown),
        );
    }
}

#[cfg_attr(rust_js, rust_js::class)]
pub struct Pattern {
    _args: Option<&'static Unknown>,
}

#[cfg_attr(rust_js, rust_js::class)]
impl Pattern {
    pub fn new(
        _url: Option<&Unknown>,
        _width: Option<&Unknown>,
        _height: Option<&Unknown>,
        _left: Option<&Unknown>,
        _top: Option<&Unknown>,
    ) -> Self {
        Pattern {
            _args: function::call(slice, Some(js::arguments()), ()),
        }
    }

    pub fn apply_fill(&self, node: &Node) {
        function::apply(
            node.fill_image(),
            Some(node),
            self._args.map(|args| args as &dyn js::IntoUnknown),
        );
    }
}

/// What a `Surface` is given: the size of what ART draws in, and the
/// attributes its element takes.
pub struct SurfaceProps {
    pub height: &'static Unknown,
    pub width: &'static Unknown,
    pub children: Option<&'static Unknown>,
    pub access_key: Option<&'static str>,
    pub class_name: Option<&'static str>,
    pub draggable: Option<bool>,
    pub role: Option<&'static str>,
    pub style: Option<&'static CSSProperties>,
    pub tab_index: Option<f64>,
    pub title: Option<&'static str>,
}

/* React Components */

#[cfg_attr(rust_js, rust_js::class)]
pub struct Surface {
    _surface: Cell<Option<&'static ArtSurface>>,
    #[cfg_attr(rust_js, rust_js::optional)]
    _mount_node: OnceCell<Rc<FiberRoot>>,
    _tag_ref: Cell<Option<&'static Element>>,
}

unsafe impl js::Defined for Surface {}

impl Component<SurfaceProps> for Surface {
    fn component_did_mount(&'static self) {
        let SurfaceProps { height, width, .. } = self.props();

        self._surface.set(Some(mode::surface(
            js::to_number(width).unwrap(),
            js::to_number(height).unwrap(),
            self._tag_ref.get().map(js::unknown),
        )));

        _ = self._mount_node.set(ReactFiberReconciler.create_container(
            js::unknown(self._surface.get().unwrap()),
            if DISABLE_LEGACY_MODE {
                CONCURRENT_ROOT
            } else {
                LEGACY_ROOT
            },
            None,
            false,
            Some(false),
            String::new(),
            Rc::new(default_on_uncaught_error),
            Rc::new(default_on_caught_error),
            Rc::new(default_on_recoverable_error),
            &default_on_default_transition_indicator,
            None,
        ));
        // We synchronously flush updates coming from above so that they commit together
        // and so that refs resolve before the parent life cycles.
        ReactFiberReconciler.update_container_sync(
            self.props().children,
            self._mount_node.get().unwrap(),
            Some(js::unknown(self)),
            None,
        );
        ReactFiberReconciler.flush_sync_work();
    }

    fn component_did_update(&'static self, prev_props: &SurfaceProps, _prev_state: &()) {
        let props = self.props();

        if !js::strict_equals(props.height, prev_props.height)
            || !js::strict_equals(props.width, prev_props.width)
        {
            self._surface.get().unwrap().resize(
                js::to_number(props.width).unwrap(),
                js::to_number(props.height).unwrap(),
            );
        }

        // We synchronously flush updates coming from above so that they commit together
        // and so that refs resolve before the parent life cycles.
        ReactFiberReconciler.update_container_sync(
            self.props().children,
            self._mount_node.get().unwrap(),
            Some(js::unknown(self)),
            None,
        );
        ReactFiberReconciler.flush_sync_work();

        if js::truthy(js::get(self._surface.get().unwrap(), "render")) {
            self._surface.get().unwrap().render();
        }
    }

    fn component_will_unmount(&'static self) {
        // We synchronously flush updates coming from above so that they commit together
        // and so that refs resolve before the parent life cycles.
        ReactFiberReconciler.update_container_sync(
            js::null(),
            self._mount_node.get().unwrap(),
            Some(js::unknown(self)),
            None,
        );
        ReactFiberReconciler.flush_sync_work();
    }

    fn render(&'static self) -> JSX::Element {
        // This is going to be a placeholder because we don't know what it will
        // actually resolve to because ART may render canvas, vml or svg tags here.
        // We only allow a subset of properties since others might conflict with
        // ART's properties.
        let props = self.props();

        // TODO: ART's Canvas Mode overrides surface title and cursor
        let Tag = mode::surface_tag_name();

        jsx! {
            <Tag
                ref={move |r| self._tag_ref.set(r)}
                accessKey={props.access_key}
                className={props.class_name}
                draggable={props.draggable}
                role={props.role}
                style={props.style}
                tabIndex={props.tab_index}
                title={props.title}
            />
        }
    }
}

/// What a `Text` is given: its node's props, read whole, and its children,
/// its text.
pub struct TextProps {
    pub children: Option<&'static Unknown>,
}

#[cfg_attr(rust_js, rust_js::class)]
pub struct Text {
    #[cfg_attr(rust_js, rust_js::optional)]
    _text: Cell<Option<&'static Node>>,
}

unsafe impl js::Defined for Text {}

#[cfg_attr(rust_js, rust_js::class)]
impl Text {
    pub fn new(_props: &TextProps) -> Self {
        let text = Text {
            _text: Cell::new(None),
        };
        // We allow reading these props. Ideally we could expose the Text node as
        // ref directly.
        ["height", "width", "x", "y"].iter().for_each(|key| {
            object::define_property(
                &text,
                *key,
                &PropertyDescriptor {
                    get: Some(&|| text._text.get().and_then(|text| js::get(text, *key))),
                    ..Default::default()
                },
            );
        });
        text
    }
}

impl Component<TextProps> for Text {
    fn render(&'static self) -> JSX::Element {
        // This means you can't have children that render into strings...
        let T: HostType<TextProps, Node> = HostType::new(TYPES.text);
        jsx! {
            <T {...self.props()} ref={move |t| self._text.set(t)}>
                {children_as_string(self.props().children)}
            </T>
        }
    }
}

js::on_load! {
    ReactFiberReconciler.inject_into_dev_tools();
}

/* API */

#[cfg_attr(rust_js, rust_js::name = "ClippingRectangle")]
pub const CLIPPING_RECTANGLE: &str = TYPES.clipping_rectangle;
#[cfg_attr(rust_js, rust_js::name = "Group")]
pub const GROUP: &str = TYPES.group;
#[cfg_attr(rust_js, rust_js::name = "Shape")]
pub const SHAPE: &str = TYPES.shape;
#[cfg_attr(rust_js, rust_js::name = "Path")]
pub use art::mode::PATH;
#[cfg_attr(rust_js, rust_js::name = "Transform")]
pub use art::transform::TRANSFORM;

#[cfg_attr(rust_js, rust_js::name = "version")]
pub use shared::react_version::REACT_VERSION as VERSION;
