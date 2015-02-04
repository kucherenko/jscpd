shjs = require 'shelljs'
TokenizerFactory = require './tokenizer/TokenizerFactory'
crypto = require 'crypto'

Storage = require './storage/StorageMemory'

Clone = require('./clone').Clone

class Strategy

  constructor: (languages) ->
    @languages = languages
    @storage = new Storage()

  detect: (map, file, @minLines, @minTokens) ->
    tokenizer = TokenizerFactory::makeTokenizer file, @languages
    unless tokenizer
      return no
    language = tokenizer.getType()

    if shjs.test '-f', file
      code = shjs.cat file
    else
      return no

    lines = code.split '\n'
    map.numberOfLines =  map.numberOfLines + lines.length

    {tokensPositions, currentMap} = tokenizer.tokenize(code).generateMap()

    firstLine = 0
    tokenNumber = 0
    isClone = false

    while tokenNumber <= tokensPositions.length - @minTokens
      mapFrame = currentMap.substring tokenNumber * 33, tokenNumber * 33 + @minTokens * 33
      hash = crypto.createHash('md5').update(mapFrame).digest('hex').substring 0, 8
      if @storage.hasHash hash, language
        isClone = true
        if firstLine is 0
          firstLine = tokensPositions[tokenNumber]
          firstHash = hash
          firstToken = tokenNumber
      else
        if isClone
          lastToken = tokenNumber + @minTokens - 2
          @addClone(
            map,
            file,
            firstHash,
            firstToken,
            lastToken,
            firstLine,
            tokensPositions[lastToken],
            language
          )
          firstLine = 0
          isClone = false
        @storage.addHash hash, file, tokensPositions[tokenNumber], language
      tokenNumber = tokenNumber + 1

    if isClone
      lastToken = tokenNumber + @minTokens - 2
      @addClone(
        map,
        file,
        firstHash,
        firstToken,
        lastToken,
        firstLine,
        tokensPositions[lastToken],
        language
      )
      isClone = false

  addClone: (map, file, hash, firstToken, lastToken, firstLine, lastLine, language) ->
    hashInfo = @storage.getHash(hash, language)
    fileA = hashInfo.file
    firstLineA = hashInfo.line
    numLines = lastLine + 1 - firstLine
    if numLines >= @minLines and (fileA isnt file or firstLineA isnt firstLine)
      map.addClone new Clone(
        fileA,
        file,
        firstLineA,
        firstLine,
        numLines,
        lastToken - firstToken + 1
      )


exports.Strategy = Strategy
