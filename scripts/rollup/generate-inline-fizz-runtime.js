'use strict';

const fs = require('fs');
const os = require('os');
const path = require('path');
const ClosureCompiler = require('google-closure-compiler').compiler;
const {getNativeImagePath} = require('google-closure-compiler/lib/utils');
const {rollup} = require('rollup');
const resolve = require('@rollup/plugin-node-resolve').default;

const instructionDir =
  './packages/react-dom-bindings/src/server/fizz-instruction-set';

// This is the name of the generated file that exports the inline instruction
// set as strings: the Rust rust-js writes its JS from.
const inlineCodeStringsFilename =
  instructionDir + '/ReactDOMFizzInstructionSetInlineCodeStrings.rs';

const config = [
  {
    entry: 'ReactDOMFizzInlineShellTime.js',
    exportName: 'markShellTime',
  },
  {
    entry: 'ReactDOMFizzInlineClientRenderBoundary.js',
    exportName: 'clientRenderBoundary',
  },
  {
    entry: 'ReactDOMFizzInlineCompleteBoundary.js',
    exportName: 'completeBoundary',
  },
  {
    entry: 'ReactDOMFizzInlineCompleteBoundaryUpgradeToViewTransitions.js',
    exportName: 'completeBoundaryUpgradeToViewTransitions',
  },
  {
    entry: 'ReactDOMFizzInlineCompleteBoundaryWithStyles.js',
    exportName: 'completeBoundaryWithStyles',
  },
  {
    entry: 'ReactDOMFizzInlineCompleteSegment.js',
    exportName: 'completeSegment',
  },
  {
    entry: 'ReactDOMFizzInlineFormReplaying.js',
    exportName: 'formReplaying',
  },
];

// A Rust string literal of `text`: raw, with as many `#`s as it needs.
function rustString(text) {
  let hashes = '';
  while (text.includes('"' + hashes)) {
    hashes += '#';
  }
  return 'r' + hashes + '"' + text + '"' + hashes;
}

async function main() {
  const bundles = fs.mkdtempSync(path.join(os.tmpdir(), 'inline-fizz-'));
  const exportStatements = await Promise.all(
    config.map(async ({entry, exportName}) => {
      const fullEntryPath = instructionDir + '/' + entry;
      // The entry, its instruction set and the @rust-js/runtime helpers that
      // rust-js's JS of them imports, one module: the helpers they call,
      // as the runtime is tree-shakable.
      const bundle = await rollup({input: fullEntryPath, plugins: [resolve()]});
      const {output} = await bundle.generate({format: 'es'});
      const bundled = output[0].code;
      const bundledEntryPath = path.join(bundles, entry);
      fs.writeFileSync(bundledEntryPath, bundled);
      // Each property the bundle reads or writes is one Closure keeps by its
      // name: rust-js's JS reads `window.$RC`, `node.dataset.precedence` and
      // `suspenseNode._reactRetry` as properties, where upstream's source
      // wrote them `window['$RC']` so that ADVANCED mode wouldn't rename
      // them. So it renames locals alone (rd274).
      // A property read, `x.name`, or a key, `{name: x}`, of an object
      // literal or a destructuring.
      const properties = new Set(
        Array.from(
          bundled.matchAll(/\.([A-Za-z_$][\w$]*)|[{,]\s*([A-Za-z_$][\w$]*)\s*:/g),
          match => match[1] ?? match[2],
        ),
      );
      const propertyExterns = path.join(bundles, entry + '.externs.js');
      fs.writeFileSync(
        propertyExterns,
        '/** @externs */\n' +
          Array.from(properties, name => `/** @type {?} */ Object.prototype.${name};`).join('\n') +
          '\n',
      );
      const compiler = new ClosureCompiler({
        entry_point: bundledEntryPath,
        js: [require.resolve('./externs/closure-externs.js'), propertyExterns, bundledEntryPath],
        compilation_level: 'ADVANCED',
        language_in: 'ECMASCRIPT_2020',
        language_out: 'ECMASCRIPT5_STRICT',
        // This is necessary to prevent Closure from inlining a Promise polyfill
        rewrite_polyfills: false,
      });
      // Closure's own binary where it's installed, else its jar.
      const nativeImagePath = getNativeImagePath();
      if (nativeImagePath) {
        compiler.JAR_PATH = null;
        compiler.javaPath = nativeImagePath;
      }

      const code = await new Promise((resolve, reject) => {
        compiler.run((exitCode, stdOut, stdErr) => {
          if (exitCode !== 0) {
            reject(new Error(stdErr));
          } else {
            resolve(stdOut);
          }
        });
      });

      // Each `$R*` global the entry sets is set in what Closure wrote.
      for (const [, global] of bundled.matchAll(/window\.(\$R\w*)\s*=[^=]/g)) {
        if (!code.includes('window.' + global + '=')) {
          throw new Error(`${entry}: Closure's output doesn't set window.${global}: ${code}`);
        }
      }
      const constName = exportName.replace(/[A-Z]/g, c => '_' + c).toUpperCase();
      return [
        `#[cfg_attr(rust_js, rust_js::name = "${exportName}")]`,
        `pub const ${constName}: &str = ${rustString(code.trim().replace('\n', ''))};`,
      ].join('\n');
    })
  );
  fs.rmSync(bundles, {recursive: true, force: true});

  let outputCode = [
    '// This is a generated file. The source files are in react-dom-bindings/src/server/fizz-instruction-set.',
    '// The build script is at scripts/rollup/generate-inline-fizz-runtime.js.',
    '// Run `yarn generate-inline-fizz-runtime` to generate.',
    ...exportStatements,
  ].join('\n') + '\n';

  // This replaces "window.$globalVar" with "$globalVar". There's probably a
  // better way to do this with Closure, with externs or something, but I
  // couldn't figure it out. Good enough for now. This only affects the inline
  // Fizz runtime, and should break immediately if there were a mistake, so I'm
  // not too worried about it.
  outputCode = outputCode.replace(
    /window\.(\$[A-z0-9_]*|matchMedia)/g,
    (_, variableName) => variableName
  );

  fs.writeFileSync(inlineCodeStringsFilename, outputCode, 'utf8');
}

main().catch(err => {
  console.error(err);
  process.exit(1);
});
