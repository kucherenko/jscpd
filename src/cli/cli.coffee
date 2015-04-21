logger = require 'winston'
cli = require("cli").enable "help", "version", "glob"
path = require "path"
jscpd = require "./../jscpd"

logger.cli();

cli.setUsage "jscpd [OPTIONS]"
cli.setApp path.resolve "#{__dirname}/../../package.json"
cli.parse {
  "min-lines": ['l', "min size of duplication in code lines", "number", 5]
  "min-tokens": ['t', "mim size of duplication in code tokens", "number", 70]
  "files": ['f', "glob pattern for find code", "string"]
  "exclude": ['e', "directory to ignore", "string"],
  "languages": [
    'g'
    "list of languages which scan for duplicates, separated with comma"
    "string", jscpd::LANGUAGES.join ','
  ]
  "output": ['o', "path to report file", "path"],
  "reporter": ['r', "reporter to use", "string", "xml"],
  "verbose": [false, "show full info about copies"]
  "debug": ['d', "show debug information(options list and selected files)"]
  "path": ['p', "path to code", "path", process.cwd()]
}

cli.main (args, options) ->
  logger.profile "All time:"
  logger.info """
jscpd - copy/paste detector for programming source code, developed by Andrey Kucherenko
"""

  options.languages = options.languages.split ','
  jscpd::run options
  logger.profile "All time:"


