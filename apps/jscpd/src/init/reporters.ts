import {
  ConsoleFullReporter,
  ConsoleReporter,
  CSVReporter,
  InFilesDetector,
  JsonReporter,
  MarkdownReporter,
  SilentReporter,
  ThresholdReporter,
  XcodeReporter,
  XmlReporter,
} from '@jscpd/finder';
import {IOptions} from '@jscpd/core';
import {grey, yellow} from 'colors/safe';
import HtmlReporter from "@jscpd/html-reporter";
import SarifReporter from "jscpd-sarif-reporter";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const reporters: Record<string, any> = {
  xml: XmlReporter,
  json: JsonReporter,
  csv: CSVReporter,
  markdown: MarkdownReporter,
  consoleFull: ConsoleFullReporter,
  html: HtmlReporter,
  console: ConsoleReporter,
  silent: SilentReporter,
  threshold: ThresholdReporter,
  xcode: XcodeReporter,
  sarif: SarifReporter,
}

export function registerReporters(options: IOptions, detector: InFilesDetector): void {

  // @ts-ignore
  options.reporters.forEach((reporter: string) => {
    if (reporter in reporters) {
      detector.registerReporter(new reporters[reporter](options));
    } else {
      try {
        const reporterClass = require(`@jscpd/${reporter}-reporter`).default;
        detector.registerReporter(new reporterClass(options));
      } catch (e) {
        try {
          const reporterClass = require(`jscpd-${reporter}-reporter`).default;
          detector.registerReporter(new reporterClass(options));
        } catch (e) {
          console.log(yellow(`warning: ${reporter} not installed (install packages named @jscpd/${reporter}-reporter or jscpd-${reporter}-reporter)`))
          console.log(grey((e as any).message));
        }
      }
    }
  });
}
