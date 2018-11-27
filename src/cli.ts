import { bold, white } from 'colors/safe';
import { Command } from 'commander';
import { IOptions, JSCPD } from '.';
import { BlamerPostHook } from './hooks/post/blamer';
import { getSupportedFormats } from './tokenizer/formats';
import { getOption, prepareOptions } from './utils/options';

const packageJson = require(__dirname + '/../package.json');

export const cli: Command = new Command(packageJson.name)
  .version(packageJson.version)
  .usage('[options] <path ...>')
  .description(packageJson.description);

cli.option(
  '-l, --min-lines [number]',
  'min size of duplication in code lines (Default is ' + getOption('minLines') + ')'
);
cli.option('-x, --max-lines [number]', 'max size of source in lines (Default is ' + getOption('maxLines') + ')');
cli.option(
  '-z, --max-size [string]',
  'max size of source in bytes, examples: 1kb, 1mb, 120kb (Default is ' + getOption('maxSize') + ')'
);
cli.option(
  '-t, --threshold [number]',
  'threshold for duplication, in case duplications >= threshold jscpd will exit with error'
);
cli.option('-c, --config [string]', 'path to config file (Default is .cpd.json in <path>)');
cli.option('-i, --ignore [string]', 'glob pattern for files what should be excluded from duplication detection');
cli.option(
  '-r, --reporters [string]',
  'reporters or list of reporters separated with coma to use (Default is time,console)'
);
cli.option('-o, --output [string]', 'reporters to use (Default is ./report/)');
cli.option(
  '-m, --mode [string]',
  'mode of quality of search, can be "strict", "mild" and "weak" (Default is "' + getOption('mode') + '")'
);
cli.option('-f, --format [string]', 'format or formats separated by coma (Example php,javascript,python)');
cli.option('-b, --blame', 'blame authors of duplications (get information about authors from git)');
cli.option('-s, --silent', 'do not write detection progress and result to a console');
cli.option('-a, --absolute', 'use absolute path in reports');
cli.option('-g, --gitignore', 'ignore all files from .gitignore file');
cli.option('--formats-exts [string]', 'list of formats with file extensions (javascript:es,es6;dart:dt)');
// cli.option('--cache', 'Cache results of duplication detection');
cli.option('-d, --debug', 'show debug information(options list and selected files)');
cli.option('--list', 'show list of total supported formats');

cli.option('--xsl-href [string]', '(Deprecated) Path to xsl file');
cli.option('-p, --path [string]', '(Deprecated) Path to repo, use `jscpd <path>`');

cli.parse(process.argv);

const options: IOptions = prepareOptions(cli);

if (cli.list) {
  console.log(bold(white('Supported formats: ')));
  console.log(getSupportedFormats().join(', '));
  process.exit(0);
}

if (cli.debug) {
  console.log(bold(white('Options:')));
  console.dir(options);
}

const cpd: JSCPD = new JSCPD(options);

if (cpd.options.blame) {
  cpd.attachPostHook(new BlamerPostHook());
}

cpd.detectInFiles(options.path);
