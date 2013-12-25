TokenizerBase = require './TokenizerBase'
CoffeeTokens = require('coffee-script').tokens

class TokenizerCoffee extends TokenizerBase

  tokenize: (code) ->
    @tokens = CoffeeTokens code
    @

  getType: () -> 'coffee'

  generateMap: ->
    currentMap = ""
    tokensPositions = []
    for [type, value, options] in @tokens
      tokensPositions.push options.first_line
      currentMap = currentMap + @createMap(type, value)

    tokensPositions: tokensPositions, currentMap: currentMap


module.exports = TokenizerCoffee