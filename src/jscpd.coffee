_ = require "underscore"
path = require "path"
glob = require "glob"
{Detector} = require './detector'
{Strategy} = require './strategy'
{Report} = require './report'

class jscpd
  run: (options)->
    options = _.extend
      'min-lines': 5
      'min-tokens': 70
      path: process.cwd()
      coffee: false
      ignore: null
      output: null
    , options

    console.log "\njscpd - copy/paste detector for JavaScript and CoffeeScript, developed by Andrey Kucherenko\n"
    
    files = []
    pattern = "#{options.path}/**/*.#{ if options.coffee then 'coffee' else 'js'}"
    exclude = process.cwd() + '/' + options.ignore if options.ignore

    files = glob.sync(pattern, {})
    files = (file for file in files when file.indexOf(exclude) is -1) if exclude
    console.log 'Scaning...' if files.length

    strategy = new Strategy options.coffee
    detector = new Detector strategy

    report = new Report
      verbose: options.verbose
      output: options.output

    codeMap = detector.start files, options['min-lines'], options['min-tokens']
    console.log 'Scaning... done!\n'
    return report.generate codeMap

module.exports = jscpd