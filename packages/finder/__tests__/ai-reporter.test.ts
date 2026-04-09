import {describe, it, expect, vi, beforeEach, afterEach} from 'vitest';
import {AiReporter, compressCloneLine} from '../src/reporters/ai';
import {IClone, IOptions, IStatistic} from '@jscpd/core';

function makeClone(
  pathA: string, startA: number, endA: number,
  pathB: string, startB: number, endB: number
): IClone {
  return {
    format: 'typescript',
    duplicationA: {
      sourceId: pathA,
      start: {line: startA},
      end: {line: endA},
      range: [0, 0],
    },
    duplicationB: {
      sourceId: pathB,
      start: {line: startB},
      end: {line: endB},
      range: [0, 0],
    },
  };
}

const opts: IOptions = {absolute: true};

const statistic: IStatistic = {
  total: {
    lines: 100, tokens: 500, sources: 3,
    duplicatedLines: 10, duplicatedTokens: 50,
    clones: 2, percentage: 10.0, percentageTokens: 10.0,
    newDuplicatedLines: 0, newClones: 0,
  },
  detectionDate: '2026-04-09',
  formats: {},
};

describe('compressCloneLine', () => {
  it('same file: shows path once with ranges separated by space', () => {
    expect(compressCloneLine('src/utils/auth.ts', 'src/utils/auth.ts', '10-25', '80-95'))
      .toBe('src/utils/auth.ts 10-25 ~ 80-95');
  });

  it('same directory: shows dir prefix, both filenames with ranges', () => {
    expect(compressCloneLine('src/utils/auth.ts', 'src/utils/helpers.ts', '10-25', '40-55'))
      .toBe('src/utils/ auth.ts:10-25 ~ helpers.ts:40-55');
  });

  it('cross-directory with shared prefix: shows common prefix and relative paths', () => {
    expect(compressCloneLine('src/utils/auth.ts', 'src/api/routes.ts', '10-25', '5-20'))
      .toBe('src/ utils/auth.ts:10-25 ~ api/routes.ts:5-20');
  });

  it('no common prefix: shows full paths with colon-separated ranges', () => {
    expect(compressCloneLine('apps/a/foo.ts', 'packages/b/bar.ts', '1-10', '5-15'))
      .toBe('apps/a/foo.ts:1-10 ~ packages/b/bar.ts:5-15');
  });

  it('single-segment filenames with no common prefix: shows full paths', () => {
    expect(compressCloneLine('foo.ts', 'bar.ts', '1-5', '10-15'))
      .toBe('foo.ts:1-5 ~ bar.ts:10-15');
  });

  it('normalises Windows backslash paths', () => {
    expect(compressCloneLine('src\\utils\\auth.ts', 'src\\utils\\helpers.ts', '10-25', '40-55'))
      .toBe('src/utils/ auth.ts:10-25 ~ helpers.ts:40-55');
  });
});

describe('AiReporter', () => {
  let logs: string[];

  beforeEach(() => {
    logs = [];
    vi.spyOn(console, 'log').mockImplementation((...args) => {
      logs.push(args.join(' '));
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('same file: shows path once with both ranges separated by space', () => {
    const clone = makeClone(
      'src/utils/auth.ts', 10, 25,
      'src/utils/auth.ts', 80, 95
    );
    new AiReporter(opts).report([clone], statistic);
    expect(logs[0]).toBe('src/utils/auth.ts 10-25 ~ 80-95');
  });

  it('same directory: shows dir prefix once, both filenames with ranges', () => {
    const clone = makeClone(
      'src/utils/auth.ts', 10, 25,
      'src/utils/helpers.ts', 40, 55
    );
    new AiReporter(opts).report([clone], statistic);
    expect(logs[0]).toBe('src/utils/ auth.ts:10-25 ~ helpers.ts:40-55');
  });

  it('cross-directory with shared prefix: shows common prefix, both relative paths', () => {
    const clone = makeClone(
      'src/utils/auth.ts', 10, 25,
      'src/api/routes.ts', 5, 20
    );
    new AiReporter(opts).report([clone], statistic);
    expect(logs[0]).toBe('src/ utils/auth.ts:10-25 ~ api/routes.ts:5-20');
  });

  it('no common prefix: shows full paths', () => {
    const clone = makeClone(
      'apps/a/foo.ts', 1, 10,
      'packages/b/bar.ts', 5, 15
    );
    new AiReporter(opts).report([clone], statistic);
    expect(logs[0]).toBe('apps/a/foo.ts:1-10 ~ packages/b/bar.ts:5-15');
  });

  it('prints summary line after clones', () => {
    const clone = makeClone(
      'src/a.ts', 1, 5,
      'src/b.ts', 1, 5
    );
    new AiReporter(opts).report([clone], statistic);
    expect(logs[1]).toBe('---');
    expect(logs[2]).toBe('1 clones · 10.0% duplication');
  });

  it('uses clone count not statistic count in summary', () => {
    const clones = [
      makeClone('src/a.ts', 1, 5, 'src/b.ts', 1, 5),
      makeClone('src/c.ts', 1, 5, 'src/d.ts', 1, 5),
      makeClone('src/e.ts', 1, 5, 'src/f.ts', 1, 5),
    ];
    new AiReporter(opts).report(clones, statistic);
    const separatorIdx = logs.indexOf('---');
    expect(separatorIdx).toBeGreaterThan(-1);
    expect(logs[separatorIdx + 1]).toBe('3 clones · 10.0% duplication');
  });

  it('omits summary when statistic is undefined', () => {
    const clone = makeClone('src/a.ts', 1, 5, 'src/b.ts', 1, 5);
    new AiReporter(opts).report([clone], undefined);
    expect(logs).toHaveLength(1);
    expect(logs[0]).not.toContain('---');
  });

  it('produces no output when options.silent is true', () => {
    const clone = makeClone('src/a.ts', 1, 5, 'src/b.ts', 1, 5);
    new AiReporter({...opts, silent: true}).report([clone], statistic);
    expect(logs).toHaveLength(0);
  });

  it('prints summary with zero clones when clone list is empty', () => {
    new AiReporter(opts).report([], statistic);
    expect(logs[0]).toBe('---');
    expect(logs[1]).toBe('0 clones · 10.0% duplication');
  });
});
