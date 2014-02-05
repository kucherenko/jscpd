vm = require("vm")
path = require("path")
fs = require("fs")
CodeMirror = require("codemirror/addon/runmode/runmode.node.js")
module.exports = CodeMirror

CodeMirror.loadMode = (name) ->
  filename = require.resolve("codemirror/mode/" + name + "/" + name + ".js")
  modeDef = undefined
  try
    modeDef = fs.readFileSync(filename, "utf8")
  catch err
    throw new Error(name + " mode isn't shipped with CodeMirror")
  vm.runInNewContext modeDef,
    CodeMirror: CodeMirror

  return

CodeMirror.tokenize = (code) ->
  CodeMirror.runMode code, "javascript", (text, style, lineNumber) ->
    if text is "\n"
      style = "new_line"
      lineNumber = ''
    style = style ? ''
    console.log "#{lineNumber} #{style} #{text}"


code = fs.readFileSync '/tmp/hightlight/src/highlight.js', 'utf-8'

CodeMirror.loadMode 'javascript'

CodeMirror.tokenize code
