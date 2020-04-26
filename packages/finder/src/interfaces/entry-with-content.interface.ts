import {Entry} from 'fast-glob';

export interface EntryWithContent extends Entry {
	content: string;
}
