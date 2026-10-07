import { describe, it, expect } from 'vitest';
import { spawnSync } from 'child_process';
import { cpSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, symlinkSync } from 'fs';
import { tmpdir } from 'os';
import { dirname, join } from 'path';

describe('HTML reporter package', () => {
  it('loads and writes a report with only its declared runtime dependencies', () => {
    const packageRoot = join(__dirname, '../../../packages/html-reporter');
    const manifest = JSON.parse(readFileSync(join(packageRoot, 'package.json'), 'utf8'));
    const tempRoot = realpathSync(tmpdir());
    const consumerRoot = mkdtempSync(join(tempRoot, 'jscpd-html-package-'));

    try {
      const installedPackage = join(consumerRoot, 'node_modules', manifest.name);
      mkdirSync(installedPackage, { recursive: true });
      cpSync(join(packageRoot, 'package.json'), join(installedPackage, 'package.json'));
      for (const file of manifest.files) {
        cpSync(join(packageRoot, file), join(installedPackage, file), { recursive: true });
      }

      // Copy the built package out of the workspace so hoisting and development
      // dependencies cannot hide a missing runtime dependency.
      for (const dependency of Object.keys(manifest.dependencies)) {
        const destination = join(installedPackage, 'node_modules', dependency);
        mkdirSync(dirname(destination), { recursive: true });
        symlinkSync(realpathSync(join(packageRoot, 'node_modules', dependency)), destination,
          process.platform === 'win32' ? 'junction' : 'dir');
      }

      const result = spawnSync(process.execPath, ['-e', `
        const HtmlReporter = require('@jscpd/html-reporter').default;
        new HtmlReporter({ output: 'report' }).report([], {
          total: {
            sources: 2, lines: 10, tokens: 50, clones: 0,
            duplicatedLines: 0, duplicatedTokens: 0,
            percentage: 0, percentageTokens: 0
          },
          formats: {}
        });
      `], { cwd: consumerRoot, encoding: 'utf8', env: { ...process.env, NODE_PATH: '' } });

      expect(result.error).toBeUndefined();
      expect(result.status, result.stderr).toBe(0);
      const reportRoot = join(consumerRoot, 'report', 'html');
      expect(readFileSync(join(reportRoot, 'index.html'), 'utf8')).toContain('Copy/Paste Detector Report');
      const report = JSON.parse(readFileSync(join(reportRoot, 'jscpd-report.json'), 'utf8'));
      expect(report.statistics.total.sources).toBe(2);
      expect(report.duplicates).toEqual([]);
      expect(readFileSync(join(reportRoot, 'js', 'prism.js'), 'utf8')).toContain('Prism');
    } finally {
      expect(dirname(realpathSync(consumerRoot))).toBe(tempRoot);
      rmSync(consumerRoot, { recursive: true, force: true });
    }
  });
});
