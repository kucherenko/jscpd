import type {IClone, IOptions, IStatistic, ITokenLocation} from '@jscpd-ai/core';
import type {IReporter} from "@jscpd/finder";
import {join} from 'path';
import {ensureDirSync, readJsonSync, writeFileSync} from "fs-extra";
import {green} from "colors/safe";
import {SarifBuilder, SarifResultBuilder, SarifRuleBuilder, SarifRunBuilder} from "node-sarif-builder";


function getSourceLocation(start: ITokenLocation, end: ITokenLocation): string {
  return `${start.line}:${start.column} - ${end.line}:${end.column}`;
}

export default class SarifReporter implements IReporter {
  constructor(private options: IOptions) {
  }

  public report(clones: IClone[], statistic: IStatistic): void {
    const url = "https://github.com/kucherenko/jscpd/";
    if (this.options.output) {
      const pkg = readJsonSync(join(__dirname, '../package.json'))
      // SARIF builder
      const sarifBuilder = new SarifBuilder();
      // SARIF Run builder
      const sarifRunBuilder = new SarifRunBuilder().initSimple({
        toolDriverName: "jscpd",
        toolDriverVersion: pkg.version,
        url
      });

      sarifRunBuilder.addRule(
        new SarifRuleBuilder().initSimple({
          ruleId: 'duplication',
          shortDescriptionText: 'Found code duplication',
          helpUri: url
        })
      )

      sarifRunBuilder.addRule(
        new SarifRuleBuilder().initSimple({
          ruleId: 'duplications-threshold',
          shortDescriptionText: 'Level of duplication is too high',
          helpUri: url
        })
      )


      for (const clone of clones) { // issues from your linter in any format
        const sarifResultBuilder = new SarifResultBuilder();
        // Init sarifResultBuilder
        sarifRunBuilder.addResult(
          sarifResultBuilder.initSimple(
            {
              // Transcode to a SARIF level:  can be "warning" or "error" or "note"
              level: "warning",
              messageText: `Clone detected in ${clone.format}, - ${clone.duplicationA.sourceId}[${getSourceLocation(clone.duplicationA.start, clone.duplicationA.end)}] and ${clone.duplicationB.sourceId}[${getSourceLocation(clone.duplicationB.start, clone.duplicationB.end)}]`,
              ruleId: 'duplication',
              fileUri: clone.duplicationA.sourceId,
              startLine: clone.duplicationA.start.line,
              startColumn: clone.duplicationA.start.column,
              endLine: clone.duplicationA.end.line,
              endColumn: clone.duplicationA.end.column
            }
          )
        )
      }

      if (statistic.total?.percentage >= (this.options.threshold || 100)) {
        const sarifResultBuilderThreshold = new SarifResultBuilder();
        sarifRunBuilder.addResult(
          sarifResultBuilderThreshold.initSimple({
            level: 'error',
            messageText: `The duplication level (${statistic.total.percentage}%) is bigger than threshold (${this.options.threshold}%)`,
            ruleId: "duplications-threshold",
          })
        )
      }

      const path = join(this.options.output, 'jscpd-sarif.json');

      sarifBuilder.addRun(sarifRunBuilder);
      const sarifJsonString = sarifBuilder.buildSarifJsonString({ indent: false });
      ensureDirSync(this.options.output);
      writeFileSync(path, sarifJsonString);

      console.log(green(`SARIF report saved to ${path}`));
    }
  }
}
