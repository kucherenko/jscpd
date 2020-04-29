import {DetectorEvents, IClone, IValidationResult} from '..';
import {TokensMap} from '@jscpd/tokenizer';

export interface ISubscriber {
	subscribe(): Partial<Record<DetectorEvents, IHandler>>;
}

export interface IHandler {
	(payload: IEventPayload): void;
}

export interface IEventPayload {
	clone?: IClone;
	source?: TokensMap;
	validation?: IValidationResult;
}
