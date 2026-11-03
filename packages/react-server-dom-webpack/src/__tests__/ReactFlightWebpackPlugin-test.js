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

// The port's own: upstream has no test of the plugin. It builds a small
// app with webpack and reads the manifests the plugin writes.

const fs = require('fs');
const os = require('os');
const path = require('path');
const {pathToFileURL} = require('url');

let webpack;
let ReactFlightWebpackPlugin;

const clientFileName = require.resolve('../../client.browser.js');

function makeApp(files) {
  const dir = fs.realpathSync(
    fs.mkdtempSync(path.join(os.tmpdir(), 'flight-plugin-')),
  );
  Object.keys(files).forEach(name => {
    const file = path.join(dir, name);
    fs.mkdirSync(path.dirname(file), {recursive: true});
    fs.writeFileSync(file, files[name]);
  });
  return dir;
}

// The Flight client's own source needs React's build to bundle, so a loader
// stands in for its code: the plugin only asks that its module is parsed.
const stubLoader = `module.exports = function () { return 'export default 1;'; };`;

function build(dir, {plugin, mode = 'development', output = {}, entry}) {
  return new Promise((resolve, reject) => {
    webpack(
      {
        mode,
        context: dir,
        devtool: false,
        entry: entry || './entry.js',
        output: {
          path: path.join(dir, 'out'),
          publicPath: '/assets/',
          ...output,
        },
        module: {
          rules: [
            {test: clientFileName, use: path.join(dir, 'stub-loader.cjs')},
          ],
        },
        plugins: [plugin],
        optimization: {minimize: false},
      },
      (err, stats) => {
        if (err) {
          reject(err);
          return;
        }
        resolve(stats);
      },
    );
  });
}

function readJSON(dir, name) {
  return JSON.parse(fs.readFileSync(path.join(dir, 'out', name), 'utf8'));
}

const app = {
  'stub-loader.cjs': stubLoader,
  // The client's package is free of side effects: a production build keeps
  // its module only where its export is used.
  'entry.js': `import client from ${JSON.stringify(clientFileName)};\nimport './Server.js';\nconsole.log(client);\n`,
  'Client.js': `'use client';\nexport default function Client() { return 'client'; }\n`,
  'nested/Button.js': `"use client";\nexport function Button() { return 'button'; }\n`,
  'Server.js': `export default function Server() { return 'server'; }\n`,
  'Later.js': `export const x = 1;\n'use client';\n`,
  'Mentioned.js': `// use client\nexport default 1;\n`,
};

describe('ReactFlightWebpackPlugin', () => {
  beforeEach(() => {
    jest.resetModules();
    webpack = require('webpack');
    ReactFlightWebpackPlugin =
      require('react-server-dom-webpack/plugin').default;
  });

  it('requires isServer as a boolean', () => {
    expect(() => new ReactFlightWebpackPlugin()).toThrow(
      'React Server Plugin: You must specify the isServer option as a boolean.',
    );
    expect(() => new ReactFlightWebpackPlugin({})).toThrow(
      'React Server Plugin: You must specify the isServer option as a boolean.',
    );
    expect(() => new ReactFlightWebpackPlugin({isServer: 'no'})).toThrow(
      'React Server Plugin: You must specify the isServer option as a boolean.',
    );
    expect(() => new ReactFlightWebpackPlugin({isServer: true})).toThrow(
      'TODO: Implement the server compiler.',
    );
  });

  it('takes its options', () => {
    const defaults = new ReactFlightWebpackPlugin({isServer: false});
    expect(defaults.chunkName).toBe('client[index]');
    expect(defaults.clientManifestFilename).toBe('react-client-manifest.json');
    expect(defaults.serverConsumerManifestFilename).toBe(
      'react-ssr-manifest.json',
    );
    expect(defaults.clientReferences).toEqual([
      {directory: '.', recursive: true, include: /\.(js|ts|jsx|tsx)$/},
    ]);

    const one = new ReactFlightWebpackPlugin({
      isServer: false,
      clientReferences: './Client.js',
      chunkName: 'rsc-',
      clientManifestFilename: 'client.json',
      serverConsumerManifestFilename: 'ssr.json',
    });
    expect(one.clientReferences).toEqual(['./Client.js']);
    expect(one.chunkName).toBe('rsc-[index]');
    expect(one.clientManifestFilename).toBe('client.json');
    expect(one.serverConsumerManifestFilename).toBe('ssr.json');

    const search = {directory: './src', include: /\.js$/};
    expect(
      new ReactFlightWebpackPlugin({
        isServer: false,
        clientReferences: search,
        chunkName: 'rsc-[request]',
      }),
    ).toMatchObject({clientReferences: [search], chunkName: 'rsc-[request]'});
    const many = ['./a.js', search];
    expect(
      new ReactFlightWebpackPlugin({
        isServer: false,
        clientReferences: many,
        clientManifestFilename: '',
      }),
    ).toMatchObject({
      clientReferences: many,
      clientManifestFilename: 'react-client-manifest.json',
    });
  });

  it('writes the manifests of the client references it finds', async () => {
    const dir = makeApp(app);
    const plugin = new ReactFlightWebpackPlugin({
      isServer: false,
      clientReferences: {
        directory: '.',
        recursive: true,
        include: /\.js$/,
        exclude: /out\//,
      },
    });
    const stats = await build(dir, {
      plugin,
      output: {crossOriginLoading: 'use-credentials'},
    });
    expect(stats.compilation.errors).toEqual([]);
    expect(stats.compilation.warnings).toEqual([]);
    // Each is a client reference's chunk, of the dependency's type.
    const {modules} = stats.toJson({modules: true, reasons: true});
    const reasons = modules.find(m => m.name === './Client.js').reasons;
    expect(reasons.map(r => r.type)).toEqual(['client-reference']);

    const clientManifest = readJSON(dir, 'react-client-manifest.json');
    const client = pathToFileURL(path.join(dir, 'Client.js')).href;
    const button = pathToFileURL(path.join(dir, 'nested/Button.js')).href;
    expect(Object.keys(clientManifest).sort()).toEqual([button, client].sort());
    expect(clientManifest[client]).toEqual({
      id: './Client.js',
      chunks: [expect.any(String), expect.stringMatching(/\.js$/)],
      name: '*',
    });
    expect(clientManifest[button].id).toBe('./nested/Button.js');
    const chunkFiles = Object.values(clientManifest).map(e => e.chunks[1]);
    expect(chunkFiles.sort()).toEqual(['client0.js', 'client1.js']);

    const ssrManifest = readJSON(dir, 'react-ssr-manifest.json');
    expect(ssrManifest).toEqual({
      moduleLoading: {prefix: '/assets/', crossOrigin: 'use-credentials'},
      moduleMap: {
        './Client.js': {'*': {specifier: client, name: '*'}},
        './nested/Button.js': {'*': {specifier: button, name: '*'}},
      },
    });
    // Its text, as the plugin indents it.
    expect(
      fs.readFileSync(path.join(dir, 'out', 'react-ssr-manifest.json'), 'utf8'),
    ).toBe(JSON.stringify(ssrManifest, null, 2));
  });

  it('keeps the numbers of a production build', async () => {
    const dir = makeApp(app);
    const plugin = new ReactFlightWebpackPlugin({
      isServer: false,
      clientReferences: [
        './Client.js',
        {directory: './nested', include: /\.js$/, recursive: false},
      ],
      chunkName: 'rsc-[request]-',
    });
    const stats = await build(dir, {
      plugin,
      mode: 'production',
      output: {crossOriginLoading: 'anonymous', chunkFilename: '[name].js'},
    });
    expect(stats.compilation.errors).toEqual([]);

    const clientManifest = readJSON(dir, 'react-client-manifest.json');
    const button = pathToFileURL(path.join(dir, 'nested/Button.js')).href;
    // A string reference isn't read for its directive, but resolved.
    expect(Object.keys(clientManifest)).toContain(button);
    Object.values(clientManifest).forEach(entry => {
      expect(typeof entry.id).toBe('number');
      expect(typeof entry.chunks[0]).toBe('number');
    });
    const files = Object.values(clientManifest).map(e => e.chunks[1]);
    expect(files).toContain('rsc-Button-js-.js');

    const ssrManifest = readJSON(dir, 'react-ssr-manifest.json');
    expect(ssrManifest.moduleLoading).toEqual({
      prefix: '/assets/',
      crossOrigin: 'anonymous',
    });
    Object.keys(ssrManifest.moduleMap).forEach(id => {
      expect(id).toMatch(/^\d+$/);
    });
  });

  it('has no crossOrigin of crossOriginLoading off, nor a prefix of no publicPath', async () => {
    const dir = makeApp(app);
    const plugin = new ReactFlightWebpackPlugin({
      isServer: false,
      clientReferences: {directory: '.', include: /Client\.js$/},
      clientManifestFilename: 'client.json',
      serverConsumerManifestFilename: 'ssr.json',
    });
    await build(dir, {plugin, output: {publicPath: ''}});
    const ssrManifest = readJSON(dir, 'ssr.json');
    expect(ssrManifest.moduleLoading).toEqual({prefix: '', crossOrigin: null});
    expect(Object.keys(readJSON(dir, 'client.json'))).toEqual([
      pathToFileURL(path.join(dir, 'Client.js')).href,
    ]);
  });

  it('warns where the client runtime is never imported', async () => {
    const dir = makeApp({...app, 'entry.js': `import './Server.js';\n`});
    const plugin = new ReactFlightWebpackPlugin({isServer: false});
    const stats = await build(dir, {plugin});
    expect(stats.compilation.warnings.map(w => w.message)).toEqual([
      'Client runtime at react-server-dom-webpack/client was not found. ' +
        'React Server Components module map file react-client-manifest.json was not created.',
    ]);
    expect(
      fs.existsSync(path.join(dir, 'out', 'react-client-manifest.json')),
    ).toBe(false);
  });

  it('gives what fails resolving a directory to webpack', async () => {
    const dir = makeApp(app);
    const plugin = new ReactFlightWebpackPlugin({
      isServer: false,
      clientReferences: {directory: './missing', include: /\.js$/},
    });
    await expect(build(dir, {plugin})).rejects.toThrow(/missing/);
  });
});
