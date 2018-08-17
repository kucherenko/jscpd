import { IOptions } from '../interfaces/options.interface';
import { IReporter } from '../interfaces/reporter.interface';
import { ConsoleReporter } from './console';
import { ConsoleFullReporter } from './consoleFull';
import { JsonReporter } from './json';
import { SilentReporter } from './silent';
import { StatisticReporter } from './statistic';
import { ThresholdReporter } from './threshold';
import { TimeReporter } from './time';
import { XmlReporter } from './xml';

const EXISTING_REPORTERS: {
  [key: string]: new (options: IOptions) => IReporter;
} = {
  console: ConsoleReporter,
  consoleFull: ConsoleFullReporter,
  time: TimeReporter,
  json: JsonReporter,
  xml: XmlReporter,
  stat: StatisticReporter,
  silent: SilentReporter,
  threshold: ThresholdReporter
};

const REPORTERS: { [key: string]: IReporter } = {};

export function registerReporter(name: string, reporter: IReporter): void {
  REPORTERS[name] = reporter;
}

export function getRegisteredReporters(): { [key: string]: IReporter } {
  return REPORTERS;
}

export function registerReportersByName(options: IOptions) {
  const { reporter = [] } = options;
  reporter.forEach(rep =>
    registerReporter(rep, new EXISTING_REPORTERS[rep](options))
  );
}
