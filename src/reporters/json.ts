import { writeFileSync } from 'fs';
import { ensureDirSync } from 'fs-extra';
import { Events } from '../events';
import { IClone } from '../interfaces/clone.interface';
import { IOptions } from '../interfaces/options.interface';
import { IReporter } from '../interfaces/reporter.interface';
import { IStatistic } from '../interfaces/statistic.interface';
import { ITokenLocation } from '../interfaces/token/token-location.interface';
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
  };
  secondFile: {
    name: string;
    start: number;
    end: number;
    startLoc: ITokenLocation;
    endLoc: ITokenLocation;
  };
  fragment: string;
}

interface IJsonReport {
  duplicates: IDuplication[];
  statistics: {
    all?: IStatistic;
    formats?: {
      [key: string]: IStatistic;
    };
  };
}

export class JsonReporter implements IReporter {
  private json: IJsonReport = {
    duplicates: [],
    statistics: {}
  };

  constructor(private options: IOptions) {}

  public attach(): void {
    Events.on('end', this.saveReport.bind(this));
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
        endLoc: clone.duplicationA.end
      },
      secondFile: {
        name: getPath(this.options, StoresManager.getStore(SOURCES_DB).get(clone.duplicationB.sourceId).id),
        start: startLineB,
        end: endLineB,
        startLoc: clone.duplicationB.start,
        endLoc: clone.duplicationB.end
      }
    });
  }

  private saveReport(clones: IClone[]) {
    const statistic = StoresManager.getStore(STATISTIC_DB).get(this.options.executionId);

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
