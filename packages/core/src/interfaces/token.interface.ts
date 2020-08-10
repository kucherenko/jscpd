import {ITokenLocation} from '.';

export interface IToken {
	type: string;
	value: string;
	length: number;
	format: string;
	range: [number, number];
	loc?: {
		start: ITokenLocation;
		end: ITokenLocation;
	};
}
