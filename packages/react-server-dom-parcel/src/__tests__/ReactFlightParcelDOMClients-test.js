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

// Parcel's Flight DOM clients (Edge, Node, Browser) read what Parcel's
// servers write, and give their server references Parcel's ids. Upstream has
// no Parcel tests; these hold the clients' behavior as the port writes them.

const {PassThrough} = require('stream');

let serverAct;
let ReactServerDOMServer;
let ReactServerDOMClient;

const model = {
  greeting: 'hello',
  list: [1, 2, 3],
  nested: {flag: true, nothing: null},
};

function setup(server, client) {
  jest.resetModules();
  global.parcelRequire = {meta: {publicUrl: '/', devServer: null}};
  serverAct = require('internal-test-utils').serverAct;
  jest.mock('react', () => require('react/react.react-server'));
  require('react');
  ReactServerDOMServer = require(server);
  jest.resetModules();
  __unmockReact();
  ReactServerDOMClient = require(client);
}

afterEach(() => {
  delete global.parcelRequire;
});

describe('ReactFlightParcelDOMClientEdge', () => {
  beforeEach(() => {
    setup(
      'react-server-dom-parcel/server.edge',
      'react-server-dom-parcel/client.edge',
    );
  });

  it('reads a stream', async () => {
    const stream = await serverAct(() =>
      ReactServerDOMServer.renderToReadableStream(model),
    );
    const result = await ReactServerDOMClient.createFromReadableStream(stream);
    expect(result).toEqual(model);
  });

  it('reads a stream with no options', async () => {
    const stream = await serverAct(() =>
      ReactServerDOMServer.renderToReadableStream('text'),
    );
    const result = await ReactServerDOMClient.createFromReadableStream(
      stream,
      undefined,
    );
    expect(result).toBe('text');
  });

  it('reads a fetch', async () => {
    const stream = await serverAct(() =>
      ReactServerDOMServer.renderToReadableStream(model),
    );
    const result = await ReactServerDOMClient.createFromFetch(
      Promise.resolve(new Response(stream)),
    );
    expect(result).toEqual(model);
  });

  it('encodes a reply', async () => {
    expect(await ReactServerDOMClient.encodeReply('hi')).toBe('"hi"');
    expect(await ReactServerDOMClient.encodeReply({a: 1})).toBe('{"a":1}');
  });

  it('names a server reference by its export and refuses to call it', () => {
    const action = ReactServerDOMClient.createServerReference('mod', 'doThing');
    if (__DEV__) {
      expect(action.name).toBe('doThing');
    }
    expect(() => action()).toThrow(
      'Server Functions cannot be called during initial render.',
    );
  });

  it('asks the dev server for source maps', () => {
    global.parcelRequire.meta.devServer = 'http://localhost:1234';
    const action = ReactServerDOMClient.createServerReference('mod', 'doThing');
    expect(typeof action).toBe('function');
  });
});

describe('ReactFlightParcelDOMClientNode', () => {
  beforeEach(() => {
    setup(
      'react-server-dom-parcel/server.node',
      'react-server-dom-parcel/client.node',
    );
  });

  it('reads a Node stream', async () => {
    const readable = new PassThrough();
    const {pipe} = await serverAct(() =>
      ReactServerDOMServer.renderToPipeableStream(model),
    );
    pipe(readable);
    const result = await ReactServerDOMClient.createFromNodeStream(readable);
    expect(result).toEqual(model);
  });

  it('has the Edge client too', () => {
    expect(typeof ReactServerDOMClient.createFromReadableStream).toBe(
      'function',
    );
    expect(typeof ReactServerDOMClient.createServerReference).toBe('function');
  });
});

describe('ReactFlightParcelDOMClientBrowser', () => {
  beforeEach(() => {
    setup(
      'react-server-dom-parcel/server.browser',
      'react-server-dom-parcel/client.browser',
    );
  });

  it('reads a stream', async () => {
    const stream = await serverAct(() =>
      ReactServerDOMServer.renderToReadableStream(model),
    );
    const result = await ReactServerDOMClient.createFromReadableStream(stream);
    expect(result).toEqual(model);
  });

  it('calls the registered server callback', async () => {
    const action = ReactServerDOMClient.createServerReference('mod', 'doThing');
    expect(() => action(1)).toThrow(
      'No server callback has been registered. Call setServerCallback to register one.',
    );
    const calls = [];
    ReactServerDOMClient.setServerCallback((id, args) => {
      calls.push([id, args]);
      return Promise.resolve('done');
    });
    expect(await action(1, 2)).toBe('done');
    expect(calls).toEqual([['mod#doThing', [1, 2]]]);
  });
});
