import { default as test, ExecutionContext } from 'ava';
import { stub } from 'sinon';
import { IMode, IReporter } from '../..';

const proxyquire = require('proxyquire').noCallThru();

const useDependencies = {
  'jscpd-test-mode': {
    default: {} as IMode
  },
  'jscpd-test-reporter': {
    default: {} as IReporter
  },
  'detect-installed': {
    sync: stub().returns(true)
  }
};

const { useMode, useReporter } = proxyquire('../use', useDependencies);

test('should import module with mode', (t: ExecutionContext) => {
  t.is(useMode('test'), useDependencies['jscpd-test-mode'].default);
});

test('should import module with reporter', (t: ExecutionContext) => {
  t.is(useReporter('test'), useDependencies['jscpd-test-reporter'].default);
});

test('should throw error if module with mode does not exists', (t: ExecutionContext) => {
  useDependencies['detect-installed'].sync.returns(false);
  t.throws(() => {
    useMode('test1');
  });
});

test('should throw error if module with reporter does not exists', (t: ExecutionContext) => {
  useDependencies['detect-installed'].sync.returns(false);
  t.throws(() => {
    useReporter('test1');
  });
});
