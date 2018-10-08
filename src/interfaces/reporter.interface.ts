import EventEmitter = NodeJS.EventEmitter;

export interface IReporter {
  attach(eventEmitter: EventEmitter): void;
}
