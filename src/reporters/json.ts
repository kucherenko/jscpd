import {IReporter} from "../interfaces/reporter.interface";
import {IClone} from "../interfaces/clone.interface";
import {IOptions} from "../interfaces/options.interface";
import {writeFileSync} from "fs";
import {ensureDirSync} from "fs-extra";
import {StoresManager} from "../stores/stores-manager";
import {ITokenLocation} from "../interfaces/token/token-location.interface";
import {Events} from "../events";

interface IDuplication {
  format: string;
  lines: number;
  tokens: number;
  firstFile: {
    name: string;
    start: number;
    end: number;
    startLoc: ITokenLocation,
    endLoc: ITokenLocation
  }
  secondFile: {
    name: string;
    start: number;
    end: number;
    startLoc: ITokenLocation,
    endLoc: ITokenLocation
  }
  fragment: string
}

interface IJsonReport {
  duplicates: IDuplication[]
  statistics: {
    clones: number;
    duplications: number;
    files: number;
    percentage: number;
    lines: number;
  }
}

export class JsonReporter implements IReporter {
  private json: IJsonReport = {
    duplicates: [],
    statistics: {
      clones: 0,
      duplications: 0,
      files: 0,
      percentage: 0,
      lines: 0
    }
  };

  constructor(private options: IOptions){}

  attach(): void {
    Events.on('end', this.saveReport.bind(this));
  }

  private cloneFound(clone: IClone) {
    const startLineA = clone.duplicationA.start.loc.start.line;
    const endLineA = clone.duplicationA.end.loc.end.line;
    const startLineB = clone.duplicationB.start.loc.start.line;
    const endLineB = clone.duplicationB.end.loc.end.line;

    this.json.duplicates.push({
      format: clone.format,
      lines: endLineA - startLineA + 1,
      fragment: clone.fragment,
      tokens: 0,
      firstFile: {
        name: StoresManager.get('source').get(clone.duplicationA.sourceId).id,
        start: startLineA,
        end: endLineA,
        startLoc: clone.duplicationA.start.loc.start,
        endLoc: clone.duplicationA.end.loc.end,
      },
      secondFile: {
        name: StoresManager.get('source').get(clone.duplicationA.sourceId).id,
        start: startLineB,
        end: endLineB,
        startLoc: clone.duplicationB.start.loc.start,
        endLoc: clone.duplicationB.end.loc.end,
      }
    });
  }

  private saveReport(clones: IClone[]) {
    clones.forEach((clone: IClone) => {
      this.cloneFound(clone);
    });
    ensureDirSync(this.options.output);
    writeFileSync(this.options.output + '/jscpd-report.json', JSON.stringify(this.json, null, '\t'));
  }
}
