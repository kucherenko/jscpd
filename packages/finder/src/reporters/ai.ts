import {IClone, IOptions, IStatistic} from '@jscpd/core';
import {IReporter} from '..';
import {getPath} from '../utils/reports';

function normalizePath(p: string): string {
  return p.replace(/\\/g, '/');
}

function formatRange(start: number, end: number): string {
  return `${start}-${end}`;
}

export function compressCloneLine(
  pathA: string,
  pathB: string,
  rangeA: string,
  rangeB: string
): string {
  const normA = normalizePath(pathA);
  const normB = normalizePath(pathB);

  if (normA === normB) {
    return `${normA} ${rangeA} ~ ${rangeB}`;
  }

  const partsA = normA.split('/');
  const partsB = normB.split('/');

  let commonLen = 0;
  const minLen = Math.min(partsA.length, partsB.length);
  // Stop before the filename segment (minLen - 1)
  while (commonLen < minLen - 1 && partsA[commonLen] === partsB[commonLen]) {
    commonLen++;
  }

  if (commonLen === 0) {
    return `${normA}:${rangeA} ~ ${normB}:${rangeB}`;
  }

  const prefix = partsA.slice(0, commonLen).join('/');
  const remA = partsA.slice(commonLen).join('/');
  const remB = partsB.slice(commonLen).join('/');
  return `${prefix}/ ${remA}:${rangeA} ~ ${remB}:${rangeB}`;
}

export class AiReporter implements IReporter {
  constructor(private readonly options: IOptions) {}

  report(clones: IClone[], statistic: IStatistic | undefined): void {
    if (this.options.silent) return;

    for (const clone of clones) {
      const pathA = getPath(clone.duplicationA.sourceId, this.options);
      const pathB = getPath(clone.duplicationB.sourceId, this.options);
      const rangeA = formatRange(clone.duplicationA.start.line, clone.duplicationA.end.line);
      const rangeB = formatRange(clone.duplicationB.start.line, clone.duplicationB.end.line);
      console.log(compressCloneLine(pathA, pathB, rangeA, rangeB));
    }

    if (statistic) {
      console.log('---');
      console.log(`${clones.length} clones · ${statistic.total.percentage.toFixed(1)}% duplication`);
    }
  }
}
