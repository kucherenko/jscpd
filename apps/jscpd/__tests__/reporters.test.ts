import {describe, it, afterEach, beforeEach, expect, vi} from "vitest";
import {jscpd} from '../src';
import {green, grey} from 'colors/safe';
import {join} from 'path'

const pathToFixtures = join(__dirname, '/../../../fixtures');

describe('jscpd reporters', () => {

	let _log, _error;

	beforeEach(() => {
		_log = console.log;
		_error = console.error;
		console.log = vi.fn();
		console.error = vi.fn();
	})

	afterEach(() => {
		console.log = _log;
		console.error = _error;
	})

	describe('JSON', () => {

		it('should save json with report', async () => {
			const log = (console.log as any);
			await jscpd(['', '', pathToFixtures + '/clike/file2.c', '--reporters', 'json']);
			expect(log).toHaveBeenCalledWith(green('JSON report saved to report/jscpd-report.json'))
		});
	});

	describe('CSV', () => {

		it('should save csv with report', async () => {
			const log = (console.log as any);
			await jscpd(['', '', pathToFixtures + '/clike/file2.c', '--reporters', 'csv']);
			expect(log).toHaveBeenCalledWith(green('CSV report saved to report/jscpd-report.csv'))
		});
	});

	describe('Markdown', () => {

		it('should save markdown with report', async () => {
			const log = (console.log as any);
			await jscpd(['', '', pathToFixtures + '/clike/file2.c', '--reporters', 'markdown']);
			expect(log).toHaveBeenCalledWith(green('Markdown report saved to report/jscpd-report.md'));
		});
	});

	describe('XML', () => {

		it('should save xml with report', async () => {
      const log = (console.log as any);
      await jscpd(['', '', pathToFixtures + '/clike/file2.c', '--reporters', 'xml']);
      expect(log).toHaveBeenCalledWith(green('XML report saved to report/jscpd-report.xml'));
    });
  });

  describe('HTML', () => {

    it('should save html with report', async () => {
      const log = (console.log as any);
      await jscpd(['', '', pathToFixtures + '/clike/file2.c', '--reporters', 'html']);
      expect(log).toHaveBeenCalledWith(green('HTML report saved to report/html/'));
    });
  });

  describe('badge', () => {

    it('should save badge', async () => {
      const log = (console.log as any);
      await jscpd(['', '', pathToFixtures + '/clike/file2.c', '--reporters', 'badge']);
      expect(log).toHaveBeenCalledWith(green('Badge saved to report/jscpd-badge.svg'))
    });
  });

  describe('Console Full', () => {
    it('should generate report with table', async () => {
      const log = (console.log as any);
      await jscpd(['', '', pathToFixtures + '/clike/file2.c', '--reporters', 'consoleFull']);
      expect(log).toHaveBeenCalledWith(grey('Found 1 clones.'));
		});
	});

	describe('Xcode', () => {
		it('should generate report with Xcode warnings with second file absolute path', async () => {
			const log = (console.log as any);
			const fullPathToFile = join(pathToFixtures, '/clike/file2.c');
			const expected = fullPathToFile + ':18:3: warning: Found 10 lines (18-28) duplicated on file ' + fullPathToFile + ' (8-18)';
			await jscpd(['', '', fullPathToFile, '--reporters', 'xcode', '--absolute']);
			expect(log).toHaveBeenCalledWith(expected)
		});

		it('should generate report with Xcode warnings with second file relative path', async () => {
			const log = (console.log as any);
			const fullPathToFile = join(pathToFixtures, '/clike/file2.c');
			const relativePath = '../../fixtures/clike/file2.c';
			const expected = fullPathToFile + ':18:3: warning: Found 10 lines (18-28) duplicated on file ' + relativePath + ' (8-18)';
			await jscpd(['', '', fullPathToFile, '--reporters', 'xcode']);
			expect(log).toHaveBeenCalledWith(expected);
		});
	});
});
