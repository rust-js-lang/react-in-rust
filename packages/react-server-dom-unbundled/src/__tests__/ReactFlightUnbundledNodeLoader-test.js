/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @emails react-core
 * @jest-environment node
 */

'use strict';

// The port's own test: no upstream test runs the Node loader, which Node
// runs as a module hook (`node --import` / `--experimental-loader`). Its
// hooks are called here as Node calls them, with Node's next hooks faked.
// Upstream's unbundled loader is webpack's, byte for byte: the modules it
// writes import "react-server-dom-webpack/server", as here.
let Loader;

const files = {
  'file:///app/plain.js': 'export const a = 1;\n',
  'file:///app/client.js':
    '"use client";\n' +
    'export const A = 1, {b, c: [d, , ...e]} = obj;\n' +
    'export function F() {}\n' +
    'export default function () {}\n' +
    'export {x as y};\n' +
    'export * from "./other.js";\n' +
    'export * as ns from "./unread.js";\n',
  'file:///app/other.js':
    'export const O = 1;\nexport * from "./deep.js";\nexport default 3;\n',
  'file:///app/deep.js': 'export let D;\n',
  'file:///app/server.js':
    "'use strict';\n" +
    "'use server';\n" +
    'export async function action() {}\n' +
    'export const value = 1;\n' +
    'let hidden = 1;\n' +
    'export {hidden as renamed, hidden as "string name"};\n' +
    'export default action;\n',
  'file:///app/mapped.js':
    "'use server';\n\nexport async function act() {}\n\nexport const v = 1;\n" +
    '//# sourceMappingURL=mapped.js.map\n',
  'mapped.js.map': JSON.stringify({
    version: 3,
    sources: ['original.ts'],
    names: ['act', 'v'],
    mappings: 'AAAA;;AAEA,sBAAsBA;;AAEtB,MAAMC;',
  }),
  'file:///app/both.js': "'use client';\n'use server';\nexport const a = 1;\n",
  'file:///app/late.js': "const a = 1;\n'use client';\nexport const b = 2;\n",
};

function nextLoad(calls) {
  return async function defaultLoad(url, context) {
    calls.push(['load', url, context]);
    if (url.endsWith('.cjs')) {
      return {format: 'commonjs', source: "'use client'"};
    }
    return {
      format: url.endsWith('.map') ? 'json' : 'module',
      source: url.endsWith('.map') ? Buffer.from(files[url]) : files[url],
    };
  };
}

function nextResolve(calls) {
  return async function defaultResolve(specifier, context) {
    calls.push(['resolve', specifier, context]);
    return {url: new URL(specifier, context.parentURL ?? 'file:///app/').href};
  };
}

function inlineSourceMap(source) {
  const prefix =
    '//# sourceMappingURL=data:application/json;charset=utf-8;base64,';
  const at = source.lastIndexOf(prefix);
  return JSON.parse(
    Buffer.from(source.slice(at + prefix.length), 'base64').toString('utf8'),
  );
}

const DEFAULT_EXPORT =
  'export default registerClientReference(function() {throw new Error(' +
  '"Attempted to call the default export of file:///app/client.js from the server ' +
  "but it's on the client. It's not possible to invoke a client function from the " +
  'server, it can only be rendered as a Component or passed to props of a Client ' +
  'Component.");},"file:///app/client.js","default");';

describe('ReactFlightUnbundledNodeLoader', () => {
  beforeEach(() => {
    jest.resetModules();
    Loader = require('react-server-dom-unbundled/src/ReactFlightUnbundledNodeLoader');
  });

  async function resolveFirst(calls) {
    spyOnDevAndProd(console, 'warn').mockImplementation(() => {});
    await Loader.resolve(
      './index.js',
      {conditions: ['node', 'import'], parentURL: 'file:///app/', extra: 1},
      nextResolve(calls),
    );
  }

  it('adds the react-server condition once, warning once, and keeps the rest of the context', async () => {
    const calls = [];
    const warn = spyOnDevAndProd(console, 'warn').mockImplementation(() => {});
    const context = {
      conditions: ['node'],
      parentURL: 'file:///app/',
      importAttributes: {type: 'json'},
    };
    const resolved = await Loader.resolve(
      './a.js',
      context,
      nextResolve(calls),
    );
    await Loader.resolve('./b.js', context, nextResolve(calls));
    expect(resolved).toEqual({url: 'file:///app/a.js'});
    expect(calls[0][2]).not.toBe(context);
    expect(calls[0][2]).toEqual({
      conditions: ['node', 'react-server'],
      parentURL: 'file:///app/',
      importAttributes: {type: 'json'},
    });
    expect(context.conditions).toEqual(['node']);
    expect(warn).toHaveBeenCalledTimes(1);
    expect(warn.mock.calls[0][0]).toBe(
      'You did not run Node.js with the `--conditions react-server` flag. ' +
        'Any "react-server" override will only work with ESM imports.',
    );

    const serverContext = {conditions: ['react-server'], parentURL: undefined};
    await Loader.resolve('./c.js', serverContext, nextResolve(calls));
    expect(calls[2][2]).toBe(serverContext);
  });

  it('gives back what isn’t a module, and a module of neither directive, as it was loaded', async () => {
    const calls = [];
    const cjs = await Loader.load('file:///app/a.cjs', {}, nextLoad(calls));
    expect(cjs).toEqual({format: 'commonjs', source: "'use client'"});
    const plain = await Loader.load(
      'file:///app/plain.js',
      {},
      nextLoad(calls),
    );
    expect(plain).toEqual({
      format: 'module',
      source: files['file:///app/plain.js'],
    });
    const late = await Loader.load('file:///app/late.js', {}, nextLoad(calls));
    expect(late.source).toBe(files['file:///app/late.js']);
  });

  it('makes a client module of references to each of its exports, those of its export * too', async () => {
    const calls = [];
    await resolveFirst(calls);
    const {format, source} = await Loader.load(
      'file:///app/client.js',
      {},
      nextLoad(calls),
    );
    expect(format).toBe('module');
    const lines = source.split('\n');
    expect(lines[0]).toBe(
      'import {registerClientReference} from "react-server-dom-webpack/server";',
    );
    expect(lines.slice(1, -1).map(line => line.split(' = ')[0])).toEqual([
      'export const A',
      'export const b',
      'export const d',
      'export const e',
      'export const F',
      DEFAULT_EXPORT,
      'export const y',
      'export const O',
      'export const D',
      DEFAULT_EXPORT,
      'export const ns',
    ]);
    expect(lines[1]).toBe(
      'export const A = registerClientReference(function() {throw new Error("Attempted to call A() from the server but A is on the client. ' +
        "It's not possible to invoke a client function from the server, it can only be rendered as a Component or passed to props of a Client Component." +
        '");},"file:///app/client.js","A");',
    );
    expect(calls.map(call => [call[0], call[1], call[2]])).toEqual([
      ['resolve', './index.js', expect.anything()],
      ['load', 'file:///app/client.js', {}],
      [
        'resolve',
        './other.js',
        {conditions: ['node', 'import'], parentURL: 'file:///app/client.js'},
      ],
      [
        'load',
        'file:///app/other.js',
        {format: 'module', conditions: [], importAssertions: {}},
      ],
      [
        'resolve',
        './deep.js',
        {conditions: ['node', 'import'], parentURL: 'file:///app/other.js'},
      ],
      [
        'load',
        'file:///app/deep.js',
        {format: 'module', conditions: [], importAssertions: {}},
      ],
    ]);
  });

  it('throws on an export * before resolve was called', async () => {
    await expect(
      Loader.load('file:///app/client.js', {}, nextLoad([])),
    ).rejects.toThrow(
      'Expected resolve to have been called before transformSource',
    );
  });

  it('registers each export of a server module, mapping it to its line', async () => {
    const {source} = await Loader.load(
      'file:///app/server.js',
      {},
      nextLoad([]),
    );
    const original = files['file:///app/server.js'];
    expect(source.startsWith(original + '\n\n;')).toBe(true);
    const added = source.slice(original.length + 3).split('\n');
    expect(added.slice(0, -1)).toEqual([
      'import {registerServerReference} from "react-server-dom-webpack/server";',
      'registerServerReference(action,"file:///app/server.js","action");',
      'if (typeof value === "function") registerServerReference(value,"file:///app/server.js","value");',
      'if (typeof hidden === "function") registerServerReference(hidden,"file:///app/server.js","renamed");',
      'if (typeof hidden === "function") registerServerReference(hidden,"file:///app/server.js",undefined);',
      'if (typeof action === "function") registerServerReference(action,"file:///app/server.js","default");',
    ]);
    expect(inlineSourceMap(source)).toEqual({
      version: 3,
      sources: ['file:///app/server.js'],
      sourcesContent: [original],
      mappings:
        'AAAA;AACA;AACA;AACA;AACA;AACA;AACA;AACA;;;AALA;AACA;AAEA;AAAA;AACA',
      sourceRoot: '',
    });
  });

  it('maps a server module’s registrations through the source map it names', async () => {
    const calls = [];
    const {source} = await Loader.load(
      'file:///app/mapped.js',
      {},
      nextLoad(calls),
    );
    expect(calls[1]).toEqual([
      'load',
      'mapped.js.map',
      {
        format: 'json',
        conditions: [],
        importAssertions: {type: 'json'},
        importAttributes: {type: 'json'},
      },
    ]);
    expect(source.indexOf('//# sourceMappingURL=mapped.js.map')).toBe(-1);
    expect(inlineSourceMap(source)).toEqual({
      version: 3,
      sources: ['original.ts'],
      names: ['act', 'v'],
      mappings: 'AAAA;;AAEA,sBAAsBA;;AAEtB,MAAMC;;;;;AAFgBD;AAEhBC',
    });
  });

  it('refuses a module of both directives', async () => {
    await expect(
      Loader.load('file:///app/both.js', {}, nextLoad([])),
    ).rejects.toThrow(
      'Cannot have both "use client" and "use server" directives in the same file.',
    );
  });

  it('transforms a module getSource and transformSource give, as Node before 16.12 did', async () => {
    const calls = [];
    await resolveFirst(calls);
    const defaultGetSource = async (url, context) => {
      calls.push(['getSource', url, context]);
      return {source: files[url]};
    };
    const got = await Loader.getSource(
      'file:///app/client.js',
      {format: 'module'},
      defaultGetSource,
    );
    expect(got).toEqual({source: files['file:///app/client.js']});
    const defaultTransformSource = async (source, context) => {
      calls.push(['transformSource', context]);
      return {source};
    };
    const {source} = await Loader.transformSource(
      files['file:///app/client.js'],
      {format: 'module', url: 'file:///app/client.js'},
      defaultTransformSource,
    );
    expect(source.split('\n')[1].startsWith('export const A = ')).toBe(true);
    expect(calls.filter(call => call[0] !== 'resolve').slice(2)).toEqual([
      ['getSource', 'file:///app/other.js', {format: 'module'}],
      ['transformSource', {format: 'module', url: 'file:///app/other.js'}],
      ['getSource', 'file:///app/deep.js', {format: 'module'}],
      ['transformSource', {format: 'module', url: 'file:///app/deep.js'}],
    ]);
    const commonjs = {source: 'module.exports = 1;'};
    expect(
      await Loader.transformSource(
        commonjs.source,
        {format: 'commonjs', url: 'file:///app/a.cjs'},
        async () => commonjs,
      ),
    ).toBe(commonjs);
  });
});
