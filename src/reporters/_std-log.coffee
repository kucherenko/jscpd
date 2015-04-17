module.exports = (numberOfClones, numberOfDuplication, numberOfFiles, numberOfLines, percentage, vlog) ->

    log = "Found #{numberOfClones} exact clones with
        #{numberOfDuplication} duplicated lines in
        #{numberOfFiles} files\n #{vlog}\n\n
        #{percentage}% (#{numberOfDuplication} lines)
        duplicated lines out of
        #{numberOfLines} total lines of code.\n"