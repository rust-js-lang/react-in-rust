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

// The port's own test: a symbol of no description, logged by a Server
// Component, is sent in the replayed log as `"$Sundefined"`, upstream's
// `'$S' + symbol.description` of `undefined`, which the client reads back as
// `Symbol.for('undefined')`.
describe('ReactFlightSymbolDescription', () => {
  afterEach(() => {
    jest.restoreAllMocks();
  });

  // @gate __DEV__
  it('sends a logged symbol of no description as $Sundefined', () => {
    jest.resetModules();
    jest.mock('react', () => require('react/react.react-server'));
    // The server patches console.log as it loads, so the mock goes first.
    const mockConsoleLog = jest
      .spyOn(console, 'log')
      .mockImplementation(() => {});
    const ReactServer = require('react');
    const ReactNoopFlightServer = require('react-noop-renderer/flight-server');

    function App() {
      console.log(Symbol());
      return null;
    }

    const chunks = ReactNoopFlightServer.render(
      ReactServer.createElement(App),
    );
    const decoder = new TextDecoder();
    const text = chunks.map(chunk => decoder.decode(chunk)).join('');

    expect(mockConsoleLog).toHaveBeenCalledTimes(1);
    expect(text).toContain('"$Sundefined"');
  });
});
