/**
 * Tests for CLI option parsing via initCli() + prepareOptions().
 *
 * These tests use a real Commander instance (via initCli) so they catch
 * breakage when the Commander API changes (e.g. direct property access
 * removed in v8+, replaced by opts()).
 *
 * Filesystem is fully mocked so no real config files are read.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { join } from 'path';
import { tmpdir } from 'os';

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

const pkg = { name: 'jscpd', version: '0.0.0-test', description: 'test' };
const fakeCwd = join(tmpdir(), 'jscpd-cli-parsing-test');

/** Build argv the way Node passes it: ['node', 'jscpd', ...flags] */
const argv = (...flags: string[]) => ['', '', ...flags];

beforeEach(() => {
  vi.spyOn(process, 'cwd').mockReturnValue(fakeCwd);
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('initCli + prepareOptions: numeric options', () => {
  it('parses --min-tokens into a number', () => {
    const cli = initCli(pkg, argv('--min-tokens', '5'));
    const opts = prepareOptions(cli);
    expect(opts.minTokens).toBe(5);
  });

  it('parses --min-lines into a number', () => {
    const cli = initCli(pkg, argv('--min-lines', '3'));
    const opts = prepareOptions(cli);
    expect(opts.minLines).toBe(3);
  });

  it('parses --max-lines into a number', () => {
    const cli = initCli(pkg, argv('--max-lines', '500'));
    const opts = prepareOptions(cli);
    expect(opts.maxLines).toBe(500);
  });

  it('parses --threshold into a number', () => {
    const cli = initCli(pkg, argv('--threshold', '15'));
    const opts = prepareOptions(cli);
    expect(opts.threshold).toBe(15);
  });
});

describe('initCli + prepareOptions: string options', () => {
  it('parses --mode', () => {
    const cli = initCli(pkg, argv('--mode', 'strict'));
    const opts = prepareOptions(cli);
    expect(opts.mode).toBe('strict');
  });

  it('parses --max-size', () => {
    const cli = initCli(pkg, argv('--max-size', '1mb'));
    const opts = prepareOptions(cli);
    expect(opts.maxSize).toBe('1mb');
  });

  it('parses --output', () => {
    const cli = initCli(pkg, argv('--output', './my-reports'));
    const opts = prepareOptions(cli);
    expect(opts.output).toBe('./my-reports');
  });

  it('parses --store', () => {
    const cli = initCli(pkg, argv('--store', 'leveldb'));
    const opts = prepareOptions(cli);
    expect(opts.store).toBe('leveldb');
  });

  it('parses --pattern', () => {
    const cli = initCli(pkg, argv('--pattern', '**/*.ts'));
    const opts = prepareOptions(cli);
    expect(opts.pattern).toBe('**/*.ts');
  });
});

describe('initCli + prepareOptions: boolean flags', () => {
  it('parses --silent', () => {
    const cli = initCli(pkg, argv('--silent'));
    const opts = prepareOptions(cli);
    expect(opts.silent).toBe(true);
  });

  it('parses --debug', () => {
    const cli = initCli(pkg, argv('--debug'));
    const opts = prepareOptions(cli);
    expect(opts.debug).toBe(true);
  });

  it('parses --verbose', () => {
    const cli = initCli(pkg, argv('--verbose'));
    const opts = prepareOptions(cli);
    expect(opts.verbose).toBe(true);
  });

  it('parses --absolute', () => {
    const cli = initCli(pkg, argv('--absolute'));
    const opts = prepareOptions(cli);
    expect(opts.absolute).toBe(true);
  });

  it('parses --blame', () => {
    const cli = initCli(pkg, argv('--blame'));
    const opts = prepareOptions(cli);
    expect(opts.blame).toBe(true);
  });

  it('parses --skipLocal', () => {
    const cli = initCli(pkg, argv('--skipLocal'));
    const opts = prepareOptions(cli);
    expect(opts.skipLocal).toBe(true);
  });

  it('parses --ignoreCase', () => {
    const cli = initCli(pkg, argv('--ignoreCase'));
    const opts = prepareOptions(cli);
    expect(opts.ignoreCase).toBe(true);
  });

  it('parses --noSymlinks', () => {
    const cli = initCli(pkg, argv('--noSymlinks'));
    const opts = prepareOptions(cli);
    expect(opts.noSymlinks).toBe(true);
  });

  it('parses --skipComments and maps to mode weak', () => {
    const cli = initCli(pkg, argv('--skipComments'));
    const opts = prepareOptions(cli);
    expect(opts.mode).toBe('weak');
  });
});

describe('initCli + prepareOptions: comma-separated options', () => {
  it('splits --reporters by comma', () => {
    const cli = initCli(pkg, argv('--reporters', 'console,badge'));
    const opts = prepareOptions(cli);
    expect(opts.reporters).toEqual(['console', 'badge']);
  });

  it('splits --format by comma', () => {
    const cli = initCli(pkg, argv('--format', 'javascript,typescript'));
    const opts = prepareOptions(cli);
    expect(opts.format).toEqual(['javascript', 'typescript']);
  });

  it('splits --ignore by comma', () => {
    const cli = initCli(pkg, argv('--ignore', '**/node_modules/**,**/dist/**'));
    const opts = prepareOptions(cli);
    expect(opts.ignore).toEqual(['**/node_modules/**', '**/dist/**']);
  });

  it('splits --ignore-pattern by comma', () => {
    const cli = initCli(pkg, argv('--ignore-pattern', 'foo.*,bar.*'));
    const opts = prepareOptions(cli);
    expect(opts.ignorePattern).toEqual(['foo.*', 'bar.*']);
  });
});

describe('initCli + prepareOptions: gitignore flag', () => {
  it('gitignore is NOT overridden when neither --gitignore nor --no-gitignore is passed', () => {
    const cli = initCli(pkg, argv());
    const opts = prepareOptions(cli);
    // When not explicitly set, argsConfig should not inject gitignore,
    // so the merged result comes from defaults (gitignore: true).
    expect(opts.gitignore).toBe(true);
  });

  it('gitignore is true when --gitignore is explicitly passed', () => {
    const cli = initCli(pkg, argv('--gitignore'));
    const opts = prepareOptions(cli);
    expect(opts.gitignore).toBe(true);
  });

  it('gitignore is false when --no-gitignore is explicitly passed', () => {
    const cli = initCli(pkg, argv('--no-gitignore'));
    const opts = prepareOptions(cli);
    expect(opts.gitignore).toBe(false);
  });
});

describe('initCli + prepareOptions: positional path args', () => {
  it('collects positional args into path', () => {
    const cli = initCli(pkg, argv('/some/path', '/other/path'));
    const opts = prepareOptions(cli);
    expect(opts.path).toContain('/some/path');
    expect(opts.path).toContain('/other/path');
  });
});

describe('initCli + prepareOptions: exitCode option', () => {
  it('parses --exitCode', () => {
    const cli = initCli(pkg, argv('--exitCode', '2'));
    const opts = prepareOptions(cli);
    expect(opts.exitCode).toBe('2');
  });
});
