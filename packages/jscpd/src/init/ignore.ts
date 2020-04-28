import {IOptions} from '@jscpd/core';
import {existsSync} from "fs";

const gitignoreToGlob = require('gitignore-to-glob');

export function initIgnore(options: IOptions): string[] {
	const ignore: string[] = options.ignore || [];

	if (options.gitignore && existsSync(process.cwd() + '/.gitignore')) {
		let gitignorePatterns: string[] = gitignoreToGlob(process.cwd() + '/.gitignore') || [];
		gitignorePatterns = gitignorePatterns.map((pattern) =>
			pattern.substr(pattern.length - 1) === '/' ? `${pattern}**/*` : pattern,
		);
		ignore.push(...gitignorePatterns);
		ignore.map((pattern) => pattern.replace('!', ''));
	}
	return ignore;
}
