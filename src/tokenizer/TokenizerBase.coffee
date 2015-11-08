
crypto = require 'crypto'

class TokenizerBase

  constructor: ->
    @skipComments = no
    @tokenTypes = []

  tokenize: (code) ->

  getType: () ->

  generateMap: () ->

  isEmptyToken: (value) -> value.replace(/^\s+|\s+$/g, '').length is 0

  validToken: (type) ->
    (@type in ['coffeescript', 'python', 'ruby'] or type isnt 'empty') and (not @skipComments or type isnt 'comment')

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
