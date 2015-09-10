_ = require "underscore"
logger = require 'winston'
path = require "path"
glob = require "glob"
yaml = require 'js-yaml'
fs   = require 'fs'

TokenizerFactory = require './tokenizer/TokenizerFactory'

{Detector} = require './detector'
{Strategy} = require './strategy'
{Report} = require './report'

class jscpd

  LANGUAGES: Object.keys TokenizerFactory::LANGUAGES

  readConfig: (file) ->
    file = path.normalize file
    try
      doc = yaml.safeLoad fs.readFileSync(file, 'utf8')
      logger.info "Used config from #{file}"
      return doc
    catch e
      logger.warn "File #{file} not found in current directory, or it is broken"
      return false

  run: (options)->
    cwd = options.path
    config = @readConfig("#{cwd}/.cpd.yaml") || @readConfig("#{cwd}/.cpd.yml") || {}

    options = _.extend
      'min-lines': 5
      'min-tokens': 70
      files: null
      exclude: null
      languages: jscpd::LANGUAGES
      output: null
      path: null
      verbose: off
      debug: off
    , options

    options = _.extend options, config

    if config.path
      options.path = path.normalize "#{cwd}/#{config.path}"
      cwd = options.path

    options.extensions = TokenizerFactory::getExtensionsByLanguages(options.languages)

    logger.profile 'Files search time:'

    excludes = []
    if options.files is null
      patterns = ["**/*.+(#{options.extensions.join '|'})"]
    else
      unless Array.isArray(options.files)
        patterns = [options.files]
      else
        patterns = options.files
    if options.exclude isnt null
      unless Array.isArray(options.exclude)
        excludes = [options.exclude]
      else
        excludes = options.exclude

    if options.debug
      logger.info '----------------------------------------'
      logger.info 'Options:'
      logger.info "#{name} = #{option}" for name, option of options
      logger.info '----------------------------------------'

    files = []
    excluded_files = []

    _.forEach patterns, (pattern) ->
      files = _.union files, glob.sync(pattern, cwd: cwd)

    if excludes.length > 0
      _.forEach excludes, (pattern) ->
        excluded_files = _.union excluded_files, glob.sync(pattern, cwd: cwd)

    files = _.difference files, excluded_files
    files = _.map files, (file) -> path.normalize "#{cwd}/#{file}"

    logger.profile 'Files search time:'
    if options.debug
      logger.info '----------------------------------------'
      logger.info 'Files:'
      logger.info file for file in files
      logger.info '----------------------------------------'
      logger.info 'Run without debug option for start detection process'
    else
      logger.profile 'Scanning for duplicates time:'
      logger.info "Scanning #{files.length} files for duplicates..." if files.length

      strategy = new Strategy options.languages
      detector = new Detector strategy

      report = new Report
        verbose: options.verbose
        output: options.output
        reporter: options.reporter

      codeMap = detector.start files, options['min-lines'], options['min-tokens']

      logger.profile 'Scanning for duplicates time:'
      logger.info 'Scanning... done!\n'

      logger.profile 'Generate report time:'
      logger.info 'Start report generation...\n'
      reportResult = report.generate codeMap
      logger.profile 'Generate report time:'

      report: reportResult, map: codeMap

module.exports = jscpd
