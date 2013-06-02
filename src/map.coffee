
class Map
  constructor: ->
    @clones = []
    @clonesByFile = {}
    @position = 0
    @numberOfDuplication = 0
    @numberOfLines = 0

  addClone: (clone)->
    @clones.push clone

exports.Map = Map