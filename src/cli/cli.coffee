cli = require("cli").enable "help", "version", "glob"
path = require "path"
jscpd = require "./../jscpd"

cli.setUsage "jscpd [OPTIONS]"
cli.setApp path.resolve "#{__dirname}/../../package.json"
cli.parse {
  "min-lines": ['l', "min size of duplication in code lines", "number", 5]
  "min-tokens": ['t', "mim size of duplication in code tokens", "number", 70]
  "files": ['f', "glob pattern for find code", "string"]
  "exclude": ['e', "directory to ignore", "string"],
  "languages": ['g', "list of languages which scan for duplicates, separated with comma", "string", jscpd::LANGUAGES.join(',')]
  "output": ['o', "path to report xml file", "path"],
  "verbose": [false, "show full info about copies"]
  "path": ['p', "path to code", "path", process.cwd()]

  #deprecated fields
  "ignore": ['i', "directory to ignore  (deprecated, use -e instant of this)", "path"],
  "coffee": ['c', "is CoffeeScript code (deprecated, use --languages for set source languages)", "boolean", false]
}

cli.main (args, options) ->
  console.log "\njscpd - copy/paste detector for programming source code, developed by Andrey Kucherenko\n"
  options.languages = options.languages.split ','
  jscpd::run options


