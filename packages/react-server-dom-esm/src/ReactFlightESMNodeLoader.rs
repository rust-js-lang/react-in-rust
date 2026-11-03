// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::Cell;
use std::collections::HashSet;
use std::rc::Rc;

use js::{Kind, Promise, Unknown};
use js::{json, string};
use node::BufferEncoding;
use node::buffer::Buffer;
use webapi::console;

use acorn_loose::{EcmaVersion, Node, Options, Position, Program, SourceLocation, SourceType};

use webpack_sources::helpers::{create_mappings_serializer, read_mappings};

#[derive(Clone)]
pub struct ResolveContext {
    pub conditions: Vec<String>,
    #[cfg_attr(rust_js, rust_js::name = "parentURL")]
    pub parent_url: Option<String>,
}

pub struct Resolved {
    pub url: String,
}

pub type ResolveFn = dyn Fn(&str, &ResolveContext, ResolveFunction) -> Promise<&'static Resolved>; // GAP rd674

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct ResolveFunction(pub &'static ResolveFn);

pub struct GetSourceContext {
    pub format: String,
}

pub type GetSourceFn = dyn Fn(&str, &GetSourceContext, GetSourceFunction) -> Promise<Rc<Sourced>>;

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct GetSourceFunction(pub &'static GetSourceFn);

pub struct TransformSourceContext {
    pub format: String,
    pub url: String,
}

pub type TransformSourceFn =
    dyn Fn(Source, &TransformSourceContext, TransformSourceFunction) -> Promise<Rc<Sourced>>;

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct TransformSourceFunction(pub &'static TransformSourceFn);

/// What a module is loaded as, `importAssertions`' and `importAttributes`':
/// its `type`, `"json"` of a source map's.
pub struct ImportAttributes {
    pub r#type: Option<String>,
}

pub struct LoadContext {
    pub conditions: Vec<String>,
    pub format: Option<String>,
    pub import_assertions: Option<ImportAttributes>,
    pub import_attributes: Option<ImportAttributes>,
}

pub type LoadFn = dyn Fn(&str, &LoadContext, LoadFunction) -> Promise<Rc<Loaded>>;

#[repr(transparent)]
#[derive(Clone)]
pub struct LoadFunction(pub Rc<LoadFn>);

pub struct Sourced {
    pub source: Source,
}

pub struct Loaded {
    pub format: String,
    pub short_circuit: Option<bool>,
    pub source: Source,
}

#[cfg_attr(rust_js, rust_js::untagged)]
#[derive(Clone)]
pub enum Source {
    String(String),
    ArrayBuffer(&'static js::ArrayBuffer),
    Uint8Array(&'static js::Uint8Array),
}

// SAFETY: each variant is its payload, a string or a buffer, never `undefined` nor `null`.
unsafe impl js::Defined for Source {}

thread_local! {
    static WARNED_ABOUT_CONDITIONS_FLAG: Cell<bool> = const { Cell::new(false) };

    // GAP rd694: `let stashedGetSource;` and `== null` of a `Cell<Option<..>>`, where upstream writes `= null`.
    static STASHED_GET_SOURCE: Cell<Option<GetSourceFunction>> = const { Cell::new(None) };
    static STASHED_RESOLVE: Cell<Option<ResolveFunction>> = const { Cell::new(None) };
}

// GAP rd695: an async fn's `.d.ts` result is `any`.
pub async fn resolve(
    specifier: &str,
    mut context: Rc<ResolveContext>,
    default_resolve: ResolveFunction,
) -> &'static Resolved {
    // We stash this in case we end up needing to resolve export * statements later.
    STASHED_RESOLVE.set(Some(default_resolve));

    if !context
        .conditions
        .iter()
        .any(|condition| condition == "react-server")
    {
        // GAP rd676: a struct update of the clone, `..(*context).clone()`, would print the clone and a read of each field.
        let mut with_react_server = (*context).clone();
        with_react_server.conditions.push("react-server".to_owned());
        context = Rc::new(with_react_server);
        if !WARNED_ABOUT_CONDITIONS_FLAG.get() {
            WARNED_ABOUT_CONDITIONS_FLAG.set(true);
            // eslint-disable-next-line react-internal/no-production-logging
            console::warn((
                "You did not run Node.js with the `--conditions react-server` flag. \
                 Any \"react-server\" override will only work with ESM imports.",
            ));
        }
    }
    (default_resolve.0)(specifier, &context, default_resolve).await
}

pub async fn get_source(
    url: &str,
    context: &GetSourceContext,
    default_get_source: GetSourceFunction,
) -> Rc<Sourced> {
    // We stash this in case we end up needing to resolve export * statements later.
    STASHED_GET_SOURCE.set(Some(default_get_source));
    (default_get_source.0)(url, context, default_get_source).await
}

#[derive(PartialEq)]
enum ExportType {
    #[cfg_attr(rust_js, rust_js::name = "function")]
    Function,
}

struct ExportedEntry {
    local_name: Option<String>,
    exported_name: Option<String>,
    #[cfg_attr(rust_js, rust_js::nullable)]
    r#type: Option<ExportType>,
    loc: Option<&'static SourceLocation>,
    original_line: f64,
    original_column: f64,
    original_source: f64,
    name_index: f64,
}

#[cfg_attr(rust_js, rust_js::nullable(r#type))]
fn add_exported_entry(
    exported_entries: &mut Vec<ExportedEntry>,
    local_names: &HashSet<Option<String>>,
    local_name: Option<String>,
    exported_name: Option<String>,
    r#type: Option<ExportType>,
    loc: Option<&'static SourceLocation>,
) {
    if local_names.contains(&local_name) {
        // If the same local name is exported more than once, we only need one of the names.
        return;
    }
    exported_entries.push(ExportedEntry {
        local_name,
        exported_name,
        r#type,
        loc,
        original_line: -1.0,
        original_column: -1.0,
        original_source: -1.0,
        name_index: -1.0,
    });
}

fn add_local_exported_names(
    exported_entries: &mut Vec<ExportedEntry>,
    local_names: &HashSet<Option<String>>,
    node: &Node,
) {
    // GAP rd677: a `match` of the tagged `Node` is an `if`/`else if` chain, not `switch (node.type)`.
    match node {
        Node::Identifier { name, loc } => {
            add_exported_entry(
                exported_entries,
                local_names,
                Some(name.clone()),
                Some(name.clone()),
                None,
                *loc,
            );
        }
        Node::ObjectPattern { properties } => {
            for property in properties.iter() {
                add_local_exported_names(exported_entries, local_names, property);
            }
        }
        Node::ArrayPattern { elements } => {
            // GAP rd691: `.flatten()` is `values().filter(item => item != null)`.
            for element in elements.iter().flatten() {
                add_local_exported_names(exported_entries, local_names, element);
            }
        }
        Node::Property { value } => {
            add_local_exported_names(exported_entries, local_names, value);
        }
        Node::AssignmentPattern { left } => {
            add_local_exported_names(exported_entries, local_names, left);
        }
        Node::RestElement { argument } => {
            add_local_exported_names(exported_entries, local_names, argument);
        }
        Node::ParenthesizedExpression { expression } => {
            add_local_exported_names(exported_entries, local_names, expression);
        }
        _ => {}
    }
}

/// A source map's JSON: `version`, `sources`, `sourcesContent`, `mappings`
/// and `sourceRoot`, of a file that had none.
struct SourceMap {
    version: u32,
    sources: Vec<String>,
    sources_content: Vec<String>,
    mappings: String,
    source_root: String,
}

// An object, never `undefined`.
unsafe impl js::Defined for SourceMap {}

fn transform_server_module(
    source: &str,
    program: &Program,
    url: &str,
    source_map: Option<&'static Unknown>,
    _loader: &LoadFunction,
) -> String {
    let body = program.body;

    // This entry list needs to be in source location order.
    let mut exported_entries: Vec<ExportedEntry> = vec![];
    // Dedupe set.
    // GAP rd693: a set of `Option` names is a `$KeySet`.
    let local_names: HashSet<Option<String>> = HashSet::new();

    for node in body {
        match node {
            Node::ExportAllDeclaration { .. } => {
                // If export * is used, the other file needs to explicitly opt into "use server" too.
            }
            Node::ExportDefaultDeclaration { declaration } => match declaration {
                Node::Identifier { name, loc } => {
                    add_exported_entry(
                        &mut exported_entries,
                        &local_names,
                        Some(name.clone()),
                        Some("default".to_owned()),
                        None,
                        *loc,
                    );
                }
                Node::FunctionDeclaration { id: Some(id) } => {
                    add_exported_entry(
                        &mut exported_entries,
                        &local_names,
                        id.name(),
                        Some("default".to_owned()),
                        Some(ExportType::Function),
                        id.loc(),
                    );
                }
                Node::FunctionDeclaration { id: None } => {
                    // TODO: This needs to be rewritten inline because it doesn't have a local name.
                }
                _ => {}
            },
            Node::ExportNamedDeclaration {
                declaration,
                specifiers,
            } => {
                if let Some(declaration) = declaration {
                    if let Node::VariableDeclaration { declarations } = declaration {
                        for declarator in declarations.iter() {
                            add_local_exported_names(
                                &mut exported_entries,
                                &local_names,
                                declarator.id,
                            );
                        }
                    } else {
                        let id = declaration.id().unwrap();
                        let name = id.name();
                        add_exported_entry(
                            &mut exported_entries,
                            &local_names,
                            name.clone(),
                            name,
                            if matches!(declaration, Node::FunctionDeclaration { .. }) {
                                Some(ExportType::Function)
                            } else {
                                None
                            },
                            id.loc(),
                        );
                    }
                }
                for specifier in specifiers.iter() {
                    add_exported_entry(
                        &mut exported_entries,
                        &local_names,
                        specifier.local.name(),
                        specifier.exported.name(),
                        None,
                        specifier.local.loc(),
                    );
                }
            }
            _ => {}
        }
    }

    let mut mappings = if js::truthy(source_map)
        && let Kind::String(mappings) = js::classify(js::get(source_map, "mappings"))
    {
        mappings.to_owned()
    } else {
        String::new()
    };
    let mut new_src = source.to_owned();
    let mut source_map = source_map;

    if !exported_entries.is_empty() {
        let mut last_source_index = 0.0;
        let mut last_original_line = 0.0;
        let mut last_original_column = 0.0;
        let mut last_name_index = 0.0;
        let source_line_count;
        let mut last_mapped_line = 0.0;

        if js::truthy(source_map) {
            // We iterate source mapping entries and our matched exports in parallel to source map
            // them to their original location.
            let mut next_entry_idx = 0;
            let mut next_entry_line = exported_entries[next_entry_idx].loc.unwrap().start.line;
            let mut next_entry_column = exported_entries[next_entry_idx].loc.unwrap().start.column;
            read_mappings(
                &mappings,
                &mut |generated_line: f64,
                      generated_column: f64,
                      source_index: f64,
                      original_line: f64,
                      original_column: f64,
                      name_index: f64| {
                    if generated_line > next_entry_line
                        || (generated_line == next_entry_line
                            && generated_column > next_entry_column)
                    {
                        // We're past the entry which means that the best match we have is the previous entry.
                        if last_mapped_line == next_entry_line {
                            // Match
                            let entry = &mut exported_entries[next_entry_idx];
                            entry.original_line = last_original_line;
                            entry.original_column = last_original_column;
                            entry.original_source = last_source_index;
                            entry.name_index = last_name_index;
                        } else {
                            // Skip if we didn't have any mappings on the exported line.
                        }
                        next_entry_idx += 1;
                        if next_entry_idx < exported_entries.len() {
                            next_entry_line =
                                exported_entries[next_entry_idx].loc.unwrap().start.line;
                            next_entry_column =
                                exported_entries[next_entry_idx].loc.unwrap().start.column;
                        } else {
                            next_entry_line = -1.0;
                            next_entry_column = -1.0;
                        }
                    }
                    last_mapped_line = generated_line;
                    if source_index > -1.0 {
                        last_source_index = source_index;
                    }
                    if original_line > -1.0 {
                        last_original_line = original_line;
                    }
                    if original_column > -1.0 {
                        last_original_column = original_column;
                    }
                    if name_index > -1.0 {
                        last_name_index = name_index;
                    }
                },
            );
            if next_entry_idx < exported_entries.len() && last_mapped_line == next_entry_line {
                // Match
                let entry = &mut exported_entries[next_entry_idx];
                entry.original_line = last_original_line;
                entry.original_column = last_original_column;
                entry.original_source = last_source_index;
                entry.name_index = last_name_index;
            }

            for c in mappings.chars().rev() {
                if c != ';' {
                    break;
                }
                // If the last mapped lines don't contain any segments, we don't get a callback from readMappings
                // so we need to pad the number of mapped lines, with one for each empty line.
                last_mapped_line += 1.0;
            }

            source_line_count = program.loc.unwrap().end.line;
            if source_line_count < last_mapped_line {
                panic!("The source map has more mappings than there are lines.");
            }
            // If the original source string had more lines than there are mappings in the source map.
            // Add some extra padding of unmapped lines so that any lines that we add line up.
            let mut extra_lines = source_line_count - last_mapped_line;
            while extra_lines > 0.0 {
                mappings += ";";
                extra_lines -= 1.0;
            }
        } else {
            // If a file doesn't have a source map then we generate a blank source map that just
            // contains the original content and segments pointing to the original lines.
            source_line_count = source.split('\n').count() as f64;
            mappings = "AAAA".to_owned() + &string::repeat(";AACA", source_line_count - 1.0);
            source_map = Some(js::unknown_of(SourceMap {
                version: 3,
                sources: vec![url.to_owned()],
                sources_content: vec![source.to_owned()],
                mappings: mappings.clone(),
                source_root: String::new(),
            }));
            last_source_index = 0.0;
            last_original_line = source_line_count;
            last_original_column = 0.0;
            last_name_index = -1.0;

            for entry in exported_entries.iter_mut() {
                // Point each entry to original location.
                entry.original_source = 0.0;
                entry.original_line = entry.loc.unwrap().start.line;
                // We use column zero since we do the short-hand line-only source maps above.
                entry.original_column = 0.0; // entry.loc.start.column;
            }
        }

        // GAP rd689: consecutive `+=`s of text are merged into one.
        new_src += "\n\n;";
        new_src += "import {registerServerReference} from \"react-server-dom-esm/server\";\n";
        if !mappings.is_empty() {
            mappings += ";;";
        }

        let create_mapping = create_mappings_serializer(None);

        // Create an empty mapping pointing to where we last left off to reset the counters.
        let mut generated_line = 1.0;
        create_mapping(
            generated_line,
            0.0,
            last_source_index,
            last_original_line,
            last_original_column,
            last_name_index,
        );
        for entry in &exported_entries {
            generated_line += 1.0;
            let local_name = entry.local_name.as_deref().unwrap_or("undefined");
            if entry.r#type != Some(ExportType::Function) {
                // We first check if the export is a function and if so annotate it.
                new_src += &("if (typeof ".to_owned() + local_name + " === \"function\") ");
            }
            new_src += &("registerServerReference(".to_owned() + local_name + ",");
            new_src += &(json::stringify(url) + ",");
            new_src += &(entry
                .exported_name
                .as_deref()
                .map_or("undefined".to_owned(), json::stringify)
                + ");\n");

            mappings += &create_mapping(
                generated_line,
                0.0,
                entry.original_source,
                entry.original_line,
                entry.original_column,
                entry.name_index,
            );
        }
    }

    if js::truthy(source_map) {
        // Override with an new mappings and serialize an inline source map.
        js::set(source_map, "mappings", &*mappings);
        new_src += &("//# sourceMappingURL=data:application/json;charset=utf-8;base64,".to_owned()
            + &Buffer::from_with_str(&json::stringify(&source_map).unwrap().unwrap(), None)
                .to_string(Some(BufferEncoding::Base64), None, None));
    }

    new_src
}

fn add_export_names(names: &mut Vec<String>, node: &Node) {
    match node {
        Node::Identifier { name, .. } => {
            names.push(name.clone());
        }
        Node::ObjectPattern { properties } => {
            for property in properties.iter() {
                add_export_names(names, property);
            }
        }
        Node::ArrayPattern { elements } => {
            // GAP rd691
            for element in elements.iter().flatten() {
                add_export_names(names, element);
            }
        }
        Node::Property { value } => {
            add_export_names(names, value);
        }
        Node::AssignmentPattern { left } => {
            add_export_names(names, left);
        }
        Node::RestElement { argument } => {
            add_export_names(names, argument);
        }
        Node::ParenthesizedExpression { expression } => {
            add_export_names(names, expression);
        }
        _ => {}
    }
}

fn resolve_client_import(specifier: &str, parent_url: &str) -> Promise<&'static Resolved> {
    // Resolve an import specifier as if it was loaded by the client. This doesn't use
    // the overrides that this loader does but instead reverts to the default.
    // This resolution algorithm will not necessarily have the same configuration
    // as the actual client loader. It should mostly work and if it doesn't you can
    // always convert to explicit exported names instead.
    let conditions = vec!["node".to_owned(), "import".to_owned()];
    let Some(stashed_resolve) = STASHED_RESOLVE.get() else {
        panic!("Expected resolve to have been called before transformSource");
    };
    (stashed_resolve.0)(
        specifier,
        &ResolveContext {
            conditions,
            parent_url: Some(parent_url.to_owned()),
        },
        stashed_resolve,
    )
}

async fn parse_export_names_into(
    body: &'static [Node],
    names: &mut Vec<String>,
    parent_url: &str,
    loader: &LoadFunction,
) {
    for node in body {
        match node {
            Node::ExportAllDeclaration { exported, source } => {
                if let Some(exported) = exported {
                    add_export_names(names, exported);
                    continue;
                } else {
                    let url = resolve_client_import(&source.value, parent_url)
                        .await
                        .url
                        .clone();
                    let loaded = (loader.0)(
                        &url,
                        &LoadContext {
                            format: Some("module".to_owned()),
                            conditions: vec![],
                            import_assertions: Some(ImportAttributes { r#type: None }),
                            import_attributes: None,
                        },
                        loader.clone(),
                    )
                    .await;
                    let Source::String(source) = &loaded.source else {
                        panic!("Expected the transformed source to be a string.");
                    };
                    // GAP rd690: a `match` of the `Result` whose `Err` continues is `$try`, not `try`/`catch`.
                    let child_body = match acorn_loose::parse(
                        source,
                        &Options {
                            ecma_version: EcmaVersion::Version(2024),
                            source_type: Some(SourceType::Module),
                            locations: None,
                            on_comment: None,
                            // GAP rd692: `Default` of the untagged `EcmaVersion` is refused, so each field is named.
                            preserve_parens: None,
                        },
                    ) {
                        Ok(program) => program.body,
                        Err(x) => {
                            // eslint-disable-next-line react-internal/no-production-logging
                            console::error(("Error parsing %s %s", &*url, &*x.message()));
                            continue;
                        }
                    };
                    Box::pin(parse_export_names_into(child_body, names, &url, loader)).await;
                    continue;
                }
            }
            Node::ExportDefaultDeclaration { .. } => {
                names.push("default".to_owned());
                continue;
            }
            Node::ExportNamedDeclaration {
                declaration,
                specifiers,
            } => {
                if let Some(declaration) = declaration {
                    if let Node::VariableDeclaration { declarations } = declaration {
                        for declarator in declarations.iter() {
                            add_export_names(names, declarator.id);
                        }
                    } else {
                        add_export_names(names, declaration.id().unwrap());
                    }
                }
                for specifier in specifiers.iter() {
                    add_export_names(names, specifier.exported);
                }
                continue;
            }
            _ => {}
        }
    }
}

async fn transform_client_module(
    program: &Program,
    url: &str,
    _source_map: Option<&'static Unknown>,
    loader: &LoadFunction,
) -> String {
    let body = program.body;

    let mut names: Vec<String> = vec![];

    parse_export_names_into(body, &mut names, url, loader).await;

    if names.is_empty() {
        return String::new();
    }

    let mut new_src =
        "import {registerClientReference} from \"react-server-dom-esm/server\";\n".to_owned();
    for name in &names {
        if name == "default" {
            new_src += "export default ";
            new_src += "registerClientReference(function() {";
            new_src += &("throw new Error(".to_owned()
                + &json::stringify(&format!(
                    "Attempted to call the default export of {url} from the server \
                     but it's on the client. It's not possible to invoke a client function from \
                     the server, it can only be rendered as a Component or passed to props of a \
                     Client Component."
                ))
                + ");");
        } else {
            new_src += &("export const ".to_owned() + name + " = ");
            new_src += "registerClientReference(function() {";
            new_src += &("throw new Error(".to_owned()
                + &json::stringify(&format!(
                    "Attempted to call {name}() from the server but {name} is on the client. \
                     It's not possible to invoke a client function from the server, it can \
                     only be rendered as a Component or passed to props of a Client Component."
                ))
                + ");");
        }
        new_src += "},";
        new_src += &(json::stringify(url) + ",");
        new_src += &(json::stringify(name.as_str()) + ");\n");
    }

    // TODO: Generate source maps for Client Reference functions so they can point to their
    // original locations.
    new_src
}

async fn load_client_import(
    url: &str,
    default_transform_source: TransformSourceFunction,
) -> Rc<Loaded> {
    let Some(stashed_get_source) = STASHED_GET_SOURCE.get() else {
        panic!("Expected getSource to have been called before transformSource");
    };
    // TODO: Validate that this is another module by calling getFormat.
    let sourced = (stashed_get_source.0)(
        url,
        &GetSourceContext {
            format: "module".to_owned(),
        },
        stashed_get_source,
    )
    .await;
    let result = (default_transform_source.0)(
        sourced.source.clone(),
        &TransformSourceContext {
            format: "module".to_owned(),
            url: url.to_owned(),
        },
        default_transform_source,
    )
    .await;
    Rc::new(Loaded {
        format: "module".to_owned(),
        short_circuit: None,
        source: result.source.clone(),
    })
}

async fn transform_module_if_needed(
    mut source: String,
    url: &str,
    loader: &LoadFunction,
) -> String {
    // Do a quick check for the exact string. If it doesn't exist, don't
    // bother parsing.
    if !source.contains("use client") && !source.contains("use server") {
        return source;
    }

    let mut source_mapping_url = None;
    let mut source_mapping_start = 0.0;
    let mut source_mapping_end = 0.0;
    let mut source_mapping_lines = 0.0;

    let program = match acorn_loose::parse(
        &source,
        &Options {
            ecma_version: EcmaVersion::Version(2024),
            source_type: Some(SourceType::Module),
            locations: Some(true),
            // GAP rd688: the closure is hoisted as `function value(..)`, not `onComment(..) {..}` in the literal.
            on_comment: Some(&mut |_block: bool,
                                   text: &str,
                                   start: f64,
                                   end: f64,
                                   start_loc: Option<&Position>,
                                   end_loc: Option<&Position>| {
                if text.starts_with("# sourceMappingURL=")
                    || text.starts_with("@ sourceMappingURL=")
                {
                    source_mapping_url = Some(js::string::slice(text, 19.0, None));
                    source_mapping_start = start;
                    source_mapping_end = end;
                    source_mapping_lines = end_loc.unwrap().line - start_loc.unwrap().line;
                }
            }),
            preserve_parens: None,
        },
    ) {
        Ok(program) => program,
        Err(x) => {
            // eslint-disable-next-line react-internal/no-production-logging
            console::error(("Error parsing %s %s", url, &*x.message()));
            return source;
        }
    };

    let mut use_client = false;
    let mut use_server = false;

    let body = program.body;
    for node in body {
        let directive = match node {
            Node::ExpressionStatement {
                directive: Some(directive),
            } if !directive.is_empty() => directive,
            _ => break,
        };
        if directive == "use client" {
            use_client = true;
        }
        if directive == "use server" {
            use_server = true;
        }
    }

    if !use_client && !use_server {
        return source;
    }

    if use_client && use_server {
        panic!("Cannot have both \"use client\" and \"use server\" directives in the same file.");
    }

    let mut source_map = None;
    if let Some(source_mapping_url) = source_mapping_url {
        let source_map_result = (loader.0)(
            &source_mapping_url,
            &LoadContext {
                format: Some("json".to_owned()),
                conditions: vec![],
                import_assertions: Some(ImportAttributes {
                    r#type: Some("json".to_owned()),
                }),
                import_attributes: Some(ImportAttributes {
                    r#type: Some("json".to_owned()),
                }),
            },
            loader.clone(),
        )
        .await;
        let source_map_string = match &source_map_result.source {
            Source::String(source) => source.clone(),
            // SAFETY: Node loads a module's source as a `Buffer`.
            source => unsafe { js::cast::<&Buffer>(Some(js::unknown(source))) }.to_string(
                Some(BufferEncoding::Utf8),
                None,
                None,
            ),
        };
        source_map = json::parse(&source_map_string).unwrap();

        // Strip the source mapping comment. We'll re-add it below if needed.
        source = js::string::slice(&source, 0.0, Some(source_mapping_start))
            + &string::repeat("\n", source_mapping_lines)
            + &js::string::slice(&source, source_mapping_end, None);
    }

    if use_client {
        return transform_client_module(program, url, source_map, loader).await;
    }

    transform_server_module(&source, program, url, source_map, loader)
}

pub async fn transform_source(
    source: Source,
    context: &TransformSourceContext,
    default_transform_source: TransformSourceFunction,
) -> Rc<Sourced> {
    let transformed = (default_transform_source.0)(source, context, default_transform_source).await;
    if context.format == "module" {
        let Source::String(transformed_source) = &transformed.source else {
            panic!("Expected source to have been transformed to a string.");
        };
        let new_src = transform_module_if_needed(
            transformed_source.clone(),
            &context.url,
            &LoadFunction(Rc::new(
                move |url: &str, _ctx: &LoadContext, _default_load: LoadFunction| {
                    js::promise(load_client_import(url, default_transform_source))
                },
            )),
        )
        .await;
        return Rc::new(Sourced {
            source: Source::String(new_src),
        });
    }
    transformed
}

pub async fn load(url: &str, context: &LoadContext, default_load: LoadFunction) -> Rc<Loaded> {
    let result = (default_load.0)(url, context, default_load.clone()).await;
    if result.format == "module" {
        let Source::String(source) = &result.source else {
            panic!("Expected source to have been loaded into a string.");
        };
        let new_src = transform_module_if_needed(source.clone(), url, &default_load).await;
        return Rc::new(Loaded {
            format: "module".to_owned(),
            short_circuit: None,
            source: Source::String(new_src),
        });
    }
    result
}
