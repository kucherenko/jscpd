import EventEmitter = require('eventemitter3');
import { IClone } from './clone.interface';

export interface IReporter {
  attach(eventEmitter: EventEmitter): void;
  report(clones: IClone[]): void;
}
