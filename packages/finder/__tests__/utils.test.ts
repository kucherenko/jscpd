import {describe, it, expect} from 'vitest';
import {compareDates, escapeXml, sanitizeCdata, getPath, getPathConsoleString, getSourceLocation, generateLine, convertStatisticToArray} from "../src/utils/reports";
import {buildClone, buildBlameData} from './helpers/clone-builder';
import {relative} from 'path';
import {cwd} from 'process';

describe('jscpd finder: utils', () => {
  describe('escapeXml', () => {
    it('should replace unsafe symbols', () => {
      expect(escapeXml(`<>&'"`)).to.eq('&lt;&gt;&amp;&apos;&quot;')
    });
  });

  describe('sanitizeCdata', () => {
    it('should neutralize every CDATA terminator, not only the first', () => {
      expect(sanitizeCdata('a]]>b]]>c')).to.eq('aCDATA_ENDbCDATA_ENDc');
    });

    it('should strip control characters that are invalid in XML', () => {
      const input = 'a\u0000b\u0008c\u000Bd\u000Ce\u001Ff\uFFFEg\uFFFFh';
      expect(sanitizeCdata(input)).to.eq('abcdefgh');
    });

    it('should keep tab, line feed and carriage return', () => {
      expect(sanitizeCdata('a\tb\nc\rd')).to.eq('a\tb\nc\rd');
    });

    it('should leave ordinary text untouched', () => {
      expect(sanitizeCdata('const a = 1;')).to.eq('const a = 1;');
    });
  });

  describe('compareDates', () => {
    it('should show left arrow', () => {
      expect(compareDates('2020-11-09T15:32:02.397Z', '2018-11-09T15:32:02.397Z')).to.eq('<=');
    });
    it('should show right arrow', () => {
      expect(compareDates('2019-11-09T15:32:02.397Z', '2019-11-10T15:32:02.397Z')).to.eq('=>');
    });
  });

  describe('getPath', () => {
    it('returns raw path when absolute: true', () => {
      const path = '/some/absolute/path/file.js';
      expect(getPath(path, { absolute: true } as any)).to.eq(path);
    });

    it('returns relative path when absolute: false', () => {
      const path = '/some/absolute/path/file.js';
      expect(getPath(path, { absolute: false } as any)).to.eq(relative(cwd(), path));
    });
  });

  describe('getPathConsoleString', () => {
    it('returns a non-empty string', () => {
      const result = getPathConsoleString('/some/path/file.js', { absolute: true } as any);
      expect(result).to.be.a('string');
      expect(result.length).to.be.greaterThan(0);
    });
  });

  describe('getSourceLocation', () => {
    it('returns formatted location string', () => {
      const start = { line: 1, column: 1, position: 0 };
      const end = { line: 10, column: 1, position: 100 };
      expect(getSourceLocation(start, end)).to.eq('1:1 - 10:1');
    });
  });

  describe('generateLine', () => {
    it('returns 3-element array without blame', () => {
      const clone = buildClone();
      const result = generateLine(clone, 0, 'some line');
      expect(result).to.have.length(3);
    });

    it('returns 6-element array with blame on both sides', () => {
      const blameA = buildBlameData('/project/src/a.js', 1, 10);
      const blameB = buildBlameData('/project/src/b.js', 1, 10);
      const clone = buildClone({
        duplicationA: { blame: blameA['/project/src/a.js'] } as any,
        duplicationB: { blame: blameB['/project/src/b.js'] } as any,
      });
      const result = generateLine(clone, 0, 'some line');
      expect(result).to.have.length(6);
    });
  });

  describe('convertStatisticToArray', () => {
    it('returns 7-element array with correct values', () => {
      const statistic = {
        sources: 10,
        lines: 200,
        tokens: 500,
        clones: 5,
        duplicatedLines: 50,
        percentage: 25,
        duplicatedTokens: 100,
        percentageTokens: 20,
      };
      const result = convertStatisticToArray('javascript', statistic as any);
      expect(result).to.have.length(7);
      expect(result[0]).to.eq('javascript');
      expect(result[1]).to.eq('10');
      expect(result[2]).to.eq('200');
      expect(result[3]).to.eq('500');
      expect(result[4]).to.eq('5');
      expect(result[5]).to.include('50');
      expect(result[6]).to.include('100');
    });
  });
})
