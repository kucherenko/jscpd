import { IOptions } from '../interfaces/options.interface';
import { IReporter } from '../interfaces/reporter.interface';
import { ConsoleReporter } from './console';
import { ConsoleFullReporter } from './consoleFull';
import { JsonReporter } from './json';
import { SilentReporter } from './silent';
import { ThresholdReporter } from './threshold';
import { TimeReporter } from './time';
import { VerboseReporter } from './verbose';
import { XmlReporter } from './xml';

const EXISTING_REPORTERS: {
  [key: string]: new (options: IOptions) => IReporter;
} = {
  console: ConsoleReporter,
  consoleFull: ConsoleFullReporter,
  time: TimeReporter,
  json: JsonReporter,
  xml: XmlReporter,
  silent: SilentReporter,
  threshold: ThresholdReporter,
  verbose: VerboseReporter
};

const REPORTERS: { [key: string]: IReporter } = {};

export function registerReporter(name: string, reporter: IReporter): void {
  REPORTERS[name] = reporter;
}

export function getRegisteredReporters(): { [key: string]: IReporter } {
  return REPORTERS;
}

export function registerReportersByName(options: IOptions) {
  const { reporters = [] } = options;
  reporters.forEach(rep => registerReporter(rep, new EXISTING_REPORTERS[rep](options)));
}
