import {IBlamedLines, ITokenLocation} from '..';

export interface IClone {
	format: string;
	isNew?: boolean;
	foundDate?: number;
	duplicationA: {
		sourceId: string;
		start: ITokenLocation;
		end: ITokenLocation;
		range: [number, number];
		fragment?: string;
		blame?: IBlamedLines;
	};
	duplicationB: {
		sourceId: string;
		start: ITokenLocation;
		end: ITokenLocation;
		range: [number, number];
		fragment?: string;
		blame?: IBlamedLines;
	};
}
