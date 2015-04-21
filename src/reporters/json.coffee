_ = require 'underscore'

# @map {Object} report object
# @options {Object} Report class options
module.exports = ->

  duplicates = []

  for clone in @map.clones
    do (clone) ->

      duplicates.push

        lines: clone.linesCount
        tokens: clone.tokensCount
        firstFile:
          start: clone.firstFileStart
          name: clone.firstFile
        secondFile:
          start: clone.secondFileStart
          name: clone.secondFile
        fragment: _.escape(clone.getLines())

  json =
    duplicates: duplicates
    statistics:
      clones: @map.clones.length
      duplications: @map.numberOfDuplication
      files: @map.numberOfFiles
      percentage: @map.getPercentage()
      lines: @map.numberOfLines

  [
    json
    JSON.stringify(json)
  ]
