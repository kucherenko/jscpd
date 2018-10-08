import { generateCloneId } from '../clone';
import { CLONE_EVENT, END_GLOB_STREAM_EVENT, FINISH_EVENT, JscpdEventEmitter } from '../events';
import { IClone } from '../interfaces/clone.interface';
import { IListener } from '../interfaces/listener.interface';
import { IStore } from '../interfaces/store/store.interface';
import { CLONES_DB } from '../stores/models';
import { StoresManager } from '../stores/stores-manager';

export class ClonesListener implements IListener {
  public attach(eventEmitter: JscpdEventEmitter): void {
    eventEmitter.on(CLONE_EVENT, this.matchClone.bind(this));
    eventEmitter.on(END_GLOB_STREAM_EVENT, () => eventEmitter.emit(FINISH_EVENT));
  }

  private matchClone(clone: IClone) {
    const clonesStore: IStore<IClone> = StoresManager.getStore(CLONES_DB);
    const cloneId: string = generateCloneId(clone);
    clonesStore.set(cloneId, clone);
  }
}
