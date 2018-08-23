import { EventEmitter } from 'events';

export const INITIALIZE_EVENT = 'initialize';
export const CLONE_EVENT = 'clone';
export const HASH_EVENT = 'hash';
export const MATCH_SOURCE_EVENT = 'match';
export const REMOVE_SOURCES_ARTIFACTS_EVENT = 'remove-sources-artifacts';
export const END_EVENT = 'end';
export const END_PROCESS_EVENT = 'end-process';
export const ERROR_EVENT = 'error';

class JscpdEvents extends EventEmitter {}

export const Events = new JscpdEvents();
