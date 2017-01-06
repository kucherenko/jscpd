
TokenizerCodeMirror = require './TokenizerCodeMirror'

class TokenizerFactory

  tokenizers: {}

  LANGUAGES:
    javascript: ['js', 'es', 'es6']
    typescript: ['ts', 'tsx']
    jsx: ['jsx']
    haxe: ['hx', 'hxml']
    coffeescript: ['coffee']
    ruby: ['rb']
    php: ['php', 'phtml']
    python: ['py']
    css: ['less', 'css']
    sass: ['scss']
    java: ['java']
    csharp: ['cs']
    go: ['go']
    clike: ['cpp', 'c', 'm', 'h']
    htmlmixed: ['html', 'htm']
    yaml: ['yaml', 'yml']
    erlang: ['erl', 'erlang']
    swift: ['swift']
    xml: ['xml', 'xsl', 'xslt']
    puppet: ['pp', 'puppet']
    twig: ['twig']

  getLanguageByExtension: (extension) ->
    for language of TokenizerFactory::LANGUAGES
      return language if extension in TokenizerFactory::LANGUAGES[language]
    return no

  getExtensionsByLanguages: (languages) ->
    languages = [languages] if typeof languages is 'string'
    result = []
    for language of TokenizerFactory::LANGUAGES when language in languages
      result.push TokenizerFactory::LANGUAGES[language]...
    return result

  makeTokenizer: (filename, supportedLanguages) ->
    extension = ''
    matches = filename.match /\.(\w*)$/
    extension = matches[1]?.toLowerCase() if matches

    language = TokenizerFactory::getLanguageByExtension extension

    return off if language not in supportedLanguages

    if language not of TokenizerFactory::tokenizers
      switch language
        when "csharp", "java", "csrc"
          TokenizerFactory::tokenizers[language] =  new TokenizerCodeMirror()
          TokenizerFactory::tokenizers[language].setType "text/x-#{language}"
        when "typescript", 'jsx'
          TokenizerFactory::tokenizers[language] =  new TokenizerCodeMirror()
          TokenizerFactory::tokenizers[language].setType "javascript"
        else
          TokenizerFactory::tokenizers[language] = new TokenizerCodeMirror()
          TokenizerFactory::tokenizers[language].setType language

    TokenizerFactory::tokenizers[language]

module.exports = TokenizerFactory
