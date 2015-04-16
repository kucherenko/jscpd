_ = require 'underscore'

module.exports = (map, options) ->

    log = ''
    xmlDoc = "<?xml version='1.0' encoding='UTF-8' ?><pmd-cpd>"
    verbose = options.verbose

    for clone in map.clones
        do (clone) ->
            log = "{#{log}
                \n\t-#{clone.firstFile}:#{clone.firstFileStart}-#{clone.firstFileStart + clone.linesCount}\n\t
                #{clone.secondFile}:#{clone.secondFileStart}-#{clone.secondFileStart + clone.linesCount}\n\t"

            log = "#{log}\n#{clone.getLines()}" if verbose

            xmlDoc = "#{xmlDoc}
            <duplication lines='#{clone.linesCount}' tokens='#{clone.tokensCount}'>
            <file path='#{clone.firstFile}' line='#{clone.firstFileStart}'/>
            <file path='#{clone.secondFile}' line='#{clone.secondFileStart}'/>
            <codefragment>#{_.escape(clone.getLines())}</codefragment>
            </duplication>"

    xmlDoc = xmlDoc + "</pmd-cpd>"

    [xmlDoc, log]
