import EventEmitter = NodeJS.EventEmitter;

export interface IListener {
  attach(eventEmitter: EventEmitter): void;
}
