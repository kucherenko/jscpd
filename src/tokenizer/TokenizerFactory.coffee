
TokenizerJS = require './TokenizerJS'
TokenizerCoffee = require './TokenizerCoffee'

class TokenizerFactory

  makeTokenizer: (filename, supportedLanguages) ->
    matches = filename.match /\.(\w*)$/
    extension = matches[1]?.toLowerCase()

    unless extension in supportedLanguages
      return no

    switch extension
      when "js" then new TokenizerJS()
      when "coffee" then new TokenizerCoffee()

module.exports = TokenizerFactory