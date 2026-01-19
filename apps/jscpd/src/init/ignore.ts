import {IOptions} from '@jscpd/core';
import {existsSync, readFileSync} from 'fs';
import {join} from 'path';

function convertGitignorePatternToGlob(line: string): string[] {
	// Handle negation patterns
	if (line.startsWith('!')) {
		// Negation in gitignore means "don't ignore this"
		// In fast-glob's ignore option, we skip negation patterns
		return [];
	}

	// Strip leading slash (means anchored to root in gitignore)
	const isRooted = line.startsWith('/');
	let pattern = isRooted ? line.slice(1) : line;

	// Strip trailing slash (gitignore uses it for "directory only", but glob patterns
	// don't distinguish file types, so we just strip it)
	if (pattern.endsWith('/')) {
		pattern = pattern.slice(0, -1);
	}

	const results: string[] = [];

	if (isRooted) {
		// Root-anchored patterns: match only at project root
		results.push(pattern);
		results.push(`${pattern}/**`);
	} else if (pattern.includes('/')) {
		// Patterns with / are relative paths, match at any depth
		if (pattern.startsWith('**/')) {
			// Already has double-star prefix, don't duplicate
			results.push(pattern);
			results.push(`${pattern}/**`);
		} else {
			results.push(pattern);
			results.push(`${pattern}/**`);
			results.push(`**/${pattern}`);
			results.push(`**/${pattern}/**`);
		}
	} else {
		// Simple patterns match at any depth
		results.push(`**/${pattern}`);
		results.push(`**/${pattern}/**`);
	}

	return results;
}

export function initIgnore(options: IOptions): string[] {
	// Create a shallow copy to avoid mutating caller's options
	const ignore: string[] = options.ignore ? [...options.ignore] : [];

	if (options.gitignore && existsSync(join(process.cwd(), '.gitignore'))) {
		const gitignorePath = join(process.cwd(), '.gitignore');
		const content = readFileSync(gitignorePath, 'utf8');

		const gitignorePatterns = content
			.split('\n')
			.map(line => line.trim())
			.filter(line => line && !line.startsWith('#'))
			.flatMap(convertGitignorePatternToGlob);

		ignore.push(...gitignorePatterns);
	}

	return ignore;
}
