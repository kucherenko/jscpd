
shjs = require 'shelljs'
JSHINT = require('jshint').JSHINT
Lexer = require('jshint/src/stable/lex').Lexer
crypto = require 'crypto'

Clone = require('./clone.coffee').Clone

class Strategy

  constructor: ->
    @skippedTokens = []
    @codeHashes = {}
    @tokenTypes = []

  detect: (map, file, minLines, minTokens) ->
    console.log file
    code = shjs.cat(file)
    lines = code.split '\n'
    map.numberOfLines =  map.numberOfLines + lines.length
    JSHINT(code, {}, {})
    tokens = []
    lexer = new Lexer(code)
    lexer.start()
    currentMap = ""
    tokensPositions = []
    loop
      token = lexer.token()
      tokens.push token
      tokensPositions.push token.line
      md5 = crypto.createHash('md5');
      currentMap = currentMap + String.fromCharCode(@getTokenTypeId(token.type)) + md5.update(token.value).digest('hex').substring(0, 8);
      break if token.type is "(end)"

    firstLine = 0
    tokenNumber = 0
    isClone = false
    while tokenNumber <= tokensPositions.length - minTokens
      md5 = crypto.createHash('md5');
      hash = md5.update(currentMap.substring(tokenNumber * 9, minTokens * 9)).digest('hex').substring(0, 8);
      if hash of @codeHashes
        isClone = true
        if firstLine is 0
          firstLine = tokensPositions[tokenNumber]
          firstHash = hash
          firstToken = tokenNumber
      else
        if isClone
          fileA = @codeHashes[firstHash].file
          firstLineA = @codeHashes[firstHash].line
          lastToken = tokenNumber + minTokens - 2
          lastLine = tokensPositions[tokenNumber]
          numLines = lastLine + 1 - firstLine
          if numLines >= minLines and (fileA isnt file or firstLineA isnt firstLine)
            map.addClone new Clone(fileA, file, firstLineA, firstLine, numLines, lastToken-firstToken+1)
          firstLine = 0
          isClone = false
        @codeHashes[hash] = line: tokensPositions[tokenNumber], file: file
      tokenNumber = tokenNumber + 1
    if isClone
      fileA = @codeHashes[firstHash].file
      firstLineA = @codeHashes[firstHash].line
      lastToken = tokenNumber + minTokens - 2
      lastLine = tokensPositions[tokenNumber]
      numLines = lastLine + 1 - firstLine
      if numLines >= minLines and (fileA isnt file or firstLineA isnt firstLine)
        map.addClone new Clone(fileA, file, firstLineA, firstLine, numLines, lastToken-firstToken+1)
      isClone = false

  getTokenTypeId: (name) ->
    result = 0
    if name in @tokenTypes
      keys = [0..@tokenTypes.length]
      result = key for key in keys when @tokenTypes[key] is name
    else
      result = @tokenTypes.length
      @tokenTypes.push name
    result

exports.Strategy = Strategy
