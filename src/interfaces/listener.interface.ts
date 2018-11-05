import EventEmitter = require('eventemitter3');

export interface IListener {
  attach(eventEmitter: EventEmitter): void;
}
