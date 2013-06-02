shjs = require 'shelljs'
JSHINT = require('jshint').JSHINT
Lexer = require('jshint/src/stable/lex').Lexer
crypto = require 'crypto'

Clone = require('./clone').Clone

class Strategy

  constructor: ->
    @codeHashes = {}
    @tokenTypes = []

  detect: (map, file, @minLines, @minTokens) ->
    code = shjs.cat(file)
    lines = code.split '\n'
    map.numberOfLines =  map.numberOfLines + lines.length
    JSHINT(code, {}, {})
    lexer = new Lexer(code)
    lexer.start()
    currentMap = ""
    tokensPositions = []
    loop
      token = lexer.token()
      tokensPositions.push token.line
      currentMap = currentMap +
        String.fromCharCode(@getTokenTypeId(token.type)) +
        crypto.createHash('md5').update(token.value).digest('hex').substring(0, 8)
      break if token.type is "(end)"

    firstLine = 0
    tokenNumber = 0
    isClone = false
    while tokenNumber <= tokensPositions.length - minTokens
      mapFrame = currentMap.substring(tokenNumber * 9, tokenNumber * 9 + minTokens * 9)
      hash = crypto.createHash('md5').update(mapFrame).digest('hex').substring(0, 8)

      if hash of @codeHashes
        isClone = true
        if firstLine is 0
          firstLine = tokensPositions[tokenNumber]
          firstHash = hash
          firstToken = tokenNumber
      else
        if isClone
          lastToken = tokenNumber + minTokens - 2
          @addClone(map, file, firstHash, firstToken, lastToken, firstLine, tokensPositions[lastToken])
          firstLine = 0
          isClone = false
        @codeHashes[hash] = line: tokensPositions[tokenNumber], file: file

      tokenNumber = tokenNumber + 1

    if isClone
      lastToken = tokenNumber + minTokens - 2
      @addClone(map, file, firstHash, firstToken, lastToken, firstLine, tokensPositions[lastToken])
      isClone = false

  addClone: (map, file, hash, firstToken, lastToken, firstLine, lastLine) ->
    fileA = @codeHashes[hash].file
    firstLineA = @codeHashes[hash].line
    numLines = lastLine + 1 - firstLine
    if numLines >= @minLines and (fileA isnt file or firstLineA isnt firstLine)
      map.addClone new Clone(fileA, file, firstLineA, firstLine, numLines, lastToken - firstToken + 1)

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
