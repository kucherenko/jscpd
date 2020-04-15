import {IClone} from '.';
import {DetectorEvents} from '..';

export interface ReporterHandler {
	(clone: IClone): void;
}

export interface IReporter {

	subscribe(): Partial<Record<DetectorEvents, ReporterHandler>>;

	report(clones: IClone[]): void;
}
