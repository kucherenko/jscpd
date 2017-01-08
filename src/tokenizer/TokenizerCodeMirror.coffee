vm = require "vm"
fs = require "fs"

TokenizerBase = require './TokenizerBase'
logger = require 'winston'
CodeMirror = require("codemirror/addon/runmode/runmode.node.js")

CodeMirror.loadMode = (name) ->
  filename = require.resolve("codemirror/mode/" + name + "/" + name + ".js")
  require filename

class TokenizerCodeMirror extends TokenizerBase
  @type = null

  setTypeAndMode: (language) ->
    switch language
      when "csharp", "java"
        @type = 'clike'
        @mode = "text/#{language}"
      when 'typescript'
        @type = 'javascript'
        @mode = "text/#{language}"
      when 'jsx'
        @type = 'javascript'
        @mode = "text/javascript"
      else
        @type = language
        @mode = language


  loadType: (type) ->
    try
      CodeMirror.loadMode type
    catch e
      if e.code is 'MODULE_NOT_FOUND'
        logger.debug "#{e}"
        console.error "JSCPD Error 01: #{type} in not supported"
    @

  tokenize: (code) =>
    @tokens = []

    @loadType @type

    CodeMirror.runMode code, @mode, (value, tokenType, lineNumber) =>
      return if not lineNumber
      tokenType = if @isEmptyToken value then 'empty' else tokenType
      tokenType = tokenType ? 'default'
      @tokens.push [tokenType, value, lineNumber]
    @

  getType: -> @type

  generateMap: ->
    currentMap = ""
    tokensPositions = []
    for [type, value, lineNumber] in @tokens when @validToken type
      tokensPositions.push lineNumber
      currentMap = currentMap + @createMap type, value

    tokensPositions: tokensPositions, currentMap: currentMap


module.exports = TokenizerCodeMirror
