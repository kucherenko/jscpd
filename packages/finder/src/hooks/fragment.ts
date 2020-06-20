import {IClone} from '@jscpd/core';
import {readFileSync} from "fs";
import {IHook} from '..';

export class FragmentsHook implements IHook {

	process(clones: IClone[]): Promise<IClone[]> {
		return Promise.all(
			clones.map((clone: IClone) => FragmentsHook.addFragments(clone)),
		);
	}

	static addFragments(clone: IClone): IClone {
		const codeA = readFileSync(clone.duplicationA.sourceId).toString();
		const codeB = readFileSync(clone.duplicationB.sourceId).toString();
		clone.duplicationA.fragment = codeA.substring(clone.duplicationA.range[0], clone.duplicationA.range[1]);
		clone.duplicationB.fragment = codeB.substring(clone.duplicationB.range[0], clone.duplicationB.range[1]);
		return clone;
	}

}

