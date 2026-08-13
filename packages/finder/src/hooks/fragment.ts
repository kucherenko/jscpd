import {IClone} from '@jscpd/core';
import {readFileSync} from "fs";
import {IHook} from '..';

/**
 * Reporters print a clone fragment next to the line numbers taken from the
 * clone location, so the fragment has to start and end on line boundaries.
 * A clone range, however, starts at the first duplicated *token*, which is not
 * necessarily the first token of its line, and may end just after the newline
 * that terminates the last line. Snapping the range to whole lines keeps the
 * printed code in step with the printed line numbers.
 */
function readWholeLines(code: string, [from, to]: [number, number]): string {
	// Beginning of the line containing `from` (0 when there is no newline before it).
	const start = code.lastIndexOf('\n', from - 1) + 1;

	// A range that ends right after a line terminator already covers whole
	// lines; without stepping back, the next line would be pulled in.
	let end = to;
	if (end > start && code[end - 1] === '\n') {
		end--;
	}

	const lineEnd = code.indexOf('\n', end);
	return code.substring(start, lineEnd === -1 ? code.length : lineEnd);
}

export class FragmentsHook implements IHook {

	process(clones: IClone[]): Promise<IClone[]> {
		return Promise.all(
			clones.map((clone: IClone) => FragmentsHook.addFragments(clone)),
		);
	}

	static addFragments(clone: IClone): IClone {
		const codeA = readFileSync(clone.duplicationA.sourceId).toString();
		const codeB = readFileSync(clone.duplicationB.sourceId).toString();
		clone.duplicationA.fragment = readWholeLines(codeA, clone.duplicationA.range);
		clone.duplicationB.fragment = readWholeLines(codeB, clone.duplicationB.range);
		return clone;
	}

}

