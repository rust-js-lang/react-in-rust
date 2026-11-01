/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @emails react-core
 * @jest-environment ./scripts/jest/ReactDOMServerIntegrationEnvironment
 */

'use strict';

import {patchMessageChannel} from '../../../../scripts/jest/patchMessageChannel';

// The port's own test: a postponed state holds `null` where a replay node or
// the root has no resume slots, as upstream's does, so its JSON is upstream's.
// The port's first draft wrote the string 'null' (resumed by itself alike).

global.ReadableStream =
  require('web-streams-polyfill/ponyfill/es6').ReadableStream;
global.TextEncoder = require('util').TextEncoder;
global.TextDecoder = require('util').TextDecoder;

let React;
let ReactDOMFizzStatic;
let Suspense;
let serverAct;

describe('ReactFizzPostponedState', () => {
  beforeEach(() => {
    jest.resetModules();
    patchMessageChannel();
    serverAct = require('internal-test-utils').serverAct;
    React = require('react');
    ReactDOMFizzStatic = require('react-dom/static.browser');
    Suspense = React.Suspense;
  });

  it('holds null where a node has no resume slots', async () => {
    const neverResolves = new Promise(() => {});
    function Pending() {
      React.use(neverResolves);
      return 'never';
    }
    function App() {
      return (
        <div>
          <span>
            <Suspense fallback="Loading">
              <Pending />
            </Suspense>
          </span>
        </div>
      );
    }

    const controller = new AbortController();
    let pendingResult;
    await serverAct(async () => {
      pendingResult = ReactDOMFizzStatic.prerender(<App />, {
        signal: controller.signal,
        onError() {},
      });
    });
    await serverAct(() => {
      controller.abort();
    });
    const prerendered = await pendingResult;
    const postponed = prerendered.postponed;

    expect(postponed).not.toBe(null);
    // Only the boundary inside is postponed: the root itself resumes nothing.
    expect(postponed.replaySlots).toBe(null);
    const app = postponed.replayNodes[0];
    expect(app[0]).toBe('App');
    expect(app[3]).toBe(null);
    const div = app[2][0];
    expect(div[0]).toBe('div');
    expect(div[3]).toBe(null);
    const span = div[2][0];
    expect(span[0]).toBe('span');
    expect(span[3]).toBe(null);
    expect(JSON.stringify(postponed)).not.toContain('"null"');
  });
});
