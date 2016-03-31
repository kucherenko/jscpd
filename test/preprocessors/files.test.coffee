require '../bootstrap'
proxyquire = require "proxyquire"

describe "Preprocessor: Files", ->
  sut = null
  jscpd = null

  beforeEach ->
    jscpd =
      options:
        files: null
        extensions: ['js', 'php']

    sut = proxyquire(
      "#{sourcePath}preprocessors/files",
      glob:
        sync: env.stub()
    )

  it 'should search files by extensions if pattern not provided', ->
    sut jscpd
    jscpd.options.patterns.should.deep.equal ["**/*.+(js|php)"]

  it 'should convert to array files option if it is string', ->
    jscpd.options.files = "zzz"
    sut jscpd
    jscpd.options.patterns.should.deep.equal ['zzz'];

  it 'should use files option as pattern if it is array', ->
    jscpd.options.files = ["zzz"]
    sut jscpd
    jscpd.options.patterns.should.deep.equal ['zzz'];

  it 'should convert to array exclude option if it is string', ->
    jscpd.options.exclude = "zzz"
    sut jscpd
    jscpd.options.exclude.should.deep.equal ['zzz'];


