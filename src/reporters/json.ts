import { writeFileSync } from 'fs';
import { ensureDirSync } from 'fs-extra';
import { IBlamedLines } from '../interfaces/blame.interface';
import { IClone } from '../interfaces/clone.interface';
import { IOptions } from '../interfaces/options.interface';
import { IReporter } from '../interfaces/reporter.interface';
import { IStatistic } from '../interfaces/statistic.interface';
import { ITokenLocation } from '../interfaces/token/token-location.interface';
import { JSCPD } from '../jscpd';
import { SOURCES_DB, STATISTIC_DB } from '../stores/models';
import { StoresManager } from '../stores/stores-manager';
import { getPath } from '../utils';

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

  public attach(): void {
    JSCPD.getEventsEmitter().on('end', this.saveReport.bind(this));
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
        name: getPath(this.options, StoresManager.getStore(SOURCES_DB).get(clone.duplicationA.sourceId).id),
        start: startLineA,
        end: endLineA,
        startLoc: clone.duplicationA.start,
        endLoc: clone.duplicationA.end,
        blame: clone.duplicationA.blame
      },
      secondFile: {
        name: getPath(this.options, StoresManager.getStore(SOURCES_DB).get(clone.duplicationB.sourceId).id),
        start: startLineB,
        end: endLineB,
        startLoc: clone.duplicationB.start,
        endLoc: clone.duplicationB.end,
        blame: clone.duplicationB.blame
      }
    });
  }

  private saveReport(clones: IClone[]) {
    const statistic: IStatistic = StoresManager.getStore(STATISTIC_DB).get(this.options.executionId);

    if (statistic) {
      this.json.statistics = statistic;
    }

    clones.forEach((clone: IClone) => {
      this.cloneFound(clone);
    });
    ensureDirSync(this.options.output);
    writeFileSync(this.options.output + '/jscpd-report.json', JSON.stringify(this.json, null, '\t'));
  }
}
