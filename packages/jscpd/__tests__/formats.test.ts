import {jscpd} from '../src';
import {IClone} from '@jscpd/core';
import {expect} from 'chai';

const pathToFixtures = __dirname + '/../../../fixtures';

describe('jscpd formats', () => {

	let _log;

	beforeEach(() => {
		_log = console.log;
		console.log = () => {
		};
	})

	afterEach(() => {
		console.log = _log;
	})


	describe('Ignore Blocks', () => {
		it('should not skip blocks marked as ignored', async () => {
			const clones: IClone[] = await jscpd(['', '', pathToFixtures + '/ignore', '--silent']);
			expect(clones.length).to.equal(0);
		});
	});

	const formats: Record<string, { name: string, folder: string, clonesCount: number, descr?: string }[]> = {
		'C-like': [
			{
				name: 'java',
				clonesCount: 2,
				folder: pathToFixtures + '/clike',
			},
			{
				name: 'cpp',
				clonesCount: 2,
				folder: pathToFixtures + '/clike',
			},
			{
				name: 'c-header',
				clonesCount: 2,
				folder: pathToFixtures + '/clike',
			},
			{
				name: 'cpp-header',
				clonesCount: 2,
				folder: pathToFixtures + '/clike',
			},
			{
				name: 'java',
				clonesCount: 2,
				folder: pathToFixtures + '/clike',
			},
			{
				name: 'cpp',
				clonesCount: 2,
				folder: pathToFixtures + '/clike',
			},
			{
				name: 'objectivec',
				clonesCount: 1,
				folder: pathToFixtures + '/objective-c',
			},
			{
				name: 'c',
				clonesCount: 2,
				folder: pathToFixtures + '/clike',
			},
			{
				name: 'csharp',
				clonesCount: 2,
				folder: pathToFixtures + '/clike',
			},
			{
				name: 'scala',
				clonesCount: 1,
				folder: pathToFixtures + '/clike',
			},

		],
		'Scripts': [
			{
				name: 'javascript',
				clonesCount: 5,
				folder: pathToFixtures + '/javascript',
			},
			{
				name: 'typescript',
				clonesCount: 2,
				folder: pathToFixtures + '/javascript',
			},
			{
				name: 'coffeescript',
				clonesCount: 2,
				folder: pathToFixtures + '/coffeescript',
			},

		],
		'Markup': [
			{
				name: 'markup',
				descr: 'HTML',
				clonesCount: 2,
				folder: pathToFixtures + '/htmlmixed',
			},
			{
				name: 'markup',
				descr: 'Vue',
				clonesCount: 2,
				folder: pathToFixtures + '/vue',
			},
			{
				name: 'markdown',
				descr: 'Text',
				clonesCount: 1,
				folder: pathToFixtures + '/text',
			},
			{
				name: 'jsx',
				clonesCount: 1,
				folder: pathToFixtures + '/jsx',
			},
			{
				name: 'markup',
				descr: 'XML',
				clonesCount: 3,
				folder: pathToFixtures + '/xml',
			},
			{
				name: 'twig',
				clonesCount: 2,
				folder: pathToFixtures + '/twig',
			},
			{
				name: 'tsx',
				clonesCount: 1,
				folder: pathToFixtures + '/jsx',
			},
			{
				name: 'markdown',
				clonesCount: 2,
				folder: pathToFixtures + '/markdown',
			},
			{
				name: 'pug',
				clonesCount: 1,
				folder: pathToFixtures + '/pug',
			},
			{
				name: 'yaml',
				clonesCount: 1,
				folder: pathToFixtures + '/yaml',
			},
		],
		'CSS': [
			{
				name: 'css',
				clonesCount: 2,
				folder: pathToFixtures + '/css',
			},
			{
				name: 'less',
				clonesCount: 2,
				folder: pathToFixtures + '/css',
			},
			{
				name: 'sass',
				clonesCount: 1,
				folder: pathToFixtures + '/sass',
			},
		],
		'Common': [
			{
				name: 'brainfuck',
				clonesCount: 4,
				folder: pathToFixtures + '/brainfuck',
			},
			{
				name: 'php',
				clonesCount: 1,
				folder: pathToFixtures + '/php',
			},
			{
				name: 'rust',
				clonesCount: 6,
				folder: pathToFixtures + '/rust',
			},
			{
				name: 'r',
				clonesCount: 1,
				folder: pathToFixtures + '/r',
			},
			{
				name: 'haskell',
				clonesCount: 2,
				folder: pathToFixtures + '/haskell',
			},
			{
				name: 'd',
				clonesCount: 1,
				folder: pathToFixtures + '/d',
			},
			{
				name: 'erlang',
				clonesCount: 1,
				folder: pathToFixtures + '/erlang',
			},
			{
				name: 'go',
				clonesCount: 1,
				folder: pathToFixtures + '/go',
			},
			{
				name: 'haxe',
				clonesCount: 3,
				folder: pathToFixtures + '/haxe',
			},
			{
				name: 'apl',
				clonesCount: 1,
				folder: pathToFixtures + '/apl',
			},
			{
				name: 'puppet',
				clonesCount: 2,
				folder: pathToFixtures + '/puppet',
			},
			{
				name: 'python',
				clonesCount: 1,
				folder: pathToFixtures + '/python',
			},
			{
				name: 'ruby',
				clonesCount: 1,
				folder: pathToFixtures + '/ruby',
			},
			{
				name: 'perl',
				clonesCount: 2,
				folder: pathToFixtures + '/perl',
			},
			{
				name: 'swift',
				clonesCount: 1,
				folder: pathToFixtures + '/swift',
			},
		],
	}

	Object.keys(formats).forEach(group => {
		describe(group, () => {
			formats[group].forEach((format) => {
				describe(`${format.name} ${format.descr ? `(${format.descr})` : ''}`, () => {
					it('should detect clones in ' + format.name, async () => {
						const argv: string[] = [
							'',
							'',
							format.folder,
							'-f',
							format.name,
						]
						const clones: IClone[] = await jscpd(argv);
						expect(clones.length).to.equal(format.clonesCount);
					});
				});
			});
		});
	});
});
