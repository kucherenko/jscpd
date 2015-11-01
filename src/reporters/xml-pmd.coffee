_ = require 'underscore'

# @map {Object} report object
# @options {Object} Report class options
module.exports = (options) ->
  xmlDoc = "<?xml version='1.0' encoding='UTF-8' ?>"
  if options['xsl-href'] then xmlDoc += '<?xml-stylesheet type="text/xsl" href="' + options['xsl-href'] + '"?>'
  xmlDoc += "<pmd-cpd>"

  for clone in @map.clones
    do (clone) ->
      xmlDoc = "#{xmlDoc}
        <duplication lines='#{clone.linesCount}' tokens='#{clone.tokensCount}'>
            <file path='#{clone.firstFile}' line='#{clone.firstFileStart}'/>
            <file path='#{clone.secondFile}' line='#{clone.secondFileStart}'/>
            <codefragment>#{_.escape(clone.getLines())}</codefragment>
        </duplication>"
  xmlDoc = xmlDoc + "</pmd-cpd>"

  [
    xmlDoc
  ]
