import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { buildClone } from './helpers/clone-builder';

vi.mock('fs', () => ({
  readFileSync: vi.fn().mockReturnValue('file content line1\nline2\nline3'),
}));

vi.mock('blamer', () => ({
  default: vi.fn().mockImplementation(function () {
    return {
      blameByFile: vi.fn().mockResolvedValue({
        '/project/src/a.js': {
          1: { line: 1, author: 'Alice', date: '2024-01-01' },
          2: { line: 2, author: 'Bob', date: '2024-01-02' },
        },
      }),
    };
  }),
}));

describe('FragmentsHook', () => {
  it('addFragments sets fragment on duplicationA and duplicationB', async () => {
    const { FragmentsHook } = await import('../src/hooks/fragment');
    const clone = buildClone();
    const result = FragmentsHook.addFragments(clone);
    expect(typeof result.duplicationA.fragment).toBe('string');
    expect(typeof result.duplicationB.fragment).toBe('string');
  });

  it('process resolves with array of clones each having fragment set', async () => {
    const { FragmentsHook } = await import('../src/hooks/fragment');
    const hook = new FragmentsHook();
    const clones = [buildClone(), buildClone()];
    const result = await hook.process(clones);
    expect(result).toHaveLength(2);
    result.forEach((clone) => {
      expect(typeof clone.duplicationA.fragment).toBe('string');
      expect(typeof clone.duplicationB.fragment).toBe('string');
    });
  });
});

describe('BlamerHook', () => {
  it('getBlamedLines returns only lines within range', async () => {
    const { BlamerHook } = await import('../src/hooks/blamer');
    const blamedFiles = {
      '/src/a.js': {
        1: { line: 1, author: 'Alice', date: '2024-01-01' },
        5: { line: 5, author: 'Bob', date: '2024-01-05' },
      },
    };
    const result = BlamerHook.getBlamedLines(blamedFiles as any, 1, 3);
    expect(result[1]).toBeDefined();
    expect((result as any)[5]).toBeUndefined();
  });

  it('process resolves with clone having blame set on both duplications', async () => {
    const { BlamerHook } = await import('../src/hooks/blamer');
    const hook = new BlamerHook();
    const clone = buildClone();
    const [result] = await hook.process([clone]);
    expect(result.duplicationA.blame).toBeDefined();
    expect(result.duplicationB.blame).toBeDefined();
  });
});
