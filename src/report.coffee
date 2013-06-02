
class Report

  constructor: (@options) ->

  generate: (@map) ->
    result = ""
    for clone in @map.clones
      do (clone) ->
        result = result + "\n\t- #{clone.firstFile}:#{clone.firstFileStart}-#{clone.firstFileStart + clone.linesCount}\n\t" +
                              "  #{clone.secondFile}:#{clone.secondFileStart}-#{clone.secondFileStart + clone.linesCount}\n\t"
    result = "Found #{@map.clones.length} exact clones with #{@map.numberOfDuplication} duplicated lines in #{@map.numberOfFiles} files\n #{result}"
    console.log result

exports.Report = Report