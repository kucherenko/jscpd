require '../bootstrap'

tv4 = require 'tv4'
fs = require 'fs'
assert = require 'assert'
jscpd = require "#{sourcePath}jscpd"

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
  'htmlmixed'
]
schema = fs.readFileSync './test/reporters/json-report.schema.json'
schema = JSON.parse schema

describe "json reporter", ->

  it "exists", ->
    expect(jscpd::run).to.be.a 'function'

  using 'Supported languages', supportedLanguages, (language) ->
    it "run for #{language} files", (done)->
      result = jscpd::run
        path: "test/fixtures/"
        languages: [language]
        reporter: 'json'

      result.report.should.be.exist
      result.map.should.be.exist

      json = result.report
      isValid = tv4.validate json, schema
      msg = tv4.error.message unless isValid

      assert.ok isValid, msg

      done()
