
jscpd = require '../src/jscpd'
expect = require('chai').expect
describe "jscpd", ->
  it "should can run proccess of searching duplication for array of files", ->
    expect(jscpd::run).to.be.a 'function'
