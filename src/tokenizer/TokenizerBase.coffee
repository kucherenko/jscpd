
crypto = require 'crypto'

class TokenizerBase

  constructor: ->
    @tokenTypes = []

  tokenize: (code) ->

  getType: () ->

  generateMap: () ->

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


module.exports = TokenizerBase