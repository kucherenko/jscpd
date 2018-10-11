import { HASH_EVENT, JscpdEventEmitter } from '../events';
import { IListener } from '../interfaces/listener.interface';
import { IMapFrame } from '../interfaces/map-frame.interface';
import { IStore } from '../interfaces/store/store.interface';
import { getSourcesHashDbName } from '../stores/models';
import { StoresManager } from '../stores/stores-manager';
import { timerStart, timerStop } from '../utils/timer';

export class HashesListener implements IListener {
  public attach(eventEmitter: JscpdEventEmitter): void {
    eventEmitter.on(HASH_EVENT, this.bindHashWithSource.bind(this));
  }

  private bindHashWithSource(mapFrame: IMapFrame) {
    timerStart(this.constructor.name);
    const { id, sourceId, format } = mapFrame;
    const sourcesHashesStore: IStore<string[]> = StoresManager.getStore(getSourcesHashDbName(format));
    const sourcesHashes: string[] = sourcesHashesStore.get(sourceId) || [];
    sourcesHashesStore.set(sourceId, [...new Set([...sourcesHashes, id])]);
    timerStop(this.constructor.name);
  }
}
