import EventEmitter = require('eventemitter3');

export const CLONE_FOUND_EVENT = 'clone';
export const MATCH_SOURCE_EVENT = 'match';
export const SOURCE_SKIPPED_EVENT = 'skip';
export const END_EVENT = 'end';

export class JscpdEventEmitter extends EventEmitter {}
