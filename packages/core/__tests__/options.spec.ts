import {getDefaultOptions, getOption} from '../src/options';

describe('Options', () => {
	describe('get option', () => {
		it('should get option from options by name', () => {
			const options = {
				path: ['path'],
			}
			expect(getOption('path', options)).toEqual(['path']);
		});

		it('should return default value if it is not provided', () => {
			const defaults = getDefaultOptions();
			expect(getOption('reporters', {})).toEqual(defaults['reporters']);
		});

		it('should return default options if options is not provided', () => {
			const defaults = getDefaultOptions();
			expect(getOption('listeners')).toEqual(defaults['listeners'])
		});
	});
});
