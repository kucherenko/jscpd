fs = require 'fs'
logger = require 'winston'


class Report

  constructor: (@options) ->

    @reporter = require @options.reporter

  generate: (@map) ->

    [report, log] = @reporter(@map, @options)

    logger.info log if log

    fs.writeFileSync(@options.output, report) if @options.output

    return report

exports.Report = Report
