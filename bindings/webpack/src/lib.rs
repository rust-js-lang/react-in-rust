//! Bindings of [webpack](https://webpack.js.org) 5.82, what a plugin of its
//! compiler uses, as webpack's own `types.d.ts` types it: the
//! [`Compiler`] a plugin's `apply` is given, its hooks and the
//! [`Compilation`] each makes, the chunks, modules and assets a compilation
//! has, and the dependency classes a plugin extends, [`ModuleDependency`].
//!
//! ```rust,ignore
//! use webpack::Compiler;
//!
//! fn apply(compiler: &Compiler) {
//!     compiler.hooks().make().tap("My Plugin", |compilation| { .. });
//! }
//! ```

// A binding's body never runs, so rustc's crate sees its parameters unread
// (rd330, the user's one crate-level allow for a bindings crate).
#![cfg_attr(not(rust_js), allow(unused_variables))]

use std::cell::RefCell;
use std::marker::PhantomData;
use std::ops::Deref;
use std::rc::Rc;

use js::{Class, Error, JsObject, Map, RegExp, Set, Unknown};
use tapable::{AsyncParallelHook, AsyncSeriesHook, HookMap, SyncBailHook, SyncHook};

/// webpack's `Compiler`: what a plugin's `apply` is given, which runs
/// each compilation.
#[cfg_attr(rust_js, rust_js::types = "webpack#Compiler")]
pub struct Compiler(PhantomData<JsObject>);

impl Compiler {
    /// `compiler.hooks`: what a plugin taps.
    #[cfg_attr(rust_js, rust_js::link_name = "get hooks")]
    pub fn hooks(&self) -> &'static CompilerHooks {
        unreachable!()
    }

    /// `compiler.resolverFactory`: what makes its resolvers.
    #[cfg_attr(rust_js, rust_js::link_name = "get resolverFactory")]
    pub fn resolver_factory(&self) -> &'static ResolverFactory {
        unreachable!()
    }

    /// `compiler.inputFileSystem`: what it reads files from.
    #[cfg_attr(rust_js, rust_js::link_name = "get inputFileSystem")]
    pub fn input_file_system(&self) -> &'static InputFileSystem {
        unreachable!()
    }

    /// `compiler.context`: the directory it resolves from.
    #[cfg_attr(rust_js, rust_js::link_name = "get context")]
    pub fn context(&self) -> String {
        unreachable!()
    }
}

/// `Compiler`'s `hooks`.
pub struct CompilerHooks(PhantomData<JsObject>);

impl CompilerHooks {
    /// `beforeCompile`: before a compilation's params are used.
    #[cfg_attr(rust_js, rust_js::link_name = "get beforeCompile")]
    pub fn before_compile(&self) -> &'static AsyncSeriesHook<(&'static CompilationParams,)> {
        unreachable!()
    }

    /// `thisCompilation`: a compilation made, before its `compilation`
    /// hook, and not of a child compiler.
    #[cfg_attr(rust_js, rust_js::link_name = "get thisCompilation")]
    pub fn this_compilation(
        &self,
    ) -> &'static SyncHook<(&'static Compilation, &'static CompilationParams)> {
        unreachable!()
    }

    /// `make`: a compilation to build its modules.
    #[cfg_attr(rust_js, rust_js::link_name = "get make")]
    pub fn make(&self) -> &'static AsyncParallelHook<(&'static Compilation,)> {
        unreachable!()
    }
}

/// webpack's `CompilationParams`: the module factories a compilation is
/// made with.
#[cfg_attr(rust_js, rust_js::types = "webpack#CompilationParams")]
pub struct CompilationParams {
    pub normal_module_factory: &'static NormalModuleFactory,
    pub context_module_factory: &'static ContextModuleFactory,
}

/// webpack's `Compilation`: one build of the modules, its chunks and its
/// assets.
#[cfg_attr(rust_js, rust_js::types = "webpack#Compilation")]
pub struct Compilation(PhantomData<JsObject>);

pub mod compilation {
    /// `Compilation.PROCESS_ASSETS_STAGE_REPORT`: the stage of
    /// `processAssets` for reports of the assets, after they're made.
    #[cfg_attr(
        rust_js,
        rust_js::link_name = "get webpack#Compilation.PROCESS_ASSETS_STAGE_REPORT"
    )]
    pub fn process_assets_stage_report() -> f64 {
        unreachable!()
    }
}

impl Compilation {
    /// `compilation.hooks`.
    #[cfg_attr(rust_js, rust_js::link_name = "get hooks")]
    pub fn hooks(&self) -> &'static CompilationHooks {
        unreachable!()
    }

    /// `compilation.dependencyFactories`: the factory of each dependency
    /// class's modules.
    #[cfg_attr(rust_js, rust_js::link_name = "get dependencyFactories")]
    pub fn dependency_factories(&self) -> &'static Map<Class, &'static ModuleFactory> {
        unreachable!()
    }

    /// `compilation.dependencyTemplates`: the template of each dependency
    /// class.
    #[cfg_attr(rust_js, rust_js::link_name = "get dependencyTemplates")]
    pub fn dependency_templates(&self) -> &'static DependencyTemplates {
        unreachable!()
    }

    /// `compilation.warnings`: its warnings, which a plugin pushes to.
    #[cfg_attr(rust_js, rust_js::link_name = "get warnings")]
    pub fn warnings(&self) -> &'static RefCell<Vec<&'static WebpackError>> {
        unreachable!()
    }

    /// `compilation.outputOptions`.
    #[cfg_attr(rust_js, rust_js::link_name = "get outputOptions")]
    pub fn output_options(&self) -> &'static OutputNormalized {
        unreachable!()
    }

    /// `compilation.entrypoints`: each entry's chunk group, by its name.
    #[cfg_attr(rust_js, rust_js::link_name = "get entrypoints")]
    pub fn entrypoints(&self) -> &'static Map<str, &'static Entrypoint> {
        unreachable!()
    }

    /// `compilation.chunkGroups`.
    #[cfg_attr(rust_js, rust_js::link_name = "get chunkGroups")]
    pub fn chunk_groups(&self) -> Vec<&'static ChunkGroup> {
        unreachable!()
    }

    /// `compilation.chunkGraph`.
    #[cfg_attr(rust_js, rust_js::link_name = "get chunkGraph")]
    pub fn chunk_graph(&self) -> &'static ChunkGraph {
        unreachable!()
    }

    /// `compilation.emitAsset(file, source, assetInfo?)`: `source` written
    /// as `file`.
    #[cfg_attr(rust_js, rust_js::link_name = "emitAsset")]
    pub fn emit_asset(&self, file: &str, source: &Source) {
        unreachable!()
    }
}

/// `Compilation`'s `hooks`.
pub struct CompilationHooks(PhantomData<JsObject>);

impl CompilationHooks {
    /// `processAssets`: the assets made, by stage.
    #[cfg_attr(rust_js, rust_js::link_name = "get processAssets")]
    pub fn process_assets(&self) -> &'static AsyncSeriesHook<(&'static CompilationAssets,)> {
        unreachable!()
    }
}

/// webpack's `CompilationAssets`: each asset's source, by its name.
pub struct CompilationAssets(PhantomData<JsObject>);

/// webpack's `OutputNormalized`: the `output` options.
pub struct OutputNormalized(PhantomData<JsObject>);

impl OutputNormalized {
    /// `crossOriginLoading`: how chunks are loaded, `false | "anonymous" |
    /// "use-credentials"`, `None` where it isn't set.
    #[cfg_attr(rust_js, rust_js::link_name = "get crossOriginLoading")]
    pub fn cross_origin_loading(&self) -> Option<CrossOriginLoading> {
        unreachable!()
    }

    /// `publicPath`: where the assets are served from, `None` where it
    /// isn't set.
    #[cfg_attr(rust_js, rust_js::link_name = "get publicPath")]
    pub fn public_path(&self) -> Option<String> {
        unreachable!()
    }
}

/// `crossOriginLoading`'s `false | "anonymous" | "use-credentials"`: off,
/// or the mode, by its name.
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum CrossOriginLoading {
    Off(bool),
    Mode(String),
}

/// webpack's `ChunkGroup`: chunks loaded together.
pub struct ChunkGroup(PhantomData<JsObject>);

impl ChunkGroup {
    /// `chunkGroup.chunks`.
    #[cfg_attr(rust_js, rust_js::link_name = "get chunks")]
    pub fn chunks(&self) -> Vec<&'static Chunk> {
        unreachable!()
    }
}

/// webpack's `Entrypoint`: an entry's chunk group.
pub struct Entrypoint(PhantomData<JsObject>);

impl Deref for Entrypoint {
    type Target = ChunkGroup;

    fn deref(&self) -> &ChunkGroup {
        // Never runs: rust-js compiles this `Deref` to the object itself.
        unsafe { &*(self as *const Self as *const ChunkGroup) }
    }
}

impl Entrypoint {
    /// `entrypoint.getRuntimeChunk()`: the chunk of webpack's bootstrap
    /// code, `null` of none.
    #[cfg_attr(rust_js, rust_js::link_name = "getRuntimeChunk")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub fn get_runtime_chunk(&self) -> Option<&'static Chunk> {
        unreachable!()
    }
}

/// webpack's `Chunk`.
#[cfg_attr(rust_js, rust_js::types = "webpack#Chunk")]
pub struct Chunk(PhantomData<JsObject>);

impl Chunk {
    /// `chunk.id`: its id, `null` before it has one.
    #[cfg_attr(rust_js, rust_js::link_name = "get id")]
    #[cfg_attr(rust_js, rust_js::nullable)]
    pub fn id(&self) -> Option<Id> {
        unreachable!()
    }

    /// `chunk.files`: the files it's written as.
    #[cfg_attr(rust_js, rust_js::link_name = "get files")]
    pub fn files(&self) -> &'static Set<str> {
        unreachable!()
    }
}

/// A chunk's or a module's id, `string | number`.
#[derive(Clone)]
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum Id {
    Number(f64),
    Text(String),
}

/// webpack's `ChunkGraph`: which modules each chunk holds.
#[cfg_attr(rust_js, rust_js::types = "webpack#ChunkGraph")]
pub struct ChunkGraph(PhantomData<JsObject>);

impl ChunkGraph {
    /// `chunkGraph.getChunkModulesIterable(chunk)`: the modules `chunk`
    /// holds.
    #[cfg_attr(rust_js, rust_js::link_name = "getChunkModulesIterable")]
    pub fn get_chunk_modules_iterable(
        &self,
        chunk: &Chunk,
    ) -> Box<dyn Iterator<Item = &'static Module>> {
        unreachable!()
    }

    /// `chunkGraph.getModuleId(module)`: `module`'s id.
    #[cfg_attr(rust_js, rust_js::link_name = "getModuleId")]
    pub fn get_module_id(&self, module: &Module) -> Id {
        unreachable!()
    }
}

/// webpack's `DependenciesBlock`: what holds dependencies, and blocks of
/// them loaded later.
pub struct DependenciesBlock(PhantomData<JsObject>);

impl DependenciesBlock {
    /// `block.addBlock(block)`: `block`, a block of what's loaded later.
    #[cfg_attr(rust_js, rust_js::link_name = "addBlock")]
    pub fn add_block(&self, block: &AsyncDependenciesBlock) {
        unreachable!()
    }

    /// `block.addDependency(dependency)`, which the block keeps.
    #[cfg_attr(rust_js, rust_js::link_name = "addDependency")]
    pub fn add_dependency<D: Dependency>(&self, dependency: Rc<D>) {
        unreachable!()
    }
}

/// webpack's `Module`, of any kind: what its properties are is its kind's,
/// a `NormalModule`'s `resource`, which TypeScript's `Module` doesn't
/// declare, so a plugin given any reads them as a value of any shape.
#[cfg_attr(rust_js, rust_js::types = "webpack#Module")]
pub struct Module(PhantomData<JsObject>);

unsafe impl js::Defined for Module {}

impl Deref for Module {
    type Target = DependenciesBlock;

    fn deref(&self) -> &DependenciesBlock {
        // Never runs: rust-js compiles this `Deref` to the object itself.
        unsafe { &*(self as *const Self as *const DependenciesBlock) }
    }
}

/// webpack's `NormalModule`: a module of a file.
#[cfg_attr(rust_js, rust_js::types = "webpack#NormalModule")]
pub struct NormalModule(PhantomData<JsObject>);

impl Deref for NormalModule {
    type Target = Module;

    fn deref(&self) -> &Module {
        // Never runs: rust-js compiles this `Deref` to the object itself.
        unsafe { &*(self as *const Self as *const Module) }
    }
}

impl NormalModule {
    /// `module.resource`: its file's path, with its query.
    #[cfg_attr(rust_js, rust_js::link_name = "get resource")]
    pub fn resource(&self) -> String {
        unreachable!()
    }
}

/// webpack's `AsyncDependenciesBlock`: dependencies loaded later, as a
/// chunk of their own.
#[cfg_attr(rust_js, rust_js::types = "webpack#AsyncDependenciesBlock")]
pub struct AsyncDependenciesBlock(PhantomData<JsObject>);

impl Deref for AsyncDependenciesBlock {
    type Target = DependenciesBlock;

    fn deref(&self) -> &DependenciesBlock {
        // Never runs: rust-js compiles this `Deref` to the object itself.
        unsafe { &*(self as *const Self as *const DependenciesBlock) }
    }
}

pub mod async_dependencies_block {
    use super::{AsyncDependenciesBlock, ChunkGroupOptions, DependencyLocation};

    /// `new AsyncDependenciesBlock(groupOptions, loc?, request?)`: a block
    /// of the chunk `groupOptions` names, `loc` where it's from, `null` of
    /// nowhere.
    #[cfg_attr(rust_js, rust_js::link_name = "new webpack#AsyncDependenciesBlock")]
    #[cfg_attr(rust_js, rust_js::nullable(loc))]
    pub fn new(
        group_options: &ChunkGroupOptions<'_>,
        loc: Option<&DependencyLocation>,
        request: Option<&str>,
    ) -> &'static AsyncDependenciesBlock {
        unreachable!()
    }
}

/// `RawChunkGroupOptions & { name?: string }`: what a chunk group is made
/// of, each `None` left out.
#[derive(Clone, Copy, Default)]
pub struct ChunkGroupOptions<'a> {
    pub name: Option<&'a str>,
}

/// webpack's `DependencyLocation`: where in its module a dependency is.
pub struct DependencyLocation(PhantomData<JsObject>);

/// A webpack `Dependency`, of any class: what a block holds.
pub trait Dependency {}

impl<T: ModuleDependency> Dependency for T {}

/// webpack's `ModuleDependency`, `webpack/lib/dependencies/ModuleDependency`'s
/// default export: a dependency on the module a request names, a class a
/// plugin extends, `class ClientReferenceDependency extends ModuleDependency
/// { get type() { .. } }`, constructed with its request, `super(request)`.
#[cfg_attr(
    rust_js,
    rust_js::types = "webpack/lib/dependencies/ModuleDependency#default"
)]
pub trait ModuleDependency: 'static {
    /// `get type()`: its kind, by name.
    #[cfg_attr(rust_js, rust_js::getter)]
    fn r#type(&self) -> &'static str;

    /// `dependency.request`: the module's request, as it's resolved.
    #[cfg_attr(rust_js, rust_js::link_name = "get request")]
    fn request(&self) -> String {
        unreachable!()
    }

    /// `dependency.userRequest`: the request as it was written.
    #[cfg_attr(rust_js, rust_js::link_name = "get userRequest")]
    fn user_request(&self) -> String {
        unreachable!()
    }

    /// `dependency.userRequest = userRequest`.
    #[cfg_attr(rust_js, rust_js::link_name = "set userRequest")]
    fn set_user_request(&self, user_request: &str) {
        unreachable!()
    }
}

/// webpack's `ContextElementDependency`: a module a context module finds.
pub struct ContextElementDependency(PhantomData<JsObject>);

impl ContextElementDependency {
    /// `dependency.userRequest`: its request, relative to the context.
    #[cfg_attr(rust_js, rust_js::link_name = "get userRequest")]
    pub fn user_request(&self) -> String {
        unreachable!()
    }
}

/// webpack's `DependencyTemplate`: what writes a dependency's code.
pub struct DependencyTemplate(PhantomData<JsObject>);

/// webpack's `NullDependencyTemplate`, `NullDependency.Template`: a
/// template that writes nothing.
pub struct NullDependencyTemplate(PhantomData<JsObject>);

impl Deref for NullDependencyTemplate {
    type Target = DependencyTemplate;

    fn deref(&self) -> &DependencyTemplate {
        // Never runs: rust-js compiles this `Deref` to the object itself.
        unsafe { &*(self as *const Self as *const DependencyTemplate) }
    }
}

pub mod null_dependency {
    use super::NullDependencyTemplate;

    /// `new NullDependency.Template()`, of
    /// `webpack/lib/dependencies/NullDependency`'s default export.
    #[cfg_attr(
        rust_js,
        rust_js::link_name = "new webpack/lib/dependencies/NullDependency#default.Template"
    )]
    pub fn new_template() -> &'static NullDependencyTemplate {
        unreachable!()
    }
}

/// webpack's `DependencyTemplates`: the template of each dependency class.
pub struct DependencyTemplates(PhantomData<JsObject>);

impl DependencyTemplates {
    /// `templates.set(dependency, dependencyTemplate)`.
    #[cfg_attr(rust_js, rust_js::link_name = "set")]
    pub fn set(&self, dependency: &Class, dependency_template: &DependencyTemplate) {
        unreachable!()
    }
}

/// webpack's `ModuleFactory`: what makes a dependency's module.
pub struct ModuleFactory(PhantomData<JsObject>);

/// webpack's `NormalModuleFactory`: what makes the modules of files.
#[cfg_attr(rust_js, rust_js::types = "webpack#NormalModuleFactory")]
pub struct NormalModuleFactory(PhantomData<JsObject>);

impl Deref for NormalModuleFactory {
    type Target = ModuleFactory;

    fn deref(&self) -> &ModuleFactory {
        // Never runs: rust-js compiles this `Deref` to the object itself.
        unsafe { &*(self as *const Self as *const ModuleFactory) }
    }
}

impl NormalModuleFactory {
    /// `normalModuleFactory.hooks`.
    #[cfg_attr(rust_js, rust_js::link_name = "get hooks")]
    pub fn hooks(&self) -> &'static NormalModuleFactoryHooks {
        unreachable!()
    }
}

/// `NormalModuleFactory`'s `hooks`.
pub struct NormalModuleFactoryHooks(PhantomData<JsObject>);

impl NormalModuleFactoryHooks {
    /// `parser`: each module type's parser, made, given to its taps.
    #[cfg_attr(rust_js, rust_js::link_name = "get parser")]
    pub fn parser(&self) -> &'static HookMap<SyncHook<(&'static JavascriptParser,)>> {
        unreachable!()
    }
}

/// webpack's `ContextModuleFactory`: what makes the modules of a
/// directory's files.
#[cfg_attr(rust_js, rust_js::types = "webpack#ContextModuleFactory")]
pub struct ContextModuleFactory(PhantomData<JsObject>);

impl ContextModuleFactory {
    /// `contextModuleFactory.resolveDependencies(fs, options, callback)`:
    /// the dependencies of each file `options` finds, given to `callback`,
    /// or what failed.
    #[cfg_attr(rust_js, rust_js::link_name = "resolveDependencies")]
    pub fn resolve_dependencies(
        &self,
        fs: &InputFileSystem,
        options: &ContextModuleOptions<'_>,
        callback: impl FnOnce(Option<&'static Error>, Vec<&'static ContextElementDependency>) + 'static,
    ) {
        unreachable!()
    }
}

/// webpack's `ContextModuleOptions`: what a context module finds, each
/// `None` left out.
pub struct ContextModuleOptions<'a> {
    pub resource: String,
    pub resource_query: Option<&'a str>,
    pub recursive: bool,
    pub reg_exp: &'a RegExp,
    pub include: Option<&'a RegExp>,
    pub exclude: Option<&'a RegExp>,
}

/// webpack's `JavascriptParser`: what parses a module's JS.
pub struct JavascriptParser(PhantomData<JsObject>);

impl JavascriptParser {
    /// `parser.hooks`.
    #[cfg_attr(rust_js, rust_js::link_name = "get hooks")]
    pub fn hooks(&self) -> &'static JavascriptParserHooks {
        unreachable!()
    }

    /// `parser.state`: what it's parsing.
    #[cfg_attr(rust_js, rust_js::link_name = "get state")]
    pub fn state(&self) -> &'static ParserState {
        unreachable!()
    }
}

/// `JavascriptParser`'s `hooks`.
pub struct JavascriptParserHooks(PhantomData<JsObject>);

impl JavascriptParserHooks {
    /// `program`: a module's program parsed, with its comments.
    #[cfg_attr(rust_js, rust_js::link_name = "get program")]
    pub fn program(
        &self,
    ) -> &'static SyncBailHook<(&'static Program, &'static [&'static Comment]), Option<bool>> {
        unreachable!()
    }
}

/// ESTree's `Program`, a module's AST.
pub struct Program(PhantomData<JsObject>);

/// ESTree's `Comment`.
pub struct Comment(PhantomData<JsObject>);

/// webpack's `ParserState`: what a parser is parsing.
pub struct ParserState(PhantomData<JsObject>);

impl ParserState {
    /// `state.module`: the module it's parsing.
    #[cfg_attr(rust_js, rust_js::link_name = "get module")]
    pub fn module(&self) -> &'static NormalModule {
        unreachable!()
    }
}

/// webpack's `ResolverFactory`: what makes a resolver of each kind.
pub struct ResolverFactory(PhantomData<JsObject>);

impl ResolverFactory {
    /// `resolverFactory.get(type, resolveOptions?)`: the resolver of
    /// `type`, `"normal"` or `"context"`.
    #[cfg_attr(rust_js, rust_js::link_name = "get")]
    pub fn get(&self, r#type: &str, resolve_options: Option<&ResolveOptions>) -> &'static Resolver {
        unreachable!()
    }
}

/// webpack's `ResolveOptionsWithDependencyType`, of none of its options.
#[derive(Clone, Copy, Default)]
pub struct ResolveOptions {}

/// enhanced-resolve's `Resolver`, webpack's `ResolverWithOptions`.
pub struct Resolver(PhantomData<JsObject>);

impl Resolver {
    /// `resolver.resolve(context, path, request, resolveContext, callback)`:
    /// the path `request` resolves to from `path`, given to `callback`,
    /// `false` of one ignored, or what failed.
    #[cfg_attr(rust_js, rust_js::link_name = "resolve")]
    pub fn resolve(
        &self,
        context: &ResolveContextInfo,
        path: &str,
        request: &str,
        resolve_context: &ResolveContext,
        callback: impl FnOnce(Option<&'static Error>, Option<Resolved>) + 'static,
    ) {
        unreachable!()
    }
}

/// What a resolver resolves to, `string | false`.
#[cfg_attr(rust_js, rust_js::untagged)]
pub enum Resolved {
    Path(String),
    Ignored(bool),
}

/// What `resolve`'s `context`, an `object`, says of the issuer: nothing.
#[derive(Clone, Copy, Default)]
pub struct ResolveContextInfo {}

/// enhanced-resolve's `ResolveContext`, of none of its sets.
#[derive(Clone, Copy, Default)]
pub struct ResolveContext {}

/// webpack's `InputFileSystem`.
pub struct InputFileSystem(PhantomData<JsObject>);

impl InputFileSystem {
    /// `fs.readFile(path, encoding, callback)`: the file's text, given to
    /// `callback`, any value, a `Buffer` where it ignores `encoding`, or
    /// what failed.
    #[cfg_attr(rust_js, rust_js::link_name = "readFile")]
    pub fn read_file(
        &self,
        path: &str,
        encoding: &str,
        callback: impl FnOnce(Option<&'static Error>, Option<&'static Unknown>) + 'static,
    ) {
        unreachable!()
    }
}

/// webpack-sources' `Source`: an asset's content.
pub struct Source(PhantomData<JsObject>);

/// webpack-sources' `RawSource`, `webpack.sources.RawSource`: a source of
/// text as it is.
pub struct RawSource(PhantomData<JsObject>);

impl Deref for RawSource {
    type Target = Source;

    fn deref(&self) -> &Source {
        // Never runs: rust-js compiles this `Deref` to the object itself.
        unsafe { &*(self as *const Self as *const Source) }
    }
}

pub mod sources {
    use super::RawSource;

    /// `new sources.RawSource(source, convertToString?)`.
    #[cfg_attr(rust_js, rust_js::link_name = "new webpack#sources.RawSource")]
    pub fn raw_source(source: &str, convert_to_string: Option<bool>) -> &'static RawSource {
        unreachable!()
    }
}

/// webpack's `WebpackError`: an `Error` of webpack's, a compilation's
/// warning or error.
#[cfg_attr(rust_js, rust_js::types = "webpack#WebpackError")]
pub struct WebpackError(PhantomData<JsObject>);

impl Deref for WebpackError {
    type Target = Error;

    fn deref(&self) -> &Error {
        // Never runs: rust-js compiles this `Deref` to the object itself.
        unsafe { &*(self as *const Self as *const Error) }
    }
}

pub mod webpack_error {
    use super::WebpackError;

    /// `new WebpackError(message?)`.
    #[cfg_attr(rust_js, rust_js::link_name = "new webpack#WebpackError")]
    pub fn new(message: Option<&str>) -> &'static WebpackError {
        unreachable!()
    }
}

pub mod template {
    /// `Template.toPath(str)`, of `webpack/lib/Template`'s default export:
    /// `str` as a file's path, its other characters `-`.
    #[cfg_attr(rust_js, rust_js::link_name = "webpack/lib/Template#default.toPath")]
    pub fn to_path(str: &str) -> String {
        unreachable!()
    }
}
