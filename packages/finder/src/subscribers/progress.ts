import {DetectorEvents, IEventPayload, IHandler, IOptions, ISubscriber} from '@jscpd/core';
import {cloneFound} from '../utils/clone-found';

export class ProgressSubscriber implements ISubscriber {

	constructor(private readonly options: IOptions) {
	}

	subscribe(): Partial<Record<DetectorEvents, IHandler>> {
		return {
			CLONE_FOUND: (payload: IEventPayload) => cloneFound(payload.clone, this.options),
		};
	}
}
