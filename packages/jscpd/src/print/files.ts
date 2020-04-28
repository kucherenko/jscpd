import {bold, grey} from 'colors/safe';
import {EntryWithContent} from '@jscpd/finder';

export function printFiles(files: EntryWithContent[]): void {
	files.forEach((stats: EntryWithContent) => {
		console.log(grey(stats.path));
	});
	console.log(bold(`Found ${files.length} files to detect.`));
}
