require '../bootstrap'

jscpd = require "#{sourcePath}jscpd"

{parseString} = require 'xml2js'

supportedLanguages = [
  'javascript'
  'jsx'
  'haxe'
  'coffeescript'
  'typescript'
  'python'
  'php'
  'css'
  'go'
  'clike'
  'ruby'
  'java'
  'csharp'
  'htmlmixed'
  'yaml'
]

checkXmlStruct = (parsedXML)->
  parsedXML.should.have.property 'pmd-cpd'
  parsedXML['pmd-cpd'].should.have.property 'duplication'
  parsedXML['pmd-cpd'].duplication.should.have.length.above 0

  duplication = parsedXML['pmd-cpd'].duplication[0]
  dAttr = duplication.$
  dAttr.should.have.property 'lines'
  dAttr.should.have.property 'tokens'
  duplication.should.have.property 'file'
  duplication.should.have.property 'codefragment'

  files = duplication.file
  files.should.have.length 2

  file = files[0]
  file.$.should.have.property 'path'
  file.$.should.have.property 'line'

describe "xml reporter", ->
  context "Blame authors", ->
    using 'Supported languages ', supportedLanguages, (language) ->
      it "run for #{language} files", (done)->
        jscpd::run(
          path: "test/fixtures/"
          languages: [language]
          reporter: 'xml'
          # override what is in base .cpd.yaml
          files: '**/*.*'
          exclude: []
          output: ''
          blame: on
        ).then (xml) ->
          xml.report.should.be.exist
          xml.map.should.be.exist

          parseString xml.report, (err, result)->
            expect(err, 'error').to.be.null
            expect(result, 'result').to.not.be.null
            checkXmlStruct result
            result['pmd-cpd'].duplication.should.not.have.length 0
            done()

  context "Not Blame authors", ->
    using 'Supported languages', supportedLanguages, (language) ->
      it "run for #{language} files", (done)->
        jsCPD = new jscpd()
        xml = jsCPD.run
          path: "test/fixtures/"
          languages: [language]
          reporter: 'xml'
          # override what is in base .cpd.yaml
          files: '**/*.*'
          exclude: []
          output: ''
          blame: off

        xml.report.should.be.exist
        xml.map.should.be.exist

        parseString xml.report, (err, result)->
          expect(err, 'error').to.be.null
          expect(result, 'result').to.not.be.null
          checkXmlStruct result
          result['pmd-cpd'].duplication.should.not.have.length 0
          done()
