_ = require "underscore"
yaml = require 'js-yaml'
logger = require 'winston'
fs = require 'fs'
path = require 'path'

TokenizerFactory = require '../tokenizer/TokenizerFactory'

prepareOptions = (options, config) ->
  optionsNew = _.extend optionsPreprocessor.default, config

  for key, value of options
    if not (value is null)
      optionsNew[key] = value

  if typeof optionsNew.languages is 'string'
    optionsNew.languages = optionsNew.languages.split ','

  optionsNew.extensions = TokenizerFactory::getExtensionsByLanguages(optionsNew.languages)
  return optionsNew

readConfig = (file) ->
  file = path.normalize file
  try
    doc = yaml.safeLoad fs.readFileSync(file, 'utf8')
    logger.info "Used config from #{file}"
    return doc
  catch error
    logger.warn "File #{file} not found in current directory, or it is broken"
    return false


optionsPreprocessor = (jscpd) ->
  config = readConfig('.cpd.yaml') or readConfig('.cpd.yml') or {}
  options = prepareOptions jscpd.options, config
  options.path = options.path or process.cwd();
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

  jscpd.options = options

optionsPreprocessor.default =
  languages: Object.keys TokenizerFactory::LANGUAGES
  verbose: off
  debug: off
  files: null
  exclude: null

module.exports = optionsPreprocessor

