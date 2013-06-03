jade = require('jade').runtime
fs = require 'fs'
class Report

  constructor: (@options) ->

  generate: (@map) ->
    result = ""
    xmlDoc = false
    if @options.output
      xmlDoc = "<?xml version='1.0' encoding='UTF-8' ?><pmd-cpd>"
    verbose = @options.verbose
    for clone in @map.clones
      do (clone) ->
        result = result + "\n\t- #{clone.firstFile}:#{clone.firstFileStart}-#{clone.firstFileStart + clone.linesCount}\n\t" +
                              "  #{clone.secondFile}:#{clone.secondFileStart}-#{clone.secondFileStart + clone.linesCount}\n\t"
        result = "#{result}\n#{clone.getLines()}" if verbose

        if xmlDoc
          xmlDoc = xmlDoc +
                   "<duplication lines='" + clone.linesCount + "' tokens='" + clone.tokensCount + "'>" +
                   "<file path='" + clone.firstFile + "' line='" + clone.firstFileStart + "'/>" +
                   "<file path='" + clone.secondFile + "' line='" + clone.secondFileStart + "'/>" +
                   "<codefragment>" + jade.escape(clone.getLines()) + "</codefragment></duplication>"

    if xmlDoc
      xmlDoc = xmlDoc + "</pmd-cpd>";
      fs.writeFileSync(@options.output, xmlDoc)

    result = "Found #{@map.clones.length} exact clones with #{@map.numberOfDuplication} duplicated lines in #{@map.numberOfFiles} files\n #{result}"

    console.log "#{result}\n\n #{@map.getPercentage()}% (#{@map.numberOfDuplication} lines) duplicated lines out of #{@map.numberOfLines} total lines of code.\n"


exports.Report = Report
