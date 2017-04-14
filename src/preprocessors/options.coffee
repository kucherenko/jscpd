_ = require "underscore"
yaml = require 'js-yaml'
fs = require 'fs'
path = require 'path'

TokenizerFactory = require '../tokenizer/TokenizerFactory'

prepareOptions = (options, config) ->
  optionsNew = _.extend optionsPreprocessor.default, config

  for key, value of options
    if not (value is null)
      optionsNew[key] = value

  if optionsNew.hasOwnProperty 'languages-exts'
    if typeof optionsNew['languages-exts'] is 'string'
      optionsNew['languages-exts'] = parseLanguagesExtensions optionsNew['languages-exts']

    for own lang, exts of optionsNew['languages-exts']
      TokenizerFactory::LANGUAGES[lang] = exts if TokenizerFactory::LANGUAGES.hasOwnProperty lang

  if typeof optionsNew.languages is 'string'
    optionsNew.languages = optionsNew.languages.split ','

  optionsNew.extensions = TokenizerFactory::getExtensionsByLanguages(optionsNew.languages)

  return optionsNew

readConfig = (file) ->
  file = path.normalize file
  try
    doc = yaml.safeLoad fs.readFileSync(file, 'utf8')
    doc.config_file = file
    return doc
  catch error
    return false

parseLanguagesExtensions = (extensions) ->
  result = {}
  extensions.split(';').forEach (language) ->
    pair = language.split ':'
    result[pair[0]] = pair[1].split ','
  return result


optionsPreprocessor = (jscpd) ->
  if jscpd.options.config?
    config = readConfig(jscpd.options.config)

    if not config then throw new Error "JSCPD Error 02: can't read config file #{jscpd.options.config}"

    process.chdir path.dirname jscpd.options.config
  else
    config = readConfig('.cpd.yaml') or readConfig('.cpd.yml') or {}

  options = prepareOptions jscpd.options, config
  if not path.isAbsolute options.path or ''
    options.path = path.resolve process.cwd(), options.path or ''
  jscpd.options = options

optionsPreprocessor.default =
  languages: Object.keys TokenizerFactory::LANGUAGES
  verbose: off
  debug: off
  files: null
  exclude: null
  "min-lines": 5
  "min-tokens": 70
  limit: 50
  reporter: "xml"

module.exports = optionsPreprocessor
