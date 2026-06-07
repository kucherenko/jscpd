import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { tmpdir } from 'os';
import { join } from 'path';
import type { IStatistic } from '@jscpd/core';
import { buildClone } from './helpers/clone-builder';

vi.mock('fs-extra', () => ({
  ensureDirSync: vi.fn(),
  writeFileSync: vi.fn(),
  readFileSync: vi.fn(),
}));

vi.mock('fs', () => ({
  writeFileSync: vi.fn(),
}));

import { ensureDirSync, writeFileSync } from 'fs-extra';
import { writeFileSync as fsWriteFileSync } from 'fs';

function buildStatistic(): IStatistic {
  return {
    total: {
      sources: 1,
      lines: 100,
      tokens: 500,
      clones: 1,
      duplicatedLines: 10,
      duplicatedTokens: 50,
      percentage: 10,
      percentageTokens: 10,
    },
    formats: {
      javascript: {
        sources: { 'file.js': {} as any },
        total: {
          sources: 1,
          lines: 100,
          tokens: 500,
          clones: 1,
          duplicatedLines: 10,
          duplicatedTokens: 50,
          percentage: 10,
          percentageTokens: 10,
        },
      },
    },
  } as IStatistic;
}

const options = { output: join(tmpdir(), 'jscpd-test'), silent: false } as any;

describe('SilentReporter', () => {
  let consoleSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    consoleSpy = vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  afterEach(() => {
    consoleSpy.mockRestore();
  });

  it('report(clones, statistic) calls console.log with clone count', async () => {
    const { SilentReporter } = await import('../src/reporters/silent');
    const reporter = new SilentReporter();
    const clones = [buildClone()];
    reporter.report(clones, buildStatistic());
    expect(consoleSpy).toHaveBeenCalledOnce();
    expect(consoleSpy.mock.calls[0][0]).toContain('1');
  });

  it('report(clones, undefined) does NOT call console.log', async () => {
    const { SilentReporter } = await import('../src/reporters/silent');
    const reporter = new SilentReporter();
    reporter.report([buildClone()], undefined as any);
    expect(consoleSpy).not.toHaveBeenCalled();
  });
});

describe('ThresholdReporter', () => {
  let consoleErrorSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
  });

  afterEach(() => {
    consoleErrorSpy.mockRestore();
  });

  it('throws when percentage exceeds threshold', async () => {
    const { ThresholdReporter } = await import('../src/reporters/threshold');
    const reporter = new ThresholdReporter({ threshold: 5 } as any);
    const stat = buildStatistic();
    stat.total.percentage = 10;
    expect(() => reporter.report([], stat)).toThrow(/10%/);
  });

  it('does NOT throw when percentage is below threshold', async () => {
    const { ThresholdReporter } = await import('../src/reporters/threshold');
    const reporter = new ThresholdReporter({ threshold: 50 } as any);
    const stat = buildStatistic();
    stat.total.percentage = 10;
    expect(() => reporter.report([], stat)).not.toThrow();
  });

  it('does NOT throw when statistic is undefined', async () => {
    const { ThresholdReporter } = await import('../src/reporters/threshold');
    const reporter = new ThresholdReporter({ threshold: 5 } as any);
    expect(() => reporter.report([], undefined as any)).not.toThrow();
  });
});

describe('ConsoleReporter', () => {
  let consoleSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    consoleSpy = vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  afterEach(() => {
    consoleSpy.mockRestore();
  });

  it('calls console.log when silent: false', async () => {
    const { ConsoleReporter } = await import('../src/reporters/console');
    const reporter = new ConsoleReporter({ silent: false } as any);
    reporter.report([buildClone()], buildStatistic());
    expect(consoleSpy).toHaveBeenCalled();
  });

  it('does NOT call console.log when silent: true', async () => {
    const { ConsoleReporter } = await import('../src/reporters/console');
    const reporter = new ConsoleReporter({ silent: true } as any);
    reporter.report([buildClone()], buildStatistic());
    expect(consoleSpy).not.toHaveBeenCalled();
  });

  it('does NOT call console.log when statistic is undefined', async () => {
    const { ConsoleReporter } = await import('../src/reporters/console');
    const reporter = new ConsoleReporter({ silent: false } as any);
    reporter.report([buildClone()], undefined);
    expect(consoleSpy).not.toHaveBeenCalled();
  });
});

describe('XcodeReporter', () => {
  let consoleSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    consoleSpy = vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  afterEach(() => {
    consoleSpy.mockRestore();
  });

  it('report([clone]) calls console.log with "Found 1 clones."', async () => {
    const { XcodeReporter } = await import('../src/reporters/xcode');
    const reporter = new XcodeReporter(options);
    reporter.report([buildClone()]);
    const calls = consoleSpy.mock.calls.map((c) => c[0]);
    expect(calls.some((msg) => msg === 'Found 1 clones.')).toBe(true);
  });

  it('report([clone]) calls console.log with source path and line numbers', async () => {
    const { XcodeReporter } = await import('../src/reporters/xcode');
    const reporter = new XcodeReporter(options);
    const clone = buildClone();
    reporter.report([clone]);
    const calls = consoleSpy.mock.calls.map((c) => c[0]);
    expect(calls.some((msg) => msg.includes('1') && msg.includes('10'))).toBe(true);
  });

  it('report([]) calls console.log with "Found 0 clones."', async () => {
    const { XcodeReporter } = await import('../src/reporters/xcode');
    const reporter = new XcodeReporter(options);
    reporter.report([]);
    expect(consoleSpy).toHaveBeenCalledWith('Found 0 clones.');
  });
});

describe('ConsoleFullReporter', () => {
  let consoleSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    consoleSpy = vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  afterEach(() => {
    consoleSpy.mockRestore();
  });

  it('report([clone]) calls console.log at least twice', async () => {
    const { ConsoleFullReporter } = await import('../src/reporters/console-full');
    const reporter = new ConsoleFullReporter(options);
    reporter.report([buildClone()]);
    expect(consoleSpy.mock.calls.length).toBeGreaterThanOrEqual(2);
  });

  it('report([]) calls console.log with "Found 0 clones."', async () => {
    const { ConsoleFullReporter } = await import('../src/reporters/console-full');
    const reporter = new ConsoleFullReporter(options);
    reporter.report([]);
    const calls = consoleSpy.mock.calls.map((c) => c[0]);
    expect(calls.some((msg) => typeof msg === 'string' && msg.includes('Found 0 clones.'))).toBe(true);
  });
});

describe('CSVReporter', () => {
  beforeEach(() => {
    vi.mocked(ensureDirSync).mockClear();
    vi.mocked(writeFileSync).mockClear();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('calls ensureDirSync with options.output', async () => {
    const { CSVReporter } = await import('../src/reporters/csv');
    const reporter = new CSVReporter(options);
    reporter.report([buildClone()], buildStatistic());
    expect(vi.mocked(ensureDirSync)).toHaveBeenCalledWith(options.output);
  });

  it('calls writeFileSync with path ending in jscpd-report.csv', async () => {
    const { CSVReporter } = await import('../src/reporters/csv');
    const reporter = new CSVReporter(options);
    reporter.report([buildClone()], buildStatistic());
    const calls = vi.mocked(writeFileSync).mock.calls;
    expect(calls.some((c) => String(c[0]).endsWith('jscpd-report.csv'))).toBe(true);
  });
});

describe('JsonReporter', () => {
  beforeEach(() => {
    vi.mocked(ensureDirSync).mockClear();
    vi.mocked(writeFileSync).mockClear();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('generateJson returns object with duplicates array and statistics', async () => {
    const { JsonReporter } = await import('../src/reporters/json');
    const reporter = new JsonReporter(options);
    const result = reporter.generateJson([buildClone()], buildStatistic());
    expect(result).toHaveProperty('duplicates');
    expect(result).toHaveProperty('statistics');
    expect(Array.isArray(result.duplicates)).toBe(true);
  });

  it('generateJson reports duplicate token count from token positions', async () => {
    const { JsonReporter } = await import('../src/reporters/json');
    const reporter = new JsonReporter(options);
    const clone = buildClone({
      duplicationA: {
        start: { line: 3, column: 1, position: 25 },
        end: { line: 12, column: 1, position: 93 },
      },
    });
    const result = reporter.generateJson([clone], buildStatistic());
    expect(result.duplicates[0].tokens).toBe(68);
  });

  it('calls writeFileSync with path ending in jscpd-report.json', async () => {
    const { JsonReporter } = await import('../src/reporters/json');
    const reporter = new JsonReporter(options);
    reporter.report([buildClone()], buildStatistic());
    const calls = vi.mocked(writeFileSync).mock.calls;
    expect(calls.some((c) => String(c[0]).endsWith('jscpd-report.json'))).toBe(true);
  });
});

describe('MarkdownReporter', () => {
  beforeEach(() => {
    vi.mocked(ensureDirSync).mockClear();
    vi.mocked(writeFileSync).mockClear();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('calls writeFileSync with path ending in jscpd-report.md', async () => {
    const { MarkdownReporter } = await import('../src/reporters/markdown');
    const reporter = new MarkdownReporter(options);
    reporter.report([buildClone()], buildStatistic());
    const calls = vi.mocked(writeFileSync).mock.calls;
    expect(calls.some((c) => String(c[0]).endsWith('jscpd-report.md'))).toBe(true);
  });
});

describe('XmlReporter', () => {
  beforeEach(() => {
    vi.mocked(ensureDirSync).mockClear();
    vi.mocked(fsWriteFileSync).mockClear();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('calls writeFileSync with path ending in jscpd-report.xml', async () => {
    const { XmlReporter } = await import('../src/reporters/xml');
    const reporter = new XmlReporter(options);
    reporter.report([buildClone()]);
    const calls = vi.mocked(fsWriteFileSync).mock.calls;
    expect(calls.some((c) => String(c[0]).endsWith('jscpd-report.xml'))).toBe(true);
  });

  it('writeFileSync content contains <pmd-cpd>', async () => {
    const { XmlReporter } = await import('../src/reporters/xml');
    const reporter = new XmlReporter(options);
    reporter.report([buildClone()]);
    const calls = vi.mocked(fsWriteFileSync).mock.calls;
    const xmlCall = calls.find((c) => String(c[0]).endsWith('jscpd-report.xml'));
    expect(xmlCall).toBeDefined();
    expect(String(xmlCall![1])).toContain('<pmd-cpd>');
  });
});
