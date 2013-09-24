combine = ->
  new RegExp("(" + [].slice.call(arguments).map((e) ->
    e = e.toString()
    "(?:" + e.substring(1, e.length - 1) + ")"
  ).join("|") + ")")

pattern =
  string1: /"(?:(?:\\\n|\\"|[^"\n]))*?"/
  string2: /'(?:(?:\\\n|\\'|[^'\n]))*?'/
  comment1: /\/\*[\s\S]*?\*\//
  comment2: /\/\/.*?\n/
  keyword: /\b(?:break|case|catch|continue|debugger|default|delete|do|else|finally|for|function|if|in|instanceof|new|return|switch|this|throw|try|typeof|var|void|while|with)\b/
  regexp: /\/(?:(?:\\\/|[^\/]))*?\//
  name: /[a-zA-Z_\$][a-zA-Z_\$0-9]*/
  number: /-?\d+(?:\.\d+)?(?:e[+-]?\d+)?/
  punct: /[;.:\?\^%()\{\}?\[\]<>=!&|+\-,]/
  newline: /\n/
  whitespace: /\s+/

match = combine (pattern[p] for p of pattern)...

getType = (e) ->
  for type of pattern
    return type if pattern[type].test(e)
  "invalid"

module.exports = (str, doNotThrow = yes) ->
  linesCount = 1
  result = str.split(match).filter (e, i) ->
    return true if i % 2
    if e isnt ""
      throw new Error "invalid token:#{JSON.stringify(e)}" unless doNotThrow
      true
  result.map (e) ->
    token =
      first_line: linesCount
      value: e
      type: getType e
    linesCount = linesCount + e.split('\n').length - 1
    token
