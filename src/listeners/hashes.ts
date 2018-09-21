import { HASH_EVENT } from '../events';
import { IListener } from '../interfaces/listener.interface';
import { IMapFrame } from '../interfaces/map-frame.interface';
import { IStore } from '../interfaces/store/store.interface';
import { JSCPD } from '../jscpd';
import { getSourcesHashDbName } from '../stores/models';
import { StoresManager } from '../stores/stores-manager';

export class HashesListener implements IListener {
  public attach(): void {
    JSCPD.getEventsEmitter().on(HASH_EVENT, this.bindHashWithSource.bind(this));
  }

  private bindHashWithSource(mapFrame: IMapFrame) {
    const { id, sourceId, format } = mapFrame;
    const sourcesHashesStore: IStore<string[]> = StoresManager.getStore(getSourcesHashDbName(format));
    const sourcesHashes: string[] = sourcesHashesStore.get(sourceId) || [];
    sourcesHashesStore.set(sourceId, [...new Set([...sourcesHashes, id])]);
  }
}
