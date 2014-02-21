shjs = require 'shelljs'
TokenizerFactory = require './tokenizer/TokenizerFactory'
crypto = require 'crypto'

Clone = require('./clone').Clone

class Strategy

  constructor: (languages) ->
    @languages = languages
    @codeHashes = {}

  detect: (map, file, @minLines, @minTokens) ->
    tokenizer = TokenizerFactory::makeTokenizer file, @languages
    unless tokenizer
      return no
    language = tokenizer.getType()

    if (shjs.test('-f', file))
      code = shjs.cat(file)
    else
      return no

    lines = code.split '\n'
    map.numberOfLines =  map.numberOfLines + lines.length

    {tokensPositions, currentMap} = tokenizer.tokenize(code).generateMap()

    firstLine = 0
    tokenNumber = 0
    isClone = false
    @codeHashes[language] = {} unless @codeHashes[language]
    while tokenNumber <= tokensPositions.length - minTokens
      mapFrame = currentMap.substring tokenNumber * 9, tokenNumber * 9 + minTokens * 9
      hash = crypto.createHash('md5').update(mapFrame).digest('hex').substring 0, 8
      if hash of @codeHashes[language]
        isClone = true
        if firstLine is 0
          firstLine = tokensPositions[tokenNumber]
          firstHash = hash
          firstToken = tokenNumber
      else
        if isClone
          lastToken = tokenNumber + minTokens - 2
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
        @codeHashes[language][hash] = line: tokensPositions[tokenNumber], file: file

      tokenNumber = tokenNumber + 1

    if isClone
      lastToken = tokenNumber + minTokens - 2
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
    fileA = @codeHashes[language][hash].file
    firstLineA = @codeHashes[language][hash].line
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
