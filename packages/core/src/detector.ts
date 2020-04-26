import {createTokenMapBasedOnCode, TokensMap} from '@jscpd/tokenizer';
import {RabinKarp} from './rabin-karp';
import {IClone, ICloneValidator, IOptions, IStore} from './interfaces';
import {LinesLengthCloneValidator} from './validators';
import EventEmitter = require('eventemitter3');


export type DetectorEvents = 'CLONE_FOUND' | 'CLONE_SKIPPED' | 'START_DETECTION';


export class Detector extends EventEmitter<DetectorEvents> {

	private algorithm: RabinKarp;

	constructor(
		private readonly options: IOptions,
		private readonly store: IStore<any>,
		private cloneValidators: ICloneValidator[] = [],
	) {
		super();
		this.initCloneValidators();
		this.algorithm = new RabinKarp(this.options, this, this.cloneValidators);
	}

	public async detect(id: string, text: string, format: string): Promise<IClone[]> {
		const tokenMaps: TokensMap[] = createTokenMapBasedOnCode(id, text, format, this.options);
		// TODO change stores implementation
		this.store.namespace(format);

		const detect = async (tokenMap: TokensMap, clones: IClone[]): Promise<IClone[]> => {
			this.emit('START_DETECTION', {source: tokenMap});
			return this.algorithm
				.run(tokenMap, this.store)
				.then((clns: IClone[]) => {
					clones.push(...clns);
					const nextTokenMap = tokenMaps.pop();
					if (nextTokenMap) {
						return detect(nextTokenMap, clones);
					} else {
						return clones;
					}
				});
		}
		return detect(tokenMaps.pop(), []);
	}

	private initCloneValidators() {
		if (this.options.minLines || this.options.maxLines) {
			this.cloneValidators.push(new LinesLengthCloneValidator())
		}
	}
}
