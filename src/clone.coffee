fs = require 'fs'
Blamer = require 'blamer'
Promise = require 'bluebird'

class Clone
  constructor: (
    @firstFile,
    @secondFile,
    @firstFileStart,
    @secondFileStart,
    @linesCount,
    @tokensCount)->
      @firstFileAnnotatedCode = {}
      @secondFileAnnotatedCode = {}


  getLines: (isFirstFile = yes) ->
    code = fs.readFileSync(if isFirstFile then @firstFile else @secondFile)
    start = if isFirstFile then @firstFileStart else @secondFileStart
    lines = code.toString().split '\n'
    end = start + @linesCount
    lines[start..end].join("\n")

  blame: ->
    blamer = new Blamer
    Promise.all([
      blamer.blameByFile(@firstFile)
      blamer.blameByFile(@secondFile)
    ]).then (results) =>
      for line, annotation of results[0][@firstFile] when @lineInRange(line, @firstFileStart)
        @firstFileAnnotatedCode[line] = annotation
      for line, annotation of results[1][@secondFile] when @lineInRange(line, @secondFileStart)
        @secondFileAnnotatedCode[line] = annotation
      return @

  lineInRange: (line, fileStart) -> 0 + line >= fileStart and 0 + line <= fileStart + @linesCount

exports.Clone = Clone
