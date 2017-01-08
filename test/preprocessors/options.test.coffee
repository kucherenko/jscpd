require '../bootstrap'
proxyquire = require "proxyquire"

describe "Preprocessor: Options", ->
  yaml = null
  sut = null
  jscpd = null
  options = null
  TokenizerFactory = null

  beforeEach ->
    TokenizerFactory = {}
    options = {test: 'option'}

    jscpd = {
      options: {}
    }

    fs =
      readFileSync: env.stub().returns 'content'
    yaml =
      safeLoad: env.stub().returns options

    sut = proxyquire(
      "#{sourcePath}preprocessors/options",
        'fs': fs,
        'js-yaml': yaml
        '../tokenizer/TokenizerFactory': TokenizerFactory
    )
    sut.default = {
      option: 'default'
      languages: ['js']
    }

  it 'should set default options if can not read .cpd.yaml and .cpd.yml', ->
    yaml.safeLoad.throws()
    sut jscpd
    jscpd.options.should.deep.equal sut.default

  it 'should set options to jscpd', ->
    sut jscpd
    jscpd.options.test.should.equal 'option'

  it 'should split languages by coma if it is string',  ->
    jscpd.options.languages = 'js,php'
    sut jscpd
    jscpd.options.languages.should.deep.equal ['js', 'php']

  it 'should parse `languages-exts` string to object', ->
    jscpd.options['languages-exts'] = "javascript:js,jsx,es6;php:php"
    sut jscpd
    jscpd.options['languages-exts'].should.deep.equal
      javascript: ['js', 'jsx', 'es6']
      php: ['php']

  it 'should set parsed `languages-exts` to TokenizerFactory', ->
    jscpd.options['languages-exts'] = "javascript:js,es5,es6,es"
    sut jscpd
    TokenizerFactory::LANGUAGES['javascript'].should.deep.equal ['js', 'es5', 'es6', 'es']

  it 'should skip not supported languages from `languages-exts`', ->
    jscpd.options['languages-exts'] = "javascript1:js,es5,es6,es"
    sut jscpd
    TokenizerFactory::LANGUAGES.should.not.have.ownProperty 'javascript1'
