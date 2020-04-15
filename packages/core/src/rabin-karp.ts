import {IStore} from '@jscpd/store';
import {IMapFrame, TokensMap} from '@jscpd/tokenizer';
import {IClone, IOptions} from './interfaces';

export class RabinKarp {
	constructor(private readonly options: IOptions) {
	}

	public async run(tokenMap: TokensMap, store: IStore<IMapFrame>, onClone: (clone: IClone) => boolean | undefined): Promise<IClone[]> {
		let mapFrame;
		let mapFrameInStore;

		let clone: IClone | null = null;
		let done = false;

		const clones: IClone[] = [];

		while (!done) {
			const iteration = tokenMap.next();

			done = iteration.done;

			mapFrame = iteration.value;
			mapFrameInStore = store.get(mapFrame.id);

			if (mapFrameInStore && !done) {
				if (clone) {
					clone = RabinKarp.enlargeClone(clone, mapFrame, mapFrameInStore);
				} else {
					clone = RabinKarp.createClone(tokenMap.getFormat(), mapFrame, mapFrameInStore);
				}
			} else {
				if (clone && onClone(clone)) {
					clones.push(clone);
				}
				clone = null;
				store.set(mapFrame.id, mapFrame);
			}
		}
		if (clone) {
			clone = RabinKarp.enlargeClone(clone, mapFrame, mapFrameInStore)
			if (onClone(clone)) {
				clones.push(clone);
			}
		}
		return clones;
	}

	private static createClone(format: string, mapFrameA: IMapFrame, mapFrameB: IMapFrame): IClone {
		return {
			format,
			foundDate: new Date().getTime(),
			duplicationA: {
				sourceId: mapFrameA.sourceId,
				start: mapFrameA.start.loc.start,
				end: mapFrameA.end.loc.end,
				range: [mapFrameA.start.range[0], mapFrameA.end.range[1]],
			},
			duplicationB: {
				sourceId: mapFrameB.sourceId,
				start: mapFrameB.start.loc.start,
				end: mapFrameB.end.loc.end,
				range: [mapFrameB.start.range[0], mapFrameB.end.range[1]],
			},
		}
	}

	private static enlargeClone(clone: IClone, mapFrameA: IMapFrame, mapFrameB: IMapFrame): IClone {
		clone.duplicationA.range[1] = mapFrameA.end.range[1];
		clone.duplicationA.end = mapFrameA.end.loc.end;
		clone.duplicationB.range[1] = mapFrameB.end.range[1];
		clone.duplicationB.end = mapFrameB.end.loc.end;
		return clone;
	}
}
