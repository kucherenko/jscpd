import {IClone, IOptions, IStatistic} from '@jscpd-ai/core';
import {IReporter} from "@jscpd/finder";
import {badgen} from "badgen";
import {join} from 'path';
import {ensureDirSync, writeFileSync} from "fs-extra";
import {green} from "colors/safe";

export default class BadgeReporter implements IReporter {
  constructor(private options: IOptions) {
  }

  // @ts-ignore
  public report(clones: IClone[], statistic: IStatistic): void {
    const badgeOptions = this.options.reportersOptions ? this.options.reportersOptions.badge || {} : {};
    if (this.options.output) {
      const badge = badgen({
        color: this.getColor(statistic),
        status: this.getStatus(statistic),
        subject: 'Copy/Paste',
        ...badgeOptions
      });
      const path = badgeOptions.path ? badgeOptions.path : join(this.options.output, 'jscpd-badge.svg');
      ensureDirSync(this.options.output);
      writeFileSync(path, badge);
      console.log(green(`Badge saved to ${path}`));
    }
  }

  public getStatus(statistic: IStatistic): string {
    return statistic ? statistic.total.percentage + '%' : 'N/A'
  }

  public getColor(statistic: IStatistic): string {
    if (this.options.threshold === undefined) {
      return 'grey';
    }
    return statistic.total.percentage < this.options.threshold ? 'green' : 'red';
  }
}
