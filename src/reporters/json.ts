import { writeFileSync } from 'fs';
import { ensureDirSync } from 'fs-extra';
import { IClone, IOptions, IReporter } from '..';
import { IBlamedLines } from '../interfaces/blame.interface';
import { IStatistic } from '../interfaces/statistic.interface';
import { ITokenLocation } from '../interfaces/token/token-location.interface';
import { getPath } from '../utils';
import { getOption } from '../utils/options';

interface IDuplication {
  format: string;
  lines: number;
  tokens: number;
  firstFile: {
    name: string;
    start: number;
    end: number;
    startLoc: ITokenLocation;
    endLoc: ITokenLocation;
    blame?: IBlamedLines;
  };
  secondFile: {
    name: string;
    start: number;
    end: number;
    startLoc: ITokenLocation;
    endLoc: ITokenLocation;
    blame?: IBlamedLines;
  };
  fragment: string;
}

interface IJsonReport {
  duplicates: IDuplication[];
  statistics: IStatistic;
}

export class JsonReporter implements IReporter {
  private json: IJsonReport = {
    duplicates: [],
    statistics: {} as IStatistic
  };

  constructor(private options: IOptions) {}

  public attach(): void {}

  public report(clones: IClone[], statistic: IStatistic): void {
    if (statistic) {
      this.json.statistics = statistic;
    }

    clones.forEach((clone: IClone) => {
      this.cloneFound(clone);
    });

    ensureDirSync(getOption('output', this.options));
    writeFileSync(getOption('output', this.options) + '/jscpd-report.json', JSON.stringify(this.json, null, '\t'));
  }

  private cloneFound(clone: IClone) {
    const startLineA = clone.duplicationA.start.line;
    const endLineA = clone.duplicationA.end.line;
    const startLineB = clone.duplicationB.start.line;
    const endLineB = clone.duplicationB.end.line;

    this.json.duplicates.push({
      format: clone.format,
      lines: endLineA - startLineA + 1,
      fragment: clone.duplicationA.fragment as string,
      tokens: 0,
      firstFile: {
        name: getPath(this.options, clone.duplicationA.sourceId),
        start: startLineA,
        end: endLineA,
        startLoc: clone.duplicationA.start,
        endLoc: clone.duplicationA.end,
        blame: clone.duplicationA.blame
      },
      secondFile: {
        name: getPath(this.options, clone.duplicationB.sourceId),
        start: startLineB,
        end: endLineB,
        startLoc: clone.duplicationB.start,
        endLoc: clone.duplicationB.end,
        blame: clone.duplicationB.blame
      }
    });
  }
}
