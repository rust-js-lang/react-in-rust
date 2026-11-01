// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::panic::{self, AssertUnwindSafe};
use std::rc::Rc;

use js::{Dict, JsObject, Kind, Unknown, dict};
use shared::environment::__DEV__;
use shared::react_dom_shared_internals::{HostDispatcher, REACT_DOM_SHARED_INTERNALS};
use shared::react_feature_flags::{
    ENABLE_FIZZ_BLOCKING_RENDER, ENABLE_FIZZ_EXTERNAL_RUNTIME, ENABLE_SRC_OBJECT,
    ENABLE_VIEW_TRANSITION, ENABLE_VIEW_TRANSITION_PARENT_ENTER_EXIT,
};
use shared::react_types::{ReactCustomFormAction, Thenable};
use webapi::{Blob, FormData, FormDataEntryValue};

use react_isomorphic::CHILDREN;
use shared::check_string_coercion::{
    check_attribute_string_coercion, check_css_property_string_coercion,
    check_html_string_coercion, check_option_string_coercion,
};
use shared::has_own_property::HAS_OWN_PROPERTY;
use shared::is_array::is_array;

use crate::cross_origin_strings::{CrossOriginString, get_cross_origin_string};
use crate::escape_text_for_browser::escape_text_for_browser;
use crate::get_attribute_alias::get_attribute_alias;
use crate::hyphenate_style_name::hyphenate_style_name;
use crate::is_attribute_name_safe::is_attribute_name_safe;
use crate::is_unitless_number::is_unitless_number;
use crate::react_controlled_value_prop_types::check_controlled_value_props;
use crate::react_dom_fizz_instruction_set_inline_code_strings::{
    CLIENT_RENDER_BOUNDARY as CLIENT_RENDER_FUNCTION,
    COMPLETE_BOUNDARY as COMPLETE_BOUNDARY_FUNCTION,
    COMPLETE_BOUNDARY_UPGRADE_TO_VIEW_TRANSITIONS as UPGRADE_TO_VIEW_TRANSITIONS_INSTRUCTION,
    COMPLETE_BOUNDARY_WITH_STYLES as STYLE_INSERTION_FUNCTION,
    COMPLETE_SEGMENT as COMPLETE_SEGMENT_FUNCTION, FORM_REPLAYING as FORM_REPLAYING_RUNTIME,
    MARK_SHELL_TIME,
};
use crate::react_dom_form_actions::{FormStatus, NOT_PENDING};
#[cfg_attr(rust_js, rust_js::name = "validateARIAProperties")]
use crate::react_dom_invalid_aria_hook::validate_properties as validate_aria_properties;
use crate::react_dom_null_input_value_prop_hook::validate_properties as validate_input_properties;
use crate::react_dom_resource_validation::get_value_descriptor_expecting_object_for_warning;
use crate::react_dom_unknown_property_hook::validate_properties as validate_unknown_properties;
use crate::sanitize_url::sanitize_url;
use crate::warn_valid_style::warn_valid_style;

/// Where a render's output is written: the host's stream, as its stream
/// config names it.
pub struct Destination(PhantomData<JsObject>);

unsafe impl js::Defined for Destination {}

/// What the host's stream config makes of text: a string, or its bytes.
pub struct Chunk(PhantomData<JsObject>);

unsafe impl js::Defined for Chunk {}

/// A chunk the stream config made once, written again and again.
pub type PrecomputedChunk = Chunk;

/// What a render keeps, React's `Request`, which Fizz's server owns.
pub struct Request(PhantomData<JsObject>);

unsafe impl js::Defined for Request {}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-server/src/ReactServerStreamConfig#writeChunk"
)]
// The destination's write runs none of this crate's code, the user's decision (ADR 0262).
#[cfg_attr(rust_js, rust_js::runs_nothing)]
pub fn write_chunk(destination: &Destination, chunk: &Chunk) {
    let _ = (destination, chunk);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-server/src/ReactServerStreamConfig#writeChunkAndReturn"
)]
// The destination's write runs none of this crate's code, the user's decision (ADR 0262).
#[cfg_attr(rust_js, rust_js::runs_nothing)]
pub fn write_chunk_and_return(destination: &Destination, chunk: &Chunk) -> bool {
    let _ = (destination, chunk);
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-server/src/ReactServerStreamConfig#stringToChunk"
)]
// It encodes the text it's given and runs nothing else.
#[cfg_attr(rust_js, rust_js::runs_only_given)]
pub fn string_to_chunk(content: &str) -> &'static Chunk {
    let _ = content;
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-server/src/ReactServerStreamConfig#stringToPrecomputedChunk"
)]
// It encodes the text it's given and runs nothing else.
#[cfg_attr(rust_js, rust_js::runs_only_given)]
pub fn string_to_precomputed_chunk(content: &str) -> &'static PrecomputedChunk {
    let _ = content;
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-server/src/ReactServerStreamConfig#readAsDataURL"
)]
fn read_as_data_url(blob: &Blob) -> &'static Thenable {
    let _ = blob;
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-server/src/ReactFizzServer#resolveRequest"
)]
fn resolve_request() -> Option<&'static Request> {
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-server/src/ReactFizzServer#getResumableState"
)]
fn get_resumable_state(request: &Request) -> &'static ResumableState {
    let _ = request;
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-server/src/ReactFizzServer#getRenderState"
)]
fn get_render_state(request: &Request) -> &'static RenderState {
    let _ = request;
    unreachable!()
}

#[cfg_attr(
    rust_js,
    rust_js::link_name = "react-server/src/ReactFizzServer#flushResources"
)]
fn flush_resources(request: &Request) {
    let _ = request;
    unreachable!()
}

unsafe extern "Rust" {
    /// The renderer's doctype, which a renderer's config may override.
    #[link_name = "react-server/src/ReactFizzConfig#doctypeChunk"]
    safe static DOCTYPE: &'static PrecomputedChunk;
}

/// We make every property of the descriptor optional because it is not a contract that
/// the headers provided by onHeaders has any particular header types.
pub struct HeadersDescriptor {
    #[cfg_attr(rust_js, rust_js::name = "Link")]
    pub link: Option<String>,
}

// Used to distinguish these contexts from ones used in other renderers.
// E.g. this can be used to distinguish legacy renderers from this modern one.
#[cfg_attr(rust_js, rust_js::name = "isPrimaryRenderer")]
pub const IS_PRIMARY_RENDERER: bool = true;

#[cfg_attr(rust_js, rust_js::name = "supportsClientAPIs")]
pub const SUPPORTS_CLIENT_APIS: bool = true;

pub type StreamingFormat = i32;
const SCRIPT_STREAMING_FORMAT: StreamingFormat = 0;
const DATA_STREAMING_FORMAT: StreamingFormat = 1;

pub type InstructionState = i32;
const NOTHING_SENT: InstructionState = /*                      */ 0b000000000;
const SENT_COMPLETE_SEGMENT_FUNCTION: InstructionState = /*      */ 0b000000001;
const SENT_COMPLETE_BOUNDARY_FUNCTION: InstructionState = /*     */ 0b000000010;
const SENT_CLIENT_RENDER_FUNCTION: InstructionState = /*         */ 0b000000100;
const SENT_STYLE_INSERTION_FUNCTION: InstructionState = /*       */ 0b000001000;
const SENT_FORM_REPLAYING_RUNTIME: InstructionState = /*         */ 0b000010000;
const SENT_COMPLETED_SHELL_ID: InstructionState = /*             */ 0b000100000;
const SENT_MARK_SHELL_TIME: InstructionState = /*                */ 0b001000000;
const NEED_UPGRADE_TO_VIEW_TRANSITIONS: InstructionState = /*    */ 0b010000000;
const SENT_UPGRADE_TO_VIEW_TRANSITIONS: InstructionState = /*    */ 0b100000000;

/// A nonce given as one string, or one for each of scripts and styles.
pub struct NonceOption {
    pub script: Option<String>,
    pub style: Option<String>,
}

/// The chunks a part of the response is made of, in order.
pub type Chunks = RefCell<Vec<&'static Chunk>>;

/// The headers captured for Resources that can flush early.
pub struct Headers {
    pub preconnects: RefCell<String>,
    pub font_preloads: RefCell<String>,
    pub high_image_preloads: RefCell<String>,
    pub remaining_capacity: Cell<f64>,
}

/// What each kind of resource that's been asked for holds by its key:
/// `null` of one that exists, `[]` of one preloaded, and of one preloaded
/// with credentials, its `[crossOrigin, integrity]`. A program tells
/// `null` from a key it lacks, so its values are any value.
pub type ResourceStates = Dict<Option<&'static Unknown>>;

/// Of each kind of connection, the ones that exist.
pub struct ConnectResources {
    pub default: &'static ResourceStates,
    pub anonymous: &'static ResourceStates,
    pub credentials: &'static ResourceStates,
}

/// What a render resets its resumable state to, if nothing flushed.
pub struct Resets {
    // corresponds to ResumableState.unknownResources["font"]
    pub font: &'static ResourceStates,
    // the rest correspond to ResumableState[<...>Resources]
    pub dns: &'static ResourceStates,
    pub connect: Rc<ConnectResources>,
    pub image: &'static ResourceStates,
    pub style: &'static ResourceStates,
}

/// Temporarily keeps track of key to preload resources before shell flushes.
pub struct Preloads {
    pub images: &'static js::Map<str, Rc<Resource>>,
    pub stylesheets: &'static js::Map<str, Rc<Resource>>,
    pub scripts: &'static js::Map<str, Rc<Resource>>,
    pub module_scripts: &'static js::Map<str, Rc<Resource>>,
}

/// Per request, global state that is not contextual to the rendering subtree.
/// This cannot be resumed and therefore should only contain things that are
/// temporary working state or are never used in the prerender pass.
pub struct RenderState {
    // These can be recreated from resumable state.
    pub placeholder_prefix: &'static PrecomputedChunk,
    pub segment_prefix: &'static PrecomputedChunk,
    pub boundary_prefix: &'static PrecomputedChunk,

    // inline script streaming format, unused if using external runtime / data
    pub start_inline_script: &'static PrecomputedChunk,

    pub start_inline_style: &'static PrecomputedChunk,

    // the preamble must always flush before resuming, so all these chunks must
    // be null or empty when resuming.

    // preamble chunks
    pub preamble: PreambleState,

    // external runtime script chunks
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub external_runtime_script: Option<ExternalRuntimeScript>,
    pub bootstrap_chunks: Chunks,
    pub import_map_chunks: Chunks,

    // Headers queues for Resources that can flush early
    pub on_headers: Option<&'static dyn Fn(&HeadersDescriptor)>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub headers: RefCell<Option<Rc<Headers>>>,
    pub resets: Resets,

    // Hoistable chunks
    pub charset_chunks: Chunks,
    pub viewport_chunks: Chunks,
    pub hoistable_chunks: Chunks,

    // Flushing queues for Resource dependencies
    pub preconnects: &'static js::Set<Rc<Resource>>,
    pub font_preloads: &'static js::Set<Rc<Resource>>,
    pub high_image_preloads: &'static js::Set<Rc<Resource>>,
    // usedImagePreloads: Set<PreloadResource>,
    pub styles: &'static js::Map<str, Rc<StyleQueue>>,
    pub bootstrap_scripts: &'static js::Set<Rc<Resource>>,
    pub scripts: &'static js::Set<Rc<Resource>>,
    pub bulk_preloads: &'static js::Set<Rc<Resource>>,

    // Temporarily keeps track of key to preload resources before shell flushes.
    pub preloads: Preloads,

    pub nonce: NonceOption,

    // like a module global for currently rendering boundary
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub hoistable_state: Cell<Option<&'static HoistableState>>,
    // Module-global-like reference for flushing/hoisting state of style resources
    // We need to track whether the current request has flushed any style resources
    // without sending an instruction to hoist them. we do that here
    pub styles_to_hoist: Cell<bool>,
}

unsafe impl js::Defined for RenderState {}

// Credentials here are things that affect whether a browser will make a request
// as well as things that affect which connection the browser will use for that request.
// We want these to be aligned across preloads and resources because otherwise the preload
// will be wasted.
// We investigated whether referrerPolicy should be included here but from experimentation
// it seems that browsers do not treat this as part of the http cache key and does not affect
// which connection is used.
//
// A preload with credentials is `[crossOrigin, integrity]`, each maybe `null`.

thread_local! {
    static EXISTS: Option<&'static Unknown> = const { js::null() };
    // This constant is to mark preloads that have no unique credentials
    // to convey. It should never be checked by identity and we should not
    // assume Preload values in ResumableState equal this value because they
    // will have come from some parsed input.
    static PRELOAD_NO_CREDS: &'static Unknown = js::unknown_of(Vec::<String>::new());
}

js::on_load! {
    if __DEV__ {
        js::object::freeze(PRELOAD_NO_CREDS.with(|preload| *preload));
    }
}

/// Per response, global state that is not contextual to the rendering subtree.
/// This is resumable and therefore should be serializable.
pub struct ResumableState {
    pub id_prefix: String,
    #[cfg_attr(rust_js, rust_js::name = "nextFormID")]
    pub next_form_id: Cell<f64>,
    pub streaming_format: StreamingFormat,

    // We carry the bootstrap intializers in resumable state in case we postpone in the shell
    // of a prerender. On resume we will reinitialize the bootstrap scripts if necessary.
    // If we end up flushing the bootstrap scripts we void these on the resumable state
    pub bootstrap_script_content: Cell<Option<&'static str>>,
    pub bootstrap_scripts: Cell<Option<&'static [BootstrapScript<'static>]>>,
    pub bootstrap_modules: Cell<Option<&'static [BootstrapScript<'static>]>>,

    // state for script streaming format, unused if using external runtime / data
    pub instructions: Cell<InstructionState>,

    // postamble state
    pub has_body: Cell<bool>,
    pub has_html: Cell<bool>,

    // Resources - Request local cache
    pub unknown_resources: Cell<&'static Dict<&'static ResourceStates>>,
    pub dns_resources: Cell<&'static ResourceStates>,
    pub connect_resources: RefCell<Rc<ConnectResources>>,
    pub image_resources: Cell<&'static ResourceStates>,
    pub style_resources: Cell<&'static ResourceStates>,
    pub script_resources: Cell<&'static ResourceStates>,
    pub module_unknown_resources: Cell<&'static Dict<&'static ResourceStates>>,
    pub module_script_resources: Cell<&'static ResourceStates>,
}

unsafe impl js::Defined for ResumableState {}

thread_local! {
    #[cfg_attr(rust_js, rust_js::nullable)]
    static CURRENTLY_FLUSHING_RENDER_STATE: Cell<Option<&'static RenderState>> = const { Cell::new(None) };

    static DATA_ELEMENT_QUOTED_END: &'static PrecomputedChunk = string_to_precomputed_chunk("\"></template>");

    static START_INLINE_SCRIPT: &'static PrecomputedChunk = string_to_precomputed_chunk("<script");
    static END_INLINE_SCRIPT: &'static PrecomputedChunk = string_to_precomputed_chunk("</script>");

    static START_SCRIPT_SRC: &'static PrecomputedChunk = string_to_precomputed_chunk("<script src=\"");
    static START_MODULE_SRC: &'static PrecomputedChunk = string_to_precomputed_chunk("<script type=\"module\" src=\"");
    static SCRIPT_NONCE: &'static PrecomputedChunk = string_to_precomputed_chunk(" nonce=\"");
    static SCRIPT_INTEGIRTY: &'static PrecomputedChunk = string_to_precomputed_chunk(" integrity=\"");
    static SCRIPT_CROSS_ORIGIN: &'static PrecomputedChunk = string_to_precomputed_chunk(" crossorigin=\"");
    static END_ASYNC_SCRIPT: &'static PrecomputedChunk = string_to_precomputed_chunk(" async=\"\"></script>");

    static START_INLINE_STYLE: &'static PrecomputedChunk = string_to_precomputed_chunk("<style");
}

/// A chunk a thread-local holds, as JS reads its module's `const`.
fn chunk(key: &'static std::thread::LocalKey<&'static Chunk>) -> &'static Chunk {
    key.with(|chunk| *chunk)
}

/// This escaping function is designed to work with with inline scripts where the entire
/// contents are escaped. Because we know we are escaping the entire script we can avoid for instance
/// escaping html comment string sequences that are valid javascript as well because
/// if there are no sebsequent <script sequences the html parser will never enter
/// script data double escaped state (see: https://www.w3.org/TR/html53/syntax.html#script-data-double-escaped-state)
///
/// While untrusted script content should be made safe before using this api it will
/// ensure that the script cannot be early terminated or never terminated state
fn escape_entire_inline_script_content(script_text: &str) -> String {
    if __DEV__ {
        check_html_string_coercion(Some(js::unknown(script_text)));
    }
    SCRIPT_REGEX
        .with(|script_regex| js::reg_exp::replace(script_text, script_regex, script_replacer))
}

thread_local! {
    static SCRIPT_REGEX: &'static js::RegExp = js::reg_exp::new("(<\\/|<)(s)(cript)", "gi");
}

fn script_replacer(
    _match: &str,
    prefix: Option<&str>,
    s: Option<&str>,
    suffix: Option<&str>,
) -> String {
    format!(
        "{}{}{}",
        prefix.unwrap_or_default(),
        if s == Some("s") { "\\u0073" } else { "\\u0053" },
        suffix.unwrap_or_default()
    )
}

pub struct BootstrapScriptDescriptor {
    pub src: String,
    pub integrity: Option<&'static Unknown>,
    pub cross_origin: Option<&'static Unknown>,
}

/// A bootstrap script, its `src` alone or its descriptor.
#[derive(Clone, Copy)]
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum BootstrapScript<'a> {
    Src(&'a str),
    Descriptor(&'a BootstrapScriptDescriptor),
}

pub struct ExternalRuntimeScript {
    pub src: String,
    pub chunks: Rc<Chunks>,
}

/// A nonce, one for both, or one of each.
#[derive(Clone, Copy)]
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum Nonce<'a> {
    Text(&'a str),
    Each(&'a NonceOption),
}

thread_local! {
    static IMPORT_MAP_SCRIPT_START: &'static PrecomputedChunk = string_to_precomputed_chunk("<script type=\"importmap\">");
    static IMPORT_MAP_SCRIPT_END: &'static PrecomputedChunk = string_to_precomputed_chunk("</script>");
}

// Since we store headers as strings we deal with their length in utf16 code units
// rather than visual characters or the utf8 encoding that is used for most binary
// serialization. Some common HTTP servers only allow for headers to be 4kB in length.
// We choose a default length that is likely to be well under this already limited length however
// pathological cases may still cause the utf-8 encoding of the headers to approach this limit.
// It should also be noted that this maximum is a soft maximum. we have not reached the limit we will
// allow one more header to be captured which means in practice if the limit is approached it will be exceeded
const DEFAULT_HEADERS_CAPACITY_IN_UTF16_CODE_UNITS: f64 = 2000.0;

thread_local! {
    static DID_WARN_FOR_NEW_BOOLEAN_PROPS_WITH_EMPTY_VALUE: &'static Dict<bool> = dict::new();
}

pub struct PreambleState {
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub html_chunks: RefCell<Option<Rc<Chunks>>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub head_chunks: RefCell<Option<Rc<Chunks>>>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub body_chunks: RefCell<Option<Rc<Chunks>>>,
}

type StylesheetState = i32;
const PENDING: StylesheetState = 0;
const PRELOADED: StylesheetState = 1;
const PREAMBLE: StylesheetState = 2;
const LATE: StylesheetState = 3;

/// A resource's chunks, which a queue holds by identity.
pub type Resource = Chunks;

pub struct StylesheetResource {
    pub props: &'static Unknown,
    pub state: Cell<StylesheetState>,
}

pub struct HoistableState {
    pub styles: &'static js::Set<Rc<StyleQueue>>,
    pub stylesheets: &'static js::Set<Rc<StylesheetResource>>,
    pub suspensey_images: Cell<bool>,
}

pub struct StyleQueue {
    pub precedence: &'static Chunk,
    pub rules: Chunks,
    pub hrefs: Chunks,
    pub sheets: &'static js::Map<str, Rc<StylesheetResource>>,
}

/// The props of a bootstrap script's preload, as `pushLinkImpl` writes them,
/// in the order upstream gives them.
struct PreloadAsProps<'a> {
    rel: &'a str,
    r#as: &'a str,
    fetch_priority: &'a str,
    nonce: Option<Nonce<'a>>,
    href: &'a str,
    integrity: Option<&'a str>,
    cross_origin: Option<&'a str>,
}

unsafe impl js::Defined for PreloadAsProps<'_> {}

/// The props of a bootstrap module's preload.
struct PreloadModuleProps<'a> {
    rel: &'a str,
    fetch_priority: &'a str,
    nonce: Option<&'a str>,
    href: &'a str,
    integrity: Option<&'a str>,
    cross_origin: Option<&'a str>,
}

unsafe impl js::Defined for PreloadModuleProps<'_> {}

/// The props of the external runtime's script.
struct ExternalRuntimeProps<'a> {
    src: &'a str,
    r#async: bool,
    integrity: Option<&'a Unknown>,
    nonce: Option<&'a str>,
}

unsafe impl js::Defined for ExternalRuntimeProps<'_> {}

/// Of a bootstrap script's descriptor, its integrity, of text alone, and its
/// crossOrigin, `use-credentials` or anonymous.
fn script_credentials(script_config: &BootstrapScriptDescriptor) -> (Option<&str>, Option<&str>) {
    let integrity = match js::classify(script_config.integrity) {
        Kind::String(integrity) => Some(integrity),
        _ => None,
    };
    let cross_origin = if script_config.cross_origin.is_none() {
        None
    } else if js::strict_equals(script_config.cross_origin, "use-credentials") {
        Some("use-credentials")
    } else {
        Some("")
    };
    (integrity, cross_origin)
}

/// The tags of a bootstrap script or module of `src`, `start` its tag's start.
fn push_bootstrap_script(
    bootstrap_chunks: &Chunks,
    resumable_state: &ResumableState,
    start: &'static Chunk,
    (src, integrity, cross_origin): (&str, Option<&str>, Option<&str>),
    nonce_script: Option<&str>,
) {
    bootstrap_chunks.borrow_mut().extend([
        start,
        string_to_chunk(&escape_text_for_browser(Some(js::unknown(src)))),
        chunk(&ATTRIBUTE_END),
    ]);
    if let Some(nonce_script) = nonce_script.filter(|nonce| !nonce.is_empty()) {
        bootstrap_chunks.borrow_mut().extend([
            chunk(&SCRIPT_NONCE),
            string_to_chunk(&escape_text_for_browser(Some(js::unknown(nonce_script)))),
            chunk(&ATTRIBUTE_END),
        ]);
    }
    if let Some(integrity) = integrity {
        bootstrap_chunks.borrow_mut().extend([
            chunk(&SCRIPT_INTEGIRTY),
            string_to_chunk(&escape_text_for_browser(Some(js::unknown(integrity)))),
            chunk(&ATTRIBUTE_END),
        ]);
    }
    if let Some(cross_origin) = cross_origin {
        bootstrap_chunks.borrow_mut().extend([
            chunk(&SCRIPT_CROSS_ORIGIN),
            string_to_chunk(&escape_text_for_browser(Some(js::unknown(cross_origin)))),
            chunk(&ATTRIBUTE_END),
        ]);
    }
    push_completed_shell_id_attribute(bootstrap_chunks, resumable_state);
    bootstrap_chunks.borrow_mut().push(chunk(&END_ASYNC_SCRIPT));
}

// Allows us to keep track of what we've already written so we can refer back to it.
// if passed externalRuntimeConfig and the enableFizzExternalRuntime feature flag
// is set, the server will send instructions via data attributes (instead of inline scripts)
pub fn create_render_state(
    resumable_state: &ResumableState,
    nonce: Option<Nonce<'static>>,
    external_runtime_config: Option<BootstrapScript<'static>>,
    import_map: Option<&'static Unknown>,
    on_headers: Option<&'static dyn Fn(&HeadersDescriptor)>,
    max_headers_length: Option<&'static Unknown>,
) -> RenderState {
    let nonce_script = match nonce {
        Some(Nonce::Text(nonce)) => Some(nonce),
        Some(Nonce::Each(nonce)) => nonce.script.as_deref(),
        None => None,
    };
    let inline_script_with_nonce = match nonce_script {
        None => chunk(&START_INLINE_SCRIPT),
        Some(nonce_script) => string_to_precomputed_chunk(
            &("<script nonce=\"".to_owned()
                + &escape_text_for_browser(Some(js::unknown(nonce_script)))
                + "\""),
        ),
    };
    let nonce_style = match nonce {
        Some(Nonce::Each(nonce)) => nonce.style.as_deref(),
        _ => None,
    };
    let inline_style_with_nonce = match nonce_style {
        None => chunk(&START_INLINE_STYLE),
        Some(nonce_style) => string_to_precomputed_chunk(
            &("<style nonce=\"".to_owned()
                + &escape_text_for_browser(Some(js::unknown(nonce_style)))
                + "\""),
        ),
    };
    let id_prefix = &resumable_state.id_prefix;

    let bootstrap_chunks: Chunks = RefCell::new(Vec::new());
    let mut external_runtime_script = None;
    let bootstrap_script_content = resumable_state.bootstrap_script_content.get();
    let bootstrap_scripts = resumable_state.bootstrap_scripts.get();
    let bootstrap_modules = resumable_state.bootstrap_modules.get();
    if let Some(bootstrap_script_content) = bootstrap_script_content {
        bootstrap_chunks.borrow_mut().push(inline_script_with_nonce);
        push_completed_shell_id_attribute(&bootstrap_chunks, resumable_state);
        bootstrap_chunks.borrow_mut().extend([
            chunk(&END_OF_START_TAG),
            string_to_chunk(&escape_entire_inline_script_content(
                bootstrap_script_content,
            )),
            chunk(&END_INLINE_SCRIPT),
        ]);
    }
    if ENABLE_FIZZ_EXTERNAL_RUNTIME.with(|enabled| *enabled)
        && let Some(external_runtime_config) = external_runtime_config
    {
        let (src, integrity) = match external_runtime_config {
            BootstrapScript::Src(src) => (src, None),
            BootstrapScript::Descriptor(config) => (config.src.as_str(), config.integrity),
        };
        let script = ExternalRuntimeScript {
            src: src.to_owned(),
            chunks: Rc::new(RefCell::new(Vec::new())),
        };
        push_script_impl(
            &script.chunks,
            js::unknown(&ExternalRuntimeProps {
                src,
                r#async: true,
                integrity,
                nonce: nonce_script,
            }),
        );
        external_runtime_script = Some(script);
    }

    let import_map_chunks: Chunks = RefCell::new(Vec::new());
    if let Some(map) = import_map {
        import_map_chunks.borrow_mut().push(match nonce_script {
            None => chunk(&IMPORT_MAP_SCRIPT_START),
            Some(nonce_script) => string_to_precomputed_chunk(
                &("<script type=\"importmap\" nonce=\"".to_owned()
                    + &escape_text_for_browser(Some(js::unknown(nonce_script)))
                    + "\">"),
            ),
        });
        import_map_chunks
            .borrow_mut()
            .push(string_to_chunk(&escape_entire_inline_script_content(
                &js::json::stringify_with(map, None, 0)
                    .unwrap()
                    .unwrap_or_default(),
            )));
        import_map_chunks
            .borrow_mut()
            .push(chunk(&IMPORT_MAP_SCRIPT_END));
    }
    if __DEV__
        && on_headers.is_some()
        && let Kind::Number(max_headers_length) = js::classify(max_headers_length)
        && max_headers_length <= 0.0
    {
        webapi::console::error((
            "React expected a positive non-zero `maxHeadersLength` option but found %s instead. When using the `onHeaders` option you may supply an optional `maxHeadersLength` option as well however, when setting this value to zero or less no headers will be captured.",
            if max_headers_length == 0.0 {
                js::unknown("zero")
            } else {
                js::unknown(&max_headers_length)
            },
        ));
    }
    let headers = on_headers.map(|_| {
        Rc::new(Headers {
            preconnects: RefCell::new(String::new()),
            font_preloads: RefCell::new(String::new()),
            high_image_preloads: RefCell::new(String::new()),
            remaining_capacity: Cell::new(
                // We seed the remainingCapacity with 2 extra bytes because when we decrement the capacity
                // we always assume we are inserting an interstitial ", " however the first header does not actually
                // consume these two extra bytes.
                2.0 + match js::classify(max_headers_length) {
                    Kind::Number(max_headers_length) => max_headers_length,
                    _ => DEFAULT_HEADERS_CAPACITY_IN_UTF16_CODE_UNITS,
                },
            ),
        })
    });
    let render_state = RenderState {
        placeholder_prefix: string_to_precomputed_chunk(&(id_prefix.to_owned() + "P:")),
        segment_prefix: string_to_precomputed_chunk(&(id_prefix.to_owned() + "S:")),
        boundary_prefix: string_to_precomputed_chunk(&(id_prefix.to_owned() + "B:")),
        start_inline_script: inline_script_with_nonce,
        start_inline_style: inline_style_with_nonce,
        preamble: create_preamble_state(),

        external_runtime_script,
        bootstrap_chunks,
        import_map_chunks,

        on_headers,
        headers: RefCell::new(headers),
        resets: Resets {
            font: dict::new(),
            dns: dict::new(),
            connect: Rc::new(ConnectResources {
                default: dict::new(),
                anonymous: dict::new(),
                credentials: dict::new(),
            }),
            image: dict::new(),
            style: dict::new(),
        },

        charset_chunks: RefCell::new(Vec::new()),
        viewport_chunks: RefCell::new(Vec::new()),
        hoistable_chunks: RefCell::new(Vec::new()),

        // cleared on flush
        preconnects: js::set::new(None),
        font_preloads: js::set::new(None),
        high_image_preloads: js::set::new(None),
        // usedImagePreloads: new Set(),
        styles: js::map::new(None),
        bootstrap_scripts: js::set::new(None),
        scripts: js::set::new(None),
        bulk_preloads: js::set::new(None),

        preloads: Preloads {
            images: js::map::new(None),
            stylesheets: js::map::new(None),
            scripts: js::map::new(None),
            module_scripts: js::map::new(None),
        },

        nonce: NonceOption {
            script: nonce_script.map(str::to_owned),
            style: nonce_style.map(str::to_owned),
        },
        // like a module global for currently rendering boundary
        hoistable_state: Cell::new(None),
        styles_to_hoist: Cell::new(false),
    };

    if let Some(bootstrap_scripts) = bootstrap_scripts {
        for &script_config in bootstrap_scripts {
            let (src, integrity, cross_origin) = match script_config {
                BootstrapScript::Src(src) => (src, None, None),
                BootstrapScript::Descriptor(config) => {
                    let (integrity, cross_origin) = script_credentials(config);
                    (config.src.as_str(), integrity, cross_origin)
                }
            };
            let props = PreloadAsProps {
                rel: "preload",
                r#as: "script",
                fetch_priority: "low",
                nonce,
                href: src,
                integrity,
                cross_origin,
            };

            preload_bootstrap_script_or_module(
                resumable_state,
                &render_state,
                src,
                js::unknown(&props),
            );

            push_bootstrap_script(
                &render_state.bootstrap_chunks,
                resumable_state,
                chunk(&START_SCRIPT_SRC),
                (src, integrity, cross_origin),
                nonce_script,
            );
        }
    }
    if let Some(bootstrap_modules) = bootstrap_modules {
        for &script_config in bootstrap_modules {
            let (src, integrity, cross_origin) = match script_config {
                BootstrapScript::Src(src) => (src, None, None),
                BootstrapScript::Descriptor(config) => {
                    let (integrity, cross_origin) = script_credentials(config);
                    (config.src.as_str(), integrity, cross_origin)
                }
            };
            let props = PreloadModuleProps {
                rel: "modulepreload",
                fetch_priority: "low",
                nonce: nonce_script,
                href: src,
                integrity,
                cross_origin,
            };

            preload_bootstrap_script_or_module(
                resumable_state,
                &render_state,
                src,
                js::unknown(&props),
            );

            push_bootstrap_script(
                &render_state.bootstrap_chunks,
                resumable_state,
                chunk(&START_MODULE_SRC),
                (src, integrity, cross_origin),
                nonce_script,
            );
        }
    }

    render_state
}

pub fn resume_render_state(
    resumable_state: &ResumableState,
    nonce: Option<Nonce<'static>>,
) -> RenderState {
    create_render_state(resumable_state, nonce, None, None, None, None)
}

pub fn create_resumable_state(
    identifier_prefix: Option<&str>,
    external_runtime_config: Option<BootstrapScript<'static>>,
    bootstrap_script_content: Option<&'static str>,
    bootstrap_scripts: Option<&'static [BootstrapScript<'static>]>,
    bootstrap_modules: Option<&'static [BootstrapScript<'static>]>,
) -> ResumableState {
    let id_prefix = identifier_prefix.unwrap_or_default();

    let mut streaming_format = SCRIPT_STREAMING_FORMAT;
    if ENABLE_FIZZ_EXTERNAL_RUNTIME.with(|enabled| *enabled) && external_runtime_config.is_some() {
        streaming_format = DATA_STREAMING_FORMAT;
    }
    ResumableState {
        id_prefix: id_prefix.to_owned(),
        next_form_id: Cell::new(0.0),
        streaming_format,
        bootstrap_script_content: Cell::new(bootstrap_script_content),
        bootstrap_scripts: Cell::new(bootstrap_scripts),
        bootstrap_modules: Cell::new(bootstrap_modules),
        instructions: Cell::new(NOTHING_SENT),
        has_body: Cell::new(false),
        has_html: Cell::new(false),

        // @TODO add bootstrap script to implicit preloads

        // persistent
        unknown_resources: Cell::new(dict::new()),
        dns_resources: Cell::new(dict::new()),
        connect_resources: RefCell::new(Rc::new(ConnectResources {
            default: dict::new(),
            anonymous: dict::new(),
            credentials: dict::new(),
        })),
        image_resources: Cell::new(dict::new()),
        style_resources: Cell::new(dict::new()),
        script_resources: Cell::new(dict::new()),
        module_unknown_resources: Cell::new(dict::new()),
        module_script_resources: Cell::new(dict::new()),
    }
}

pub fn reset_resumable_state(resumable_state: &ResumableState, render_state: &RenderState) {
    // Resets the resumable state based on what didn't manage to fully flush in the render state.
    // This currently assumes nothing was flushed.
    resumable_state.next_form_id.set(0.0);
    resumable_state.has_body.set(false);
    resumable_state.has_html.set(false);
    resumable_state
        .unknown_resources
        .set(dict::from_entries(vec![("font", render_state.resets.font)]));
    resumable_state.dns_resources.set(render_state.resets.dns);
    resumable_state
        .connect_resources
        .replace(Rc::clone(&render_state.resets.connect));
    resumable_state
        .image_resources
        .set(render_state.resets.image);
    resumable_state
        .style_resources
        .set(render_state.resets.style);
    resumable_state.script_resources.set(dict::new());
    resumable_state.module_unknown_resources.set(dict::new());
    resumable_state.module_script_resources.set(dict::new());
    resumable_state.instructions.set(NOTHING_SENT); // Nothing was flushed so no instructions could've flushed.
}

pub fn complete_resumable_state(resumable_state: &ResumableState) {
    // This function is called when we have completed a prerender and there is a shell.
    resumable_state.bootstrap_script_content.set(None);
    resumable_state.bootstrap_scripts.set(None);
    resumable_state.bootstrap_modules.set(None);
}

pub fn create_preamble_state() -> PreambleState {
    PreambleState {
        html_chunks: RefCell::new(None),
        head_chunks: RefCell::new(None),
        body_chunks: RefCell::new(None),
    }
}

// Constants for the insertion mode we're currently writing in. We don't encode all HTML5 insertion
// modes. We only include the variants as they matter for the sake of our purposes.
// We don't actually provide the namespace therefore we use constants instead of the string.
pub const ROOT_HTML_MODE: InsertionMode = 0; // Used for the root most element tag.
// We have a less than HTML_HTML_MODE check elsewhere. If you add more cases here, make sure it
// still makes sense
const HTML_HTML_MODE: InsertionMode = 1; // Used for the <html> if it is at the top level.
const HTML_MODE: InsertionMode = 2;
const HTML_HEAD_MODE: InsertionMode = 3;
const SVG_MODE: InsertionMode = 4;
const MATHML_MODE: InsertionMode = 5;
const HTML_TABLE_MODE: InsertionMode = 6;
const HTML_TABLE_BODY_MODE: InsertionMode = 7;
const HTML_TABLE_ROW_MODE: InsertionMode = 8;
const HTML_COLGROUP_MODE: InsertionMode = 9;
// We have a greater than HTML_TABLE_MODE check elsewhere. If you add more cases here, make sure it
// still makes sense

type InsertionMode = i32;

const NO_SCOPE: i32 = /*         */ 0b0000000;
const NOSCRIPT_SCOPE: i32 = /*   */ 0b0000001;
const PICTURE_SCOPE: i32 = /*    */ 0b0000010;
const FALLBACK_SCOPE: i32 = /*   */ 0b0000100;
const EXIT_SCOPE: i32 = /*       */ 0b0001000; // A direct Instance below a Suspense fallback is the only thing that can "exit"
const ENTER_SCOPE: i32 = /*      */ 0b0010000; // A direct Instance below Suspense content is the only thing that can "enter"
const UPDATE_SCOPE: i32 = /*     */ 0b0100000; // Inside a scope that applies "update" ViewTransitions if anything mutates here.
const APPEARING_SCOPE: i32 = /*  */ 0b1000000; // Below Suspense content subtree which might appear in an "enter" animation or "shared" animation.
const PARENT_EXIT_SCOPE: i32 = /* */ 0b10000000; // Below a ViewTransition that is exiting.
const PARENT_ENTER_SCOPE: i32 = /* */ 0b100000000; // Below a ViewTransition that is entering.

// Everything not listed here are tracked for the whole subtree as opposed to just
// until the next Instance.
const SUBTREE_SCOPE: i32 = !(ENTER_SCOPE | EXIT_SCOPE);

pub struct ViewTransitionContext {
    pub update: String,
    pub enter: String,
    pub exit: String,
    pub share: String,
    pub parent_enter: String,
    pub parent_exit: String,
    pub name: String,
    pub auto_name: String, // a name that can be used if an explicit one is not defined.
    pub name_idx: Cell<f64>, // keeps track of how many duplicates of this name we've emitted.
}

/// The selected value(s) inside a `<select>`, as its props gave them.
pub type SelectedValue = Option<&'static Unknown>;

// Lets us keep track of contextual state and pick it back up after suspending.
pub struct FormatContext {
    pub insertion_mode: InsertionMode, // root/svg/html/mathml/table
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub selected_value: SelectedValue, // the selected value(s) inside a <select>, or null outside <select>
    pub tag_scope: i32,
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub view_transition: Option<Rc<ViewTransitionContext>>, // tracks if we're inside a ViewTransition outside the first DOM node
}

fn create_format_context(
    insertion_mode: InsertionMode,
    selected_value: SelectedValue,
    tag_scope: i32,
    view_transition: Option<Rc<ViewTransitionContext>>,
) -> Rc<FormatContext> {
    Rc::new(FormatContext {
        insertion_mode,
        selected_value,
        tag_scope,
        view_transition,
    })
}

pub fn can_have_preamble(format_context: &FormatContext) -> bool {
    format_context.insertion_mode < HTML_MODE
}

pub fn create_root_format_context(namespace_uri: Option<&str>) -> Rc<FormatContext> {
    let insertion_mode = if namespace_uri == Some("http://www.w3.org/2000/svg") {
        SVG_MODE
    } else if namespace_uri == Some("http://www.w3.org/1998/Math/MathML") {
        MATHML_MODE
    } else {
        ROOT_HTML_MODE
    };
    create_format_context(insertion_mode, None, NO_SCOPE, None)
}

pub fn get_child_format_context(
    parent_context: &Rc<FormatContext>,
    r#type: &str,
    props: &Unknown,
) -> Rc<FormatContext> {
    let subtree_scope = parent_context.tag_scope & SUBTREE_SCOPE;
    match r#type {
        "noscript" => {
            return create_format_context(HTML_MODE, None, subtree_scope | NOSCRIPT_SCOPE, None);
        }
        "select" => {
            return create_format_context(
                HTML_MODE,
                js::get(props, "value").or_else(|| js::get(props, "defaultValue")),
                subtree_scope,
                None,
            );
        }
        "svg" => return create_format_context(SVG_MODE, None, subtree_scope, None),
        "picture" => {
            return create_format_context(HTML_MODE, None, subtree_scope | PICTURE_SCOPE, None);
        }
        "math" => return create_format_context(MATHML_MODE, None, subtree_scope, None),
        "foreignObject" => return create_format_context(HTML_MODE, None, subtree_scope, None),
        // Table parents are special in that their children can only be created at all if they're
        // wrapped in a table parent. So we need to encode that we're entering this mode.
        "table" => return create_format_context(HTML_TABLE_MODE, None, subtree_scope, None),
        "thead" | "tbody" | "tfoot" => {
            return create_format_context(HTML_TABLE_BODY_MODE, None, subtree_scope, None);
        }
        "colgroup" => return create_format_context(HTML_COLGROUP_MODE, None, subtree_scope, None),
        "tr" => return create_format_context(HTML_TABLE_ROW_MODE, None, subtree_scope, None),
        "head" => {
            if parent_context.insertion_mode < HTML_MODE {
                // We are either at the root or inside the <html> tag and can enter
                // the <head> scope
                return create_format_context(HTML_HEAD_MODE, None, subtree_scope, None);
            }
        }
        "html" if parent_context.insertion_mode == ROOT_HTML_MODE => {
            return create_format_context(HTML_HTML_MODE, None, subtree_scope, None);
        }
        _ => {}
    }
    if parent_context.insertion_mode >= HTML_TABLE_MODE {
        // Whatever tag this was, it wasn't a table parent or other special parent, so we must have
        // entered plain HTML again.
        return create_format_context(HTML_MODE, None, subtree_scope, None);
    }
    if parent_context.insertion_mode < HTML_MODE {
        return create_format_context(HTML_MODE, None, subtree_scope, None);
    }
    if ENABLE_VIEW_TRANSITION && parent_context.view_transition.is_some() {
        // If we're inside a view transition, regardless what element we were in, it consumes
        // the view transition context.
        return create_format_context(
            parent_context.insertion_mode,
            parent_context.selected_value,
            subtree_scope,
            None,
        );
    }
    if parent_context.tag_scope != subtree_scope {
        return create_format_context(
            parent_context.insertion_mode,
            parent_context.selected_value,
            subtree_scope,
            None,
        );
    }
    Rc::clone(parent_context)
}

fn get_suspense_view_transition(
    parent_view_transition: Option<&ViewTransitionContext>,
) -> Option<Rc<ViewTransitionContext>> {
    let parent_view_transition = parent_view_transition?;
    // If a ViewTransition wraps a Suspense boundary it applies to the children Instances
    // in both the fallback and the content.
    // Since we only have a representation of ViewTransitions on the Instances themselves
    // we cannot model the parent ViewTransition activating "enter", "exit" or "share"
    // since those would be ambiguous with the Suspense boundary changing states and
    // affecting the same Instances.
    // We also can't model an "update" when that update is fallback nodes swapping for
    // content nodes. However, we can model is as a "share" from the fallback nodes to
    // the content nodes using the same name. We just have to assign the same name that
    // we would've used (the parent ViewTransition name or auto-assign one).
    let view_transition = ViewTransitionContext {
        update: parent_view_transition.update.clone(), // For deep updates.
        enter: "none".to_owned(),
        exit: "none".to_owned(),
        share: parent_view_transition.update.clone(), // For exit or enter of reveals.
        parent_enter: "none".to_owned(),
        parent_exit: "none".to_owned(),
        name: parent_view_transition.auto_name.clone(),
        auto_name: parent_view_transition.auto_name.clone(),
        // TOOD: If we have more than just this Suspense boundary as a child of the ViewTransition
        // then the parent needs to isolate the names so that they don't conflict.
        name_idx: Cell::new(0.0),
    };
    Some(Rc::new(view_transition))
}

pub fn get_suspense_fallback_format_context(
    resumable_state: &ResumableState,
    parent_context: &FormatContext,
) -> Rc<FormatContext> {
    if parent_context.tag_scope & UPDATE_SCOPE != 0 {
        // If we're rendering a Suspense in fallback mode and that is inside a ViewTransition,
        // which hasn't disabled updates, then revealing it might animate the parent so we need
        // the ViewTransition instructions.
        resumable_state
            .instructions
            .set(resumable_state.instructions.get() | NEED_UPGRADE_TO_VIEW_TRANSITIONS);
    }
    create_format_context(
        parent_context.insertion_mode,
        parent_context.selected_value,
        parent_context.tag_scope | FALLBACK_SCOPE | EXIT_SCOPE,
        get_suspense_view_transition(parent_context.view_transition.as_deref()),
    )
}

pub fn get_suspense_content_format_context(
    _resumable_state: &ResumableState,
    parent_context: &FormatContext,
) -> Rc<FormatContext> {
    let view_transition = get_suspense_view_transition(parent_context.view_transition.as_deref());
    let mut subtree_scope = parent_context.tag_scope | ENTER_SCOPE;
    if let Some(view_transition) = &view_transition
        && view_transition.share != "none"
    {
        // If we have a ViewTransition wrapping Suspense then the appearing animation
        // will be applied just like an "enter" below. Mark it as animating.
        subtree_scope |= APPEARING_SCOPE;
    }
    create_format_context(
        parent_context.insertion_mode,
        parent_context.selected_value,
        subtree_scope,
        view_transition,
    )
}

pub fn get_view_transition_format_context(
    resumable_state: &ResumableState,
    parent_context: &FormatContext,
    update: Option<&str>,
    enter: Option<&str>,
    exit: Option<&str>,
    share: Option<&str>,
    // The class of each, `null` of "auto" and `undefined` where the prop is absent,
    // which stops the relay where "auto" continues it.
    parent_enter: &Unknown,
    parent_exit: &Unknown,
    has_parent_enter_handler: bool,
    has_parent_exit_handler: bool,
    name: Option<&str>,
    auto_name: &str, // name or an autogenerated unique name
) -> Rc<FormatContext> {
    // We're entering a <ViewTransition>. Normalize props.
    let update = update.unwrap_or("auto");
    let mut enter = enter.unwrap_or("auto");
    let mut exit = exit.unwrap_or("auto");
    let (name, share) = match name {
        None => match &parent_context.view_transition {
            // If we have multiple nested ViewTransition and the parent has a "share"
            // but the child doesn't, then the parent ViewTransition can still activate
            // a share scenario so we reuse the name and share from the parent.
            Some(parent_view_transition) => (
                parent_view_transition.name.clone(),
                parent_view_transition.share.clone(),
            ),
            // share is only relevant if there's an explicit name
            None => ("auto".to_owned(), "none".to_owned()),
        },
        Some(name) => {
            if parent_context.tag_scope & FALLBACK_SCOPE != 0 {
                // If we have an explicit name and share is not disabled, and we're inside
                // a fallback, then that fallback might pair with content and so we might need
                // the ViewTransition instructions to animate between them.
                resumable_state
                    .instructions
                    .set(resumable_state.instructions.get() | NEED_UPGRADE_TO_VIEW_TRANSITIONS);
            }
            (name.to_owned(), share.unwrap_or("auto").to_owned())
        }
    };
    if parent_context.tag_scope & EXIT_SCOPE == 0 {
        exit = "none"; // exit is only relevant for the first ViewTransition inside fallback
    } else {
        resumable_state
            .instructions
            .set(resumable_state.instructions.get() | NEED_UPGRADE_TO_VIEW_TRANSITIONS);
    }
    if parent_context.tag_scope & ENTER_SCOPE == 0 {
        enter = "none"; // enter is only relevant for the first ViewTransition inside content
    } else {
        resumable_state
            .instructions
            .set(resumable_state.instructions.get() | NEED_UPGRADE_TO_VIEW_TRANSITIONS);
    }
    let parent_enter_exit = ENABLE_VIEW_TRANSITION_PARENT_ENTER_EXIT.with(|enabled| *enabled);
    let mut resolved_parent_enter = "none";
    let mut resolved_parent_exit = "none";
    if parent_enter_exit {
        if let Kind::String(parent_enter) = js::classify(Some(parent_enter))
            && (parent_context.tag_scope & PARENT_ENTER_SCOPE) != 0
        {
            resolved_parent_enter = parent_enter;
        }
        if let Kind::String(parent_exit) = js::classify(Some(parent_exit))
            && (parent_context.tag_scope & PARENT_EXIT_SCOPE) != 0
        {
            resolved_parent_exit = parent_exit;
        }
    }
    let view_transition = ViewTransitionContext {
        update: update.to_owned(),
        enter: enter.to_owned(),
        exit: exit.to_owned(),
        share,
        parent_enter: resolved_parent_enter.to_owned(),
        parent_exit: resolved_parent_exit.to_owned(),
        name,
        auto_name: auto_name.to_owned(),
        name_idx: Cell::new(0.0),
    };
    let mut subtree_scope = parent_context.tag_scope & SUBTREE_SCOPE;
    if update != "none" {
        subtree_scope |= UPDATE_SCOPE;
    } else {
        subtree_scope &= !UPDATE_SCOPE;
    }
    if enter != "none" {
        subtree_scope |= APPEARING_SCOPE;
    }
    if parent_enter_exit {
        // Parent enter relay: a ViewTransition that is itself entering starts a relay
        // for its subtree. A nested ViewTransition continues that relay as long as it
        // opts in with a parentEnter prop that doesn't resolve to "none", or with an
        // onParentEnter handler. A missing parentEnter (with no handler) or an
        // explicit "none" stops the relay so descendants below it don't participate.
        // This mirrors commitParentEnterViewTransitions.
        if enter != "none" {
            subtree_scope |= PARENT_ENTER_SCOPE;
        } else if (parent_context.tag_scope & PARENT_ENTER_SCOPE) != 0
            && (js::strict_equals(Some(parent_enter), "none")
                || (matches!(js::classify(Some(parent_enter)), Kind::Undefined)
                    && !has_parent_enter_handler))
        {
            subtree_scope &= !PARENT_ENTER_SCOPE;
        }
        // Parent exit relay: mirror of the enter relay above.
        if exit != "none" {
            subtree_scope |= PARENT_EXIT_SCOPE;
        } else if (parent_context.tag_scope & PARENT_EXIT_SCOPE) != 0
            && (js::strict_equals(Some(parent_exit), "none")
                || (matches!(js::classify(Some(parent_exit)), Kind::Undefined)
                    && !has_parent_exit_handler))
        {
            subtree_scope &= !PARENT_EXIT_SCOPE;
        }
    }
    create_format_context(
        parent_context.insertion_mode,
        parent_context.selected_value,
        subtree_scope,
        Some(Rc::new(view_transition)),
    )
}

pub fn is_preamble_context(format_context: &FormatContext) -> bool {
    format_context.insertion_mode == HTML_HEAD_MODE
}

pub fn make_id(resumable_state: &ResumableState, tree_id: &str, local_id: f64) -> String {
    let id_prefix = &resumable_state.id_prefix;

    let mut id = "_".to_owned() + id_prefix + "R_" + tree_id;

    // Unless this is the first id at this level, append a number at the end
    // that represents the position of this useId hook among all the useId
    // hooks for this fiber.
    if local_id > 0.0 {
        id += &("H".to_owned() + &js::number::to_string(local_id, Some(32)));
    }

    id + "_"
}

#[cfg_attr(rust_js, rust_js::name = "encodeHTMLTextNode")]
fn encode_html_text_node(text: &str) -> String {
    escape_text_for_browser(Some(js::unknown(text)))
}

thread_local! {
    static TEXT_SEPARATOR: &'static PrecomputedChunk = string_to_precomputed_chunk("<!-- -->");
}

pub fn push_text_instance(
    target: &Chunks,
    text: &str,
    _render_state: &RenderState,
    text_embedded: bool,
) -> bool {
    if text.is_empty() {
        // Empty text doesn't have a DOM node representation and the hydration is aware of this.
        return text_embedded;
    }
    if text_embedded {
        target.borrow_mut().push(chunk(&TEXT_SEPARATOR));
    }
    target
        .borrow_mut()
        .push(string_to_chunk(&encode_html_text_node(text)));
    true
}

// Called when Fizz is done with a Segment. Currently the only purpose is to conditionally
// emit a text separator when we don't know for sure it is safe to omit
pub fn push_segment_finale(
    target: &Chunks,
    _render_state: &RenderState,
    last_pushed_text: bool,
    text_embedded: bool,
) {
    if last_pushed_text && text_embedded {
        target.borrow_mut().push(chunk(&TEXT_SEPARATOR));
    }
}

fn push_view_transition_attributes(target: &Chunks, format_context: &FormatContext) {
    if !ENABLE_VIEW_TRANSITION {
        return;
    }
    let Some(view_transition) = &format_context.view_transition else {
        return;
    };
    if view_transition.name != "auto" {
        let name_idx = view_transition.name_idx.get();
        push_string_attribute(
            target,
            "vt-name",
            js::unknown(&if name_idx == 0.0 {
                view_transition.name.clone()
            } else {
                format!("{}_{}", view_transition.name, name_idx)
            }),
        );
        // Increment the index in case we have multiple children to the same ViewTransition.
        // Because this is a side-effect in render, we should ideally call pushViewTransitionAttributes
        // after we've suspended (like forms do), so that we don't increment each attempt.
        // TODO: Make this deterministic.
        view_transition.name_idx.set(name_idx + 1.0);
    }
    push_string_attribute(
        target,
        "vt-update",
        js::unknown(view_transition.update.as_str()),
    );
    if view_transition.enter != "none" {
        push_string_attribute(
            target,
            "vt-enter",
            js::unknown(view_transition.enter.as_str()),
        );
    }
    if view_transition.exit != "none" {
        push_string_attribute(
            target,
            "vt-exit",
            js::unknown(view_transition.exit.as_str()),
        );
    }
    if view_transition.share != "none" {
        push_string_attribute(
            target,
            "vt-share",
            js::unknown(view_transition.share.as_str()),
        );
    }
    let parent_enter_exit = ENABLE_VIEW_TRANSITION_PARENT_ENTER_EXIT.with(|enabled| *enabled);
    if parent_enter_exit && view_transition.parent_enter != "none" {
        push_string_attribute(
            target,
            "vt-parent-enter",
            js::unknown(view_transition.parent_enter.as_str()),
        );
    }
    if parent_enter_exit && view_transition.parent_exit != "none" {
        push_string_attribute(
            target,
            "vt-parent-exit",
            js::unknown(view_transition.parent_exit.as_str()),
        );
    }
}

thread_local! {
    static STYLE_NAME_CACHE: &'static js::Map<str, &'static PrecomputedChunk> = js::map::new(None);
}

fn process_style_name(style_name: &str) -> &'static PrecomputedChunk {
    STYLE_NAME_CACHE.with(|style_name_cache| {
        if let Some(chunk) = style_name_cache.get(style_name) {
            return chunk;
        }
        let result = string_to_precomputed_chunk(&escape_text_for_browser(Some(js::unknown(
            hyphenate_style_name(style_name).as_str(),
        ))));
        style_name_cache.set(style_name, result);
        result
    })
}

thread_local! {
    static STYLE_ATTRIBUTE_START: &'static PrecomputedChunk = string_to_precomputed_chunk(" style=\"");
    static STYLE_ASSIGN: &'static PrecomputedChunk = string_to_precomputed_chunk(":");
    static STYLE_SEPARATOR: &'static PrecomputedChunk = string_to_precomputed_chunk(";");
}

fn push_style_attribute(target: &Chunks, style: &Unknown) {
    if js::type_of(Some(style)) != "object" {
        panic!(
            "{}",
            concat!(
                "The `style` prop expects a mapping from style properties to values, ",
                "not a string. For example, style={{marginRight: spacing + 'em'}} when ",
                "using JSX."
            )
        );
    }

    let mut is_first = true;
    for style_name in js::keys_in(style) {
        if !HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(style, &style_name)) {
            continue;
        }
        // If you provide unsafe user data here they can inject arbitrary CSS
        // which may be problematic (I couldn't repro this):
        // https://www.owasp.org/index.php/XSS_Filter_Evasion_Cheat_Sheet
        // http://www.thespanner.co.uk/2007/11/26/ultimate-xss-css-injection/
        // This is not an XSS hole but instead a potential CSS injection issue
        // which has lead to a greater discussion about how we're going to
        // trust URLs moving forward. See #2115901
        let style_value = js::get(style, &style_name);
        if style_value.is_none()
            || js::type_of(style_value) == "boolean"
            || js::strict_equals(style_value, "")
        {
            // TODO: We used to set empty string as a style with an empty value. Does that ever make sense?
            continue;
        }

        let name_chunk;
        let value_chunk;
        let is_custom_property = js::string::index_of(&style_name, "--", None) == 0.0;
        if is_custom_property {
            name_chunk = string_to_chunk(&escape_text_for_browser(Some(js::unknown(
                style_name.as_str(),
            ))));
            if __DEV__ {
                check_css_property_string_coercion(style_value, &style_name);
            }
            value_chunk = string_to_chunk(&escape_text_for_browser(Some(js::unknown(
                js::concat_text(style_value).unwrap().trim(),
            ))));
        } else {
            if __DEV__ {
                warn_valid_style(&style_name, style_value);
            }

            name_chunk = process_style_name(&style_name);
            if let Kind::Number(style_value) = js::classify(style_value) {
                if style_value != 0.0 && !is_unitless_number(&style_name) {
                    value_chunk =
                        string_to_chunk(&(js::number::to_string(style_value, None) + "px")); // Presumes implicit 'px' suffix for unitless numbers
                } else {
                    value_chunk = string_to_chunk(&js::number::to_string(style_value, None));
                }
            } else {
                if __DEV__ {
                    check_css_property_string_coercion(style_value, &style_name);
                }
                value_chunk = string_to_chunk(&escape_text_for_browser(Some(js::unknown(
                    js::concat_text(style_value).unwrap().trim(),
                ))));
            }
        }
        if is_first {
            is_first = false;
            // If it's first, we don't need any separators prefixed.
            target.borrow_mut().extend([
                chunk(&STYLE_ATTRIBUTE_START),
                name_chunk,
                chunk(&STYLE_ASSIGN),
                value_chunk,
            ]);
        } else {
            target.borrow_mut().extend([
                chunk(&STYLE_SEPARATOR),
                name_chunk,
                chunk(&STYLE_ASSIGN),
                value_chunk,
            ]);
        }
    }
    if !is_first {
        target.borrow_mut().push(chunk(&ATTRIBUTE_END));
    }
}

thread_local! {
    static ATTRIBUTE_SEPARATOR: &'static PrecomputedChunk = string_to_precomputed_chunk(" ");
    static ATTRIBUTE_ASSIGN: &'static PrecomputedChunk = string_to_precomputed_chunk("=\"");
    static ATTRIBUTE_END: &'static PrecomputedChunk = string_to_precomputed_chunk("\"");
    static ATTRIBUTE_EMPTY_STRING: &'static PrecomputedChunk = string_to_precomputed_chunk("=\"\"");
}

fn push_boolean_attribute(
    target: &Chunks,
    name: &str,
    value: &Unknown, // not null or undefined
) {
    if js::truthy(Some(value))
        && js::type_of(Some(value)) != "function"
        && js::type_of(Some(value)) != "symbol"
    {
        target.borrow_mut().extend([
            chunk(&ATTRIBUTE_SEPARATOR),
            string_to_chunk(name),
            chunk(&ATTRIBUTE_EMPTY_STRING),
        ]);
    }
}

fn push_string_attribute(
    target: &Chunks,
    name: &str,
    value: &Unknown, // not null or undefined
) {
    if js::type_of(Some(value)) != "function"
        && js::type_of(Some(value)) != "symbol"
        && js::type_of(Some(value)) != "boolean"
    {
        target.borrow_mut().extend([
            chunk(&ATTRIBUTE_SEPARATOR),
            string_to_chunk(name),
            chunk(&ATTRIBUTE_ASSIGN),
            string_to_chunk(&escape_text_for_browser(Some(value))),
            chunk(&ATTRIBUTE_END),
        ]);
    }
}

fn make_form_field_prefix(resumable_state: &ResumableState) -> String {
    // TODO: Make this deterministic.
    let id = resumable_state.next_form_id.get();
    resumable_state.next_form_id.set(id + 1.0);
    resumable_state.id_prefix.clone() + &js::number::to_string(id, None)
}

thread_local! {
    // Since this will likely be repeated a lot in the HTML, we use a more concise message
    // than on the client and hopefully it's googleable.
    static ACTION_JAVA_SCRIPT_URL: &'static PrecomputedChunk = string_to_precomputed_chunk(&escape_text_for_browser(
        Some(js::unknown("javascript:throw new Error('React form unexpectedly submitted.')")),
    ));

    static START_HIDDEN_INPUT_CHUNK: &'static PrecomputedChunk = string_to_precomputed_chunk("<input type=\"hidden\"");
}

fn push_additional_form_field(target: &Chunks, value: FormDataEntryValue<'_>, key: &str) {
    target.borrow_mut().push(chunk(&START_HIDDEN_INPUT_CHUNK));
    let value = validate_additional_form_field(value, key);
    push_string_attribute(target, "name", js::unknown(key));
    push_string_attribute(target, "value", js::unknown(value));
    target
        .borrow_mut()
        .push(chunk(&END_OF_START_TAG_SELF_CLOSING));
}

fn push_additional_form_fields(target: &Chunks, form_data: Option<&FormData>) {
    if let Some(form_data) = form_data {
        for (key, value) in webapi::form_data::iter(form_data) {
            push_additional_form_field(target, value, &key);
        }
    }
}

fn validate_additional_form_field<'a>(value: FormDataEntryValue<'a>, _key: &str) -> &'a str {
    match value {
        FormDataEntryValue::Str(value) => value,
        FormDataEntryValue::File(_) => panic!(
            "{}",
            concat!(
                "File/Blob fields are not yet supported in progressive forms. ",
                "Will fallback to client hydration."
            )
        ),
    }
}

fn validate_additional_form_fields(
    form_data: Option<&'static FormData>,
) -> Option<&'static FormData> {
    if let Some(form_data) = form_data {
        form_data.for_each(Box::new(|value, key, _| {
            validate_additional_form_field(value, key);
        }));
    }
    form_data
}

/// A form action a Server Action's reference is, which encodes its fields
/// for progressive enhancement.
struct ServerAction(PhantomData<JsObject>);

impl ServerAction {
    #[cfg_attr(rust_js, rust_js::link_name = "$$FORM_ACTION")]
    fn form_action(&self, prefix: &str) -> Option<&'static ReactCustomFormAction> {
        let _ = prefix;
        unreachable!()
    }
}

fn get_custom_form_fields(
    resumable_state: &ResumableState,
    form_action: &'static Unknown,
) -> Option<&'static ReactCustomFormAction> {
    let custom_action = js::get(form_action, "$$FORM_ACTION");
    if js::type_of(custom_action) == "function" {
        let prefix = make_form_field_prefix(resumable_state);
        match panic::catch_unwind(AssertUnwindSafe(|| {
            let server_action: &ServerAction = unsafe { js::cast(Some(form_action)) };
            let custom_fields = server_action.form_action(&prefix);
            if let Some(custom_fields) = custom_fields {
                validate_additional_form_fields(custom_fields.data);
            }
            custom_fields
        })) {
            Ok(custom_fields) => return custom_fields,
            Err(payload) => {
                let x = payload
                    .downcast_ref::<&'static js::JsError>()
                    .and_then(|x| x.as_unknown());
                if js::type_of(x) == "object"
                    && x.is_some()
                    && js::type_of(js::get(x, "then")) == "function"
                {
                    // Rethrow suspense.
                    panic::resume_unwind(payload);
                }
                // If we fail to encode the form action for progressive enhancement for some reason,
                // fallback to trying replaying on the client instead of failing the page. It might
                // work there.
                if __DEV__ {
                    // TODO: Should this be some kind of recoverable error?
                    webapi::console::error((
                        "Failed to serialize an action for progressive enhancement:\n%s",
                        x,
                    ));
                }
            }
        }
    }
    None
}

/// The props a form's or a button's action is written with.
struct FormActionProps {
    form_action: Option<&'static Unknown>,
    form_enc_type: Option<&'static Unknown>,
    form_method: Option<&'static Unknown>,
    form_target: Option<&'static Unknown>,
    name: Option<&'static Unknown>,
}

fn push_form_action_attribute(
    target: &Chunks,
    resumable_state: &ResumableState,
    render_state: &RenderState,
    props: FormActionProps,
) -> Option<&'static FormData> {
    let FormActionProps {
        mut form_action,
        mut form_enc_type,
        mut form_method,
        mut form_target,
        mut name,
    } = props;
    let mut form_data = None;
    if let Some(action) = form_action
        && js::type_of(Some(action)) == "function"
    {
        // Function form actions cannot control the form properties
        if __DEV__ {
            if name.is_some() && !DID_WARN_FORM_ACTION_NAME.get() {
                DID_WARN_FORM_ACTION_NAME.set(true);
                webapi::console::error((concat!(
                    "Cannot specify a \"name\" prop for a button that specifies a function as a formAction. ",
                    "React needs it to encode which action should be invoked. It will get overridden."
                ),));
            }
            if (form_enc_type.is_some() || form_method.is_some())
                && !DID_WARN_FORM_ACTION_METHOD.get()
            {
                DID_WARN_FORM_ACTION_METHOD.set(true);
                webapi::console::error((concat!(
                    "Cannot specify a formEncType or formMethod for a button that specifies a ",
                    "function as a formAction. React provides those automatically. They will get overridden."
                ),));
            }
            if form_target.is_some() && !DID_WARN_FORM_ACTION_TARGET.get() {
                DID_WARN_FORM_ACTION_TARGET.set(true);
                webapi::console::error((concat!(
                    "Cannot specify a formTarget for a button that specifies a function as a formAction. ",
                    "The function will always be executed in the same window."
                ),));
            }
        }
        if let Some(custom_fields) = get_custom_form_fields(resumable_state, action) {
            // This action has a custom progressive enhancement form that can submit the form
            // back to the server if it's invoked before hydration. Such as a Server Action.
            name = custom_fields.name.as_deref().map(js::unknown);
            form_action = Some(js::unknown(
                custom_fields.action.as_deref().unwrap_or_default(),
            ));
            form_enc_type = custom_fields.enc_type.as_deref().map(js::unknown);
            form_method = custom_fields.method.as_deref().map(js::unknown);
            form_target = custom_fields.target.as_deref().map(js::unknown);
            form_data = custom_fields.data;
        } else {
            // Set a javascript URL that doesn't do anything. We don't expect this to be invoked
            // because we'll preventDefault in the Fizz runtime, but it can happen if a form is
            // manually submitted or if someone calls stopPropagation before React gets the event.
            // If CSP is used to block javascript: URLs that's fine too. It just won't show this
            // error message but the URL will be logged.
            target.borrow_mut().extend([
                chunk(&ATTRIBUTE_SEPARATOR),
                string_to_chunk("formAction"),
                chunk(&ATTRIBUTE_ASSIGN),
                chunk(&ACTION_JAVA_SCRIPT_URL),
                chunk(&ATTRIBUTE_END),
            ]);
            name = None;
            form_action = None;
            form_enc_type = None;
            form_method = None;
            form_target = None;
            inject_form_replaying_runtime(resumable_state, render_state);
        }
    }
    if let Some(name) = name {
        push_attribute(target, "name", name);
    }
    if let Some(form_action) = form_action {
        push_attribute(target, "formAction", form_action);
    }
    if let Some(form_enc_type) = form_enc_type {
        push_attribute(target, "formEncType", form_enc_type);
    }
    if let Some(form_method) = form_method {
        push_attribute(target, "formMethod", form_method);
    }
    if let Some(form_target) = form_target {
        push_attribute(target, "formTarget", form_target);
    }
    form_data
}

thread_local! {
    #[cfg_attr(rust_js, rust_js::nullable)]
    static BLOB_CACHE: Cell<Option<&'static js::WeakMap<Blob, &'static Thenable>>> = const { Cell::new(None) };
}

fn push_src_object_attribute(target: &Chunks, blob: &'static Blob) {
    // Throwing a Promise style suspense read of the Blob content.
    if BLOB_CACHE.get().is_none() {
        BLOB_CACHE.set(Some(js::weak_map::new()));
    }
    let suspense_cache = BLOB_CACHE.get().unwrap();
    let thenable = match suspense_cache.get(blob) {
        Some(thenable) => thenable,
        None => {
            let thenable = read_as_data_url(blob);
            thenable.then(
                move |result| {
                    thenable.set_status("fulfilled");
                    thenable.set_value(result);
                },
                move |error| {
                    thenable.set_status("rejected");
                    thenable.set_reason(error);
                },
            );
            suspense_cache.set(blob, thenable);
            thenable
        }
    };
    if thenable.status().as_deref() == Some("rejected") {
        js::throw(thenable.reason());
    } else if thenable.status().as_deref() != Some("fulfilled") {
        js::throw(Some(js::unknown(thenable)));
    }
    let url = thenable.value();
    target.borrow_mut().extend([
        chunk(&ATTRIBUTE_SEPARATOR),
        string_to_chunk("src"),
        chunk(&ATTRIBUTE_ASSIGN),
        string_to_chunk(&escape_text_for_browser(url)),
        chunk(&ATTRIBUTE_END),
    ]);
}

/// `value`'s attribute of `name`, its text escaped.
fn push_text_attribute(target: &Chunks, name: &str, value: &Unknown) {
    target.borrow_mut().extend([
        chunk(&ATTRIBUTE_SEPARATOR),
        string_to_chunk(name),
        chunk(&ATTRIBUTE_ASSIGN),
        string_to_chunk(&escape_text_for_browser(Some(value))),
        chunk(&ATTRIBUTE_END),
    ]);
}

/// A URL's attribute of `name`, of `value` sanitized.
fn push_url_attribute(target: &Chunks, name: &str, attribute_name: &str, value: &'static Unknown) {
    if __DEV__ {
        check_attribute_string_coercion(Some(value), name);
    }
    let sanitized_value = sanitize_url(Some(js::unknown_of(js::concat_text(Some(value)).unwrap())));
    target.borrow_mut().extend([
        chunk(&ATTRIBUTE_SEPARATOR),
        string_to_chunk(attribute_name),
        chunk(&ATTRIBUTE_ASSIGN),
        string_to_chunk(&escape_text_for_browser(sanitized_value)),
        chunk(&ATTRIBUTE_END),
    ]);
}

fn push_attribute(
    target: &Chunks,
    name: &str,
    value: &'static Unknown, // not null or undefined
) {
    let callable = matches!(
        js::classify(Some(value)),
        Kind::Function(_) | Kind::Symbol(_)
    );
    match name {
        // These are very common props and therefore are in the beginning of the switch.
        // TODO: aria-label is a very common prop but allows booleans so is not like the others
        // but should ideally go in this list too.
        "className" => push_string_attribute(target, "class", value),
        "tabIndex" => push_string_attribute(target, "tabindex", value),
        "dir" | "role" | "viewBox" | "width" | "height" => push_string_attribute(target, name, value),
        "style" => push_style_attribute(target, value),
        "src" | "href" | "action" | "formAction" => {
            if name == "src"
                && ENABLE_SRC_OBJECT.with(|enabled| *enabled)
                && js::classify(Some(value)).object().is_some()
                && js::is_function(js::class::<Blob>)
                && js::instance_of(Some(value), js::class::<Blob>())
            {
                push_src_object_attribute(target, unsafe { js::cast(Some(value)) });
                return;
            }
            if (name == "src" || name == "href") && js::strict_equals(Some(value), "") {
                if __DEV__ {
                    if name == "src" {
                        webapi::console::error((
                            concat!(
                                "An empty string (\"\") was passed to the %s attribute. ",
                                "This may cause the browser to download the whole page again over the network. ",
                                "To fix this, either do not render the element at all ",
                                "or pass null to %s instead of an empty string."
                            ),
                            name,
                            name,
                        ));
                    } else {
                        webapi::console::error((
                            concat!(
                                "An empty string (\"\") was passed to the %s attribute. ",
                                "To fix this, either do not render the element at all ",
                                "or pass null to %s instead of an empty string."
                            ),
                            name,
                            name,
                        ));
                    }
                }
                return;
            }
            // TODO: Consider only special casing these for each tag.
            if callable || js::type_of(Some(value)) == "boolean" {
                return;
            }
            push_url_attribute(target, name, name, value);
        }
        // These shouldn't be set as attributes on generic HTML elements.
        "defaultValue" | "defaultChecked"
        // Must use dangerouslySetInnerHTML instead.
        | "innerHTML"
        | "suppressContentEditableWarning"
        | "suppressHydrationWarning"
        | "ref" => {
            // Ignored. These are built-in to React on the client.
        }
        "autoFocus" | "multiple" | "muted" => push_boolean_attribute(target, &name.to_lowercase(), value),
        "xlinkHref" => {
            if callable || js::type_of(Some(value)) == "boolean" {
                return;
            }
            push_url_attribute(target, name, "xlink:href", value);
        }
        "contentEditable" | "spellCheck" | "draggable" | "value" | "autoReverse"
        | "externalResourcesRequired" | "focusable" | "preserveAlpha" => {
            // Booleanish String
            // These are "enumerated" attributes that accept "true" and "false".
            // In React, we let users pass `true` and `false` even though technically
            // these aren't boolean attributes (they are coerced to strings).
            if !callable {
                push_text_attribute(target, name, value);
            }
        }
        "inert" | "allowFullScreen" | "async" | "autoPlay" | "controls" | "credentialless" | "default"
        | "defer" | "disabled" | "disablePictureInPicture" | "disableRemotePlayback" | "formNoValidate"
        | "hidden" | "loop" | "noModule" | "noValidate" | "open" | "playsInline" | "readOnly"
        | "required" | "reversed" | "scoped" | "seamless" | "itemScope" => {
            if __DEV__
                && name == "inert"
                && js::strict_equals(Some(value), "")
                && dict::get(DID_WARN_FOR_NEW_BOOLEAN_PROPS_WITH_EMPTY_VALUE.with(|did_warn| *did_warn), name)
                    .is_none_or(|did_warn| !did_warn)
            {
                dict::set(DID_WARN_FOR_NEW_BOOLEAN_PROPS_WITH_EMPTY_VALUE.with(|did_warn| *did_warn), name, true);
                webapi::console::error((
                    concat!(
                        "Received an empty string for a boolean attribute `%s`. ",
                        "This will treat the attribute as if it were false. ",
                        "Either pass `false` to silence this warning, or ",
                        "pass `true` if you used an empty string in earlier versions of React to indicate this attribute is true."
                    ),
                    name,
                ));
            }
            // Boolean
            if js::truthy(Some(value)) && !callable {
                target.borrow_mut().extend([
                    chunk(&ATTRIBUTE_SEPARATOR),
                    string_to_chunk(name),
                    chunk(&ATTRIBUTE_EMPTY_STRING),
                ]);
            }
        }
        "capture" | "download" => {
            // Overloaded Boolean
            if js::strict_equals(Some(value), true) {
                target.borrow_mut().extend([
                    chunk(&ATTRIBUTE_SEPARATOR),
                    string_to_chunk(name),
                    chunk(&ATTRIBUTE_EMPTY_STRING),
                ]);
            } else if js::strict_equals(Some(value), false) {
                // Ignored
            } else if !callable {
                push_text_attribute(target, name, value);
            }
        }
        "cols" | "rows" | "size" | "span" => {
            // These are HTML attributes that must be positive numbers.
            if !callable
                && !js::is_nan(unsafe { js::cast(Some(value)) })
                && unsafe { js::cast::<f64>(Some(value)) } >= 1.0
            {
                push_text_attribute(target, name, value);
            }
        }
        "rowSpan" | "start" => {
            // These are HTML attributes that must be numbers.
            if !callable && !js::is_nan(unsafe { js::cast(Some(value)) }) {
                push_text_attribute(target, name, value);
            }
        }
        "xlinkActuate" => push_string_attribute(target, "xlink:actuate", value),
        "xlinkArcrole" => push_string_attribute(target, "xlink:arcrole", value),
        "xlinkRole" => push_string_attribute(target, "xlink:role", value),
        "xlinkShow" => push_string_attribute(target, "xlink:show", value),
        "xlinkTitle" => push_string_attribute(target, "xlink:title", value),
        "xlinkType" => push_string_attribute(target, "xlink:type", value),
        "xmlBase" => push_string_attribute(target, "xml:base", value),
        "xmlLang" => push_string_attribute(target, "xml:lang", value),
        "xmlSpace" => push_string_attribute(target, "xml:space", value),
        _ => {
            if
            // shouldIgnoreAttribute
            // We have already filtered out null/undefined and reserved words.
            js::string::length(name) > 2
                && (js::string::get(name, 0.0) == Some("o") || js::string::get(name, 0.0) == Some("O"))
                && (js::string::get(name, 1.0) == Some("n") || js::string::get(name, 1.0) == Some("N"))
            {
                return;
            }

            let attribute_name = get_attribute_alias(name);
            if is_attribute_name_safe(attribute_name) {
                // shouldRemoveAttribute
                match js::type_of(Some(value)).as_str() {
                    "function" | "symbol" => return,
                    "boolean" => {
                        let prefix = js::string::slice(&attribute_name.to_lowercase(), 0.0, Some(5.0));
                        if prefix != "data-" && prefix != "aria-" {
                            return;
                        }
                    }
                    _ => {}
                }
                push_text_attribute(target, attribute_name, value);
            }
        }
    }
}

#[cfg_attr(rust_js, rust_js::name = "pushInnerHTML")]
fn push_inner_html(
    target: &Chunks,
    inner_html: Option<&'static Unknown>,
    children: Option<&'static Unknown>,
) {
    if let Some(inner_html) = inner_html {
        if children.is_some() {
            panic!("Can only set one of `children` or `props.dangerouslySetInnerHTML`.");
        }

        if js::type_of(Some(inner_html)) != "object" || !js::has(inner_html, "__html") {
            panic!(
                "{}",
                concat!(
                    "`props.dangerouslySetInnerHTML` must be in the form `{__html: ...}`. ",
                    "Please visit https://react.dev/link/dangerously-set-inner-html ",
                    "for more information."
                )
            );
        }

        let html = js::get(inner_html, "__html");
        if html.is_some() {
            if __DEV__ {
                check_html_string_coercion(html);
            }
            target
                .borrow_mut()
                .push(string_to_chunk(&js::concat_text(html).unwrap()));
        }
    }
}

// TODO: Move these to RenderState so that we warn for every request.
// It would help debugging in stateful servers (e.g. service worker).
thread_local! {
    static DID_WARN_DEFAULT_INPUT_VALUE: Cell<bool> = const { Cell::new(false) };
    static DID_WARN_DEFAULT_CHECKED: Cell<bool> = const { Cell::new(false) };
    static DID_WARN_DEFAULT_SELECT_VALUE: Cell<bool> = const { Cell::new(false) };
    static DID_WARN_DEFAULT_TEXTAREA_VALUE: Cell<bool> = const { Cell::new(false) };
    static DID_WARN_INVALID_OPTION_CHILDREN: Cell<bool> = const { Cell::new(false) };
    #[cfg_attr(rust_js, rust_js::name = "didWarnInvalidOptionInnerHTML")]
    static DID_WARN_INVALID_OPTION_INNER_HTML: Cell<bool> = const { Cell::new(false) };
    static DID_WARN_SELECTED_SET_ON_OPTION: Cell<bool> = const { Cell::new(false) };
    static DID_WARN_FORM_ACTION_TYPE: Cell<bool> = const { Cell::new(false) };
    static DID_WARN_FORM_ACTION_NAME: Cell<bool> = const { Cell::new(false) };
    static DID_WARN_FORM_ACTION_TARGET: Cell<bool> = const { Cell::new(false) };
    static DID_WARN_FORM_ACTION_METHOD: Cell<bool> = const { Cell::new(false) };
}

fn check_select_prop(props: &Unknown, prop_name: &str) {
    if __DEV__ {
        let value = js::get(props, prop_name);
        if value.is_some() {
            let array = is_array(value);
            if js::truthy(js::get(props, "multiple")) && !array {
                webapi::console::error((
                    concat!(
                        "The `%s` prop supplied to <select> must be an array if ",
                        "`multiple` is true."
                    ),
                    prop_name,
                ));
            } else if !js::truthy(js::get(props, "multiple")) && array {
                webapi::console::error((
                    concat!(
                        "The `%s` prop supplied to <select> must be a scalar ",
                        "value if `multiple` is false."
                    ),
                    prop_name,
                ));
            }
        }
    }
}

fn push_start_anchor(
    target: &Chunks,
    props: &Unknown,
    format_context: &FormatContext,
) -> Option<&'static Unknown> {
    target.borrow_mut().push(start_chunk_for_tag("a"));

    let mut children = js::null();
    let mut inner_html = None;
    for prop_key in js::keys_in(props) {
        if HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(props, &prop_key)) {
            let Some(prop_value) = js::get(props, &prop_key) else {
                continue;
            };
            match prop_key.as_str() {
                "children" => children = Some(prop_value),
                "dangerouslySetInnerHTML" => inner_html = Some(prop_value),
                "href" => {
                    if js::strict_equals(Some(prop_value), "") {
                        // Empty `href` is special on anchors so we're short-circuiting here.
                        // On other tags it should trigger a warning
                        push_string_attribute(target, "href", js::unknown(""));
                    } else {
                        push_attribute(target, &prop_key, prop_value);
                    }
                }
                _ => push_attribute(target, &prop_key, prop_value),
            }
        }
    }

    push_view_transition_attributes(target, format_context);

    target.borrow_mut().push(chunk(&END_OF_START_TAG));
    push_inner_html(target, inner_html, children);
    if let Kind::String(children) = js::classify(children) {
        // Special case children as a string to avoid the unnecessary comment.
        // TODO: Remove this special case after the general optimization is in place.
        target
            .borrow_mut()
            .push(string_to_chunk(&encode_html_text_node(children)));
        return js::null();
    }
    children
}

fn push_start_object(
    target: &Chunks,
    props: &Unknown,
    format_context: &FormatContext,
) -> Option<&'static Unknown> {
    target.borrow_mut().push(start_chunk_for_tag("object"));

    let mut children = js::null();
    let mut inner_html = None;
    for prop_key in js::keys_in(props) {
        if HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(props, &prop_key)) {
            let Some(prop_value) = js::get(props, &prop_key) else {
                continue;
            };
            match prop_key.as_str() {
                "children" => children = Some(prop_value),
                "dangerouslySetInnerHTML" => inner_html = Some(prop_value),
                "data" => {
                    if __DEV__ {
                        check_attribute_string_coercion(Some(prop_value), "data");
                    }
                    let sanitized_value = sanitize_url(Some(js::unknown_of(
                        js::concat_text(Some(prop_value)).unwrap(),
                    )));
                    if js::strict_equals(sanitized_value, "") {
                        if __DEV__ {
                            webapi::console::error((
                                concat!(
                                    "An empty string (\"\") was passed to the %s attribute. ",
                                    "To fix this, either do not render the element at all ",
                                    "or pass null to %s instead of an empty string."
                                ),
                                prop_key.as_str(),
                                prop_key.as_str(),
                            ));
                        }
                        continue;
                    }
                    target.borrow_mut().extend([
                        chunk(&ATTRIBUTE_SEPARATOR),
                        string_to_chunk("data"),
                        chunk(&ATTRIBUTE_ASSIGN),
                        string_to_chunk(&escape_text_for_browser(sanitized_value)),
                        chunk(&ATTRIBUTE_END),
                    ]);
                }
                _ => push_attribute(target, &prop_key, prop_value),
            }
        }
    }

    push_view_transition_attributes(target, format_context);

    target.borrow_mut().push(chunk(&END_OF_START_TAG));
    push_inner_html(target, inner_html, children);
    if let Kind::String(children) = js::classify(children) {
        // Special case children as a string to avoid the unnecessary comment.
        // TODO: Remove this special case after the general optimization is in place.
        target
            .borrow_mut()
            .push(string_to_chunk(&encode_html_text_node(children)));
        return js::null();
    }
    children
}

fn push_start_select(
    target: &Chunks,
    props: &Unknown,
    format_context: &FormatContext,
) -> Option<&'static Unknown> {
    if __DEV__ {
        check_controlled_value_props("select", props);

        check_select_prop(props, "value");
        check_select_prop(props, "defaultValue");

        if js::get(props, "value").is_some()
            && js::get(props, "defaultValue").is_some()
            && !DID_WARN_DEFAULT_SELECT_VALUE.get()
        {
            webapi::console::error((concat!(
                "Select elements must be either controlled or uncontrolled ",
                "(specify either the value prop, or the defaultValue prop, but not ",
                "both). Decide between using a controlled or uncontrolled select ",
                "element and remove one of these props. More info: ",
                "https://react.dev/link/controlled-components"
            ),));
            DID_WARN_DEFAULT_SELECT_VALUE.set(true);
        }
    }

    target.borrow_mut().push(start_chunk_for_tag("select"));

    let mut children = js::null();
    let mut inner_html = None;
    for prop_key in js::keys_in(props) {
        if HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(props, &prop_key)) {
            let Some(prop_value) = js::get(props, &prop_key) else {
                continue;
            };
            match prop_key.as_str() {
                "children" => children = Some(prop_value),
                // TODO: This doesn't really make sense for select since it can't use the controlled
                // value in the innerHTML.
                "dangerouslySetInnerHTML" => inner_html = Some(prop_value),
                "defaultValue" | "value" => {
                    // These are set on the Context instead and applied to the nested options.
                }
                _ => push_attribute(target, &prop_key, prop_value),
            }
        }
    }

    push_view_transition_attributes(target, format_context);

    target.borrow_mut().push(chunk(&END_OF_START_TAG));
    push_inner_html(target, inner_html, children);
    children
}

fn flatten_option_children(children: Option<&'static Unknown>) -> String {
    let content = Rc::new(RefCell::new(String::new()));
    // Flatten children and warn if they aren't strings or numbers;
    // invalid types are ignored.
    let flattened = Rc::clone(&content);
    CHILDREN.with(|react_children| {
        (react_children.for_each)(
            children,
            &move |child| {
                if child.is_none() {
                    return;
                }
                let text = js::concat_text(child).unwrap();
                flattened.borrow_mut().push_str(&text);
                if __DEV__
                    && !DID_WARN_INVALID_OPTION_CHILDREN.get()
                    && js::type_of(child) != "string"
                    && js::type_of(child) != "number"
                    && js::type_of(child) != "bigint"
                {
                    DID_WARN_INVALID_OPTION_CHILDREN.set(true);
                    webapi::console::error((concat!(
                        "Cannot infer the option value of complex children. ",
                        "Pass a `value` prop or use a plain string as children to <option>."
                    ),));
                }
            },
            None,
        );
    });
    content.take()
}

thread_local! {
    static SELECTED_MARKER_ATTRIBUTE: &'static PrecomputedChunk = string_to_precomputed_chunk(" selected=\"\"");
}

fn push_start_option(
    target: &Chunks,
    props: &Unknown,
    format_context: &FormatContext,
) -> Option<&'static Unknown> {
    let selected_value = format_context.selected_value;

    target.borrow_mut().push(start_chunk_for_tag("option"));

    let mut children = js::null();
    let mut value = None;
    let mut selected = None;
    let mut inner_html = None;
    for prop_key in js::keys_in(props) {
        if HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(props, &prop_key)) {
            let Some(prop_value) = js::get(props, &prop_key) else {
                continue;
            };
            match prop_key.as_str() {
                "children" => children = Some(prop_value),
                "selected" => {
                    // ignore
                    selected = Some(prop_value);
                    // TODO: Remove support for `selected` in <option>.
                    if __DEV__ && !DID_WARN_SELECTED_SET_ON_OPTION.get() {
                        webapi::console::error((concat!(
                            "Use the `defaultValue` or `value` props on <select> instead of ",
                            "setting `selected` on <option>."
                        ),));
                        DID_WARN_SELECTED_SET_ON_OPTION.set(true);
                    }
                }
                "dangerouslySetInnerHTML" => inner_html = Some(prop_value),
                _ => {
                    if prop_key == "value" {
                        value = Some(prop_value);
                    }
                    // We intentionally fallthrough to also set the attribute on the node.
                    push_attribute(target, &prop_key, prop_value);
                }
            }
        }
    }

    if selected_value.is_some() {
        let string_value = if value.is_some() {
            if __DEV__ {
                check_attribute_string_coercion(value, "value");
            }
            js::concat_text(value).unwrap()
        } else {
            if __DEV__ && inner_html.is_some() && !DID_WARN_INVALID_OPTION_INNER_HTML.get() {
                DID_WARN_INVALID_OPTION_INNER_HTML.set(true);
                webapi::console::error((concat!(
                    "Pass a `value` prop if you set dangerouslyInnerHTML so React knows ",
                    "which value should be selected."
                ),));
            }
            flatten_option_children(children)
        };
        if is_array(selected_value) {
            // multiple
            let selected_values: &[Option<&Unknown>] = unsafe { js::cast(selected_value) };
            for &selected in selected_values {
                if __DEV__ {
                    check_attribute_string_coercion(selected, "value");
                }
                let v = js::concat_text(selected).unwrap();
                if v == string_value {
                    target.borrow_mut().push(chunk(&SELECTED_MARKER_ATTRIBUTE));
                    break;
                }
            }
        } else {
            if __DEV__ {
                check_attribute_string_coercion(selected_value, "select.value");
            }
            if js::concat_text(selected_value).unwrap() == string_value {
                target.borrow_mut().push(chunk(&SELECTED_MARKER_ATTRIBUTE));
            }
        }
    } else if js::truthy(selected) {
        target.borrow_mut().push(chunk(&SELECTED_MARKER_ATTRIBUTE));
    }

    // Options never participate as ViewTransitions.
    target.borrow_mut().push(chunk(&END_OF_START_TAG));
    push_inner_html(target, inner_html, children);
    children
}

thread_local! {
    static FORM_REPLAYING_RUNTIME_SCRIPT: &'static PrecomputedChunk =
        string_to_precomputed_chunk(FORM_REPLAYING_RUNTIME);
}

fn inject_form_replaying_runtime(resumable_state: &ResumableState, render_state: &RenderState) {
    // If we haven't sent it yet, inject the runtime that tracks submitted JS actions
    // for later replaying by Fiber. If we use an external runtime, we don't need
    // to emit anything. It's always used.
    if (resumable_state.instructions.get() & SENT_FORM_REPLAYING_RUNTIME) == NOTHING_SENT
        && (!ENABLE_FIZZ_EXTERNAL_RUNTIME.with(|enabled| *enabled)
            || render_state.external_runtime_script.is_none())
    {
        resumable_state
            .instructions
            .set(resumable_state.instructions.get() | SENT_FORM_REPLAYING_RUNTIME);
        let preamble = &render_state.preamble;
        let bootstrap_chunks = &render_state.bootstrap_chunks;
        if (preamble.html_chunks.borrow().is_some() || preamble.head_chunks.borrow().is_some())
            && bootstrap_chunks.borrow().is_empty()
        {
            // If we rendered the whole document, then we emitted a rel="expect" that needs a
            // matching target. If we haven't emitted that yet, we need to include it in this
            // script tag.
            bootstrap_chunks
                .borrow_mut()
                .push(render_state.start_inline_script);
            push_completed_shell_id_attribute(bootstrap_chunks, resumable_state);
            bootstrap_chunks.borrow_mut().extend([
                chunk(&END_OF_START_TAG),
                chunk(&FORM_REPLAYING_RUNTIME_SCRIPT),
                chunk(&END_INLINE_SCRIPT),
            ]);
        } else {
            // Otherwise we added to the beginning of the scripts. This will mean that it
            // appears before the shell ID unfortunately.
            bootstrap_chunks.borrow_mut().splice(
                0..0,
                [
                    render_state.start_inline_script,
                    chunk(&END_OF_START_TAG),
                    chunk(&FORM_REPLAYING_RUNTIME_SCRIPT),
                    chunk(&END_INLINE_SCRIPT),
                ],
            );
        }
    }
}

thread_local! {
    static FORM_STATE_MARKER_IS_MATCHING: &'static PrecomputedChunk = string_to_precomputed_chunk("<!--F!-->");
    static FORM_STATE_MARKER_IS_NOT_MATCHING: &'static PrecomputedChunk = string_to_precomputed_chunk("<!--F-->");
}

pub fn push_form_state_marker_is_matching(target: &Chunks) {
    target
        .borrow_mut()
        .push(chunk(&FORM_STATE_MARKER_IS_MATCHING));
}

pub fn push_form_state_marker_is_not_matching(target: &Chunks) {
    target
        .borrow_mut()
        .push(chunk(&FORM_STATE_MARKER_IS_NOT_MATCHING));
}

fn push_start_form(
    target: &Chunks,
    props: &Unknown,
    resumable_state: &ResumableState,
    render_state: &RenderState,
    format_context: &FormatContext,
) -> Option<&'static Unknown> {
    target.borrow_mut().push(start_chunk_for_tag("form"));

    let mut children = js::null();
    let mut inner_html = None;
    let mut form_action = None;
    let mut form_enc_type = None;
    let mut form_method = None;
    let mut form_target = None;

    for prop_key in js::keys_in(props) {
        if HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(props, &prop_key)) {
            let Some(prop_value) = js::get(props, &prop_key) else {
                continue;
            };
            match prop_key.as_str() {
                "children" => children = Some(prop_value),
                "dangerouslySetInnerHTML" => inner_html = Some(prop_value),
                "action" => form_action = Some(prop_value),
                "encType" => form_enc_type = Some(prop_value),
                "method" => form_method = Some(prop_value),
                "target" => form_target = Some(prop_value),
                _ => push_attribute(target, &prop_key, prop_value),
            }
        }
    }

    let mut form_data = None;
    let mut form_action_name = None;
    if let Some(action) = form_action
        && js::type_of(Some(action)) == "function"
    {
        // Function form actions cannot control the form properties
        if __DEV__ {
            if (form_enc_type.is_some() || form_method.is_some())
                && !DID_WARN_FORM_ACTION_METHOD.get()
            {
                DID_WARN_FORM_ACTION_METHOD.set(true);
                webapi::console::error((concat!(
                    "Cannot specify a encType or method for a form that specifies a ",
                    "function as the action. React provides those automatically. ",
                    "They will get overridden."
                ),));
            }
            if form_target.is_some() && !DID_WARN_FORM_ACTION_TARGET.get() {
                DID_WARN_FORM_ACTION_TARGET.set(true);
                webapi::console::error((concat!(
                    "Cannot specify a target for a form that specifies a function as the action. ",
                    "The function will always be executed in the same window."
                ),));
            }
        }
        if let Some(custom_fields) = get_custom_form_fields(resumable_state, action) {
            // This action has a custom progressive enhancement form that can submit the form
            // back to the server if it's invoked before hydration. Such as a Server Action.
            form_action = Some(js::unknown(
                custom_fields.action.as_deref().unwrap_or_default(),
            ));
            form_enc_type = custom_fields.enc_type.as_deref().map(js::unknown);
            form_method = custom_fields.method.as_deref().map(js::unknown);
            form_target = custom_fields.target.as_deref().map(js::unknown);
            form_data = custom_fields.data;
            form_action_name = custom_fields.name.as_deref();
        } else {
            // Set a javascript URL that doesn't do anything. We don't expect this to be invoked
            // because we'll preventDefault in the Fizz runtime, but it can happen if a form is
            // manually submitted or if someone calls stopPropagation before React gets the event.
            // If CSP is used to block javascript: URLs that's fine too. It just won't show this
            // error message but the URL will be logged.
            target.borrow_mut().extend([
                chunk(&ATTRIBUTE_SEPARATOR),
                string_to_chunk("action"),
                chunk(&ATTRIBUTE_ASSIGN),
                chunk(&ACTION_JAVA_SCRIPT_URL),
                chunk(&ATTRIBUTE_END),
            ]);
            form_action = None;
            form_enc_type = None;
            form_method = None;
            form_target = None;
            inject_form_replaying_runtime(resumable_state, render_state);
        }
    }
    if let Some(form_action) = form_action {
        push_attribute(target, "action", form_action);
    }
    if let Some(form_enc_type) = form_enc_type {
        push_attribute(target, "encType", form_enc_type);
    }
    if let Some(form_method) = form_method {
        push_attribute(target, "method", form_method);
    }
    if let Some(form_target) = form_target {
        push_attribute(target, "target", form_target);
    }

    push_view_transition_attributes(target, format_context);

    target.borrow_mut().push(chunk(&END_OF_START_TAG));

    if let Some(form_action_name) = form_action_name {
        target.borrow_mut().push(chunk(&START_HIDDEN_INPUT_CHUNK));
        push_string_attribute(target, "name", js::unknown(form_action_name));
        target
            .borrow_mut()
            .push(chunk(&END_OF_START_TAG_SELF_CLOSING));
        push_additional_form_fields(target, form_data);
    }

    push_inner_html(target, inner_html, children);
    if let Kind::String(children) = js::classify(children) {
        // Special case children as a string to avoid the unnecessary comment.
        // TODO: Remove this special case after the general optimization is in place.
        target
            .borrow_mut()
            .push(string_to_chunk(&encode_html_text_node(children)));
        return js::null();
    }
    children
}

fn push_input(
    target: &Chunks,
    props: &Unknown,
    resumable_state: &ResumableState,
    render_state: &RenderState,
    format_context: &FormatContext,
) -> Option<&'static Unknown> {
    if __DEV__ {
        check_controlled_value_props("input", props);
    }

    target.borrow_mut().push(start_chunk_for_tag("input"));

    let mut name = None;
    let mut form_action = None;
    let mut form_enc_type = None;
    let mut form_method = None;
    let mut form_target = None;
    let mut value = None;
    let mut default_value = None;
    let mut checked = None;
    let mut default_checked = None;

    for prop_key in js::keys_in(props) {
        if HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(props, &prop_key)) {
            let Some(prop_value) = js::get(props, &prop_key) else {
                continue;
            };
            match prop_key.as_str() {
                "children" | "dangerouslySetInnerHTML" => panic!(
                    "{} is a self-closing tag and must neither have `children` nor use `dangerouslySetInnerHTML`.",
                    "input"
                ),
                "name" => name = Some(prop_value),
                "formAction" => form_action = Some(prop_value),
                "formEncType" => form_enc_type = Some(prop_value),
                "formMethod" => form_method = Some(prop_value),
                "formTarget" => form_target = Some(prop_value),
                "defaultChecked" => default_checked = Some(prop_value),
                "defaultValue" => default_value = Some(prop_value),
                "checked" => checked = Some(prop_value),
                "value" => value = Some(prop_value),
                _ => push_attribute(target, &prop_key, prop_value),
            }
        }
    }

    if __DEV__
        && form_action.is_some()
        && !js::strict_equals(js::get(props, "type"), "image")
        && !js::strict_equals(js::get(props, "type"), "submit")
        && !DID_WARN_FORM_ACTION_TYPE.get()
    {
        DID_WARN_FORM_ACTION_TYPE.set(true);
        webapi::console::error((
            "An input can only specify a formAction along with type=\"submit\" or type=\"image\".",
        ));
    }

    let form_data = push_form_action_attribute(
        target,
        resumable_state,
        render_state,
        FormActionProps {
            form_action,
            form_enc_type,
            form_method,
            form_target,
            name,
        },
    );

    if __DEV__ {
        if checked.is_some() && default_checked.is_some() && !DID_WARN_DEFAULT_CHECKED.get() {
            webapi::console::error((
                concat!(
                    "%s contains an input of type %s with both checked and defaultChecked props. ",
                    "Input elements must be either controlled or uncontrolled ",
                    "(specify either the checked prop, or the defaultChecked prop, but not ",
                    "both). Decide between using a controlled or uncontrolled input ",
                    "element and remove one of these props. More info: ",
                    "https://react.dev/link/controlled-components"
                ),
                "A component",
                js::get(props, "type"),
            ));
            DID_WARN_DEFAULT_CHECKED.set(true);
        }
        if value.is_some() && default_value.is_some() && !DID_WARN_DEFAULT_INPUT_VALUE.get() {
            webapi::console::error((
                concat!(
                    "%s contains an input of type %s with both value and defaultValue props. ",
                    "Input elements must be either controlled or uncontrolled ",
                    "(specify either the value prop, or the defaultValue prop, but not ",
                    "both). Decide between using a controlled or uncontrolled input ",
                    "element and remove one of these props. More info: ",
                    "https://react.dev/link/controlled-components"
                ),
                "A component",
                js::get(props, "type"),
            ));
            DID_WARN_DEFAULT_INPUT_VALUE.set(true);
        }
    }

    if let Some(checked) = checked {
        push_boolean_attribute(target, "checked", checked);
    } else if let Some(default_checked) = default_checked {
        push_boolean_attribute(target, "checked", default_checked);
    }
    if let Some(value) = value {
        push_attribute(target, "value", value);
    } else if let Some(default_value) = default_value {
        push_attribute(target, "value", default_value);
    }

    push_view_transition_attributes(target, format_context);

    target
        .borrow_mut()
        .push(chunk(&END_OF_START_TAG_SELF_CLOSING));

    // We place any additional hidden form fields after the input.
    push_additional_form_fields(target, form_data);

    js::null()
}

fn push_start_button(
    target: &Chunks,
    props: &Unknown,
    resumable_state: &ResumableState,
    render_state: &RenderState,
    format_context: &FormatContext,
) -> Option<&'static Unknown> {
    target.borrow_mut().push(start_chunk_for_tag("button"));

    let mut children = js::null();
    let mut inner_html = None;
    let mut name = None;
    let mut form_action = None;
    let mut form_enc_type = None;
    let mut form_method = None;
    let mut form_target = None;

    for prop_key in js::keys_in(props) {
        if HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(props, &prop_key)) {
            let Some(prop_value) = js::get(props, &prop_key) else {
                continue;
            };
            match prop_key.as_str() {
                "children" => children = Some(prop_value),
                "dangerouslySetInnerHTML" => inner_html = Some(prop_value),
                "name" => name = Some(prop_value),
                "formAction" => form_action = Some(prop_value),
                "formEncType" => form_enc_type = Some(prop_value),
                "formMethod" => form_method = Some(prop_value),
                "formTarget" => form_target = Some(prop_value),
                _ => push_attribute(target, &prop_key, prop_value),
            }
        }
    }

    if __DEV__
        && form_action.is_some()
        && js::get(props, "type").is_some()
        && !js::strict_equals(js::get(props, "type"), "submit")
        && !DID_WARN_FORM_ACTION_TYPE.get()
    {
        DID_WARN_FORM_ACTION_TYPE.set(true);
        webapi::console::error((
            "A button can only specify a formAction along with type=\"submit\" or no type.",
        ));
    }

    let form_data = push_form_action_attribute(
        target,
        resumable_state,
        render_state,
        FormActionProps {
            form_action,
            form_enc_type,
            form_method,
            form_target,
            name,
        },
    );

    push_view_transition_attributes(target, format_context);

    target.borrow_mut().push(chunk(&END_OF_START_TAG));

    // We place any additional hidden form fields we need to include inside the button itself.
    push_additional_form_fields(target, form_data);

    push_inner_html(target, inner_html, children);
    if let Kind::String(children) = js::classify(children) {
        // Special case children as a string to avoid the unnecessary comment.
        // TODO: Remove this special case after the general optimization is in place.
        target
            .borrow_mut()
            .push(string_to_chunk(&encode_html_text_node(children)));
        return js::null();
    }

    children
}

fn push_start_text_area(
    target: &Chunks,
    props: &Unknown,
    format_context: &FormatContext,
) -> Option<&'static Unknown> {
    if __DEV__ {
        check_controlled_value_props("textarea", props);
        if js::get(props, "value").is_some()
            && js::get(props, "defaultValue").is_some()
            && !DID_WARN_DEFAULT_TEXTAREA_VALUE.get()
        {
            webapi::console::error((concat!(
                "Textarea elements must be either controlled or uncontrolled ",
                "(specify either the value prop, or the defaultValue prop, but not ",
                "both). Decide between using a controlled or uncontrolled textarea ",
                "and remove one of these props. More info: ",
                "https://react.dev/link/controlled-components"
            ),));
            DID_WARN_DEFAULT_TEXTAREA_VALUE.set(true);
        }
    }

    target.borrow_mut().push(start_chunk_for_tag("textarea"));

    let mut value = None;
    let mut default_value = None;
    let mut children = None;
    for prop_key in js::keys_in(props) {
        if HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(props, &prop_key)) {
            let Some(prop_value) = js::get(props, &prop_key) else {
                continue;
            };
            match prop_key.as_str() {
                "children" => children = Some(prop_value),
                "value" => value = Some(prop_value),
                "defaultValue" => default_value = Some(prop_value),
                "dangerouslySetInnerHTML" => {
                    panic!("`dangerouslySetInnerHTML` does not make sense on <textarea>.")
                }
                _ => push_attribute(target, &prop_key, prop_value),
            }
        }
    }
    if value.is_none() && default_value.is_some() {
        value = default_value;
    }

    push_view_transition_attributes(target, format_context);

    target.borrow_mut().push(chunk(&END_OF_START_TAG));

    // TODO (yungsters): Remove support for children content in <textarea>.
    if children.is_some() {
        if __DEV__ {
            webapi::console::error((concat!(
                "Use the `defaultValue` or `value` props instead of setting ",
                "children on <textarea>."
            ),));
        }

        if value.is_some() {
            panic!("If you supply `defaultValue` on a <textarea>, do not pass children.");
        }

        if is_array(children) {
            let items: &[Option<&Unknown>] = unsafe { js::cast(children) };
            if items.len() > 1 {
                panic!("<textarea> can only have at most one child.");
            }

            // TODO: remove the coercion and the DEV check below because it will
            // always be overwritten by the coercion several lines below it. #22309
            if __DEV__ {
                check_html_string_coercion(items.first().copied().flatten());
            }
            js::concat_text(items.first().copied().flatten()).unwrap();
        }
        if __DEV__ {
            check_html_string_coercion(children);
        }
        value = Some(js::unknown_of(js::concat_text(children).unwrap()));
    }

    if let Kind::String(text) = js::classify(value)
        && js::string::get(text, 0.0) == Some("\n")
    {
        // text/html ignores the first character in these tags if it's a newline
        // Prefer to break application/xml over text/html (for now) by adding
        // a newline specifically to get eaten by the parser. (Alternately for
        // textareas, replacing "^\n" with "\r\n" doesn't get eaten, and the first
        // \r is normalized out by HTMLTextAreaElement#value.)
        // See: <http://www.w3.org/TR/html-polyglot/#newlines-in-textarea-and-pre>
        // See: <http://www.w3.org/TR/html5/syntax.html#element-restrictions>
        // See: <http://www.w3.org/TR/html5/syntax.html#newlines>
        // See: Parsing of "textarea" "listing" and "pre" elements
        //  from <http://www.w3.org/TR/html5/syntax.html#parsing-main-inbody>
        target.borrow_mut().push(chunk(&LEADING_NEWLINE));
    }

    // ToString and push directly instead of recurse over children.
    // We don't really support complex children in the value anyway.
    // This also currently avoids a trailing comment node which breaks textarea.
    if value.is_some() {
        if __DEV__ {
            check_attribute_string_coercion(value, "value");
        }
        target
            .borrow_mut()
            .push(string_to_chunk(&encode_html_text_node(
                &js::concat_text(value).unwrap(),
            )));
    }

    js::null()
}

fn push_meta(
    target: &Chunks,
    props: &Unknown,
    render_state: &RenderState,
    text_embedded: bool,
    format_context: &FormatContext,
) -> Option<&'static Unknown> {
    let noscript_tag_in_scope = format_context.tag_scope & NOSCRIPT_SCOPE;
    let is_fallback = format_context.tag_scope & FALLBACK_SCOPE;
    if format_context.insertion_mode == SVG_MODE
        || noscript_tag_in_scope != 0
        || js::get(props, "itemProp").is_some()
    {
        push_self_closing(target, props, "meta", format_context)
    } else {
        if text_embedded {
            // This link follows text but we aren't writing a tag. while not as efficient as possible we need
            // to be safe and assume text will follow by inserting a textSeparator
            target.borrow_mut().push(chunk(&TEXT_SEPARATOR));
        }

        if is_fallback != 0 {
            // Hoistable Elements for fallbacks are simply omitted. we don't want to emit them early
            // because they are likely superceded by primary content and we want to avoid needing to clean
            // them up when the primary content is ready. They are never hydrated on the client anyway because
            // boundaries in fallback are awaited or client render, in either case there is never hydration
            js::null()
        } else if js::type_of(js::get(props, "charSet")) == "string" {
            // "charset" Should really be config and not picked up from tags however since this is
            // the only way to embed the tag today we flush it on a special queue on the Request so it
            // can go before everything else. Like viewport this means that the tag will escape it's
            // parent container.
            push_self_closing(&render_state.charset_chunks, props, "meta", format_context)
        } else if js::strict_equals(js::get(props, "name"), "viewport") {
            // "viewport" is flushed on the Request so it can go earlier that Float resources that
            // might be affected by it. This means it can escape the boundary it is rendered within.
            // This is a pragmatic solution to viewport being incredibly sensitive to document order
            // without requiring all hoistables to be flushed too early.
            push_self_closing(&render_state.viewport_chunks, props, "meta", format_context)
        } else {
            push_self_closing(
                &render_state.hoistable_chunks,
                props,
                "meta",
                format_context,
            )
        }
    }
}

/// Of a resource state, `undefined` of none, whether it's EXISTS, `null`.
fn exists(resources: &ResourceStates, key: &str) -> bool {
    js::unknown(resources).has_own_property(key) && js::get(resources, key).is_none()
}

fn push_link(
    target: &Chunks,
    props: &'static Unknown,
    resumable_state: &ResumableState,
    render_state: &RenderState,
    hoistable_state: Option<&HoistableState>,
    text_embedded: bool,
    format_context: &FormatContext,
) -> Option<&'static Unknown> {
    let noscript_tag_in_scope = format_context.tag_scope & NOSCRIPT_SCOPE;
    let is_fallback = format_context.tag_scope & FALLBACK_SCOPE;
    let rel = js::get(props, "rel");
    let href = js::get(props, "href");
    let precedence = js::get(props, "precedence");
    let (Kind::String(_), Kind::String(href)) = (js::classify(rel), js::classify(href)) else {
        return push_link_not_hoisted(target, props, href);
    };
    if format_context.insertion_mode == SVG_MODE
        || noscript_tag_in_scope != 0
        || js::get(props, "itemProp").is_some()
        || href.is_empty()
    {
        return push_link_not_hoisted(target, props, Some(js::unknown(href)));
    }

    if js::strict_equals(js::get(props, "rel"), "stylesheet") {
        // This <link> may hoistable as a Stylesheet Resource, otherwise it will emit in place
        let key = get_resource_key(href);
        let Kind::String(precedence) = js::classify(precedence) else {
            // This stylesheet is either not opted into Resource semantics or has conflicting properties which
            // disqualify it for such. We can still create a preload resource to help it load faster on the
            // client
            return push_link_impl(target, props);
        };
        if js::get(props, "disabled").is_some()
            || js::truthy(js::get(props, "onLoad"))
            || js::truthy(js::get(props, "onError"))
        {
            if __DEV__ {
                if js::get(props, "disabled").is_some() {
                    webapi::console::error((
                        "React encountered a `<link rel=\"stylesheet\" .../>` with a `precedence` prop and a `disabled` prop. The presence of the `disabled` prop indicates an intent to manage the stylesheet active state from your from your Component code and React will not hoist or deduplicate this stylesheet. If your intent was to have React hoist and deduplciate this stylesheet using the `precedence` prop remove the `disabled` prop, otherwise remove the `precedence` prop.",
                    ));
                } else if js::truthy(js::get(props, "onLoad"))
                    || js::truthy(js::get(props, "onError"))
                {
                    let prop_description = if js::truthy(js::get(props, "onLoad"))
                        && js::truthy(js::get(props, "onError"))
                    {
                        "`onLoad` and `onError` props"
                    } else if js::truthy(js::get(props, "onLoad")) {
                        "`onLoad` prop"
                    } else {
                        "`onError` prop"
                    };
                    webapi::console::error((
                        "React encountered a `<link rel=\"stylesheet\" .../>` with a `precedence` prop and %s. The presence of loading and error handlers indicates an intent to manage the stylesheet loading state from your from your Component code and React will not hoist or deduplicate this stylesheet. If your intent was to have React hoist and deduplciate this stylesheet using the `precedence` prop remove the %s, otherwise remove the `precedence` prop.",
                        prop_description,
                        prop_description,
                    ));
                }
            }
            return push_link_impl(target, props);
        }
        // This stylesheet refers to a Resource and we create a new one if necessary
        let mut style_queue = render_state.styles.get(precedence);
        let style_resources = resumable_state.style_resources.get();
        let has_key = js::unknown(style_resources).has_own_property(&key);
        let resource_state = if has_key {
            js::get(style_resources, &key)
        } else {
            None
        };
        if !exists(style_resources, &key) {
            // We are going to create this resource now so it is marked as Exists
            dict::set(style_resources, &key, EXISTS.with(|exists| *exists));

            // If this is the first time we've encountered this precedence we need
            // to create a StyleQueue
            let queue = match style_queue {
                Some(queue) => queue,
                None => {
                    let queue = Rc::new(StyleQueue {
                        precedence: string_to_chunk(&escape_text_for_browser(Some(js::unknown(
                            precedence,
                        )))),
                        rules: RefCell::new(Vec::new()),
                        hrefs: RefCell::new(Vec::new()),
                        sheets: js::map::new(None),
                    });
                    render_state.styles.set(precedence, Rc::clone(&queue));
                    queue
                }
            };

            let resource = Rc::new(StylesheetResource {
                state: Cell::new(PENDING),
                props: stylesheet_props_from_raw_props(props),
            });

            if let Some(preload_state) = resource_state {
                // When resourceState is truty it is a Preload state. We cast it for clarity
                let preload_state: &[Option<&'static Unknown>] =
                    unsafe { js::cast(Some(preload_state)) };
                if preload_state.len() == 2 {
                    adopt_preload_credentials(resource.props, preload_state);
                }

                let preload_resource = render_state.preloads.stylesheets.get(&key);
                if let Some(preload_resource) = preload_resource
                    && !preload_resource.borrow().is_empty()
                {
                    // The Preload for this resource was created in this render pass and has not flushed yet so
                    // we need to clear it to avoid it flushing.
                    preload_resource.borrow_mut().clear();
                } else {
                    // Either the preload resource from this render already flushed in this render pass
                    // or the preload flushed in a prior pass (prerender). In either case we need to mark
                    // this resource as already having been preloaded.
                    resource.state.set(PRELOADED);
                }
            } else {
                // We don't need to check whether a preloadResource exists in the renderState
                // because if it did exist then the resourceState would also exist and we would
                // have hit the primary if condition above.
            }

            // We add the newly created resource to our StyleQueue and if necessary
            // track the resource with the currently rendering boundary
            queue.sheets.set(&key, Rc::clone(&resource));
            if let Some(hoistable_state) = hoistable_state {
                hoistable_state.stylesheets.add(&resource);
            }
            style_queue = Some(queue);
        } else {
            // We need to track whether this boundary should wait on this resource or not.
            // Typically this resource should always exist since we either had it or just created
            // it. However, it's possible when you resume that the style has already been emitted
            // and then it wouldn't be recreated in the RenderState and there's no need to track
            // it again since we should've hoisted it to the shell already.
            if let Some(style_queue) = &style_queue
                && let Some(resource) = style_queue.sheets.get(&key)
                && let Some(hoistable_state) = hoistable_state
            {
                hoistable_state.stylesheets.add(&resource);
            }
        }
        let _ = style_queue;
        if text_embedded {
            // This link follows text but we aren't writing a tag. while not as efficient as possible we need
            // to be safe and assume text will follow by inserting a textSeparator
            target.borrow_mut().push(chunk(&TEXT_SEPARATOR));
        }
        js::null()
    } else if js::truthy(js::get(props, "onLoad")) || js::truthy(js::get(props, "onError")) {
        // When using load handlers we cannot hoist and need to emit links in place
        push_link_impl(target, props)
    } else {
        // We can hoist this link so we may need to emit a text separator.
        // @TODO refactor text separators so we don't have to defensively add
        // them when we don't end up emitting a tag as a result of pushStartInstance
        if text_embedded {
            // This link follows text but we aren't writing a tag. while not as efficient as possible we need
            // to be safe and assume text will follow by inserting a textSeparator
            target.borrow_mut().push(chunk(&TEXT_SEPARATOR));
        }

        if is_fallback != 0 {
            // Hoistable Elements for fallbacks are simply omitted. we don't want to emit them early
            // because they are likely superceded by primary content and we want to avoid needing to clean
            // them up when the primary content is ready. They are never hydrated on the client anyway because
            // boundaries in fallback are awaited or client render, in either case there is never hydration
            js::null()
        } else {
            push_link_impl(&render_state.hoistable_chunks, props)
        }
    }
}

/// A `<link>` written in place, as one that can't be hoisted is.
fn push_link_not_hoisted(
    target: &Chunks,
    props: &Unknown,
    href: Option<&Unknown>,
) -> Option<&'static Unknown> {
    if __DEV__
        && js::strict_equals(js::get(props, "rel"), "stylesheet")
        && js::type_of(js::get(props, "precedence")) == "string"
        && (js::type_of(href) != "string" || !js::truthy(href))
    {
        webapi::console::error((
            "React encountered a `<link rel=\"stylesheet\" .../>` with a `precedence` prop and expected the `href` prop to be a non-empty string but ecountered %s instead. If your intent was to have React hoist and deduplciate this stylesheet using the `precedence` prop ensure there is a non-empty string `href` prop as well, otherwise remove the `precedence` prop.",
            get_value_descriptor_expecting_object_for_warning(href),
        ));
    }
    push_link_impl(target, props);
    js::null()
}

fn push_link_impl(target: &Chunks, props: &Unknown) -> Option<&'static Unknown> {
    target.borrow_mut().push(start_chunk_for_tag("link"));

    for prop_key in js::keys_in(props) {
        if HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(props, &prop_key)) {
            let Some(prop_value) = js::get(props, &prop_key) else {
                continue;
            };
            match prop_key.as_str() {
                "children" | "dangerouslySetInnerHTML" => panic!(
                    "{} is a self-closing tag and must neither have `children` nor use `dangerouslySetInnerHTML`.",
                    "link"
                ),
                _ => push_attribute(target, &prop_key, prop_value),
            }
        }
    }

    // Link never participate as a ViewTransition

    target
        .borrow_mut()
        .push(chunk(&END_OF_START_TAG_SELF_CLOSING));
    js::null()
}

/// A `<style>`'s one child: of an array, its first if it has at most one,
/// else none.
fn style_child(children: Option<&'static Unknown>) -> Option<&'static Unknown> {
    if js::array::is_array(children) {
        let items: &[Option<&'static Unknown>] = unsafe { js::cast(children) };
        if items.len() < 2 {
            items.first().copied().flatten()
        } else {
            js::null()
        }
    } else {
        children
    }
}

fn push_style(
    target: &Chunks,
    props: &'static Unknown,
    resumable_state: &ResumableState,
    render_state: &RenderState,
    hoistable_state: Option<&HoistableState>,
    text_embedded: bool,
    format_context: &FormatContext,
) -> Option<&'static Unknown> {
    let noscript_tag_in_scope = format_context.tag_scope & NOSCRIPT_SCOPE;
    if __DEV__ && HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(props, "children"))
    {
        let child = style_child(js::get(props, "children"));

        if matches!(js::classify(child), Kind::Function(_) | Kind::Symbol(_))
            || js::array::is_array(child)
        {
            let child_type = match js::classify(child) {
                Kind::Function(_) => "a Function",
                Kind::Symbol(_) => "a Sybmol",
                _ => "an Array",
            };
            webapi::console::error((
                concat!(
                    "React expect children of <style> tags to be a string, number, or object with a `toString` method but found %s instead. ",
                    "In browsers style Elements can only have `Text` Nodes as children."
                ),
                child_type,
            ));
        }
    }
    let precedence = js::get(props, "precedence");
    let href = js::get(props, "href");
    let nonce = js::get(props, "nonce");

    let (Kind::String(precedence), Kind::String(href)) =
        (js::classify(precedence), js::classify(href))
    else {
        // This style tag is not able to be turned into a Style Resource
        return push_style_impl(target, props);
    };
    if format_context.insertion_mode == SVG_MODE
        || noscript_tag_in_scope != 0
        || js::get(props, "itemProp").is_some()
        || href.is_empty()
    {
        // This style tag is not able to be turned into a Style Resource
        return push_style_impl(target, props);
    }

    if __DEV__ && href.contains(' ') {
        webapi::console::error((
            "React expected the `href` prop for a <style> tag opting into hoisting semantics using the `precedence` prop to not have any spaces but ecountered spaces instead. using spaces in this prop will cause hydration of this style to fail on the client. The href for the <style> where this ocurred is \"%s\".",
            href,
        ));
    }

    let key = get_resource_key(href);
    let mut style_queue = render_state.styles.get(precedence);
    let style_resources = resumable_state.style_resources.get();
    let has_key = js::unknown(style_resources).has_own_property(&key);
    let resource_state = if has_key {
        js::get(style_resources, &key)
    } else {
        None
    };
    if !exists(style_resources, &key) {
        // We are going to create this resource now so it is marked as Exists
        dict::set(style_resources, &key, EXISTS.with(|exists| *exists));

        if __DEV__ && js::truthy(resource_state) {
            webapi::console::error((
                "React encountered a hoistable style tag for the same href as a preload: \"%s\". When using a style tag to inline styles you should not also preload it as a stylsheet.",
                href,
            ));
        }

        let queue = match style_queue {
            Some(queue) => queue,
            None => {
                // This is the first time we've encountered this precedence we need
                // to create a StyleQueue.
                let queue = Rc::new(StyleQueue {
                    precedence: string_to_chunk(&escape_text_for_browser(Some(js::unknown(
                        precedence,
                    )))),
                    rules: RefCell::new(Vec::new()),
                    hrefs: RefCell::new(Vec::new()),
                    sheets: js::map::new(None),
                });
                render_state.styles.set(precedence, Rc::clone(&queue));
                queue
            }
        };

        let nonce_style = render_state.nonce.style.as_deref();
        if nonce_style.is_none_or(str::is_empty)
            || js::strict_equals(nonce, nonce_style.unwrap_or_default())
        {
            if __DEV__ && nonce_style.is_none_or(str::is_empty) && js::truthy(nonce) {
                webapi::console::error((
                    "React encountered a style tag with `precedence` \"%s\" and `nonce` \"%s\". When React manages style rules using `precedence` it will only include a nonce attributes if you also provide the same style nonce value as a render option.",
                    precedence,
                    nonce,
                ));
            }
            queue
                .hrefs
                .borrow_mut()
                .push(string_to_chunk(&escape_text_for_browser(Some(
                    js::unknown(href),
                ))));
            push_style_contents(&queue.rules, props);
        } else if __DEV__ {
            webapi::console::error((
                "React encountered a style tag with `precedence` \"%s\" and `nonce` \"%s\". When React manages style rules using `precedence` it will only include rules if the nonce matches the style nonce \"%s\" that was included with this render.",
                precedence,
                nonce,
                nonce_style,
            ));
        }
        style_queue = Some(queue);
    }
    if let Some(style_queue) = &style_queue {
        // We need to track whether this boundary should wait on this resource or not.
        // Typically this resource should always exist since we either had it or just created
        // it. However, it's possible when you resume that the style has already been emitted
        // and then it wouldn't be recreated in the RenderState and there's no need to track
        // it again since we should've hoisted it to the shell already.
        if let Some(hoistable_state) = hoistable_state {
            hoistable_state.styles.add(style_queue);
        }
    }

    if text_embedded {
        // This link follows text but we aren't writing a tag. while not as efficient as possible we need
        // to be safe and assume text will follow by inserting a textSeparator
        target.borrow_mut().push(chunk(&TEXT_SEPARATOR));
    }
    None
}

/// This escaping function is designed to work with style tag textContent only.
///
/// While untrusted style content should be made safe before using this api it will
/// ensure that the style cannot be early terminated or never terminated state
fn escape_style_text_content(style_text: Option<&Unknown>) -> String {
    if __DEV__ {
        check_html_string_coercion(style_text);
    }
    STYLE_REGEX.with(|style_regex| {
        js::reg_exp::replace(
            &js::concat_text(style_text).unwrap(),
            style_regex,
            style_replacer,
        )
    })
}

thread_local! {
    static STYLE_REGEX: &'static js::RegExp = js::reg_exp::new("(<\\/|<)(s)(tyle)", "gi");
}

fn style_replacer(
    _match: &str,
    prefix: Option<&str>,
    s: Option<&str>,
    suffix: Option<&str>,
) -> String {
    format!(
        "{}{}{}",
        prefix.unwrap_or_default(),
        if s == Some("s") { "\\73 " } else { "\\53 " },
        suffix.unwrap_or_default()
    )
}

/// The text of a `<style>`'s child, where it has one that's text.
fn push_style_child(target: &Chunks, children: Option<&'static Unknown>) {
    let child = style_child(children);
    if !matches!(js::classify(child), Kind::Function(_) | Kind::Symbol(_)) && child.is_some() {
        target
            .borrow_mut()
            .push(string_to_chunk(&escape_style_text_content(child)));
    }
}

fn push_style_impl(target: &Chunks, props: &Unknown) -> Option<&'static Unknown> {
    target.borrow_mut().push(start_chunk_for_tag("style"));

    let mut children = None;
    let mut inner_html = None;
    for prop_key in js::keys_in(props) {
        if HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(props, &prop_key)) {
            let Some(prop_value) = js::get(props, &prop_key) else {
                continue;
            };
            match prop_key.as_str() {
                "children" => children = Some(prop_value),
                "dangerouslySetInnerHTML" => inner_html = Some(prop_value),
                _ => push_attribute(target, &prop_key, prop_value),
            }
        }
    }

    // Style never participate as a ViewTransition.
    target.borrow_mut().push(chunk(&END_OF_START_TAG));

    push_style_child(target, children);
    push_inner_html(target, inner_html, children);
    target.borrow_mut().push(end_chunk_for_tag("style"));
    js::null()
}

fn push_style_contents(target: &Chunks, props: &Unknown) {
    let mut children = None;
    let mut inner_html = None;
    for prop_key in js::keys_in(props) {
        if HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(props, &prop_key)) {
            let Some(prop_value) = js::get(props, &prop_key) else {
                continue;
            };
            match prop_key.as_str() {
                "children" => children = Some(prop_value),
                "dangerouslySetInnerHTML" => inner_html = Some(prop_value),
                _ => {}
            }
        }
    }

    push_style_child(target, children);
    push_inner_html(target, inner_html, children);
}

/// Whether `value` is a data URI: text whose first five characters are
/// `data:`, of any case.
fn is_data_uri(value: Option<&Unknown>) -> bool {
    if let Kind::String(value) = js::classify(value) {
        js::string::get(value, 4.0) == Some(":")
            && (js::string::get(value, 0.0) == Some("d")
                || js::string::get(value, 0.0) == Some("D"))
            && (js::string::get(value, 1.0) == Some("a")
                || js::string::get(value, 1.0) == Some("A"))
            && (js::string::get(value, 2.0) == Some("t")
                || js::string::get(value, 2.0) == Some("T"))
            && (js::string::get(value, 3.0) == Some("a")
                || js::string::get(value, 3.0) == Some("A"))
    } else {
        false
    }
}

/// What an image preload's header is made of.
struct ImageHeaderParams<'a> {
    image_src_set: Option<&'a Unknown>,
    image_sizes: Option<&'a Unknown>,
    cross_origin: Option<&'a CrossOriginString>,
    integrity: Option<&'a Unknown>,
    nonce: Option<&'a Unknown>,
    r#type: Option<&'a Unknown>,
    fetch_priority: Option<&'a Unknown>,
    referrer_policy: Option<&'a Unknown>,
}

unsafe impl js::Defined for ImageHeaderParams<'_> {}

/// What an image's preload `<link>` is written of.
struct ImagePreloadProps<'a> {
    rel: &'a str,
    r#as: &'a str,
    // There is a bug in Safari where imageSrcSet is not respected on preload links
    // so we omit the href here if we have imageSrcSet b/c safari will load the wrong image.
    // This harms older browers that do not support imageSrcSet by making their preloads not work
    // but this population is shrinking fast and is already small so we accept this tradeoff.
    href: Option<&'a Unknown>,
    image_src_set: Option<&'a Unknown>,
    image_sizes: Option<&'a Unknown>,
    cross_origin: Option<&'a CrossOriginString>,
    integrity: Option<&'a Unknown>,
    r#type: Option<&'a Unknown>,
    fetch_priority: Option<&'a Unknown>,
    referrer_policy: Option<&'a Unknown>,
}

unsafe impl js::Defined for ImagePreloadProps<'_> {}

fn push_img(
    target: &Chunks,
    props: &'static Unknown,
    resumable_state: &ResumableState,
    render_state: &RenderState,
    hoistable_state: Option<&HoistableState>,
    format_context: &FormatContext,
) -> Option<&'static Unknown> {
    let picture_or_no_script_tag_in_scope =
        format_context.tag_scope & (PICTURE_SCOPE | NOSCRIPT_SCOPE);
    let src = js::get(props, "src");
    let src_set = js::get(props, "srcSet");
    if !js::strict_equals(js::get(props, "loading"), "lazy")
        && (js::truthy(src) || js::truthy(src_set))
        && (js::type_of(src) == "string" || src.is_none())
        && (js::type_of(src_set) == "string" || src_set.is_none())
        && !js::strict_equals(js::get(props, "fetchPriority"), "low")
        && picture_or_no_script_tag_in_scope == 0
        // We exclude data URIs in src and srcSet since these should not be preloaded
        && !is_data_uri(src)
        && !is_data_uri(src_set)
    {
        // We have a suspensey image and ought to preload it to optimize the loading of display blocking
        // resumableState.

        if let Some(hoistable_state) = hoistable_state {
            // Mark this boundary's state as having suspensey images.
            // Only do that if we have a ViewTransition that might trigger a parent Suspense boundary
            // to animate its appearing. Since that's the only case we'd actually apply suspensey images
            // for SSR reveals.
            let is_in_suspense_with_enter_view_transition =
                format_context.tag_scope & APPEARING_SCOPE;
            if is_in_suspense_with_enter_view_transition != 0 {
                hoistable_state.suspensey_images.set(true);
            }
        }

        let sizes = match js::classify(js::get(props, "sizes")) {
            Kind::String(_) => js::get(props, "sizes"),
            _ => None,
        };
        let key = get_image_resource_key(src, src_set, sizes);

        let promotable_preloads = render_state.preloads.images;

        if let Some(resource) = promotable_preloads.get(&key) {
            // We consider whether this preload can be promoted to higher priority flushing queue.
            // The only time a resource will exist here is if it was created during this render
            // and was not already in the high priority queue.
            if js::strict_equals(js::get(props, "fetchPriority"), "high")
                || render_state.high_image_preloads.size() < 10
            {
                // Delete the resource from the map since we are promoting it and don't want to
                // reenter this branch in a second pass for duplicate img hrefs.
                promotable_preloads.delete(&key);

                render_state.high_image_preloads.add(&resource);
            }
        } else if !js::unknown(resumable_state.image_resources.get()).has_own_property(&key) {
            // We must construct a new preload resource
            dict::set(
                resumable_state.image_resources.get(),
                &key,
                Some(PRELOAD_NO_CREDS.with(|preload| *preload)),
            );
            let cross_origin = get_cross_origin_string(js::get(props, "crossOrigin"));

            let headers = render_state.headers.borrow().clone();
            let header = match headers.as_deref() {
                Some(headers)
                    if headers.remaining_capacity.get() > 0.0
                        // browsers today don't support preloading responsive images from link headers so we bail out
                        // if the img has srcset defined
                        && js::type_of(js::get(props, "srcSet")) != "string"
                        // this is a hueristic similar to capping element preloads to 10 unless explicitly
                        // fetchPriority="high". We use length here which means it will fit fewer images when
                        // the urls are long and more when short. arguably byte size is a better hueristic because
                        // it directly translates to how much we send down before content is actually seen.
                        // We could unify the counts and also make it so the total is tracked regardless of
                        // flushing output but since the headers are likely to be go earlier than content
                        // they don't really conflict so for now I've kept them separate
                        && (js::strict_equals(js::get(props, "fetchPriority"), "high")
                            || js::string::length(&headers.high_image_preloads.borrow()) < 500) =>
                {
                    // We manually construct the options for the preload only from strings. We don't want to pollute
                    // the params list with arbitrary props and if we copied everything over as it we might get
                    // coercion errors. We have checks for this in Dev but it seems safer to just only accept values
                    // that are strings
                    let header = get_preload_as_header(
                        src,
                        "image",
                        Some(js::unknown(&ImageHeaderParams {
                            image_src_set: js::get(props, "srcSet"),
                            image_sizes: js::get(props, "sizes"),
                            cross_origin,
                            integrity: js::get(props, "integrity"),
                            nonce: js::get(props, "nonce"),
                            r#type: js::get(props, "type"),
                            fetch_priority: js::get(props, "fetchPriority"),
                            referrer_policy: js::get(props, "referrerPolicy"),
                        })),
                    );
                    // We always consume the header length since once we find one header that doesn't fit
                    // we assume all the rest won't as well. This is to avoid getting into a situation
                    // where we have a very small remaining capacity but no headers will ever fit and we end
                    // up constantly trying to see if the next resource might make it. In the future we can
                    // make this behavior different between render and prerender since in the latter case
                    // we are less sensitive to the current requests runtime per and more sensitive to maximizing
                    // headers.
                    let remaining = headers.remaining_capacity.get()
                        - (f64::from(js::string::length(&header)) + 2.0);
                    headers.remaining_capacity.set(remaining);
                    (remaining >= 0.0).then_some((headers, header))
                }
                _ => None,
            };
            if let Some((headers, header)) = header {
                // If we postpone in the shell we will still emit this preload so we track
                // it to make sure we don't reset it.
                dict::set(
                    render_state.resets.image,
                    &key,
                    Some(PRELOAD_NO_CREDS.with(|preload| *preload)),
                );
                if !headers.high_image_preloads.borrow().is_empty() {
                    headers.high_image_preloads.borrow_mut().push_str(", ");
                }
                headers.high_image_preloads.borrow_mut().push_str(&header);
            } else {
                let resource = Rc::new(RefCell::new(Vec::new()));
                push_link_impl(
                    &resource,
                    js::unknown(&ImagePreloadProps {
                        rel: "preload",
                        r#as: "image",
                        href: if js::truthy(src_set) { None } else { src },
                        image_src_set: src_set,
                        image_sizes: sizes,
                        cross_origin,
                        integrity: js::get(props, "integrity"),
                        r#type: js::get(props, "type"),
                        fetch_priority: js::get(props, "fetchPriority"),
                        referrer_policy: js::get(props, "referrerPolicy"),
                    }),
                );
                if js::strict_equals(js::get(props, "fetchPriority"), "high")
                    || render_state.high_image_preloads.size() < 10
                {
                    render_state.high_image_preloads.add(&resource);
                } else {
                    render_state.bulk_preloads.add(&resource);
                    // We can bump the priority up if the same img is rendered later
                    // with fetchPriority="high"
                    promotable_preloads.set(&key, resource);
                }
            }
        }
    }
    push_self_closing(target, props, "img", format_context)
}

fn push_self_closing(
    target: &Chunks,
    props: &Unknown,
    tag: &str,
    format_context: &FormatContext,
) -> Option<&'static Unknown> {
    target.borrow_mut().push(start_chunk_for_tag(tag));

    for prop_key in js::keys_in(props) {
        if HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(props, &prop_key)) {
            let Some(prop_value) = js::get(props, &prop_key) else {
                continue;
            };
            match prop_key.as_str() {
                "children" | "dangerouslySetInnerHTML" => panic!(
                    "{tag} is a self-closing tag and must neither have `children` nor use `dangerouslySetInnerHTML`."
                ),
                _ => push_attribute(target, &prop_key, prop_value),
            }
        }
    }

    push_view_transition_attributes(target, format_context);

    target
        .borrow_mut()
        .push(chunk(&END_OF_START_TAG_SELF_CLOSING));
    js::null()
}

fn push_start_menu_item(
    target: &Chunks,
    props: &Unknown,
    format_context: &FormatContext,
) -> Option<&'static Unknown> {
    target.borrow_mut().push(start_chunk_for_tag("menuitem"));

    for prop_key in js::keys_in(props) {
        if HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(props, &prop_key)) {
            let Some(prop_value) = js::get(props, &prop_key) else {
                continue;
            };
            match prop_key.as_str() {
                "children" | "dangerouslySetInnerHTML" => {
                    panic!("menuitems cannot have `children` nor `dangerouslySetInnerHTML`.")
                }
                _ => push_attribute(target, &prop_key, prop_value),
            }
        }
    }

    push_view_transition_attributes(target, format_context);

    target.borrow_mut().push(chunk(&END_OF_START_TAG));
    js::null()
}

fn push_title(
    target: &Chunks,
    props: &Unknown,
    render_state: &RenderState,
    format_context: &FormatContext,
) -> Option<&'static Unknown> {
    let noscript_tag_in_scope = format_context.tag_scope & NOSCRIPT_SCOPE;
    let is_fallback = format_context.tag_scope & FALLBACK_SCOPE;
    if __DEV__ && HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(props, "children"))
    {
        let children = js::get(props, "children");

        let child = style_child(children);

        if js::array::is_array(children)
            && unsafe { js::cast::<&[Option<&Unknown>]>(children) }.len() > 1
        {
            webapi::console::error((
                concat!(
                    "React expects the `children` prop of <title> tags to be a string, number, bigint, or object with a novel `toString` method but found an Array with length %s instead.",
                    " Browsers treat all child Nodes of <title> tags as Text content and React expects to be able to convert `children` of <title> tags to a single string value",
                    " which is why Arrays of length greater than 1 are not supported. When using JSX it can be common to combine text nodes and value nodes.",
                    " For example: <title>hello {nameOfUser}</title>. While not immediately apparent, `children` in this case is an Array with length 2. If your `children` prop",
                    " is using this form try rewriting it using a template string: <title>{`hello ${nameOfUser}`}</title>."
                ),
                js::get(children, "length"),
            ));
        } else if matches!(js::classify(child), Kind::Function(_) | Kind::Symbol(_)) {
            let child_type = if js::type_of(child) == "function" {
                "a Function"
            } else {
                "a Sybmol"
            };
            webapi::console::error((
                concat!(
                    "React expect children of <title> tags to be a string, number, bigint, or object with a novel `toString` method but found %s instead.",
                    " Browsers treat all child Nodes of <title> tags as Text content and React expects to be able to convert children of <title>",
                    " tags to a single string value."
                ),
                child_type,
            ));
        } else if js::truthy(child)
            && js::strict_equals(
                js::get(child, "toString"),
                js::get(dict::new::<bool>(), "toString"),
            )
        {
            if js::get(child, "$$typeof").is_some() {
                webapi::console::error((concat!(
                    "React expects the `children` prop of <title> tags to be a string, number, bigint, or object with a novel `toString` method but found an object that appears to be",
                    " a React element which never implements a suitable `toString` method. Browsers treat all child Nodes of <title> tags as Text content and React expects to",
                    " be able to convert children of <title> tags to a single string value which is why rendering React elements is not supported. If the `children` of <title> is",
                    " a React Component try moving the <title> tag into that component. If the `children` of <title> is some HTML markup change it to be Text only to be valid HTML."
                ),));
            } else {
                webapi::console::error((concat!(
                    "React expects the `children` prop of <title> tags to be a string, number, bigint, or object with a novel `toString` method but found an object that does not implement",
                    " a suitable `toString` method. Browsers treat all child Nodes of <title> tags as Text content and React expects to be able to convert children of <title> tags",
                    " to a single string value. Using the default `toString` method available on every object is almost certainly an error. Consider whether the `children` of this <title>",
                    " is an object in error and change it to a string or number value if so. Otherwise implement a `toString` method that React can use to produce a valid <title>."
                ),));
            }
        }
    }

    if format_context.insertion_mode != SVG_MODE
        && noscript_tag_in_scope == 0
        && js::get(props, "itemProp").is_none()
    {
        if is_fallback != 0 {
            // Hoistable Elements for fallbacks are simply omitted. we don't want to emit them early
            // because they are likely superceded by primary content and we want to avoid needing to clean
            // them up when the primary content is ready. They are never hydrated on the client anyway because
            // boundaries in fallback are awaited or client render, in either case there is never hydration
            js::null()
        } else {
            push_title_impl(&render_state.hoistable_chunks, props);
            None
        }
    } else {
        push_title_impl(target, props)
    }
}

fn push_title_impl(target: &Chunks, props: &Unknown) -> Option<&'static Unknown> {
    target.borrow_mut().push(start_chunk_for_tag("title"));

    let mut children = None;
    let mut inner_html = None;
    for prop_key in js::keys_in(props) {
        if HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(props, &prop_key)) {
            let Some(prop_value) = js::get(props, &prop_key) else {
                continue;
            };
            match prop_key.as_str() {
                "children" => children = Some(prop_value),
                "dangerouslySetInnerHTML" => inner_html = Some(prop_value),
                _ => push_attribute(target, &prop_key, prop_value),
            }
        }
    }
    // Title never participate as a ViewTransition
    target.borrow_mut().push(chunk(&END_OF_START_TAG));

    let child = style_child(children);
    if !matches!(js::classify(child), Kind::Function(_) | Kind::Symbol(_)) && child.is_some() {
        target
            .borrow_mut()
            .push(string_to_chunk(&escape_text_for_browser(Some(
                js::unknown(js::concat_text(child).unwrap().as_str()),
            ))));
    }
    push_inner_html(target, inner_html, children);
    target.borrow_mut().push(end_chunk_for_tag("title"));
    js::null()
}

thread_local! {
    // These are used by the client if we clear a boundary and we find these, then we
    // also clear the singleton as well.
    static HEAD_PREAMBLE_CONTRIBUTION_CHUNK: &'static PrecomputedChunk = string_to_precomputed_chunk("<!--head-->");
    static BODY_PREAMBLE_CONTRIBUTION_CHUNK: &'static PrecomputedChunk = string_to_precomputed_chunk("<!--body-->");
    static HTML_PREAMBLE_CONTRIBUTION_CHUNK: &'static PrecomputedChunk = string_to_precomputed_chunk("<!--html-->");
}

/// A singleton's tag in the preamble, `head`, `body` or `html`: its chunks,
/// made, and the marker of its contribution.
fn push_start_preamble_singleton(
    target: &Chunks,
    props: &Unknown,
    preamble: &RefCell<Option<Rc<Chunks>>>,
    (tag, marker, first): (
        &str,
        &'static std::thread::LocalKey<&'static Chunk>,
        Option<&'static Chunk>,
    ),
    in_preamble: bool,
    format_context: &FormatContext,
) -> Option<&'static Unknown> {
    if preamble.borrow().is_some() {
        panic!("The `<{tag}>` tag may only be rendered once.");
    }

    // Insert a marker in the body where the contribution to the head was in case we need to clear it.
    if in_preamble {
        target.borrow_mut().push(chunk(marker));
    }

    let chunks = Rc::new(RefCell::new(first.into_iter().collect()));
    *preamble.borrow_mut() = Some(Rc::clone(&chunks));
    push_start_singleton_element(&chunks, props, tag, format_context)
}

fn push_start_head(
    target: &Chunks,
    props: &Unknown,
    render_state: &RenderState,
    preamble_state: Option<&PreambleState>,
    format_context: &FormatContext,
) -> Option<&'static Unknown> {
    if format_context.insertion_mode < HTML_MODE {
        // This <head> is the Document.head and should be part of the preamble
        let preamble = preamble_state.unwrap_or(&render_state.preamble);
        push_start_preamble_singleton(
            target,
            props,
            &preamble.head_chunks,
            ("head", &HEAD_PREAMBLE_CONTRIBUTION_CHUNK, None),
            preamble_state.is_some(),
            format_context,
        )
    } else {
        // This <head> is deep and is likely just an error. we emit it inline though.
        // Validation should warn that this tag is the the wrong spot.
        push_start_generic_element(target, props, "head", format_context)
    }
}

fn push_start_body(
    target: &Chunks,
    props: &Unknown,
    render_state: &RenderState,
    preamble_state: Option<&PreambleState>,
    format_context: &FormatContext,
) -> Option<&'static Unknown> {
    if format_context.insertion_mode < HTML_MODE {
        // This <body> is the Document.body
        let preamble = preamble_state.unwrap_or(&render_state.preamble);
        push_start_preamble_singleton(
            target,
            props,
            &preamble.body_chunks,
            ("body", &BODY_PREAMBLE_CONTRIBUTION_CHUNK, None),
            preamble_state.is_some(),
            format_context,
        )
    } else {
        // This <head> is deep and is likely just an error. we emit it inline though.
        // Validation should warn that this tag is the the wrong spot.
        push_start_generic_element(target, props, "body", format_context)
    }
}

fn push_start_html(
    target: &Chunks,
    props: &Unknown,
    render_state: &RenderState,
    preamble_state: Option<&PreambleState>,
    format_context: &FormatContext,
) -> Option<&'static Unknown> {
    if format_context.insertion_mode == ROOT_HTML_MODE {
        // This <html> is the Document.documentElement
        let preamble = preamble_state.unwrap_or(&render_state.preamble);
        push_start_preamble_singleton(
            target,
            props,
            &preamble.html_chunks,
            ("html", &HTML_PREAMBLE_CONTRIBUTION_CHUNK, Some(DOCTYPE)),
            preamble_state.is_some(),
            format_context,
        )
    } else {
        // This <html> is deep and is likely just an error. we emit it inline though.
        // Validation should warn that this tag is the the wrong spot.
        push_start_generic_element(target, props, "html", format_context)
    }
}

fn push_script(
    target: &Chunks,
    props: &'static Unknown,
    resumable_state: &ResumableState,
    render_state: &RenderState,
    text_embedded: bool,
    format_context: &FormatContext,
) -> Option<&'static Unknown> {
    let noscript_tag_in_scope = format_context.tag_scope & NOSCRIPT_SCOPE;
    let async_prop = js::get(props, "async");
    let Kind::String(src) = js::classify(js::get(props, "src")) else {
        // This script will not be a resource, we bailout early and emit it in place.
        return push_script_impl(target, props);
    };
    if src.is_empty()
        || !(js::truthy(async_prop)
            && !matches!(
                js::classify(async_prop),
                Kind::Function(_) | Kind::Symbol(_)
            ))
        || js::truthy(js::get(props, "onLoad"))
        || js::truthy(js::get(props, "onError"))
        || format_context.insertion_mode == SVG_MODE
        || noscript_tag_in_scope != 0
        || js::get(props, "itemProp").is_some()
    {
        // This script will not be a resource, we bailout early and emit it in place.
        return push_script_impl(target, props);
    }

    let key = get_resource_key(src);
    // We can make this <script> into a ScriptResource

    let (resources, preloads) = if js::strict_equals(js::get(props, "type"), "module") {
        (
            resumable_state.module_script_resources.get(),
            render_state.preloads.module_scripts,
        )
    } else {
        (
            resumable_state.script_resources.get(),
            render_state.preloads.scripts,
        )
    };

    let has_key = js::unknown(resources).has_own_property(&key);
    let resource_state = if has_key {
        js::get(resources, &key)
    } else {
        None
    };
    if !exists(resources, &key) {
        // We are going to create this resource now so it is marked as Exists
        dict::set(resources, &key, EXISTS.with(|exists| *exists));

        let mut script_props = props;
        if let Some(resource_state) = resource_state {
            // When resourceState is truty it is a Preload state. We cast it for clarity
            let preload_state: &[Option<&'static Unknown>] =
                unsafe { js::cast(Some(resource_state)) };
            if preload_state.len() == 2 {
                script_props = js::object::spread(Some(props));
                adopt_preload_credentials(script_props, preload_state);
            }

            if let Some(preload_resource) = preloads.get(&key) {
                // the preload resource exists was created in this render. Now that we have
                // a script resource which will emit earlier than a preload would if it
                // hasn't already flushed we prevent it from flushing by zeroing the length
                preload_resource.borrow_mut().clear();
            }
        }

        let resource = Rc::new(RefCell::new(Vec::new()));
        // Add to the script flushing queue
        render_state.scripts.add(&resource);
        // encode the tag as Chunks
        push_script_impl(&resource, script_props);
    }

    if text_embedded {
        // This script follows text but we aren't writing a tag. while not as efficient as possible we need
        // to be safe and assume text will follow by inserting a textSeparator
        target.borrow_mut().push(chunk(&TEXT_SEPARATOR));
    }
    js::null()
}

fn push_script_impl(target: &Chunks, props: &Unknown) -> Option<&'static Unknown> {
    target.borrow_mut().push(start_chunk_for_tag("script"));

    let mut children = None;
    let mut inner_html = None;
    for prop_key in js::keys_in(props) {
        if HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(props, &prop_key)) {
            let Some(prop_value) = js::get(props, &prop_key) else {
                continue;
            };
            match prop_key.as_str() {
                "children" => children = Some(prop_value),
                "dangerouslySetInnerHTML" => inner_html = Some(prop_value),
                _ => push_attribute(target, &prop_key, prop_value),
            }
        }
    }
    // Scripts never participate as a ViewTransition
    target.borrow_mut().push(chunk(&END_OF_START_TAG));

    if __DEV__ && children.is_some() && js::type_of(children) != "string" {
        let descriptive_statement = if js::type_of(children) == "number" {
            "a number for children"
        } else if js::array::is_array(children) {
            "an array for children"
        } else {
            "something unexpected for children"
        };
        webapi::console::error((
            concat!(
                "A script element was rendered with %s. If script element has children it must be a single string.",
                " Consider using dangerouslySetInnerHTML or passing a plain string as children."
            ),
            descriptive_statement,
        ));
    }

    push_inner_html(target, inner_html, children);
    if let Kind::String(children) = js::classify(children) {
        target
            .borrow_mut()
            .push(string_to_chunk(&escape_entire_inline_script_content(
                children,
            )));
    }
    target.borrow_mut().push(end_chunk_for_tag("script"));
    js::null()
}

fn push_start_singleton_element(
    target: &Chunks,
    props: &Unknown,
    tag: &str,
    format_context: &FormatContext,
) -> Option<&'static Unknown> {
    target.borrow_mut().push(start_chunk_for_tag(tag));

    let mut children = js::null();
    let mut inner_html = None;
    for prop_key in js::keys_in(props) {
        if HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(props, &prop_key)) {
            let Some(prop_value) = js::get(props, &prop_key) else {
                continue;
            };
            match prop_key.as_str() {
                "children" => children = Some(prop_value),
                "dangerouslySetInnerHTML" => inner_html = Some(prop_value),
                _ => push_attribute(target, &prop_key, prop_value),
            }
        }
    }

    push_view_transition_attributes(target, format_context);

    target.borrow_mut().push(chunk(&END_OF_START_TAG));
    push_inner_html(target, inner_html, children);
    children
}

fn push_start_generic_element(
    target: &Chunks,
    props: &Unknown,
    tag: &str,
    format_context: &FormatContext,
) -> Option<&'static Unknown> {
    target.borrow_mut().push(start_chunk_for_tag(tag));

    let mut children = js::null();
    let mut inner_html = None;
    for prop_key in js::keys_in(props) {
        if HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(props, &prop_key)) {
            let Some(prop_value) = js::get(props, &prop_key) else {
                continue;
            };
            match prop_key.as_str() {
                "children" => children = Some(prop_value),
                "dangerouslySetInnerHTML" => inner_html = Some(prop_value),
                _ => push_attribute(target, &prop_key, prop_value),
            }
        }
    }

    push_view_transition_attributes(target, format_context);

    target.borrow_mut().push(chunk(&END_OF_START_TAG));
    push_inner_html(target, inner_html, children);
    if let Kind::String(children) = js::classify(children) {
        // Special case children as a string to avoid the unnecessary comment.
        // TODO: Remove this special case after the general optimization is in place.
        target
            .borrow_mut()
            .push(string_to_chunk(&encode_html_text_node(children)));
        return js::null();
    }
    children
}

fn push_start_custom_element(
    target: &Chunks,
    props: &Unknown,
    tag: &str,
    format_context: &FormatContext,
) -> Option<&'static Unknown> {
    target.borrow_mut().push(start_chunk_for_tag(tag));

    let mut children = js::null();
    let mut inner_html = None;
    for prop_key in js::keys_in(props) {
        if HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(props, &prop_key)) {
            let Some(mut prop_value) = js::get(props, &prop_key) else {
                continue;
            };
            let mut attribute_name = prop_key.as_str();
            match prop_key.as_str() {
                "children" => children = Some(prop_value),
                "dangerouslySetInnerHTML" => inner_html = Some(prop_value),
                "style" => push_style_attribute(target, prop_value),
                "suppressContentEditableWarning" | "suppressHydrationWarning" | "ref" => {
                    // Ignored. These are built-in to React on the client.
                }
                _ => {
                    if prop_key == "className" {
                        // className gets rendered as class on the client, so it should be
                        // rendered as class on the server.
                        attribute_name = "class";
                    }
                    // intentional fallthrough
                    if is_attribute_name_safe(&prop_key)
                        && !matches!(
                            js::classify(Some(prop_value)),
                            Kind::Function(_) | Kind::Symbol(_)
                        )
                    {
                        if js::strict_equals(Some(prop_value), false) {
                            continue;
                        } else if js::strict_equals(Some(prop_value), true) {
                            prop_value = js::unknown("");
                        } else if js::type_of(Some(prop_value)) == "object" {
                            continue;
                        }
                        push_text_attribute(target, attribute_name, prop_value);
                    }
                }
            }
        }
    }

    // TODO: ViewTransition attributes gets observed by the Custom Element which is a bit sketchy.
    push_view_transition_attributes(target, format_context);

    target.borrow_mut().push(chunk(&END_OF_START_TAG));
    push_inner_html(target, inner_html, children);
    children
}

thread_local! {
    static LEADING_NEWLINE: &'static PrecomputedChunk = string_to_precomputed_chunk("\n");
}

fn push_start_preformatted_element(
    target: &Chunks,
    props: &Unknown,
    tag: &str,
    format_context: &FormatContext,
) -> Option<&'static Unknown> {
    target.borrow_mut().push(start_chunk_for_tag(tag));

    let mut children = js::null();
    let mut inner_html = None;
    for prop_key in js::keys_in(props) {
        if HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(props, &prop_key)) {
            let Some(prop_value) = js::get(props, &prop_key) else {
                continue;
            };
            match prop_key.as_str() {
                "children" => children = Some(prop_value),
                "dangerouslySetInnerHTML" => inner_html = Some(prop_value),
                _ => push_attribute(target, &prop_key, prop_value),
            }
        }
    }

    push_view_transition_attributes(target, format_context);

    target.borrow_mut().push(chunk(&END_OF_START_TAG));

    // text/html ignores the first character in these tags if it's a newline
    // Prefer to break application/xml over text/html (for now) by adding
    // a newline specifically to get eaten by the parser. (Alternately for
    // textareas, replacing "^\n" with "\r\n" doesn't get eaten, and the first
    // \r is normalized out by HTMLTextAreaElement#value.)
    // See: <http://www.w3.org/TR/html-polyglot/#newlines-in-textarea-and-pre>
    // See: <http://www.w3.org/TR/html5/syntax.html#element-restrictions>
    // See: <http://www.w3.org/TR/html5/syntax.html#newlines>
    // See: Parsing of "textarea" "listing" and "pre" elements
    //  from <http://www.w3.org/TR/html5/syntax.html#parsing-main-inbody>
    // TODO: This doesn't deal with the case where the child is an array
    // or component that returns a string.
    if let Some(inner_html) = inner_html {
        if children.is_some() {
            panic!("Can only set one of `children` or `props.dangerouslySetInnerHTML`.");
        }

        if js::type_of(Some(inner_html)) != "object" || !js::has(inner_html, "__html") {
            panic!(
                "{}",
                concat!(
                    "`props.dangerouslySetInnerHTML` must be in the form `{__html: ...}`. ",
                    "Please visit https://react.dev/link/dangerously-set-inner-html ",
                    "for more information."
                )
            );
        }

        let html = js::get(inner_html, "__html");
        if html.is_some() {
            if let Kind::String(html) = js::classify(html)
                && js::string::length(html) > 0
                && js::string::get(html, 0.0) == Some("\n")
            {
                target
                    .borrow_mut()
                    .extend([chunk(&LEADING_NEWLINE), string_to_chunk(html)]);
            } else {
                if __DEV__ {
                    check_html_string_coercion(html);
                }
                target
                    .borrow_mut()
                    .push(string_to_chunk(&js::concat_text(html).unwrap()));
            }
        }
    }
    if let Kind::String(text) = js::classify(children)
        && js::string::get(text, 0.0) == Some("\n")
    {
        target.borrow_mut().push(chunk(&LEADING_NEWLINE));
    }
    children
}

thread_local! {
    // We accept any tag to be rendered but since this gets injected into arbitrary
    // HTML, we want to make sure that it's a safe tag.
    // http://www.w3.org/TR/REC-xml/#NT-Name
    static VALID_TAG_REGEX: &'static js::RegExp = js::reg_exp::new("^[a-zA-Z][a-zA-Z:_\\.\\-\\d]*$", ""); // Simplified subset
    static VALIDATED_TAG_CACHE: &'static js::Map<str, &'static PrecomputedChunk> = js::map::new(None);
}

fn start_chunk_for_tag(tag: &str) -> &'static PrecomputedChunk {
    VALIDATED_TAG_CACHE.with(|validated_tag_cache| {
        if let Some(tag_start_chunk) = validated_tag_cache.get(tag) {
            return tag_start_chunk;
        }
        if !VALID_TAG_REGEX.with(|valid_tag_regex| valid_tag_regex.test(tag)) {
            panic!("Invalid tag: {tag}");
        }

        let tag_start_chunk = string_to_precomputed_chunk(&("<".to_owned() + tag));
        validated_tag_cache.set(tag, tag_start_chunk);
        tag_start_chunk
    })
}

thread_local! {
    pub static DOCTYPE_CHUNK: &'static PrecomputedChunk = string_to_precomputed_chunk("<!DOCTYPE html>");
}

pub fn push_start_instance(
    target: &Chunks,
    r#type: &str,
    props: &'static Unknown,
    resumable_state: &ResumableState,
    render_state: &RenderState,
    preamble_state: Option<&PreambleState>,
    hoistable_state: Option<&HoistableState>,
    format_context: &FormatContext,
    text_embedded: bool,
) -> Option<&'static Unknown> {
    if __DEV__ {
        validate_aria_properties(r#type, props);
        validate_input_properties(r#type, Some(props));
        validate_unknown_properties(r#type, props, None);

        if !js::truthy(js::get(props, "suppressContentEditableWarning"))
            && js::truthy(js::get(props, "contentEditable"))
            && js::get(props, "children").is_some()
        {
            webapi::console::error((concat!(
                "A component is `contentEditable` and contains `children` managed by ",
                "React. It is now your responsibility to guarantee that none of ",
                "those nodes are unexpectedly modified or duplicated. This is ",
                "probably not intentional."
            ),));
        }

        if format_context.insertion_mode != SVG_MODE
            && format_context.insertion_mode != MATHML_MODE
            && js::string::index_of(r#type, "-", None) == -1.0
            && r#type.to_lowercase() != r#type
        {
            webapi::console::error((
                concat!(
                    "<%s /> is using incorrect casing. ",
                    "Use PascalCase for React components, ",
                    "or lowercase for HTML elements."
                ),
                r#type,
            ));
        }
    }

    match r#type {
        "div" | "span" | "svg" | "path" => {
            // Fast track very common tags
        }
        "a" => return push_start_anchor(target, props, format_context),
        "g" | "p" | "li" => {
            // Fast track very common tags
        }
        // Special tags
        "select" => return push_start_select(target, props, format_context),
        "option" => return push_start_option(target, props, format_context),
        "textarea" => return push_start_text_area(target, props, format_context),
        "input" => return push_input(target, props, resumable_state, render_state, format_context),
        "button" => {
            return push_start_button(target, props, resumable_state, render_state, format_context);
        }
        "form" => {
            return push_start_form(target, props, resumable_state, render_state, format_context);
        }
        "menuitem" => return push_start_menu_item(target, props, format_context),
        "object" => return push_start_object(target, props, format_context),
        "title" => return push_title(target, props, render_state, format_context),
        "link" => {
            return push_link(
                target,
                props,
                resumable_state,
                render_state,
                hoistable_state,
                text_embedded,
                format_context,
            );
        }
        "script" => {
            return push_script(
                target,
                props,
                resumable_state,
                render_state,
                text_embedded,
                format_context,
            );
        }
        "style" => {
            return push_style(
                target,
                props,
                resumable_state,
                render_state,
                hoistable_state,
                text_embedded,
                format_context,
            );
        }
        "meta" => return push_meta(target, props, render_state, text_embedded, format_context),
        // Newline eating tags
        "listing" | "pre" => {
            return push_start_preformatted_element(target, props, r#type, format_context);
        }
        "img" => {
            return push_img(
                target,
                props,
                resumable_state,
                render_state,
                hoistable_state,
                format_context,
            );
        }
        // Omitted close tags
        "base" | "area" | "br" | "col" | "embed" | "hr" | "keygen" | "param" | "source"
        | "track" | "wbr" => {
            return push_self_closing(target, props, r#type, format_context);
        }
        // These are reserved SVG and MathML elements, that are never custom elements.
        // https://html.spec.whatwg.org/multipage/custom-elements.html#custom-elements-core-concepts
        "annotation-xml" | "color-profile" | "font-face" | "font-face-src" | "font-face-uri"
        | "font-face-format" | "font-face-name" | "missing-glyph" => {}
        // Preamble start tags
        "head" => {
            return push_start_head(target, props, render_state, preamble_state, format_context);
        }
        "body" => {
            return push_start_body(target, props, render_state, preamble_state, format_context);
        }
        "html" => {
            return push_start_html(target, props, render_state, preamble_state, format_context);
        }
        _ => {
            if js::string::index_of(r#type, "-", None) != -1.0 {
                // Custom element
                return push_start_custom_element(target, props, r#type, format_context);
            }
        }
    }
    // Generic element
    push_start_generic_element(target, props, r#type, format_context)
}

thread_local! {
    static END_TAG_CACHE: &'static js::Map<str, &'static PrecomputedChunk> = js::map::new(None);
}

fn end_chunk_for_tag(tag: &str) -> &'static PrecomputedChunk {
    END_TAG_CACHE.with(|end_tag_cache| {
        if let Some(chunk) = end_tag_cache.get(tag) {
            return chunk;
        }
        let chunk = string_to_precomputed_chunk(&("</".to_owned() + tag + ">"));
        end_tag_cache.set(tag, chunk);
        chunk
    })
}

pub fn push_end_instance(
    target: &Chunks,
    r#type: &str,
    _props: &Unknown,
    resumable_state: &ResumableState,
    format_context: &FormatContext,
) {
    match r#type {
        // We expect title and script tags to always be pushed in a unit and never
        // return children. when we end up pushing the end tag we want to ensure
        // there is no extra closing tag pushed
        "title" | "style" | "script"
        // Omitted close tags
        // TODO: Instead of repeating this switch we could try to pass a flag from above.
        // That would require returning a tuple. Which might be ok if it gets inlined.
        | "area" | "base" | "br" | "col" | "embed" | "hr" | "img" | "input" | "keygen" | "link" | "meta"
        | "param" | "source" | "track" | "wbr" => {
            // No close tag needed.
            return;
        }
        // Postamble end tags
        // When float is enabled we omit the end tags for body and html when
        // they represent the Document.body and Document.documentElement Nodes.
        // This is so we can withhold them until the postamble when we know
        // we won't emit any more tags
        "body" => {
            if format_context.insertion_mode <= HTML_HTML_MODE {
                resumable_state.has_body.set(true);
                return;
            }
        }
        "html" => {
            if format_context.insertion_mode == ROOT_HTML_MODE {
                resumable_state.has_html.set(true);
                return;
            }
        }
        "head" if format_context.insertion_mode <= HTML_HTML_MODE => {
            return;
        }
        _ => {}
    }
    target.borrow_mut().push(end_chunk_for_tag(r#type));
}

pub fn hoist_preamble_state(render_state: &RenderState, preamble_state: &PreambleState) {
    let root_preamble = &render_state.preamble;
    if root_preamble.html_chunks.borrow().is_none()
        && let Some(html_chunks) = preamble_state.html_chunks.borrow().as_ref()
    {
        *root_preamble.html_chunks.borrow_mut() = Some(Rc::clone(html_chunks));
    }
    if root_preamble.head_chunks.borrow().is_none()
        && let Some(head_chunks) = preamble_state.head_chunks.borrow().as_ref()
    {
        *root_preamble.head_chunks.borrow_mut() = Some(Rc::clone(head_chunks));
    }
    if root_preamble.body_chunks.borrow().is_none()
        && let Some(body_chunks) = preamble_state.body_chunks.borrow().as_ref()
    {
        *root_preamble.body_chunks.borrow_mut() = Some(Rc::clone(body_chunks));
    }
}

pub fn is_preamble_ready(
    render_state: &RenderState,
    // This means there are unfinished Suspense boundaries which could contain
    // a preamble. In the case of DOM we constrain valid programs to only having
    // one instance of each singleton so we can determine the preamble is ready
    // as long as we have chunks for each of these tags.
    has_pending_preambles: bool,
) -> bool {
    let preamble = &render_state.preamble;
    // There are no remaining boundaries which might contain a preamble so
    // the preamble is as complete as it is going to get
    !has_pending_preambles
        // we have a head and body tag. we don't need to wait for any more
        // because it would be invalid to render additional copies of these tags
        || (preamble.head_chunks.borrow().is_some() && preamble.body_chunks.borrow().is_some())
}

fn write_bootstrap(destination: &Destination, render_state: &RenderState) -> bool {
    let bootstrap_chunks = &render_state.bootstrap_chunks;
    let mut i = 0;
    while i + 1 < bootstrap_chunks.borrow().len() {
        write_chunk(destination, bootstrap_chunks.borrow()[i]);
        i += 1;
    }
    if i < bootstrap_chunks.borrow().len() {
        let last_chunk = bootstrap_chunks.borrow()[i];
        bootstrap_chunks.borrow_mut().clear();
        return write_chunk_and_return(destination, last_chunk);
    }
    true
}

thread_local! {
    static SHELL_TIME_RUNTIME_SCRIPT: &'static PrecomputedChunk = string_to_precomputed_chunk(MARK_SHELL_TIME);
}

fn write_shell_time_instruction(
    destination: &Destination,
    resumable_state: &ResumableState,
    render_state: &RenderState,
) -> bool {
    if ENABLE_FIZZ_EXTERNAL_RUNTIME.with(|enabled| *enabled)
        && resumable_state.streaming_format != SCRIPT_STREAMING_FORMAT
    {
        // External runtime always tracks the shell time in the runtime.
        return true;
    }
    if (resumable_state.instructions.get() & SENT_MARK_SHELL_TIME) != NOTHING_SENT {
        // We already sent this instruction.
        return true;
    }
    resumable_state
        .instructions
        .set(resumable_state.instructions.get() | SENT_MARK_SHELL_TIME);
    write_chunk(destination, render_state.start_inline_script);
    write_completed_shell_id_attribute(destination, resumable_state);
    write_chunk(destination, chunk(&END_OF_START_TAG));
    write_chunk(destination, chunk(&SHELL_TIME_RUNTIME_SCRIPT));
    write_chunk_and_return(destination, chunk(&END_INLINE_SCRIPT))
}

pub fn write_completed_root(
    destination: &Destination,
    resumable_state: &ResumableState,
    render_state: &RenderState,
    is_complete: bool,
) -> bool {
    if !is_complete {
        // If we're not already fully complete, we might complete another boundary. If so,
        // we need to track the paint time of the shell so we know how much to throttle the reveal.
        write_shell_time_instruction(destination, resumable_state, render_state);
    }
    if ENABLE_FIZZ_BLOCKING_RENDER.with(|enabled| *enabled) {
        let preamble = &render_state.preamble;
        if (preamble.html_chunks.borrow().is_some() || preamble.head_chunks.borrow().is_some())
            // If we rendered the whole document, then we emitted a rel="expect" that needs a
            // matching target. Normally we use one of the bootstrap scripts for this but if
            // there are none, then we need to emit a tag to complete the shell.
            && (resumable_state.instructions.get() & SENT_COMPLETED_SHELL_ID) == NOTHING_SENT
        {
            write_chunk(destination, start_chunk_for_tag("template"));
            write_completed_shell_id_attribute(destination, resumable_state);
            write_chunk(destination, chunk(&END_OF_START_TAG));
            write_chunk(destination, end_chunk_for_tag("template"));
        }
    }
    write_bootstrap(destination, render_state)
}

// Structural Nodes

thread_local! {
    // A placeholder is a node inside a hidden partial tree that can be filled in later, but before
    // display. It's never visible to users. We use the template tag because it can be used in every
    // type of parent. <script> tags also work in every other tag except <colgroup>.
    static PLACEHOLDER1: &'static PrecomputedChunk = string_to_precomputed_chunk("<template id=\"");
    static PLACEHOLDER2: &'static PrecomputedChunk = string_to_precomputed_chunk("\"></template>");
}

pub fn write_placeholder(destination: &Destination, render_state: &RenderState, id: f64) -> bool {
    write_chunk(destination, chunk(&PLACEHOLDER1));
    write_chunk(destination, render_state.placeholder_prefix);
    #[cfg_attr(rust_js, rust_js::name = "formattedID")]
    let formatted_id = string_to_chunk(&js::number::to_string(id, Some(16)));
    write_chunk(destination, formatted_id);
    write_chunk_and_return(destination, chunk(&PLACEHOLDER2))
}

thread_local! {
    // Activity boundaries are encoded as comments.
    static START_ACTIVITY_BOUNDARY: &'static PrecomputedChunk = string_to_precomputed_chunk("<!--&-->");
    static END_ACTIVITY_BOUNDARY: &'static PrecomputedChunk = string_to_precomputed_chunk("<!--/&-->");
}

pub fn push_start_activity_boundary(target: &Chunks, _render_state: &RenderState) {
    target.borrow_mut().push(chunk(&START_ACTIVITY_BOUNDARY));
}

pub fn push_end_activity_boundary(target: &Chunks, _render_state: &RenderState) {
    target.borrow_mut().push(chunk(&END_ACTIVITY_BOUNDARY));
}

thread_local! {
    // Suspense boundaries are encoded as comments.
    static START_COMPLETED_SUSPENSE_BOUNDARY: &'static PrecomputedChunk = string_to_precomputed_chunk("<!--$-->");
    static START_PENDING_SUSPENSE_BOUNDARY1: &'static PrecomputedChunk =
        string_to_precomputed_chunk("<!--$?--><template id=\"");
    static START_PENDING_SUSPENSE_BOUNDARY2: &'static PrecomputedChunk = string_to_precomputed_chunk("\"></template>");
    static START_CLIENT_RENDERED_SUSPENSE_BOUNDARY: &'static PrecomputedChunk = string_to_precomputed_chunk("<!--$!-->");
    static END_SUSPENSE_BOUNDARY: &'static PrecomputedChunk = string_to_precomputed_chunk("<!--/$-->");

    static CLIENT_RENDERED_SUSPENSE_BOUNDARY_ERROR1: &'static PrecomputedChunk = string_to_precomputed_chunk("<template");
    static CLIENT_RENDERED_SUSPENSE_BOUNDARY_ERROR_ATTR_INTERSTITIAL: &'static PrecomputedChunk =
        string_to_precomputed_chunk("\"");
    static CLIENT_RENDERED_SUSPENSE_BOUNDARY_ERROR1_A: &'static PrecomputedChunk = string_to_precomputed_chunk(" data-dgst=\"");
    static CLIENT_RENDERED_SUSPENSE_BOUNDARY_ERROR1_B: &'static PrecomputedChunk = string_to_precomputed_chunk(" data-msg=\"");
    static CLIENT_RENDERED_SUSPENSE_BOUNDARY_ERROR1_C: &'static PrecomputedChunk = string_to_precomputed_chunk(" data-stck=\"");
    static CLIENT_RENDERED_SUSPENSE_BOUNDARY_ERROR1_D: &'static PrecomputedChunk = string_to_precomputed_chunk(" data-cstck=\"");
    static CLIENT_RENDERED_SUSPENSE_BOUNDARY_ERROR2: &'static PrecomputedChunk = string_to_precomputed_chunk("></template>");
}

pub fn write_start_completed_suspense_boundary(
    destination: &Destination,
    _render_state: &RenderState,
) -> bool {
    write_chunk_and_return(destination, chunk(&START_COMPLETED_SUSPENSE_BOUNDARY))
}

pub fn write_start_pending_suspense_boundary(
    destination: &Destination,
    render_state: &RenderState,
    id: Option<f64>,
) -> bool {
    write_chunk(destination, chunk(&START_PENDING_SUSPENSE_BOUNDARY1));

    let Some(id) = id else {
        panic!("An ID must have been assigned before we can complete the boundary.");
    };

    write_chunk(destination, render_state.boundary_prefix);
    write_chunk(
        destination,
        string_to_chunk(&js::number::to_string(id, Some(16))),
    );
    write_chunk_and_return(destination, chunk(&START_PENDING_SUSPENSE_BOUNDARY2))
}

/// An attribute of a client rendered boundary's error, `data-dgst` say.
fn write_error_attribute(destination: &Destination, start: &'static Chunk, value: &str) {
    write_chunk(destination, start);
    write_chunk(
        destination,
        string_to_chunk(&escape_text_for_browser(Some(js::unknown(value)))),
    );
    write_chunk(
        destination,
        chunk(&CLIENT_RENDERED_SUSPENSE_BOUNDARY_ERROR_ATTR_INTERSTITIAL),
    );
}

pub fn write_start_client_rendered_suspense_boundary(
    destination: &Destination,
    _render_state: &RenderState,
    error_digest: Option<&str>,
    error_message: Option<&str>,
    error_stack: Option<&str>,
    error_component_stack: Option<&str>,
) -> bool {
    write_chunk_and_return(destination, chunk(&START_CLIENT_RENDERED_SUSPENSE_BOUNDARY));
    write_chunk(
        destination,
        chunk(&CLIENT_RENDERED_SUSPENSE_BOUNDARY_ERROR1),
    );
    if let Some(error_digest) = error_digest {
        write_error_attribute(
            destination,
            chunk(&CLIENT_RENDERED_SUSPENSE_BOUNDARY_ERROR1_A),
            error_digest,
        );
    }
    if __DEV__ {
        if let Some(error_message) = error_message.filter(|text| !text.is_empty()) {
            write_error_attribute(
                destination,
                chunk(&CLIENT_RENDERED_SUSPENSE_BOUNDARY_ERROR1_B),
                error_message,
            );
        }
        if let Some(error_stack) = error_stack.filter(|text| !text.is_empty()) {
            write_error_attribute(
                destination,
                chunk(&CLIENT_RENDERED_SUSPENSE_BOUNDARY_ERROR1_C),
                error_stack,
            );
        }
        if let Some(error_component_stack) = error_component_stack.filter(|text| !text.is_empty()) {
            write_error_attribute(
                destination,
                chunk(&CLIENT_RENDERED_SUSPENSE_BOUNDARY_ERROR1_D),
                error_component_stack,
            );
        }
    }
    write_chunk_and_return(
        destination,
        chunk(&CLIENT_RENDERED_SUSPENSE_BOUNDARY_ERROR2),
    )
}

pub fn write_end_completed_suspense_boundary(
    destination: &Destination,
    _render_state: &RenderState,
) -> bool {
    write_chunk_and_return(destination, chunk(&END_SUSPENSE_BOUNDARY))
}

pub fn write_end_pending_suspense_boundary(
    destination: &Destination,
    _render_state: &RenderState,
) -> bool {
    write_chunk_and_return(destination, chunk(&END_SUSPENSE_BOUNDARY))
}

pub fn write_end_client_rendered_suspense_boundary(
    destination: &Destination,
    _render_state: &RenderState,
) -> bool {
    write_chunk_and_return(destination, chunk(&END_SUSPENSE_BOUNDARY))
}

thread_local! {
    static START_SEGMENT_HTML: &'static PrecomputedChunk = string_to_precomputed_chunk("<div hidden id=\"");
    static START_SEGMENT_HTML2: &'static PrecomputedChunk = string_to_precomputed_chunk("\">");
    static END_SEGMENT_HTML: &'static PrecomputedChunk = string_to_precomputed_chunk("</div>");

    static START_SEGMENT_SVG: &'static PrecomputedChunk =
        string_to_precomputed_chunk("<svg aria-hidden=\"true\" style=\"display:none\" id=\"");
    static START_SEGMENT_SVG2: &'static PrecomputedChunk = string_to_precomputed_chunk("\">");
    static END_SEGMENT_SVG: &'static PrecomputedChunk = string_to_precomputed_chunk("</svg>");

    static START_SEGMENT_MATH_ML: &'static PrecomputedChunk =
        string_to_precomputed_chunk("<math aria-hidden=\"true\" style=\"display:none\" id=\"");
    static START_SEGMENT_MATH_ML2: &'static PrecomputedChunk = string_to_precomputed_chunk("\">");
    static END_SEGMENT_MATH_ML: &'static PrecomputedChunk = string_to_precomputed_chunk("</math>");

    static START_SEGMENT_TABLE: &'static PrecomputedChunk = string_to_precomputed_chunk("<table hidden id=\"");
    static START_SEGMENT_TABLE2: &'static PrecomputedChunk = string_to_precomputed_chunk("\">");
    static END_SEGMENT_TABLE: &'static PrecomputedChunk = string_to_precomputed_chunk("</table>");

    static START_SEGMENT_TABLE_BODY: &'static PrecomputedChunk = string_to_precomputed_chunk("<table hidden><tbody id=\"");
    static START_SEGMENT_TABLE_BODY2: &'static PrecomputedChunk = string_to_precomputed_chunk("\">");
    static END_SEGMENT_TABLE_BODY: &'static PrecomputedChunk = string_to_precomputed_chunk("</tbody></table>");

    static START_SEGMENT_TABLE_ROW: &'static PrecomputedChunk = string_to_precomputed_chunk("<table hidden><tr id=\"");
    static START_SEGMENT_TABLE_ROW2: &'static PrecomputedChunk = string_to_precomputed_chunk("\">");
    static END_SEGMENT_TABLE_ROW: &'static PrecomputedChunk = string_to_precomputed_chunk("</tr></table>");

    static START_SEGMENT_COL_GROUP: &'static PrecomputedChunk =
        string_to_precomputed_chunk("<table hidden><colgroup id=\"");
    static START_SEGMENT_COL_GROUP2: &'static PrecomputedChunk = string_to_precomputed_chunk("\">");
    static END_SEGMENT_COL_GROUP: &'static PrecomputedChunk = string_to_precomputed_chunk("</colgroup></table>");
}

pub fn write_start_segment(
    destination: &Destination,
    render_state: &RenderState,
    format_context: &FormatContext,
    id: f64,
) -> bool {
    let (start, end) = match format_context.insertion_mode {
        ROOT_HTML_MODE | HTML_HTML_MODE | HTML_HEAD_MODE | HTML_MODE => {
            (&START_SEGMENT_HTML, &START_SEGMENT_HTML2)
        }
        SVG_MODE => (&START_SEGMENT_SVG, &START_SEGMENT_SVG2),
        MATHML_MODE => (&START_SEGMENT_MATH_ML, &START_SEGMENT_MATH_ML2),
        HTML_TABLE_MODE => (&START_SEGMENT_TABLE, &START_SEGMENT_TABLE2),
        // TODO: For the rest of these, there will be extra wrapper nodes that never
        // get deleted from the document. We need to delete the table too as part
        // of the injected scripts. They are invisible though so it's not too terrible
        // and it's kind of an edge case to suspend in a table. Totally supported though.
        HTML_TABLE_BODY_MODE => (&START_SEGMENT_TABLE_BODY, &START_SEGMENT_TABLE_BODY2),
        HTML_TABLE_ROW_MODE => (&START_SEGMENT_TABLE_ROW, &START_SEGMENT_TABLE_ROW2),
        HTML_COLGROUP_MODE => (&START_SEGMENT_COL_GROUP, &START_SEGMENT_COL_GROUP2),
        _ => panic!("Unknown insertion mode. This is a bug in React."),
    };
    write_chunk(destination, chunk(start));
    write_chunk(destination, render_state.segment_prefix);
    write_chunk(
        destination,
        string_to_chunk(&js::number::to_string(id, Some(16))),
    );
    write_chunk_and_return(destination, chunk(end))
}

pub fn write_end_segment(destination: &Destination, format_context: &FormatContext) -> bool {
    let end = match format_context.insertion_mode {
        ROOT_HTML_MODE | HTML_HTML_MODE | HTML_HEAD_MODE | HTML_MODE => &END_SEGMENT_HTML,
        SVG_MODE => &END_SEGMENT_SVG,
        MATHML_MODE => &END_SEGMENT_MATH_ML,
        HTML_TABLE_MODE => &END_SEGMENT_TABLE,
        HTML_TABLE_BODY_MODE => &END_SEGMENT_TABLE_BODY,
        HTML_TABLE_ROW_MODE => &END_SEGMENT_TABLE_ROW,
        HTML_COLGROUP_MODE => &END_SEGMENT_COL_GROUP,
        _ => panic!("Unknown insertion mode. This is a bug in React."),
    };
    write_chunk_and_return(destination, chunk(end))
}

thread_local! {
    static COMPLETE_SEGMENT_SCRIPT1_FULL: &'static PrecomputedChunk =
        string_to_precomputed_chunk(&(COMPLETE_SEGMENT_FUNCTION.to_owned() + "$RS(\""));
    static COMPLETE_SEGMENT_SCRIPT1_PARTIAL: &'static PrecomputedChunk = string_to_precomputed_chunk("$RS(\"");
    static COMPLETE_SEGMENT_SCRIPT2: &'static PrecomputedChunk = string_to_precomputed_chunk("\",\"");
    static COMPLETE_SEGMENT_SCRIPT_END: &'static PrecomputedChunk = string_to_precomputed_chunk("\")</script>");

    static COMPLETE_SEGMENT_DATA1: &'static PrecomputedChunk =
        string_to_precomputed_chunk("<template data-rsi=\"\" data-sid=\"");
    static COMPLETE_SEGMENT_DATA2: &'static PrecomputedChunk = string_to_precomputed_chunk("\" data-pid=\"");
    static COMPLETE_SEGMENT_DATA_END: &'static PrecomputedChunk = chunk(&DATA_ELEMENT_QUOTED_END);
}

/// Whether instructions are written as inline scripts, not as the data the
/// external runtime reads.
fn script_format(resumable_state: &ResumableState) -> bool {
    !ENABLE_FIZZ_EXTERNAL_RUNTIME.with(|enabled| *enabled)
        || resumable_state.streaming_format == SCRIPT_STREAMING_FORMAT
}

/// Whether `instruction` hasn't been sent, marking it sent.
fn send_once(resumable_state: &ResumableState, instruction: InstructionState) -> bool {
    if (resumable_state.instructions.get() & instruction) == NOTHING_SENT {
        resumable_state
            .instructions
            .set(resumable_state.instructions.get() | instruction);
        true
    } else {
        false
    }
}

pub fn write_completed_segment_instruction(
    destination: &Destination,
    resumable_state: &ResumableState,
    render_state: &RenderState,
    content_segment_id: f64,
) -> bool {
    let script_format = script_format(resumable_state);
    if script_format {
        write_chunk(destination, render_state.start_inline_script);
        write_chunk(destination, chunk(&END_OF_START_TAG));
        if send_once(resumable_state, SENT_COMPLETE_SEGMENT_FUNCTION) {
            // The first time we write this, we'll need to include the full implementation.
            write_chunk(destination, chunk(&COMPLETE_SEGMENT_SCRIPT1_FULL));
        } else {
            // Future calls can just reuse the same function.
            write_chunk(destination, chunk(&COMPLETE_SEGMENT_SCRIPT1_PARTIAL));
        }
    } else {
        write_chunk(destination, chunk(&COMPLETE_SEGMENT_DATA1));
    }

    // Write function arguments, which are string literals
    write_chunk(destination, render_state.segment_prefix);
    #[cfg_attr(rust_js, rust_js::name = "formattedID")]
    let formatted_id = string_to_chunk(&js::number::to_string(content_segment_id, Some(16)));
    write_chunk(destination, formatted_id);
    if script_format {
        write_chunk(destination, chunk(&COMPLETE_SEGMENT_SCRIPT2));
    } else {
        write_chunk(destination, chunk(&COMPLETE_SEGMENT_DATA2));
    }
    write_chunk(destination, render_state.placeholder_prefix);
    write_chunk(destination, formatted_id);

    if script_format {
        write_chunk_and_return(destination, chunk(&COMPLETE_SEGMENT_SCRIPT_END))
    } else {
        write_chunk_and_return(destination, chunk(&COMPLETE_SEGMENT_DATA_END))
    }
}

thread_local! {
    static COMPLETE_BOUNDARY_SCRIPT_FUNCTION_ONLY: &'static PrecomputedChunk =
        string_to_precomputed_chunk(COMPLETE_BOUNDARY_FUNCTION);
    static COMPLETE_BOUNDARY_UPGRADE_TO_VIEW_TRANSITIONS_INSTRUCTION: &'static Chunk =
        string_to_chunk(UPGRADE_TO_VIEW_TRANSITIONS_INSTRUCTION);
    static COMPLETE_BOUNDARY_SCRIPT1_PARTIAL: &'static PrecomputedChunk = string_to_precomputed_chunk("$RC(\"");

    static COMPLETE_BOUNDARY_WITH_STYLES_SCRIPT1_FULL_PARTIAL: &'static PrecomputedChunk =
        string_to_precomputed_chunk(&(STYLE_INSERTION_FUNCTION.to_owned() + "$RR(\""));

    static COMPLETE_BOUNDARY_WITH_STYLES_SCRIPT1_PARTIAL: &'static PrecomputedChunk = string_to_precomputed_chunk("$RR(\"");
    static COMPLETE_BOUNDARY_SCRIPT2: &'static PrecomputedChunk = string_to_precomputed_chunk("\",\"");
    static COMPLETE_BOUNDARY_SCRIPT3A: &'static PrecomputedChunk = string_to_precomputed_chunk("\",");
    static COMPLETE_BOUNDARY_SCRIPT3B: &'static PrecomputedChunk = string_to_precomputed_chunk("\"");
    static COMPLETE_BOUNDARY_SCRIPT_END: &'static PrecomputedChunk = string_to_precomputed_chunk(")</script>");

    static COMPLETE_BOUNDARY_DATA1: &'static PrecomputedChunk =
        string_to_precomputed_chunk("<template data-rci=\"\" data-bid=\"");
    static COMPLETE_BOUNDARY_WITH_STYLES_DATA1: &'static PrecomputedChunk =
        string_to_precomputed_chunk("<template data-rri=\"\" data-bid=\"");
    static COMPLETE_BOUNDARY_DATA2: &'static PrecomputedChunk = string_to_precomputed_chunk("\" data-sid=\"");
    static COMPLETE_BOUNDARY_DATA3A: &'static PrecomputedChunk = string_to_precomputed_chunk("\" data-sty=\"");
    static COMPLETE_BOUNDARY_DATA_END: &'static PrecomputedChunk = chunk(&DATA_ELEMENT_QUOTED_END);
}

pub fn write_completed_boundary_instruction(
    destination: &Destination,
    resumable_state: &ResumableState,
    render_state: &RenderState,
    id: f64,
    hoistable_state: &HoistableState,
) -> bool {
    let requires_style_insertion = render_state.styles_to_hoist.get();
    let requires_view_transitions = ENABLE_VIEW_TRANSITION
        && (resumable_state.instructions.get() & NEED_UPGRADE_TO_VIEW_TRANSITIONS) != NOTHING_SENT;
    // If necessary stylesheets will be flushed with this instruction.
    // Any style tags not yet hoisted in the Document will also be hoisted.
    // We reset this state since after this instruction executes all styles
    // up to this point will have been hoisted
    render_state.styles_to_hoist.set(false);
    let script_format = script_format(resumable_state);
    if script_format {
        write_chunk(destination, render_state.start_inline_script);
        write_chunk(destination, chunk(&END_OF_START_TAG));
        if requires_style_insertion {
            if send_once(resumable_state, SENT_CLIENT_RENDER_FUNCTION) {
                // The completeBoundaryWithStyles function depends on the client render function.
                write_chunk(destination, chunk(&CLIENT_RENDER_SCRIPT_FUNCTION_ONLY));
            }
            if send_once(resumable_state, SENT_COMPLETE_BOUNDARY_FUNCTION) {
                // The completeBoundaryWithStyles function depends on the complete boundary function.
                write_chunk(destination, chunk(&COMPLETE_BOUNDARY_SCRIPT_FUNCTION_ONLY));
            }
            if requires_view_transitions
                && send_once(resumable_state, SENT_UPGRADE_TO_VIEW_TRANSITIONS)
            {
                write_chunk(
                    destination,
                    chunk(&COMPLETE_BOUNDARY_UPGRADE_TO_VIEW_TRANSITIONS_INSTRUCTION),
                );
            }
            if send_once(resumable_state, SENT_STYLE_INSERTION_FUNCTION) {
                write_chunk(
                    destination,
                    chunk(&COMPLETE_BOUNDARY_WITH_STYLES_SCRIPT1_FULL_PARTIAL),
                );
            } else {
                write_chunk(
                    destination,
                    chunk(&COMPLETE_BOUNDARY_WITH_STYLES_SCRIPT1_PARTIAL),
                );
            }
        } else {
            if send_once(resumable_state, SENT_COMPLETE_BOUNDARY_FUNCTION) {
                write_chunk(destination, chunk(&COMPLETE_BOUNDARY_SCRIPT_FUNCTION_ONLY));
            }
            if requires_view_transitions
                && send_once(resumable_state, SENT_UPGRADE_TO_VIEW_TRANSITIONS)
            {
                write_chunk(
                    destination,
                    chunk(&COMPLETE_BOUNDARY_UPGRADE_TO_VIEW_TRANSITIONS_INSTRUCTION),
                );
            }
            write_chunk(destination, chunk(&COMPLETE_BOUNDARY_SCRIPT1_PARTIAL));
        }
    } else if requires_style_insertion {
        write_chunk(destination, chunk(&COMPLETE_BOUNDARY_WITH_STYLES_DATA1));
    } else {
        write_chunk(destination, chunk(&COMPLETE_BOUNDARY_DATA1));
    }

    let id_chunk = string_to_chunk(&js::number::to_string(id, Some(16)));

    write_chunk(destination, render_state.boundary_prefix);
    write_chunk(destination, id_chunk);

    // Write function arguments, which are string and array literals
    if script_format {
        write_chunk(destination, chunk(&COMPLETE_BOUNDARY_SCRIPT2));
    } else {
        write_chunk(destination, chunk(&COMPLETE_BOUNDARY_DATA2));
    }
    write_chunk(destination, render_state.segment_prefix);
    write_chunk(destination, id_chunk);
    if requires_style_insertion {
        // Script and data writers must format this differently:
        //  - script writer emits an array literal, whose string elements are
        //    escaped for javascript  e.g. ["A", "B"]
        //  - data writer emits a string literal, which is escaped as html
        //    e.g. [&#34;A&#34;, &#34;B&#34;]
        if script_format {
            write_chunk(destination, chunk(&COMPLETE_BOUNDARY_SCRIPT3A));
            // hoistableState encodes an array literal
            write_style_resource_dependencies_in_js(destination, hoistable_state);
        } else {
            write_chunk(destination, chunk(&COMPLETE_BOUNDARY_DATA3A));
            write_style_resource_dependencies_in_attr(destination, hoistable_state);
        }
    } else if script_format {
        write_chunk(destination, chunk(&COMPLETE_BOUNDARY_SCRIPT3B));
    }
    let write_more = if script_format {
        write_chunk_and_return(destination, chunk(&COMPLETE_BOUNDARY_SCRIPT_END))
    } else {
        write_chunk_and_return(destination, chunk(&COMPLETE_BOUNDARY_DATA_END))
    };
    write_bootstrap(destination, render_state) && write_more
}

thread_local! {
    static CLIENT_RENDER_SCRIPT_FUNCTION_ONLY: &'static PrecomputedChunk = string_to_precomputed_chunk(CLIENT_RENDER_FUNCTION);

    static CLIENT_RENDER_SCRIPT1_FULL: &'static PrecomputedChunk =
        string_to_precomputed_chunk(&(CLIENT_RENDER_FUNCTION.to_owned() + ";$RX(\""));
    static CLIENT_RENDER_SCRIPT1_PARTIAL: &'static PrecomputedChunk = string_to_precomputed_chunk("$RX(\"");
    static CLIENT_RENDER_SCRIPT1_A: &'static PrecomputedChunk = string_to_precomputed_chunk("\"");
    static CLIENT_RENDER_ERROR_SCRIPT_ARG_INTERSTITIAL: &'static PrecomputedChunk = string_to_precomputed_chunk(",");
    static CLIENT_RENDER_ERROR_SCRIPT_NULL: &'static PrecomputedChunk = string_to_precomputed_chunk("null");
    static CLIENT_RENDER_SCRIPT_END: &'static PrecomputedChunk = string_to_precomputed_chunk(")</script>");

    static CLIENT_RENDER_DATA1: &'static PrecomputedChunk = string_to_precomputed_chunk("<template data-rxi=\"\" data-bid=\"");
    static CLIENT_RENDER_DATA2: &'static PrecomputedChunk = string_to_precomputed_chunk("\" data-dgst=\"");
    static CLIENT_RENDER_DATA3: &'static PrecomputedChunk = string_to_precomputed_chunk("\" data-msg=\"");
    static CLIENT_RENDER_DATA4: &'static PrecomputedChunk = string_to_precomputed_chunk("\" data-stck=\"");
    static CLIENT_RENDER_DATA5: &'static PrecomputedChunk = string_to_precomputed_chunk("\" data-cstck=\"");
    static CLIENT_RENDER_DATA_END: &'static PrecomputedChunk = chunk(&DATA_ELEMENT_QUOTED_END);
}

/// One of a client rendered boundary's error's texts: in a script, a JS
/// string's literal, after `,`; as data, an attribute of `data`.
fn write_client_render_error(
    destination: &Destination,
    script_format: bool,
    data: &'static Chunk,
    text: &str,
) {
    if script_format {
        // ,"JSONString"
        write_chunk(
            destination,
            chunk(&CLIENT_RENDER_ERROR_SCRIPT_ARG_INTERSTITIAL),
        );
        write_chunk(
            destination,
            string_to_chunk(&escape_js_strings_for_instruction_scripts(text)),
        );
    } else {
        // " data-msg="HTMLString
        write_chunk(destination, data);
        write_chunk(
            destination,
            string_to_chunk(&escape_text_for_browser(Some(js::unknown(text)))),
        );
    }
}

pub fn write_client_render_boundary_instruction(
    destination: &Destination,
    resumable_state: &ResumableState,
    render_state: &RenderState,
    id: f64,
    error_digest: Option<&str>,
    error_message: Option<&str>,
    error_stack: Option<&str>,
    error_component_stack: Option<&str>,
) -> bool {
    let script_format = script_format(resumable_state);
    if script_format {
        write_chunk(destination, render_state.start_inline_script);
        write_chunk(destination, chunk(&END_OF_START_TAG));
        if send_once(resumable_state, SENT_CLIENT_RENDER_FUNCTION) {
            // The first time we write this, we'll need to include the full implementation.
            write_chunk(destination, chunk(&CLIENT_RENDER_SCRIPT1_FULL));
        } else {
            // Future calls can just reuse the same function.
            write_chunk(destination, chunk(&CLIENT_RENDER_SCRIPT1_PARTIAL));
        }
    } else {
        // <template data-rxi="" data-bid="
        write_chunk(destination, chunk(&CLIENT_RENDER_DATA1));
    }

    write_chunk(destination, render_state.boundary_prefix);
    write_chunk(
        destination,
        string_to_chunk(&js::number::to_string(id, Some(16))),
    );
    if script_format {
        // " needs to be inserted for scripts, since ArgInterstitual does not contain
        // leading or trailing quotes
        write_chunk(destination, chunk(&CLIENT_RENDER_SCRIPT1_A));
    }

    let error_message = error_message.filter(|text| !text.is_empty());
    let error_stack = error_stack.filter(|text| !text.is_empty());
    let error_component_stack = error_component_stack.filter(|text| !text.is_empty());
    if error_digest.is_some()
        || error_message.is_some()
        || error_stack.is_some()
        || error_component_stack.is_some()
    {
        if script_format {
            // ,null or ,"JSONString"
            write_chunk(
                destination,
                chunk(&CLIENT_RENDER_ERROR_SCRIPT_ARG_INTERSTITIAL),
            );
            match error_digest {
                None => write_chunk(destination, chunk(&CLIENT_RENDER_ERROR_SCRIPT_NULL)),
                Some(error_digest) => write_chunk(
                    destination,
                    string_to_chunk(&escape_js_strings_for_instruction_scripts(error_digest)),
                ),
            }
        } else if let Some(error_digest) = error_digest {
            // " data-dgst="HTMLString
            write_chunk(destination, chunk(&CLIENT_RENDER_DATA2));
            write_chunk(
                destination,
                string_to_chunk(&escape_text_for_browser(Some(js::unknown(error_digest)))),
            );
        }
    }
    if error_message.is_some() || error_stack.is_some() || error_component_stack.is_some() {
        write_client_render_error(
            destination,
            script_format,
            chunk(&CLIENT_RENDER_DATA3),
            error_message.unwrap_or_default(),
        );
    }
    if error_stack.is_some() || error_component_stack.is_some() {
        write_client_render_error(
            destination,
            script_format,
            chunk(&CLIENT_RENDER_DATA4),
            error_stack.unwrap_or_default(),
        );
    }
    if let Some(error_component_stack) = error_component_stack {
        write_client_render_error(
            destination,
            script_format,
            chunk(&CLIENT_RENDER_DATA5),
            error_component_stack,
        );
    }

    if script_format {
        // ></script>
        write_chunk_and_return(destination, chunk(&CLIENT_RENDER_SCRIPT_END))
    } else {
        // "></template>
        write_chunk_and_return(destination, chunk(&CLIENT_RENDER_DATA_END))
    }
}

thread_local! {
    static REGEX_FOR_JS_STRINGS_IN_INSTRUCTION_SCRIPTS: &'static js::RegExp = js::reg_exp::new("[<\u{2028}\u{2029}]", "g");
}

#[cfg_attr(rust_js, rust_js::name = "escapeJSStringsForInstructionScripts")]
fn escape_js_strings_for_instruction_scripts(input: &str) -> String {
    let escaped = js::json::stringify(input);
    REGEX_FOR_JS_STRINGS_IN_INSTRUCTION_SCRIPTS.with(|regex| {
        js::reg_exp::replace(&escaped, regex, |r#match: &str| match r#match {
            // santizing breaking out of strings and script tags
            "<" => "\\u003c",
            "\u{2028}" => "\\u2028",
            "\u{2029}" => "\\u2029",
            _ => panic!(
                "escapeJSStringsForInstructionScripts encountered a match it does not know how to replace. this means the match regex and the replacement characters are no longer in sync. This is a bug in React"
            ),
        })
    })
}

thread_local! {
    static REGEX_FOR_JS_STRINGS_IN_SCRIPTS: &'static js::RegExp = js::reg_exp::new("[&><\u{2028}\u{2029}]", "g");
}

#[cfg_attr(rust_js, rust_js::name = "escapeJSObjectForInstructionScripts")]
fn escape_js_object_for_instruction_scripts(input: &Unknown) -> String {
    let escaped = js::json::stringify_with(input, None, 0)
        .unwrap()
        .unwrap_or_default();
    REGEX_FOR_JS_STRINGS_IN_SCRIPTS.with(|regex| {
        js::reg_exp::replace(&escaped, regex, |r#match: &str| match r#match {
            // santizing breaking out of strings and script tags
            "&" => "\\u0026",
            ">" => "\\u003e",
            "<" => "\\u003c",
            "\u{2028}" => "\\u2028",
            "\u{2029}" => "\\u2029",
            _ => panic!(
                "escapeJSObjectForInstructionScripts encountered a match it does not know how to replace. this means the match regex and the replacement characters are no longer in sync. This is a bug in React"
            ),
        })
    })
}

thread_local! {
    static LATE_STYLE_TAG_RESOURCE_OPEN1: &'static PrecomputedChunk =
        string_to_precomputed_chunk(" media=\"not all\" data-precedence=\"");
    static LATE_STYLE_TAG_RESOURCE_OPEN2: &'static PrecomputedChunk = string_to_precomputed_chunk("\" data-href=\"");
    static LATE_STYLE_TAG_RESOURCE_OPEN3: &'static PrecomputedChunk = string_to_precomputed_chunk("\">");
    static LATE_STYLE_TAG_TEMPLATE_CLOSE: &'static PrecomputedChunk = string_to_precomputed_chunk("</style>");

    // Tracks whether the boundary currently flushing is flushign style tags or has any
    // stylesheet dependencies not flushed in the Preamble.
    static CURRENTLY_RENDERING_BOUNDARY_HAS_STYLES_TO_HOIST: Cell<bool> = const { Cell::new(false) };

    // Acts as a return value for the forEach execution of style tag flushing.
    static DESTINATION_HAS_CAPACITY: Cell<bool> = const { Cell::new(true) };
}

fn flush_style_tags_late_for_boundary(destination: &Destination, style_queue: &StyleQueue) {
    let rules = &style_queue.rules;
    let hrefs = &style_queue.hrefs;
    if __DEV__ && !rules.borrow().is_empty() && hrefs.borrow().is_empty() {
        webapi::console::error((
            "React expected to have at least one href for an a hoistable style but found none. This is a bug in React.",
        ));
    }
    if !hrefs.borrow().is_empty() {
        let render_state = CURRENTLY_FLUSHING_RENDER_STATE
            .get()
            .expect("styles flush while a render state flushes");
        write_chunk(destination, render_state.start_inline_style);
        write_chunk(destination, chunk(&LATE_STYLE_TAG_RESOURCE_OPEN1));
        write_chunk(destination, style_queue.precedence);
        write_chunk(destination, chunk(&LATE_STYLE_TAG_RESOURCE_OPEN2));
        let mut i = 0;
        while i + 1 < hrefs.borrow().len() {
            write_chunk(destination, hrefs.borrow()[i]);
            write_chunk(destination, chunk(&SPACE_SEPARATOR));
            i += 1;
        }
        write_chunk(destination, hrefs.borrow()[i]);
        write_chunk(destination, chunk(&LATE_STYLE_TAG_RESOURCE_OPEN3));
        for &rule in rules.borrow().iter() {
            write_chunk(destination, rule);
        }
        DESTINATION_HAS_CAPACITY.set(write_chunk_and_return(
            destination,
            chunk(&LATE_STYLE_TAG_TEMPLATE_CLOSE),
        ));

        // We wrote style tags for this boundary and we may need to emit a script
        // to hoist them.
        CURRENTLY_RENDERING_BOUNDARY_HAS_STYLES_TO_HOIST.set(true);

        // style resources can flush continuously since more rules may be written into
        // them with new hrefs. Instead of marking it flushed, we simply reset the chunks
        // and hrefs
        rules.borrow_mut().clear();
        hrefs.borrow_mut().clear();
    }
}

fn has_styles_to_hoist(stylesheet: &StylesheetResource) -> bool {
    // We need to reveal boundaries with styles whenever a stylesheet it depends on is either
    // not flushed or flushed after the preamble (shell).
    if stylesheet.state.get() != PREAMBLE {
        CURRENTLY_RENDERING_BOUNDARY_HAS_STYLES_TO_HOIST.set(true);
        return true;
    }
    false
}

pub fn write_hoistables_for_boundary(
    destination: &Destination,
    hoistable_state: &HoistableState,
    render_state: &'static RenderState,
) -> bool {
    // Reset these on each invocation, they are only safe to read in this function
    CURRENTLY_RENDERING_BOUNDARY_HAS_STYLES_TO_HOIST.set(false);
    DESTINATION_HAS_CAPACITY.set(true);

    // Flush style tags for each precedence this boundary depends on
    CURRENTLY_FLUSHING_RENDER_STATE.set(Some(render_state));
    hoistable_state
        .styles
        .for_each(|style_queue| flush_style_tags_late_for_boundary(destination, style_queue));
    CURRENTLY_FLUSHING_RENDER_STATE.set(None);

    // Determine if this boundary has stylesheets that need to be awaited upon completion
    hoistable_state.stylesheets.for_each(|stylesheet| {
        has_styles_to_hoist(stylesheet);
    });

    // We don't actually want to flush any hoistables until the boundary is complete so we omit
    // any further writing here. This is because unlike Resources, Hoistable Elements act more like
    // regular elements, each rendered element has a unique representation in the DOM. We don't want
    // these elements to appear in the DOM early, before the boundary has actually completed

    if CURRENTLY_RENDERING_BOUNDARY_HAS_STYLES_TO_HOIST.get() {
        render_state.styles_to_hoist.set(true);
    }
    DESTINATION_HAS_CAPACITY.get()
}

fn flush_resource(destination: &Destination, resource: &Resource) {
    for &chunk in resource.borrow().iter() {
        write_chunk(destination, chunk);
    }
    resource.borrow_mut().clear();
}

thread_local! {
    static STYLESHEET_FLUSHING_QUEUE: Chunks = const { RefCell::new(Vec::new()) };
}

fn flush_style_in_preamble(destination: &Destination, stylesheet: &StylesheetResource) {
    // We still need to encode stylesheet chunks
    // because unlike most Hoistables and Resources we do not eagerly encode
    // them during render. This is because if we flush late we have to send a
    // different encoding and we don't want to encode multiple times
    STYLESHEET_FLUSHING_QUEUE.with(|stylesheet_flushing_queue| {
        push_link_impl(stylesheet_flushing_queue, stylesheet.props);
        for &chunk in stylesheet_flushing_queue.borrow().iter() {
            write_chunk(destination, chunk);
        }
        stylesheet_flushing_queue.borrow_mut().clear();
    });
    stylesheet.state.set(PREAMBLE);
}

thread_local! {
    static STYLE_TAG_RESOURCE_OPEN1: &'static PrecomputedChunk = string_to_precomputed_chunk(" data-precedence=\"");
    static STYLE_TAG_RESOURCE_OPEN2: &'static PrecomputedChunk = string_to_precomputed_chunk("\" data-href=\"");
    static SPACE_SEPARATOR: &'static PrecomputedChunk = string_to_precomputed_chunk(" ");
    static STYLE_TAG_RESOURCE_OPEN3: &'static PrecomputedChunk = string_to_precomputed_chunk("\">");

    static STYLE_TAG_RESOURCE_CLOSE: &'static PrecomputedChunk = string_to_precomputed_chunk("</style>");
}

fn flush_styles_in_preamble(destination: &Destination, style_queue: &StyleQueue) {
    let has_stylesheets = style_queue.sheets.size() > 0;
    style_queue
        .sheets
        .for_each(|stylesheet, _, _| flush_style_in_preamble(destination, &stylesheet));
    style_queue.sheets.clear();

    let rules = &style_queue.rules;
    let hrefs = &style_queue.hrefs;
    // If we don't emit any stylesheets at this precedence we still need to maintain the precedence
    // order so even if there are no rules for style tags at this precedence we emit an empty style
    // tag with the data-precedence attribute
    if !has_stylesheets || !hrefs.borrow().is_empty() {
        let render_state = CURRENTLY_FLUSHING_RENDER_STATE
            .get()
            .expect("styles flush while a render state flushes");
        write_chunk(destination, render_state.start_inline_style);
        write_chunk(destination, chunk(&STYLE_TAG_RESOURCE_OPEN1));
        write_chunk(destination, style_queue.precedence);
        if !hrefs.borrow().is_empty() {
            write_chunk(destination, chunk(&STYLE_TAG_RESOURCE_OPEN2));
            let mut i = 0;
            while i + 1 < hrefs.borrow().len() {
                write_chunk(destination, hrefs.borrow()[i]);
                write_chunk(destination, chunk(&SPACE_SEPARATOR));
                i += 1;
            }
            write_chunk(destination, hrefs.borrow()[i]);
        }
        write_chunk(destination, chunk(&STYLE_TAG_RESOURCE_OPEN3));
        for &rule in rules.borrow().iter() {
            write_chunk(destination, rule);
        }
        write_chunk(destination, chunk(&STYLE_TAG_RESOURCE_CLOSE));

        // style resources can flush continuously since more rules may be written into
        // them with new hrefs. Instead of marking it flushed, we simply reset the chunks
        // and hrefs
        rules.borrow_mut().clear();
        hrefs.borrow_mut().clear();
    }
}

fn preload_late_style(destination: &Destination, stylesheet: &StylesheetResource) {
    if stylesheet.state.get() == PENDING {
        stylesheet.state.set(PRELOADED);
        let preload_props =
            preload_as_style_props_from_props(js::get(stylesheet.props, "href"), stylesheet.props);
        STYLESHEET_FLUSHING_QUEUE.with(|stylesheet_flushing_queue| {
            push_link_impl(stylesheet_flushing_queue, preload_props);
            for &chunk in stylesheet_flushing_queue.borrow().iter() {
                write_chunk(destination, chunk);
            }
            stylesheet_flushing_queue.borrow_mut().clear();
        });
    }
}

fn preload_late_styles(destination: &Destination, style_queue: &StyleQueue) {
    style_queue
        .sheets
        .for_each(|stylesheet, _, _| preload_late_style(destination, &stylesheet));
    style_queue.sheets.clear();
}

thread_local! {
    static BLOCKING_RENDER_CHUNK_START: &'static PrecomputedChunk =
        string_to_precomputed_chunk("<link rel=\"expect\" href=\"#");
    static BLOCKING_RENDER_CHUNK_END: &'static PrecomputedChunk = string_to_precomputed_chunk("\" blocking=\"render\"/>");
}

fn write_blocking_render_instruction(
    destination: &Destination,
    resumable_state: &ResumableState,
    _render_state: &RenderState,
) {
    if ENABLE_FIZZ_BLOCKING_RENDER.with(|enabled| *enabled) {
        let id_prefix = &resumable_state.id_prefix;
        let shell_id = "_".to_owned() + id_prefix + "R_";
        write_chunk(destination, chunk(&BLOCKING_RENDER_CHUNK_START));
        write_chunk(
            destination,
            string_to_chunk(&escape_text_for_browser(Some(js::unknown(
                shell_id.as_str(),
            )))),
        );
        write_chunk(destination, chunk(&BLOCKING_RENDER_CHUNK_END));
    }
}

thread_local! {
    static COMPLETED_SHELL_ID_ATTRIBUTE_START: &'static PrecomputedChunk = string_to_precomputed_chunk(" id=\"");
}

fn write_completed_shell_id_attribute(destination: &Destination, resumable_state: &ResumableState) {
    if (resumable_state.instructions.get() & SENT_COMPLETED_SHELL_ID) != NOTHING_SENT {
        return;
    }
    resumable_state
        .instructions
        .set(resumable_state.instructions.get() | SENT_COMPLETED_SHELL_ID);
    let id_prefix = &resumable_state.id_prefix;
    let shell_id = "_".to_owned() + id_prefix + "R_";
    write_chunk(destination, chunk(&COMPLETED_SHELL_ID_ATTRIBUTE_START));
    write_chunk(
        destination,
        string_to_chunk(&escape_text_for_browser(Some(js::unknown(
            shell_id.as_str(),
        )))),
    );
    write_chunk(destination, chunk(&ATTRIBUTE_END));
}

fn push_completed_shell_id_attribute(target: &Chunks, resumable_state: &ResumableState) {
    if (resumable_state.instructions.get() & SENT_COMPLETED_SHELL_ID) != NOTHING_SENT {
        return;
    }
    resumable_state
        .instructions
        .set(resumable_state.instructions.get() | SENT_COMPLETED_SHELL_ID);
    let id_prefix = &resumable_state.id_prefix;
    let shell_id = "_".to_owned() + id_prefix + "R_";
    target.borrow_mut().extend([
        chunk(&COMPLETED_SHELL_ID_ATTRIBUTE_START),
        string_to_chunk(&escape_text_for_browser(Some(js::unknown(
            shell_id.as_str(),
        )))),
        chunk(&ATTRIBUTE_END),
    ]);
}

/// Each of `chunks` written, and then none left.
fn flush_chunks(destination: &Destination, chunks: &Chunks) {
    for &chunk in chunks.borrow().iter() {
        write_chunk(destination, chunk);
    }
    chunks.borrow_mut().clear();
}

/// Each of `resources` flushed, and then none of them held.
fn flush_resources_of(destination: &Destination, resources: &js::Set<Rc<Resource>>) {
    resources.for_each(|resource| flush_resource(destination, resource));
    resources.clear();
}

// We don't bother reporting backpressure at the moment because we expect to
// flush the entire preamble in a single pass. This probably should be modified
// in the future to be backpressure sensitive but that requires a larger refactor
// of the flushing code in Fizz.
pub fn write_preamble_start(
    destination: &Destination,
    resumable_state: &ResumableState,
    render_state: &'static RenderState,
    skip_blocking_shell: bool,
) {
    // This function must be called exactly once on every request
    if ENABLE_FIZZ_EXTERNAL_RUNTIME.with(|enabled| *enabled)
        && let Some(external_runtime_script) = &render_state.external_runtime_script
    {
        // If the root segment is incomplete due to suspended tasks
        // (e.g. willFlushAllSegments = false) and we are using data
        // streaming format, ensure the external runtime is sent.
        // (User code could choose to send this even earlier by calling
        //  preinit(...), if they know they will suspend).
        let ExternalRuntimeScript { src, chunks } = external_runtime_script;
        internal_preinit_script(resumable_state, render_state, src, chunks);
    }

    let preamble = &render_state.preamble;

    let html_chunks = preamble.html_chunks.borrow().clone();
    let head_chunks = preamble.head_chunks.borrow().clone();

    // Emit open tags before Hoistables and Resources
    if let Some(html_chunks) = &html_chunks {
        // We have an <html> to emit as part of the preamble
        for &chunk in html_chunks.borrow().iter() {
            write_chunk(destination, chunk);
        }
        if let Some(head_chunks) = &head_chunks {
            for &chunk in head_chunks.borrow().iter() {
                write_chunk(destination, chunk);
            }
        } else {
            // We did not render a head but we emitted an <html> so we emit one now
            write_chunk(destination, start_chunk_for_tag("head"));
            write_chunk(destination, chunk(&END_OF_START_TAG));
        }
    } else if let Some(head_chunks) = &head_chunks {
        // We do not have an <html> but we do have a <head>
        for &chunk in head_chunks.borrow().iter() {
            write_chunk(destination, chunk);
        }
    }

    // Emit high priority Hoistables
    flush_chunks(destination, &render_state.charset_chunks);

    // emit preconnect resources
    flush_resources_of(destination, render_state.preconnects);

    flush_chunks(destination, &render_state.viewport_chunks);

    flush_resources_of(destination, render_state.font_preloads);

    flush_resources_of(destination, render_state.high_image_preloads);

    // Flush unblocked stylesheets by precedence
    CURRENTLY_FLUSHING_RENDER_STATE.set(Some(render_state));
    render_state
        .styles
        .for_each(|style_queue, _, _| flush_styles_in_preamble(destination, &style_queue));
    CURRENTLY_FLUSHING_RENDER_STATE.set(None);

    flush_chunks(destination, &render_state.import_map_chunks);

    render_state
        .bootstrap_scripts
        .for_each(|resource| flush_resource(destination, resource));

    flush_resources_of(destination, render_state.scripts);

    flush_resources_of(destination, render_state.bulk_preloads);

    if (html_chunks.is_some() || head_chunks.is_some()) && !skip_blocking_shell {
        // If we have any html or head chunks we know that we're rendering a full document.
        // A full document should block display until the full shell has downloaded.
        // Therefore we insert a render blocking instruction referring to the last body
        // element that's considered part of the shell. We do this after the important loads
        // have already been emitted so we don't do anything to delay them but early so that
        // the browser doesn't risk painting too early.
        write_blocking_render_instruction(destination, resumable_state, render_state);
    } else {
        // We don't need to add the shell id so mark it as if sent.
        // Currently it might still be sent if it was already added to a bootstrap script.
        resumable_state
            .instructions
            .set(resumable_state.instructions.get() | SENT_COMPLETED_SHELL_ID);
    }

    // Write embedding hoistableChunks
    flush_chunks(destination, &render_state.hoistable_chunks);
}

// We don't bother reporting backpressure at the moment because we expect to
// flush the entire preamble in a single pass. This probably should be modified
// in the future to be backpressure sensitive but that requires a larger refactor
// of the flushing code in Fizz.
pub fn write_preamble_end(destination: &Destination, render_state: &RenderState) {
    let preamble = &render_state.preamble;
    if preamble.html_chunks.borrow().is_some() || preamble.head_chunks.borrow().is_some() {
        // we have an <html> but we inserted an implicit <head> tag. We need
        // to close it since the main content won't have it
        write_chunk(destination, end_chunk_for_tag("head"));
    }

    if let Some(body_chunks) = preamble.body_chunks.borrow().as_ref() {
        for &chunk in body_chunks.borrow().iter() {
            write_chunk(destination, chunk);
        }
    }
}

// We don't bother reporting backpressure at the moment because we expect to
// flush the entire preamble in a single pass. This probably should be modified
// in the future to be backpressure sensitive but that requires a larger refactor
// of the flushing code in Fizz.
pub fn write_hoistables(
    destination: &Destination,
    _resumable_state: &ResumableState,
    render_state: &RenderState,
) {
    // Emit high priority Hoistables

    // We omit charsetChunks because we have already sent the shell and if it wasn't
    // already sent it is too late now.

    flush_chunks(destination, &render_state.viewport_chunks);

    flush_resources_of(destination, render_state.preconnects);

    flush_resources_of(destination, render_state.font_preloads);

    flush_resources_of(destination, render_state.high_image_preloads);

    // Preload any stylesheets. these will emit in a render instruction that follows this
    // but we want to kick off preloading as soon as possible
    render_state
        .styles
        .for_each(|style_queue, _, _| preload_late_styles(destination, &style_queue));

    // We only hoist importmaps that are configured through createResponse and that will
    // always flush in the preamble. Generally we don't expect people to render them as
    // tags when using React but if you do they are going to be treated like regular inline
    // scripts and flush after other hoistables which is problematic

    // bootstrap scripts should flush above script priority but these can only flush in the preamble
    // so we elide the code here for performance

    flush_resources_of(destination, render_state.scripts);

    flush_resources_of(destination, render_state.bulk_preloads);

    // Write embedding hoistableChunks
    flush_chunks(destination, &render_state.hoistable_chunks);
}

pub fn write_postamble(destination: &Destination, resumable_state: &ResumableState) {
    if resumable_state.has_body.get() {
        write_chunk(destination, end_chunk_for_tag("body"));
    }
    if resumable_state.has_html.get() {
        write_chunk(destination, end_chunk_for_tag("html"));
    }
}

thread_local! {
    static ARRAY_FIRST_OPEN_BRACKET: &'static PrecomputedChunk = string_to_precomputed_chunk("[");
    static ARRAY_SUBSEQUENT_OPEN_BRACKET: &'static PrecomputedChunk = string_to_precomputed_chunk(",[");
    static ARRAY_INTERSTITIAL: &'static PrecomputedChunk = string_to_precomputed_chunk(",");
    static ARRAY_CLOSE_BRACKET: &'static PrecomputedChunk = string_to_precomputed_chunk("]");
}

/// Text in a script: its JSON, escaped for an inline script.
fn encode_in_js(text: &str) -> String {
    escape_js_object_for_instruction_scripts(js::unknown(text))
}

/// Text in an attribute: its JSON, escaped as HTML.
fn encode_in_attr(text: &str) -> String {
    escape_text_for_browser(Some(js::unknown(js::json::stringify(text).as_str())))
}

// This function writes a 2D array of strings to be embedded in javascript.
// E.g.
//  [["JS_escaped_string1", "JS_escaped_string2"]]
fn write_style_resource_dependencies_in_js(
    destination: &Destination,
    hoistable_state: &HoistableState,
) {
    write_style_resource_dependencies(destination, hoistable_state, encode_in_js);
}

// This function writes a 2D array of strings to be embedded in an attribute
// value and read with JSON.parse in ReactDOMServerExternalRuntime.js
// E.g.
//  [[&quot;JSON_escaped_string1&quot;, &quot;JSON_escaped_string2&quot;]]
fn write_style_resource_dependencies_in_attr(
    destination: &Destination,
    hoistable_state: &HoistableState,
) {
    write_style_resource_dependencies(destination, hoistable_state, encode_in_attr);
}

fn write_style_resource_dependencies(
    destination: &Destination,
    hoistable_state: &HoistableState,
    encode: fn(&str) -> String,
) {
    write_chunk(destination, chunk(&ARRAY_FIRST_OPEN_BRACKET));

    let next_array_open_brack_chunk = Cell::new(chunk(&ARRAY_FIRST_OPEN_BRACKET));
    hoistable_state.stylesheets.for_each(|resource| {
        if resource.state.get() == PREAMBLE {
            // We can elide this dependency because it was flushed in the shell and
            // should be ready before content is shown on the client
        } else if resource.state.get() == LATE {
            // We only need to emit the href because this resource flushed in an earlier
            // boundary already which encoded the attributes necessary to construct
            // the resource instance on the client.
            write_chunk(destination, next_array_open_brack_chunk.get());
            write_style_resource_dependency_href_only(
                destination,
                js::get(resource.props, "href"),
                encode,
            );
            write_chunk(destination, chunk(&ARRAY_CLOSE_BRACKET));
            next_array_open_brack_chunk.set(chunk(&ARRAY_SUBSEQUENT_OPEN_BRACKET));
        } else {
            // We need to emit the whole resource for insertion on the client
            write_chunk(destination, next_array_open_brack_chunk.get());
            write_style_resource_dependency(
                destination,
                js::get(resource.props, "href"),
                js::get(resource.props, "data-precedence"),
                resource.props,
                encode,
            );
            write_chunk(destination, chunk(&ARRAY_CLOSE_BRACKET));
            next_array_open_brack_chunk.set(chunk(&ARRAY_SUBSEQUENT_OPEN_BRACKET));

            resource.state.set(LATE);
        }
    });
    write_chunk(destination, chunk(&ARRAY_CLOSE_BRACKET));
}

/* Helper functions */
fn write_style_resource_dependency_href_only(
    destination: &Destination,
    href: Option<&Unknown>,
    encode: fn(&str) -> String,
) {
    // We should actually enforce this earlier when the resource is created but for
    // now we make sure we are actually dealing with a string here.
    if __DEV__ {
        check_attribute_string_coercion(href, "href");
    }
    let coerced_href = js::concat_text(href).unwrap();
    write_chunk(destination, string_to_chunk(&encode(&coerced_href)));
}

fn write_style_resource_dependency(
    destination: &Destination,
    href: Option<&'static Unknown>,
    precedence: Option<&Unknown>,
    props: &Unknown,
    encode: fn(&str) -> String,
) {
    let coerced_href = sanitize_url(Some(js::unknown_of(js::concat_text(href).unwrap())));
    write_chunk(
        destination,
        string_to_chunk(&encode(&js::concat_text(coerced_href).unwrap())),
    );

    if __DEV__ {
        check_attribute_string_coercion(precedence, "precedence");
    }
    let coerced_precedence = js::concat_text(precedence).unwrap();
    write_chunk(destination, chunk(&ARRAY_INTERSTITIAL));
    write_chunk(destination, string_to_chunk(&encode(&coerced_precedence)));

    for prop_key in js::keys_in(props) {
        if HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(props, &prop_key)) {
            let Some(prop_value) = js::get(props, &prop_key) else {
                continue;
            };
            match prop_key.as_str() {
                "href" | "rel" | "precedence" | "data-precedence" => {}
                "children" | "dangerouslySetInnerHTML" => panic!(
                    "{} is a self-closing tag and must neither have `children` nor use `dangerouslySetInnerHTML`.",
                    "link"
                ),
                _ => write_style_resource_attribute(destination, &prop_key, prop_value, encode),
            }
        }
    }
}

fn write_style_resource_attribute(
    destination: &Destination,
    name: &str,
    value: &'static Unknown, // not null or undefined
    encode: fn(&str) -> String,
) {
    let mut attribute_name = name.to_lowercase();
    if matches!(
        js::classify(Some(value)),
        Kind::Function(_) | Kind::Symbol(_)
    ) {
        return;
    }

    let attribute_value = match name {
        // Reserved names
        "innerHTML"
        | "dangerouslySetInnerHTML"
        | "suppressContentEditableWarning"
        | "suppressHydrationWarning"
        | "style"
        | "ref" => {
            // Ignored
            return;
        }

        // Attribute renames
        "className" => {
            attribute_name = "class".to_owned();
            if __DEV__ {
                check_attribute_string_coercion(Some(value), &attribute_name);
            }
            js::concat_text(Some(value)).unwrap()
        }
        // Booleans
        "hidden" => {
            if js::strict_equals(Some(value), false) {
                return;
            }
            String::new()
        }
        // Santized URLs
        "src" | "href" => {
            let value = sanitize_url(Some(value));
            if __DEV__ {
                check_attribute_string_coercion(value, &attribute_name);
            }
            js::concat_text(value).unwrap()
        }
        _ => {
            if
            // unrecognized event handlers are not SSR'd and we (apparently)
            // use on* as hueristic for these handler props
            js::string::length(name) > 2
                && (js::string::get(name, 0.0) == Some("o")
                    || js::string::get(name, 0.0) == Some("O"))
                && (js::string::get(name, 1.0) == Some("n")
                    || js::string::get(name, 1.0) == Some("N"))
            {
                return;
            }
            if !is_attribute_name_safe(name) {
                return;
            }
            if __DEV__ {
                check_attribute_string_coercion(Some(value), &attribute_name);
            }
            js::concat_text(Some(value)).unwrap()
        }
    };
    write_chunk(destination, chunk(&ARRAY_INTERSTITIAL));
    write_chunk(destination, string_to_chunk(&encode(&attribute_name)));
    write_chunk(destination, chunk(&ARRAY_INTERSTITIAL));
    write_chunk(destination, string_to_chunk(&encode(&attribute_value)));
}

pub fn create_hoistable_state() -> HoistableState {
    HoistableState {
        styles: js::set::new(None),
        stylesheets: js::set::new(None),
        suspensey_images: Cell::new(false),
    }
}

fn get_resource_key(href: &str) -> String {
    href.to_owned()
}

fn get_image_resource_key(
    href: Option<&Unknown>,
    image_src_set: Option<&Unknown>,
    image_sizes: Option<&Unknown>,
) -> String {
    if js::truthy(image_src_set) {
        return js::concat_text(image_src_set).unwrap()
            + "\n"
            + &(if js::truthy(image_sizes) {
                js::concat_text(image_sizes).unwrap()
            } else {
                String::new()
            });
    }
    js::concat_text(href).unwrap()
}

thread_local! {
    static PREVIOUS_DISPATCHER: &'static HostDispatcher =
        REACT_DOM_SHARED_INTERNALS.d(); /* ReactDOMCurrentDispatcher */
}

js::on_load! {
    REACT_DOM_SHARED_INTERNALS /* ReactDOMCurrentDispatcher */
        .set_d(HostDispatcher {
            f /* flushSyncWork */: PREVIOUS_DISPATCHER.with(|previous_dispatcher| previous_dispatcher.f), /* flushSyncWork */
            r /* requestFormReset */: PREVIOUS_DISPATCHER.with(|previous_dispatcher| previous_dispatcher.r), /* requestFormReset */
            prefetch_dns: &prefetch_dns,
            preconnect: &preconnect,
            preload: &preload,
            m: &preload_module,
            preinit_script: &preinit_script,
            preinit_style: &preinit_style,
            preinit_module_script: &preinit_module_script,
        });
}

/// Consumes a header's length of the headers' capacity, as each one found
/// does, whether it fits or not: the headers, where it fits.
fn fitting_header(
    headers: Option<&Headers>,
    make: impl FnOnce() -> String,
) -> Option<(&Headers, String)> {
    let headers = headers.filter(|headers| headers.remaining_capacity.get() > 0.0)?;
    // We always consume the header length since once we find one header that doesn't fit
    // we assume all the rest won't as well. This is to avoid getting into a situation
    // where we have a very small remaining capacity but no headers will ever fit and we end
    // up constantly trying to see if the next resource might make it. In the future we can
    // make this behavior different between render and prerender since in the latter case
    // we are less sensitive to the current requests runtime per and more sensitive to maximizing
    // headers.
    let header = make();
    let remaining =
        headers.remaining_capacity.get() - (f64::from(js::string::length(&header)) + 2.0);
    headers.remaining_capacity.set(remaining);
    (remaining >= 0.0).then_some((headers, header))
}

/// `header` added to a list of headers, `, ` between them.
fn append_header(list: &RefCell<String>, header: &str) {
    if !list.borrow().is_empty() {
        list.borrow_mut().push_str(", ");
    }
    list.borrow_mut().push_str(header);
}

struct PreconnectProps<'a> {
    rel: &'a str,
    href: &'a str,
    cross_origin: Option<&'a Unknown>,
}

unsafe impl js::Defined for PreconnectProps<'_> {}

#[cfg_attr(rust_js, rust_js::name = "prefetchDNS")]
fn prefetch_dns(href: &str) {
    let Some(request) = resolve_request() else {
        // In async contexts we can sometimes resolve resources from AsyncLocalStorage. If we can't we can also
        // possibly get them from the stack if we are not in an async context. Since we were not able to resolve
        // the resources for this call in either case we opt to do nothing. We can consider making this a warning
        // but there may be times where calling a function outside of render is intentional (i.e. to warm up data
        // fetching) and we don't want to warn in those cases.
        (PREVIOUS_DISPATCHER.with(|previous_dispatcher| previous_dispatcher.prefetch_dns))(href);
        return;
    };
    let resumable_state = get_resumable_state(request);
    let render_state = get_render_state(request);

    if !href.is_empty() {
        let key = get_resource_key(href);
        let dns_resources = resumable_state.dns_resources.get();
        if !js::unknown(dns_resources).has_own_property(&key) {
            dict::set(dns_resources, &key, EXISTS.with(|exists| *exists));

            // Compute the header since we might be able to fit it in the max length
            if let Some((headers, header)) =
                fitting_header(render_state.headers.borrow().clone().as_deref(), || {
                    get_prefetch_dns_as_header(href)
                })
            {
                // Store this as resettable in case we are prerendering and postpone in the Shell
                dict::set(render_state.resets.dns, &key, EXISTS.with(|exists| *exists));
                append_header(&headers.preconnects, &header);
            } else {
                // Encode as element
                let resource = Rc::new(RefCell::new(Vec::new()));
                push_link_impl(
                    &resource,
                    js::unknown(&DnsPrefetchProps {
                        href,
                        rel: "dns-prefetch",
                    }),
                );
                render_state.preconnects.add(&resource);
            }
        }
        flush_resources(request);
    }
}

struct DnsPrefetchProps<'a> {
    href: &'a str,
    rel: &'a str,
}

unsafe impl js::Defined for DnsPrefetchProps<'_> {}

fn preconnect(href: &str, cross_origin: Option<&'static Unknown>) {
    let Some(request) = resolve_request() else {
        // In async contexts we can sometimes resolve resources from AsyncLocalStorage. If we can't we can also
        // possibly get them from the stack if we are not in an async context. Since we were not able to resolve
        // the resources for this call in either case we opt to do nothing. We can consider making this a warning
        // but there may be times where calling a function outside of render is intentional (i.e. to warm up data
        // fetching) and we don't want to warn in those cases.
        (PREVIOUS_DISPATCHER.with(|previous_dispatcher| previous_dispatcher.preconnect))(
            href,
            cross_origin,
        );
        return;
    };
    let resumable_state = get_resumable_state(request);
    let render_state = get_render_state(request);

    if !href.is_empty() {
        let bucket = if js::strict_equals(cross_origin, "use-credentials") {
            "credentials"
        } else if js::type_of(cross_origin) == "string" {
            "anonymous"
        } else {
            "default"
        };
        let bucket_of = |connect: &ConnectResources| match bucket {
            "credentials" => connect.credentials,
            "anonymous" => connect.anonymous,
            _ => connect.default,
        };
        let key = get_resource_key(href);
        let resources = bucket_of(&resumable_state.connect_resources.borrow());
        if !js::unknown(resources).has_own_property(&key) {
            dict::set(resources, &key, EXISTS.with(|exists| *exists));

            // Compute the header since we might be able to fit it in the max length
            if let Some((headers, header)) =
                fitting_header(render_state.headers.borrow().clone().as_deref(), || {
                    get_preconnect_as_header(href, cross_origin)
                })
            {
                // Store this in resettableState in case we are prerending and postpone in the Shell
                dict::set(
                    bucket_of(&render_state.resets.connect),
                    &key,
                    EXISTS.with(|exists| *exists),
                );
                append_header(&headers.preconnects, &header);
            } else {
                let resource = Rc::new(RefCell::new(Vec::new()));
                push_link_impl(
                    &resource,
                    js::unknown(&PreconnectProps {
                        rel: "preconnect",
                        href,
                        cross_origin,
                    }),
                );
                render_state.preconnects.add(&resource);
            }
        }
        flush_resources(request);
    }
}

/// The props a preload's `<link>` starts with, which its options extend.
struct PreloadBaseProps<'a> {
    rel: &'a str,
    href: Option<&'a str>,
    r#as: &'a str,
}

unsafe impl js::Defined for PreloadBaseProps<'_> {}

/// A preload's credentials, `[crossOrigin, integrity]`, where its options give
/// either as text; else none to convey.
fn preload_state_of(options: Option<&Unknown>) -> Option<&'static Unknown> {
    if js::truthy(options)
        && (js::type_of(js::get(options, "crossOrigin")) == "string"
            || js::type_of(js::get(options, "integrity")) == "string")
    {
        Some(js::unknown_of(vec![
            js::get(options, "crossOrigin"),
            js::get(options, "integrity"),
        ]))
    } else {
        Some(PRELOAD_NO_CREDS.with(|preload| *preload))
    }
}

fn preload(href: &str, r#as: &str, options: Option<&'static Unknown>) {
    let Some(request) = resolve_request() else {
        // In async contexts we can sometimes resolve resources from AsyncLocalStorage. If we can't we can also
        // possibly get them from the stack if we are not in an async context. Since we were not able to resolve
        // the resources for this call in either case we opt to do nothing. We can consider making this a warning
        // but there may be times where calling a function outside of render is intentional (i.e. to warm up data
        // fetching) and we don't want to warn in those cases.
        (PREVIOUS_DISPATCHER.with(|previous_dispatcher| previous_dispatcher.preload))(
            href, r#as, options,
        );
        return;
    };
    let resumable_state = get_resumable_state(request);
    let render_state = get_render_state(request);
    if !r#as.is_empty() && !href.is_empty() {
        match r#as {
            "image" => {
                let (image_src_set, image_sizes, fetch_priority) = if js::truthy(options) {
                    (
                        js::get(options, "imageSrcSet"),
                        js::get(options, "imageSizes"),
                        js::get(options, "fetchPriority"),
                    )
                } else {
                    (None, None, None)
                };
                let key =
                    get_image_resource_key(Some(js::unknown(href)), image_src_set, image_sizes);
                let image_resources = resumable_state.image_resources.get();
                if js::unknown(image_resources).has_own_property(&key) {
                    // we can return if we already have this resource
                    return;
                }
                dict::set(
                    image_resources,
                    &key,
                    Some(PRELOAD_NO_CREDS.with(|preload| *preload)),
                );

                let headers = render_state.headers.borrow().clone();
                let headers = headers.as_deref().filter(|_| {
                    // browsers today don't support preloading responsive images from link headers so we bail out
                    // if the img has srcset defined
                    js::type_of(image_src_set) != "string"
                        // We only include high priority images in the link header
                        && js::strict_equals(fetch_priority, "high")
                });
                if let Some((headers, header)) = fitting_header(headers, || {
                    // Compute the header since we might be able to fit it in the max length
                    get_preload_as_header(Some(js::unknown(href)), r#as, options)
                }) {
                    // If we postpone in the shell we will still emit a preload as a header so we
                    // track this to make sure we don't reset it.
                    dict::set(
                        render_state.resets.image,
                        &key,
                        Some(PRELOAD_NO_CREDS.with(|preload| *preload)),
                    );
                    append_header(&headers.high_image_preloads, &header);
                } else {
                    // If we don't have headers to write to we have to encode as elements to flush in the head
                    // When we have imageSrcSet the browser probably cannot load the right version from headers
                    // (this should be verified by testing). For now we assume these need to go in the head
                    // as elements even if headers are available.
                    let resource = Rc::new(RefCell::new(Vec::new()));
                    let base = PreloadBaseProps {
                        rel: "preload",
                        // There is a bug in Safari where imageSrcSet is not respected on preload links
                        // so we omit the href here if we have imageSrcSet b/c safari will load the wrong image.
                        // This harms older browers that do not support imageSrcSet by making their preloads not work
                        // but this population is shrinking fast and is already small so we accept this tradeoff.
                        href: if js::truthy(image_src_set) {
                            None
                        } else {
                            Some(href)
                        },
                        r#as,
                    };
                    push_link_impl(
                        &resource,
                        js::object::assign(js::unknown(&base), (options,)),
                    );
                    if js::strict_equals(fetch_priority, "high") {
                        render_state.high_image_preloads.add(&resource);
                    } else {
                        render_state.bulk_preloads.add(&resource);
                        // Stash the resource in case we need to promote it to higher priority
                        // when an img tag is rendered
                        render_state.preloads.images.set(&key, resource);
                    }
                }
            }
            "style" => {
                let key = get_resource_key(href);
                let style_resources = resumable_state.style_resources.get();
                if js::unknown(style_resources).has_own_property(&key) {
                    // we can return if we already have this resource
                    return;
                }
                let resource = Rc::new(RefCell::new(Vec::new()));
                let base = PreloadBaseProps {
                    rel: "preload",
                    href: Some(href),
                    r#as,
                };
                push_link_impl(
                    &resource,
                    js::object::assign(js::unknown(&base), (options,)),
                );
                dict::set(style_resources, &key, preload_state_of(options));
                render_state
                    .preloads
                    .stylesheets
                    .set(&key, Rc::clone(&resource));
                render_state.bulk_preloads.add(&resource);
            }
            "script" => {
                let key = get_resource_key(href);
                let script_resources = resumable_state.script_resources.get();
                if js::unknown(script_resources).has_own_property(&key) {
                    // we can return if we already have this resource
                    return;
                }
                let resource = Rc::new(RefCell::new(Vec::new()));
                render_state
                    .preloads
                    .scripts
                    .set(&key, Rc::clone(&resource));
                render_state.bulk_preloads.add(&resource);
                let base = PreloadBaseProps {
                    rel: "preload",
                    href: Some(href),
                    r#as,
                };
                push_link_impl(
                    &resource,
                    js::object::assign(js::unknown(&base), (options,)),
                );
                dict::set(script_resources, &key, preload_state_of(options));
            }
            _ => {
                let key = get_resource_key(href);
                let unknown_resources = resumable_state.unknown_resources.get();
                let has_as_type = js::unknown(unknown_resources).has_own_property(r#as);
                let resources = if has_as_type {
                    let resources = *dict::get(unknown_resources, r#as).unwrap();
                    if js::unknown(resources).has_own_property(&key) {
                        // we can return if we already have this resource
                        return;
                    }
                    resources
                } else {
                    let resources = dict::new();
                    dict::set(unknown_resources, r#as, resources);
                    resources
                };
                dict::set(
                    resources,
                    &key,
                    Some(PRELOAD_NO_CREDS.with(|preload| *preload)),
                );

                let headers = render_state.headers.borrow().clone();
                let headers = headers.as_deref().filter(|_| r#as == "font");
                if let Some((headers, header)) = fitting_header(headers, || {
                    // We compute the header here because we might be able to fit it in the max length
                    get_preload_as_header(Some(js::unknown(href)), r#as, options)
                }) {
                    // If we postpone in the shell we will still emit this preload so we
                    // track it here to prevent it from being reset.
                    dict::set(
                        render_state.resets.font,
                        &key,
                        Some(PRELOAD_NO_CREDS.with(|preload| *preload)),
                    );
                    append_header(&headers.font_preloads, &header);
                } else {
                    // We either don't have headers or we are preloading something that does
                    // not warrant elevated priority so we encode as an element.
                    let resource = Rc::new(RefCell::new(Vec::new()));
                    let base = PreloadBaseProps {
                        rel: "preload",
                        href: Some(href),
                        r#as,
                    };
                    let props = js::object::assign(js::unknown(&base), (options,));
                    push_link_impl(&resource, props);
                    match r#as {
                        "font" => render_state.font_preloads.add(&resource),
                        // intentional fall through
                        _ => render_state.bulk_preloads.add(&resource),
                    }
                }
            }
        }
        // If we got this far we created a new resource
        flush_resources(request);
    }
}

struct ModulePreloadBaseProps<'a> {
    rel: &'a str,
    href: &'a str,
}

unsafe impl js::Defined for ModulePreloadBaseProps<'_> {}

fn preload_module(href: &str, options: Option<&'static Unknown>) {
    let Some(request) = resolve_request() else {
        // In async contexts we can sometimes resolve resources from AsyncLocalStorage. If we can't we can also
        // possibly get them from the stack if we are not in an async context. Since we were not able to resolve
        // the resources for this call in either case we opt to do nothing. We can consider making this a warning
        // but there may be times where calling a function outside of render is intentional (i.e. to warm up data
        // fetching) and we don't want to warn in those cases.
        (PREVIOUS_DISPATCHER.with(|previous_dispatcher| previous_dispatcher.m))(href, options);
        return;
    };
    let resumable_state = get_resumable_state(request);
    let render_state = get_render_state(request);
    if !href.is_empty() {
        let key = get_resource_key(href);
        let r#as = if js::truthy(options) && js::type_of(js::get(options, "as")) == "string" {
            js::concat_text(js::get(options, "as")).unwrap()
        } else {
            "script".to_owned()
        };

        let resource = match r#as.as_str() {
            "script" => {
                let module_script_resources = resumable_state.module_script_resources.get();
                if js::unknown(module_script_resources).has_own_property(&key) {
                    // we can return if we already have this resource
                    return;
                }
                let resource = Rc::new(RefCell::new(Vec::new()));
                dict::set(module_script_resources, &key, preload_state_of(options));
                render_state
                    .preloads
                    .module_scripts
                    .set(&key, Rc::clone(&resource));
                resource
            }
            _ => {
                let module_unknown_resources = resumable_state.module_unknown_resources.get();
                let has_as_type = js::unknown(module_unknown_resources).has_own_property(&r#as);
                let resources = if has_as_type {
                    let resources = *dict::get(module_unknown_resources, &r#as).unwrap();
                    if js::unknown(resources).has_own_property(&key) {
                        // we can return if we already have this resource
                        return;
                    }
                    resources
                } else {
                    let resources = dict::new();
                    dict::set(module_unknown_resources, &r#as, resources);
                    resources
                };
                let resource = Rc::new(RefCell::new(Vec::new()));
                dict::set(
                    resources,
                    &key,
                    Some(PRELOAD_NO_CREDS.with(|preload| *preload)),
                );
                resource
            }
        };

        push_link_impl(
            &resource,
            js::object::assign(
                js::unknown(&ModulePreloadBaseProps {
                    rel: "modulepreload",
                    href,
                }),
                (options,),
            ),
        );
        render_state.bulk_preloads.add(&resource);
        // If we got this far we created a new resource
        flush_resources(request);
    }
}

/// The props a preinit's stylesheet starts with, which its options extend.
struct PreinitStylesheetProps {
    rel: &'static str,
    href: String,
    #[cfg_attr(rust_js, rust_js::name = "data-precedence")]
    data_precedence: String,
}

unsafe impl js::Defined for PreinitStylesheetProps {}

/// A preload's state, where `resource_state` is one: its credentials, to
/// adopt, if it has them.
fn preload_credentials(
    resource_state: &'static Unknown,
) -> Option<&'static [Option<&'static Unknown>]> {
    // When resourceState is truty it is a Preload state. We cast it for clarity
    let preload_state: &[Option<&'static Unknown>] = unsafe { js::cast(Some(resource_state)) };
    (preload_state.len() == 2).then_some(preload_state)
}

fn preinit_style(
    href: &str,
    precedence: Option<&'static Unknown>,
    options: Option<&'static Unknown>,
) {
    let Some(request) = resolve_request() else {
        // In async contexts we can sometimes resolve resources from AsyncLocalStorage. If we can't we can also
        // possibly get them from the stack if we are not in an async context. Since we were not able to resolve
        // the resources for this call in either case we opt to do nothing. We can consider making this a warning
        // but there may be times where calling a function outside of render is intentional (i.e. to warm up data
        // fetching) and we don't want to warn in those cases.
        (PREVIOUS_DISPATCHER.with(|previous_dispatcher| previous_dispatcher.preinit_style))(
            href, precedence, options,
        );
        return;
    };
    let resumable_state = get_resumable_state(request);
    let render_state = get_render_state(request);
    if !href.is_empty() {
        let precedence = if js::truthy(precedence) {
            js::concat_text(precedence).unwrap()
        } else {
            "default".to_owned()
        };
        let key = get_resource_key(href);

        let style_queue = render_state.styles.get(&precedence);
        let style_resources = resumable_state.style_resources.get();
        let has_key = js::unknown(style_resources).has_own_property(&key);
        let resource_state = if has_key {
            js::get(style_resources, &key)
        } else {
            None
        };
        if !exists(style_resources, &key) {
            // We are going to create this resource now so it is marked as Exists
            dict::set(style_resources, &key, EXISTS.with(|exists| *exists));

            // If this is the first time we've encountered this precedence we need
            // to create a StyleQueue
            let style_queue = match style_queue {
                Some(style_queue) => style_queue,
                None => {
                    let style_queue = Rc::new(StyleQueue {
                        precedence: string_to_chunk(&escape_text_for_browser(Some(js::unknown(
                            precedence.as_str(),
                        )))),
                        rules: RefCell::new(Vec::new()),
                        hrefs: RefCell::new(Vec::new()),
                        sheets: js::map::new(None),
                    });
                    render_state
                        .styles
                        .set(&precedence, Rc::clone(&style_queue));
                    style_queue
                }
            };

            let resource = Rc::new(StylesheetResource {
                state: Cell::new(PENDING),
                props: js::object::assign(
                    js::unknown_of(PreinitStylesheetProps {
                        rel: "stylesheet",
                        href: href.to_owned(),
                        data_precedence: precedence,
                    }),
                    (options,),
                ),
            });

            if let Some(resource_state) = resource_state {
                if let Some(preload_state) = preload_credentials(resource_state) {
                    adopt_preload_credentials(resource.props, preload_state);
                }

                let preload_resource = render_state.preloads.stylesheets.get(&key);
                if let Some(preload_resource) = preload_resource
                    && !preload_resource.borrow().is_empty()
                {
                    // The Preload for this resource was created in this render pass and has not flushed yet so
                    // we need to clear it to avoid it flushing.
                    preload_resource.borrow_mut().clear();
                } else {
                    // Either the preload resource from this render already flushed in this render pass
                    // or the preload flushed in a prior pass (prerender). In either case we need to mark
                    // this resource as already having been preloaded.
                    resource.state.set(PRELOADED);
                }
            } else {
                // We don't need to check whether a preloadResource exists in the renderState
                // because if it did exist then the resourceState would also exist and we would
                // have hit the primary if condition above.
            }

            // We add the newly created resource to our StyleQueue and if necessary
            // track the resource with the currently rendering boundary
            style_queue.sheets.set(&key, resource);

            // Notify the request that there are resources to flush even if no work is currently happening
            flush_resources(request);
        }
    }
}

struct PreinitScriptProps {
    src: String,
    r#async: bool,
}

unsafe impl js::Defined for PreinitScriptProps {}

fn preinit_script(src: &str, options: Option<&'static Unknown>) {
    let Some(request) = resolve_request() else {
        // In async contexts we can sometimes resolve resources from AsyncLocalStorage. If we can't we can also
        // possibly get them from the stack if we are not in an async context. Since we were not able to resolve
        // the resources for this call in either case we opt to do nothing. We can consider making this a warning
        // but there may be times where calling a function outside of render is intentional (i.e. to warm up data
        // fetching) and we don't want to warn in those cases.
        (PREVIOUS_DISPATCHER.with(|previous_dispatcher| previous_dispatcher.preinit_script))(
            src, options,
        );
        return;
    };
    let resumable_state = get_resumable_state(request);
    let render_state = get_render_state(request);
    if !src.is_empty() {
        let key = get_resource_key(src);

        let script_resources = resumable_state.script_resources.get();
        let has_key = js::unknown(script_resources).has_own_property(&key);
        let resource_state = if has_key {
            js::get(script_resources, &key)
        } else {
            None
        };
        if !exists(script_resources, &key) {
            // We are going to create this resource now so it is marked as Exists
            dict::set(script_resources, &key, EXISTS.with(|exists| *exists));

            let props = js::object::assign(
                js::unknown_of(PreinitScriptProps {
                    src: src.to_owned(),
                    r#async: true,
                }),
                (options,),
            );
            if let Some(resource_state) = resource_state {
                if let Some(preload_state) = preload_credentials(resource_state) {
                    adopt_preload_credentials(props, preload_state);
                }

                if let Some(preload_resource) = render_state.preloads.scripts.get(&key) {
                    // the preload resource exists was created in this render. Now that we have
                    // a script resource which will emit earlier than a preload would if it
                    // hasn't already flushed we prevent it from flushing by zeroing the length
                    preload_resource.borrow_mut().clear();
                }
            }

            let resource = Rc::new(RefCell::new(Vec::new()));
            // Add to the script flushing queue
            render_state.scripts.add(&resource);
            // encode the tag as Chunks
            push_script_impl(&resource, props);
            // Notify the request that there are resources to flush even if no work is currently happening
            flush_resources(request);
        }
    }
}

struct PreinitModuleScriptProps {
    src: String,
    r#type: &'static str,
    r#async: bool,
}

unsafe impl js::Defined for PreinitModuleScriptProps {}

fn preinit_module_script(src: &str, options: Option<&'static Unknown>) {
    let Some(request) = resolve_request() else {
        // In async contexts we can sometimes resolve resources from AsyncLocalStorage. If we can't we can also
        // possibly get them from the stack if we are not in an async context. Since we were not able to resolve
        // the resources for this call in either case we opt to do nothing. We can consider making this a warning
        // but there may be times where calling a function outside of render is intentional (i.e. to warm up data
        // fetching) and we don't want to warn in those cases.
        (PREVIOUS_DISPATCHER.with(|previous_dispatcher| previous_dispatcher.preinit_module_script))(
            src, options,
        );
        return;
    };
    let resumable_state = get_resumable_state(request);
    let render_state = get_render_state(request);
    if !src.is_empty() {
        let key = get_resource_key(src);
        let module_script_resources = resumable_state.module_script_resources.get();
        let has_key = js::unknown(module_script_resources).has_own_property(&key);
        let resource_state = if has_key {
            js::get(module_script_resources, &key)
        } else {
            None
        };
        if !exists(module_script_resources, &key) {
            // We are going to create this resource now so it is marked as Exists
            dict::set(module_script_resources, &key, EXISTS.with(|exists| *exists));

            let props = js::object::assign(
                js::unknown_of(PreinitModuleScriptProps {
                    src: src.to_owned(),
                    r#type: "module",
                    r#async: true,
                }),
                (options,),
            );
            if let Some(resource_state) = resource_state {
                if let Some(preload_state) = preload_credentials(resource_state) {
                    adopt_preload_credentials(props, preload_state);
                }

                if let Some(preload_resource) = render_state.preloads.module_scripts.get(&key) {
                    // the preload resource exists was created in this render. Now that we have
                    // a script resource which will emit earlier than a preload would if it
                    // hasn't already flushed we prevent it from flushing by zeroing the length
                    preload_resource.borrow_mut().clear();
                }
            }

            let resource = Rc::new(RefCell::new(Vec::new()));
            // Add to the script flushing queue
            render_state.scripts.add(&resource);
            // encode the tag as Chunks
            push_script_impl(&resource, props);
            // Notify the request that there are resources to flush even if no work is currently happening
            flush_resources(request);
        }
    }
}

// This function is only safe to call at Request start time since it assumes
// that each module has not already been preloaded. If we find a need to preload
// scripts at any other point in time we will need to check whether the preload
// already exists and not assume it
fn preload_bootstrap_script_or_module(
    resumable_state: &ResumableState,
    render_state: &RenderState,
    href: &str,
    props: &Unknown,
) {
    let key = get_resource_key(href);

    if __DEV__
        && (js::unknown(resumable_state.script_resources.get()).has_own_property(&key)
            || js::unknown(resumable_state.module_script_resources.get()).has_own_property(&key))
    {
        // This is coded as a React error because it should be impossible for a userspace preload to preempt this call
        // If a userspace preload can preempt it then this assumption is broken and we need to reconsider this strategy
        // rather than instruct the user to not preload their bootstrap scripts themselves
        webapi::console::error((
            "Internal React Error: React expected bootstrap script or module with src \"%s\" to not have been preloaded already. please file an issue",
            href,
        ));
    }

    // The href used for bootstrap scripts and bootstrap modules should never be
    // used to preinit the resource. If a script can be preinited then it shouldn't
    // be a bootstrap script/module and if it is a bootstrap script/module then it
    // must not be safe to emit early. To avoid possibly allowing for preinits of
    // bootstrap scripts/modules we occlude these keys.
    dict::set(
        resumable_state.script_resources.get(),
        &key,
        EXISTS.with(|exists| *exists),
    );
    dict::set(
        resumable_state.module_script_resources.get(),
        &key,
        EXISTS.with(|exists| *exists),
    );

    let resource = Rc::new(RefCell::new(Vec::new()));
    push_link_impl(&resource, props);
    render_state.bootstrap_scripts.add(&resource);
}

fn internal_preinit_script(
    resumable_state: &ResumableState,
    render_state: &RenderState,
    src: &str,
    chunks: &Rc<Chunks>,
) {
    let key = get_resource_key(src);
    let script_resources = resumable_state.script_resources.get();
    if !js::unknown(script_resources).has_own_property(&key) {
        let resource = chunks;
        dict::set(script_resources, &key, EXISTS.with(|exists| *exists));
        render_state.scripts.add(resource);
    }
}

struct PreloadAsStyleProps {
    rel: &'static str,
    r#as: &'static str,
    href: Option<&'static Unknown>,
    cross_origin: Option<&'static Unknown>,
    fetch_priority: Option<&'static Unknown>,
    integrity: Option<&'static Unknown>,
    media: Option<&'static Unknown>,
    href_lang: Option<&'static Unknown>,
    referrer_policy: Option<&'static Unknown>,
}

unsafe impl js::Defined for PreloadAsStyleProps {}

fn preload_as_style_props_from_props(
    href: Option<&'static Unknown>,
    props: &'static Unknown,
) -> &'static Unknown {
    js::unknown_of(PreloadAsStyleProps {
        rel: "preload",
        r#as: "style",
        href,
        cross_origin: js::get(props, "crossOrigin"),
        fetch_priority: js::get(props, "fetchPriority"),
        integrity: js::get(props, "integrity"),
        media: js::get(props, "media"),
        href_lang: js::get(props, "hrefLang"),
        referrer_policy: js::get(props, "referrerPolicy"),
    })
}

struct StylesheetPrecedence {
    #[cfg_attr(rust_js, rust_js::name = "data-precedence")]
    data_precedence: Option<&'static Unknown>,
    precedence: Option<&'static Unknown>,
}

unsafe impl js::Defined for StylesheetPrecedence {}

fn stylesheet_props_from_raw_props(raw_props: &'static Unknown) -> &'static Unknown {
    js::object::spread((
        Some(raw_props),
        Some(js::unknown_of(StylesheetPrecedence {
            data_precedence: js::get(raw_props, "precedence"),
            precedence: js::null(),
        })),
    ))
}

fn adopt_preload_credentials(target: &Unknown, preload_state: &[Option<&'static Unknown>]) {
    if js::get(target, "crossOrigin").is_none() {
        js::set(target, "crossOrigin", preload_state[0]);
    }
    if js::get(target, "integrity").is_none() {
        js::set(target, "integrity", preload_state[1]);
    }
}

#[cfg_attr(rust_js, rust_js::name = "getPrefetchDNSAsHeader")]
fn get_prefetch_dns_as_header(href: &str) -> String {
    let escaped_href = escape_href_for_link_header_url_context(Some(js::unknown(href)));
    format!("<{escaped_href}>; rel=dns-prefetch")
}

fn get_preconnect_as_header(href: &str, cross_origin: Option<&Unknown>) -> String {
    let escaped_href = escape_href_for_link_header_url_context(Some(js::unknown(href)));
    let mut value = format!("<{escaped_href}>; rel=preconnect");
    if js::type_of(cross_origin) == "string" {
        let escaped_cross_origin =
            escape_string_for_link_header_quoted_param_value_context(cross_origin, "crossOrigin");
        value += &format!("; crossorigin=\"{escaped_cross_origin}\"");
    }
    value
}

fn get_preload_as_header(href: Option<&Unknown>, r#as: &str, params: Option<&Unknown>) -> String {
    let escaped_href = escape_href_for_link_header_url_context(href);
    let escaped_as =
        escape_string_for_link_header_quoted_param_value_context(Some(js::unknown(r#as)), "as");
    let mut value = format!("<{escaped_href}>; rel=preload; as=\"{escaped_as}\"");
    if let Some(params) = params {
        for param_name in js::keys_in(params) {
            if HAS_OWN_PROPERTY.with(|has_own_property| has_own_property.call(params, &param_name))
            {
                let param_value = js::get(params, &param_name);
                if js::type_of(param_value) == "string" {
                    value += &format!(
                        "; {}=\"{}\"",
                        param_name.to_lowercase(),
                        escape_string_for_link_header_quoted_param_value_context(
                            param_value,
                            &param_name
                        )
                    );
                }
            }
        }
    }
    value
}

struct StylesheetPreloadOptions {
    cross_origin: Option<&'static Unknown>,
    integrity: Option<&'static Unknown>,
    nonce: Option<&'static Unknown>,
    r#type: Option<&'static Unknown>,
    fetch_priority: Option<&'static Unknown>,
    referrer_policy: Option<&'static Unknown>,
    media: Option<&'static Unknown>,
}

unsafe impl js::Defined for StylesheetPreloadOptions {}

fn get_stylesheet_preload_as_header(stylesheet: &StylesheetResource) -> String {
    let props = stylesheet.props;
    let preload_options = StylesheetPreloadOptions {
        cross_origin: js::get(props, "crossOrigin"),
        integrity: js::get(props, "integrity"),
        nonce: js::get(props, "nonce"),
        r#type: js::get(props, "type"),
        fetch_priority: js::get(props, "fetchPriority"),
        referrer_policy: js::get(props, "referrerPolicy"),
        media: js::get(props, "media"),
    };
    get_preload_as_header(
        js::get(props, "href"),
        "style",
        Some(js::unknown(&preload_options)),
    )
}

thread_local! {
    // This escaping function is only safe to use for href values being written into
    // a "Link" header in between `<` and `>` characters. The primary concern with the href is
    // to escape the bounding characters as well as new lines. This is unsafe to use in any other
    // context
    static REGEX_FOR_HREF_IN_LINK_HEADER_URL_CONTEXT: &'static js::RegExp = js::reg_exp::new("[<>\\r\\n]", "g");
}

#[cfg_attr(rust_js, rust_js::name = "escapeHrefForLinkHeaderURLContext")]
fn escape_href_for_link_header_url_context(href_input: Option<&Unknown>) -> String {
    if __DEV__ {
        check_attribute_string_coercion(href_input, "href");
    }
    let coerced_href = js::concat_text(href_input).unwrap();
    REGEX_FOR_HREF_IN_LINK_HEADER_URL_CONTEXT.with(|regex| {
        js::reg_exp::replace(
            &coerced_href,
            regex,
            escape_href_for_link_header_url_context_replacer,
        )
    })
}

#[cfg_attr(rust_js, rust_js::name = "escapeHrefForLinkHeaderURLContextReplacer")]
fn escape_href_for_link_header_url_context_replacer(r#match: &str) -> &'static str {
    match r#match {
        "<" => "%3C",
        ">" => "%3E",
        "\n" => "%0A",
        "\r" => "%0D",
        _ => panic!(
            "escapeLinkHrefForHeaderContextReplacer encountered a match it does not know how to replace. this means the match regex and the replacement characters are no longer in sync. This is a bug in React"
        ),
    }
}

thread_local! {
    // This escaping function is only safe to use for quoted param values in an HTTP header.
    // It is unsafe to use for any value not inside quote marks in parater value position.
    static REGEX_FOR_LINK_HEADER_QUOTED_PARAM_VALUE_CONTEXT: &'static js::RegExp =
        js::reg_exp::new("[\"';,\\r\\n]", "g");
}

fn escape_string_for_link_header_quoted_param_value_context(
    value: Option<&Unknown>,
    name: &str,
) -> String {
    if __DEV__ {
        check_option_string_coercion(value, name);
    }
    let coerced = js::concat_text(value).unwrap();
    REGEX_FOR_LINK_HEADER_QUOTED_PARAM_VALUE_CONTEXT.with(|regex| {
        js::reg_exp::replace(
            &coerced,
            regex,
            escape_string_for_link_header_quoted_param_value_context_replacer,
        )
    })
}

fn escape_string_for_link_header_quoted_param_value_context_replacer(
    r#match: &str,
) -> &'static str {
    match r#match {
        "\"" => "%22",
        "'" => "%27",
        ";" => "%3B",
        "," => "%2C",
        "\n" => "%0A",
        "\r" => "%0D",
        _ => panic!(
            "escapeStringForLinkHeaderQuotedParamValueContextReplacer encountered a match it does not know how to replace. this means the match regex and the replacement characters are no longer in sync. This is a bug in React"
        ),
    }
}

pub fn hoist_hoistables(parent_state: &HoistableState, child_state: &HoistableState) {
    child_state
        .styles
        .for_each(|style_queue| parent_state.styles.add(style_queue));
    child_state
        .stylesheets
        .for_each(|stylesheet| parent_state.stylesheets.add(stylesheet));
    if child_state.suspensey_images.get() {
        // If the child has suspensey images, the parent now does too if it's inlined.
        // Similarly, if a SuspenseList row has a suspensey image then effectively
        // the next row should be blocked on it as well since the next row can't show
        // earlier. In practice, since the child will be outlined this transferring
        // may never matter but is conceptually correct.
        parent_state.suspensey_images.set(true);
    }
}

pub fn has_suspensey_content(hoistable_state: &HoistableState, flushing_in_shell: bool) -> bool {
    if flushing_in_shell {
        // When flushing the shell, stylesheets with precedence are already emitted
        // in the <head> which blocks paint. There's no benefit to outlining for CSS
        // alone during the shell flush. However, suspensey images (for ViewTransition
        // animation reveals) should still trigger outlining even during the shell.
        return hoistable_state.suspensey_images.get();
    }
    hoistable_state.stylesheets.size() > 0 || hoistable_state.suspensey_images.get()
}

// This function is called at various times depending on whether we are rendering
// or prerendering. In this implementation we only actually emit headers once and
// subsequent calls are ignored. We track whether the request has a completed shell
// to determine whether we will follow headers with a flush including stylesheets.
// In the context of prerrender we don't have a completed shell when the request finishes
// with a postpone in the shell. In the context of a render we don't have a completed shell
// if this is called before the shell finishes rendering which usually will happen anytime
// anything suspends in the shell.
pub fn emit_early_preloads(
    render_state: &RenderState,
    _resumable_state: &ResumableState,
    shell_complete: bool,
) {
    let on_headers = render_state.on_headers;
    if let Some(on_headers) = on_headers {
        let headers = render_state.headers.borrow().clone();
        if let Some(headers) = headers {
            // Even if onHeaders throws we don't want to call this again so
            // we drop the headers state from this point onwards.
            *render_state.headers.borrow_mut() = None;

            let mut link_header = headers.preconnects.borrow().clone();
            if !headers.font_preloads.borrow().is_empty() {
                if !link_header.is_empty() {
                    link_header += ", ";
                }
                link_header += &headers.font_preloads.borrow();
            }
            if !headers.high_image_preloads.borrow().is_empty() {
                if !link_header.is_empty() {
                    link_header += ", ";
                }
                link_header += &headers.high_image_preloads.borrow();
            }

            if !shell_complete {
                // We use raw iterators because we want to be able to halt iteration
                // We could refactor renderState to store these dually in arrays to
                // make this more efficient at the cost of additional memory and
                // write overhead. However this code only runs once per request so
                // for now I consider this sufficient.
                'outer: for queue in render_state.styles.values() {
                    if headers.remaining_capacity.get() <= 0.0 {
                        break;
                    }
                    let sheets = queue.sheets;
                    for sheet in sheets.values() {
                        if headers.remaining_capacity.get() <= 0.0 {
                            break;
                        }
                        let props = sheet.props;
                        let key =
                            get_resource_key(&js::concat_text(js::get(props, "href")).unwrap());

                        let header = get_stylesheet_preload_as_header(&sheet);
                        // We mutate the capacity b/c we don't want to keep checking if later headers will fit.
                        // This means that a particularly long header might close out the header queue where later
                        // headers could still fit. We could in the future alter the behavior here based on prerender vs render
                        // since during prerender we aren't as concerned with pure runtime performance.
                        let remaining = headers.remaining_capacity.get()
                            - (f64::from(js::string::length(&header)) + 2.0);
                        headers.remaining_capacity.set(remaining);
                        if remaining >= 0.0 {
                            dict::set(
                                render_state.resets.style,
                                &key,
                                Some(PRELOAD_NO_CREDS.with(|preload| *preload)),
                            );
                            if !link_header.is_empty() {
                                link_header += ", ";
                            }
                            link_header += &header;

                            // We already track that the resource exists in resumableState however
                            // if the resumableState resets because we postponed in the shell
                            // which is what is happening in this branch if we are prerendering
                            // then we will end up resetting the resumableState. When it resets we
                            // want to record the fact that this stylesheet was already preloaded
                            dict::set(
                                render_state.resets.style,
                                &key,
                                preload_state_of(Some(props)),
                            );
                        } else {
                            break 'outer;
                        }
                    }
                }
            }
            if !link_header.is_empty() {
                on_headers(&HeadersDescriptor {
                    link: Some(link_header),
                });
            } else {
                // We still call this with no headers because a user may be using it as a signal that
                // it React will not provide any headers
                on_headers(&HeadersDescriptor { link: None });
            }
        }
    }
}

pub type TransitionStatus = FormStatus;

thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "NotPendingTransition")]
    pub static NOT_PENDING_TRANSITION: &'static TransitionStatus = NOT_PENDING.with(|not_pending| *not_pending);
}

thread_local! {
    static END_OF_START_TAG: &'static PrecomputedChunk = string_to_precomputed_chunk(">");
    static END_OF_START_TAG_SELF_CLOSING: &'static PrecomputedChunk = string_to_precomputed_chunk("/>");
}
