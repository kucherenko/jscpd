import {
  ConsoleFullReporter,
  ConsoleReporter,
  InFilesDetector,
  JsonReporter,
  SilentReporter,
  CSVReporter,
  MarkdownReporter,
  ThresholdReporter,
  XcodeReporter,
  XmlReporter,
} from '@jscpd/finder';
import {IOptions} from '@jscpd/core';
import {yellow, grey} from 'colors/safe';
import HtmlReporter from "@jscpd/html-reporter";

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
}

export function registerReporters(options: IOptions, detector: InFilesDetector): void {

  options.reporters.forEach((reporter: string) => {
    if (reporter in reporters) {
      detector.registerReporter(new reporters[reporter](options));
    } else {
      try {
        const reporterClass = require(`jscpd-${reporter}-reporter`).default;
        detector.registerReporter(new reporterClass(options));
      } catch (e) {
        try {
          const reporterClass = require(`@jscpd/${reporter}-reporter`).default;
          detector.registerReporter(new reporterClass(options));
        } catch (e) {
          console.log(yellow(`warning: ${reporter} not installed (install packages named @jscpd/${reporter}-reporter or jscpd-${reporter}-reporter)`))
          console.log(grey(e.message));
        }
      }
    }
  });
}
