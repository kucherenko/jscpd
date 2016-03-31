require './bootstrap'

JsCpd = require "#{sourcePath}jscpd"

optionsPreprocessor = require "#{sourcePath}preprocessors/options"
filesPreprocessor = require "#{sourcePath}preprocessors/files"
debugPreprocessor = require "#{sourcePath}preprocessors/debug"

describe "jscpd", ->

  jscpd = null
  options = null

  beforeEach ->
    options = {
      debug: on
    }
    jscpd = new JsCpd

  it "should run preparation stage while run ", ->
    env.spy jscpd, 'execPreProcessors'
    jscpd.run options
    jscpd.execPreProcessors.should.have.been.calledWith jscpd.preProcessors

  context "PreProcessors", ->
    it 'should run each preprocessor', ->
      preprocessor = env.spy()
      jscpd.execPreProcessors [preprocessor]
      preprocessor.should.have.been.calledWith jscpd

    it 'should have options preprocessor', ->
      jscpd.preProcessors.should.include optionsPreprocessor

    it 'should have files preprocessor', ->
      jscpd.preProcessors.should.include filesPreprocessor

    it 'should have debug preprocessor', ->
      jscpd.preProcessors.should.include debugPreprocessor
