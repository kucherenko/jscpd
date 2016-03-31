require '../bootstrap'

jscpd = require "#{sourcePath}jscpd"

describe "custom reporter", ->

  it "run custom coffee reporter on go files", (done)->
    jscpd::run(
      path: "test/fixtures/"
      languages: ['go']
      reporter: './test/reporters/custom-reporter.coffee'
      output: ''
      blame: on
      debug: off
    ).then (result) ->
      result.report.should.be.exist
      result.map.should.be.exist

      report = result.report
      report.should.equal 'this_is_a_custom_report_raw'

      done()

  it "run custom javascript reporter on go files", (done)->
    result = jscpd::run
      path: "test/fixtures/"
      languages: ['go']
      reporter: process.cwd() + '/test/reporters/custom-reporter.js'
      output: ''
      blame: off
      debug: off

    result.report.should.be.exist
    result.map.should.be.exist
    report = result.report
    report.should.equal 'this_is_a_custom_report_raw_js'
    done()
