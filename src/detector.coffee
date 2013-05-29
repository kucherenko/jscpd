

class Detecor

  constructor: (@strategy) ->

  start: (files = [], minLines = 5, minTokens = 70)->
    @strategy.detect(file, minLines, minTokens) for file in files