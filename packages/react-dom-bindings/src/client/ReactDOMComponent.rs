// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;
use std::panic;

use js::{Dict, Kind, RegExp, Set, Unknown, dict, reg_exp};
use react_reconciler::react_fiber_mutation_tracking::track_host_mutation;
use shared::check_string_coercion::{check_attribute_string_coercion, check_html_string_coercion};
use shared::environment::__DEV__;
use shared::react_feature_flags::{
    ENABLE_HYDRATION_CHANGE_EVENT, ENABLE_SCROLL_END_POLYFILL, ENABLE_SRC_OBJECT,
    ENABLE_VIEW_TRANSITION, ENABLE_VIEW_TRANSITION_PARENT_ENTER_EXIT,
};
use webapi::{Element, HTMLElement, PointerEvent, Text, console, html_element, url};

use crate::css_property_operations::{create_dangerous_string_for_styles, set_value_for_styles};
use crate::dom_namespaces::{MATH_NAMESPACE, SVG_NAMESPACE};
use crate::dom_plugin_event_system::{MEDIA_EVENT_TYPES, listen_to_non_delegated_event};
use crate::dom_property_operations::{
    get_value_for_attribute, get_value_for_attribute_on_custom_component, set_value_for_attribute,
    set_value_for_known_attribute, set_value_for_namespaced_attribute,
    set_value_for_property_on_custom_component,
};
use crate::event_registry::{POSSIBLE_REGISTRATION_NAMES, REGISTRATION_NAME_DEPENDENCIES};
use crate::get_attribute_alias::get_attribute_alias;
use crate::is_custom_element::is_custom_element;
use crate::possible_standard_names::POSSIBLE_STANDARD_NAMES;
use crate::react_controlled_value_prop_types::check_controlled_value_props;
use crate::react_dom_input::{
    init_input, restore_controlled_input_state, update_input, validate_input_props,
};
use crate::react_dom_invalid_aria_hook::validate_properties as validate_aria_properties;
use crate::react_dom_null_input_value_prop_hook::validate_properties as validate_input_properties;
use crate::react_dom_option::validate_option_props;
use crate::react_dom_select::{
    init_select, restore_controlled_select_state, update_select, validate_select_props,
};
use crate::react_dom_src_object::set_src_object;
use crate::react_dom_textarea::{
    init_textarea, restore_controlled_textarea_state, update_textarea, validate_textarea_props,
};
use crate::react_dom_unknown_property_hook::{
    EventRegistry, validate_properties as validate_unknown_properties,
};
use crate::react_fiber_config_dom::HOST_CONTEXT_NAMESPACE_NONE;
use crate::sanitize_url::sanitize_url;
use crate::set_text_content::set_text_content;
use crate::validate_dom_nesting::validate_text_nesting;

thread_local! {
    static DID_WARN_CONTROLLED_TO_UNCONTROLLED: Cell<bool> = const { Cell::new(false) };
    static DID_WARN_UNCONTROLLED_TO_CONTROLLED: Cell<bool> = const { Cell::new(false) };
    static DID_WARN_FORM_ACTION_TYPE: Cell<bool> = const { Cell::new(false) };
    static DID_WARN_FORM_ACTION_NAME: Cell<bool> = const { Cell::new(false) };
    static DID_WARN_FORM_ACTION_TARGET: Cell<bool> = const { Cell::new(false) };
    static DID_WARN_FORM_ACTION_METHOD: Cell<bool> = const { Cell::new(false) };
    static DID_WARN_FOR_NEW_BOOLEAN_PROPS_WITH_EMPTY_VALUE: &'static Dict<bool> = dict::new();
    static DID_WARN_POPOVER_TARGET_OBJECT: Cell<bool> = const { Cell::new(false) };
}

fn validate_properties_in_development(r#type: &str, props: &'static Unknown) {
    if __DEV__ {
        validate_aria_properties(r#type, props);
        validate_input_properties(r#type, Some(props));
        validate_unknown_properties(
            r#type,
            props,
            Some(&EventRegistry {
                registration_name_dependencies: REGISTRATION_NAME_DEPENDENCIES
                    .with(|registration_name_dependencies| *registration_name_dependencies),
                possible_registration_names: POSSIBLE_REGISTRATION_NAMES
                    .with(|possible_registration_names| possible_registration_names.unwrap()),
            }),
        );
        if js::truthy(js::get(props, "contentEditable"))
            && !js::truthy(js::get(props, "suppressContentEditableWarning"))
            && js::get(props, "children").is_some()
        {
            console::error((concat!(
                "A component is `contentEditable` and contains `children` managed by ",
                "React. It is now your responsibility to guarantee that none of ",
                "those nodes are unexpectedly modified or duplicated. This is ",
                "probably not intentional."
            ),));
        }
    }
}

fn validate_form_action_in_development(
    tag: &str,
    key: &str,
    value: Option<&Unknown>,
    props: &Unknown,
) {
    if __DEV__ {
        if value.is_none() {
            return;
        }
        if tag == "form" {
            if key == "formAction" {
                console::error((
                    "You can only pass the formAction prop to <input> or <button>. Use the action prop on <form>.",
                ));
            } else if let Kind::Function(_) = js::classify(value) {
                if (js::get(props, "encType").is_some() || js::get(props, "method").is_some())
                    && !DID_WARN_FORM_ACTION_METHOD.get()
                {
                    DID_WARN_FORM_ACTION_METHOD.set(true);
                    console::error((concat!(
                        "Cannot specify a encType or method for a form that specifies a ",
                        "function as the action. React provides those automatically. ",
                        "They will get overridden."
                    ),));
                }
                if js::get(props, "target").is_some() && !DID_WARN_FORM_ACTION_TARGET.get() {
                    DID_WARN_FORM_ACTION_TARGET.set(true);
                    console::error((concat!(
                        "Cannot specify a target for a form that specifies a function as the action. ",
                        "The function will always be executed in the same window."
                    ),));
                }
            }
        } else if tag == "input" || tag == "button" {
            if key == "action" {
                console::error((
                    "You can only pass the action prop to <form>. Use the formAction prop on <input> or <button>.",
                ));
            } else if tag == "input"
                && !js::strict_equals(js::get(props, "type"), "submit")
                && !js::strict_equals(js::get(props, "type"), "image")
                && !DID_WARN_FORM_ACTION_TYPE.get()
            {
                DID_WARN_FORM_ACTION_TYPE.set(true);
                console::error((
                    "An input can only specify a formAction along with type=\"submit\" or type=\"image\".",
                ));
            } else if tag == "button"
                && js::get(props, "type").is_some()
                && !js::strict_equals(js::get(props, "type"), "submit")
                && !DID_WARN_FORM_ACTION_TYPE.get()
            {
                DID_WARN_FORM_ACTION_TYPE.set(true);
                console::error((
                    "A button can only specify a formAction along with type=\"submit\" or no type.",
                ));
            } else if let Kind::Function(_) = js::classify(value) {
                // Function form actions cannot control the form properties
                if js::get(props, "name").is_some() && !DID_WARN_FORM_ACTION_NAME.get() {
                    DID_WARN_FORM_ACTION_NAME.set(true);
                    console::error((concat!(
                        "Cannot specify a \"name\" prop for a button that specifies a function as a formAction. ",
                        "React needs it to encode which action should be invoked. It will get overridden."
                    ),));
                }
                if (js::get(props, "formEncType").is_some()
                    || js::get(props, "formMethod").is_some())
                    && !DID_WARN_FORM_ACTION_METHOD.get()
                {
                    DID_WARN_FORM_ACTION_METHOD.set(true);
                    console::error((concat!(
                        "Cannot specify a formEncType or formMethod for a button that specifies a ",
                        "function as a formAction. React provides those automatically. They will get overridden."
                    ),));
                }
                if js::get(props, "formTarget").is_some() && !DID_WARN_FORM_ACTION_TARGET.get() {
                    DID_WARN_FORM_ACTION_TARGET.set(true);
                    console::error((concat!(
                        "Cannot specify a formTarget for a button that specifies a function as a formAction. ",
                        "The function will always be executed in the same window."
                    ),));
                }
            }
        } else if key == "action" {
            console::error(("You can only pass the action prop to <form>.",));
        } else {
            console::error(("You can only pass the formAction prop to <input> or <button>.",));
        }
    }
}

fn warn_for_prop_difference(
    prop_name: &str,
    server_value: Option<&'static Unknown>,
    client_value: Option<&Unknown>,
    server_differences: &Dict<Option<&'static Unknown>>,
) {
    if __DEV__ {
        if js::strict_equals(server_value, client_value) {
            return;
        }
        let normalized_client_value = normalize_markup_for_text_or_attribute(client_value);
        let normalized_server_value = normalize_markup_for_text_or_attribute(server_value);
        if normalized_server_value == normalized_client_value {
            return;
        }

        dict::set(server_differences, prop_name, server_value);
    }
}

fn has_view_transition(html_element: &HTMLElement) -> bool {
    js::truthy(html_element.get_attribute("vt-share").as_deref())
        || js::truthy(html_element.get_attribute("vt-exit").as_deref())
        || js::truthy(html_element.get_attribute("vt-enter").as_deref())
        || js::truthy(html_element.get_attribute("vt-update").as_deref())
        || (ENABLE_VIEW_TRANSITION_PARENT_ENTER_EXIT.with(|&enabled| enabled)
            && (js::truthy(html_element.get_attribute("vt-parent-enter").as_deref())
                || js::truthy(html_element.get_attribute("vt-parent-exit").as_deref())))
}

fn is_expected_view_transition_name(html_element: &HTMLElement) -> bool {
    if !has_view_transition(html_element) {
        // We didn't expect to see a view transition name applied.
        return false;
    }
    let expected_vt_name = html_element.get_attribute("vt-name");
    let actual_vt_name: String =
        unsafe { js::cast(js::get(html_element.style(), "view-transition-name")) };
    if let Some(expected_vt_name) = expected_vt_name.filter(|name| !name.is_empty()) {
        expected_vt_name == actual_vt_name
    } else {
        // Auto-generated name.
        // TODO: If Fizz starts applying a prefix to this name, we need to consider that.
        actual_vt_name.starts_with("_T_")
    }
}

fn warn_for_extra_attributes(
    dom_element: &Element,
    attribute_names: &Set<str>,
    server_differences: &Dict<Option<&'static Unknown>>,
) {
    if __DEV__ {
        attribute_names.for_each(|attribute_name| {
            if attribute_name == "style" {
                if dom_element.get_attribute(attribute_name).as_deref() == Some("") {
                    // Skip empty style. It's fine.
                    return;
                }
                let html_element = html_element::unchecked_from(dom_element);
                let style = html_element.style();
                #[cfg_attr(rust_js, rust_js::name = "isOnlyVTStyles")]
                let is_only_vt_styles = (style.length() == 1.0
                    && style.item(0) == "view-transition-name")
                    || (style.length() == 2.0
                        && style.item(0) == "view-transition-class"
                        && style.item(1) == "view-transition-name");
                if is_only_vt_styles && is_expected_view_transition_name(html_element) {
                    // If the only extra style was the view-transition-name that we applied from the Fizz
                    // runtime, then we should ignore it.
                } else {
                    dict::set(
                        server_differences,
                        "style",
                        Some(js::unknown(get_styles_object_from_element(dom_element))),
                    );
                }
            } else {
                dict::set(
                    server_differences,
                    get_prop_name_from_attribute_name(attribute_name),
                    dom_element
                        .get_attribute(attribute_name)
                        .map(js::unknown_of),
                );
            }
        });
    }
}

fn warn_for_invalid_event_listener(registration_name: &str, listener: Option<&Unknown>) {
    if __DEV__ {
        if js::strict_equals(listener, false) {
            console::error((
                concat!(
                    "Expected `%s` listener to be a function, instead got `false`.\n\n",
                    "If you used to conditionally omit it with %s={condition && value}, ",
                    "pass %s={condition ? value : undefined} instead."
                ),
                registration_name,
                registration_name,
                registration_name,
            ));
        } else {
            console::error((
                "Expected `%s` listener to be a function, instead got a value of `%s` type.",
                registration_name,
                js::type_of(listener),
            ));
        }
    }
}

// Parse the HTML and read it back to normalize the HTML string so that it
// can be used for comparison.
#[cfg_attr(rust_js, rust_js::name = "normalizeHTML")]
fn normalize_html(parent: &Element, html: &str) -> Option<String> {
    if __DEV__ {
        // We could have created a separate document here to avoid
        // re-initializing custom elements if they exist. But this breaks
        // how <noscript> is being handled. So we use the same document.
        // See the discussion in https://github.com/react/react/pull/11157.
        let test_element = if js::strict_equals(parent.namespace_uri(), MATH_NAMESPACE)
            || js::strict_equals(parent.namespace_uri(), SVG_NAMESPACE)
        {
            parent.owner_document().create_element_ns_named(
                unsafe { js::cast(parent.namespace_uri().map(js::unknown_of)) },
                &parent.tag_name(),
                None,
            )
        } else {
            parent
                .owner_document()
                .create_element_named(&parent.tag_name(), None)
        };
        test_element.set_inner_html(html);
        return Some(test_element.inner_html());
    }
    None
}

thread_local! {
    // HTML parsing normalizes CR and CRLF to LF.
    // It also can turn \u0000 into \uFFFD inside attributes.
    // https://www.w3.org/TR/html5/single-page.html#preprocessing-the-input-stream
    // If we have a mismatch, it might be caused by that.
    // We will still patch up in this case but not fire the warning.
    #[cfg_attr(rust_js, rust_js::name = "NORMALIZE_NEWLINES_REGEX")]
    static NORMALIZE_NEWLINES_REGEX: &'static RegExp = reg_exp::new("\\r\\n?", "g");
    #[cfg_attr(rust_js, rust_js::name = "NORMALIZE_NULL_AND_REPLACEMENT_REGEX")]
    static NORMALIZE_NULL_AND_REPLACEMENT_REGEX: &'static RegExp =
        reg_exp::new("\\u0000|\\uFFFD", "g");
}

fn normalize_markup_for_text_or_attribute(markup: Option<&Unknown>) -> String {
    if __DEV__ {
        check_html_string_coercion(markup);
    }
    let markup_string = match js::classify(markup) {
        Kind::String(markup) => markup.to_owned(),
        _ => js::concat_text(markup).unwrap(),
    };
    NORMALIZE_NULL_AND_REPLACEMENT_REGEX.with(|normalize_null_and_replacement_regex| {
        NORMALIZE_NEWLINES_REGEX.with(|normalize_newlines_regex| {
            reg_exp::replace(
                &reg_exp::replace(&markup_string, normalize_newlines_regex, "\n"),
                normalize_null_and_replacement_regex,
                "",
            )
        })
    })
}

fn check_for_unmatched_text(server_text: Option<String>, client_text: Option<&Unknown>) -> bool {
    let normalized_client_text = normalize_markup_for_text_or_attribute(client_text);
    let normalized_server_text =
        normalize_markup_for_text_or_attribute(server_text.map(js::unknown_of));
    if normalized_server_text == normalized_client_text {
        return true;
    }
    false
}

fn noop(_event: &PointerEvent) {}

pub fn trap_click_on_non_interactive_element(node: &HTMLElement) {
    // Mobile Safari does not fire properly bubble click events on
    // non-interactive elements, which means delegated click listeners do not
    // fire. The workaround for this bug involves attaching an empty click
    // listener on the target node.
    // https://www.quirksmode.org/blog/archives/2010/09/click_event_del.html
    // Just set it using the onclick property so that we don't have to manage any
    // bookkeeping for it. HostSingleton release clears the property only if it
    // still points to this noop.
    // TODO: Only do this for the relevant Safaris maybe?
    node.set_onclick(Some(&noop));
}

pub fn clear_click_listener(node: &HTMLElement) {
    if node
        .onclick()
        .is_some_and(|onclick| std::ptr::eq(onclick, &noop))
    {
        node.set_onclick(None);
    }
}

#[cfg_attr(rust_js, rust_js::name = "xlinkNamespace")]
const XLINK_NAMESPACE: &str = "http://www.w3.org/1999/xlink";
#[cfg_attr(rust_js, rust_js::name = "xmlNamespace")]
const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";

fn set_prop(
    dom_element: &'static Element,
    tag: &str,
    key: &str,
    value: Option<&'static Unknown>,
    props: &'static Unknown,
    prev_value: Option<&'static Unknown>,
) {
    match key {
        "children" => {
            if let Kind::String(value) = js::classify(value) {
                if __DEV__ {
                    validate_text_nesting(value, tag, false);
                }
                // Avoid setting initial textContent when the text is empty. In IE11 setting
                // textContent on a <textarea> will cause the placeholder to not
                // show within the <textarea> until it has been focused and blurred again.
                // https://github.com/react/react/issues/6731#issuecomment-254874553
                let can_set_text_content = tag != "body" && (tag != "textarea" || !value.is_empty());
                if can_set_text_content {
                    set_text_content(dom_element, value);
                }
            } else if let Kind::Number(_) | Kind::BigInt(_) = js::classify(value) {
                if __DEV__ {
                    validate_text_nesting(&js::concat_text(value).unwrap(), tag, false);
                }
                let can_set_text_content = tag != "body";
                if can_set_text_content {
                    set_text_content(dom_element, &js::concat_text(value).unwrap());
                }
            } else {
                return;
            }
        }
        // These are very common props and therefore are in the beginning of the switch.
        // TODO: aria-label is a very common prop but allows booleans so is not like the others
        // but should ideally go in this list too.
        "className" => set_value_for_known_attribute(dom_element, "class", value),
        "tabIndex" => {
            // This has to be case sensitive in SVG.
            set_value_for_known_attribute(dom_element, "tabindex", value);
        }
        "dir" | "role" | "viewBox" | "width" | "height" => {
            set_value_for_known_attribute(dom_element, key, value);
        }
        "style" => {
            set_value_for_styles(dom_element, value, prev_value);
            return;
        }
        // These attributes accept URLs. These must not allow javascript: URLS.
        "data" if tag != "object" => set_value_for_known_attribute(dom_element, "data", value),
        "data" | "src" | "href" => {
            if key != "href"
                && ENABLE_SRC_OBJECT.with(|&enabled| enabled)
                && let Some(object) = js::classify(value).object()
            {
                // Some tags support object sources like Blob, File, MediaSource and MediaStream.
                if tag == "img" || tag == "video" || tag == "audio" {
                    if let Ok(()) =
                        panic::catch_unwind(|| set_src_object(dom_element, tag, js::unknown(object)))
                    {
                        track_host_mutation();
                        return;
                    }
                    // If URL.createObjectURL() errors, it was probably some other object type
                    // that should be toString:ed instead, so we just fall-through to the normal
                    // path.
                } else if __DEV__ {
                    let _ = panic::catch_unwind(|| {
                        // This should always error.
                        url::revoke_object_url(&url::create_object_url(unsafe {
                            js::cast::<&webapi::Blob>(Some(js::unknown(object)))
                        }));
                        if tag == "source" {
                            console::error((concat!(
                                "Passing Blob, MediaSource or MediaStream to <source src> is not supported. ",
                                "Pass it directly to <img src>, <video src> or <audio src> instead."
                            ),));
                        } else {
                            console::error((
                                "Passing Blob, MediaSource or MediaStream to <%s src> is not supported.",
                                tag,
                            ));
                        }
                    });
                }
            }
            if js::strict_equals(value, "")
                // <a href=""> is fine for "reload" links.
                && !(tag == "a" && key == "href")
            {
                if __DEV__ {
                    if key == "src" {
                        console::error((
                            concat!(
                                "An empty string (\"\") was passed to the %s attribute. ",
                                "This may cause the browser to download the whole page again over the network. ",
                                "To fix this, either do not render the element at all ",
                                "or pass null to %s instead of an empty string."
                            ),
                            key,
                            key,
                        ));
                    } else {
                        console::error((
                            concat!(
                                "An empty string (\"\") was passed to the %s attribute. ",
                                "To fix this, either do not render the element at all ",
                                "or pass null to %s instead of an empty string."
                            ),
                            key,
                            key,
                        ));
                    }
                }
                dom_element.remove_attribute(key);
            } else if value.is_none()
                || matches!(
                    js::classify(value),
                    Kind::Function(_) | Kind::Symbol(_) | Kind::Bool(_)
                )
            {
                dom_element.remove_attribute(key);
            } else {
                if __DEV__ {
                    check_attribute_string_coercion(value, key);
                }
                let sanitized_value = sanitize_url(value);
                dom_element.set_attribute(key, sanitized_value);
            }
        }
        "action" | "formAction" => {
            // TODO: Consider moving these special cases to the form, input and button tags.
            if __DEV__ {
                validate_form_action_in_development(tag, key, value, props);
            }
            if let Kind::Function(_) = js::classify(value) {
                // Set a javascript URL that doesn't do anything. We don't expect this to be invoked
                // because we'll preventDefault, but it can happen if a form is manually submitted or
                // if someone calls stopPropagation before React gets the event.
                // If CSP is used to block javascript: URLs that's fine too. It just won't show this
                // error message but the URL will be logged.
                dom_element.set_attribute(
                    key,
                    // eslint-disable-next-line no-script-url
                    concat!(
                        "javascript:throw new Error('",
                        "A React form was unexpectedly submitted. If you called form.submit() manually, ",
                        "consider using form.requestSubmit() instead. If you\\'re trying to use ",
                        "event.stopPropagation() in a submit event handler, consider also calling ",
                        "event.preventDefault().",
                        "')"
                    ),
                );
            } else {
                if let Kind::Function(_) = js::classify(prev_value) {
                    // When we're switching off a Server Action that was originally hydrated.
                    // The server control these fields during SSR that are now trailing.
                    // The regular diffing doesn't apply since we compare against the previous props.
                    // Instead, we need to force them to be set to whatever they should be now.
                    // This would be a lot cleaner if we did this whole fork in the per-tag approach.
                    if key == "formAction" {
                        if tag != "input" {
                            // Setting the name here isn't completely safe for inputs if this is switching
                            // to become a radio button. In that case we let the tag based override take
                            // control.
                            set_prop(dom_element, tag, "name", js::get(props, "name"), props, js::null());
                        }
                        set_prop(
                            dom_element,
                            tag,
                            "formEncType",
                            js::get(props, "formEncType"),
                            props,
                            js::null(),
                        );
                        set_prop(
                            dom_element,
                            tag,
                            "formMethod",
                            js::get(props, "formMethod"),
                            props,
                            js::null(),
                        );
                        set_prop(
                            dom_element,
                            tag,
                            "formTarget",
                            js::get(props, "formTarget"),
                            props,
                            js::null(),
                        );
                    } else {
                        set_prop(
                            dom_element,
                            tag,
                            "encType",
                            js::get(props, "encType"),
                            props,
                            js::null(),
                        );
                        set_prop(dom_element, tag, "method", js::get(props, "method"), props, js::null());
                        set_prop(dom_element, tag, "target", js::get(props, "target"), props, js::null());
                    }
                }
                if value.is_none() || matches!(js::classify(value), Kind::Symbol(_) | Kind::Bool(_)) {
                    dom_element.remove_attribute(key);
                } else {
                    if __DEV__ {
                        check_attribute_string_coercion(value, key);
                    }
                    let sanitized_value = sanitize_url(value);
                    dom_element.set_attribute(key, sanitized_value);
                }
            }
        }
        "onClick" => {
            // TODO: This cast may not be sound for SVG, MathML or custom elements.
            if value.is_some() {
                if __DEV__ && !matches!(js::classify(value), Kind::Function(_)) {
                    warn_for_invalid_event_listener(key, value);
                }
                trap_click_on_non_interactive_element(html_element::unchecked_from(dom_element));
            }
            return;
        }
        "onScroll" => {
            if value.is_some() {
                if __DEV__ && !matches!(js::classify(value), Kind::Function(_)) {
                    warn_for_invalid_event_listener(key, value);
                }
                listen_to_non_delegated_event("scroll", dom_element);
            }
            return;
        }
        "onScrollEnd" => {
            if value.is_some() {
                if __DEV__ && !matches!(js::classify(value), Kind::Function(_)) {
                    warn_for_invalid_event_listener(key, value);
                }
                listen_to_non_delegated_event("scrollend", dom_element);
                if ENABLE_SCROLL_END_POLYFILL.with(|&enabled| enabled) {
                    // For use by the polyfill.
                    listen_to_non_delegated_event("scroll", dom_element);
                }
            }
            return;
        }
        "dangerouslySetInnerHTML" => {
            if let Some(value) = value {
                if !matches!(
                    js::classify(Some(value)),
                    Kind::Object(_) | Kind::Array(_) | Kind::Null
                ) || !js::has(value, "__html")
                {
                    panic!(
                        "{}",
                        concat!(
                            "`props.dangerouslySetInnerHTML` must be in the form `{__html: ...}`. ",
                            "Please visit https://react.dev/link/dangerously-set-inner-html ",
                            "for more information."
                        )
                    );
                }
                let next_html = js::get(value, "__html");
                if next_html.is_some() {
                    if js::get(props, "children").is_some() {
                        panic!("Can only set one of `children` or `props.dangerouslySetInnerHTML`.");
                    }
                    let last_html = match prev_value {
                        Some(prev_value) => js::get(prev_value, "__html"),
                        None => None,
                    };
                    if !js::strict_equals(last_html, next_html) {
                        dom_element.set_inner_html(unsafe { js::cast(next_html) });
                    }
                }
            }
        }
        // Note: `option.selected` is not updated if `select.multiple` is
        // disabled with `removeAttribute`. We have special logic for handling this.
        "multiple" => {
            js::set(
                dom_element,
                "multiple",
                if js::truthy(value) {
                    Some(js::unknown_of(!matches!(
                        js::classify(value),
                        Kind::Function(_) | Kind::Symbol(_)
                    )))
                } else {
                    value
                },
            );
        }
        "muted" => {
            js::set(
                dom_element,
                "muted",
                if js::truthy(value) {
                    Some(js::unknown_of(!matches!(
                        js::classify(value),
                        Kind::Function(_) | Kind::Symbol(_)
                    )))
                } else {
                    value
                },
            );
        }
        "suppressContentEditableWarning"
        | "suppressHydrationWarning"
        | "defaultValue" // Reserved
        | "defaultChecked"
        | "innerHTML"
        | "ref" => {
            // TODO: `ref` is pretty common, should we move it up?
            // Noop
        }
        "autoFocus" => {
            // We polyfill it separately on the client during commit.
            // We could have excluded it in the property list instead of
            // adding a special case here, but then it wouldn't be emitted
            // on server rendering (but we *do* want to emit it in SSR).
        }
        "xlinkHref" => {
            if value.is_none()
                || matches!(
                    js::classify(value),
                    Kind::Function(_) | Kind::Bool(_) | Kind::Symbol(_)
                )
            {
                dom_element.remove_attribute("xlink:href");
            } else {
                if __DEV__ {
                    check_attribute_string_coercion(value, key);
                }
                let sanitized_value = sanitize_url(value);
                dom_element.set_attribute_ns(Some(XLINK_NAMESPACE), "xlink:href", sanitized_value);
            }
        }
        "contentEditable"
        | "spellCheck"
        | "draggable"
        | "value"
        | "autoReverse"
        | "externalResourcesRequired"
        | "focusable"
        | "preserveAlpha" => {
            // Booleanish String
            // These are "enumerated" attributes that accept "true" and "false".
            // In React, we let users pass `true` and `false` even though technically
            // these aren't boolean attributes (they are coerced to strings).
            // The SVG attributes are case-sensitive. Since the HTML attributes are
            // insensitive they also work even though we canonically use lower case.
            if value.is_some() && !matches!(js::classify(value), Kind::Function(_) | Kind::Symbol(_)) {
                if __DEV__ {
                    check_attribute_string_coercion(value, key);
                }
                dom_element.set_attribute(key, value);
            } else {
                dom_element.remove_attribute(key);
            }
        }
        // Boolean
        "inert"
        // Fallthrough for boolean props that don't have a warning for empty strings.
        | "allowFullScreen"
        | "async"
        | "autoPlay"
        | "controls"
        | "credentialless"
        | "default"
        | "defer"
        | "disabled"
        | "disablePictureInPicture"
        | "disableRemotePlayback"
        | "formNoValidate"
        | "hidden"
        | "loop"
        | "noModule"
        | "noValidate"
        | "open"
        | "playsInline"
        | "readOnly"
        | "required"
        | "reversed"
        | "scoped"
        | "seamless"
        | "itemScope" => {
            if key == "inert" && __DEV__ {
                DID_WARN_FOR_NEW_BOOLEAN_PROPS_WITH_EMPTY_VALUE.with(
                    |did_warn_for_new_boolean_props_with_empty_value| {
                        if js::strict_equals(value, "")
                            && !dict::get(did_warn_for_new_boolean_props_with_empty_value, key)
                                .is_some_and(|&did_warn| did_warn)
                        {
                            dict::set(did_warn_for_new_boolean_props_with_empty_value, key, true);
                            console::error((
                                concat!(
                                    "Received an empty string for a boolean attribute `%s`. ",
                                    "This will treat the attribute as if it were false. ",
                                    "Either pass `false` to silence this warning, or ",
                                    "pass `true` if you used an empty string in earlier versions of React to indicate this attribute is true."
                                ),
                                key,
                            ));
                        }
                    },
                );
            }
            if js::truthy(value) && !matches!(js::classify(value), Kind::Function(_) | Kind::Symbol(_)) {
                dom_element.set_attribute(key, "");
            } else {
                dom_element.remove_attribute(key);
            }
        }
        // Overloaded Boolean
        "capture" | "download" => {
            // An attribute that can be used as a flag as well as with a value.
            // When true, it should be present (set either to an empty string or its name).
            // When false, it should be omitted.
            // For any other value, should be present with that value.
            if js::strict_equals(value, true) {
                dom_element.set_attribute(key, "");
            } else if !js::strict_equals(value, false)
                && value.is_some()
                && !matches!(js::classify(value), Kind::Function(_) | Kind::Symbol(_))
            {
                if __DEV__ {
                    check_attribute_string_coercion(value, key);
                }
                dom_element.set_attribute(key, value);
            } else {
                dom_element.remove_attribute(key);
            }
        }
        "cols" | "rows" | "size" | "span" => {
            // These are HTML attributes that must be positive numbers.
            if value.is_some()
                && !matches!(js::classify(value), Kind::Function(_) | Kind::Symbol(_))
                && !js::is_nan(unsafe { js::cast(value) })
                && unsafe { js::cast::<f64>(value) } >= 1.0
            {
                if __DEV__ {
                    check_attribute_string_coercion(value, key);
                }
                dom_element.set_attribute(key, value);
            } else {
                dom_element.remove_attribute(key);
            }
        }
        "rowSpan" | "start" => {
            // These are HTML attributes that must be numbers.
            if value.is_some()
                && !matches!(js::classify(value), Kind::Function(_) | Kind::Symbol(_))
                && !js::is_nan(unsafe { js::cast(value) })
            {
                if __DEV__ {
                    check_attribute_string_coercion(value, key);
                }
                dom_element.set_attribute(key, value);
            } else {
                dom_element.remove_attribute(key);
            }
        }
        "popover" => {
            listen_to_non_delegated_event("beforetoggle", dom_element);
            listen_to_non_delegated_event("toggle", dom_element);
            set_value_for_attribute(dom_element, "popover", value);
        }
        "xlinkActuate" => {
            set_value_for_namespaced_attribute(dom_element, XLINK_NAMESPACE, "xlink:actuate", value);
        }
        "xlinkArcrole" => {
            set_value_for_namespaced_attribute(dom_element, XLINK_NAMESPACE, "xlink:arcrole", value);
        }
        "xlinkRole" => {
            set_value_for_namespaced_attribute(dom_element, XLINK_NAMESPACE, "xlink:role", value);
        }
        "xlinkShow" => {
            set_value_for_namespaced_attribute(dom_element, XLINK_NAMESPACE, "xlink:show", value);
        }
        "xlinkTitle" => {
            set_value_for_namespaced_attribute(dom_element, XLINK_NAMESPACE, "xlink:title", value);
        }
        "xlinkType" => {
            set_value_for_namespaced_attribute(dom_element, XLINK_NAMESPACE, "xlink:type", value);
        }
        "xmlBase" => {
            set_value_for_namespaced_attribute(dom_element, XML_NAMESPACE, "xml:base", value);
        }
        "xmlLang" => {
            set_value_for_namespaced_attribute(dom_element, XML_NAMESPACE, "xml:lang", value);
        }
        "xmlSpace" => {
            set_value_for_namespaced_attribute(dom_element, XML_NAMESPACE, "xml:space", value);
        }
        // Properties that should not be allowed on custom elements.
        "is" => {
            if __DEV__ && prev_value.is_some() {
                console::error(("Cannot update the \"is\" prop after it has been initialized.",));
            }
            // TODO: We shouldn't actually set this attribute, because we've already
            // passed it to createElement. We don't also need the attribute.
            // However, our tests currently query for it so it's plausible someone
            // else does too so it's break.
            set_value_for_attribute(dom_element, "is", value);
        }
        "innerText" | "textContent" => return,
        _ => {
            if key == "popoverTarget"
                && __DEV__
                && !DID_WARN_POPOVER_TARGET_OBJECT.get()
                && js::classify(value).object().is_some()
            {
                DID_WARN_POPOVER_TARGET_OBJECT.set(true);
                console::error((
                    "The `popoverTarget` prop expects the ID of an Element as a string. Received %s instead.",
                    value,
                ));
            }
            if js::string::length(key) > 2
                && (js::string::get(key, 0.0) == Some("o") || js::string::get(key, 0.0) == Some("O"))
                && (js::string::get(key, 1.0) == Some("n") || js::string::get(key, 1.0) == Some("N"))
            {
                if __DEV__
                    && REGISTRATION_NAME_DEPENDENCIES.with(|registration_name_dependencies| {
                        js::unknown(*registration_name_dependencies).has_own_property(key)
                    })
                    && value.is_some()
                    && !matches!(js::classify(value), Kind::Function(_))
                {
                    warn_for_invalid_event_listener(key, value);
                }
                // Updating events doesn't affect the visuals.
                return;
            } else {
                let attribute_name = get_attribute_alias(key);
                set_value_for_attribute(dom_element, attribute_name, value);
            }
        }
    }
    // To avoid marking things as host mutations we do early returns above.
    track_host_mutation();
}

fn set_prop_on_custom_element(
    dom_element: &'static Element,
    _tag: &str,
    key: &str,
    value: Option<&'static Unknown>,
    props: &'static Unknown,
    prev_value: Option<&'static Unknown>,
) {
    match key {
        "style" => {
            set_value_for_styles(dom_element, value, prev_value);
            return;
        }
        "dangerouslySetInnerHTML" => {
            if let Some(value) = value {
                if !matches!(
                    js::classify(Some(value)),
                    Kind::Object(_) | Kind::Array(_) | Kind::Null
                ) || !js::has(value, "__html")
                {
                    panic!(
                        "{}",
                        concat!(
                            "`props.dangerouslySetInnerHTML` must be in the form `{__html: ...}`. ",
                            "Please visit https://react.dev/link/dangerously-set-inner-html ",
                            "for more information."
                        )
                    );
                }
                let next_html = js::get(value, "__html");
                if next_html.is_some() {
                    if js::get(props, "children").is_some() {
                        panic!("Can only set one of `children` or `props.dangerouslySetInnerHTML`.");
                    }
                    let last_html = match prev_value {
                        Some(prev_value) => js::get(prev_value, "__html"),
                        None => None,
                    };
                    if !js::strict_equals(last_html, next_html) {
                        dom_element.set_inner_html(unsafe { js::cast(next_html) });
                    }
                }
            }
        }
        "children" => {
            if let Kind::String(value) = js::classify(value) {
                set_text_content(dom_element, value);
            } else if let Kind::Number(_) | Kind::BigInt(_) = js::classify(value) {
                set_text_content(dom_element, &js::concat_text(value).unwrap());
            } else {
                return;
            }
        }
        "onScroll" => {
            if value.is_some() {
                if __DEV__ && !matches!(js::classify(value), Kind::Function(_)) {
                    warn_for_invalid_event_listener(key, value);
                }
                listen_to_non_delegated_event("scroll", dom_element);
            }
            return;
        }
        "onScrollEnd" => {
            if value.is_some() {
                if __DEV__ && !matches!(js::classify(value), Kind::Function(_)) {
                    warn_for_invalid_event_listener(key, value);
                }
                listen_to_non_delegated_event("scrollend", dom_element);
                if ENABLE_SCROLL_END_POLYFILL.with(|&enabled| enabled) {
                    // For use by the polyfill.
                    listen_to_non_delegated_event("scroll", dom_element);
                }
            }
            return;
        }
        "onClick" => {
            // TODO: This cast may not be sound for SVG, MathML or custom elements.
            if value.is_some() {
                if __DEV__ && !matches!(js::classify(value), Kind::Function(_)) {
                    warn_for_invalid_event_listener(key, value);
                }
                trap_click_on_non_interactive_element(html_element::unchecked_from(dom_element));
            }
            return;
        }
        "suppressContentEditableWarning" | "suppressHydrationWarning" | "innerHTML" | "ref" => {
            // Noop
            return;
        }
        "innerText" // Properties
        | "textContent" => return,
        // Fall through
        _ => {
            if REGISTRATION_NAME_DEPENDENCIES.with(|registration_name_dependencies| {
                js::unknown(*registration_name_dependencies).has_own_property(key)
            }) {
                if __DEV__ && value.is_some() && !matches!(js::classify(value), Kind::Function(_)) {
                    warn_for_invalid_event_listener(key, value);
                }
                return;
            } else {
                set_value_for_property_on_custom_component(dom_element, key, value);
                // We track mutations inside this call.
                return;
            }
        }
    }
    // To avoid marking things as host mutations we do early returns above.
    track_host_mutation();
}

pub fn set_initial_properties(dom_element: &'static Element, tag: &str, props: &'static Unknown) {
    if __DEV__ {
        validate_properties_in_development(tag, props);
    }

    // TODO: Make sure that we check isMounted before firing any of these events.

    match tag {
        "div" | "span" | "svg" | "path" | "a" | "g" | "p" | "li" => {
            // Fast track the most common tag types
        }
        // img tags previously were implemented as void elements with non delegated events however Safari (and possibly Firefox)
        // begin fetching the image as soon as the `src` or `srcSet` property is set and if we set these before other properties
        // that can modify the request (such as crossorigin) or the resource fetch (such as sizes) then the browser will load
        // the wrong thing or load more than one thing. This implementation ensures src and srcSet are set on the instance last
        "img" => {
            listen_to_non_delegated_event("error", dom_element);
            listen_to_non_delegated_event("load", dom_element);
            // Mostly a port of Void Element logic with special casing to ensure srcset and src are set last
            let mut has_src = false;
            let mut has_src_set = false;
            for prop_key in js::keys_in(props) {
                if !props.has_own_property(&prop_key) {
                    continue;
                }
                let prop_value = js::get(props, &prop_key);
                if prop_value.is_none() {
                    continue;
                }
                match prop_key.as_str() {
                    "src" => has_src = true,
                    "srcSet" => has_src_set = true,
                    "children" | "dangerouslySetInnerHTML" => {
                        // TODO: Can we make this a DEV warning to avoid this deny list?
                        panic!(
                            "{tag} is a void element tag and must neither have `children` nor use `dangerouslySetInnerHTML`."
                        );
                    }
                    // defaultChecked and defaultValue are ignored by setProp
                    _ => set_prop(dom_element, tag, &prop_key, prop_value, props, js::null()),
                }
            }
            if has_src_set {
                set_prop(
                    dom_element,
                    tag,
                    "srcSet",
                    js::get(props, "srcSet"),
                    props,
                    js::null(),
                );
            }
            if has_src {
                set_prop(
                    dom_element,
                    tag,
                    "src",
                    js::get(props, "src"),
                    props,
                    js::null(),
                );
            }
            return;
        }
        "input" => {
            if __DEV__ {
                check_controlled_value_props("input", props);
            }
            // We listen to this event in case to ensure emulated bubble
            // listeners still fire for the invalid event.
            listen_to_non_delegated_event("invalid", dom_element);

            let mut name = js::null();
            let mut r#type = js::null();
            let mut value = js::null();
            let mut default_value = js::null();
            let mut checked = js::null();
            let mut default_checked = js::null();
            for prop_key in js::keys_in(props) {
                if !props.has_own_property(&prop_key) {
                    continue;
                }
                let prop_value = js::get(props, &prop_key);
                if prop_value.is_none() {
                    continue;
                }
                match prop_key.as_str() {
                    "name" => name = prop_value,
                    "type" => r#type = prop_value,
                    "checked" => checked = prop_value,
                    "defaultChecked" => default_checked = prop_value,
                    "value" => value = prop_value,
                    "defaultValue" => default_value = prop_value,
                    "children" | "dangerouslySetInnerHTML" => {
                        if prop_value.is_some() {
                            panic!(
                                "{tag} is a void element tag and must neither have `children` nor use `dangerouslySetInnerHTML`."
                            );
                        }
                    }
                    _ => set_prop(dom_element, tag, &prop_key, prop_value, props, js::null()),
                }
            }
            // TODO: Make sure we check if this is still unmounted or do any clean
            // up necessary since we never stop tracking anymore.
            validate_input_props(dom_element, props);
            init_input(
                dom_element,
                value,
                default_value,
                checked,
                default_checked,
                r#type,
                name,
                false,
            );
            return;
        }
        "select" => {
            if __DEV__ {
                check_controlled_value_props("select", props);
            }
            // We listen to this event in case to ensure emulated bubble
            // listeners still fire for the invalid event.
            listen_to_non_delegated_event("invalid", dom_element);
            let mut value = js::null();
            let mut default_value = js::null();
            let mut multiple = js::null();
            for prop_key in js::keys_in(props) {
                if !props.has_own_property(&prop_key) {
                    continue;
                }
                let prop_value = js::get(props, &prop_key);
                if prop_value.is_none() {
                    continue;
                }
                match prop_key.as_str() {
                    "value" => {
                        value = prop_value;
                        // This is handled by initSelect below.
                    }
                    "defaultValue" => {
                        default_value = prop_value;
                        // This is handled by initSelect below.
                    }
                    _ => {
                        if prop_key == "multiple" {
                            multiple = prop_value;
                            // TODO: We don't actually have to fall through here because we set it
                            // in initSelect anyway. We can remove the special case in setProp.
                        }
                        set_prop(dom_element, tag, &prop_key, prop_value, props, js::null());
                    }
                }
            }
            validate_select_props(dom_element, props);
            init_select(dom_element, value, default_value, multiple);
            return;
        }
        "textarea" => {
            if __DEV__ {
                check_controlled_value_props("textarea", props);
            }
            // We listen to this event in case to ensure emulated bubble
            // listeners still fire for the invalid event.
            listen_to_non_delegated_event("invalid", dom_element);
            let mut value = js::null();
            let mut default_value = js::null();
            let mut children = js::null();
            for prop_key in js::keys_in(props) {
                if !props.has_own_property(&prop_key) {
                    continue;
                }
                let prop_value = js::get(props, &prop_key);
                if prop_value.is_none() {
                    continue;
                }
                match prop_key.as_str() {
                    "value" => {
                        value = prop_value;
                        // This is handled by initTextarea below.
                    }
                    "defaultValue" => default_value = prop_value,
                    "children" => {
                        children = prop_value;
                        // Handled by initTextarea above.
                    }
                    "dangerouslySetInnerHTML" => {
                        if prop_value.is_some() {
                            // TODO: Do we really need a special error message for this. It's also pretty blunt.
                            panic!("`dangerouslySetInnerHTML` does not make sense on <textarea>.");
                        }
                    }
                    _ => set_prop(dom_element, tag, &prop_key, prop_value, props, js::null()),
                }
            }
            // TODO: Make sure we check if this is still unmounted or do any clean
            // up necessary since we never stop tracking anymore.
            validate_textarea_props(dom_element, props);
            init_textarea(dom_element, value, default_value, children);
            return;
        }
        "option" => {
            validate_option_props(dom_element, props);
            for prop_key in js::keys_in(props) {
                if !props.has_own_property(&prop_key) {
                    continue;
                }
                let prop_value = js::get(props, &prop_key);
                if prop_value.is_none() {
                    continue;
                }
                match prop_key.as_str() {
                    "selected" => {
                        // TODO: Remove support for selected on option.
                        js::set(
                            dom_element,
                            "selected",
                            if js::truthy(prop_value) {
                                Some(js::unknown_of(!matches!(
                                    js::classify(prop_value),
                                    Kind::Function(_) | Kind::Symbol(_)
                                )))
                            } else {
                                prop_value
                            },
                        );
                    }
                    _ => set_prop(dom_element, tag, &prop_key, prop_value, props, js::null()),
                }
            }
            return;
        }
        "dialog" => {
            listen_to_non_delegated_event("beforetoggle", dom_element);
            listen_to_non_delegated_event("toggle", dom_element);
            listen_to_non_delegated_event("cancel", dom_element);
            listen_to_non_delegated_event("close", dom_element);
        }
        "iframe" | "object" => {
            // We listen to this event in case to ensure emulated bubble
            // listeners still fire for the load event.
            listen_to_non_delegated_event("load", dom_element);
        }
        "video" | "audio" => {
            // We listen to these events in case to ensure emulated bubble
            // listeners still fire for all the media events.
            MEDIA_EVENT_TYPES.with(|media_event_types| {
                for media_event_type in media_event_types {
                    listen_to_non_delegated_event(media_event_type, dom_element);
                }
            });
        }
        "image" => {
            // We listen to these events in case to ensure emulated bubble
            // listeners still fire for error and load events.
            listen_to_non_delegated_event("error", dom_element);
            listen_to_non_delegated_event("load", dom_element);
        }
        "details" => {
            // We listen to this event in case to ensure emulated bubble
            // listeners still fire for the toggle event.
            listen_to_non_delegated_event("toggle", dom_element);
        }
        "embed" | "source" | "link" | "area" | "base" | "br" | "col" | "hr" | "keygen" | "meta"
        | "param" | "track" | "wbr" | "menuitem" => {
            if tag == "embed" || tag == "source" || tag == "link" {
                // These are void elements that also need delegated events.
                listen_to_non_delegated_event("error", dom_element);
                listen_to_non_delegated_event("load", dom_element);
                // We fallthrough to the return of the void elements
            }
            // Void elements
            for prop_key in js::keys_in(props) {
                if !props.has_own_property(&prop_key) {
                    continue;
                }
                let prop_value = js::get(props, &prop_key);
                if prop_value.is_none() {
                    continue;
                }
                match prop_key.as_str() {
                    "children" | "dangerouslySetInnerHTML" => {
                        // TODO: Can we make this a DEV warning to avoid this deny list?
                        panic!(
                            "{tag} is a void element tag and must neither have `children` nor use `dangerouslySetInnerHTML`."
                        );
                    }
                    // defaultChecked and defaultValue are ignored by setProp
                    _ => set_prop(dom_element, tag, &prop_key, prop_value, props, js::null()),
                }
            }
            return;
        }
        _ => {
            if is_custom_element(tag, props) {
                for prop_key in js::keys_in(props) {
                    if !props.has_own_property(&prop_key) {
                        continue;
                    }
                    let prop_value = js::get(props, &prop_key);
                    if js::strict_equals(prop_value, ()) {
                        continue;
                    }
                    set_prop_on_custom_element(
                        dom_element,
                        tag,
                        &prop_key,
                        prop_value,
                        props,
                        None,
                    );
                }
                return;
            }
        }
    }

    for prop_key in js::keys_in(props) {
        if !props.has_own_property(&prop_key) {
            continue;
        }
        let prop_value = js::get(props, &prop_key);
        if prop_value.is_none() {
            continue;
        }
        set_prop(dom_element, tag, &prop_key, prop_value, props, js::null());
    }
}

thread_local! {
    static EMPTY_PROPS: &'static Dict<Option<&'static Unknown>> = dict::new();
}

pub fn clear_singleton_properties(
    dom_element: &'static Element,
    tag: &str,
    props: &'static Unknown,
) {
    // This is equivalent to updating to empty props for tags without
    // tag-specific update logic. Host singletons are limited to html, head, and
    // body, so they always use this generic path.
    for prop_key in js::keys_in(props) {
        let prop_value = js::get(props, &prop_key);
        if props.has_own_property(&prop_key) && prop_value.is_some() {
            EMPTY_PROPS.with(|&empty_props| {
                set_prop(
                    dom_element,
                    tag,
                    &prop_key,
                    js::null(),
                    js::unknown(empty_props),
                    prop_value,
                );
            });
        }
    }
}

pub fn update_properties(
    dom_element: &'static Element,
    tag: &str,
    last_props: &'static Unknown,
    next_props: &'static Unknown,
) {
    if __DEV__ {
        validate_properties_in_development(tag, next_props);
    }

    match tag {
        "div" | "span" | "svg" | "path" | "a" | "g" | "p" | "li" => {
            // Fast track the most common tag types
        }
        "input" => {
            let mut name = js::null();
            let mut r#type = js::null();
            let mut value = js::null();
            let mut default_value = js::null();
            let mut last_default_value = js::null();
            let mut checked = js::null();
            let mut default_checked = js::null();
            for prop_key in js::keys_in(last_props) {
                let last_prop = js::get(last_props, &prop_key);
                if last_props.has_own_property(&prop_key) && last_prop.is_some() {
                    match prop_key.as_str() {
                        "checked" => {}
                        "value" => {
                            // This is handled by updateWrapper below.
                        }
                        _ => {
                            if prop_key == "defaultValue" {
                                last_default_value = last_prop;
                            }
                            // defaultChecked and defaultValue are ignored by setProp
                            if !next_props.has_own_property(&prop_key) {
                                set_prop(
                                    dom_element,
                                    tag,
                                    &prop_key,
                                    js::null(),
                                    next_props,
                                    last_prop,
                                );
                            }
                        }
                    }
                }
            }
            for prop_key in js::keys_in(next_props) {
                let next_prop = js::get(next_props, &prop_key);
                let last_prop = js::get(last_props, &prop_key);
                if next_props.has_own_property(&prop_key)
                    && (next_prop.is_some() || last_prop.is_some())
                {
                    match prop_key.as_str() {
                        "type" => {
                            if !js::strict_equals(next_prop, last_prop) {
                                track_host_mutation();
                            }
                            r#type = next_prop;
                        }
                        "name" => {
                            if !js::strict_equals(next_prop, last_prop) {
                                track_host_mutation();
                            }
                            name = next_prop;
                        }
                        "checked" => {
                            if !js::strict_equals(next_prop, last_prop) {
                                track_host_mutation();
                            }
                            checked = next_prop;
                        }
                        "defaultChecked" => {
                            if !js::strict_equals(next_prop, last_prop) {
                                track_host_mutation();
                            }
                            default_checked = next_prop;
                        }
                        "value" => {
                            if !js::strict_equals(next_prop, last_prop) {
                                track_host_mutation();
                            }
                            value = next_prop;
                        }
                        "defaultValue" => {
                            if !js::strict_equals(next_prop, last_prop) {
                                track_host_mutation();
                            }
                            default_value = next_prop;
                        }
                        "children" | "dangerouslySetInnerHTML" => {
                            if next_prop.is_some() {
                                panic!(
                                    "{tag} is a void element tag and must neither have `children` nor use `dangerouslySetInnerHTML`."
                                );
                            }
                        }
                        _ => {
                            if !js::strict_equals(next_prop, last_prop) {
                                set_prop(
                                    dom_element,
                                    tag,
                                    &prop_key,
                                    next_prop,
                                    next_props,
                                    last_prop,
                                );
                            }
                        }
                    }
                }
            }

            if __DEV__ {
                let was_controlled = if js::strict_equals(js::get(last_props, "type"), "checkbox")
                    || js::strict_equals(js::get(last_props, "type"), "radio")
                {
                    js::get(last_props, "checked").is_some()
                } else {
                    js::get(last_props, "value").is_some()
                };
                let is_controlled = if js::strict_equals(js::get(next_props, "type"), "checkbox")
                    || js::strict_equals(js::get(next_props, "type"), "radio")
                {
                    js::get(next_props, "checked").is_some()
                } else {
                    js::get(next_props, "value").is_some()
                };

                if !was_controlled && is_controlled && !DID_WARN_UNCONTROLLED_TO_CONTROLLED.get() {
                    console::error((concat!(
                        "A component is changing an uncontrolled input to be controlled. ",
                        "This is likely caused by the value changing from undefined to ",
                        "a defined value, which should not happen. ",
                        "Decide between using a controlled or uncontrolled input ",
                        "element for the lifetime of the component. More info: https://react.dev/link/controlled-components"
                    ),));
                    DID_WARN_UNCONTROLLED_TO_CONTROLLED.set(true);
                }
                if was_controlled && !is_controlled && !DID_WARN_CONTROLLED_TO_UNCONTROLLED.get() {
                    console::error((concat!(
                        "A component is changing a controlled input to be uncontrolled. ",
                        "This is likely caused by the value changing from a defined to ",
                        "undefined, which should not happen. ",
                        "Decide between using a controlled or uncontrolled input ",
                        "element for the lifetime of the component. More info: https://react.dev/link/controlled-components"
                    ),));
                    DID_WARN_CONTROLLED_TO_UNCONTROLLED.set(true);
                }
            }

            // Update the wrapper around inputs *after* updating props. This has to
            // happen after updating the rest of props. Otherwise HTML5 input validations
            // raise warnings and prevent the new value from being assigned.
            update_input(
                dom_element,
                value,
                default_value,
                last_default_value,
                checked,
                default_checked,
                r#type,
                name,
            );
            return;
        }
        "select" => {
            let mut value = js::null();
            let mut default_value = js::null();
            let mut multiple = js::null();
            let mut was_multiple = js::null();
            for prop_key in js::keys_in(last_props) {
                let last_prop = js::get(last_props, &prop_key);
                if last_props.has_own_property(&prop_key) && last_prop.is_some() {
                    match prop_key.as_str() {
                        "value" => {
                            // This is handled by updateWrapper below.
                        }
                        // defaultValue are ignored by setProp
                        _ => {
                            if prop_key == "multiple" {
                                was_multiple = last_prop;
                                // TODO: Move special case in here from setProp.
                            }
                            if !next_props.has_own_property(&prop_key) {
                                set_prop(
                                    dom_element,
                                    tag,
                                    &prop_key,
                                    js::null(),
                                    next_props,
                                    last_prop,
                                );
                            }
                        }
                    }
                }
            }
            for prop_key in js::keys_in(next_props) {
                let next_prop = js::get(next_props, &prop_key);
                let last_prop = js::get(last_props, &prop_key);
                if next_props.has_own_property(&prop_key)
                    && (next_prop.is_some() || last_prop.is_some())
                {
                    match prop_key.as_str() {
                        "value" => {
                            if !js::strict_equals(next_prop, last_prop) {
                                track_host_mutation();
                            }
                            value = next_prop;
                            // This is handled by updateSelect below.
                        }
                        "defaultValue" => {
                            if !js::strict_equals(next_prop, last_prop) {
                                track_host_mutation();
                            }
                            default_value = next_prop;
                        }
                        _ => {
                            if prop_key == "multiple" {
                                if !js::strict_equals(next_prop, last_prop) {
                                    track_host_mutation();
                                }
                                multiple = next_prop;
                                // TODO: Just move the special case in here from setProp.
                            }
                            if !js::strict_equals(next_prop, last_prop) {
                                set_prop(
                                    dom_element,
                                    tag,
                                    &prop_key,
                                    next_prop,
                                    next_props,
                                    last_prop,
                                );
                            }
                        }
                    }
                }
            }
            // <select> value update needs to occur after <option> children
            // reconciliation
            update_select(dom_element, value, default_value, multiple, was_multiple);
            return;
        }
        "textarea" => {
            let mut value = js::null();
            let mut default_value = js::null();
            for prop_key in js::keys_in(last_props) {
                let last_prop = js::get(last_props, &prop_key);
                if last_props.has_own_property(&prop_key)
                    && last_prop.is_some()
                    && !next_props.has_own_property(&prop_key)
                {
                    match prop_key.as_str() {
                        "value" => {
                            // This is handled by updateTextarea below.
                        }
                        "children" => {
                            // TODO: This doesn't actually do anything if it updates.
                        }
                        // defaultValue is ignored by setProp
                        _ => set_prop(
                            dom_element,
                            tag,
                            &prop_key,
                            js::null(),
                            next_props,
                            last_prop,
                        ),
                    }
                }
            }
            for prop_key in js::keys_in(next_props) {
                let next_prop = js::get(next_props, &prop_key);
                let last_prop = js::get(last_props, &prop_key);
                if next_props.has_own_property(&prop_key)
                    && (next_prop.is_some() || last_prop.is_some())
                {
                    match prop_key.as_str() {
                        "value" => {
                            if !js::strict_equals(next_prop, last_prop) {
                                track_host_mutation();
                            }
                            value = next_prop;
                            // This is handled by updateTextarea below.
                        }
                        "defaultValue" => {
                            if !js::strict_equals(next_prop, last_prop) {
                                track_host_mutation();
                            }
                            default_value = next_prop;
                        }
                        "children" => {
                            // TODO: This doesn't actually do anything if it updates.
                        }
                        "dangerouslySetInnerHTML" => {
                            if next_prop.is_some() {
                                // TODO: Do we really need a special error message for this. It's also pretty blunt.
                                panic!(
                                    "`dangerouslySetInnerHTML` does not make sense on <textarea>."
                                );
                            }
                        }
                        _ => {
                            if !js::strict_equals(next_prop, last_prop) {
                                set_prop(
                                    dom_element,
                                    tag,
                                    &prop_key,
                                    next_prop,
                                    next_props,
                                    last_prop,
                                );
                            }
                        }
                    }
                }
            }
            update_textarea(dom_element, value, default_value);
            return;
        }
        "option" => {
            for prop_key in js::keys_in(last_props) {
                let last_prop = js::get(last_props, &prop_key);
                if last_props.has_own_property(&prop_key)
                    && last_prop.is_some()
                    && !next_props.has_own_property(&prop_key)
                {
                    match prop_key.as_str() {
                        "selected" => {
                            // TODO: Remove support for selected on option.
                            js::set(dom_element, "selected", false);
                        }
                        _ => set_prop(
                            dom_element,
                            tag,
                            &prop_key,
                            js::null(),
                            next_props,
                            last_prop,
                        ),
                    }
                }
            }
            for prop_key in js::keys_in(next_props) {
                let next_prop = js::get(next_props, &prop_key);
                let last_prop = js::get(last_props, &prop_key);
                if next_props.has_own_property(&prop_key)
                    && !js::strict_equals(next_prop, last_prop)
                    && (next_prop.is_some() || last_prop.is_some())
                {
                    match prop_key.as_str() {
                        "selected" => {
                            if !js::strict_equals(next_prop, last_prop) {
                                track_host_mutation();
                            }
                            // TODO: Remove support for selected on option.
                            js::set(
                                dom_element,
                                "selected",
                                if js::truthy(next_prop) {
                                    Some(js::unknown_of(!matches!(
                                        js::classify(next_prop),
                                        Kind::Function(_) | Kind::Symbol(_)
                                    )))
                                } else {
                                    next_prop
                                },
                            );
                        }
                        _ => set_prop(
                            dom_element,
                            tag,
                            &prop_key,
                            next_prop,
                            next_props,
                            last_prop,
                        ),
                    }
                }
            }
            return;
        }
        "img" | "link" | "area" | "base" | "br" | "col" | "embed" | "hr" | "keygen" | "meta"
        | "param" | "source" | "track" | "wbr" | "menuitem" => {
            // Void elements
            for prop_key in js::keys_in(last_props) {
                let last_prop = js::get(last_props, &prop_key);
                if last_props.has_own_property(&prop_key)
                    && last_prop.is_some()
                    && !next_props.has_own_property(&prop_key)
                {
                    set_prop(
                        dom_element,
                        tag,
                        &prop_key,
                        js::null(),
                        next_props,
                        last_prop,
                    );
                }
            }
            for prop_key in js::keys_in(next_props) {
                let next_prop = js::get(next_props, &prop_key);
                let last_prop = js::get(last_props, &prop_key);
                if next_props.has_own_property(&prop_key)
                    && !js::strict_equals(next_prop, last_prop)
                    && (next_prop.is_some() || last_prop.is_some())
                {
                    match prop_key.as_str() {
                        "children" | "dangerouslySetInnerHTML" => {
                            if next_prop.is_some() {
                                // TODO: Can we make this a DEV warning to avoid this deny list?
                                panic!(
                                    "{tag} is a void element tag and must neither have `children` nor use `dangerouslySetInnerHTML`."
                                );
                            }
                        }
                        // defaultChecked and defaultValue are ignored by setProp
                        _ => set_prop(
                            dom_element,
                            tag,
                            &prop_key,
                            next_prop,
                            next_props,
                            last_prop,
                        ),
                    }
                }
            }
            return;
        }
        _ => {
            if is_custom_element(tag, next_props) {
                for prop_key in js::keys_in(last_props) {
                    let last_prop = js::get(last_props, &prop_key);
                    if last_props.has_own_property(&prop_key)
                        && !js::strict_equals(last_prop, ())
                        && !next_props.has_own_property(&prop_key)
                    {
                        set_prop_on_custom_element(
                            dom_element,
                            tag,
                            &prop_key,
                            None,
                            next_props,
                            last_prop,
                        );
                    }
                }
                for prop_key in js::keys_in(next_props) {
                    let next_prop = js::get(next_props, &prop_key);
                    let last_prop = js::get(last_props, &prop_key);
                    if next_props.has_own_property(&prop_key)
                        && !js::strict_equals(next_prop, last_prop)
                        && (!js::strict_equals(next_prop, ()) || !js::strict_equals(last_prop, ()))
                    {
                        set_prop_on_custom_element(
                            dom_element,
                            tag,
                            &prop_key,
                            next_prop,
                            next_props,
                            last_prop,
                        );
                    }
                }
                return;
            }
        }
    }

    for prop_key in js::keys_in(last_props) {
        let last_prop = js::get(last_props, &prop_key);
        if last_props.has_own_property(&prop_key)
            && last_prop.is_some()
            && !next_props.has_own_property(&prop_key)
        {
            set_prop(
                dom_element,
                tag,
                &prop_key,
                js::null(),
                next_props,
                last_prop,
            );
        }
    }
    for prop_key in js::keys_in(next_props) {
        let next_prop = js::get(next_props, &prop_key);
        let last_prop = js::get(last_props, &prop_key);
        if next_props.has_own_property(&prop_key)
            && !js::strict_equals(next_prop, last_prop)
            && (next_prop.is_some() || last_prop.is_some())
        {
            set_prop(
                dom_element,
                tag,
                &prop_key,
                next_prop,
                next_props,
                last_prop,
            );
        }
    }
}

fn get_possible_standard_name(prop_name: &str) -> Option<&'static str> {
    if __DEV__ {
        let lower_cased_name = prop_name.to_lowercase();
        if !js::unknown(
            POSSIBLE_STANDARD_NAMES.with(|possible_standard_names| *possible_standard_names),
        )
        .has_own_property(&lower_cased_name)
        {
            return js::null();
        }
        return dict::get(
            POSSIBLE_STANDARD_NAMES.with(|possible_standard_names| *possible_standard_names),
            &lower_cased_name,
        )
        .copied()
        .filter(|standard_name| !standard_name.is_empty())
        .or(js::null());
    }
    js::null()
}

fn get_prop_name_from_attribute_name(attr_name: &str) -> &str {
    match attr_name {
        "class" => "className",
        "for" => "htmlFor",
        // TODO: The rest of the aliases.
        _ => attr_name,
    }
}

pub fn get_props_from_element(dom_element: &Element) -> &'static Dict<&'static Unknown> {
    let server_differences: &'static Dict<&'static Unknown> = dict::new();
    let attributes = dom_element.attributes();
    for i in 0..attributes.length() as usize {
        let attr = &attributes[i];
        dict::set(
            server_differences,
            get_prop_name_from_attribute_name(&attr.name()),
            if attr.name().to_lowercase() == "style" {
                js::unknown(get_styles_object_from_element(dom_element))
            } else {
                js::unknown_of(attr.value())
            },
        );
    }
    server_differences
}

fn get_styles_object_from_element(dom_element: &Element) -> &'static Dict<String> {
    let server_value_in_object_form: &'static Dict<String> = dict::new();
    let html_element = html_element::unchecked_from(dom_element);
    let style = html_element.style();
    for i in 0..style.length() as usize {
        let style_name: String = style.item(i as f64);
        // TODO: We should use the original prop value here if it is equivalent.
        // TODO: We could use the original client capitalization if the equivalent
        // other capitalization exists in the DOM.
        if style_name == "view-transition-name" && is_expected_view_transition_name(html_element) {
            // This is a view transition name added by the Fizz runtime, not the user's props.
        } else {
            dict::set(
                server_value_in_object_form,
                &style_name,
                style.get_property_value(&style_name),
            );
        }
    }
    server_value_in_object_form
}

fn diff_hydrated_styles(
    dom_element: &Element,
    value: Option<&'static Unknown>,
    server_differences: &Dict<Option<&'static Unknown>>,
) {
    if value.is_some()
        && !matches!(
            js::classify(value),
            Kind::Object(_) | Kind::Array(_) | Kind::Null
        )
    {
        if __DEV__ {
            console::error((concat!(
                "The `style` prop expects a mapping from style properties to values, ",
                "not a string. For example, style={{marginRight: spacing + 'em'}} when ",
                "using JSX."
            ),));
        }
        return;
    }
    // First we compare the string form and see if it's equivalent.
    // This lets us bail out on anything that used to pass in this form.
    // It also lets us compare anything that's not parsed by this browser.
    let client_value = create_dangerous_string_for_styles(value);
    let server_value = dom_element.get_attribute("style");

    if server_value == client_value {
        return;
    }
    let normalized_client_value =
        normalize_markup_for_text_or_attribute(client_value.map(js::unknown_of));
    let normalized_server_value =
        normalize_markup_for_text_or_attribute(server_value.map(js::unknown_of));
    if normalized_server_value == normalized_client_value {
        return;
    }

    if
    // Trailing semi-colon means this was regenerated.
    js::string::get(
        &normalized_server_value,
        f64::from(js::string::length(&normalized_server_value)) - 1.0,
    ) == Some(";")
        // TODO: Should we just ignore any style if the style as been manipulated?
        && has_view_transition(html_element::unchecked_from(dom_element))
    {
        // If this had a view transition we might have applied a view transition
        // name/class and removed it. If that happens, the style attribute gets
        // regenerated from the style object. This means we've lost the format
        // that we sent from the server and is unable to diff it. We just treat
        // it as passing even if it should be a mismatch in this edge case.
        return;
    }

    // Otherwise, we create the object from the DOM for the diff view.
    dict::set(
        server_differences,
        "style",
        Some(js::unknown(get_styles_object_from_element(dom_element))),
    );
}

fn hydrate_attribute(
    dom_element: &Element,
    prop_key: &str,
    attribute_name: &str,
    value: Option<&'static Unknown>,
    extra_attributes: &Set<str>,
    server_differences: &Dict<Option<&'static Unknown>>,
) {
    extra_attributes.delete(attribute_name);
    let server_value = dom_element.get_attribute(attribute_name);
    if server_value.is_none() {
        match js::classify(value) {
            Kind::Undefined | Kind::Function(_) | Kind::Symbol(_) | Kind::Bool(_) => return,
            _ => {}
        }
    } else if value.is_none() {
        // We had an attribute but shouldn't have had one, so read it
        // for the error message.
    } else {
        match js::classify(value) {
            Kind::Function(_) | Kind::Symbol(_) | Kind::Bool(_) => {}
            _ => {
                if __DEV__ {
                    check_attribute_string_coercion(value, prop_key);
                }
                if js::strict_equals(&server_value, js::concat_text(value).unwrap()) {
                    return;
                }
            }
        }
    }
    warn_for_prop_difference(
        prop_key,
        server_value.map(js::unknown_of),
        value,
        server_differences,
    );
}

fn hydrate_boolean_attribute(
    dom_element: &Element,
    prop_key: &str,
    attribute_name: &str,
    value: Option<&'static Unknown>,
    extra_attributes: &Set<str>,
    server_differences: &Dict<Option<&'static Unknown>>,
) {
    extra_attributes.delete(attribute_name);
    let server_value = dom_element.get_attribute(attribute_name);
    if server_value.is_none() {
        match js::classify(value) {
            Kind::Function(_) | Kind::Symbol(_) => return,
            _ => {}
        }
        if !js::truthy(value) {
            return;
        }
    } else {
        match js::classify(value) {
            Kind::Function(_) | Kind::Symbol(_) => {}
            _ => {
                if js::truthy(value) {
                    // If this was a boolean, it doesn't matter what the value is
                    // the fact that we have it is the same as the expected.
                    // As long as it's positive.
                    return;
                }
            }
        }
    }
    warn_for_prop_difference(
        prop_key,
        server_value.map(js::unknown_of),
        value,
        server_differences,
    );
}

fn hydrate_overloaded_boolean_attribute(
    dom_element: &Element,
    prop_key: &str,
    attribute_name: &str,
    value: Option<&'static Unknown>,
    extra_attributes: &Set<str>,
    server_differences: &Dict<Option<&'static Unknown>>,
) {
    extra_attributes.delete(attribute_name);
    let server_value = dom_element.get_attribute(attribute_name);
    if server_value.is_none() {
        match js::classify(value) {
            Kind::Undefined | Kind::Function(_) | Kind::Symbol(_) => return,
            _ => {}
        }
        if js::strict_equals(value, false) {
            return;
        }
    } else if value.is_none() {
        // We had an attribute but shouldn't have had one, so read it
        // for the error message.
    } else {
        match js::classify(value) {
            Kind::Function(_) | Kind::Symbol(_) => {}
            Kind::Bool(_) => {
                if js::strict_equals(value, true) && server_value.as_deref() == Some("") {
                    return;
                }
            }
            _ => {
                if __DEV__ {
                    check_attribute_string_coercion(value, prop_key);
                }
                if js::strict_equals(&server_value, js::concat_text(value).unwrap()) {
                    return;
                }
            }
        }
    }
    warn_for_prop_difference(
        prop_key,
        server_value.map(js::unknown_of),
        value,
        server_differences,
    );
}

fn hydrate_booleanish_attribute(
    dom_element: &Element,
    prop_key: &str,
    attribute_name: &str,
    value: Option<&'static Unknown>,
    extra_attributes: &Set<str>,
    server_differences: &Dict<Option<&'static Unknown>>,
) {
    extra_attributes.delete(attribute_name);
    let server_value = dom_element.get_attribute(attribute_name);
    if server_value.is_none() {
        match js::classify(value) {
            Kind::Undefined | Kind::Function(_) | Kind::Symbol(_) => return,
            _ => {}
        }
    } else if value.is_none() {
        // We had an attribute but shouldn't have had one, so read it
        // for the error message.
    } else {
        match js::classify(value) {
            Kind::Function(_) | Kind::Symbol(_) => {}
            _ => {
                if __DEV__ {
                    check_attribute_string_coercion(value, attribute_name);
                }
                if js::strict_equals(&server_value, js::concat_text(value).unwrap()) {
                    return;
                }
            }
        }
    }
    warn_for_prop_difference(
        prop_key,
        server_value.map(js::unknown_of),
        value,
        server_differences,
    );
}

fn hydrate_numeric_attribute(
    dom_element: &Element,
    prop_key: &str,
    attribute_name: &str,
    value: Option<&'static Unknown>,
    extra_attributes: &Set<str>,
    server_differences: &Dict<Option<&'static Unknown>>,
) {
    extra_attributes.delete(attribute_name);
    let server_value = dom_element.get_attribute(attribute_name);
    if server_value.is_none() {
        match js::classify(value) {
            Kind::Undefined | Kind::Function(_) | Kind::Symbol(_) | Kind::Bool(_) => return,
            _ => {}
        }
        if js::is_nan(unsafe { js::cast(value) }) {
            return;
        }
    } else if value.is_none() {
        // We had an attribute but shouldn't have had one, so read it
        // for the error message.
    } else {
        match js::classify(value) {
            Kind::Function(_) | Kind::Symbol(_) | Kind::Bool(_) => {}
            _ => {
                if js::is_nan(unsafe { js::cast(value) }) {
                    // We had an attribute but shouldn't have had one, so read it
                    // for the error message.
                } else {
                    if __DEV__ {
                        check_attribute_string_coercion(value, prop_key);
                    }
                    if js::strict_equals(&server_value, js::concat_text(value).unwrap()) {
                        return;
                    }
                }
            }
        }
    }
    warn_for_prop_difference(
        prop_key,
        server_value.map(js::unknown_of),
        value,
        server_differences,
    );
}

fn hydrate_positive_numeric_attribute(
    dom_element: &Element,
    prop_key: &str,
    attribute_name: &str,
    value: Option<&'static Unknown>,
    extra_attributes: &Set<str>,
    server_differences: &Dict<Option<&'static Unknown>>,
) {
    extra_attributes.delete(attribute_name);
    let server_value = dom_element.get_attribute(attribute_name);
    if server_value.is_none() {
        match js::classify(value) {
            Kind::Undefined | Kind::Function(_) | Kind::Symbol(_) | Kind::Bool(_) => return,
            _ => {}
        }
        if js::is_nan(unsafe { js::cast(value) }) || unsafe { js::cast::<f64>(value) } < 1.0 {
            return;
        }
    } else if value.is_none() {
        // We had an attribute but shouldn't have had one, so read it
        // for the error message.
    } else {
        match js::classify(value) {
            Kind::Function(_) | Kind::Symbol(_) | Kind::Bool(_) => {}
            _ => {
                if js::is_nan(unsafe { js::cast(value) }) || unsafe { js::cast::<f64>(value) } < 1.0
                {
                    // We had an attribute but shouldn't have had one, so read it
                    // for the error message.
                } else {
                    if __DEV__ {
                        check_attribute_string_coercion(value, prop_key);
                    }
                    if js::strict_equals(&server_value, js::concat_text(value).unwrap()) {
                        return;
                    }
                }
            }
        }
    }
    warn_for_prop_difference(
        prop_key,
        server_value.map(js::unknown_of),
        value,
        server_differences,
    );
}

fn hydrate_sanitized_attribute(
    dom_element: &Element,
    prop_key: &str,
    attribute_name: &str,
    value: Option<&'static Unknown>,
    extra_attributes: &Set<str>,
    server_differences: &Dict<Option<&'static Unknown>>,
) {
    extra_attributes.delete(attribute_name);
    let server_value = dom_element.get_attribute(attribute_name);
    if server_value.is_none() {
        match js::classify(value) {
            Kind::Undefined | Kind::Function(_) | Kind::Symbol(_) | Kind::Bool(_) => return,
            _ => {}
        }
    } else if value.is_none() {
        // We had an attribute but shouldn't have had one, so read it
        // for the error message.
    } else {
        match js::classify(value) {
            Kind::Function(_) | Kind::Symbol(_) | Kind::Bool(_) => {}
            _ => {
                if __DEV__ {
                    check_attribute_string_coercion(value, prop_key);
                }
                let sanitized_value =
                    sanitize_url(Some(js::unknown_of(js::concat_text(value).unwrap())));
                if js::strict_equals(&server_value, sanitized_value) {
                    return;
                }
            }
        }
    }
    warn_for_prop_difference(
        prop_key,
        server_value.map(js::unknown_of),
        value,
        server_differences,
    );
}

fn hydrate_src_object_attribute(
    dom_element: &Element,
    value: &'static Unknown,
    extra_attributes: &Set<str>,
    server_differences: &Dict<Option<&'static Unknown>>,
) {
    let attribute_name = "src";
    extra_attributes.delete(attribute_name);
    let server_value = dom_element.get_attribute(attribute_name);
    if let Some(server_value) = &server_value {
        let size = js::get(value, "size");
        let r#type = js::get(value, "type");
        if let Kind::Number(size) = js::classify(size)
            && let Kind::String(r#type) = js::classify(r#type)
            && js::string::index_of(
                server_value,
                &("data:".to_owned() + r#type + ";base64,"),
                None,
            ) == 0.0
        {
            // For Blobs we don't bother reading the actual data but just diff by checking if
            // the byte length size of the Blob maches the length of the data url.
            let prefix_length = 5.0 + f64::from(js::string::length(r#type)) + 8.0;
            let mut byte_length =
                ((f64::from(js::string::length(server_value)) - prefix_length) / 4.0) * 3.0;
            if js::string::get(
                server_value,
                f64::from(js::string::length(server_value)) - 1.0,
            ) == Some("=")
            {
                byte_length -= 1.0;
            }
            if js::string::get(
                server_value,
                f64::from(js::string::length(server_value)) - 2.0,
            ) == Some("=")
            {
                byte_length -= 1.0;
            }
            if byte_length == size {
                return;
            }
        }
    }
    warn_for_prop_difference(
        "src",
        server_value.map(js::unknown_of),
        Some(value),
        server_differences,
    );
}

fn diff_hydrated_custom_component(
    dom_element: &'static Element,
    tag: &str,
    props: &'static Unknown,
    host_context: &'static Unknown,
    extra_attributes: &Set<str>,
    server_differences: &Dict<Option<&'static Unknown>>,
) {
    for prop_key in js::keys_in(props) {
        if !props.has_own_property(&prop_key) {
            continue;
        }
        let value = js::get(props, &prop_key);
        if value.is_none() {
            continue;
        }
        if REGISTRATION_NAME_DEPENDENCIES.with(|registration_name_dependencies| {
            js::unknown(*registration_name_dependencies).has_own_property(&prop_key)
        }) {
            if !matches!(js::classify(value), Kind::Function(_)) {
                warn_for_invalid_event_listener(&prop_key, value);
            }
            continue;
        }
        if js::strict_equals(js::get(props, "suppressHydrationWarning"), true) {
            // Don't bother comparing. We're ignoring all these warnings.
            continue;
        }
        // Validate that the properties correspond to their expected values.
        match prop_key.as_str() {
            "children" => {
                if let Kind::String(_) | Kind::Number(_) = js::classify(value) {
                    warn_for_prop_difference(
                        "children",
                        dom_element.text_content().map(js::unknown_of),
                        value,
                        server_differences,
                    );
                }
            }
            // Checked above already
            "suppressContentEditableWarning"
            | "suppressHydrationWarning"
            | "defaultValue"
            | "defaultChecked"
            | "innerHTML"
            | "ref" => {
                // Noop
            }
            "dangerouslySetInnerHTML" => {
                #[cfg_attr(rust_js, rust_js::name = "serverHTML")]
                let server_html = Some(js::unknown_of(dom_element.inner_html()));
                let next_html = if js::truthy(value) {
                    js::get(value, "__html")
                } else {
                    None
                };
                if next_html.is_some() {
                    #[cfg_attr(rust_js, rust_js::name = "expectedHTML")]
                    let expected_html = normalize_html(dom_element, unsafe { js::cast(next_html) });
                    warn_for_prop_difference(
                        &prop_key,
                        server_html,
                        expected_html.map(js::unknown_of),
                        server_differences,
                    );
                }
            }
            "style" => {
                extra_attributes.delete(&prop_key);
                diff_hydrated_styles(dom_element, value, server_differences);
            }
            "offsetParent" | "offsetTop" | "offsetLeft" | "offsetWidth" | "offsetHeight"
            | "isContentEditable" | "outerText" | "outerHTML" => {
                extra_attributes.delete(&prop_key.to_lowercase());
                if __DEV__ {
                    console::error((
                        "Assignment to read-only property will result in a no-op: `%s`",
                        &prop_key,
                    ));
                }
            }
            // Fall through
            "className" => {
                // className is a special cased property on the server to render as an attribute.
                extra_attributes.delete("class");
                let server_value =
                    get_value_for_attribute_on_custom_component(dom_element, "class", value);
                warn_for_prop_difference("className", server_value, value, server_differences);
            }
            _ => {
                // This is a DEV-only path
                let host_context_prod = js::get(host_context, "context");
                if js::strict_equals(host_context_prod, HOST_CONTEXT_NAMESPACE_NONE)
                    && tag != "svg"
                    && tag != "math"
                {
                    extra_attributes.delete(&prop_key.to_lowercase());
                } else {
                    extra_attributes.delete(&prop_key);
                }
                let value_on_custom_component =
                    get_value_for_attribute_on_custom_component(dom_element, &prop_key, value);
                warn_for_prop_difference(
                    &prop_key,
                    value_on_custom_component,
                    value,
                    server_differences,
                );
            }
        }
    }
}

// This is the exact URL string we expect that Fizz renders if we provide a function action.
// We use this for hydration warnings. It needs to be in sync with Fizz. Maybe makes sense
// as a shared module for that reason.
const EXPECTED_FORM_ACTION_URL: &str =
    // eslint-disable-next-line no-script-url
    "javascript:throw new Error('React form unexpectedly submitted.')";

/// `{__html: ..}`, the server's HTML as a `dangerouslySetInnerHTML` prop shows it.
pub struct DangerouslySetInnerHTML {
    pub __html: Option<&'static Unknown>,
}

unsafe impl js::Defined for DangerouslySetInnerHTML {}

fn diff_hydrated_generic_element(
    dom_element: &'static Element,
    tag: &str,
    props: &'static Unknown,
    host_context: &'static Unknown,
    extra_attributes: &Set<str>,
    server_differences: &Dict<Option<&'static Unknown>>,
) {
    for prop_key in js::keys_in(props) {
        if !props.has_own_property(&prop_key) {
            continue;
        }
        let value = js::get(props, &prop_key);
        if value.is_none() {
            continue;
        }
        if REGISTRATION_NAME_DEPENDENCIES.with(|registration_name_dependencies| {
            js::unknown(*registration_name_dependencies).has_own_property(&prop_key)
        }) {
            if !matches!(js::classify(value), Kind::Function(_)) {
                warn_for_invalid_event_listener(&prop_key, value);
            }
            continue;
        }
        if js::strict_equals(js::get(props, "suppressHydrationWarning"), true) {
            // Don't bother comparing. We're ignoring all these warnings.
            continue;
        }
        // Validate that the properties correspond to their expected values.
        match prop_key.as_str() {
            "children" => {
                if let Kind::String(_) | Kind::Number(_) = js::classify(value) {
                    warn_for_prop_difference(
                        "children",
                        dom_element.text_content().map(js::unknown_of),
                        value,
                        server_differences,
                    );
                }
            }
            // Checked above already
            "suppressContentEditableWarning"
            | "suppressHydrationWarning"
            | "value" // Controlled attributes are not validated
            | "checked" // TODO: Only ignore them on controlled tags.
            | "selected"
            | "defaultValue"
            | "defaultChecked"
            | "innerHTML"
            | "ref" => {
                // Noop
            }
            "dangerouslySetInnerHTML" => {
                #[cfg_attr(rust_js, rust_js::name = "serverHTML")]
                let server_html = Some(js::unknown_of(dom_element.inner_html()));
                let next_html = if js::truthy(value) { js::get(value, "__html") } else { None };
                if next_html.is_some() {
                    #[cfg_attr(rust_js, rust_js::name = "expectedHTML")]
                    let expected_html = normalize_html(dom_element, unsafe { js::cast(next_html) });
                    if !js::strict_equals(server_html, expected_html) {
                        dict::set(
                            server_differences,
                            &prop_key,
                            Some(js::unknown_of(DangerouslySetInnerHTML { __html: server_html })),
                        );
                    }
                }
            }
            "className" => {
                hydrate_attribute(dom_element, &prop_key, "class", value, extra_attributes, server_differences);
            }
            "tabIndex" => {
                hydrate_attribute(dom_element, &prop_key, "tabindex", value, extra_attributes, server_differences);
            }
            "style" => {
                extra_attributes.delete(&prop_key);
                diff_hydrated_styles(dom_element, value, server_differences);
            }
            "multiple" => {
                extra_attributes.delete(&prop_key);
                let server_value = js::get(dom_element, "multiple");
                warn_for_prop_difference(&prop_key, server_value, value, server_differences);
            }
            "muted" => {
                extra_attributes.delete(&prop_key);
                let server_value = js::get(dom_element, "muted");
                warn_for_prop_difference(&prop_key, server_value, value, server_differences);
            }
            "autoFocus" => {
                extra_attributes.delete("autofocus");
                let server_value = js::get(dom_element, "autofocus");
                warn_for_prop_difference(&prop_key, server_value, value, server_differences);
            }
            "data" if tag != "object" => {
                extra_attributes.delete(&prop_key);
                let server_value = dom_element.get_attribute("data");
                warn_for_prop_difference(&prop_key, server_value.map(js::unknown_of), value, server_differences);
            }
            "data" | "src" | "href" => {
                if prop_key != "href"
                    && ENABLE_SRC_OBJECT.with(|&enabled| enabled)
                    && let Some(object) = js::classify(value).object()
                {
                    // Some tags support object sources like Blob, File, MediaSource and MediaStream.
                    if tag == "img" || tag == "video" || tag == "audio" {
                        if let Ok(()) = panic::catch_unwind(|| {
                            // Test if this is a compatible object
                            url::revoke_object_url(&url::create_object_url(unsafe {
                                js::cast::<&webapi::Blob>(Some(js::unknown(object)))
                            }));
                            hydrate_src_object_attribute(
                                dom_element,
                                js::unknown(object),
                                extra_attributes,
                                server_differences,
                            );
                        }) {
                            continue;
                        }
                        // If not, just fall through to the normal toString flow.
                    } else if __DEV__ {
                        let _ = panic::catch_unwind(|| {
                            // This should always error.
                            url::revoke_object_url(&url::create_object_url(unsafe {
                                js::cast::<&webapi::Blob>(Some(js::unknown(object)))
                            }));
                            if tag == "source" {
                                console::error((concat!(
                                    "Passing Blob, MediaSource or MediaStream to <source src> is not supported. ",
                                    "Pass it directly to <img src>, <video src> or <audio src> instead."
                                ),));
                            } else {
                                console::error((
                                    "Passing Blob, MediaSource or MediaStream to <%s src> is not supported.",
                                    tag,
                                ));
                            }
                        });
                    }
                }
                if js::strict_equals(value, "")
                    // <a href=""> is fine for "reload" links.
                    && !(tag == "a" && prop_key == "href")
                    && !(tag == "object" && prop_key == "data")
                {
                    if __DEV__ {
                        if prop_key == "src" {
                            console::error((
                                concat!(
                                    "An empty string (\"\") was passed to the %s attribute. ",
                                    "This may cause the browser to download the whole page again over the network. ",
                                    "To fix this, either do not render the element at all ",
                                    "or pass null to %s instead of an empty string."
                                ),
                                &prop_key,
                                &prop_key,
                            ));
                        } else {
                            console::error((
                                concat!(
                                    "An empty string (\"\") was passed to the %s attribute. ",
                                    "To fix this, either do not render the element at all ",
                                    "or pass null to %s instead of an empty string."
                                ),
                                &prop_key,
                                &prop_key,
                            ));
                        }
                    }
                    continue;
                }
                hydrate_sanitized_attribute(
                    dom_element,
                    &prop_key,
                    &prop_key,
                    value,
                    extra_attributes,
                    server_differences,
                );
            }
            "action" | "formAction" => {
                let server_value = dom_element.get_attribute(&prop_key);
                if let Kind::Function(_) = js::classify(value) {
                    extra_attributes.delete(&prop_key.to_lowercase());
                    // The server can set these extra properties to implement actions.
                    // So we remove them from the extra attributes warnings.
                    if prop_key == "formAction" {
                        extra_attributes.delete("name");
                        extra_attributes.delete("formenctype");
                        extra_attributes.delete("formmethod");
                        extra_attributes.delete("formtarget");
                    } else {
                        extra_attributes.delete("enctype");
                        extra_attributes.delete("method");
                        extra_attributes.delete("target");
                    }
                    // Ideally we should be able to warn if the server value was not a function
                    // however since the function can return any of these attributes any way it
                    // wants as a custom progressive enhancement, there's nothing to compare to.
                    // We can check if the function has the $FORM_ACTION property on the client
                    // and if it's not, warn, but that's an unnecessary constraint that they
                    // have to have the extra extension that doesn't do anything on the client.
                    continue;
                } else if js::strict_equals(&server_value, EXPECTED_FORM_ACTION_URL) {
                    extra_attributes.delete(&prop_key.to_lowercase());
                    warn_for_prop_difference(
                        &prop_key,
                        Some(js::unknown("function")),
                        value,
                        server_differences,
                    );
                    continue;
                }
                hydrate_sanitized_attribute(
                    dom_element,
                    &prop_key,
                    &prop_key.to_lowercase(),
                    value,
                    extra_attributes,
                    server_differences,
                );
            }
            "xlinkHref" => {
                hydrate_sanitized_attribute(
                    dom_element,
                    &prop_key,
                    "xlink:href",
                    value,
                    extra_attributes,
                    server_differences,
                );
            }
            "contentEditable" => {
                // Lower-case Booleanish String
                hydrate_booleanish_attribute(
                    dom_element,
                    &prop_key,
                    "contenteditable",
                    value,
                    extra_attributes,
                    server_differences,
                );
            }
            "spellCheck" => {
                // Lower-case Booleanish String
                hydrate_booleanish_attribute(
                    dom_element,
                    &prop_key,
                    "spellcheck",
                    value,
                    extra_attributes,
                    server_differences,
                );
            }
            "draggable" | "autoReverse" | "externalResourcesRequired" | "focusable" | "preserveAlpha" => {
                // Case-sensitive Booleanish String
                hydrate_booleanish_attribute(
                    dom_element,
                    &prop_key,
                    &prop_key,
                    value,
                    extra_attributes,
                    server_differences,
                );
            }
            "allowFullScreen" | "async" | "autoPlay" | "controls" | "credentialless" | "default"
            | "defer" | "disabled" | "disablePictureInPicture" | "disableRemotePlayback"
            | "formNoValidate" | "hidden" | "loop" | "noModule" | "noValidate" | "open"
            | "playsInline" | "readOnly" | "required" | "reversed" | "scoped" | "seamless"
            | "itemScope" => {
                // Some of these need to be lower case to remove them from the extraAttributes list.
                hydrate_boolean_attribute(
                    dom_element,
                    &prop_key,
                    &prop_key.to_lowercase(),
                    value,
                    extra_attributes,
                    server_differences,
                );
            }
            "capture" | "download" => {
                hydrate_overloaded_boolean_attribute(
                    dom_element,
                    &prop_key,
                    &prop_key,
                    value,
                    extra_attributes,
                    server_differences,
                );
            }
            "cols" | "rows" | "size" | "span" => {
                hydrate_positive_numeric_attribute(
                    dom_element,
                    &prop_key,
                    &prop_key,
                    value,
                    extra_attributes,
                    server_differences,
                );
            }
            "rowSpan" => {
                hydrate_numeric_attribute(
                    dom_element,
                    &prop_key,
                    "rowspan",
                    value,
                    extra_attributes,
                    server_differences,
                );
            }
            "start" => {
                hydrate_numeric_attribute(
                    dom_element,
                    &prop_key,
                    &prop_key,
                    value,
                    extra_attributes,
                    server_differences,
                );
            }
            "xHeight" => {
                hydrate_attribute(dom_element, &prop_key, "x-height", value, extra_attributes, server_differences);
            }
            "xlinkActuate" => {
                hydrate_attribute(dom_element, &prop_key, "xlink:actuate", value, extra_attributes, server_differences);
            }
            "xlinkArcrole" => {
                hydrate_attribute(dom_element, &prop_key, "xlink:arcrole", value, extra_attributes, server_differences);
            }
            "xlinkRole" => {
                hydrate_attribute(dom_element, &prop_key, "xlink:role", value, extra_attributes, server_differences);
            }
            "xlinkShow" => {
                hydrate_attribute(dom_element, &prop_key, "xlink:show", value, extra_attributes, server_differences);
            }
            "xlinkTitle" => {
                hydrate_attribute(dom_element, &prop_key, "xlink:title", value, extra_attributes, server_differences);
            }
            "xlinkType" => {
                hydrate_attribute(dom_element, &prop_key, "xlink:type", value, extra_attributes, server_differences);
            }
            "xmlBase" => {
                hydrate_attribute(dom_element, &prop_key, "xml:base", value, extra_attributes, server_differences);
            }
            "xmlLang" => {
                hydrate_attribute(dom_element, &prop_key, "xml:lang", value, extra_attributes, server_differences);
            }
            "xmlSpace" => {
                hydrate_attribute(dom_element, &prop_key, "xml:space", value, extra_attributes, server_differences);
            }
            "inert" => {
                if __DEV__ {
                    DID_WARN_FOR_NEW_BOOLEAN_PROPS_WITH_EMPTY_VALUE.with(
                        |did_warn_for_new_boolean_props_with_empty_value| {
                            if js::strict_equals(value, "")
                                && !dict::get(did_warn_for_new_boolean_props_with_empty_value, &prop_key)
                                    .is_some_and(|&did_warn| did_warn)
                            {
                                dict::set(did_warn_for_new_boolean_props_with_empty_value, &prop_key, true);
                                console::error((
                                    concat!(
                                        "Received an empty string for a boolean attribute `%s`. ",
                                        "This will treat the attribute as if it were false. ",
                                        "Either pass `false` to silence this warning, or ",
                                        "pass `true` if you used an empty string in earlier versions of React to indicate this attribute is true."
                                    ),
                                    &prop_key,
                                ));
                            }
                        },
                    );
                }
                hydrate_boolean_attribute(
                    dom_element,
                    &prop_key,
                    &prop_key,
                    value,
                    extra_attributes,
                    server_differences,
                );
            }
            _ => {
                if
                // shouldIgnoreAttribute
                // We have already filtered out null/undefined and reserved words.
                js::string::length(&prop_key) > 2
                    && (js::string::get(&prop_key, 0.0) == Some("o")
                        || js::string::get(&prop_key, 0.0) == Some("O"))
                    && (js::string::get(&prop_key, 1.0) == Some("n")
                        || js::string::get(&prop_key, 1.0) == Some("N"))
                {
                    continue;
                }
                let attribute_name = get_attribute_alias(&prop_key);
                let mut is_mismatch_due_to_bad_casing = false;

                // This is a DEV-only path
                let host_context_prod = js::get(host_context, "context");

                if js::strict_equals(host_context_prod, HOST_CONTEXT_NAMESPACE_NONE)
                    && tag != "svg"
                    && tag != "math"
                {
                    extra_attributes.delete(&attribute_name.to_lowercase());
                } else {
                    let standard_name = get_possible_standard_name(&prop_key);
                    if let Some(standard_name) = standard_name
                        && standard_name != prop_key
                    {
                        // If an SVG prop is supplied with bad casing, it will
                        // be successfully parsed from HTML, but will produce a mismatch
                        // (and would be incorrectly rendered on the client).
                        // However, we already warn about bad casing elsewhere.
                        // So we'll skip the misleading extra mismatch warning in this case.
                        is_mismatch_due_to_bad_casing = true;
                        extra_attributes.delete(standard_name);
                    }
                    extra_attributes.delete(attribute_name);
                }
                let server_value = get_value_for_attribute(dom_element, attribute_name, value);
                if !is_mismatch_due_to_bad_casing {
                    warn_for_prop_difference(&prop_key, server_value, value, server_differences);
                }
            }
        }
    }
}

pub fn hydrate_properties(
    dom_element: &'static Element,
    tag: &str,
    props: &'static Unknown,
    _host_context: &'static Unknown,
) -> bool {
    if __DEV__ {
        validate_properties_in_development(tag, props);
    }

    // TODO: Make sure that we check isMounted before firing any of these events.
    match tag {
        "dialog" => {
            listen_to_non_delegated_event("cancel", dom_element);
            listen_to_non_delegated_event("close", dom_element);
        }
        "iframe" | "object" | "embed" => {
            // We listen to this event in case to ensure emulated bubble
            // listeners still fire for the load event.
            listen_to_non_delegated_event("load", dom_element);
        }
        "video" | "audio" => {
            // We listen to these events in case to ensure emulated bubble
            // listeners still fire for all the media events.
            MEDIA_EVENT_TYPES.with(|media_event_types| {
                for media_event_type in media_event_types {
                    listen_to_non_delegated_event(media_event_type, dom_element);
                }
            });
        }
        "source" => {
            // We listen to this event in case to ensure emulated bubble
            // listeners still fire for the error event.
            listen_to_non_delegated_event("error", dom_element);
        }
        "img" | "image" | "link" => {
            // We listen to these events in case to ensure emulated bubble
            // listeners still fire for error and load events.
            listen_to_non_delegated_event("error", dom_element);
            listen_to_non_delegated_event("load", dom_element);
        }
        "details" => {
            // We listen to this event in case to ensure emulated bubble
            // listeners still fire for the toggle event.
            listen_to_non_delegated_event("toggle", dom_element);
        }
        "input" => {
            if __DEV__ {
                check_controlled_value_props("input", props);
            }
            // We listen to this event in case to ensure emulated bubble
            // listeners still fire for the invalid event.
            listen_to_non_delegated_event("invalid", dom_element);
            // TODO: Make sure we check if this is still unmounted or do any clean
            // up necessary since we never stop tracking anymore.
            validate_input_props(dom_element, props);
            // For input and textarea we current always set the value property at
            // post mount to force it to diverge from attributes. However, for
            // option and select we don't quite do the same thing and select
            // is not resilient to the DOM state changing so we don't do that here.
            // TODO: Consider not doing this for input and textarea.
            if !ENABLE_HYDRATION_CHANGE_EVENT.with(|&enabled| enabled) {
                init_input(
                    dom_element,
                    js::get(props, "value"),
                    js::get(props, "defaultValue"),
                    js::get(props, "checked"),
                    js::get(props, "defaultChecked"),
                    js::get(props, "type"),
                    js::get(props, "name"),
                    true,
                );
            }
        }
        "option" => validate_option_props(dom_element, props),
        "select" => {
            if __DEV__ {
                check_controlled_value_props("select", props);
            }
            // We listen to this event in case to ensure emulated bubble
            // listeners still fire for the invalid event.
            listen_to_non_delegated_event("invalid", dom_element);
            validate_select_props(dom_element, props);
        }
        "textarea" => {
            if __DEV__ {
                check_controlled_value_props("textarea", props);
            }
            // We listen to this event in case to ensure emulated bubble
            // listeners still fire for the invalid event.
            listen_to_non_delegated_event("invalid", dom_element);
            // TODO: Make sure we check if this is still unmounted or do any clean
            // up necessary since we never stop tracking anymore.
            validate_textarea_props(dom_element, props);
            if !ENABLE_HYDRATION_CHANGE_EVENT.with(|&enabled| enabled) {
                init_textarea(
                    dom_element,
                    js::get(props, "value"),
                    js::get(props, "defaultValue"),
                    js::get(props, "children"),
                );
            }
        }
        _ => {}
    }

    let children = js::get(props, "children");
    // For text content children we compare against textContent. This
    // might match additional HTML that is hidden when we read it using
    // textContent. E.g. "foo" will match "f<span>oo</span>" but that still
    // satisfies our requirement. Our requirement is not to produce perfect
    // HTML and attributes. Ideally we should preserve structure but it's
    // ok not to if the visible content is still enough to indicate what
    // even listeners these nodes might be wired up to.
    // TODO: Warn if there is more than a single textNode as a child.
    // TODO: Should we use domElement.firstChild.nodeValue to compare?
    if let Kind::String(_) | Kind::Number(_) | Kind::BigInt(_) = js::classify(children)
        && !js::strict_equals(
            dom_element.text_content(),
            js::concat_text(children).unwrap(),
        )
        && !js::strict_equals(js::get(props, "suppressHydrationWarning"), true)
        && !check_for_unmatched_text(dom_element.text_content(), children)
    {
        return false;
    }

    if js::get(props, "popover").is_some() {
        // We listen to this event in case to ensure emulated bubble
        // listeners still fire for the toggle event.
        listen_to_non_delegated_event("beforetoggle", dom_element);
        listen_to_non_delegated_event("toggle", dom_element);
    }

    if js::get(props, "onScroll").is_some() {
        listen_to_non_delegated_event("scroll", dom_element);
    }

    if js::get(props, "onScrollEnd").is_some() {
        listen_to_non_delegated_event("scrollend", dom_element);
        if ENABLE_SCROLL_END_POLYFILL.with(|&enabled| enabled) {
            // For use by the polyfill.
            listen_to_non_delegated_event("scroll", dom_element);
        }
    }

    if js::get(props, "onClick").is_some() {
        // TODO: This cast may not be sound for SVG, MathML or custom elements.
        trap_click_on_non_interactive_element(html_element::unchecked_from(dom_element));
    }

    true
}

pub fn diff_hydrated_properties(
    dom_element: &'static Element,
    tag: &str,
    props: &'static Unknown,
    host_context: &'static Unknown,
) -> Option<&'static Dict<Option<&'static Unknown>>> {
    let server_differences: &'static Dict<Option<&'static Unknown>> = dict::new();
    if __DEV__ {
        let extra_attributes: &'static Set<str> = js::set::new(None);
        let attributes = dom_element.attributes();
        for i in 0..attributes.length() as usize {
            let name = attributes[i].name().to_lowercase();
            match name.as_str() {
                // Controlled attributes are not validated
                // TODO: Only ignore them on controlled tags.
                "value" => {}
                "checked" => {}
                "selected" => {}
                "vt-name" | "vt-update" | "vt-enter" | "vt-exit" | "vt-share"
                | "vt-parent-enter" | "vt-parent-exit"
                    if ENABLE_VIEW_TRANSITION =>
                {
                    // View Transition annotations are expected from the Server Runtime.
                    // However, if they're also specified on the client and don't match
                    // that's an error.
                }
                // Fallthrough
                _ => {
                    // Intentionally use the original name.
                    // See discussion in https://github.com/react/react/pull/10676.
                    extra_attributes.add(&attributes[i].name());
                }
            }
        }
        if is_custom_element(tag, props) {
            diff_hydrated_custom_component(
                dom_element,
                tag,
                props,
                host_context,
                extra_attributes,
                server_differences,
            );
        } else {
            diff_hydrated_generic_element(
                dom_element,
                tag,
                props,
                host_context,
                extra_attributes,
                server_differences,
            );
        }
        if extra_attributes.size() > 0
            && !js::strict_equals(js::get(props, "suppressHydrationWarning"), true)
        {
            warn_for_extra_attributes(dom_element, extra_attributes, server_differences);
        }
    }
    if js::object::keys(js::unknown(server_differences)).is_empty() {
        return js::null();
    }
    Some(server_differences)
}

pub fn hydrate_text(text_node: &Text, text: &str, parent_props: Option<&'static Unknown>) -> bool {
    let is_different = !js::strict_equals(text_node.node_value(), text);
    if is_different
        && (matches!(js::classify(parent_props), Kind::Null)
            || !js::strict_equals(js::get(parent_props, "suppressHydrationWarning"), true))
        && !check_for_unmatched_text(text_node.node_value(), Some(js::unknown(text)))
    {
        return false;
    }
    true
}

pub fn diff_hydrated_text(text_node: &Text, text: &str) -> Option<String> {
    if js::strict_equals(text_node.node_value(), text) {
        return js::null();
    }
    let normalized_client_text = normalize_markup_for_text_or_attribute(Some(js::unknown(text)));
    let normalized_server_text =
        normalize_markup_for_text_or_attribute(text_node.node_value().map(js::unknown_of));
    if normalized_server_text == normalized_client_text {
        return js::null();
    }
    text_node.node_value()
}

pub fn restore_controlled_state(dom_element: &Element, tag: &str, props: &Unknown) {
    match tag {
        "input" => restore_controlled_input_state(dom_element, props),
        "textarea" => restore_controlled_textarea_state(dom_element, props),
        "select" => restore_controlled_select_state(dom_element, props),
        _ => {}
    }
}
