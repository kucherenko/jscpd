require '../bootstrap'
proxyquire = require "proxyquire"

describe "Preprocessor: Debug", ->
  sut = null
  jscpd = null
  logger = null

  beforeEach ->
    logger = info: env.stub()
    jscpd =
      options:
        debug: yes
        files: null
        selectedFiles: []
        extensions: ['js', 'php']

    sut = proxyquire(
      "#{sourcePath}preprocessors/debug",
      winston: logger
    )

  it 'should print to log info about options', ->
    sut jscpd
    logger.info.should.have.been.called;

