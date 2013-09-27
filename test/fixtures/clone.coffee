shjs = require "shelljs"

class Clone
  constructor: (@firstFile, @secondFile, @firstFileStart, @secondFileStart, @linesCount, @tokensCount)->

  getLines: ->
    code = shjs.cat(@firstFile)
    lines = code.split '\n'
    start = @firstFileStart
    end = start + @linesCount
    lines[start..end].join("\n")

exports.Clone = Clone


class Map
  constructor: ->
    @clones = []
    @clonesByFile = {}
    @numberOfDuplication = 0
    @numberOfLines = 0
    @numberOfFiles = 0

  addClone: (clone)->
    @clones.push clone
    @numberOfDuplication = @numberOfDuplication + clone.linesCount

    if clone.firstFile of @clonesByFile
      @clonesByFile[clone.firstFile].push clone.firstFile
    else
      @clonesByFile[clone.firstFile] = [clone.firstFile]
      @numberOfFiles++

    if clone.secondFile of @clonesByFile
      @clonesByFile[clone.secondFile].push clone
    else
      @clonesByFile[clone.secondFile] = [clone]
      @numberOfFiles++

  getPercentage: ->
    result = 100
    if @numberOfLines > 0
      result = @numberOfDuplication / @numberOfLines * 100
    result.toFixed 2


exports.Map = Map
