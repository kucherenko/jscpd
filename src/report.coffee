fs = require 'fs'
logger = require 'winston'


class Report

  constructor: (@options) ->

  generate: (@map) ->
    result = ""
    xmlDoc = "<?xml version='1.0' encoding='UTF-8' ?><pmd-cpd>"
    verbose = @options.verbose
    
    for clone in @map.clones
      do (clone) ->
        result = result + "\n\t-
#{clone.firstFile}:#{clone.firstFileStart}-#{clone.firstFileStart + clone.linesCount}\n\t
#{clone.secondFile}:#{clone.secondFileStart}-#{clone.secondFileStart + clone.linesCount}\n\t"

        result = "#{result}\n#{clone.getLines()}" if verbose
        xmlDoc = "#{xmlDoc}
<duplication lines='#{clone.linesCount}' tokens='#{clone.tokensCount}'>
<file path='#{clone.firstFile}' line='#{clone.firstFileStart}'/>
<file path='#{clone.secondFile}' line='#{clone.secondFileStart}'/>
<codefragment>#{htmlspecialchars(clone.getLines())}</codefragment>
</duplication>"

    xmlDoc = xmlDoc + "</pmd-cpd>"
    fs.writeFileSync(@options.output, xmlDoc) if @options.output

    result = "Found #{@map.clones.length} exact clones with
 #{@map.numberOfDuplication} duplicated lines in
 #{@map.numberOfFiles} files\n #{result}"

    logger.info "#{result}\n\n
#{@map.getPercentage()}% (#{@map.numberOfDuplication} lines)
 duplicated lines out of
 #{@map.numberOfLines} total lines of code.\n"

    return xmlDoc


htmlspecialchars = (str) ->
  if (typeof(str) == "string")
    str = str.replace(/&/g, "&amp;")
    str = str.replace(/"/g, "&quot;")
    str = str.replace(/'/g, "&#039;")
    str = str.replace(/</g, "&lt;")
    str = str.replace(/>/g, "&gt;")
  str


exports.Report = Report
