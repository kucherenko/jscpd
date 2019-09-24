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

let newHashes: Array<Promise<any>> = [];

export class Detector {
  constructor(private options: IOptions, private eventEmitter: EventEmitter) {}

  public async detectByMap(tokenMap: TokensMap): Promise<IClone[]> {
    let clones: IClone[] = [];
    await Promise.all(newHashes);
    newHashes = [];
    if (tokenMap.getLength() >= getOption('minTokens', this.options)) {
      const HashesStore: IStore<IMapFrame> = StoresManager.getStore(getHashDbName(tokenMap.getFormat())) as IStore<
        IMapFrame
      >;

      const tokenMaps: IMapFrame[] = [];

      const initialMapFramesArray: IMapFrame[][] = [];
      for (const mapFrame of tokenMap) {
        const localDuplicate = tokenMaps.map(fr => fr.id).includes(mapFrame.id);
        if (localDuplicate) {
          if (tokenMaps[tokenMaps.length - 1].localDuplicate) {
            initialMapFramesArray[initialMapFramesArray.length - 1].push({ ...mapFrame, isClone: true });
          } else {
            initialMapFramesArray.push([{ ...mapFrame, isClone: true }]);
          }
        }
        tokenMaps.push({ ...mapFrame, localDuplicate });
      }

      const tokensStatuses: boolean[] = await HashesStore.hasKeys(tokenMaps.map(fr => fr.id));
      clones.push(
        ...(await Promise.all(
          tokenMaps
            .reduce((mapFramesArray: IMapFrame[][], frame: IMapFrame, index: number) => {
              if (tokensStatuses[index]) {
                if (!tokensStatuses[index - 1]) {
                  mapFramesArray.push([{ ...frame, isClone: true }]);
                } else {
                  mapFramesArray[mapFramesArray.length - 1].push({ ...frame, isClone: true });
                }
              } else if (!frame.localDuplicate) {
                newHashes.push(HashesStore.set(frame.id, frame));
              }
              return mapFramesArray;
            }, initialMapFramesArray)
            .map(
              (mapFrames: IMapFrame[]): Promise<IClone> => createClone(mapFrames[0], mapFrames[mapFrames.length - 1])
            )
        ))
      );
      clones = clones.filter((clone: IClone): boolean => {
        const isAcceptableClone: boolean = isCloneLinesBiggerLimit(clone, getOption('minLines', this.options));
        if (isAcceptableClone) {
          this.eventEmitter.emit(CLONE_FOUND_EVENT, clone);
        }
        return isAcceptableClone;
      });
    }
    return clones;
  }
}
