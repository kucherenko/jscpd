
class Clone
  constructor: (@firstFile, @secondFile, @firstFileStart, @secondFileStart, @linesCount, @tokensCount)->

  getLines: ->
    console.log "getLines"

exports.Clone = Clone