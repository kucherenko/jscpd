
fs = require 'fs'
JSHINT = require('jshint').JSHINT
Lexer = require('jshint/src/stable/lex').Lexer

class Strategy

  constructor: ->
    @skippedTokens = []
    @codeHashes = []

  detect: (map, file, minLines, minTokens) ->
    console.log file
    code = fs.readFileSync(file, {encoding: 'utf-8'})
    lines = code.split '\n'
    map.numberOfLines =  map.numberOfLines + lines.length
    success = JSHINT(code, {}, {});
    tokens = []
    lexer = new Lexer(code)
    lexer.start()

    loop
      token = lexer.token()
      tokens.push token
      break if token.type is "(end)"

    console.log tokens


exports.Strategy = Strategy