require '../bootstrap'

TokenizerFactory = require "#{sourcePath}tokenizer/TokenizerFactory"
TokenizerCodeMirror = require "#{sourcePath}tokenizer/TokenizerCodeMirror"

describe "TokenizerFactory", ->

  it "should return tokenizer for javascript if file with extension js", ->
    TokenizerFactory::makeTokenizer('file.js', ['javascript']).should.be.an.instanceOf TokenizerCodeMirror

  it "should return tokenizer for php if file with extension php", ->
    TokenizerFactory::makeTokenizer('file.php', ['php']).should.be.an.instanceOf TokenizerCodeMirror

  it "should return tokenizer for jsx if file with extension jsx", ->
    TokenizerFactory::makeTokenizer('file.jsx', ['jsx']).should.be.an.instanceOf TokenizerCodeMirror

  it "should return tokenizer for haxe if file with extension hx", ->
    TokenizerFactory::makeTokenizer('file.hx', ['haxe']).should.be.an.instanceOf TokenizerCodeMirror

  it "should return tokenizer for haxe if file with extension hxml", ->
    TokenizerFactory::makeTokenizer('file.hxml', ['haxe']).should.be.an.instanceOf TokenizerCodeMirror

  it "should return tokenizer for typescript if file with extension ts", ->
    TokenizerFactory::makeTokenizer('file.ts', ['typescript']).should.be.an.instanceOf TokenizerCodeMirror

  it "should return tokenizer for python if file with extension py", ->
    TokenizerFactory::makeTokenizer('file.py', ['python']).should.be.an.instanceOf TokenizerCodeMirror

  it "should return tokenizer for coffeescript if file with extension coffee", ->
    TokenizerFactory::makeTokenizer('file.coffee', ['coffeescript']).should.be.an.instanceOf TokenizerCodeMirror

  it "should return tokenizer for yaml if file with extension yml", ->
    TokenizerFactory::makeTokenizer('file.yml', ['yaml']).should.be.an.instanceOf TokenizerCodeMirror

  it "should return tokenizer for yaml if file with extension yaml", ->
    TokenizerFactory::makeTokenizer('file.yaml', ['yaml']).should.be.an.instanceOf TokenizerCodeMirror

  it "should return tokenizer for erlang if file with extension erl", ->
    TokenizerFactory::makeTokenizer('file.erl', ['erlang']).should.be.an.instanceOf TokenizerCodeMirror

  it "should return false if language is not supported ", ->
    TokenizerFactory::makeTokenizer('file.coffee', ['php']).should.be.equal false
