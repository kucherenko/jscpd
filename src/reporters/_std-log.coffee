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

compareDates = (firstDate, secondDate)->
	firstDate = new Date firstDate
	secondDate = new Date secondDate
	switch yes
		when firstDate < secondDate then "=>"
		when firstDate > secondDate then "<="
		else "=="

module.exports = ->

	clog = ''
	formatedClog = ''
	verbose = @options.verbose or @options.blame
	silent = @options.silent

	for clone in @map.clones
		do (clone) ->
			table = new Table TABLE_CONFIGURATION
			firstFile = clone.firstFile
			secondFile = clone.secondFile
			if verbose
				fragment = clone.getLines().split("\n").reduce (tbl, current, lineNumber) ->
					firstFileLine = clone.firstFileStart + lineNumber
					secondFileLine = clone.secondFileStart + lineNumber
					if Object.keys(clone.firstFileAnnotatedCode).length > 0 and
						clone.firstFileAnnotatedCode[firstFileLine] and
						clone.secondFileAnnotatedCode[secondFileLine]
							tbl.push [
								firstFileLine
								clone.firstFileAnnotatedCode[firstFileLine].author
								compareDates(
									clone.firstFileAnnotatedCode[firstFileLine].date,
									clone.secondFileAnnotatedCode[secondFileLine].date
								)
								secondFileLine
								clone.secondFileAnnotatedCode[secondFileLine].author
								current.dim
							]
					else
						tbl.push [
							firstFileLine
							secondFileLine
							current.dim
						]
					tbl
				, table

			if silent is false
				clog = "#{clog}\n\t-
              #{firstFile.green.bold}:
              #{clone.firstFileStart}-#{clone.firstFileStart + clone.linesCount}\n\t
              #{secondFile.green.bold}:
              #{clone.secondFileStart}-#{clone.secondFileStart + clone.linesCount}\n" 


		clog = "#{clog}\n#{fragment.toString()}\n" if verbose

	if silent is false
		formatedClog = "\n #{clog}\n\n";

	percent = @map.getPercentage()

	log = "Found #{@map.clones.length} exact clones with
    #{@map.numberOfDuplication} duplicated lines in
    #{@map.numberOfFiles} files#{formatedClog}
    #{percent}% (#{@map.numberOfDuplication} lines)
    duplicated lines out of
    #{@map.numberOfLines} total lines of code.\n"

	return log
