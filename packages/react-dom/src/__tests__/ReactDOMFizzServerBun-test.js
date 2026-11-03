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

// The port's own test: no upstream test runs react-dom/server.bun's
// renderToReadableStream. Bun's direct ReadableStream, `{type: 'direct',
// pull(controller)}`, is faked here: the stream keeps the source and the
// strategy it's given, and a read calls `pull` with a controller that
// collects what's written until `end()`.
let React;
let ReactDOMFizzServer;
let Suspense;

class DirectReadableStream {
  constructor(source, strategy) {
    this.source = source;
    this.strategy = strategy;
  }
}

describe('ReactDOMFizzServerBun', () => {
  let originalReadableStream;

  beforeEach(() => {
    jest.resetModules();
    jest.useRealTimers();
    originalReadableStream = global.ReadableStream;
    global.ReadableStream = DirectReadableStream;
    global.Bun = {hash: jest.fn(() => 7n)};
    React = require('react');
    Suspense = React.Suspense;
    ReactDOMFizzServer = require('react-dom/server.bun');
  });

  afterEach(() => {
    global.ReadableStream = originalReadableStream;
    delete global.Bun;
  });

  function read(stream) {
    const log = [];
    let result = '';
    const done = new Promise(resolve => {
      stream.source.pull({
        write(chunk) {
          log.push('write');
          result += chunk;
          return chunk.length;
        },
        flush() {
          log.push('flush');
        },
        end() {
          log.push('end');
          resolve();
        },
      });
    });
    return done.then(() => ({result, log}));
  }

  it('streams the shell to a direct stream of a 2048 high water mark', async () => {
    const stream = await ReactDOMFizzServer.renderToReadableStream(
      <div>hello world</div>,
    );
    expect(stream).toBeInstanceOf(DirectReadableStream);
    expect(stream.source.type).toBe('direct');
    expect(typeof stream.source.pull).toBe('function');
    expect(typeof stream.source.cancel).toBe('function');
    expect(stream.strategy).toEqual({highWaterMark: 2048});
    const {result, log} = await read(stream);
    expect(result).toMatchInlineSnapshot(`"<div>hello world</div>"`);
    expect(log.slice(-2)).toEqual(['flush', 'end']);
    expect(log.slice(0, -2).every(entry => entry === 'write')).toBe(true);
    await stream.allReady;
  });

  it('streams a boundary once it resolves, and allReady after', async () => {
    let resolve;
    const promise = new Promise(r => (resolve = r));
    function Wait() {
      React.use(promise);
      return 'done';
    }
    let allReady = false;
    const stream = await ReactDOMFizzServer.renderToReadableStream(
      <div>
        <Suspense fallback="loading">
          <Wait />
        </Suspense>
      </div>,
    );
    stream.allReady.then(() => (allReady = true));
    const reading = read(stream);
    await new Promise(r => setTimeout(r, 0));
    expect(allReady).toBe(false);
    resolve();
    const {result} = await reading;
    expect(allReady).toBe(true);
    expect(result).toContain('loading');
    expect(result).toContain('done');
  });

  it('rejects with a shell error, and reports it', async () => {
    const errors = [];
    function Throw() {
      throw new Error('shell');
    }
    let caught;
    try {
      await ReactDOMFizzServer.renderToReadableStream(<Throw />, {
        onError(error) {
          errors.push(error.message);
        },
      });
    } catch (error) {
      caught = error;
    }
    expect(caught.message).toBe('shell');
    expect(errors).toEqual(['shell']);
  });

  it('aborts what is pending when the stream is cancelled', async () => {
    const errors = [];
    function Never() {
      React.use(new Promise(() => {}));
    }
    const stream = await ReactDOMFizzServer.renderToReadableStream(
      <div>
        <Suspense fallback="loading">
          <Never />
        </Suspense>
      </div>,
      {
        onError(error) {
          errors.push(error);
        },
      },
    );
    const reason = new Error('cancelled');
    stream.source.cancel(reason);
    await stream.allReady;
    expect(errors).toEqual([reason]);
  });

  it('aborts by its signal', async () => {
    const errors = [];
    const controller = new AbortController();
    function Never() {
      React.use(new Promise(() => {}));
    }
    const stream = await ReactDOMFizzServer.renderToReadableStream(
      <div>
        <Suspense fallback="loading">
          <Never />
        </Suspense>
      </div>,
      {
        signal: controller.signal,
        onError(error) {
          errors.push(error);
        },
      },
    );
    const reading = read(stream);
    const reason = new Error('aborted');
    controller.abort(reason);
    const {result} = await reading;
    expect(errors).toEqual([reason]);
    expect(result).toContain('loading');
  });

  it('gives onHeaders its headers as Headers', async () => {
    let headers;
    await ReactDOMFizzServer.renderToReadableStream(<div>hello</div>, {
      onHeaders(h) {
        headers = h;
      },
    });
    expect(headers).toBeInstanceOf(Headers);
  });

  it('hashes a form state key path with Bun.hash', async () => {
    async function action(state) {
      return state;
    }
    // A server action, as a Flight client gives one.
    action.$$FORM_ACTION = () => ({});
    action.$$IS_SIGNATURE_EQUAL = () => true;
    function App() {
      const [state] = React.useActionState(action, 0);
      return <span>{state}</span>;
    }
    const stream = await ReactDOMFizzServer.renderToReadableStream(<App />, {
      formState: [1, 'k7', 'id', 0],
    });
    const {result} = await read(stream);
    // The key, 'k' + the hash's bigint, matches the form state's, so its
    // state is the form state's.
    expect(result).toBe('<!--F!--><span>1</span>');
    expect(global.Bun.hash).toHaveBeenCalledTimes(1);
    expect(typeof global.Bun.hash.mock.calls[0][0]).toBe('string');
  });
});
