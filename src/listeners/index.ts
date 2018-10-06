import { IListener } from '../interfaces/listener.interface';
import { IOptions } from '../interfaces/options.interface';
import { BlamerListener } from './blamer';
import { ClonesListener } from './clones';
import { HashesListener } from './hashes';
import { SourcesListener } from './sources';
import { StateListener } from './state';
import { StatisticListener } from './statistic';

const EXISTING_LISTENERS: {
  [key: string]: new (options: IOptions) => IListener;
} = {
  statistic: StatisticListener,
  state: StateListener,
  sources: SourcesListener,
  clones: ClonesListener,
  hashes: HashesListener,
  blamer: BlamerListener
};

const LISTENERS: { [key: string]: IListener } = {};

export function registerListener(name: string, reporter: IListener): void {
  LISTENERS[name] = reporter;
}

export function getRegisteredListeners(): { [key: string]: IListener } {
  return LISTENERS;
}

export function registerListenerByName(options: IOptions) {
  const { listeners = [] } = options;
  listeners.forEach(listener => registerListener(listener, new EXISTING_LISTENERS[listener](options)));
}
