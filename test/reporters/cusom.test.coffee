require '../bootstrap'

assert = require 'assert'
jscpd = require "#{sourcePath}jscpd"

describe "cusom reporter", ->

  it "exists", ->
    expect(jscpd::run).to.be.a 'function'

  it "run custom coffee reporter on go files", (done)->
    result = jscpd::run
      path: "test/fixtures/"
      languages: ['go']
      reporter: './test/reporters/custom-reporter.coffee'

    result.report.should.be.exist
    result.map.should.be.exist

    report = result.report
    assert.equal('this_is_a_custom_report_raw', report)

    done()

  it "run custom javascript reporter on go files", (done)->
    result = jscpd::run
      path: "test/fixtures/"
      languages: ['go']
      reporter: process.cwd() + '/test/reporters/custom-reporter.js'

    result.report.should.be.exist
    result.map.should.be.exist

    report = result.report
    assert.equal('this_is_a_custom_report_raw_js', report)

    done()
