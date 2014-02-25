
if process.env['COVERAGE']
  console.log 'COVERAGE mode is on'
  global.sourcePath = __dirname + '/../.tmp/'
else
  global.sourcePath = __dirname + '/../src/'


chai = require 'chai'
sinon = require 'sinon'
chai.should()
global.expect = chai.expect

logger = require 'winston'
logger.remove(logger.transports.Console);


global.using = (name, values, func) ->
  i = 0
  count = values.length
  _results = []
  while i < count
    values[i] = [values[i]]  if "[object Array]" isnt Object::toString.call(values[i])
    func.apply this, values[i]
    _results.push i++
  _results

beforeEach ->
  global.env = sinon.sandbox.create()

afterEach ->
  global.env.restore()