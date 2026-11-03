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

// The ESM Flight DOM clients (Node, Browser) read what the ESM server
// writes. Upstream has no ESM tests; these hold the clients' behavior as the
// port writes them.

const {PassThrough, Readable} = require('stream');

let serverAct;
let ReactServerDOMServer;
let ReactServerDOMClient;

const model = {
  greeting: 'hello',
  list: [1, 2, 3],
  nested: {flag: true, nothing: null},
};

function setup(client) {
  jest.resetModules();
  serverAct = require('internal-test-utils').serverAct;
  jest.mock('react', () => require('react/react.react-server'));
  require('react');
  ReactServerDOMServer = require('react-server-dom-esm/server.node');
  jest.resetModules();
  __unmockReact();
  ReactServerDOMClient = require(client);
}

async function render(value) {
  const readable = new PassThrough();
  const {pipe} = await serverAct(() =>
    ReactServerDOMServer.renderToPipeableStream(value, '/root/'),
  );
  pipe(readable);
  return readable;
}

describe('ReactFlightESMDOMClientNode', () => {
  beforeEach(() => {
    setup('react-server-dom-esm/client.node');
  });

  it('reads a Node stream', async () => {
    const readable = await render(model);
    const result = await ReactServerDOMClient.createFromNodeStream(
      readable,
      '/root/',
      'http://localhost/',
    );
    expect(result).toEqual(model);
  });

  it('refuses to call a server reference', () => {
    const action = ReactServerDOMClient.createServerReference('id', null);
    expect(() => action()).toThrow(
      'Server Functions cannot be called during initial render.',
    );
  });
});

describe('ReactFlightESMDOMClientBrowser', () => {
  beforeEach(() => {
    setup('react-server-dom-esm/client.browser');
  });

  it('reads a stream', async () => {
    const readable = await render(model);
    const result = await ReactServerDOMClient.createFromReadableStream(
      Readable.toWeb(readable),
      {moduleBaseURL: 'http://localhost/'},
    );
    expect(result).toEqual(model);
  });

  it('reads a stream with no options', async () => {
    const readable = await render('text');
    const result = await ReactServerDOMClient.createFromReadableStream(
      Readable.toWeb(readable),
    );
    expect(result).toBe('text');
  });

  it('calls the server through callServer', async () => {
    const readable = await render('text');
    const calls = [];
    await ReactServerDOMClient.createFromReadableStream(
      Readable.toWeb(readable),
      {
        callServer(id, args) {
          calls.push([id, args]);
          return Promise.resolve('done');
        },
      },
    );
    const action = ReactServerDOMClient.createServerReference(
      'mod#fn',
      (id, args) => {
        calls.push([id, args]);
        return Promise.resolve('done');
      },
    );
    expect(await action(1)).toBe('done');
    expect(calls).toEqual([['mod#fn', [1]]]);
  });

  it('encodes a reply', async () => {
    expect(await ReactServerDOMClient.encodeReply({a: 1})).toBe('{"a":1}');
  });
});
