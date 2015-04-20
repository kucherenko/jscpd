fs = require 'fs'
logger = require 'winston'


class Report

  constructor: (@options) ->

    reporter = @options.reporter

    ext = @options.output.split('.').pop() if @options.output

    if ext is 'xml' and reporter is 'json' or
       ext is 'json' and reporter is 'xml'

      logger.warn 'output file extention does not match reporter'


    switch reporter
      when 'xml' then reporter = './reporters/xml-pmd'
      when 'json' then reporter = './reporters/json'

    @reporter = require reporter
    @stdReporter = require './reporters/_std-log'

  generate: (@map) ->

    [report, out, log] = @reporter()
    log = @stdReporter() unless log

    logger.info log
    if @options.output
      fs.writeFileSync(@options.output, report)
    else
      logger.warn 'output file is not provided'

    return out or report

exports.Report = Report
