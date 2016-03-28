require 'colors'
Table = require 'cli-table'

TABLE_CONFIGURATION = chars:
          'mid': ''
          'left-mid': ''
          'mid-mid': ''
          'right-mid': ''
          'top': ''
          'top-mid': ''
          'top-left': ''
          'top-right': ''
          'bottom': ''
          'bottom-mid': ''
          'bottom-left': ''
          'bottom-right': ''
          'left': ''
          'right': ''

module.exports = ->

  clog = ''
  verbose = @options.verbose

  for clone in @map.clones
    do (clone) ->
      table = ''
      firstFile = clone.firstFile
      secondFile = clone.secondFile

      if verbose
        table = new Table TABLE_CONFIGURATION

        fragment = clone.getLines().split("\n").reduce (tbl, current, lineNumber) ->
          tbl.push [
            clone.firstFileStart + lineNumber
            clone.secondFileStart + lineNumber
            current.dim
          ]
          tbl
        , table

      clog = "#{clog}\n\t-
        #{firstFile.green.bold}:
        #{clone.firstFileStart}-#{clone.firstFileStart + clone.linesCount}\n\t
        #{secondFile.green.bold}:
        #{clone.secondFileStart}-#{clone.secondFileStart + clone.linesCount}\n"

      clog = "#{clog}\n#{fragment.toString()}\n" if verbose

  percent = @map.getPercentage()
  
  log = "Found #{@map.clones.length} exact clones with
    #{@map.numberOfDuplication} duplicated lines in
    #{@map.numberOfFiles} files\n #{clog}\n\n
    #{percent}% (#{@map.numberOfDuplication} lines)
    duplicated lines out of
    #{@map.numberOfLines} total lines of code.\n"

  if @options.limit <= percent
    console.error log
    console.error "ERROR: jscpd found too many duplicates over threshold"
    process.exit(1)

  return log
