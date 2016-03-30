logger = require 'winston'

{Detector} = require './detector'
{Strategy} = require './strategy'
{Report} = require './report'

optionsPreprocessor = require './preprocessors/options'
filesPreprocessor = require './preprocessors/files'
debugPreprocessor = require './preprocessors/debug'

Promise = require "bluebird"

class jscpd

  preProcessors: [
    optionsPreprocessor
    filesPreprocessor
    debugPreprocessor
  ]

  LANGUAGES: []

  execPreProcessors: (list) ->
    logger.profile 'Preprocessors running time:'
    preProcessor @ for preProcessor in list
    logger.profile 'Preprocessors running time:'

  run: (options) ->
    @options = options
    @execPreProcessors @preProcessors

    unless @options.debug
      logger.profile 'Scanning for duplicates time:'
      logger.info "Scanning #{@options.selectedFiles.length} files for duplicates..." if @options.selectedFiles.length

      strategy = new Strategy @options
      detector = new Detector strategy

      report = new Report @options

      codeMap = detector.start @options.selectedFiles, @options['min-lines'], @options['min-tokens']

      logger.profile 'Scanning for duplicates time:'
      logger.info 'Scanning... done!\n'

      logger.profile 'Generate report time:'
      logger.info 'Start report generation...\n'

      generateReport = =>
        reportResult = report.generate codeMap
        logger.profile 'Generate report time:'
        @report = reportResult
        @map = codeMap
        report: @report, map: @map

      if @options.blame
        Promise
        .all(clone.blame() for clone in codeMap.clones)
        .then generateReport
        .error generateReport
      else
        return generateReport()

module.exports = jscpd
