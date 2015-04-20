fs = require 'fs'
logger = require 'winston'


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
