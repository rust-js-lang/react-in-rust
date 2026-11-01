/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @emails react-core
 */

'use strict';

// react-in-rust: upstream's mapChildren gives back null or undefined
// children as it was given (`if (children == null) return children;`),
// which no upstream test checks.
describe('ReactChildren given null or undefined', () => {
  let React;

  beforeEach(() => {
    jest.resetModules();
    React = require('react');
  });

  it('map gives back undefined children as undefined', () => {
    const callback = jest.fn();
    expect(React.Children.map(undefined, callback)).toBe(undefined);
    expect(callback).not.toHaveBeenCalled();
  });

  it('map gives back null children as null', () => {
    const callback = jest.fn();
    expect(React.Children.map(null, callback)).toBe(null);
    expect(callback).not.toHaveBeenCalled();
  });

  it('forEach, count and toArray take null and undefined', () => {
    const callback = jest.fn();
    React.Children.forEach(undefined, callback);
    React.Children.forEach(null, callback);
    expect(callback).not.toHaveBeenCalled();
    expect(React.Children.count(undefined)).toBe(0);
    expect(React.Children.count(null)).toBe(0);
    expect(React.Children.toArray(undefined)).toEqual([]);
    expect(React.Children.toArray(null)).toEqual([]);
  });
});
