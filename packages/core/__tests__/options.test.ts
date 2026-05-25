import { describe, it, expect } from 'vitest';
import { getDefaultOptions, getOption } from '../src/options';

describe('getDefaultOptions', () => {
  it('returns an object with expected default values', () => {
    const opts = getDefaultOptions();
    expect(opts.minLines).toBe(5);
    expect(opts.maxLines).toBe(1000);
    expect(opts.minTokens).toBe(50);
    expect(opts.maxSize).toBe('100kb');
    expect(opts.output).toBe('./report');
    expect(opts.reporters).toEqual(['console']);
    expect(opts.ignore).toEqual([]);
    expect(opts.debug).toBe(false);
    expect(opts.silent).toBe(false);
    expect(opts.blame).toBe(false);
    expect(opts.cache).toBe(true);
    expect(opts.absolute).toBe(false);
    expect(opts.noSymlinks).toBe(false);
    expect(opts.skipLocal).toBe(false);
    expect(opts.ignoreCase).toBe(false);
    expect(opts.gitignore).toBe(true);
    expect(opts.exitCode).toBe(0);
    expect(opts.noTips).toBe(!!process.env['CI']);
  });

  it('path defaults to [cwd]', () => {
    const opts = getDefaultOptions();
    expect(opts.path).toEqual([process.cwd()]);
  });

  it('threshold is undefined by default', () => {
    const opts = getDefaultOptions();
    expect(opts.threshold).toBeUndefined();
  });

  it('returns a fresh object on each call', () => {
    const a = getDefaultOptions();
    const b = getDefaultOptions();
    expect(a).not.toBe(b);
  });
});

describe('getOption', () => {
  it('returns the value from the provided options when set', () => {
    const opts = getDefaultOptions();
    opts.minLines = 10;
    expect(getOption('minLines', opts)).toBe(10);
  });

  it('falls back to the default when the option is missing from the provided object', () => {
    expect(getOption('minLines', {} as any)).toBe(5);
  });

  it('returns the default when no options object is passed', () => {
    expect(getOption('minTokens')).toBe(50);
  });

  it('returns the default reporters when not set', () => {
    expect(getOption('reporters', {} as any)).toEqual(['console']);
  });
});
