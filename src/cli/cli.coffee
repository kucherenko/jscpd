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
  "coffee": ['c', "is CoffeeScript code", "boolean", false]
  "output": ['o', "path to report xml file", "path"],
  "verbose": [false, "show full info about copies"]

  #deprecated fields
  "path": ['p', "path to code (depricated, use -d instant of this)", "path", process.cwd()]
  "ignore": ['i', "directory to ignore  (depricated, use -e instant of this)", "path"],
}

cli.main (args, options) ->
  jscpd::run options


