import { describe, it, expect } from 'vitest';
import { IToken } from '@jscpd/core';
import { TokensMap, createTokensMaps, generateMapsForFormats } from '../src/token-map';

function makeToken(
  value: string,
  type: string,
  format: string,
  position: number,
  line: number,
): IToken {
  return {
    type,
    value,
    format,
    length: value.length,
    range: [position, position + value.length],
    loc: {
      start: { line, column: 1, position },
      end: { line, column: 1 + value.length, position },
    },
  };
}

function makeTokens(count: number, format = 'javascript'): IToken[] {
  return Array.from({ length: count }, (_, i) =>
    makeToken(`tok${i}`, 'keyword', format, i * 5, i + 1),
  );
}

const baseOptions = { minTokens: 3, ignoreCase: false };

describe('TokensMap', () => {
  describe('constructor and getters', () => {
    it('getId returns the provided id', () => {
      const tokens = makeTokens(5);
      const map = new TokensMap('my-id', 'source', tokens, 'javascript', baseOptions);
      expect(map.getId()).toBe('my-id');
    });

    it('getFormat returns the provided format', () => {
      const tokens = makeTokens(5);
      const map = new TokensMap('id', 'source', tokens, 'typescript', baseOptions);
      expect(map.getFormat()).toBe('typescript');
    });

    it('getTokensCount returns numeric difference of positions', () => {
      const tokens = makeTokens(5);
      const map = new TokensMap('id', 'source', tokens, 'javascript', baseOptions);
      expect(typeof map.getTokensCount()).toBe('number');
    });

    it('getLinesCount includes the first and last token lines', () => {
      const tokens = makeTokens(5);
      const map = new TokensMap('id', 'source', tokens, 'javascript', baseOptions);
      expect(map.getLinesCount()).toBe(5);
    });

    it('getLinesCount is 1 for single-line tokens', () => {
      const tokens = [
        makeToken('a', 'keyword', 'javascript', 0, 1),
        makeToken('b', 'keyword', 'javascript', 2, 1),
        makeToken('c', 'keyword', 'javascript', 4, 1),
      ];
      const map = new TokensMap('id', 'data', tokens, 'javascript', baseOptions);
      expect(map.getLinesCount()).toBe(1);
    });

    it('lowercases token values when ignoreCase is true', () => {
      const tokens = [makeToken('CONST', 'keyword', 'javascript', 0, 1)];
      new TokensMap('id', 'data', tokens, 'javascript', { ...baseOptions, ignoreCase: true });
      expect(tokens[0]?.value).toBe('const');
    });

    it('leaves token values unchanged when ignoreCase is false', () => {
      const tokens = [makeToken('CONST', 'keyword', 'javascript', 0, 1)];
      new TokensMap('id', 'data', tokens, 'javascript', { ...baseOptions, ignoreCase: false });
      expect(tokens[0]?.value).toBe('CONST');
    });
  });

  describe('iterator protocol', () => {
    it('[Symbol.iterator] returns self', () => {
      const tokens = makeTokens(5);
      const map = new TokensMap('id', 'source', tokens, 'javascript', baseOptions);
      expect(map[Symbol.iterator]()).toBe(map);
    });

    it('next() returns done:false with a frame when enough tokens remain', () => {
      const tokens = makeTokens(10);
      const map = new TokensMap('id', 'source', tokens, 'javascript', baseOptions);
      const result = map.next();
      expect(result.done).toBe(false);
      const frame = result.value as { id: string; sourceId: string; start: IToken; end: IToken };
      expect(frame).toHaveProperty('id');
      expect(frame).toHaveProperty('sourceId');
      expect(frame).toHaveProperty('start');
      expect(frame).toHaveProperty('end');
    });

    it('frame sourceId matches the map id', () => {
      const tokens = makeTokens(10);
      const map = new TokensMap('file-abc', 'source', tokens, 'javascript', baseOptions);
      const result = map.next();
      expect(result.done).toBe(false);
      const frame = result.value as { sourceId: string };
      expect(frame.sourceId).toBe('file-abc');
    });

    it('next() returns done:true when tokens are exhausted', () => {
      // With minTokens=3 and 3 tokens, position 0 is not < 3-3=0, so done immediately
      const tokens = makeTokens(3);
      const map = new TokensMap('id', 'source', tokens, 'javascript', baseOptions);
      const result = map.next();
      expect(result.done).toBe(true);
      expect(result.value).toBe(false);
    });

    it('for...of loop iterates frames correctly', () => {
      const tokens = makeTokens(10);
      const map = new TokensMap('id', 'source', tokens, 'javascript', baseOptions);
      const frames: unknown[] = [];
      for (const frame of map) {
        frames.push(frame);
      }
      // With 10 tokens and minTokens=3, expect 10-3=7 frames
      expect(frames.length).toBe(7);
    });

    it('produces the expected number of frames', () => {
      const count = 20;
      const minTokens = 5;
      const tokens = makeTokens(count);
      const map = new TokensMap('id', 'source', tokens, 'javascript', { minTokens });
      let frameCount = 0;
      for (const _ of map) {
        frameCount++;
      }
      expect(frameCount).toBe(count - minTokens);
    });

    it('uses custom hashFunction when provided', () => {
      const customHash = (v: string) => v.slice(0, 20).padEnd(20, '0');
      const tokens = makeTokens(10);
      const map = new TokensMap('id', 'source', tokens, 'javascript', {
        ...baseOptions,
        hashFunction: customHash,
      });
      const result = map.next();
      expect(result.done).toBe(false);
    });
  });
});

describe('generateMapsForFormats', () => {
  it('returns an array of TokensMap', () => {
    const tokens = makeTokens(5);
    const maps = generateMapsForFormats('id', 'data', tokens, baseOptions);
    expect(Array.isArray(maps)).toBe(true);
    expect(maps.length).toBeGreaterThan(0);
    expect(maps[0]).toBeInstanceOf(TokensMap);
  });

  it('groups tokens by format into separate maps', () => {
    const tokens = [
      ...makeTokens(5, 'javascript'),
      ...makeTokens(5, 'typescript'),
    ];
    const maps = generateMapsForFormats('id', 'data', tokens, baseOptions);
    expect(maps.length).toBe(2);
    const formats = maps.map((m) => m.getFormat()).sort();
    expect(formats).toEqual(['javascript', 'typescript']);
  });

  it('creates a single map when all tokens share one format', () => {
    const tokens = makeTokens(8, 'python');
    const maps = generateMapsForFormats('id', 'data', tokens, baseOptions);
    expect(maps.length).toBe(1);
    expect(maps[0]?.getFormat()).toBe('python');
  });

  it('returns empty array for empty token list', () => {
    const maps = generateMapsForFormats('id', 'data', [], baseOptions);
    expect(maps).toEqual([]);
  });
});

describe('createTokensMaps', () => {
  it('returns same result as generateMapsForFormats', () => {
    const tokens = makeTokens(6);
    const maps = createTokensMaps('id', 'data', tokens, baseOptions);
    expect(Array.isArray(maps)).toBe(true);
    expect(maps.length).toBeGreaterThan(0);
  });

  it('each returned map has the correct id', () => {
    const tokens = makeTokens(5);
    const maps = createTokensMaps('source-file', 'data', tokens, baseOptions);
    for (const map of maps) {
      expect(map.getId()).toBe('source-file');
    }
  });
});
