global.sourcePath = __dirname + '/../src/'

chai = require 'chai'
sinon = require 'sinon'
sinonChai = require 'sinon-chai'

chai.should()
chai.use sinonChai

logger = require 'winston'
logger.remove logger.transports.Console

global.expect = chai.expect

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
