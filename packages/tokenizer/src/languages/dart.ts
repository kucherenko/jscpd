/**
 * Enhanced Dart language support for jscpd-ai
 * Includes modern Dart 3.0+ and Flutter-specific patterns
 */

import { IToken } from '@jscpd-ai/core';

/**
 * Dart-specific patterns for better duplicate detection
 */
export const dartPatterns = {
  // Null safety operators
  nullSafety: {
    pattern: /(\?\?|\?\.|!(?=\s|;|\)))/g,
    type: 'null-safety-operator',
  },

  // Flutter widget patterns
  widgetBuild: {
    pattern: /(Widget\s+build\s*\(BuildContext\s+context\))/g,
    type: 'widget-build-method',
  },

  // State management patterns
  statefulWidget: {
    pattern: /(State<\w+>|StatefulWidget|StatelessWidget)/g,
    type: 'widget-type',
  },

  // Async patterns
  asyncAwait: {
    pattern: /(async\s*\{|await\s+)/g,
    type: 'async-keyword',
  },

  // Extension methods
  extension: {
    pattern: /(extension\s+\w+\s+on\s+\w+)/g,
    type: 'extension-declaration',
  },

  // Mixin patterns
  mixin: {
    pattern: /(mixin\s+\w+|with\s+\w+)/g,
    type: 'mixin-keyword',
  },

  // Late initialization
  lateKeyword: {
    pattern: /\blate\s+/g,
    type: 'late-keyword',
  },

  // Record types (Dart 3.0+)
  recordType: {
    pattern: /\([\w\s,:<>]+\)\s*(?=\w+\s*[=;])/g,
    type: 'record-type',
  },

  // Sealed classes (Dart 3.0+)
  sealedClass: {
    pattern: /\bsealed\s+class\s+/g,
    type: 'sealed-keyword',
  },

  // Pattern matching (Dart 3.0+)
  switchExpression: {
    pattern: /\bswitch\s*\([^)]+\)\s*\{/g,
    type: 'switch-expression',
  },
};

/**
 * Common Flutter widget patterns that might be duplicated
 */
export const flutterWidgetPatterns = [
  'Container',
  'Column',
  'Row',
  'Padding',
  'Center',
  'Expanded',
  'SizedBox',
  'Card',
  'ListView',
  'GridView',
  'Stack',
  'Positioned',
  'TextField',
  'TextFormField',
  'ElevatedButton',
  'TextButton',
  'IconButton',
  'AppBar',
  'Scaffold',
  'SafeArea',
  'GestureDetector',
  'InkWell',
];

/**
 * Detect if a code block is a Flutter widget build method
 */
export function isFlutterWidgetBuild(code: string): boolean {
  return /Widget\s+build\s*\(BuildContext\s+context\)/.test(code);
}

/**
 * Detect if code uses null safety features
 */
export function usesNullSafety(code: string): boolean {
  return /(\?\?|\?\.|!(?=\s|;|\)))/.test(code);
}

/**
 * Extract widget tree structure for similarity comparison
 * This helps identify structurally similar widgets even with different content
 */
export function extractWidgetStructure(code: string): string[] {
  const structure: string[] = [];
  const widgetPattern = new RegExp(`\\b(${flutterWidgetPatterns.join('|')})\\s*\\(`, 'g');
  let match;

  while ((match = widgetPattern.exec(code)) !== null) {
    structure.push(match[1]);
  }

  return structure;
}

/**
 * Normalize Dart code for better duplicate detection
 * - Removes null safety operators for similarity comparison
 * - Normalizes async/await patterns
 * - Standardizes formatting
 */
export function normalizeDartCode(code: string): string {
  let normalized = code;

  // Normalize null safety operators (optional for similarity matching)
  // normalized = normalized.replace(/\?\./g, '.');
  // normalized = normalized.replace(/\?\?/g, '||');

  // Normalize async/await
  normalized = normalized.replace(/\basync\s*\{/g, 'async {');
  normalized = normalized.replace(/\bawait\s+/g, 'await ');

  // Normalize whitespace in generics
  normalized = normalized.replace(/(<\s+)/g, '<');
  normalized = normalized.replace(/(\s+>)/g, '>');

  return normalized;
}

/**
 * Calculate similarity score between two widget structures
 */
export function calculateWidgetSimilarity(structure1: string[], structure2: string[]): number {
  if (structure1.length === 0 || structure2.length === 0) {
    return 0;
  }

  const longer = structure1.length > structure2.length ? structure1 : structure2;
  const shorter = structure1.length > structure2.length ? structure2 : structure1;

  let matches = 0;
  shorter.forEach((widget, index) => {
    if (longer[index] === widget) {
      matches++;
    }
  });

  return matches / longer.length;
}

/**
 * Detect common Dart/Flutter code smells that might indicate duplication
 */
export interface DartCodeSmell {
  type: string;
  description: string;
  line?: number;
  suggestion?: string;
}

export function detectDartCodeSmells(code: string): DartCodeSmell[] {
  const smells: DartCodeSmell[] = [];

  // Detect very similar widget builds
  const widgetBuilds = code.match(/Widget\s+build\w*\s*\([^)]*\)\s*\{[^}]*\}/g);
  if (widgetBuilds && widgetBuilds.length > 1) {
    smells.push({
      type: 'repeated-widget-build',
      description: 'Multiple similar widget build methods found',
      suggestion: 'Consider extracting common widget into a reusable component',
    });
  }

  // Detect repeated setState patterns
  const setStateCalls = code.match(/setState\s*\(\s*\(\s*\)\s*\{[^}]*\}/g);
  if (setStateCalls && setStateCalls.length > 3) {
    smells.push({
      type: 'excessive-setstate',
      description: 'Many setState calls detected',
      suggestion: 'Consider using a state management solution (Provider, Riverpod, Bloc)',
    });
  }

  // Detect deeply nested widgets (common in Flutter)
  const nestedLevel = (code.match(/\w+\s*\(/g) || []).length;
  if (nestedLevel > 10) {
    smells.push({
      type: 'deep-widget-nesting',
      description: 'Deeply nested widget tree detected',
      suggestion: 'Extract nested widgets into separate methods or classes',
    });
  }

  return smells;
}

/**
 * Enhanced Dart token analyzer
 * Provides additional context for AI-powered analysis
 */
export function analyzeDartTokens(tokens: IToken[]): {
  hasNullSafety: boolean;
  hasAsyncCode: boolean;
  hasExtensions: boolean;
  widgetCount: number;
  complexity: number;
} {
  let hasNullSafety = false;
  let hasAsyncCode = false;
  let hasExtensions = false;
  let widgetCount = 0;
  let complexity = 0;

  tokens.forEach(token => {
    const value = token.value;

    if (value.match(/(\?\?|\?\.|!)/)) {
      hasNullSafety = true;
    }

    if (value.match(/(async|await)/)) {
      hasAsyncCode = true;
    }

    if (value.match(/extension\s+/)) {
      hasExtensions = true;
    }

    if (flutterWidgetPatterns.some(widget => value.includes(widget))) {
      widgetCount++;
    }

    // Simple complexity calculation
    if (value.match(/\bif\b|\bfor\b|\bwhile\b|\bswitch\b/)) {
      complexity++;
    }
  });

  return {
    hasNullSafety,
    hasAsyncCode,
    hasExtensions,
    widgetCount,
    complexity,
  };
}

/**
 * Configuration for Dart-specific duplicate detection
 */
export interface DartDetectionConfig {
  ignoreNullSafetyDifferences?: boolean;
  compareWidgetStructure?: boolean;
  minWidgetSimilarity?: number; // 0-1
  detectFlutterPatterns?: boolean;
  analyzeStateManagement?: boolean;
}

export const defaultDartConfig: DartDetectionConfig = {
  ignoreNullSafetyDifferences: false,
  compareWidgetStructure: true,
  minWidgetSimilarity: 0.7,
  detectFlutterPatterns: true,
  analyzeStateManagement: true,
};
