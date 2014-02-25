
class StorageMemory

  constructor: ->
    @codeHashes = {}

  addHash: (hash, file, line, language)->
    @codeHashes[language] = @codeHashes[language] ? {}
    @codeHashes[language][hash] = line: line, file: file

  hasHash: (hash, language) ->
    @codeHashes[language] and hash of @codeHashes[language]

  getHash: (hash, language) ->
    @codeHashes[language]?[hash]


module.exports = StorageMemory