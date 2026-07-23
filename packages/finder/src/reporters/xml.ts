import {writeFileSync} from 'fs';
import {ensureDirSync} from 'fs-extra';
import {IReporter} from '..';
import {getOption, IClone, IOptions} from '@jscpd/core';
import {escapeXml, getPath, sanitizeCdata} from '../utils/reports';
import {green} from 'colors/safe';
import {join} from "path";

export class XmlReporter implements IReporter {
  constructor(private options: IOptions) {
  }


  public report(clones: IClone[]): void {
    let xmlDoc = '<?xml version="1.0" encoding="UTF-8" ?>';

    xmlDoc += '<pmd-cpd>';

    clones.forEach((clone: IClone) => {
      const fragmentA = sanitizeCdata(clone.duplicationA.fragment ?? '');
      const fragmentB = sanitizeCdata(clone.duplicationB.fragment ?? '');
      xmlDoc = `${xmlDoc}
      <duplication lines="${clone.duplicationA.end.line - clone.duplicationA.start.line}">
            <file path="${escapeXml(getPath(clone.duplicationA.sourceId, this.options))}" line="${clone.duplicationA.start.line}">
              <codefragment><![CDATA[${fragmentA}]]></codefragment>
            </file>
            <file path="${escapeXml(getPath(clone.duplicationB.sourceId, this.options))}" line="${clone.duplicationB.start.line}">
              <codefragment><![CDATA[${fragmentB}]]></codefragment>
            </file>
            <codefragment><![CDATA[${fragmentA}]]></codefragment>
        </duplication>
      `;
		});
		xmlDoc += '</pmd-cpd>';

		ensureDirSync(getOption('output', this.options));
		writeFileSync(getOption('output', this.options) + '/jscpd-report.xml', xmlDoc);
		console.log(green(`XML report saved to ${join(this.options.output as string, 'jscpd-report.xml')}`));
	}
}
