import { IClone } from '..';
import { IMapFrame } from '../interfaces/map-frame.interface';
import { ISourceOptions } from '../interfaces/source-options.interface';
import { IStore } from '../interfaces/store/store.interface';
import { getHashDbName } from '../stores/models';
import { StoresManager } from '../stores/stores-manager';
import { sourceToString } from '../utils/source';

export async function createClone(startMap: IMapFrame, endMap: IMapFrame): Promise<IClone> {
  const { format } = startMap;
  const hashesStore: IStore<IMapFrame> = StoresManager.getStore(getHashDbName(format)) as IStore<IMapFrame>;
  const sourceStart: IMapFrame = await hashesStore.get(startMap.id);
  const sourceEnd: IMapFrame = await hashesStore.get(endMap.id);

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

export function getFragment(id: string, start: number, end: number): string {
  return sourceToString({ id } as ISourceOptions).substring(start, end);
}

export function getSourceFragment(source: ISourceOptions, start: number, end: number): string {
  return sourceToString(source).substring(start, end);
}

export function getSourceFragmentLength(source: ISourceOptions, start: number, end: number): number {
  return getSourceFragment(source, start, end).split('\n').length;
}

export function isCloneLinesBiggerLimit(clone: IClone, minLines: number): boolean {
  return clone.duplicationA.end.line - clone.duplicationA.start.line >= minLines;
}
