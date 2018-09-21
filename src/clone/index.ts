import { IClone } from '../interfaces/clone.interface';
import { IMapFrame } from '../interfaces/map-frame.interface';
import { ISource } from '../interfaces/source.interface';
import { IStore } from '../interfaces/store/store.interface';
import { getHashDbName, SOURCES_DB } from '../stores/models';
import { StoresManager } from '../stores/stores-manager';
import { md5 } from '../utils';

export function generateCloneId(clone: IClone): string {
  return md5(JSON.stringify(clone));
}
export function createClone(startMap: IMapFrame, endMap: IMapFrame): IClone {
  const { format } = startMap;
  const hashesStore: IStore<IMapFrame> = StoresManager.getStore(getHashDbName(format));
  const sourceStart: IMapFrame = hashesStore.get(startMap.id);
  const sourceEnd: IMapFrame = hashesStore.get(endMap.id);

  return {
    format,
    foundDate: new Date().getTime(),
    duplicationA: {
      sourceId: startMap.sourceId,
      start: startMap.start.loc.start,
      end: endMap.end.loc.end,
      range: [startMap.start.range[0], endMap.end.range[1]],
      fragment: getFragment(startMap.sourceId, startMap.start.range[0], endMap.end.range[1])
    },
    duplicationB: {
      sourceId: sourceStart.sourceId,
      start: sourceStart.start.loc.start,
      end: sourceEnd.end.loc.end,
      range: [sourceStart.start.range[0], sourceEnd.end.range[1]],
      fragment: getFragment(sourceStart.sourceId, sourceStart.start.range[0], sourceEnd.end.range[1])
    }
  };
}

export function getFragment(sourceId: string, start: number, end: number): string {
  const sourcesStore: IStore<ISource> = StoresManager.getStore(SOURCES_DB);
  return sourcesStore.get(sourceId).source.substring(start, end);
}

export function getSourceFragment(source: ISource, start: number, end: number): string {
  return source.source.substring(start, end);
}

export function getSourceFragmentLength(source: ISource, start: number, end: number): number {
  return getSourceFragment(source, start, end).split('\n').length;
}

export function isCloneLinesBiggerLimit(clone: IClone, minLines: number): boolean {
  return clone.duplicationA.end.line - clone.duplicationA.start.line >= minLines;
}
