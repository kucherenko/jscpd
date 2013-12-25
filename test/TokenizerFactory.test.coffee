if process.env['COVERAGE']
  console.log 'COVERAGE mode is on'
  sourcePath = '../.tmp/'
else
  sourcePath = '../src/'

TokenizerFactory = require "#{sourcePath}tokenizer/TokenizerFactory"
TokenizerJS = require "#{sourcePath}tokenizer/TokenizerJS"
TokenizerCoffee = require "#{sourcePath}tokenizer/TokenizerCoffee"

expect = require('chai').expect
should = require('chai').should()

describe "TokenizerFactory", ->

  it "should return tokinezer for javascript if file with extension js", ->
    TokenizerFactory::makeTokenizer('file.js').should.be.an.instanceOf TokenizerJS

  it "should return tokinezer for javascript if file with extension coffee", ->
    TokenizerFactory::makeTokenizer('file.coffee').should.be.an.instanceOf TokenizerCoffee