import { IListener } from '../interfaces/listener.interface';
import { IOptions } from '../interfaces/options.interface';
import { StateListener } from './state';
import { StatisticListener } from './statistic';

const EXISTING_LISTENERS: {
  [key: string]: new (options: IOptions) => IListener;
} = {
  statistic: StatisticListener,
  state: StateListener
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
