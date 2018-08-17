import { writeFileSync } from 'fs';
import { ensureDirSync } from 'fs-extra';
import { StoresManager } from '..';
import { Events } from '../events';
import { IClone } from '../interfaces/clone.interface';
import { IOptions } from '../interfaces/options.interface';
import { IReporter } from '../interfaces/reporter.interface';
import { SOURCES_DB } from '../stores/models';

export class XmlReporter implements IReporter {
  constructor(private options: IOptions) {}

  public attach(): void {
    Events.on('end', this.saveReport.bind(this));
  }

  private saveReport(clones: IClone[]) {
    let xmlDoc: string = '<?xml version="1.0" encoding="UTF-8" ?>';

    xmlDoc = this.options.xslHref
      ? xmlDoc +
        '<?xml-stylesheet type="text/xsl" href="' +
        this.options.xslHref +
        '"?>'
      : xmlDoc;
    xmlDoc += '<pmd-cpd>';

    clones.forEach((clone: IClone) => {
      xmlDoc = `${xmlDoc}
      <duplication lines="${clone.duplicationA.end.loc.end.line -
        clone.duplicationA.start.loc.start.line}">
            <file path="${
              StoresManager.getStore(SOURCES_DB).get(
                clone.duplicationA.sourceId
              ).id
            }" line="${clone.duplicationA.start.loc.start.line}" />
            <file path="${
              StoresManager.getStore(SOURCES_DB).get(
                clone.duplicationB.sourceId
              ).id
            }" line="${clone.duplicationB.start.loc.start.line}" />
            <codefragment><![CDATA[${clone.fragment}]]></codefragment>
        </duplication>
      `;
    });
    xmlDoc += '</pmd-cpd>';

    ensureDirSync(this.options.output);
    writeFileSync(this.options.output + '/jscpd-report.xml', xmlDoc);
  }
}
