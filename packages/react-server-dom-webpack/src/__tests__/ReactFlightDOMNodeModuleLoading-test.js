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

import {patchSetImmediate} from '../../../../scripts/jest/patchSetImmediate';

// react-in-rust: the SSR client preinits a client reference's chunks only
// where the server consumer manifest has a moduleLoading. Ours is an
// `Option`, so a manifest without the key loads nothing, as a `None` does:
// the response holds `null` for it and prepareDestinationWithChunks tests
// `moduleLoading !== null`, as upstream's. No upstream test leaves it out.
let clientExports;
let webpackMap;
let webpackModules;
let webpackModuleLoading;
let React;
let ReactDOMServer;
let ReactServerDOMServer;
let ReactServerDOMClient;
let Stream;
let use;
let serverAct;

const streamOptions = {
  objectMode: true,
};

describe('ReactFlightDOMNode moduleLoading', () => {
  beforeEach(() => {
    jest.resetModules();

    patchSetImmediate();
    serverAct = require('internal-test-utils').serverAct;

    // Simulate the condition resolution
    jest.mock('react', () => require('react/react.react-server'));
    jest.mock('react-server-dom-webpack/server', () =>
      jest.requireActual('react-server-dom-webpack/server.node'),
    );
    ReactServerDOMServer = require('react-server-dom-webpack/server');

    const WebpackMock = require('./utils/WebpackMock');
    clientExports = WebpackMock.clientExports;
    webpackMap = WebpackMock.webpackMap;
    webpackModules = WebpackMock.webpackModules;
    webpackModuleLoading = WebpackMock.moduleLoading;

    jest.resetModules();
    __unmockReact();
    jest.unmock('react-server-dom-webpack/server');
    jest.mock('react-server-dom-webpack/client', () =>
      jest.requireActual('react-server-dom-webpack/client.node'),
    );

    React = require('react');
    ReactDOMServer = require('react-dom/server.node');
    ReactServerDOMClient = require('react-server-dom-webpack/client');
    Stream = require('stream');
    use = React.use;
  });

  function readResult(stream) {
    return new Promise((resolve, reject) => {
      let buffer = '';
      const writable = new Stream.PassThrough(streamOptions);
      writable.setEncoding('utf8');
      writable.on('data', chunk => {
        buffer += chunk;
      });
      writable.on('error', error => {
        reject(error);
      });
      writable.on('end', () => {
        resolve(buffer);
      });
      stream.pipe(writable);
    });
  }

  // Renders a client reference whose client chunk is path/to/chunk.js through
  // the SSR client with the given server consumer manifest's moduleLoading.
  async function renderWithModuleLoading(manifestRest) {
    function ClientComponent() {
      return <span>Client Component</span>;
    }
    const ClientComponentOnTheClient = clientExports(
      ClientComponent,
      123,
      'path/to/chunk.js',
    );
    const ClientComponentOnTheServer = clientExports(ClientComponent);

    const clientId = webpackMap[ClientComponentOnTheClient.$$id].id;
    delete webpackModules[clientId];
    const serverConsumerManifest = {
      moduleMap: {
        [clientId]: {'*': webpackMap[ClientComponentOnTheServer.$$id]},
      },
      ...manifestRest,
    };

    function App() {
      return <ClientComponentOnTheClient />;
    }

    const stream = await serverAct(() =>
      ReactServerDOMServer.renderToPipeableStream(<App />, webpackMap),
    );
    const readable = new Stream.PassThrough(streamOptions);
    let response;

    stream.pipe(readable);

    function ClientRoot() {
      if (response) return use(response);
      response = ReactServerDOMClient.createFromNodeStream(
        readable,
        serverConsumerManifest,
      );
      return use(response);
    }

    const ssrStream = await serverAct(() =>
      ReactDOMServer.renderToPipeableStream(<ClientRoot />),
    );
    return readResult(ssrStream);
  }

  it('preinits the chunks with a moduleLoading', async () => {
    const result = await renderWithModuleLoading({
      moduleLoading: webpackModuleLoading,
    });
    expect(result).toEqual(
      '<script src="/path/to/chunk.js" async=""></script><span>Client Component</span>',
    );
  });

  it('preinits nothing with a null moduleLoading', async () => {
    const result = await renderWithModuleLoading({moduleLoading: null});
    expect(result).toEqual('<span>Client Component</span>');
  });

  it('preinits nothing without a moduleLoading, as a None', async () => {
    const result = await renderWithModuleLoading({});
    expect(result).toEqual('<span>Client Component</span>');
  });
});
