import {join} from 'path';
import {IClone, IOptions, IStatistic} from '@jscpd/core';
import {IReporter, JsonReporter} from "@jscpd/finder";
import {copySync, writeFileSync} from "fs-extra";
import {green, red} from "colors/safe";
import * as pug from "pug";

export default class HtmlReporter implements IReporter {
  constructor(private options: IOptions) {
  }

  public report(clones: IClone[], statistic: IStatistic): void {
    const jsonReporter = new JsonReporter(this.options);
    const json = jsonReporter.generateJson(clones, statistic);
    const result = pug.renderFile(join(__dirname, './templates/main.pug'), json)
    if (this.options.output) {
      const destination = join(this.options.output, 'html/');
      try {
        copySync(join(__dirname, '../public'), destination, {overwrite: true});
        const index = join(destination, 'index.html');
        writeFileSync(index, result)
        writeFileSync(join(destination, 'jscpd-report.json'),
          JSON.stringify(json, null, '  ')
        );
        console.log(green(`HTML report saved to ${join(this.options.output, 'html/')}`));
      } catch (e) {
        console.log(red(e as string))
      }
    }
  }
}
