_ = require 'underscore'
stdLog = require './_std-log'

module.exports = (map, options) ->

    vlog = ''
    xmlDoc = "<?xml version='1.0' encoding='UTF-8' ?><pmd-cpd>"
    verbose = options.verbose

    for clone in map.clones
        do (clone) ->
            vlog = "#{vlog}
                \n\t-#{clone.firstFile}:#{clone.firstFileStart}-#{clone.firstFileStart + clone.linesCount}\n\t
                #{clone.secondFile}:#{clone.secondFileStart}-#{clone.secondFileStart + clone.linesCount}\n\t"

            vlog = "#{vlog}\n#{clone.getLines()}" if verbose

            xmlDoc = "#{xmlDoc}
                <duplication lines='#{clone.linesCount}' tokens='#{clone.tokensCount}'>
                <file path='#{clone.firstFile}' line='#{clone.firstFileStart}'/>
                <file path='#{clone.secondFile}' line='#{clone.secondFileStart}'/>
                <codefragment>#{_.escape(clone.getLines())}</codefragment>
                </duplication>"

    xmlDoc = xmlDoc + "</pmd-cpd>"

    log = stdLog(
        @map.clones.length,
        @map.numberOfDuplication,
        @map.numberOfFiles,
        @map.numberOfLines,
        @map.getPercentage(),
        vlog
    )

    [xmlDoc, log]
