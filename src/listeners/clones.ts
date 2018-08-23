import { generateCloneId } from '../clone';
import { CLONE_EVENT, Events } from '../events';
import { IClone } from '../interfaces/clone.interface';
import { IListener } from '../interfaces/listener.interface';
import { IStore } from '../interfaces/store/store.interface';
import { CLONES_DB, SOURCES_CLONES_DB } from '../stores/models';
import { StoresManager } from '../stores/stores-manager';

export class ClonesListener implements IListener {
  public attach(): void {
    Events.on(CLONE_EVENT, this.matchClone.bind(this));
  }

  private matchClone(clone: IClone) {
    const clonesStore: IStore<IClone> = StoresManager.getStore(CLONES_DB);
    const cloneId: string = generateCloneId(clone);
    clonesStore.set(cloneId, clone);
    this.addCloneToSource(cloneId, [clone.duplicationA.sourceId, clone.duplicationB.sourceId]);
  }

  private addCloneToSource(cloneId: string, sourcesIds: string[]) {
    const sourcesClonesStore: IStore<string[]> = StoresManager.getStore(SOURCES_CLONES_DB);
    sourcesIds.map(sourceId => {
      const clonesIds: string[] = sourcesClonesStore.get(sourceId) || [];
      sourcesClonesStore.set(sourceId, [...new Set([...clonesIds, cloneId])]);
    });
  }
}
