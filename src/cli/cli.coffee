cli = require("cli").enable("help", "version", "glob")
glob = require "glob"
Detector = require('./../detector').Detector
Strategy = require('./../strategy').Strategy
Report = require('./../report').Report

cli.setUsage "jscpd [OPTIONS]"
cli.setApp('jscpd', '0.1.1')
cli.parse {
  "min-lines": ['m', "min size of duplication in code lines", "number", 5]
  "min-tokens": ['t', "mim size of duplication in code tokens", "number", 70]
  "path": ['p', "path to JavaScript code", "path", process.cwd()]
  "ignore": ['i', "directory to ignore", "path"]
  "log": ['l', "path to log file", "path"]
}

cli.main (args, options) ->
  files = []
  pattern = "#{options.path}/**/*.js"
  exclude = process.cwd() + '/' + options.ignore if options.ignore
  files = glob.sync(pattern, {})
  files = (file for file in files when file.indexOf(exclude) is -1) if exclude

  strategy = new Strategy()
  detector = new Detector(strategy)
  report = new Report({})
  report.generate detector.start(files, options['min-lines'], options['min-tokens'])


