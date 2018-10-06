import { generateCloneId } from '../clone';
import { CLONE_EVENT, END_GLOB_STREAM_EVENT, FINISH_EVENT } from '../events';
import { IBlamedLines } from '../interfaces/blame.interface';
import { IClone } from '../interfaces/clone.interface';
import { IListener } from '../interfaces/listener.interface';
import { ISource } from '../interfaces/source.interface';
import { IStore } from '../interfaces/store/store.interface';
import { JSCPD } from '../jscpd';
import { CLONES_DB, SOURCES_DB } from '../stores/models';
import { StoresManager } from '../stores/stores-manager';

const Blamer = require('blamer');

export class BlamerListener implements IListener {

  private static getBlamedLines(blamedFiles: { [key: string]: IBlamedLines }, start: number, end: number): IBlamedLines {
    const [file] = Object.keys(blamedFiles);
    const result: IBlamedLines = {};
    Object
      .keys(blamedFiles[file])
      .filter((lineNumber) => {
        return Number(lineNumber) >= start && Number(lineNumber) <= end;
      })
      .map((lineNumber) => blamedFiles[file][lineNumber])
      .forEach((info) => {
        result[info.line] = info;
      });
    return result;
  }

  private promises: Array<Promise<IClone>> = [];

  public attach(): void {
    JSCPD.on(CLONE_EVENT, this.matchClone.bind(this));
    JSCPD.on(END_GLOB_STREAM_EVENT, () => {
      Promise.all(this.promises).then(() => JSCPD.emit(FINISH_EVENT));
    });
  }

  private matchClone(clone: IClone) {
    const clonesStore: IStore<IClone> = StoresManager.getStore(CLONES_DB);
    const sourcesStore: IStore<ISource> = StoresManager.getStore(SOURCES_DB);
    const cloneId: string = generateCloneId(clone);

    const blamer = new Blamer();

    const blameFileA = blamer.blameByFile(sourcesStore.get(clone.duplicationA.sourceId).id);
    const blameFileB = blamer.blameByFile(sourcesStore.get(clone.duplicationB.sourceId).id);
    this.promises.push(
      Promise
        .all([blameFileA, blameFileB])
        .then(([blamedFileA, blamedFileB]) => {
          const cloneBlamed: IClone = {
            ...clone,
            duplicationA: {
              ...clone.duplicationA, blame: BlamerListener.getBlamedLines(
                blamedFileA,
                clone.duplicationA.start.line,
                clone.duplicationA.end.line
              )
            },
            duplicationB: {
              ...clone.duplicationB, blame: BlamerListener.getBlamedLines(
                blamedFileB,
                clone.duplicationB.start.line,
                clone.duplicationB.end.line
              )
            }
          };
          clonesStore.set(cloneId, cloneBlamed);
          return cloneBlamed;
        })
    );
  }
}
