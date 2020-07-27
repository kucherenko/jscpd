import {RabinKarp} from './rabin-karp';
import {IClone, ICloneValidator, IMapFrame, IOptions, IStore, ITokenizer, ITokensMap} from './interfaces';
import {LinesLengthCloneValidator} from './validators';
import {mild} from './mode';
// TODO replace to own event emitter
import * as EventEmitter from 'eventemitter3';

export type DetectorEvents = 'CLONE_FOUND' | 'CLONE_SKIPPED' | 'START_DETECTION';

export class Detector extends EventEmitter<DetectorEvents> {

  private algorithm: RabinKarp;

  constructor(
    private readonly tokenizer: ITokenizer,
    private readonly store: IStore<IMapFrame>,
    private readonly cloneValidators: ICloneValidator[] = [],
    private readonly options: IOptions) {
    super();
    this.initCloneValidators();
    this.algorithm = new RabinKarp(this.options, this, this.cloneValidators);
    this.options.minTokens = this.options.minTokens || 50;
    this.options.maxLines = this.options.maxLines || 500;
    this.options.minLines = this.options.minLines || 5;
    this.options.mode = this.options.mode || mild;
  }

  public async detect(id: string, text: string, format: string): Promise<IClone[]> {
    const tokenMaps: ITokensMap[] = this.tokenizer.generateMaps(id, text, format, this.options);
    // TODO change stores implementation
    this.store.namespace(format);

    const detect = async (tokenMap: ITokensMap, clones: IClone[]): Promise<IClone[]> => {
      if (tokenMap) {
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
    }
    return detect(tokenMaps.pop(), []);
  }

  private initCloneValidators(): void {
    if (this.options.minLines || this.options.maxLines) {
      this.cloneValidators.push(new LinesLengthCloneValidator())
    }
  }
}
