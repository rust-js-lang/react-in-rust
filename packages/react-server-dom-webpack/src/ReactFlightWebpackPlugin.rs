// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use acorn_loose as acorn;
use js::{Dict, Error, Kind, RegExp, Unknown, dict, json, reg_exp};
use node::path::join;
use node::url::path_to_file_url;
use webpack::compilation::process_assets_stage_report;
use webpack::{
    AsyncDependenciesBlock, ChunkGroupOptions, CompilationParams, Compiler, ContextModuleFactory,
    ContextModuleOptions, CrossOriginLoading, Id, InputFileSystem, JavascriptParser, Module,
    ModuleDependency, ResolveContext, ResolveContextInfo, ResolveOptions, Resolved, Resolver,
    null_dependency, sources, template, webpack_error,
};

#[cfg_attr(rust_js, rust_js::class)]
pub struct ClientReferenceDependency {}

impl js::Classed for ClientReferenceDependency {
    type Constructor = js::AnyConstructor;
}

#[cfg_attr(rust_js, rust_js::class)]
impl ClientReferenceDependency {
    pub fn new(_request: &str) -> Self {
        ClientReferenceDependency {}
    }
}

impl ModuleDependency for ClientReferenceDependency {
    #[cfg_attr(rust_js, rust_js::getter)]
    fn r#type(&self) -> &'static str {
        "client-reference"
    }
}

// This is the module that will be used to anchor all client references to.
// I.e. it will have all the client files as async deps from this point on.
// We use the Flight client implementation because you can't get to these
// without the client runtime so it's the first time in the loading sequence
// you might want them.
#[cfg_attr(rust_js, rust_js::name = "clientImportName")]
const CLIENT_IMPORT_NAME: &str = "react-server-dom-webpack/client";
thread_local! {
    #[cfg_attr(rust_js, rust_js::name = "clientFileName")]
    static CLIENT_FILE_NAME: String = node::require::resolve("../client.browser.js", None);
}

#[derive(Clone)]
pub struct ClientReferenceSearchPath {
    pub directory: String,
    pub recursive: Option<bool>,
    pub include: &'static RegExp,
    pub exclude: Option<&'static RegExp>,
}

#[derive(Clone)]
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum ClientReferencePath {
    Path(String),
    Search(ClientReferenceSearchPath),
}

/// `ClientReferencePath | $ReadOnlyArray<ClientReferencePath>`.
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum ClientReferences {
    Path(String),
    Many(Vec<ClientReferencePath>),
    Search(ClientReferenceSearchPath),
}

pub struct Options {
    // A JS caller's, of any value: the constructor tests it's a boolean.
    pub is_server: Option<&'static Unknown>,
    pub client_references: Option<ClientReferences>,
    pub chunk_name: Option<String>,
    pub client_manifest_filename: Option<String>,
    pub server_consumer_manifest_filename: Option<String>,
}

const PLUGIN_NAME: &str = "React Server Plugin";

// GAP: upstream fills the shared `ImportManifestEntry` (shared/ReactFlightImportMetadata),
// whose `id` and `chunks` the port types `String`, where webpack's `Id` is
// `string | number`; widening it changes the Flight client's JS. One type
// waits on rust-js's untagged string-or-integer enum formatted in place
// (m6-react's gap); until then the plugin writes its own entry.
/// An `ImportManifestEntry` of what webpack gives: a module's id and its
/// chunks' ids, each a string or a number.
struct ClientManifestEntry {
    id: Id,
    chunks: Rc<Vec<Option<Id>>>,
    name: &'static str,
}

unsafe impl js::JsonText for ClientManifestEntry {}

struct SsrExport {
    specifier: String,
    name: &'static str,
}

unsafe impl js::JsonText for SsrExport {}

type ServerConsumerModuleMap = Dict<&'static Dict<SsrExport>>;

struct ModuleLoading {
    prefix: String,
    #[cfg_attr(rust_js, rust_js::nullable)]
    cross_origin: Option<String>,
}

unsafe impl js::JsonText for ModuleLoading {}

struct SsrBundleConfig {
    module_loading: ModuleLoading,
    module_map: &'static ServerConsumerModuleMap,
}

unsafe impl js::JsonText for SsrBundleConfig {}

type ClientReferenceDependencies = Rc<Vec<Rc<ClientReferenceDependency>>>;

type ResolvedCallback =
    Box<dyn FnOnce(Option<&'static Error>, Option<Vec<Rc<ClientReferenceDependency>>>)>;

#[cfg_attr(rust_js, rust_js::class(es))]
pub struct ReactFlightWebpackPlugin {
    pub client_references: Vec<ClientReferencePath>,
    pub chunk_name: String,
    pub client_manifest_filename: String,
    pub server_consumer_manifest_filename: String,
}

impl js::Classed for ReactFlightWebpackPlugin {
    type Constructor = js::AnyConstructor;
}

#[cfg_attr(rust_js, rust_js::class)]
impl ReactFlightWebpackPlugin {
    pub fn new(options: Option<&Options>) -> Self {
        let Some(options) = options else {
            panic!("{PLUGIN_NAME}: You must specify the isServer option as a boolean.");
        };
        let Kind::Bool(is_server) = js::classify(options.is_server) else {
            panic!("{PLUGIN_NAME}: You must specify the isServer option as a boolean.");
        };
        if is_server {
            panic!("TODO: Implement the server compiler.");
        }
        let client_references = match &options.client_references {
            None => vec![ClientReferencePath::Search(ClientReferenceSearchPath {
                directory: ".".to_owned(),
                recursive: Some(true),
                include: reg_exp::new(r"\.(js|ts|jsx|tsx)$", ""),
                exclude: None,
            })],
            Some(ClientReferences::Path(path)) => vec![ClientReferencePath::Path(path.clone())],
            Some(ClientReferences::Search(search)) => {
                vec![ClientReferencePath::Search(search.clone())]
            }
            Some(ClientReferences::Many(client_references)) => client_references.clone(),
        };
        let chunk_name = match &options.chunk_name {
            Some(chunk_name) => {
                let mut chunk_name = chunk_name.clone();
                if !reg_exp::new(r"\[(index|request)\]", "").test(&chunk_name) {
                    chunk_name += "[index]";
                }
                chunk_name
            }
            None => "client[index]".to_owned(),
        };
        ReactFlightWebpackPlugin {
            client_references,
            chunk_name,
            client_manifest_filename: (options.client_manifest_filename.as_deref())
                .filter(|filename| !filename.is_empty())
                .unwrap_or("react-client-manifest.json")
                .to_owned(),
            server_consumer_manifest_filename: (options
                .server_consumer_manifest_filename
                .as_deref())
            .filter(|filename| !filename.is_empty())
            .unwrap_or("react-ssr-manifest.json")
            .to_owned(),
        }
    }

    pub fn apply(&'static self, compiler: &'static Compiler) {
        let resolved_client_references: Rc<RefCell<Option<ClientReferenceDependencies>>> =
            Rc::new(RefCell::new(None));
        let client_file_name_found = Rc::new(Cell::new(false));

        // Find all client files on the file system
        let resolved = Rc::clone(&resolved_client_references);
        compiler.hooks().before_compile().tap_async(
            PLUGIN_NAME,
            move |&CompilationParams {
                      context_module_factory,
                      ..
                  },
                  callback| {
                let context_resolver = compiler
                    .resolver_factory()
                    .get("context", Some(&ResolveOptions {}));
                let normal_resolver = compiler.resolver_factory().get("normal", None);

                let resolved = Rc::clone(&resolved);
                self.resolve_all_client_files(
                    &compiler.context(),
                    context_resolver,
                    normal_resolver,
                    compiler.input_file_system(),
                    context_module_factory,
                    Box::new(move |err, resolved_client_refs| {
                        if let Some(err) = err {
                            callback(Some(err));
                            return;
                        }

                        resolved.replace(resolved_client_refs.map(Rc::new));
                        callback(None);
                    }),
                );
            },
        );

        let resolved = Rc::clone(&resolved_client_references);
        let found = Rc::clone(&client_file_name_found);
        compiler.hooks().this_compilation().tap(
            PLUGIN_NAME,
            move |compilation,
                  &CompilationParams {
                      normal_module_factory,
                      ..
                  }| {
                compilation.dependency_factories().set(
                    js::class::<ClientReferenceDependency>(),
                    normal_module_factory,
                );
                compilation.dependency_templates().set(
                    js::class::<ClientReferenceDependency>(),
                    null_dependency::new_template(),
                );

                let resolved = Rc::clone(&resolved);
                let found = Rc::clone(&found);
                let handler = move |parser: &'static JavascriptParser| {
                    // We need to add all client references as dependency of something in the graph so
                    // Webpack knows which entries need to know about the relevant chunks and include the
                    // map in their runtime. The things that actually resolves the dependency is the Flight
                    // client runtime. So we add them as a dependency of the Flight client runtime.
                    // Anything that imports the runtime will be made aware of these chunks.
                    let resolved = Rc::clone(&resolved);
                    let found = Rc::clone(&found);
                    parser.hooks().program().tap(PLUGIN_NAME, move |_, _| {
                        let module = parser.state().module();

                        if CLIENT_FILE_NAME
                            .with(|client_file_name| module.resource() != *client_file_name)
                        {
                            return None;
                        }

                        found.set(true);

                        let resolved_client_references = resolved.borrow().clone();
                        if let Some(resolved_client_references) = resolved_client_references {
                            for (i, dep) in resolved_client_references.iter().enumerate() {
                                let chunk_name = reg_exp::replace(
                                    &reg_exp::replace(
                                        &self.chunk_name,
                                        reg_exp::new(r"\[index\]", "g"),
                                        &i.to_string(),
                                    ),
                                    reg_exp::new(r"\[request\]", "g"),
                                    &template::to_path(&dep.user_request()),
                                );

                                let block: &AsyncDependenciesBlock =
                                    webpack::async_dependencies_block::new(
                                        &ChunkGroupOptions {
                                            name: Some(&chunk_name),
                                        },
                                        None,
                                        Some(&dep.request()),
                                    );

                                block.add_dependency(Rc::clone(dep));
                                module.add_block(block);
                            }
                        }
                        None
                    });
                };

                (normal_module_factory
                    .hooks()
                    .parser()
                    .r#for("javascript/auto"))
                .tap("HarmonyModulesPlugin", handler.clone());

                (normal_module_factory
                    .hooks()
                    .parser()
                    .r#for("javascript/esm"))
                .tap("HarmonyModulesPlugin", handler.clone());

                (normal_module_factory
                    .hooks()
                    .parser()
                    .r#for("javascript/dynamic"))
                .tap("HarmonyModulesPlugin", handler);
            },
        );

        compiler.hooks().make().tap(PLUGIN_NAME, move |compilation| {
            let resolved = Rc::clone(&resolved_client_references);
            let found = Rc::clone(&client_file_name_found);
            compilation.hooks().process_assets().tap(
                &tapable::Tap {
                    name: PLUGIN_NAME,
                    stage: Some(process_assets_stage_report()),
                    ..Default::default()
                },
                move |_| {
                    if !found.get() {
                        compilation.warnings().borrow_mut().push(webpack_error::new(Some(&format!(
                            "Client runtime at {CLIENT_IMPORT_NAME} was not found. React Server Components module map file {} was not created.",
                            self.client_manifest_filename
                        ))));
                        return;
                    }

                    let configured_cross_origin_loading = compilation.output_options().cross_origin_loading();
                    let cross_origin_mode =
                        if let Some(CrossOriginLoading::Mode(configured_cross_origin_loading)) = configured_cross_origin_loading {
                            Some(if configured_cross_origin_loading == "use-credentials" {
                                configured_cross_origin_loading
                            } else {
                                "anonymous".to_owned()
                            })
                        } else {
                            None
                        };

                    let resolved_client_files: HashSet<String> = (resolved.borrow().as_deref().map(Vec::as_slice).unwrap_or_default())
                        .iter()
                        .map(|r#ref| r#ref.request())
                        .collect();

                    let client_manifest: &Dict<ClientManifestEntry> = dict::new();
                    let module_map: &'static ServerConsumerModuleMap = dict::new();
                    let ssr_bundle_config = SsrBundleConfig {
                        module_loading: ModuleLoading {
                            prefix: (compilation.output_options().public_path())
                                .filter(|prefix| !prefix.is_empty())
                                .unwrap_or_default(),
                            cross_origin: cross_origin_mode,
                        },
                        module_map,
                    };

                    // We figure out which files are always loaded by any initial chunk (entrypoint).
                    // We use this to filter out chunks that Flight will never need to load
                    let empty_set: &js::Set<str> = js::set::new(None);
                    let runtime_chunk_files = empty_set;
                    compilation.entrypoints().for_each(|entrypoint, _, _| {
                        let runtime_chunk = entrypoint.get_runtime_chunk();
                        if let Some(runtime_chunk) = runtime_chunk {
                            runtime_chunk.files().for_each(|runtime_file| {
                                runtime_chunk_files.add(runtime_file);
                            });
                        }
                    });

                    compilation.chunk_groups().iter().for_each(|chunk_group| {
                        let mut chunks: Vec<Option<Id>> = Vec::new();
                        chunk_group.chunks().iter().for_each(|c| {
                            if let Some(file) = c.files().values().next() {
                                if !(file.ends_with(".js") || file.ends_with(".mjs")) {
                                    return;
                                }
                                if file.ends_with(".hot-update.js") || file.ends_with(".hot-update.mjs") {
                                    return;
                                }
                                chunks.push(c.id());
                                chunks.push(Some(Id::Text(file.to_owned())));
                            }
                        });
                        let chunks = Rc::new(chunks);

                        let record_module = |id: &Id, module: &Module| {
                            // TODO: Hook into deps instead of the target module.
                            // That way we know by the type of dep whether to include.
                            // It also resolves conflicts when the same module is in multiple chunks.
                            let Kind::String(resource) = js::classify(js::get(module, "resource")) else {
                                return;
                            };
                            if !resolved_client_files.contains(resource) {
                                return;
                            }

                            let href = path_to_file_url(resource, None).href();

                            let ssr_exports: &'static Dict<SsrExport> = dict::new();

                            dict::set(
                                client_manifest,
                                &href,
                                ClientManifestEntry {
                                    id: id.clone(),
                                    chunks: Rc::clone(&chunks),
                                    name: "*",
                                },
                            );
                            dict::set(
                                ssr_exports,
                                "*",
                                SsrExport {
                                    specifier: href,
                                    name: "*",
                                },
                            );

                            // TODO: If this module ends up split into multiple modules, then
                            // we should encode each the chunks needed for the specific export.
                            // When the module isn't split, it doesn't matter and we can just
                            // encode the id of the whole module. This code doesn't currently
                            // deal with module splitting so is likely broken from ESM anyway.
                            /*
                            clientManifest[href + '#'] = {
                              id,
                              chunks,
                              name: '',
                            };
                            ssrExports[''] = {
                              specifier: href,
                              name: '',
                            };

                            const moduleProvidedExports = compilation.moduleGraph
                              .getExportsInfo(module)
                              .getProvidedExports();

                            if (Array.isArray(moduleProvidedExports)) {
                              moduleProvidedExports.forEach(function (name) {
                                clientManifest[href + '#' + name] = {
                                  id,
                                  chunks,
                                  name: name,
                                };
                                ssrExports[name] = {
                                  specifier: href,
                                  name: name,
                                };
                              });
                            }
                            */

                            let key = match id {
                                Id::Number(id) => js::concat_text(*id),
                                Id::Text(id) => id.clone(),
                            };
                            dict::set(module_map, &key, ssr_exports);
                        };

                        chunk_group.chunks().iter().for_each(|chunk| {
                            let chunk_modules = compilation.chunk_graph().get_chunk_modules_iterable(chunk);

                            chunk_modules.collect::<Vec<_>>().iter().for_each(|module| {
                                let module_id = compilation.chunk_graph().get_module_id(module);

                                record_module(&module_id, module);
                                // If this is a concatenation, register each child to the parent ID.
                                let modules = js::get(*module, "modules");
                                if js::truthy(modules) {
                                    let modules: &[&Module] = unsafe { js::cast(modules) };
                                    modules.iter().for_each(|concatenated_mod| {
                                        record_module(&module_id, concatenated_mod);
                                    });
                                }
                            });
                        });
                    });

                    let client_output = json::stringify_with(client_manifest, None, 2);
                    compilation.emit_asset(
                        &self.client_manifest_filename,
                        sources::raw_source(&client_output, Some(false)),
                    );
                    let ssr_output = json::stringify_with(&ssr_bundle_config, None, 2);
                    compilation.emit_asset(
                        &self.server_consumer_manifest_filename,
                        sources::raw_source(&ssr_output, Some(false)),
                    );
                },
            );
        });
    }

    // This attempts to replicate the dynamic file path resolution used for other wildcard
    // resolution in Webpack is using.
    pub fn resolve_all_client_files(
        &'static self,
        context: &str,
        context_resolver: &'static Resolver,
        normal_resolver: &'static Resolver,
        fs: &'static InputFileSystem,
        context_module_factory: &'static ContextModuleFactory,
        callback: ResolvedCallback,
    ) {
        fn has_use_client_directive(source: &str) -> bool {
            if !source.contains("use client") {
                return false;
            }
            let body = match acorn::parse(
                source,
                &acorn::Options {
                    ecma_version: 2024,
                    source_type: Some("module"),
                },
            ) {
                Ok(program) => program.body(),
                Err(_) => return false,
            };
            for node in body {
                let directive = node.directive();
                if node.r#type() != "ExpressionStatement"
                    || directive.as_deref().is_none_or(str::is_empty)
                {
                    break;
                }
                if directive.as_deref() == Some("use client") {
                    return true;
                }
            }
            false
        }

        let context = context.to_owned();
        neo_async::map(
            &self.client_references,
            move |client_reference_path: &'static ClientReferencePath,
                  cb: &'static dyn Fn(
                Option<&'static Error>,
                Option<Vec<Rc<ClientReferenceDependency>>>,
            )| {
                let client_reference_search = match client_reference_path {
                    ClientReferencePath::Path(client_reference_path) => {
                        cb(
                            None,
                            Some(vec![Rc::new(ClientReferenceDependency::new(
                                client_reference_path,
                            ))]),
                        );
                        return;
                    }
                    ClientReferencePath::Search(client_reference_search) => client_reference_search,
                };
                let context = context.clone();
                context_resolver.resolve(
                    &ResolveContextInfo {},
                    &context.clone(),
                    &client_reference_search.directory,
                    &ResolveContext {},
                    move |err, resolved_directory| {
                        if let Some(err) = err {
                            return cb(Some(err), None);
                        }
                        let Some(Resolved::Path(resolved_directory)) = resolved_directory else {
                            unreachable!("a context resolves to its directory");
                        };
                        let options = ContextModuleOptions {
                            resource: resolved_directory.clone(),
                            resource_query: Some(""),
                            recursive: client_reference_search.recursive.unwrap_or(true),
                            reg_exp: client_reference_search.include,
                            include: None,
                            exclude: client_reference_search.exclude,
                        };
                        context_module_factory.resolve_dependencies(
                            fs,
                            &options,
                            move |err2, deps| {
                                if let Some(err2) = err2 {
                                    return cb(Some(err2), None);
                                }

                                let client_ref_deps: Vec<Rc<ClientReferenceDependency>> = deps
                                    .iter()
                                    .map(|dep| {
                                        // use userRequest instead of request. request always end with undefined which is wrong
                                        let request =
                                            join((&resolved_directory, &dep.user_request()));
                                        let client_ref_dep =
                                            ClientReferenceDependency::new(&request);
                                        client_ref_dep.set_user_request(&dep.user_request());
                                        Rc::new(client_ref_dep)
                                    })
                                    .collect();

                                neo_async::filter(
                                    &client_ref_deps,
                                    move |client_ref_dep: &'static Rc<
                                        ClientReferenceDependency,
                                    >,
                                          filter_cb: &'static dyn Fn(
                                        Option<&'static Error>,
                                        bool,
                                    )| {
                                        normal_resolver.resolve(
                                            &ResolveContextInfo {},
                                            &context,
                                            &client_ref_dep.request(),
                                            &ResolveContext {},
                                            move |err3, resolved_path| {
                                                let (None, Some(Resolved::Path(resolved_path))) =
                                                    (err3, resolved_path)
                                                else {
                                                    return filter_cb(None, false);
                                                };
                                                fs.read_file(
                                                    &resolved_path,
                                                    "utf-8",
                                                    move |err4, content| {
                                                        let (None, Kind::String(content)) =
                                                            (err4, js::classify(content))
                                                        else {
                                                            return filter_cb(None, false);
                                                        };
                                                        let use_client =
                                                            has_use_client_directive(content);
                                                        filter_cb(None, use_client);
                                                    },
                                                );
                                            },
                                        );
                                    },
                                    cb,
                                );
                            },
                        );
                    },
                );
            },
            move |err, result: Vec<Vec<Rc<ClientReferenceDependency>>>| {
                if let Some(err) = err {
                    return callback(Some(err), None);
                }
                let mut flat = Vec::new();
                for deps in result {
                    flat.extend(deps);
                }
                callback(None, Some(flat));
            },
        );
    }
}

js::export_default!(ReactFlightWebpackPlugin);
