import { createClone, isCloneLinesBiggerLimit } from './clone';
import { CLONE_EVENT } from './events';
import { IClone } from './interfaces/clone.interface';
import { IMapFrame } from './interfaces/map-frame.interface';
import { IOptions } from './interfaces/options.interface';
import { IStore } from './interfaces/store/store.interface';
import { getHashDbName } from './stores/models';
import { StoresManager } from './stores/stores-manager';
import { TokensMap } from './tokenizer/token-map';
import EventEmitter = NodeJS.EventEmitter;

export class Detector {
  constructor(private options: IOptions, private eventEmitter: EventEmitter) {}

  public detectByMap(tokenMap: TokensMap): IClone[] {
    const clones: IClone[] = [];
    if (tokenMap.getLength() >= this.options.minTokens) {
      let isClone: boolean = false;
      let start: IMapFrame | undefined;
      let end: IMapFrame | undefined;

      const HashesStore: IStore<IMapFrame> = StoresManager.getStore(getHashDbName(tokenMap.getFormat()));

      for (const mapFrame of tokenMap) {
        if (HashesStore.has(mapFrame.id)) {
          isClone = true;
          if (!start) {
            start = end = mapFrame;
          } else {
            end = mapFrame;
          }
        } else {
          if (isClone && start && end) {
            const clone: IClone = createClone(start, end);
            if (isCloneLinesBiggerLimit(clone, this.options.minLines)) {
              clones.push(clone);
              this.eventEmitter.emit(CLONE_EVENT, clone);
            }
          }
          isClone = false;
          start = undefined;
          HashesStore.set(mapFrame.id, mapFrame);
        }
      }

      if (isClone && start && end) {
        const clone: IClone = createClone(start, end);
        if (isCloneLinesBiggerLimit(clone, this.options.minLines)) {
          clones.push(clone);
          this.eventEmitter.emit(CLONE_EVENT, clone);
        }
      }
    }
    return clones;
  }
}
