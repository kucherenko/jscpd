import EventEmitter = require('eventemitter3');
import { createClone, isCloneLinesBiggerLimit } from './clone';
import { CLONE_FOUND_EVENT } from './events';
import { IClone } from './interfaces/clone.interface';
import { IMapFrame } from './interfaces/map-frame.interface';
import { IOptions } from './interfaces/options.interface';
import { IStore } from './interfaces/store/store.interface';
import { getHashDbName } from './stores/models';
import { StoresManager } from './stores/stores-manager';
import { TokensMap } from './tokenizer/token-map';
import { getOption } from './utils/options';

export class Detector {
  constructor(private options: IOptions, private eventEmitter: EventEmitter) {
  }

  public detectByMap(tokenMap: TokensMap): IClone[] {
    const clones: IClone[] = [];
    if (tokenMap.getLength() >= getOption('minTokens', this.options)) {
      let isClone: boolean = false;
      let start: IMapFrame | undefined;
      let end: IMapFrame | undefined;

      const HashesStore: IStore<IMapFrame> =
        StoresManager.getStore(getHashDbName(tokenMap.getFormat())) as IStore<IMapFrame>;

      for (const mapFrame of tokenMap) {
        if (HashesStore.has(mapFrame.id)) {
          isClone = true;
          if (!start) {
            start = end = mapFrame;
          } else {
            end = mapFrame;
          }
        } else {
          this._cloneFound(isClone, start, end, clones);
          isClone = false;
          start = undefined;
          HashesStore.set(mapFrame.id, mapFrame);
        }
      }
      this._cloneFound(isClone, start, end, clones);
    }
    return clones;
  }

  private _cloneFound(isClone: boolean, start: IMapFrame | undefined, end: IMapFrame | undefined, clones: IClone[]) {
    if (isClone && start && end) {
      const clone: IClone = createClone(start, end);
      if (isCloneLinesBiggerLimit(clone, getOption('minLines', this.options))) {
        clones.push(clone);
        this.eventEmitter.emit(CLONE_FOUND_EVENT, clone);
      }
    }
  }
}
