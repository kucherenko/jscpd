import {IToken} from './token.interface';

export interface IMapFrame {
	id: string;
	sourceId: string;
	start: IToken;
	end: IToken;
	isClone?: boolean;
	localDuplicate?: boolean;
}
