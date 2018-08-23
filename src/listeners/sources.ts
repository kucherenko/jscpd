import { Events, MATCH_SOURCE_EVENT, REMOVE_SOURCES_ARTIFACTS_EVENT } from '../events';
import { IClone } from '../interfaces/clone.interface';
import { IListener } from '../interfaces/listener.interface';
import { IMapFrame } from '../interfaces/map-frame.interface';
import { ISource } from '../interfaces/source.interface';
import { IStore } from '../interfaces/store/store.interface';
import { CLONES_DB, getHashDbName, getSourcesHashDbName, SOURCES_CLONES_DB, SOURCES_DB } from '../stores/models';
import { StoresManager } from '../stores/stores-manager';
import { generateSourceId } from '../utils';

export class SourcesListener implements IListener {
  public attach(): void {
    Events.on(MATCH_SOURCE_EVENT, this.matchSource.bind(this));
    Events.on(REMOVE_SOURCES_ARTIFACTS_EVENT, this.removeSourcesArtifacts.bind(this));
  }

  private matchSource(source: ISource) {
    const sourceId: string = generateSourceId(source);
    const sourcesStore: IStore<ISource> = StoresManager.getStore(SOURCES_DB);
    sourcesStore.set(sourceId, source);
  }

  private removeSourcesArtifacts(source: ISource) {
    const sourceId: string = generateSourceId(source);
    const clonesStore: IStore<IClone> = StoresManager.getStore(CLONES_DB);
    const hashesStore: IStore<IMapFrame> = StoresManager.getStore(getHashDbName(source.format));
    const sourcesClonesStore: IStore<string[]> = StoresManager.getStore(SOURCES_CLONES_DB);
    const sourcesHashesStore: IStore<string[]> = StoresManager.getStore(getSourcesHashDbName(source.format));

    (sourcesClonesStore.get(sourceId) || []).map((cloneId: string) => {
      clonesStore.delete(cloneId);
    });

    (sourcesHashesStore.get(sourceId) || []).map((hash: string) => {
      hashesStore.delete(hash);
    });
  }
}
