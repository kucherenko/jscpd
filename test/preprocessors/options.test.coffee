require '../bootstrap'
proxyquire = require "proxyquire"

describe "Preprocessor: Options", ->
  yaml = null
  sut = null
  jscpd = null
  options = null

  beforeEach ->
    options = {test: 'option'}

    jscpd = {}
    fs =
    readFileSync: env.stub().returns 'content'
    yaml =
    safeLoad: env.stub().returns options

    sut = proxyquire(
      "#{sourcePath}preprocessors/options",
    'fs': fs, 'js-yaml': yaml
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

