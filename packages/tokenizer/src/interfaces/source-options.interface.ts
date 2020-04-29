export interface ISourceOptions {
	id: string;
	format: string;
	source?: string;
	isNew?: boolean;
	detectionDate?: number;
	lastUpdateDate?: number;
	lines?: number;
	range?: number[];
}
