import {IReporter} from "../interfaces/reporter.interface";
import {ConsoleReporter} from "./console";
import {SpinnerReporter} from "./spinner";
import {TimeReporter} from "./time";
import {ConsoleFullReporter} from "./consoleFull";
import {IOptions} from "../interfaces/options.interface";
import {JsonReporter} from "./json";

const REPORTERS: { [key: string]: IReporter } = {};

export function registerReporter(name: string, reporter: IReporter): void {
  REPORTERS[name] = reporter;
}

export function getRegisteredReporters(): { [key: string]: IReporter } {
  return REPORTERS;
}

export function registerReportersByName(options: IOptions) {
  const {reporter = []} = options;
  if (reporter.includes('console')) {
    registerReporter('console', new ConsoleReporter(options));
  }

  if (reporter.includes('consoleFull')) {
    registerReporter('consoleFull', new ConsoleFullReporter(options));
  }

  if (reporter.includes('spinner')) {
    registerReporter('spinner', new SpinnerReporter(options));
  }

  if (reporter.includes('time')) {
    registerReporter('time', new TimeReporter(options));
  }

  if (reporter.includes('json')) {
    registerReporter('json', new JsonReporter(options));
  }
}
