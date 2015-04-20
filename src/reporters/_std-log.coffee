module.exports = ->

  clog = ''
  verbose = @options.verbose

  for clone in @map.clones
    do (clone) ->

      firstFile = clone.firstFile
      secondFile = clone.secondFile
      fragment = clone.getLines()

      clog = "#{clog}\n\t-
        #{firstFile}:
        #{clone.firstFileStart}-#{clone.firstFileStart + clone.linesCount}\n\t
        #{secondFile}:
        #{clone.secondFileStart}-
        #{clone.secondFileStart + clone.linesCount}\n\t"

      clog = "#{clog}\n#{fragment}" if verbose

  log = "Found #{@map.clones.length} exact clones with
    #{@map.numberOfDuplication} duplicated lines in
    #{@map.numberOfFiles} files\n #{clog}\n\n
    #{@map.getPercentage()}% (#{@map.numberOfDuplication} lines)
    duplicated lines out of
    #{@map.numberOfLines} total lines of code.\n"
