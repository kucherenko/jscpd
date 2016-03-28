logger = require "winston"


debugPreprocessor = (jscpd) ->
  if jscpd.options.debug
    logger.info '----------------------------------------'
    logger.info 'Options:'
    logger.info "#{name} = #{option}" for name, option of jscpd.options when name isnt 'selectedFiles'
    logger.info '----------------------------------------'
    logger.info 'Files:'
    logger.info file for file in jscpd.options.selectedFiles
    logger.info '----------------------------------------'
    logger.info 'Run without debug option for start detection process'


module.exports = debugPreprocessor

