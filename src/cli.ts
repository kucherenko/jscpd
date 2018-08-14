import {Command} from 'commander';
import {JSCPD} from "./jscpd";
import {prepareOptions} from "./utils";
import {IOptions} from "./interfaces/options.interface";

const packageJson = require('../package.json');

const cli: Command = new Command(packageJson.name)
  .version(packageJson.version)
  .usage('[options] <id>')
  .description(packageJson.description);

cli.option(
  '-l, --min-lines [number]',
  'min size of duplication in code lines (Default is 5)'
);
cli.option(
  '-t, --threshold [number]',
  'threshold for duplication, in case duplications >= threshold jscpd will exit with error'
);
cli.option(
  '-c, --config [string]',
  'id to config file (Default is .cpd.json in <id>)'
);
cli.option(
  '-i, --ignore [string]',
  'glob pattern for files what should be excluded from duplication detection'
);
cli.option(
  '-r, --reporter [string]',
  'reporter or list of reporters separated with coma to use (Default is time,console)'
);
cli.option(
  '-o, --output [string]',
  'reporter to use (Default is ./report/)'
);
cli.option(
  '-m, --mode [string]',
  'mode of quality of search, can be "strict", "mild" and "weak"(Default is "mild")'
);
cli.option(
  '-f, --format [string]',
  'format or formats separated by coma (Example php,javascript,python)'
);
cli.option(
  '-b, --blame',
  'blame authors of duplications (get information about authors from git)'
);
cli.option(
  '-s, --silent',
  'Do not write detection progress and result to a console'
);
cli.option(
  '-p, --path',
  '(Deprecated) Path to repo'
);
cli.option(
  '-d, --debug',
  'show debug information(options list and selected files)'
);
cli.option(
  '--list',
  'show list of all supported formats'
);

cli.parse(process.argv);

const options: IOptions = prepareOptions(cli);

const cpd: JSCPD =  new JSCPD(options);

cpd.detectInFiles(options.path);

