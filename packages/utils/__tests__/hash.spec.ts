import * as crypto from 'crypto';
import {hash} from '../src';
import Mock = jest.Mock;

jest.mock('crypto');

describe('Hash', () => {
	it('should create hash based on string', () => {
		const digest = jest.fn();
		const update = jest.fn().mockReturnValue({
			digest,
		});
		(crypto.createHash as Mock).mockReturnValue({
			update,
		})
		hash('string');
		expect(crypto.createHash).toHaveBeenCalledWith('md5');
		expect(update).toHaveBeenCalledWith('string');
		expect(digest).toHaveBeenCalledWith('hex');
	});
});
