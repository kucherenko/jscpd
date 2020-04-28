import {DetectorEvents, IClone, IValidationResult} from '@jscpd/core';
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
