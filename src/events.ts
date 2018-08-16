import { EventEmitter } from 'events';

export const CLONE_EVENT = 'clone';
export const MATCH_FILE_EVENT = 'match';
export const END_EVENT = 'end';
export const END_PROCESS_EVENT = 'end-process';
export const ERROR_EVENT = 'error';

class JscpdEvents extends EventEmitter {}

export const Events = new JscpdEvents();
