import {expect} from 'chai';
import {IClone} from '@jscpd/core';
import {jscpd} from '../src';

const pathToFixtures = __dirname + '/../../../fixtures/modes';
describe('jscpd modes', () => {

	let _log;

	beforeEach(() => {
		_log = console.log;
		console.log = () => {
		};
	})

	afterEach(() => {
		console.log = _log;
	})

	describe('strict mode', () => {
		it('should detect clones with strict mode, ignore just symbols marked for ignore', async () => {
			const clones: IClone[] = await jscpd(['', '', pathToFixtures, '-m', 'strict']);
			const clone: IClone = clones[0];
			expect(clone.duplicationA.start.line).to.equal(17);
			expect(clone.duplicationA.end.line).to.equal(93);
			expect(clone.duplicationB.start.line).to.equal(11);
			expect(clone.duplicationB.end.line).to.equal(87);
		})
	});

	describe('weak mode', () => {
		it('should detect clones with weak mode, ignore new line and empty symbols and comments', async () => {
			const clones: IClone[] = await jscpd(['', '', pathToFixtures, '-m', 'weak']);
			const clone: IClone = clones[0];
			expect(clone.duplicationA.start.line).to.equal(9);
			expect(clone.duplicationA.end.line).to.equal(92);
			expect(clone.duplicationB.start.line).to.equal(9);
			expect(clone.duplicationB.end.line).to.equal(86);
		})
	});

	describe('not exist mode', () => {
		it('should not run ', async () => {
			try {
				await jscpd(['', '', pathToFixtures, '-m', 'zzz']);
			} catch (e) {
				expect(e.message).to.equal(`Mode zzz does not supported yet.`);
			}
		})
	});
});
