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

// The port's own test: the server tells `null` from `undefined` where
// upstream does. An element without an `_owner` (its `_debugStack` `null`, so
// no other read throws first) passes renderClientElement's `debugOwner !==
// null` and throws where outlineComponentInfo reads its `owner`; a forwarded I/O info's `null` value is sent, an `undefined` one is
// left out (emitIOInfoChunk's `value !== undefined`).
let ReactServer;
let ReactNoopFlightServer;
let assertConsoleErrorDev;

describe('ReactFlightGivenDebugValues', () => {
  beforeEach(() => {
    jest.resetModules();
    jest.mock('react', () => require('react/react.react-server'));
    ReactServer = require('react');
    ReactNoopFlightServer = require('react-noop-renderer/flight-server');
    assertConsoleErrorDev =
      require('internal-test-utils').assertConsoleErrorDev;
  });

  it('throws reading the owner of an element without one', () => {
    const element = ReactServer.createElement('span', {children: 'Free!'});
    // bad clone, without an owner, its stack and task null
    const clone = {...element, _debugStack: null, _debugTask: null};
    delete clone._owner;
    ReactNoopFlightServer.render(clone);

    assertConsoleErrorDev([
      'Attempted to render <span> without development properties. This is not supported. It can happen if:' +
        '\n- The element is created with a production version of React but rendered in development.' +
        '\n- The element was cloned with a custom function instead of `React.cloneElement`.\n' +
        "The props of this element may help locate this element: { children: 'Free!' }",
      "TypeError: Cannot read properties of undefined (reading 'owner')" +
        '\n    in <stack>',
    ]);
  });

  function renderAwaiting(awaited) {
    const thenable = {
      status: 'fulfilled',
      value: 'hi',
      then() {},
      _debugInfo: [{awaited, env: 'Server'}],
    };
    const chunks = ReactNoopFlightServer.render(thenable);
    const decoder = new TextDecoder();
    return chunks.map(chunk => decoder.decode(chunk)).join('');
  }

  // @gate __DEV__
  it('sends a forwarded I/O info value of null', () => {
    const text = renderAwaiting({
      name: 'io',
      start: 0,
      end: Number.MAX_SAFE_INTEGER,
      value: null,
    });
    expect(text).toContain('"name":"io"');
    expect(text).toContain('"value":null');
  });

  // @gate __DEV__
  it('leaves out a forwarded I/O info value of undefined', () => {
    const text = renderAwaiting({
      name: 'io',
      start: 0,
      end: Number.MAX_SAFE_INTEGER,
      value: undefined,
    });
    expect(text).toContain('"name":"io"');
    expect(text).not.toContain('"value"');
  });
});
