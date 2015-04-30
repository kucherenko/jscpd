fs = require 'fs'
logger = require 'winston'
path = require 'path'



class Report

  constructor: (@options) ->

    reporter = @options.reporter

    ext = @options.output.split('.').pop() if @options.output

    if ext is 'xml' and reporter is 'json' or
       ext is 'json' and reporter is 'xml'

      logger.warn "output file extention '#{@options.output}'
                  does not match reporter '#{reporter}'"

    switch reporter
      when 'xml' then reporter = './reporters/xml-pmd'
      when 'json' then reporter = './reporters/json'
      else
        cwd = process.cwd()
        reporter = path.normalize reporter
        isAbsolute = reporter.indexOf(cwd) is 0
        reporter = path.join(cwd, reporter) unless isAbsolute

    @reporter = require reporter
    @stdReporter = require './reporters/_std-log'

  generate: (@map) ->

    [raw, dump, log] = @reporter()
    log = @stdReporter() unless log

    logger.info log
    if @options.output
      fs.writeFileSync(@options.output, dump or raw)
    else
      logger.warn 'output file is not provided'

    return raw or dump

exports.Report = Report
