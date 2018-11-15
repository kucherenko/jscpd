import EventEmitter = require('eventemitter3');
import { IClone } from './clone.interface';
import { IStatistic } from './statistic.interface';

export interface IReporter {
  attach(eventEmitter: EventEmitter): void;
  report(clones?: IClone[], statistic?: IStatistic): void;
}
