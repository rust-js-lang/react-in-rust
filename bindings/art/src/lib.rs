//! Bindings of [art](https://github.com/sebmarkbage/art) 0.10, ART's vector
//! graphics library: its `Transform`, its current mode and the nodes a mode
//! makes, and the `fast-noSideEffects` mode.
//!
//! art has no TypeScript types: these are written from its JS. What art
//! takes of any kind, and coerces as it draws, is `Option<&Unknown>`.

// A binding's body never runs, so rustc's crate sees its parameters unread
// (rd330, the user's one crate-level allow for a bindings crate).
#![cfg_attr(not(rust_js), allow(unused_variables))]

use std::marker::PhantomData;
use std::ops::Deref;

use js::{JsObject, Unknown};

/// `art/core/transform`'s class: an affine transform, `xx yx xy yy x y`,
/// which every node of a mode is too.
pub struct Transform(PhantomData<JsObject>);

unsafe impl js::Defined for Transform {}

impl PartialEq for Transform {
    #[cfg_attr(rust_js, rust_js::link_name = "===")]
    fn eq(&self, other: &Self) -> bool {
        core::ptr::eq(self, other)
    }
}

impl Eq for Transform {}

/// `Transform`, the class itself, and its constructor.
pub mod transform {
    use js::JsObject;

    use super::Transform;

    unsafe extern "Rust" {
        /// `Transform`, `art/core/transform`'s default export.
        #[cfg_attr(rust_js, rust_js::name = "Transform")]
        #[link_name = "art/core/transform#default"]
        pub safe static TRANSFORM: &'static JsObject;
    }

    /// `new Transform()`: the identity.
    #[cfg_attr(rust_js, rust_js::name = "Transform")]
    #[cfg_attr(rust_js, rust_js::link_name = "new art/core/transform#default")]
    pub fn new() -> &'static Transform {
        unreachable!()
    }
}

impl Transform {
    /// `transform.xx`.
    #[cfg_attr(rust_js, rust_js::link_name = "get xx")]
    pub fn xx(&self) -> f64 {
        unreachable!()
    }

    /// `transform.yx`.
    #[cfg_attr(rust_js, rust_js::link_name = "get yx")]
    pub fn yx(&self) -> f64 {
        unreachable!()
    }

    /// `transform.xy`.
    #[cfg_attr(rust_js, rust_js::link_name = "get xy")]
    pub fn xy(&self) -> f64 {
        unreachable!()
    }

    /// `transform.yy`.
    #[cfg_attr(rust_js, rust_js::link_name = "get yy")]
    pub fn yy(&self) -> f64 {
        unreachable!()
    }

    /// `transform.x`.
    #[cfg_attr(rust_js, rust_js::link_name = "get x")]
    pub fn x(&self) -> f64 {
        unreachable!()
    }

    /// `transform.y`.
    #[cfg_attr(rust_js, rust_js::link_name = "get y")]
    pub fn y(&self) -> f64 {
        unreachable!()
    }

    /// `transform.transformTo(xx, yx, xy, yy, x, y)`: these six, itself.
    #[cfg_attr(rust_js, rust_js::link_name = "transformTo")]
    pub fn transform_to(
        &self,
        xx: f64,
        yx: f64,
        xy: f64,
        yy: f64,
        x: f64,
        y: f64,
    ) -> &'static Transform {
        unreachable!()
    }

    /// `transform.transformTo(other)`: another's six, itself.
    #[cfg_attr(rust_js, rust_js::link_name = "transformTo")]
    pub fn transform_to_transform(&self, other: &Transform) -> &'static Transform {
        unreachable!()
    }

    /// `transform.transform(other)`: itself times `other`, a transform or
    /// an object of its six.
    #[cfg_attr(rust_js, rust_js::link_name = "transform")]
    pub fn transform(&self, other: Option<&Unknown>) -> &'static Transform {
        unreachable!()
    }

    /// `transform.move(x, y)`: moved by `x || 0` and `y || 0`, itself.
    #[cfg_attr(rust_js, rust_js::link_name = "move")]
    pub fn r#move(&self, x: Option<&Unknown>, y: Option<&Unknown>) -> &'static Transform {
        unreachable!()
    }

    /// `transform.rotate(deg, x, y)`: rotated by `deg` degrees about
    /// `(x, y)`, the origin where they're absent, itself.
    #[cfg_attr(rust_js, rust_js::link_name = "rotate")]
    pub fn rotate(
        &self,
        deg: Option<&Unknown>,
        x: Option<&Unknown>,
        y: Option<&Unknown>,
    ) -> &'static Transform {
        unreachable!()
    }

    /// `transform.scale(x, y)`: scaled by `x` and `y`, `x` where `y` is
    /// absent, itself.
    #[cfg_attr(rust_js, rust_js::link_name = "scale")]
    pub fn scale(&self, x: Option<&Unknown>, y: Option<&Unknown>) -> &'static Transform {
        unreachable!()
    }
}

/// A mode's node: a group, a shape, a text or a clipping rectangle, each a
/// transform too.
pub struct Node(PhantomData<JsObject>);

unsafe impl js::Defined for Node {}

impl Deref for Node {
    type Target = Transform;

    fn deref(&self) -> &Transform {
        unsafe { &*(self as *const Self).cast::<Transform>() }
    }
}

impl PartialEq for Node {
    #[cfg_attr(rust_js, rust_js::link_name = "===")]
    fn eq(&self, other: &Self) -> bool {
        core::ptr::eq(self, other)
    }
}

impl Eq for Node {}

impl Node {
    /// `node.parentNode`: the group or surface it's in, `null` once it's
    /// ejected, absent where it was never injected.
    #[cfg_attr(rust_js, rust_js::link_name = "get parentNode")]
    pub fn parent_node(&self) -> Option<&'static Unknown> {
        unreachable!()
    }

    /// `node.inject(container)`: appended to a group or a surface.
    #[cfg_attr(rust_js, rust_js::link_name = "inject")]
    pub fn inject(&self, container: &impl Container) -> &'static Node {
        unreachable!()
    }

    /// `node.injectBefore(sibling)`: put before `sibling`, in its parent.
    #[cfg_attr(rust_js, rust_js::link_name = "injectBefore")]
    pub fn inject_before(&self, sibling: &Node) -> &'static Node {
        unreachable!()
    }

    /// `node.eject()`: taken out of its parent.
    #[cfg_attr(rust_js, rust_js::link_name = "eject")]
    pub fn eject(&self) -> &'static Node {
        unreachable!()
    }

    /// `node.indicate(cursor, tooltip)`: the cursor and tooltip it shows.
    #[cfg_attr(rust_js, rust_js::link_name = "indicate")]
    pub fn indicate(&self, cursor: Option<&Unknown>, tooltip: Option<&Unknown>) -> &'static Node {
        unreachable!()
    }

    /// `node.blend`: its opacity's setter, which a mode may lack.
    #[cfg_attr(rust_js, rust_js::link_name = "blend")]
    pub fn blend(&self, opacity: Option<&Unknown>) -> &'static Node {
        unreachable!()
    }

    /// `node.show()`.
    #[cfg_attr(rust_js, rust_js::link_name = "show")]
    pub fn show(&self) -> &'static Node {
        unreachable!()
    }

    /// `node.hide()`.
    #[cfg_attr(rust_js, rust_js::link_name = "hide")]
    pub fn hide(&self) -> &'static Node {
        unreachable!()
    }

    /// `node.subscribe(type, fn, bind)`: `fn` called with `bind` as its
    /// `this` for each event of `type`, an event's name or an object of
    /// names and their functions; what it gives unsubscribes.
    #[cfg_attr(rust_js, rust_js::link_name = "subscribe")]
    pub fn subscribe(
        &self,
        r#type: Option<&Unknown>,
        r#fn: &'static dyn Fn(&'static Unknown),
        bind: &Node,
    ) -> &'static dyn Fn() {
        unreachable!()
    }

    /// `node.width = width`: a group's or a clipping rectangle's.
    #[cfg_attr(rust_js, rust_js::link_name = "set width")]
    pub fn set_width(&self, width: Option<&Unknown>) {
        unreachable!()
    }

    /// `node.height = height`: a group's or a clipping rectangle's.
    #[cfg_attr(rust_js, rust_js::link_name = "set height")]
    pub fn set_height(&self, height: Option<&Unknown>) {
        unreachable!()
    }

    /// `node.fill(color)`: a shape's or a text's.
    #[cfg_attr(rust_js, rust_js::link_name = "fill")]
    pub fn fill(&self, color: Option<&Unknown>) -> &'static Node {
        unreachable!()
    }

    /// `node.fillLinear(stops, x1, y1, x2, y2)`: a shape's or a text's.
    #[cfg_attr(rust_js, rust_js::link_name = "fillLinear")]
    pub fn fill_linear(
        &self,
        stops: Option<&Unknown>,
        x1: Option<&Unknown>,
        y1: Option<&Unknown>,
        x2: Option<&Unknown>,
        y2: Option<&Unknown>,
    ) -> &'static Node {
        unreachable!()
    }

    /// `node.fillRadial(stops, focusX, focusY, radiusX, radiusY, centerX,
    /// centerY)`: a shape's or a text's.
    #[cfg_attr(rust_js, rust_js::link_name = "fillRadial")]
    pub fn fill_radial(
        &self,
        stops: Option<&Unknown>,
        focus_x: Option<&Unknown>,
        focus_y: Option<&Unknown>,
        radius_x: Option<&Unknown>,
        radius_y: Option<&Unknown>,
        center_x: Option<&Unknown>,
        center_y: Option<&Unknown>,
    ) -> &'static Node {
        unreachable!()
    }

    /// `node.fillImage(url, width, height, left, top, color1, color2)`: a
    /// shape's or a text's.
    #[cfg_attr(rust_js, rust_js::link_name = "fillImage")]
    pub fn fill_image(
        &self,
        url: Option<&Unknown>,
        width: Option<&Unknown>,
        height: Option<&Unknown>,
        left: Option<&Unknown>,
        top: Option<&Unknown>,
        color1: Option<&Unknown>,
        color2: Option<&Unknown>,
    ) -> &'static Node {
        unreachable!()
    }

    /// `node.stroke(color, width, cap, join, dash)`: a shape's or a text's.
    #[cfg_attr(rust_js, rust_js::link_name = "stroke")]
    pub fn stroke(
        &self,
        color: Option<&Unknown>,
        width: Option<&Unknown>,
        cap: Option<&Unknown>,
        join: Option<&Unknown>,
        dash: Option<&Unknown>,
    ) -> &'static Node {
        unreachable!()
    }

    /// `shape.draw(path, width, height)`: a shape's path and size.
    #[cfg_attr(rust_js, rust_js::link_name = "draw")]
    pub fn draw_shape(
        &self,
        path: Option<&Unknown>,
        width: Option<&Unknown>,
        height: Option<&Unknown>,
    ) -> &'static Node {
        unreachable!()
    }

    /// `text.draw(text, font, alignment, path)`: a text's text and how
    /// it's set.
    #[cfg_attr(rust_js, rust_js::link_name = "draw")]
    pub fn draw_text(
        &self,
        text: Option<&Unknown>,
        font: Option<&Unknown>,
        alignment: Option<&Unknown>,
        path: Option<&Unknown>,
    ) -> &'static Node {
        unreachable!()
    }
}

/// What a node is injected into: a group, another node, or a surface.
pub trait Container: js::Defined {}

impl Container for Node {}

impl Container for Surface {}

/// A mode's surface: the canvas, SVG or VML element its nodes are drawn
/// into.
pub struct Surface(PhantomData<JsObject>);

unsafe impl js::Defined for Surface {}

impl Surface {
    /// `surface.resize(width, height)`.
    #[cfg_attr(rust_js, rust_js::link_name = "resize")]
    pub fn resize(&self, width: f64, height: f64) -> &'static Surface {
        unreachable!()
    }

    /// `surface.render()`: drawn now, where the mode draws later; a mode
    /// may lack it.
    #[cfg_attr(rust_js, rust_js::link_name = "render")]
    pub fn render(&self) {
        unreachable!()
    }
}

/// `art/modes/current`: the mode `setCurrent` chose, its node makers.
pub mod mode {
    use js::{JsObject, Unknown};

    use super::{Node, Surface};

    unsafe extern "Rust" {
        /// `Mode.Path`, the mode's path class.
        #[link_name = "art/modes/current#default.Path"]
        pub safe static PATH: &'static JsObject;
    }

    /// `Mode.setCurrent(mode)`: `mode`'s makers the current ones.
    #[cfg_attr(rust_js, rust_js::link_name = "art/modes/current#default.setCurrent")]
    pub fn set_current(mode: &JsObject) {
        unreachable!()
    }

    /// `Mode.Surface(width, height, existingElement)`: a surface, of the
    /// element where one is given.
    #[cfg_attr(rust_js, rust_js::link_name = "art/modes/current#default.Surface")]
    pub fn surface(
        width: f64,
        height: f64,
        existing_element: Option<&Unknown>,
    ) -> &'static Surface {
        unreachable!()
    }

    /// `Mode.Surface.tagName`: the tag of the element a surface draws into.
    #[cfg_attr(
        rust_js,
        rust_js::link_name = "get art/modes/current#default.Surface.tagName"
    )]
    pub fn surface_tag_name() -> &'static str {
        unreachable!()
    }

    /// `Mode.Group()`.
    #[cfg_attr(rust_js, rust_js::link_name = "art/modes/current#default.Group")]
    pub fn group() -> &'static Node {
        unreachable!()
    }

    /// `Mode.Shape()`.
    #[cfg_attr(rust_js, rust_js::link_name = "art/modes/current#default.Shape")]
    pub fn shape() -> &'static Node {
        unreachable!()
    }

    /// `Mode.ClippingRectangle()`.
    #[cfg_attr(
        rust_js,
        rust_js::link_name = "art/modes/current#default.ClippingRectangle"
    )]
    pub fn clipping_rectangle() -> &'static Node {
        unreachable!()
    }

    /// `Mode.Text(text, font, alignment, path)`.
    #[cfg_attr(rust_js, rust_js::link_name = "art/modes/current#default.Text")]
    pub fn text(
        text: Option<&Unknown>,
        font: Option<&Unknown>,
        alignment: Option<&Unknown>,
        path: Option<&Unknown>,
    ) -> &'static Node {
        unreachable!()
    }
}

unsafe extern "Rust" {
    /// `art/modes/fast-noSideEffects`: canvas where the host has it, VML
    /// otherwise, chosen without making it current.
    #[cfg_attr(rust_js, rust_js::name = "FastNoSideEffects")]
    #[link_name = "art/modes/fast-noSideEffects#default"]
    pub safe static FAST_NO_SIDE_EFFECTS: &'static JsObject;
}
