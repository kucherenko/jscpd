import { IOptions } from '..';
import { END_PROCESS_EVENT, INITIALIZE_EVENT, JscpdEventEmitter } from '../events';
import { IListener } from '../interfaces/listener.interface';
import { StoresManager } from '../stores/stores-manager';
import { timerStart, timerStop } from '../utils/timer';
export class StateListener implements IListener {
  constructor(private options: IOptions) {}

  public attach(eventEmitter: JscpdEventEmitter): void {
    eventEmitter.on(INITIALIZE_EVENT, this.initialize.bind(this));
    eventEmitter.on(END_PROCESS_EVENT, this.endProcess.bind(this));
  }

  private initialize() {
    timerStart(this.constructor.name + '::initialize');
    StoresManager.initialize(this.options.storeOptions);
    StoresManager.flush();
    timerStop(this.constructor.name + '::initialize');
  }

  private endProcess() {
    timerStart(this.constructor.name + '::endProcess');
    StoresManager.close();
    timerStop(this.constructor.name + '::endProcess');
  }
}
