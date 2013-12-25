if process.env['COVERAGE']
  console.log 'COVERAGE mode is on'
  sourcePath = '../.tmp/'
else
  sourcePath = '../src/'

TokenizerJS = require "#{sourcePath}tokenizer/TokenizerJS"

expect = require('chai').expect
should = require('chai').should()



describe "TokenizerJS", ->

  it "should have method tokenize(code)", ->
    TokenizerJS::tokenize.should.be.a 'function'
