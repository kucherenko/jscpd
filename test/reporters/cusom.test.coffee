require '../bootstrap'

assert = require 'assert'
jscpd = require "#{sourcePath}jscpd"

describe "cusom reporter", ->

  it "exists", ->
    expect(jscpd::run).to.be.a 'function'

  it "run custom report on go files", (done)->
    result = jscpd::run
      path: "test/fixtures/"
      languages: ['go']
      reporter: '../test/reporters/custom-reporter'

    result.report.should.be.exist
    result.map.should.be.exist

    report = result.report
    assert.equal('this_is_a_custom_report', report)

    done()
