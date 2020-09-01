import {IClone, IOptions, IStatisticRow, ITokenLocation} from '@jscpd/core';
import {relative} from "path";
import {cwd} from "process";
import {bold, green, grey} from 'colors/safe';

export const compareDates = (firstDate: string, secondDate: string): string => {
  const first = new Date(firstDate);
  const second = new Date(secondDate);
  switch (true) {
    case first < second:
      return '=>';
    case first > second:
      return '<=';
    default:
      return '==';
  }
}

export function escapeXml(unsafe: string): string {
  return unsafe.replace(/[<>&'"]/g, function (c) {
    switch (c) {
      case '<': return '&lt;';
      case '>': return '&gt;';
      case '&': return '&amp;';
      case '\'': return '&apos;';
      case '"': return '&quot;';
    }
  });
}

export function getPath(path: string, options: IOptions): string {
  return options.absolute ? path : relative(cwd(), path);
}

export function getPathConsoleString(path: string, options: IOptions): string {
  return bold(green(getPath(path, options)));
}

export function getSourceLocation(start: ITokenLocation, end: ITokenLocation): string {
	return `${start.line}:${start.column} - ${end.line}:${end.column}`;
}

export function generateLine(clone: IClone, position: number, line: string): string[] {
	const lineNumberA: string = (clone.duplicationA.start.line + position).toString();
	const lineNumberB: string = (clone.duplicationB.start.line + position).toString();
	if (clone.duplicationA.blame && clone.duplicationB.blame) {
		return [
			lineNumberA,
			clone.duplicationA.blame[lineNumberA] ? clone.duplicationA.blame[lineNumberA].author : '',
			clone.duplicationA.blame[lineNumberA] && clone.duplicationB.blame[lineNumberB]
				? compareDates(clone.duplicationA.blame[lineNumberA].date, clone.duplicationB.blame[lineNumberB].date)
				: '',
			lineNumberB,
			clone.duplicationB.blame[lineNumberB] ? clone.duplicationB.blame[lineNumberB].author : '',
			grey(line),
		];
	} else {
		return [lineNumberA, lineNumberB, grey(line)];
	}
}

export function convertStatisticToArray(format: string, statistic: IStatisticRow): string[] {
  return [
    format,
    `${statistic.sources}`,
    `${statistic.lines}`,
    `${statistic.tokens}`,
    `${statistic.clones}`,
    `${statistic.duplicatedLines} (${statistic.percentage}%)`,
    `${statistic.duplicatedTokens} (${statistic.percentageTokens}%)`,
  ]
}

