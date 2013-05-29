
fs = require 'fs'

class Strategy

  constructor: ->
    @tokensToIgnore = []
    @codeHashes = []

  detect: (file, minLines, minTokens, map) ->
    result = {}
    code = fs.read file