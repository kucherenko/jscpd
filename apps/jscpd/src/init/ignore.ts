import {IOptions} from '@jscpd/core';
import {existsSync, readFileSync} from 'fs';

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

	// Strip trailing slash (means directory only)
	const isDirectory = pattern.endsWith('/');
	if (isDirectory) {
		pattern = pattern.slice(0, -1);
	}

	const results: string[] = [];

	if (isRooted) {
		// Root-anchored patterns: match only at project root
		results.push(pattern);
		results.push(`${pattern}/**`);
	} else if (pattern.includes('/')) {
		// Patterns with / are relative paths, match at any depth
		results.push(pattern);
		results.push(`${pattern}/**`);
		results.push(`**/${pattern}`);
		results.push(`**/${pattern}/**`);
	} else {
		// Simple patterns match at any depth
		results.push(`**/${pattern}`);
		results.push(`**/${pattern}/**`);
	}

	return results;
}

export function initIgnore(options: IOptions): string[] {
	const ignore: string[] = options.ignore || [];

	if (options.gitignore && existsSync(process.cwd() + '/.gitignore')) {
		const gitignorePath = process.cwd() + '/.gitignore';
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
