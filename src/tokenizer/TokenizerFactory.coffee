
TokenizerCoffee = require './TokenizerCoffee'
TokenizerCodeMirror = require './TokenizerCodeMirror'

class TokenizerFactory

  LANGUAGES:
    javascript: ['js']
    coffeescript: ['coffee']
    ruby: ['rb']
    php: ['php', 'phtml']
    python: ['py']
    css: ['css']
    less: ['less']


  getLanguageByExtension: (extension) ->
    for language of TokenizerFactory::LANGUAGES
      return language if extension in TokenizerFactory::LANGUAGES[language]
    return no

  getExtensionsByLanguages: (languages) ->
    languages = [languages] if typeof languages is 'string'
    result = []
    result.push TokenizerFactory::LANGUAGES[language]... for language of TokenizerFactory::LANGUAGES when language in languages
    return result

  makeTokenizer: (filename, supportedLanguages) ->
    matches = filename.match /\.(\w*)$/
    extension = matches[1]?.toLowerCase()

    language = TokenizerFactory::getLanguageByExtension extension

    return off if language not in supportedLanguages

    switch language
      when "coffeescript" then new TokenizerCoffee()
      else
        tokenizer = new TokenizerCodeMirror()
        tokenizer.setType(language)
        tokenizer

module.exports = TokenizerFactory