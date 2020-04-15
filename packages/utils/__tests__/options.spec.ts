import {parseFormatsExtensions} from '../src';

describe('Options', () => {

	it('should parse format extensions string to object', () => {
		expect(parseFormatsExtensions('format:zz,xx;format2:aa,bb')).toEqual({
			format: ['zz', 'xx'],
			format2: ['aa', 'bb'],
		})
	});
});
