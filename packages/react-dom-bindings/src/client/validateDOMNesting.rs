// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use js::{Dict, RegExp, Unknown, dict, reg_exp};
use react_reconciler::react_current_fiber::{CURRENT, run_with_fiber_in_dev};
use react_reconciler::react_fiber_hydration_diffs::{
    HydrationDiffNode, ServerProps, describe_diff,
};
use react_reconciler::react_internal_types::Fiber;
use react_reconciler::react_work_tags::{
    HOST_COMPONENT, HOST_HOISTABLE, HOST_SINGLETON, HOST_TEXT,
};
use shared::environment::__DEV__;
use webapi::console;

fn describe_ancestors(
    ancestor: &Rc<Fiber>,
    child: &Rc<Fiber>,
    props: ServerProps<'static>,
) -> String {
    let mut fiber: Option<Rc<Fiber>> = Some(Rc::clone(child));
    let mut node: Option<Rc<HydrationDiffNode>> = None;
    let mut distance_from_leaf = 0;
    while let Some(current_fiber) = fiber {
        if Rc::ptr_eq(&current_fiber, ancestor) {
            distance_from_leaf = 0;
        }
        node = Some(Rc::new(HydrationDiffNode {
            fiber: Rc::clone(&current_fiber),
            children: RefCell::new(match node {
                Some(node) => vec![node],
                None => vec![],
            }),
            server_props: Cell::new(if Rc::ptr_eq(&current_fiber, child) {
                props
            } else if Rc::ptr_eq(&current_fiber, ancestor) {
                ServerProps::Unmatched
            } else {
                ServerProps::Unknown
            }),
            server_tail: RefCell::new(Vec::new()),
            distance_from_leaf: Cell::new(distance_from_leaf),
        }));
        distance_from_leaf += 1;
        fiber = current_fiber.r#return.borrow().clone();
    }
    if let Some(node) = node {
        // Describe the node using the hydration diff logic.
        // Replace + with - to mark ancestor and child. It's kind of arbitrary.
        return reg_exp::replace(&describe_diff(&node), reg_exp::new("^[+-]", "gm"), ">");
    }
    String::new()
}

pub struct Info {
    pub tag: String,
}

#[derive(Clone)]
pub struct AncestorInfoDev {
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub current: Option<Rc<Info>>,

    #[cfg_attr(rust_js, rust_js::nullable)]
    pub form_tag: Option<Rc<Info>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub a_tag_in_scope: Option<Rc<Info>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub button_tag_in_scope: Option<Rc<Info>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub nobr_tag_in_scope: Option<Rc<Info>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub p_tag_in_button_scope: Option<Rc<Info>>,

    #[cfg_attr(rust_js, rust_js::nullable)]
    pub list_item_tag_autoclosing: Option<Rc<Info>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub dl_item_tag_autoclosing: Option<Rc<Info>>,

    // <head> or <body>
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub container_tag_in_scope: Option<Rc<Info>>,
    pub implicit_root_scope: bool,
}

// This validation code was written based on the HTML5 parsing spec:
// https://html.spec.whatwg.org/multipage/syntax.html#has-an-element-in-scope
//
// Note: this does not catch all invalid nesting, nor does it try to (as it's
// not clear what practical benefit doing so provides); instead, we warn only
// for cases where the parser will give a parse tree differing from what React
// intended. For example, <b><div></div></b> is invalid but we don't warn
// because it still parses correctly; we do warn for other cases like nested
// <p> tags where the beginning of the second element implicitly closes the
// first, causing a confusing mess.

// https://html.spec.whatwg.org/multipage/syntax.html#special
const SPECIAL_TAGS: &[&str] = &[
    "address",
    "applet",
    "area",
    "article",
    "aside",
    "base",
    "basefont",
    "bgsound",
    "blockquote",
    "body",
    "br",
    "button",
    "caption",
    "center",
    "col",
    "colgroup",
    "dd",
    "details",
    "dir",
    "div",
    "dl",
    "dt",
    "embed",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "frame",
    "frameset",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "header",
    "hgroup",
    "hr",
    "html",
    "iframe",
    "img",
    "input",
    "isindex",
    "li",
    "link",
    "listing",
    "main",
    "marquee",
    "menu",
    "menuitem",
    "meta",
    "nav",
    "noembed",
    "noframes",
    "noscript",
    "object",
    "ol",
    "p",
    "param",
    "plaintext",
    "pre",
    "script",
    "section",
    "select",
    "source",
    "style",
    "summary",
    "table",
    "tbody",
    "td",
    "template",
    "textarea",
    "tfoot",
    "th",
    "thead",
    "title",
    "tr",
    "track",
    "ul",
    "wbr",
    "xmp",
];

// https://html.spec.whatwg.org/multipage/syntax.html#has-an-element-in-scope
const IN_SCOPE_TAGS: &[&str] = &[
    "applet",
    "caption",
    "html",
    "table",
    "td",
    "th",
    "marquee",
    "object",
    "select",
    "template",
    // https://html.spec.whatwg.org/multipage/syntax.html#html-integration-point
    // TODO: Distinguish by namespace here -- for <title>, including it here
    // errs on the side of fewer warnings
    "foreignObject",
    "desc",
    "title",
];

thread_local! {
    // https://html.spec.whatwg.org/multipage/syntax.html#has-an-element-in-button-scope
    static BUTTON_SCOPE_TAGS: Vec<&'static str> = if __DEV__ { [IN_SCOPE_TAGS, &["button"]].concat() } else { Vec::new() };
}

// https://html.spec.whatwg.org/multipage/syntax.html#generate-implied-end-tags
const IMPLIED_END_TAGS: &[&str] = &["dd", "dt", "li", "option", "optgroup", "p", "rp", "rt"];

const EMPTY_ANCESTOR_INFO_DEV: AncestorInfoDev = AncestorInfoDev {
    current: None,

    form_tag: None,
    a_tag_in_scope: None,
    button_tag_in_scope: None,
    nobr_tag_in_scope: None,
    p_tag_in_button_scope: None,

    list_item_tag_autoclosing: None,
    dl_item_tag_autoclosing: None,

    container_tag_in_scope: None,
    implicit_root_scope: false,
};

#[cfg_attr(rust_js, rust_js::nullable)]
pub fn updated_ancestor_info_dev(
    old_info: Option<&AncestorInfoDev>,
    tag: &str,
) -> Option<AncestorInfoDev> {
    if __DEV__ {
        let mut ancestor_info = old_info.cloned().unwrap_or(EMPTY_ANCESTOR_INFO_DEV);
        let info = Rc::new(Info {
            tag: tag.to_owned(),
        });

        if IN_SCOPE_TAGS.contains(&tag) {
            ancestor_info.a_tag_in_scope = None;
            ancestor_info.button_tag_in_scope = None;
            ancestor_info.nobr_tag_in_scope = None;
        }
        if BUTTON_SCOPE_TAGS.with(|button_scope_tags| button_scope_tags.contains(&tag)) {
            ancestor_info.p_tag_in_button_scope = None;
        }

        if SPECIAL_TAGS.contains(&tag) && tag != "address" && tag != "div" && tag != "p" {
            // See rules for 'li', 'dd', 'dt' start tags in
            // https://html.spec.whatwg.org/multipage/syntax.html#parsing-main-inbody
            ancestor_info.list_item_tag_autoclosing = None;
            ancestor_info.dl_item_tag_autoclosing = None;
        }

        ancestor_info.current = Some(Rc::clone(&info));

        if tag == "form" {
            ancestor_info.form_tag = Some(Rc::clone(&info));
        }
        if tag == "a" {
            ancestor_info.a_tag_in_scope = Some(Rc::clone(&info));
        }
        if tag == "button" {
            ancestor_info.button_tag_in_scope = Some(Rc::clone(&info));
        }
        if tag == "nobr" {
            ancestor_info.nobr_tag_in_scope = Some(Rc::clone(&info));
        }
        if tag == "p" {
            ancestor_info.p_tag_in_button_scope = Some(Rc::clone(&info));
        }
        if tag == "li" {
            ancestor_info.list_item_tag_autoclosing = Some(Rc::clone(&info));
        }
        if tag == "dd" || tag == "dt" {
            ancestor_info.dl_item_tag_autoclosing = Some(Rc::clone(&info));
        }
        if tag == "#document" || tag == "html" {
            ancestor_info.container_tag_in_scope = None;
        } else if ancestor_info.container_tag_in_scope.is_none() {
            ancestor_info.container_tag_in_scope = Some(info);
        }

        if old_info.is_none() && (tag == "#document" || tag == "html" || tag == "body") {
            // While <head> is also a singleton we don't want to support semantics where
            // you can escape the head by rendering a body singleton so we treat it like a normal scope
            ancestor_info.implicit_root_scope = true;
        } else if ancestor_info.implicit_root_scope {
            ancestor_info.implicit_root_scope = false;
        }

        Some(ancestor_info)
    } else {
        None
    }
}

/**
 * Returns whether
 */
fn is_tag_valid_with_parent(
    tag: &str,
    parent_tag: Option<&str>,
    implicit_root_scope: bool,
) -> bool {
    // First, let's check if we're in an unusual parsing mode...
    match parent_tag {
        // https://html.spec.whatwg.org/multipage/syntax.html#parsing-main-intd
        // https://html.spec.whatwg.org/multipage/syntax.html#parsing-main-incaption
        // No special behavior since these rules fall back to "in body" mode for
        // all except special table nodes which cause bad parsing behavior anyway.

        // https://html.spec.whatwg.org/multipage/syntax.html#parsing-main-intr
        Some("tr") => {
            return tag == "th"
                || tag == "td"
                || tag == "style"
                || tag == "script"
                || tag == "template";
        }
        // https://html.spec.whatwg.org/multipage/syntax.html#parsing-main-intbody
        Some("tbody" | "thead" | "tfoot") => {
            return tag == "tr" || tag == "style" || tag == "script" || tag == "template";
        }
        // https://html.spec.whatwg.org/multipage/syntax.html#parsing-main-incolgroup
        Some("colgroup") => return tag == "col" || tag == "template",
        // https://html.spec.whatwg.org/multipage/syntax.html#parsing-main-intable
        Some("table") => {
            return tag == "caption"
                || tag == "colgroup"
                || tag == "tbody"
                || tag == "tfoot"
                || tag == "thead"
                || tag == "style"
                || tag == "script"
                || tag == "template";
        }
        // https://html.spec.whatwg.org/multipage/syntax.html#parsing-main-inhead
        Some("head") => {
            return tag == "base"
                || tag == "basefont"
                || tag == "bgsound"
                || tag == "link"
                || tag == "meta"
                || tag == "title"
                || tag == "noscript"
                || tag == "noframes"
                || tag == "style"
                || tag == "script"
                || tag == "template";
        }
        // https://html.spec.whatwg.org/multipage/semantics.html#the-html-element
        Some("html") => {
            if implicit_root_scope {
                // When our parent tag is html and we're in the root scope we will actually
                // insert most tags into the body so we need to fall through to validating
                // the specific tag with "in body" parsing mode below
            } else {
                return tag == "head" || tag == "body" || tag == "frameset";
            }
        }
        Some("frameset") => return tag == "frame",
        Some("#document") => {
            if implicit_root_scope {
                // When our parent is the Document and we're in the root scope we will actually
                // insert most tags into the body so we need to fall through to validating
                // the specific tag with "in body" parsing mode below
            } else {
                return tag == "html";
            }
        }
        _ => {}
    }

    // Probably in the "in body" parsing mode, so we outlaw only tag combos
    // where the parsing rules cause implicit opens or closes to be added.
    // https://html.spec.whatwg.org/multipage/syntax.html#parsing-main-inbody
    match tag {
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            return parent_tag != Some("h1")
                && parent_tag != Some("h2")
                && parent_tag != Some("h3")
                && parent_tag != Some("h4")
                && parent_tag != Some("h5")
                && parent_tag != Some("h6");
        }

        "rp" | "rt" => {
            // $FlowFixMe[incompatible-type]
            return !parent_tag.is_some_and(|parent_tag| IMPLIED_END_TAGS.contains(&parent_tag));
        }

        // <input> causes open <select> tags to close.
        "caption" | "col" | "colgroup" | "input" => return parent_tag != Some("select"),
        // These tags are only valid with a few parents that have special child
        // parsing rules -- if we're down here, then none of those matched and
        // so we allow it only if we don't know what the parent is, as all other
        // cases are invalid.
        "frameset" | "frame" | "tbody" | "td" | "tfoot" | "th" | "thead" | "tr" => {
            return parent_tag.is_none();
        }
        // We support rendering <head> in the root when the container is
        // #document, <html>, or <body>.
        "head" => return implicit_root_scope || parent_tag.is_none(),
        // We support rendering <html> in the root when the container is
        // #document
        "html" => {
            return (implicit_root_scope && parent_tag == Some("#document"))
                || parent_tag.is_none();
        }
        // We support rendering <body> in the root when the container is
        // #document or <html>
        "body" => {
            return (implicit_root_scope
                && (parent_tag == Some("#document") || parent_tag == Some("html")))
                || parent_tag.is_none();
        }
        _ => {}
    }

    true
}

/**
 * Returns whether
 */
fn find_invalid_ancestor_for_tag(tag: &str, ancestor_info: &AncestorInfoDev) -> Option<Rc<Info>> {
    match tag {
        "address" | "article" | "aside" | "blockquote" | "center" | "details" | "dialog"
        | "dir" | "div" | "dl" | "fieldset" | "figcaption" | "figure" | "footer" | "header"
        | "hgroup" | "main" | "menu" | "nav" | "ol" | "p" | "section" | "summary" | "ul"
        | "pre" | "listing" | "table" | "hr" | "xmp" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            return ancestor_info.p_tag_in_button_scope.clone();
        }

        "form" => {
            return ancestor_info
                .form_tag
                .clone()
                .or_else(|| ancestor_info.p_tag_in_button_scope.clone());
        }

        "li" => return ancestor_info.list_item_tag_autoclosing.clone(),

        "dd" | "dt" => return ancestor_info.dl_item_tag_autoclosing.clone(),

        "button" => return ancestor_info.button_tag_in_scope.clone(),

        // Spec says something about storing a list of markers, but it sounds
        // equivalent to this check.
        "a" => return ancestor_info.a_tag_in_scope.clone(),

        "nobr" => return ancestor_info.nobr_tag_in_scope.clone(),
        _ => {}
    }

    js::null()
}

thread_local! {
    static DID_WARN: &'static Dict<bool> = dict::new();
}

#[cfg_attr(rust_js, rust_js::nullable)]
fn find_ancestor(mut parent: Option<Rc<Fiber>>, tag_name: &str) -> Option<Rc<Fiber>> {
    while let Some(current_parent) = parent {
        match current_parent.tag.get() {
            HOST_COMPONENT | HOST_HOISTABLE | HOST_SINGLETON
                if js::strict_equals(current_parent.r#type.get(), tag_name) =>
            {
                return Some(current_parent);
            }
            _ => {}
        }
        parent = current_parent.r#return.borrow().clone();
    }
    None
}

#[cfg_attr(rust_js, rust_js::name = "validateDOMNesting")]
pub fn validate_dom_nesting(child_tag: &str, ancestor_info: Option<&AncestorInfoDev>) -> bool {
    if __DEV__ {
        let ancestor_info = ancestor_info.unwrap_or(&EMPTY_ANCESTOR_INFO_DEV);
        let parent_info = ancestor_info.current.clone();
        let parent_tag = parent_info
            .as_ref()
            .map(|parent_info| parent_info.tag.as_str());

        let invalid_parent =
            if is_tag_valid_with_parent(child_tag, parent_tag, ancestor_info.implicit_root_scope) {
                None
            } else {
                parent_info.clone()
            };
        let invalid_ancestor = if invalid_parent.is_some() {
            None
        } else {
            find_invalid_ancestor_for_tag(child_tag, ancestor_info)
        };
        let Some(invalid_parent_or_ancestor) = invalid_parent.clone().or(invalid_ancestor) else {
            return true;
        };

        let ancestor_tag = invalid_parent_or_ancestor.tag.clone();

        let warn_key =
            // eslint-disable-next-line react-internal/safe-string-coercion
            invalid_parent.is_some().to_string() + "|" + child_tag + "|" + &ancestor_tag;
        if DID_WARN.with(|did_warn| dict::get(did_warn, &warn_key).is_some_and(|&warned| warned)) {
            return false;
        }
        DID_WARN.with(|did_warn| dict::set(did_warn, &warn_key, true));

        let child = CURRENT.with_borrow(|current| current.clone());
        let ancestor = match &child {
            Some(child) => find_ancestor(child.r#return.borrow().clone(), &ancestor_tag),
            None => js::null(),
        };

        let ancestor_description = if let Some(child) = &child
            && let Some(ancestor) = &ancestor
        {
            describe_ancestors(ancestor, child, ServerProps::Unmatched)
        } else {
            String::new()
        };

        let tag_display_name = "<".to_owned() + child_tag + ">";
        if invalid_parent.is_some() {
            let mut info = String::new();
            if ancestor_tag == "table" && child_tag == "tr" {
                info += concat!(
                    " Add a <tbody>, <thead> or <tfoot> to your code to match the DOM tree generated by ",
                    "the browser."
                );
            }
            console::error((
                concat!(
                    "In HTML, %s cannot be a child of <%s>.%s\n",
                    "This will cause a hydration error.%s"
                ),
                &tag_display_name,
                &ancestor_tag,
                info,
                ancestor_description,
            ));
        } else {
            console::error((
                concat!(
                    "In HTML, %s cannot be a descendant of <%s>.\n",
                    "This will cause a hydration error.%s"
                ),
                &tag_display_name,
                &ancestor_tag,
                ancestor_description,
            ));
        }
        if let Some(child) = &child {
            // For debugging purposes find the nearest ancestor that caused the issue.
            // The stack trace of this ancestor can be useful to find the cause.
            // If the parent is a direct parent in the same owner, we don't bother.
            let parent = child.r#return.borrow().clone();
            if let Some(ancestor) = &ancestor
                && let Some(parent) = &parent
                && (!Rc::ptr_eq(ancestor, parent)
                    || !js::strict_equals(parent.debug_owner.get(), child.debug_owner.get()))
            {
                run_with_fiber_in_dev(
                    Some(Rc::clone(ancestor)),
                    |_: (), _: (), _: (), _: (), _: ()| {
                        console::error((
                            // We repeat some context because this log might be taken out of context
                            // such as in React DevTools or grouped server logs.
                            concat!(
                                "<%s> cannot contain a nested %s.\n",
                                "See this log for the ancestor stack trace."
                            ),
                            &ancestor_tag,
                            &tag_display_name,
                        ));
                    },
                    (),
                    (),
                    (),
                    (),
                    (),
                );
            }
        }
        return false;
    }
    true
}

/// The props a text node's description shows of its parent: no children.
struct NoChildren {
    #[cfg_attr(rust_js, rust_js::nullable)]
    children: Option<&'static Unknown>,
}
unsafe impl js::Defined for NoChildren {}

pub fn validate_text_nesting(
    child_text: &str,
    parent_tag: &str,
    implicit_root_scope: bool,
) -> bool {
    if __DEV__ {
        if implicit_root_scope || is_tag_valid_with_parent("#text", Some(parent_tag), false) {
            return true;
        }

        let warn_key = "#text|".to_owned() + parent_tag;
        if DID_WARN.with(|did_warn| dict::get(did_warn, &warn_key).is_some_and(|&warned| warned)) {
            return false;
        }
        DID_WARN.with(|did_warn| dict::set(did_warn, &warn_key, true));

        let child = CURRENT.with_borrow(|current| current.clone());
        let ancestor = match &child {
            Some(child) => find_ancestor(Some(Rc::clone(child)), parent_tag),
            None => js::null(),
        };

        let ancestor_description = if let Some(child) = &child
            && let Some(ancestor) = &ancestor
        {
            describe_ancestors(
                ancestor,
                child,
                if child.tag.get() != HOST_TEXT {
                    ServerProps::Props(js::unknown_of(NoChildren {
                        children: js::null(),
                    }))
                } else {
                    ServerProps::Unmatched
                },
            )
        } else {
            String::new()
        };

        if NON_WHITESPACE.with(|non_whitespace| non_whitespace.test(child_text)) {
            console::error((
                concat!(
                    "In HTML, text nodes cannot be a child of <%s>.\n",
                    "This will cause a hydration error.%s"
                ),
                parent_tag,
                ancestor_description,
            ));
        } else {
            console::error((
                concat!(
                    "In HTML, whitespace text nodes cannot be a child of <%s>. ",
                    "Make sure you don't have any extra whitespace between tags on ",
                    "each line of your source code.\n",
                    "This will cause a hydration error.%s"
                ),
                parent_tag,
                ancestor_description,
            ));
        }
        return false;
    }
    true
}

thread_local! {
    static NON_WHITESPACE: &'static RegExp = reg_exp::new("\\S", "");
}
