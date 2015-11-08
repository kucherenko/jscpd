_ = require 'underscore'

# @map {Object} report object
# @options {Object} Report class options
module.exports = (options) ->
  xmlDoc = "<?xml version='1.0' encoding='UTF-8' ?>"
  if options['xsl-href'] then xmlDoc += '<?xml-stylesheet type="text/xsl" href="' + options['xsl-href'] + '"?>'
  xmlDoc += '<pmd-cpd>'

  for clone in @map.clones
    do (clone) ->
      firstFragment = _.escape clone.getLines(yes)
      secondFragment = _.escape clone.getLines(no)
      xmlDoc = "#{xmlDoc}
        <duplication lines='#{clone.linesCount}' tokens='#{clone.tokensCount}'>
            <file path='#{clone.firstFile}' line='#{clone.firstFileStart}'>
              <codefragment><![CDATA[#{firstFragment}]]></codefragment>
            </file>
            <file path='#{clone.secondFile}' line='#{clone.secondFileStart}'>
              <codefragment><![CDATA[#{secondFragment}]]></codefragment>
            </file>
            <codefragment><![CDATA[#{firstFragment}]]></codefragment>
        </duplication>"
  xmlDoc = xmlDoc + '</pmd-cpd>'

  [
    xmlDoc
  ]
