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
  addCloneToSource(cloneId, [clone.duplicationA.sourceId, clone.duplicationB.sourceId]);
  Events.emit(CLONE_EVENT, { ...clone, is_new: true });
}

function addCloneToSource(cloneId: string, sourcesIds: string[]) {
  const sourcesStore: IStore<ISource> = StoresManager.getStore(SOURCES_DB);
  sourcesIds.map(sid => {
    const source: ISource = sourcesStore.get(sid);
    if (source && source.meta && source.meta.clones) {
      source.meta.clones = [...new Set(source.meta.clones.concat(cloneId))];
      source.meta.last_update_date = (new Date()).getTime();
      sourcesStore.set(sid, source);
    }
  });
}

export function createClone(startMap: IMapFrame, endMap: IMapFrame, format: string): IClone {
  const hashesStore: IStore<IMapFrame> = StoresManager.getStore(getHashDbName(format));
  const sourceStart: IMapFrame = hashesStore.get(startMap.id);
  const sourceEnd: IMapFrame = hashesStore.get(endMap.id);

  return {
    format,
    found_date: (new Date()).getTime(),
    duplicationA: {
      sourceId: startMap.start.sourceId,
      start: startMap.start.loc.start,
      end: endMap.end.loc.end,
      range: [startMap.start.range[0], endMap.end.range[1]],
      fragment: getFragment(startMap.start.sourceId, startMap.start.range[0], endMap.end.range[1])
    },
    duplicationB: {
      sourceId: sourceStart.start.sourceId,
      start: sourceStart.start.loc.start,
      end: sourceEnd.end.loc.end,
      range: [sourceStart.start.range[0], sourceEnd.end.range[1]],
      fragment: getFragment(sourceStart.start.sourceId, sourceStart.start.range[0], sourceEnd.end.range[1])
    }
  };
}

export function getFragment(sourceId: string, start: number, end: number): string {
  const sourcesStore: IStore<ISource> = StoresManager.getStore(SOURCES_DB);
  return sourcesStore.get(sourceId).source.substring(start, end);
}

export function isCloneLinesBiggerLimit(clone: IClone, minLines: number): boolean {
  return clone.duplicationA.end.line - clone.duplicationA.start.line >= minLines;
}
