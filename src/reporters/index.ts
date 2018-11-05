import { IOptions } from '../interfaces/options.interface';
import { IReporter } from '../interfaces/reporter.interface';
import { useReporter } from '../utils/use';
import { ConsoleReporter } from './console';
import { ConsoleFullReporter } from './console-full';
import { ExecTimerReporter } from './exec-timer';
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
  verbose: VerboseReporter,
  execTimer: ExecTimerReporter
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
  reporters.forEach(rep => {
    const reporter: new (options: IOptions) => IReporter = EXISTING_REPORTERS[rep] || useReporter(rep);
    registerReporter(rep, new reporter(options));
  });
}
