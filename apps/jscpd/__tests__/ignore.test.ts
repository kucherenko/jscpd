import {describe, it, expect, beforeEach, afterEach} from 'vitest';
import {existsSync, writeFileSync, mkdirSync, rmSync} from 'fs';
import {initIgnore} from '../src/init/ignore';
import {IOptions} from '@jscpd/core';
import {join} from 'path';

describe('initIgnore with gitignore', () => {
	const testDir = join(process.cwd(), 'test-fixtures', 'gitignore');
	const gitignorePath = join(testDir, '.gitignore');
	let originalCwd: string;

	beforeEach(() => {
		originalCwd = process.cwd();
		if (!existsSync(testDir)) {
			mkdirSync(testDir, {recursive: true});
		}
		// Change to test directory
		process.chdir(testDir);
	});

	afterEach(() => {
		// Restore original directory
		process.chdir(originalCwd);
		// Clean up
		if (existsSync(testDir)) {
			rmSync(testDir, {recursive: true, force: true});
		}
	});

	it('should handle dot-prefixed patterns', () => {
		writeFileSync(gitignorePath, '.next\n.env');
		const options: IOptions = {gitignore: true};
		const patterns = initIgnore(options);

		expect(patterns).toContain('**/.next');
		expect(patterns).toContain('**/.next/**');
		expect(patterns).toContain('**/.env');
		expect(patterns).toContain('**/.env/**');
	});

	it('should handle leading slash patterns', () => {
		writeFileSync(gitignorePath, '/node_modules\n/coverage');
		const options: IOptions = {gitignore: true};
		const patterns = initIgnore(options);

		expect(patterns).toContain('node_modules');
		expect(patterns).toContain('node_modules/**');
		expect(patterns).toContain('coverage');
		expect(patterns).toContain('coverage/**');
	});

	it('should handle trailing slash patterns', () => {
		writeFileSync(gitignorePath, 'build/\ndist/');
		const options: IOptions = {gitignore: true};
		const patterns = initIgnore(options);

		expect(patterns).toContain('**/build');
		expect(patterns).toContain('**/build/**');
		expect(patterns).toContain('**/dist');
		expect(patterns).toContain('**/dist/**');
	});

	it('should handle complex patterns with leading slash, dot, and trailing slash', () => {
		writeFileSync(gitignorePath, '/.next/');
		const options: IOptions = {gitignore: true};
		const patterns = initIgnore(options);

		expect(patterns).toContain('.next');
		expect(patterns).toContain('.next/**');
	});

	it('should handle patterns with slashes in the middle', () => {
		writeFileSync(gitignorePath, 'src/dist');
		const options: IOptions = {gitignore: true};
		const patterns = initIgnore(options);

		expect(patterns).toContain('src/dist');
		expect(patterns).toContain('src/dist/**');
		expect(patterns).toContain('**/src/dist');
		expect(patterns).toContain('**/src/dist/**');
	});

	it('should ignore negation patterns', () => {
		writeFileSync(gitignorePath, 'logs\n!important.log');
		const options: IOptions = {gitignore: true};
		const patterns = initIgnore(options);

		expect(patterns).toContain('**/logs');
		expect(patterns).toContain('**/logs/**');
		expect(patterns.some(p => p.includes('important.log'))).toBe(false);
	});

	it('should filter out comments and empty lines', () => {
		writeFileSync(gitignorePath, '# Comment\n\nnode_modules\n\n# Another comment\ndist');
		const options: IOptions = {gitignore: true};
		const patterns = initIgnore(options);

		expect(patterns.some(p => p.includes('#'))).toBe(false);
		expect(patterns).toContain('**/node_modules');
		expect(patterns).toContain('**/dist');
	});

	it('should merge with existing ignore patterns', () => {
		writeFileSync(gitignorePath, 'node_modules');
		const options: IOptions = {
			gitignore: true,
			ignore: ['**/*.test.js']
		};
		const patterns = initIgnore(options);

		expect(patterns).toContain('**/*.test.js');
		expect(patterns).toContain('**/node_modules');
	});

	it('should handle missing gitignore file gracefully', () => {
		const options: IOptions = {gitignore: true};
		const patterns = initIgnore(options);

		expect(patterns).toEqual([]);
	});

	it('should handle simple patterns without slashes', () => {
		writeFileSync(gitignorePath, 'node_modules\ncoverage');
		const options: IOptions = {gitignore: true};
		const patterns = initIgnore(options);

		expect(patterns).toContain('**/node_modules');
		expect(patterns).toContain('**/node_modules/**');
		expect(patterns).toContain('**/coverage');
		expect(patterns).toContain('**/coverage/**');
	});

	it('should handle wildcard patterns', () => {
		writeFileSync(gitignorePath, '*.log\n*.tmp');
		const options: IOptions = {gitignore: true};
		const patterns = initIgnore(options);

		expect(patterns).toContain('**/*.log');
		expect(patterns).toContain('**/*.log/**');
		expect(patterns).toContain('**/*.tmp');
		expect(patterns).toContain('**/*.tmp/**');
	});

	it('should handle double-star patterns', () => {
		writeFileSync(gitignorePath, '**/dist\n**/build');
		const options: IOptions = {gitignore: true};
		const patterns = initIgnore(options);

		expect(patterns).toContain('**/dist');
		expect(patterns).toContain('**/dist/**');
		expect(patterns).toContain('**/build');
		expect(patterns).toContain('**/build/**');
	});
});
