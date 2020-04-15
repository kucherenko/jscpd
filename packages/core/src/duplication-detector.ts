import {IStore} from '@jscpd/store';
import {createTokenMapBasedOnCode, TokensMap} from '@jscpd/tokenizer';
import {RabinKarp} from './rabin-karp';
import EventEmitter = require('eventemitter3');
import {IClone, IOptions} from './interfaces';

export type DetectorEvents = 'CLONE_FOUND' | 'CLONE_SKIPPED' | 'START_DETECTION';

export class DuplicationDetector extends EventEmitter<DetectorEvents> {

	private algorithm: RabinKarp;

	constructor(private readonly options: IOptions, private readonly store: IStore<any>) {
		super();
		this.algorithm = new RabinKarp(this.options);
	}

	public async detect(id: string, text: string, format: string): Promise<IClone[]> {
		const tokenMaps: TokensMap[] = createTokenMapBasedOnCode(id, text, format, this.options);
		this.store.namespace(format);
		const clones: IClone[][] = await Promise.all(tokenMaps.map(
			(tokenMap: TokensMap) => {
				this.emit('START_DETECTION', tokenMap);
				return this.algorithm.run(
					tokenMap,
					this.store,
					(clone: IClone): boolean => {
						// TODO add skipers
						const lines = clone.duplicationA.end.line - clone.duplicationA.start.line;
						if ( lines >= this.options.minLines) {
							this.emit('CLONE_FOUND', clone)
							return true;
						} else {
							this.emit('CLONE_SKIPPED', clone)
							return false;
						}
					},
				)
			},
		));

		return clones.reduce((acc: IClone[], current: IClone[]) => {
			if (current.length > 0) {
				acc.push(...current)
			}
			return acc;
		}, []);
	}
}
