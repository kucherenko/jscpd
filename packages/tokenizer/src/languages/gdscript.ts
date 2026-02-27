const grammar = {
  language: 'gdscript',
  init(Prism: any) {
    Prism.languages.gdscript = {
      'string': {
        pattern: /("""|''')[\s\S]*?\1|("|')(?:\\[\s\S]|(?!\2)[^\\])*\2/,
        greedy: true,
      },
      'comment': {
        pattern: /#.*/,
        greedy: true,
      },
      'annotation': {
        pattern: /@\w+/,
        alias: 'decorator',
      },
      'class-name': {
        pattern: /(\b(?:class|class_name|extends|is)\s+)\w+/,
        lookbehind: true,
      },
      'keyword': /\b(?:and|as|assert|await|break|breakpoint|class|class_name|const|continue|elif|else|enum|export|extends|for|func|get|if|in|is|match|not|onready|or|pass|preload|print|remote|return|self|set|setget|signal|static|super|tool|var|while|yield)\b/,
      'function': {
        pattern: /(\bfunc\s+)\w+/,
        lookbehind: true,
      },
      'builtin': /\b(?:bool|int|float|String|Vector2|Vector2i|Vector3|Vector3i|Vector4|Vector4i|Rect2|Rect2i|Transform2D|Transform3D|Plane|Quaternion|AABB|Basis|Projection|Color|NodePath|RID|Object|Callable|Signal|Dictionary|Array|PackedByteArray|PackedInt32Array|PackedInt64Array|PackedFloat32Array|PackedFloat64Array|PackedStringArray|PackedVector2Array|PackedVector3Array|PackedColorArray|null|true|false|PI|TAU|INF|NAN)\b/,
      'number': [
        // Hexadecimal
        /\b0x[\da-fA-F]+\b/,
        // Binary
        /\b0b[01]+\b/,
        // Float with exponent or decimal
        /\b\d+(?:\.\d+)?(?:e[+-]?\d+)?\b/i,
      ],
      'operator': /->|:=|&&|\|\||<<|>>|[-+*/%&|^~<>=!]=?|\.{2,3}/,
      'punctuation': /[{}[\]();,.:]/,
    };
  },
}
export default grammar;
