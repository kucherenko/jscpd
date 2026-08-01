/**
 * Tests for ANSI color resolution: the pure precedence helper
 * (shouldEnableColors), the CLI flag parsing for --colors / --no-colors,
 * and configureColors() applying the result to the shared colors singleton.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { join } from 'path';
import { tmpdir } from 'os';
import * as colors from 'colors/safe';

vi.mock('fs', () => ({
  default: { existsSync: vi.fn(() => false) },
  existsSync: vi.fn(() => false),
}));

vi.mock('fs-extra', () => ({
  default: { readJSONSync: vi.fn(() => ({})) },
  readJSONSync: vi.fn(() => ({})),
}));

import { initCli } from '../src/init/cli';
import { prepareOptions } from '../src/options';
import { shouldEnableColors, configureColors } from '../src/init/colors';

const pkg = { name: 'jscpd', version: '0.0.0-test', description: 'test' };
const fakeCwd = join(tmpdir(), 'jscpd-colors-test');
const argv = (...flags: string[]) => ['', '', ...flags];

beforeEach(() => {
  vi.spyOn(process, 'cwd').mockReturnValue(fakeCwd);
});

afterEach(() => {
  vi.restoreAllMocks();
  colors.enable();
});

describe('shouldEnableColors precedence', () => {
  it('explicit colors:false wins over everything', () => {
    expect(
      shouldEnableColors({ colors: false, env: { FORCE_COLOR: '1' }, isTTY: true }),
    ).toBe(false);
  });

  it('explicit colors:true wins over NO_COLOR and non-TTY', () => {
    expect(
      shouldEnableColors({ colors: true, env: { NO_COLOR: '1' }, isTTY: false }),
    ).toBe(true);
  });

  it('NO_COLOR disables when set to a non-empty value', () => {
    expect(shouldEnableColors({ env: { NO_COLOR: '1' }, isTTY: true })).toBe(false);
  });

  it('empty NO_COLOR is ignored', () => {
    expect(shouldEnableColors({ env: { NO_COLOR: '' }, isTTY: true })).toBe(true);
  });

  it('FORCE_COLOR enables even without a TTY', () => {
    expect(shouldEnableColors({ env: { FORCE_COLOR: '1' }, isTTY: false })).toBe(true);
  });

  it('FORCE_COLOR=0 does not force colors', () => {
    expect(shouldEnableColors({ env: { FORCE_COLOR: '0' }, isTTY: false })).toBe(false);
  });

  it('NO_COLOR takes precedence over FORCE_COLOR', () => {
    expect(
      shouldEnableColors({ env: { NO_COLOR: '1', FORCE_COLOR: '1' }, isTTY: true }),
    ).toBe(false);
  });

  it('falls back to TTY detection when nothing is set', () => {
    expect(shouldEnableColors({ env: {}, isTTY: true })).toBe(true);
    expect(shouldEnableColors({ env: {}, isTTY: false })).toBe(false);
  });
});

describe('CLI parsing: --colors / --no-colors', () => {
  it('colors is undefined when neither flag is passed', () => {
    const opts = prepareOptions(initCli(pkg, argv()));
    expect(opts.colors).toBeUndefined();
  });

  it('colors is false when --no-colors is passed', () => {
    const opts = prepareOptions(initCli(pkg, argv('--no-colors')));
    expect(opts.colors).toBe(false);
  });

  it('colors is true when --colors is passed', () => {
    const opts = prepareOptions(initCli(pkg, argv('--colors')));
    expect(opts.colors).toBe(true);
  });
});

describe('configureColors applies to the colors singleton', () => {
  it('disables colors for a non-TTY stream without overrides', () => {
    const enabled = configureColors({}, {}, false);
    expect(enabled).toBe(false);
    expect(colors.enabled).toBe(false);
  });

  it('keeps colors enabled for a TTY stream', () => {
    const enabled = configureColors({}, {}, true);
    expect(enabled).toBe(true);
    expect(colors.enabled).toBe(true);
  });

  it('honors an explicit colors:false override on a TTY', () => {
    const enabled = configureColors({ colors: false }, {}, true);
    expect(enabled).toBe(false);
    expect(colors.enabled).toBe(false);
  });
});
