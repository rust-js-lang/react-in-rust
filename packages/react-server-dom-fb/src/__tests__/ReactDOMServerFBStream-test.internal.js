/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @emails react-core
 */

'use strict';

// The port's own: what ReactDOMServerFB-test.internal.js leaves untested,
// each chunk holding only what was written since the last, and `debug`.

let React;
let ReactDOMServer;
let Suspense;

describe('ReactDOMServerFBStream', () => {
  beforeEach(() => {
    jest.resetModules();
    React = require('react');
    ReactDOMServer = require('../ReactDOMServerFB');
    Suspense = React.Suspense;
  });

  function render() {
    let hasLoaded = false;
    let resolve;
    const promise = new Promise(r => (resolve = r));
    function Wait() {
      if (!hasLoaded) {
        throw promise;
      }
      return 'Done';
    }
    const stream = ReactDOMServer.renderToStream(
      <div>
        <Suspense fallback="Loading">
          <Wait />
        </Suspense>
      </div>,
      {
        onError(x) {
          console.error(x);
        },
      },
    );
    return {
      stream,
      load() {
        hasLoaded = true;
        resolve();
      },
    };
  }

  it('gives each chunk only what was written since the last', async () => {
    const {stream, load} = render();
    const first = ReactDOMServer.renderNextChunk(stream);
    expect(first).toContain('Loading');
    expect(ReactDOMServer.hasFinished(stream)).toBe(false);
    expect(ReactDOMServer.renderNextChunk(stream)).toBe('');

    load();
    await null;

    const second = ReactDOMServer.renderNextChunk(stream);
    expect(second).toContain('Done');
    expect(second).not.toContain('Loading');
    expect(ReactDOMServer.hasFinished(stream)).toBe(true);
  });

  it('tells what the request has left to do', async () => {
    const {stream, load} = render();
    expect(ReactDOMServer.debug(stream)).toEqual({
      pendingRootTasks: 1,
      clientRenderedBoundaries: 0,
      completedBoundaries: 0,
      partialBoundaries: 0,
      allPendingTasks: 1,
      pingedTasks: 1,
    });

    ReactDOMServer.renderNextChunk(stream);
    expect(ReactDOMServer.debug(stream)).toEqual({
      pendingRootTasks: 0,
      clientRenderedBoundaries: 0,
      completedBoundaries: 0,
      partialBoundaries: 0,
      allPendingTasks: 1,
      pingedTasks: 0,
    });

    load();
    await null;
    expect(ReactDOMServer.debug(stream)).toEqual({
      pendingRootTasks: 0,
      clientRenderedBoundaries: 0,
      completedBoundaries: 0,
      partialBoundaries: 0,
      allPendingTasks: 1,
      pingedTasks: 1,
    });
  });
});
