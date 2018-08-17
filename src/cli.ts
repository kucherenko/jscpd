import { bold, white } from 'colors/safe';
import { Command } from 'commander';
import { getSupportedFormats } from './formats';
import { IOptions } from './interfaces/options.interface';
import { JSCPD } from './jscpd';
import { prepareOptions } from './utils';

const packageJson = require(__dirname + '/../package.json');

export const cli: Command = new Command(packageJson.name)
  .version(packageJson.version)
  .usage('[options] <path>')
  .description(packageJson.description);

cli.option(
  '-l, --min-lines [number]',
  'min size of duplication in code lines (Default is 5)'
);
cli.option(
  '-t, --thresh' + 'old [number]',
  'threshold for duplication, in case duplications >= threshold jscpd will exit with error'
);
cli.option(
  '-c, --config [string]',
  'path to config file (Default is .cpd.json in <path>)'
);
cli.option(
  '-i, --ignore [string]',
  'glob pattern for files what should be excluded from duplication detection'
);
cli.option(
  '-r, --reporter [string]',
  'reporter or list of reporters separated with coma to use (Default is time,console)'
);
cli.option('-o, --output [string]', 'reporter to use (Default is ./report/)');
cli.option(
  '-m, --mode [string]',
  'mode of quality of search, can be "strict", "mild" and "weak" (Default is "mild")'
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
cli.option('-n, --no-cache', 'Do not cache results');
cli.option('--xsl-href', '(Deprecated) Path to xsl file');
cli.option('-p, --path', '(Deprecated) Path to repo');
cli.option(
  '-d, --debug',
  'show debug information(options list and selected files)'
);
cli.option('--list', 'show list of all supported formats');

cli.parse(process.argv);

const options: IOptions = prepareOptions(cli);

if (cli.list) {
  console.log(bold(white("Supported formats: ")));
  console.log(getSupportedFormats().join(', '));
} else {
  const cpd: JSCPD = new JSCPD({
    ...options,
    storeOptions: {
      '*': { type: 'files' }
    }
  });
  cpd.detectInFiles(options.path);
}

