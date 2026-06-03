/**
 * Unit tests for apps/jscpd/src/options.ts
 *
 * Tests readPackageJsonConfig() and resolveIgnorePattern() indirectly via
 * prepareOptions(), with fs and fs-extra fully mocked so no real filesystem
 * access occurs.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { resolve, join } from 'path';
import { tmpdir } from 'os';

vi.mock('fs', () => ({
  default: { existsSync: vi.fn() },
  existsSync: vi.fn(),
}));

vi.mock('fs-extra', () => ({
  default: { readJSONSync: vi.fn() },
  readJSONSync: vi.fn(),
}));

import { existsSync } from 'fs';
import { readJSONSync } from 'fs-extra';
import { prepareOptions } from '../src/options';

/**
 * Minimal Commander-shaped object that satisfies prepareOptions().
 *
 * prepareOptions() calls cli.opts() to read option values (Commander v8+
 * style — direct property access was removed in v8).  The returned object
 * therefore needs an opts() method that returns the option bag, plus the
 * args array for positional arguments.
 *
 * All CLI flags default to undefined so they are stripped by
 * convertCliToOptions — only fs-based config sources shape the result.
 */
const makeCmd = (overrides: Record<string, any> = {}) => {
  const options = {
    config: undefined,
    path: undefined,
    minTokens: undefined,
    minLines: undefined,
    maxLines: undefined,
    maxSize: undefined,
    debug: undefined,
    store: undefined,
    pattern: undefined,
    executionId: undefined,
    silent: undefined,
    blame: undefined,
    verbose: undefined,
    cache: undefined,
    output: undefined,
    format: undefined,
    formatsExts: undefined,
    formatsNames: undefined,
    list: undefined,
    mode: undefined,
    absolute: undefined,
    noSymlinks: undefined,
    skipLocal: undefined,
    ignoreCase: undefined,
    gitignore: undefined,
    exitCode: undefined,
    threshold: undefined,
    reporters: undefined,
    ignore: undefined,
    ignorePattern: undefined,
    skipComments: undefined,
    noTips: undefined,
    ...overrides,
  };
  return {
    opts: () => options,
    args: [],
  } as any;
};

// Stable fake cwd used for all tests — avoids touching the real filesystem.
const fakeCwd = join(tmpdir(), 'jscpd-options-unit-test');
const fakeConfigFile = join(fakeCwd, '.jscpd.json');
const fakePackageJson = join(fakeCwd, 'package.json');

// ─── readPackageJsonConfig ────────────────────────────────────────────────────

describe('readPackageJsonConfig (via prepareOptions)', () => {
  beforeEach(() => {
    // Default: no files exist and readJSONSync returns nothing
    vi.mocked(existsSync).mockReturnValue(false);
    vi.mocked(readJSONSync).mockReturnValue({});
    vi.spyOn(process, 'cwd').mockReturnValue(fakeCwd);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('returns empty config when package.json does not exist', () => {
    vi.mocked(existsSync).mockReturnValue(false);
    const opts = prepareOptions(makeCmd());
    // readPackageJsonConfig returns {} → no `config` key injected
    expect(opts).not.toHaveProperty('config');
  });

  it('returns empty config when package.json contains no jscpd key', () => {
    vi.mocked(existsSync).mockImplementation((p) => String(p) === fakePackageJson);
    vi.mocked(readJSONSync).mockReturnValue({ name: 'my-project', version: '1.0.0' });
    const opts = prepareOptions(makeCmd());
    // json.jscpd is falsy → readPackageJsonConfig returns {}
    expect(opts).not.toHaveProperty('config');
  });

  it('merges scalar jscpd options from package.json into result', () => {
    vi.mocked(existsSync).mockImplementation((p) => String(p) === fakePackageJson);
    vi.mocked(readJSONSync).mockReturnValue({ jscpd: { minLines: 7, silent: true } });
    const opts = prepareOptions(makeCmd());
    expect(opts.minLines).toBe(7);
    expect(opts.silent).toBe(true);
  });

  it('resolves relative path entries in jscpd.path to absolute', () => {
    vi.mocked(existsSync).mockImplementation((p) => String(p) === fakePackageJson);
    vi.mocked(readJSONSync).mockReturnValue({ jscpd: { path: ['./src'] } });
    const opts = prepareOptions(makeCmd());
    expect(opts.path).toContain(resolve(fakeCwd, './src'));
  });

  it('processes jscpd.ignore through resolveIgnorePattern', () => {
    vi.mocked(existsSync).mockImplementation((p) => String(p) === fakePackageJson);
    // **/ glob — should be returned unchanged by resolveIgnorePattern
    vi.mocked(readJSONSync).mockReturnValue({ jscpd: { ignore: ['**/node_modules/**'] } });
    const opts = prepareOptions(makeCmd());
    expect(opts.ignore).toContain('**/node_modules/**');
  });

  it('returns empty config and does not throw when package.json is malformed JSON', () => {
    vi.mocked(existsSync).mockImplementation((p) => String(p) === fakePackageJson);
    vi.mocked(readJSONSync).mockImplementation(() => {
      throw new SyntaxError('Unexpected token } in JSON at position 42');
    });
    // Must not throw — jscpd should continue with defaults
    expect(() => prepareOptions(makeCmd())).not.toThrow();
    const opts = prepareOptions(makeCmd());
    expect(opts).not.toHaveProperty('config');
  });
});

// ─── resolveIgnorePattern ─────────────────────────────────────────────────────

describe('resolveIgnorePattern (via readConfigJson in prepareOptions)', () => {
  beforeEach(() => {
    vi.mocked(existsSync).mockReturnValue(false);
    vi.mocked(readJSONSync).mockReturnValue({});
    vi.spyOn(process, 'cwd').mockReturnValue(fakeCwd);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  /** Drive a pattern through resolveIgnorePattern via readConfigJson. */
  const optsWithIgnore = (patterns: string[]) => {
    // Make .jscpd.json "exist" at fakeConfigFile
    vi.mocked(existsSync).mockImplementation((p) => String(p) === fakeConfigFile);
    vi.mocked(readJSONSync).mockReturnValue({ ignore: patterns });
    return prepareOptions(makeCmd());
  };

  it('returns absolute patterns unchanged', () => {
    const absPattern = '/absolute/path/to/vendor';
    const opts = optsWithIgnore([absPattern]);
    expect(opts.ignore).toContain(absPattern);
  });

  it('returns patterns starting with **/ unchanged', () => {
    const globPattern = '**/node_modules/**';
    const opts = optsWithIgnore([globPattern]);
    expect(opts.ignore).toContain(globPattern);
  });

  it('converts relative patterns within cwd to cwd-relative paths', () => {
    // configDir === fakeCwd; resolve(fakeCwd, './src') = fakeCwd/src
    // relative(fakeCwd, fakeCwd/src) = 'src' — no leading '..'
    const opts = optsWithIgnore(['./src']);
    expect(opts.ignore).toContain('src');
  });

  it('converts relative patterns that escape cwd to absolute paths', () => {
    // resolve(fakeCwd, '../outside') goes above fakeCwd → relative starts with '..'
    // resolveIgnorePattern returns absolute
    const opts = optsWithIgnore(['../outside-project']);
    const expectedAbsolute = resolve(fakeCwd, '../outside-project');
    expect(opts.ignore).toContain(expectedAbsolute);
    // Must be absolute (not contain '..')
    expect(expectedAbsolute).toMatch(/^\//);
  });
});
