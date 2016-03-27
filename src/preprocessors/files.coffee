_ = require "underscore"
path = require "path"
glob = require "glob"

findFiles = (jscpd) ->
    files = []
    excluded_files = []

    for pattern in jscpd.options.patterns
      files = _.union files, glob.sync(pattern, cwd: jscpd.options.path)

    if jscpd.options.exclude.length > 0
      for pattern in jscpd.options.exclude
        excluded_files = _.union excluded_files, glob.sync(pattern, cwd: jscpd.options.path)

    files = _.difference files, excluded_files
    files = _.map files, (file) -> path.normalize "#{jscpd.options.path}/#{file}"
    return files

prepareOptions = (jscpd) ->
  if jscpd.options.files is null
    jscpd.options.patterns = ["**/*.+(#{jscpd.options.extensions.join '|'})"]
  else
    unless Array.isArray(jscpd.options.files)
      jscpd.options.patterns = [jscpd.options.files]
    else
      jscpd.options.patterns = jscpd.options.files
  if jscpd.options.exclude isnt null
    unless Array.isArray(jscpd.options.exclude)
      jscpd.options.exclude = [jscpd.options.exclude]


filesPreprocessor = (jscpd) ->
  prepareOptions jscpd
  jscpd.options.selectedFiles = findFiles jscpd

module.exports = filesPreprocessor

