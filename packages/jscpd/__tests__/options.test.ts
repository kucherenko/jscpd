import {expect} from 'chai';
import {isAbsolute} from 'path';
import {IClone} from '@jscpd/core';
import {jscpd, detectClones} from '../src';
import {bold, yellow} from 'colors/safe';
import sinon = require('sinon');

const pathToFixtures = __dirname + '/../../../fixtures';

const fileWithClones = pathToFixtures + '/clike/file2.c';
describe('jscpd options', () => {

  let _log, _error, _dir;

  beforeEach(() => {
    _log = console.log;
    _dir = console.dir;
    _error = console.error;
    console.log = sinon.spy();
    console.dir = sinon.spy();
    console.error = sinon.spy();
  })

  afterEach(() => {
    console.log = _log;
    console.dir = _dir;
    console.error = _error;
  })

  describe('LevelDB Store', () => {
    it('should use leveldb store', async () => {
      const clones: IClone[] = await jscpd(['', '', fileWithClones, '--store', 'leveldb']);
      expect(clones.length).to.equal(1);
    });
  });

  describe('Ignore Blocks', () => {
    it('should skip blocks marked as ignored', async () => {
      const clones: IClone[] = await jscpd(['', '', pathToFixtures + '/ignore', '--silent']);
      expect(clones.length).to.equal(0);
    });
  });

  describe('Ignore Patterns', () => {
    it('should skip blocks marked as ignored', async () => {
      const clones: IClone[] = await jscpd(['', '', pathToFixtures + '/ignore-pattern', '--ignore-pattern', "import.*from\\s*'.*'",  '--min-lines', '5', '--min-tokens', '20']);
      expect(clones.length).to.equal(1);
      const clone = clones[0];
      expect(clone.duplicationA.start.line).to.equal(6);
      expect(clone.duplicationA.end.line).to.equal(13);
      expect(clone.duplicationB.start.line).to.equal(8);
      expect(clone.duplicationB.end.line).to.equal(16);
    });
  });

  describe('detect in one file', () => {
    it('should detect duplications inside one file', async () => {
      const clones: IClone[] = await jscpd(['', '', fileWithClones])
      const clone = clones[0];
      expect(clone.duplicationA.start.line).to.equal(18);
      expect(clone.duplicationA.end.line).to.equal(28);
      expect(clone.duplicationB.start.line).to.equal(8);
      expect(clone.duplicationB.end.line).to.equal(18);
    });
	});

	describe('clones not found', () => {
		it('should return empty array if clones not found', async () => {
			const clones: IClone[] = await jscpd(['', '', pathToFixtures + '/clike/file1.c']);
			expect(clones.length).to.equal(0);
		});
	});

	describe('blame lines', () => {
		it('should get information from git about authors of cloned code', async () => {
			const clones: IClone[] = await jscpd(['', '',
        fileWithClones,
        '--blame',
        '-r',
        'consoleFull',
      ]);
			const clone = clones[0];
			expect(clone.duplicationA.blame['18'].author).to.equal('Andrey Kucherenko');
		});
	});

	describe('min lines', () => {
		it('should skip clone if it is length less than min lines option', async () => {
      const clones: IClone[] = await jscpd(['', '', fileWithClones, '--min-lines', '20']);
      expect(clones.length).to.equal(0);
    });
	});

	describe('formats-exts', () => {
		it('should detect clones in files with custom extensions', async () => {
			const clones: IClone[] = await jscpd([
				'', '',
				pathToFixtures + '/custom/',
				'--formats-exts',
				'c:ccc,cc1',
			]);
			expect(clones.length).to.equal(2);
		});
	});

	describe('skip local', () => {

		it('should not skip clone if it is located in same folder without --skipLocal option', async () => {
			const clones: IClone[] = await jscpd([
				'', '',
				pathToFixtures + '/folder1',
				pathToFixtures + '/folder2',
			]);
			expect(clones.length).to.equal(3);
		});

		it('should skip clone if it is located in same folder with --skipLocal option', async () => {
			const clones: IClone[] = await jscpd([
				'', '',
				pathToFixtures + '/folder1',
				pathToFixtures + '/folder2',
				'--skipLocal',
			]);
			// ??? Investigate the skipLocal
			expect(clones.length).to.equal(1);
		});
	});

  describe('skip isolated', () => {
    const folder1Path = pathToFixtures + '/folder1';
    const folder2Path = pathToFixtures + '/folder2';
    const lib1Path = pathToFixtures + '/lib1';
    const lib2Path = pathToFixtures + '/lib2';

    it('should not skip clone if it is located in isolated folder without --skipIsolated option', async () => {
      const clones: IClone[] = await jscpd([
        '', '',
        folder1Path,
        folder2Path,
        lib1Path,
        lib2Path,
      ]);
      // lib2 file_2.js lib2 file_2.js
      // lib1 file_1.js lib2 file_2.js
      // lib1 file2.c lib1 file2.c
      // folder2 file_2.js lib2 file_2.js
      // folder1 file_1.js lib1 file_1.js
      // folder1 file2.c lib1 file2.c
      expect(clones.length).to.equal(6);
    });

    it('should skip clone if it is located in isolated folder with --skipIsolated option', async () => {
      const clones: IClone[] = await jscpd([
        '', '',
        folder1Path,
        folder2Path,
        lib1Path,
        lib2Path,
        '--skipIsolated',
        // folder1Path is isolated with folder2Path, lib1Path is isolated with lib2Path
        `${folder1Path}|${folder2Path},${lib1Path}|${lib2Path}`
      ]);
      // lib2 file_2.js lib2 file_2.js
      // lib1 file2.c lib1 file2.c
      // folder2 file_2.js lib2 file_2.js
      // folder1 file_1.js lib1 file_1.js
      // folder1 file2.c lib1 file2.c
      expect(clones.length).to.equal(5);
    });
  });

	describe('silent', () => {
    it('should not print more information about detection process', async () => {
      await jscpd(['', '', fileWithClones, '--silent']);
      const log = (console.log as any);
      expect(log.callCount).to.equal(1);
      expect(
        log.calledWith(`Duplications detection: Found ${bold('1')} exact clones with ${bold('10')}(35.71%) duplicated lines in ${bold('1')} (1 formats) files.`),
      ).to.be.ok;
    });

    it('should not print information about clones', async () => {
      // console.log = _log;
      await detectClones({
        silent: true,
        pattern: fileWithClones,
      });
      const log = (console.log as any);
      _log(log.firstCall);
      expect(log.callCount).to.equal(0);
    });
  });

  describe('Not Supported Format', () => {
    it('should skip files with not supported formats', async () => {
      const clones: IClone[] = await jscpd(['', '',
        fileWithClones,
        '-f', 'javascript',
      ]);
      expect(clones.length).to.equal(0);
    });
  });

  describe('Ignore Case', () => {
		it('should not skip case of symbols if --ignoreCase is not enabled', async () => {
			const clones: IClone[] = await jscpd(['', '', pathToFixtures + '/ignore-case', '--silent']);
			expect(clones.length).to.equal(0);
		});

		it('should skip symbols case if --ignoreCase is enabled', async () => {
			const clones: IClone[] = await jscpd([
				'', '',
				pathToFixtures + '/ignore-case',
				'--silent',
				'--ignoreCase',
			]);
			expect(clones.length).to.equal(1);
		});
	});

	describe('threshold', () => {
		it('should throw error if current level of copy/paste more then threshold', async () => {
			try {
        await jscpd(['', '', fileWithClones, '--threshold', '10']);
      } catch (e) {
				expect(e.message).to.equal('ERROR: jscpd found too many duplicates (35.71%) over threshold (10%)');
			}
		});
	});

	describe('verbose', () => {
		it('should log information about start detection process', async () => {
      await jscpd(['', '', fileWithClones, '--verbose']);
      const log = (console.log as any);
      expect(log.calledWith(yellow('START_DETECTION'))).to.be.ok;
    });
		it('should log information about detected clone', async () => {
      const clones: IClone[] = await jscpd(['', '', fileWithClones, '--verbose']);
      const log = (console.log as any);
      expect(log.calledWith(yellow('CLONE_FOUND'))).to.be.ok;
      expect(clones.length).to.equal(1);
    });
		it('should log information about skipped clone', async () => {
      await jscpd(['', '', fileWithClones, '--verbose', '--min-lines', '20']);
      const log = (console.log as any);
      expect(log.calledWith(yellow('CLONE_SKIPPED'))).to.be.ok;
    });
  });

  describe('debug', () => {
    it('should log information about start detection process', async () => {
      await jscpd(['', '', fileWithClones, '--debug']);
      const log = (console.log as any);
      expect(log.calledWith(bold(`Found 1 files to detect.`))).to.be.ok;
    });
  });

  describe('installed reporter', () => {
    it('should detect clones and report with custom installed reporters', async () => {
      await jscpd(['', '', fileWithClones, '--reporters', 'badge', '--silent']);
      const log = (console.log as any);
      expect(
        log.calledWith(`Duplications detection: Found ${bold('1')} exact clones with ${bold('10')}(35.71%) duplicated lines in ${bold('1')} (1 formats) files.`),
      ).to.be.ok;
    });
    it('should show warning if reporter does not installed', async () => {
      await jscpd(['', '', fileWithClones, '--reporters', 'badgezz', '--silent']);
      const log = (console.log as any);
      expect(
        log.calledWith(
          yellow(`warning: badgezz not installed (install packages named @jscpd/badgezz-reporter or jscpd-badgezz-reporter)`),
        ),
      ).to.be.ok;
    });
  });

  describe('absolute', () => {
    it('should return files with absolute path', async () => {
      const clones: IClone[] = await jscpd(['', '', fileWithClones, '--absolute']);
      const clone = clones[0];
      expect(isAbsolute(clone.duplicationA.sourceId)).to.be.ok;
    });
  });

  describe('exitCode', () => {
    let log, exit

    beforeEach(() => {
      log = (console.log as any);
      exit = sinon.spy()
    })

    it('should exit with 0 when exitCode is not specified', async () => {
      await jscpd(['', '', fileWithClones], exit);
      expect(exit.calledWith(0)).to.be.ok
    });

  });
});
