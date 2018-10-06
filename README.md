![jscpd logo](assets/logo.svg)


## Copy/paste detector (ALPHA VERSION).

**(IMPORTANT) If you are looking for stable version, try to install jscpd v.0.6.x!!!!**

Copy/paste detector for programming source code supports 140+ formats.

## Getting started

### Installation
```bash
npm install jscpd@1.0.0-alpha.2 -g
```

### Usage
```bash
  jscpd /path/to/source
```

### Options
```bash
  jscpd --help

  Usage: jscpd [options] <path>

  Copy/paste detector for programming code, support JavaScript, CoffeeScript, PHP, Ruby, Python, Less, Go, Java, Yaml, C#, C++, C, Puppet, Twig languages

  Options:

    -V, --version             output the version number
    -l, --min-lines [number]  min size of duplication in code lines (Default is 5)
    -t, --threshold [number]  threshold for duplication, in case duplications >= threshold jscpd will exit with error
    -c, --config [string]     path to config file (Default is .cpd.json in <path>)
    -i, --ignore [string]     glob pattern for files what should be excluded from duplication detection
    -r, --reporters [string]  reporters or list of reporters separated with coma to use (Default is time,console)
    -o, --output [string]     reporters to use (Default is ./report/)
    -m, --mode [string]       mode of quality of search, can be "strict", "mild" and "weak" (Default is "mild")
    -f, --format [string]     format or formats separated by coma (Example php,javascript,python)
    -b, --blame               blame authors of duplications (get information about authors from git)
    -s, --silent              do not write detection progress and result to a console
    -a, --absolute            use absolute path in reports
    --formats-exts [string]   list of formats with file extensions (javascript:es,es6;dart:dt)
    -d, --debug               show debug information(options list and selected files)
    --list                    show list of all supported formats
    --xsl-href [string]       (Deprecated) Path to xsl file
    -p, --path                (Deprecated) Path to repo
    -h, --help                output usage information
```

## API

