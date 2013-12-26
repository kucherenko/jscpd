_ = require "underscore"
path = require "path"
glob = require "glob"
{Detector} = require './detector'
{Strategy} = require './strategy'
{Report} = require './report'

class jscpd
#  LANGUAGES: ['js', 'coffee']
  run: (options)->

    options = _.extend
      'min-lines': 5
      'min-tokens': 70
      files: null
      exclude: null
      languages: ['js', 'coffee']
      coffee: false
      output: null
      path: null
      ignore: null
    , options

    options.languages = ['coffee'] if options.coffee

    console.log "\njscpd - copy/paste detector for JavaScript and CoffeeScript, developed by Andrey Kucherenko\n"

    excludes = []
    if options.files is null
      patterns = ["**/*.+(#{options.languages.join '|'})"]
    else
      unless Array.isArray(options.files)
        patterns = [options.files]
      else
        patterns = options.files
    if options.exclude is null
      excludes = ["**/#{options.ignore}/**"] if options.ignore
    else
      unless Array.isArray(options.exclude)
        excludes = [options.exclude]
      else
        excludes = options.exclude

    files = []
    excluded_files = []

    _.forEach patterns, (pattern) ->
      files = _.union files, glob.sync(pattern, cwd: options.path)

    if excludes.length > 0
      _.forEach excludes, (pattern) ->
        excluded_files = _.union excluded_files, glob.sync(pattern, cwd: options.path)

    files = _.difference files, excluded_files
    files = _.map files, (file) -> "#{options.path}#{file}"

    console.log "Scaning #{files.length} files for copies..." if files.length

    strategy = new Strategy options.languages
    detector = new Detector strategy

    report = new Report
      verbose: options.verbose
      output: options.output

    codeMap = detector.start files, options['min-lines'], options['min-tokens']
    console.log 'Scaning... done!\n'

    report.generate codeMap

module.exports = jscpd