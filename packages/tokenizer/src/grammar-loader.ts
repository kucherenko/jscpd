import * as reprism from 'reprism';

import * as abap from 'reprism/languages/abap';
import * as actionscript from 'reprism/languages/actionscript';
import * as ada from 'reprism/languages/ada';
import * as apacheconf from 'reprism/languages/apacheconf';
import * as apl from 'reprism/languages/apl';
import * as applescript from 'reprism/languages/applescript';
import * as arff from 'reprism/languages/arff';
import * as asciidoc from 'reprism/languages/asciidoc';
import * as asm6502 from 'reprism/languages/asm6502';
import * as aspnet from 'reprism/languages/aspnet';
import * as autohotkey from 'reprism/languages/autohotkey';
import * as autoit from 'reprism/languages/autoit';
import * as bash from 'reprism/languages/bash';
import * as basic from 'reprism/languages/basic';
import * as batch from 'reprism/languages/batch';
import * as brainfuck from 'reprism/languages/brainfuck';
import * as bro from 'reprism/languages/bro';
import * as c from 'reprism/languages/c';
import * as clike from 'reprism/languages/clike';
import * as clojure from 'reprism/languages/clojure';
import * as coffeescript from 'reprism/languages/coffeescript';
import * as cpp from 'reprism/languages/cpp';
import * as csharp from 'reprism/languages/csharp';
import * as csp from 'reprism/languages/csp';
import * as cssExtras from 'reprism/languages/css-extras';
import * as css from 'reprism/languages/css';
import * as d from 'reprism/languages/d';
import * as dart from 'reprism/languages/dart';
import * as diff from 'reprism/languages/diff';
import * as django from 'reprism/languages/django';
import * as docker from 'reprism/languages/docker';
import * as eiffel from 'reprism/languages/eiffel';
import * as elixir from 'reprism/languages/elixir';
import * as erlang from 'reprism/languages/erlang';
import * as flow from 'reprism/languages/flow';
import * as fortran from 'reprism/languages/fortran';
import * as fsharp from 'reprism/languages/fsharp';
import * as gedcom from 'reprism/languages/gedcom';
import * as gherkin from 'reprism/languages/gherkin';
import * as git from 'reprism/languages/git';
import * as glsl from 'reprism/languages/glsl';
import * as go from 'reprism/languages/go';
import * as graphql from 'reprism/languages/graphql';
import * as groovy from 'reprism/languages/groovy';
import * as haml from 'reprism/languages/haml';
import * as handlebars from 'reprism/languages/handlebars';
import * as haskell from 'reprism/languages/haskell';
import * as haxe from 'reprism/languages/haxe';
import * as hpkp from 'reprism/languages/hpkp';
import * as hsts from 'reprism/languages/hsts';
import * as http from 'reprism/languages/http';
import * as ichigojam from 'reprism/languages/ichigojam';
import * as icon from 'reprism/languages/icon';
import * as inform7 from 'reprism/languages/inform7';
import * as ini from 'reprism/languages/ini';
import * as io from 'reprism/languages/io';
import * as j from 'reprism/languages/j';
import * as java from 'reprism/languages/java';
import * as javascript from 'reprism/languages/javascript';
import * as jolie from 'reprism/languages/jolie';
import * as json from 'reprism/languages/json';
import * as jsx from 'reprism/languages/jsx';
import * as julia from 'reprism/languages/julia';
import * as keyman from 'reprism/languages/keyman';
import * as kotlin from 'reprism/languages/kotlin';
import * as latex from 'reprism/languages/latex';
import * as less from 'reprism/languages/less';
import * as liquid from 'reprism/languages/liquid';
import * as lisp from 'reprism/languages/lisp';
import * as livescript from 'reprism/languages/livescript';
import * as lolcode from 'reprism/languages/lolcode';
import * as lua from 'reprism/languages/lua';
import * as makefile from 'reprism/languages/makefile';
import * as markdown from 'reprism/languages/markdown';
import * as markupTemplating from 'reprism/languages/markup-templating';
import * as markup from 'reprism/languages/markup';
import * as matlab from 'reprism/languages/matlab';
import * as mel from 'reprism/languages/mel';
import * as mizar from 'reprism/languages/mizar';
import * as monkey from 'reprism/languages/monkey';
import * as n4js from 'reprism/languages/n4js';
import * as nasm from 'reprism/languages/nasm';
import * as nginx from 'reprism/languages/nginx';
import * as nim from 'reprism/languages/nim';
import * as nix from 'reprism/languages/nix';
import * as nsis from 'reprism/languages/nsis';
import * as objectivec from 'reprism/languages/objectivec';
import * as ocaml from 'reprism/languages/ocaml';
import * as opencl from 'reprism/languages/opencl';
import * as oz from 'reprism/languages/oz';
import * as parigp from 'reprism/languages/parigp';
import * as parser from 'reprism/languages/parser';
import * as pascal from 'reprism/languages/pascal';
import * as perl from 'reprism/languages/perl';
import * as phpExtras from 'reprism/languages/php-extras';
import * as php from 'reprism/languages/php';
import * as powershell from 'reprism/languages/powershell';
import * as processing from 'reprism/languages/processing';
import * as prolog from 'reprism/languages/prolog';
import * as properties from 'reprism/languages/properties';
import * as protobuf from 'reprism/languages/protobuf';
import * as pug from 'reprism/languages/pug';
import * as puppet from 'reprism/languages/puppet';
import * as pure from 'reprism/languages/pure';
import * as python from 'reprism/languages/python';
import * as q from 'reprism/languages/q';
import * as qore from 'reprism/languages/qore';
import * as r from 'reprism/languages/r';
import * as reason from 'reprism/languages/reason';
import * as renpy from 'reprism/languages/renpy';
import * as rest from 'reprism/languages/rest';
import * as rip from 'reprism/languages/rip';
import * as roboconf from 'reprism/languages/roboconf';
import * as ruby from 'reprism/languages/ruby';
import * as rust from 'reprism/languages/rust';
import * as sas from 'reprism/languages/sas';
import * as sass from 'reprism/languages/sass';
import * as scala from 'reprism/languages/scala';
import * as scheme from 'reprism/languages/scheme';
import * as scss from 'reprism/languages/scss';
import * as smalltalk from 'reprism/languages/smalltalk';
import * as smarty from 'reprism/languages/smarty';
import * as soy from 'reprism/languages/soy';
import * as stylus from 'reprism/languages/stylus';
import * as swift from 'reprism/languages/swift';
import * as tcl from 'reprism/languages/tcl';
import * as textile from 'reprism/languages/textile';
import * as tsx from 'reprism/languages/tsx';
import * as twig from 'reprism/languages/twig';
import * as typescript from 'reprism/languages/typescript';
import * as vbnet from 'reprism/languages/vbnet';
import * as velocity from 'reprism/languages/velocity';
import * as verilog from 'reprism/languages/verilog';
import * as vhdl from 'reprism/languages/vhdl';
import * as vim from 'reprism/languages/vim';
import * as visualBasic from 'reprism/languages/visual-basic';
import * as wasm from 'reprism/languages/wasm';
import * as wiki from 'reprism/languages/wiki';
import * as xeora from 'reprism/languages/xeora';
import * as xojo from 'reprism/languages/xojo';
import * as yaml from 'reprism/languages/yaml';
import * as sql from './languages/sql';
import * as plsql from './languages/plsql';

export const languages = {
  abap, actionscript, ada, apacheconf, apl, applescript, arff,
  asciidoc, asm6502, aspnet, autohotkey, autoit, bash, basic, batch,
  brainfuck, bro, c, clike, clojure, coffeescript, cpp, csharp, csp, cssExtras,
  css, d, dart, diff, django, docker, eiffel, elixir, erlang, flow, fortran, fsharp,
  gedcom, gherkin, git, glsl, go, graphql, groovy, haml, handlebars, haskell, haxe,
  hpkp, hsts, http, ichigojam, icon, inform7, ini, io, j, java, javascript, jolie,
  json, jsx, julia, keyman, kotlin, latex, less, liquid, lisp, livescript,
  lolcode, lua, makefile, markdown, markupTemplating, markup, matlab, mel, mizar,
  monkey, n4js, nasm, nginx, nim, nix, nsis, objectivec, ocaml, opencl, oz, parigp,
  parser, pascal, perl, php, phpExtras, powershell, processing, prolog,
  properties, protobuf, pug, puppet, pure, python, q, qore, r, reason, renpy, rest,
  rip, roboconf, ruby, rust, sas, sass, scala, scheme, scss, smalltalk, smarty, soy,
  stylus, swift, tcl, textile, twig, typescript, vbnet, velocity, verilog, vhdl,
  vim, visualBasic, wasm, wiki, xeora, xojo, yaml, tsx, sql, plsql
};

export const loadLanguages = (): void => {
  reprism.loadLanguages(Object.values(languages).map(v => v.default));
}

