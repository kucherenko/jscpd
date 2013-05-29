
class Map
  constructor: ->
    @clones = []
    @clonesByFile = []
    @position = 0
    @numberOfDuplication = 0
    @numberOfLInes = 0

  addClone: (clone)->
    console.log clone