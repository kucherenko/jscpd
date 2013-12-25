
TokenizerJS = require './TokenizerJS'
TokenizerCoffee = require './TokenizerCoffee'

class TokenizerFactory

  makeTokenizer: (filename) ->
    matches = filename.match /\.(\w*)$/
    extension = matches[1]
    switch extension.toLowerCase()
      when "js" then new TokenizerJS()
      when "coffee" then new TokenizerCoffee()

module.exports = TokenizerFactory