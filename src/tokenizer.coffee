JsTokinizer = require './tokenize-js'
CoffeeTokens = require('coffee-script').tokens
Lexer = require('jshint/src/stable/lex').Lexer
crypto = require 'crypto'


class Tokenizer

  constructor: (@isCoffee = no) ->
    @tokenTypes = []

  tokenize: (code) ->
    if @isCoffee
      @tokenizeCoffee(code)
    else
      @tokenizeJS(code)


  tokenizeCoffee: (code) ->
    currentMap = ""
    tokensPositions = []
    for [type, value, options] in CoffeeTokens code
      tokensPositions.push options.first_line
      currentMap = currentMap + @createMap(type, value)

    tokensPositions: tokensPositions, currentMap: currentMap


  tokenizeJS: (code) ->
    currentMap = ""
    tokensPositions = []
    for {type, value, first_line} in JsTokinizer code
      tokensPositions.push first_line
      currentMap = currentMap + @createMap(type, value)

    tokensPositions: tokensPositions, currentMap: currentMap

  getTokenTypeId: (name) ->
    result = 0
    if name in @tokenTypes
      result = @tokenTypes.indexOf(name)
    else
      result = @tokenTypes.length
      @tokenTypes.push name
    result

  createMap: (type, value) ->
    String.fromCharCode(@getTokenTypeId(type)) +
    crypto
      .createHash('md5')
      .update(value.toString())
      .digest('hex')
      .substring 0, 8

exports.Tokenizer = Tokenizer