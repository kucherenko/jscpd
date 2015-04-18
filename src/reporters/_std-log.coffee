module.exports = (map, vlog) ->

  log = "Found #{map.clones.length} exact clones with
    #{map.numberOfDuplication} duplicated lines in
    #{map.numberOfFiles} files\n #{vlog}\n\n
    #{map.getPercentage()}% (#{map.numberOfDuplication} lines)
    duplicated lines out of
    #{map.numberOfLines} total lines of code.\n"
