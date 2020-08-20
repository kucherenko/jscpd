import {extname} from "path";
import {IFormatMeta} from './interfaces';

export const FORMATS: {
	[key: string]: IFormatMeta;
} = {
	abap: {
		exts: [],
	},
	actionscript: {
		exts: ['as'],
	},
	ada: {
		exts: ['ada'],
	},
	apacheconf: {
		exts: [],
	},
	apl: {
		exts: ['apl'],
	},
	applescript: {
		exts: [],
	},
	arduino: {
		exts: [],
	},
	arff: {
		exts: [],
	},
	asciidoc: {
		exts: [],
	},
	asm6502: {
		exts: [],
	},
	aspnet: {
		exts: ['asp', 'aspx'],
	},
	autohotkey: {
		exts: [],
	},
	autoit: {
		exts: [],
	},
	bash: {
		exts: ['sh', 'ksh', 'bash'],
	},
	basic: {
		exts: ['bas'],
	},
	batch: {
		exts: [],
	},
	bison: {
		exts: [],
	},
	brainfuck: {
		exts: ['b', 'bf'],
	},
	bro: {
		exts: [],
	},
	c: {
		exts: ['c', 'z80'],
	},
	'c-header': {
		exts: ['h'],
		parent: 'c',
	},
	clike: {
		exts: [],
	},
	clojure: {
		exts: ['cljs', 'clj', 'cljc', 'cljx', 'edn'],
	},
	coffeescript: {
		exts: ['coffee'],
	},
	comments: {
		exts: []
	},
	cpp: {
		exts: ['cpp', 'c++', 'cc', 'cxx'],
	},
	'cpp-header': {
		exts: ['hpp', 'h++', 'hh', 'hxx'],
		parent: 'cpp',
	},
	crystal: {
		exts: ['cr'],
	},
	csharp: {
		exts: ['cs'],
	},
	csp: {
		exts: [],
	},
	'css-extras': {
		exts: [],
	},
	css: {
		exts: ['css', 'gss'],
	},
	d: {
		exts: ['d'],
	},
	dart: {
		exts: ['dart'],
	},
	diff: {
		exts: ['diff', 'patch'],
	},
	django: {
		exts: [],
	},
	docker: {
		exts: [],
	},
	eiffel: {
		exts: ['e'],
	},
	elixir: {
		exts: [],
	},
	elm: {
		exts: ['elm'],
	},
	erb: {
		exts: [],
	},
	erlang: {
		exts: ['erl', 'erlang'],
	},
	flow: {
		exts: [],
	},
	fortran: {
		exts: ['f', 'for', 'f77', 'f90'],
	},
	fsharp: {
		exts: ['fs'],
	},
	gedcom: {
		exts: [],
	},
	gherkin: {
		exts: ['feature'],
	},
	git: {
		exts: [],
	},
	glsl: {
		exts: [],
	},
	go: {
		exts: ['go'],
	},
	graphql: {
		exts: ['graphql'],
	},
	groovy: {
		exts: ['groovy', 'gradle'],
	},
	haml: {
		exts: ['haml'],
	},
	handlebars: {
		exts: ['hb', 'hbs', 'handlebars'],
	},
	haskell: {
		exts: ['hs', 'lhs '],
	},
	haxe: {
		exts: ['hx', 'hxml'],
	},
	hpkp: {
		exts: [],
	},
	hsts: {
		exts: [],
	},
	http: {
		exts: [],
	},
	ichigojam: {
		exts: [],
	},
	icon: {
		exts: [],
	},
	inform7: {
		exts: [],
	},
	ini: {
		exts: ['ini'],
	},
	io: {
		exts: [],
	},
	j: {
		exts: [],
	},
	java: {
		exts: ['java'],
	},
	javascript: {
		exts: ['js', 'es', 'es6'],
	},
	jolie: {
		exts: [],
	},
	json: {
		exts: ['json', 'map', 'jsonld'],
	},
	jsx: {
		exts: ['jsx'],
	},
	julia: {
		exts: ['jl'],
	},
	keymap: {
		exts: [],
	},
	kotlin: {
		exts: ['kt', 'kts'],
	},
	latex: {
		exts: ['tex'],
	},
	less: {
		exts: ['less'],
	},
	liquid: {
		exts: [],
	},
	lisp: {
		exts: ['cl', 'lisp', 'el'],
	},
	livescript: {
		exts: ['ls'],
	},
	lolcode: {
		exts: [],
	},
	lua: {
		exts: ['lua'],
	},
	makefile: {
		exts: [],
	},
	markdown: {
		exts: ['md', 'markdown', 'mkd', 'txt'],
	},
	markup: {
		exts: ['html', 'htm', 'xml', 'xsl', 'xslt', 'svg', 'vue', 'ejs', 'jsp'],
	},
	matlab: {
		exts: [],
	},
	mel: {
		exts: [],
	},
	mizar: {
		exts: [],
	},
	monkey: {
		exts: [],
	},
	n4js: {
		exts: [],
	},
	nasm: {
		exts: [],
	},
	nginx: {
		exts: [],
	},
	nim: {
		exts: [],
	},
	nix: {
		exts: [],
	},
	nsis: {
		exts: ['nsh', 'nsi'],
	},
	objectivec: {
		exts: ['m', 'mm'],
	},
	ocaml: {
		exts: ['ocaml', 'ml', 'mli', 'mll', 'mly'],
	},
	opencl: {
		exts: [],
	},
	oz: {
		exts: ['oz'],
	},
	parigp: {
		exts: [],
	},
	pascal: {
		exts: ['pas', 'p'],
	},
	perl: {
		exts: ['pl', 'pm'],
	},
	php: {
		exts: ['php', 'phtml'],
	},
	plsql: {
		exts: ['plsql'],
	},
	powershell: {
		exts: ['ps1', 'psd1', 'psm1'],
	},
	processing: {
		exts: [],
	},
	prolog: {
		exts: ['pro'],
	},
	properties: {
		exts: ['properties'],
	},
	protobuf: {
		exts: ['proto'],
	},
	pug: {
		exts: ['pug', 'jade'],
	},
	puppet: {
		exts: ['pp', 'puppet'],
	},
	pure: {
		exts: [],
	},
	python: {
		exts: ['py', 'pyx', 'pxd', 'pxi'],
	},
	q: {
		exts: ['q'],
	},
	qore: {
		exts: [],
	},
	r: {
		exts: ['r', 'R'],
	},
	reason: {
		exts: [],
	},
	renpy: {
		exts: [],
	},
	rest: {
		exts: [],
	},
	rip: {
		exts: [],
	},
	roboconf: {
		exts: [],
	},
	ruby: {
		exts: ['rb'],
	},
	rust: {
		exts: ['rs'],
	},
	sas: {
		exts: ['sas'],
	},
	sass: {
		exts: ['sass'],
	},
	scala: {
		exts: ['scala'],
	},
	scheme: {
		exts: ['scm', 'ss'],
	},
	scss: {
		exts: ['scss'],
	},
	smalltalk: {
		exts: ['st'],
	},
	smarty: {
		exts: ['smarty', 'tpl'],
	},
	soy: {
		exts: ['soy'],
	},
	sql: {
		exts: ['sql', 'cql'],
	},
	stylus: {
		exts: ['styl', 'stylus'],
	},
	swift: {
		exts: ['swift'],
	},
	tap: {
		exts: ['tap'],
	},
	tcl: {
		exts: ['tcl'],
	},
	textile: {
		exts: ['textile'],
	},
	tsx: {
		exts: ['tsx'],
	},
	tt2: {
		exts: ['tt2'],
	},
	twig: {
		exts: ['twig'],
	},
	typescript: {
		exts: ['ts'],
	},
	vbnet: {
		exts: ['vb'],
	},
	velocity: {
		exts: ['vtl'],
	},
	verilog: {
		exts: ['v'],
	},
	vhdl: {
		exts: ['vhd', 'vhdl'],
	},
	vim: {
		exts: [],
	},
	'visual-basic': {
		exts: ['vb'],
	},
	wasm: {
		exts: [],
	},
	url: {
		exts: [],
	},
	wiki: {
		exts: [],
	},
	xeora: {
		exts: [],
	},
	xojo: {
		exts: [],
	},
	xquery: {
		exts: ['xy', 'xquery'],
	},
	yaml: {
		exts: ['yaml', 'yml'],
	},
};

export function getSupportedFormats(): string[] {
	return Object.keys(FORMATS).filter((name) => name !== 'important' && name !== 'url');
}

export function getFormatByFile(path: string, formatsExts?: { [key: string]: string[] }): string | undefined {
	const ext: string = extname(path).slice(1);
	if (formatsExts && Object.keys(formatsExts).length) {
		return Object.keys(formatsExts).find((format) => formatsExts[format].includes(ext));
	}
	return Object.keys(FORMATS).find((language) => FORMATS[language].exts.includes(ext));
}
