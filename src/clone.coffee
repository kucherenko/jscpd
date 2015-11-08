shjs = require "shelljs"

class Clone
  constructor: (
    @firstFile,
    @secondFile,
    @firstFileStart,
    @secondFileStart,
    @linesCount,
    @tokensCount)->

  getLines: (isFirstFile = yes) ->
    code = shjs.cat(if isFirstFile then @firstFile else @secondFile)
    start = if isFirstFile then @firstFileStart else @secondFileStart
    lines = code.split '\n'
    end = start + @linesCount
    lines[start..end].join("\n")

exports.Clone = Clone
