import { CLONE_EVENT, Events } from '../events';
import { IClone } from '../interfaces/clone.interface';
import { IMapFrame } from '../interfaces/map-frame.interface';
import { ISource } from '../interfaces/source.interface';
import { IStore } from '../interfaces/store/store.interface';
import { CLONES_DB, getHashDbName, SOURCES_DB } from '../stores/models';
import { StoresManager } from '../stores/stores-manager';
import { md5 } from '../utils';

export function addClone(clone: IClone) {
  const clonesStore: IStore<IClone> = StoresManager.getStore(CLONES_DB);
  const cloneId: string = md5(JSON.stringify(clone));
  clonesStore.set(cloneId, clone);
  addCloneToSource(cloneId, [
    clone.duplicationA.sourceId,
    clone.duplicationB.sourceId
  ]);
  Events.emit(CLONE_EVENT, { ...clone, is_new: true });
}

function addCloneToSource(cloneId: string, sourcesIds: string[]) {
  const sourcesStore: IStore<ISource> = StoresManager.getStore(SOURCES_DB);
  sourcesIds.map(sid => {
    const source: ISource = sourcesStore.get(sid);
    if (source && source.clones) {
      if (!source.clones.includes(cloneId)) {
        source.clones = source.clones.concat(cloneId);
        sourcesStore.set(sid, source);
      }
    }
  });
}

export function createClone(
  startMap: IMapFrame,
  endMap: IMapFrame,
  format: string
): IClone {
  const hashesStore: IStore<IMapFrame> = StoresManager.getStore(
    getHashDbName(format)
  );

  const sourceStart: IMapFrame = hashesStore.get(startMap.id);
  const sourceEnd: IMapFrame = hashesStore.get(endMap.id);

  const fragment: string = getFragment(
    startMap.start.sourceId,
    startMap.start.range[0],
    endMap.end.range[1]
  );

  const clone: IClone = {
    format,
    fragment,
    duplicationA: {
      sourceId: startMap.start.sourceId,
      start: startMap.start,
      end: endMap.end
    },
    duplicationB: {
      sourceId: sourceStart.start.sourceId,
      start: sourceStart.start,
      end: sourceEnd.end
    }
  };
  return clone;
}

export function getFragment(
  sourceId: string,
  start: number,
  end: number
): string {
  const sourcesStore: IStore<ISource> = StoresManager.getStore(SOURCES_DB);
  return sourcesStore.get(sourceId).source.substring(start, end);
}

export function isCloneLinesBiggerLimit(
  clone: IClone,
  minLines: number
): boolean {
  return (
    clone.duplicationA.end.loc.end.line -
      clone.duplicationA.start.loc.end.line >=
    minLines
  );
}
