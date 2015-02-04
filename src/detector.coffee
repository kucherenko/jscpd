
Map = require('./map').Map

class Detector

  constructor: (@strategy) ->

  start: (files = [], minLines = 5, minTokens = 70)->
    map = new Map()
    @strategy.detect map, file, minLines, minTokens for file in files
    map

exports.Detector = Detector
