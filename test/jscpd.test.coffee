if process.env['COVERAGE']
  console.log 'COVERAGE mode is on'
  jscpd = require '../.tmp/jscpd'
else
  jscpd = require '../index'
expect = require('chai').expect
should = require('chai').should()
{parseString} = require 'xml2js'
logger = require 'winston'
logger.remove(logger.transports.Console);

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

describe "jscpd", ->
  it "exists", ->
    expect(jscpd::run).to.be.a 'function'

  it "run for javascript files", (done)->
    xml = jscpd::run
      path: "test/fixtures/"
      languages: ['javascript']

    expect(xml, 'xml').to.be.exist
    parseString xml, (err, result)->
      expect(err, 'error').to.be.null
      expect(result, 'result').to.not.be.null

      checkXmlStruct result
      result['pmd-cpd'].duplication.should.have.length 4

      done()

  it "run for all supported files", (done)->
    xml = jscpd::run
      path: "test/fixtures/"
      languages: ['javascript', 'coffeescript']

    expect(xml, 'xml').to.be.exist
    parseString xml, (err, result)->
      expect(err, 'error').to.be.null
      expect(result, 'result').to.not.be.null

      checkXmlStruct result
      result['pmd-cpd'].duplication.should.have.length 5

      done()

  it "run for coffeescript files", (done)->
    xml = jscpd::run
      path: "test/fixtures/"
      languages: ['coffeescript']


    expect(xml, 'xml').to.be.exist
    parseString xml, (err, result)->
      expect(err, 'error').to.be.null
      expect(result, 'result').to.not.be.null

      checkXmlStruct result
      result['pmd-cpd'].duplication.should.have.length 1

      done()

  it "run for python files", (done)->
    xml = jscpd::run
      path: "test/fixtures/"
      languages: ['python']


    expect(xml, 'xml').to.be.exist
    parseString xml, (err, result)->
      expect(err, 'error').to.be.null
      expect(result, 'result').to.not.be.null

      checkXmlStruct result
      result['pmd-cpd'].duplication.should.have.length 1

      done()

  it "run for php files", (done)->
    xml = jscpd::run
      path: "test/fixtures/"
      languages: ['php']


    expect(xml, 'xml').to.be.exist
    parseString xml, (err, result)->
      expect(err, 'error').to.be.null
      expect(result, 'result').to.not.be.null

      checkXmlStruct result
      result['pmd-cpd'].duplication.should.have.length 1

      done()

  it "run for less files", (done)->
    xml = jscpd::run
      path: "test/fixtures/"
      languages: ['less']


    expect(xml, 'xml').to.be.exist
    parseString xml, (err, result)->
      expect(err, 'error').to.be.null
      expect(result, 'result').to.not.be.null

      checkXmlStruct result
      result['pmd-cpd'].duplication.should.have.length 1

      done()
