cli = require("cli").enable "help", "version", "glob"
path = require "path"
jscpd = require "./../jscpd"

cli.setUsage "jscpd [OPTIONS]"
cli.setApp path.resolve "#{__dirname}/../../package.json"
cli.parse {
  "min-lines": ['l', "min size of duplication in code lines", "number", 5]
  "min-tokens": ['t', "mim size of duplication in code tokens", "number", 70]
  "path": ['p', "path to code", "path", process.cwd()]
  "coffee": ['c', "is CoffeeScript code", "boolean", false]
  "ignore": ['i', "directory to ignore", "path"],
  "output": ['o', "path to report xml file", "path"],
  "verbose": [false, "show full info about copies"]
}

cli.main (args, options) ->
  jscpd::run options


