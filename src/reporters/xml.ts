import { writeFileSync } from 'fs';
import { ensureDirSync } from 'fs-extra';
import { IOptions, IReporter } from '..';
import { IClone } from '../interfaces/clone.interface';
import { getPath } from '../utils';
import { getOption } from '../utils/options';

export class XmlReporter implements IReporter {
  constructor(private options: IOptions) {}

  public attach(): void {}

  public report(clones: IClone[]) {
    let xmlDoc: string = '<?xml version="1.0" encoding="UTF-8" ?>';

    xmlDoc = this.options.xslHref
      ? xmlDoc + '<?xml-stylesheet type="text/xsl" href="' + this.options.xslHref + '"?>'
      : xmlDoc;
    xmlDoc += '<pmd-cpd>';

    clones.forEach((clone: IClone) => {
      xmlDoc = `${xmlDoc}
      <duplication lines="${clone.duplicationA.end.line - clone.duplicationA.start.line}">
            <file path="${getPath(this.options, clone.duplicationA.sourceId)}" line="${clone.duplicationA.start.line}">
              <codefragment><![CDATA[${clone.duplicationA.fragment}]]></codefragment>
            </file>
            <file path="${getPath(this.options, clone.duplicationB.sourceId)}" line="${clone.duplicationB.start.line}">
              <codefragment><![CDATA[${clone.duplicationB.fragment}]]></codefragment>
            </file>
            <codefragment><![CDATA[${clone.duplicationA.fragment}]]></codefragment>
        </duplication>
      `;
    });
    xmlDoc += '</pmd-cpd>';

    ensureDirSync(getOption('output', this.options));
    writeFileSync(getOption('output', this.options) + '/jscpd-report.xml', xmlDoc);
  }
}
