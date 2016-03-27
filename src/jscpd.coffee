logger = require 'winston'

{Detector} = require './detector'
{Strategy} = require './strategy'
{Report} = require './report'

optionsPreprocessor = require './preprocessors/options'
filesPreprocessor = require './preprocessors/files'

class jscpd

  preProcessors: [
    optionsPreprocessor
    filesPreprocessor
  ]

  LANGUAGES: []

  execPreProcessors: (list) ->
    preProcessor @ for preProcessor in list

  run: (options) ->
    @options = options
    @execPreProcessors @preProcessors
#    config = @readConfig(".cpd.yaml") || @readConfig(".cpd.yml") || {}
#    options = @prepareOptions options, config
#    options.path = options.path or process.cwd();
#
#    logger.profile 'Files search time:'
#
#    excludes = []
#    if options.files is null
#      patterns = ["**/*.+(#{options.extensions.join '|'})"]
#    else
#      unless Array.isArray(options.files)
#        patterns = [options.files]
#      else
#        patterns = options.files
#    if options.exclude isnt null
#      unless Array.isArray(options.exclude)
#        excludes = [options.exclude]
#      else
#        excludes = options.exclude
#
#    if options.debug
#      logger.info '----------------------------------------'
#      logger.info 'Options:'
#      logger.info "#{name} = #{option}" for name, option of options
#      logger.info '----------------------------------------'
#
#    files = []
#    excluded_files = []
#
#    _.forEach patterns, (pattern) ->
#      files = _.union files, glob.sync(pattern, cwd: options.path)
#
#    if excludes.length > 0
#      _.forEach excludes, (pattern) ->
#        excluded_files = _.union excluded_files, glob.sync(pattern, cwd: options.path)
#
#    files = _.difference files, excluded_files
#    files = _.map files, (file) -> path.normalize "#{options.path}/#{file}"
#
#    logger.profile 'Files search time:'
#    if options.debug
#      logger.info '----------------------------------------'
#      logger.info 'Files:'
#      logger.info file for file in files
#      logger.info '----------------------------------------'
#      logger.info 'Run without debug option for start detection process'
#    else
#      logger.profile 'Scanning for duplicates time:'
#      logger.info "Scanning #{files.length} files for duplicates..." if files.length
#
#      strategy = new Strategy options
#      detector = new Detector strategy
#
#      report = new Report options
#
#      codeMap = detector.start files, options['min-lines'], options['min-tokens']
#
#      logger.profile 'Scanning for duplicates time:'
#      logger.info 'Scanning... done!\n'
#
#      logger.profile 'Generate report time:'
#      logger.info 'Start report generation...\n'
#      reportResult = report.generate codeMap
#      logger.profile 'Generate report time:'
#
#      report: reportResult, map: codeMap

module.exports = jscpd
