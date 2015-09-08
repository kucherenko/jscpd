vm = require "vm"
fs = require "fs"

TokenizerBase = require './TokenizerBase'
logger = require 'winston'
CodeMirror = require("codemirror/addon/runmode/runmode.node.js")

CodeMirror.loadMode = (name) ->
  filename = require.resolve("codemirror/mode/" + name + "/" + name + ".js")
  modeDef = null
  try
    modeDef = fs.readFileSync(filename, "utf8")
  catch err
    throw new Error(name + " mode isn't shipped with CodeMirror")
  vm.runInNewContext modeDef,
    CodeMirror: CodeMirror

CodeMirror.loadMode 'xml'
CodeMirror.loadMode 'clike'

class TokenizerCodeMirror extends TokenizerBase
  @type = null

  setType: (type) ->
    @type = type

  loadType: (type) ->
    try
      CodeMirror.loadMode type
    catch e
      if e.code is 'MODULE_NOT_FOUND'
        logger.debug "#{e}"

    @

  tokenize: (code) =>
    @tokens = []

    @loadType @type

    CodeMirror.runMode code, @type, (value, tokenType, lineNumber) =>
      return if not lineNumber
      tokenType = tokenType ? 'default'
      @tokens.push [tokenType, value, lineNumber]
    @

  getType: -> @type

  generateMap: ->
    currentMap = ""
    tokensPositions = []
    for [type, value, lineNumber] in @tokens
      tokensPositions.push lineNumber
      currentMap = currentMap + @createMap type, value

    tokensPositions: tokensPositions, currentMap: currentMap


module.exports = TokenizerCodeMirror
