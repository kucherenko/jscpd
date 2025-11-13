<p align="center">
  <img src="https://raw.githubusercontent.com/kucherenko/jscpd/master/assets/logo.svg?sanitize=true">
</p>

## jscpd-ai

[![npm](https://img.shields.io/npm/v/jscpd-ai.svg?style=flat-square)](https://www.npmjs.com/package/jscpd-ai)
[![license](https://img.shields.io/github/license/moinsen-dev/jscpd2025.svg?style=flat-square)](https://github.com/moinsen-dev/jscpd2025/blob/master/LICENSE)

> AI-enhanced copy/paste detector for programming source code, supports 150+ formats with local AI-powered analysis.

**JSCPD-AI** is an enhanced fork of the popular jscpd tool, adding AI-powered semantic analysis and intelligent refactoring suggestions using local Ollama models. All AI processing happens on your machine - your code never leaves your system.

## ✨ New AI Features

- 🤖 **Semantic Similarity Analysis** - Detect functionally similar code even with different syntax
- 🔧 **Smart Refactoring Suggestions** - Get AI-powered recommendations for eliminating duplicates
- 📊 **Enhanced Reports** - Detailed analysis with confidence scores and reasoning
- 💬 **Explanations** - Understand why duplications exist and how to fix them
- 🎯 **Dart/Flutter Optimized** - Enhanced support for modern Dart 3.0 and Flutter patterns
- 🔒 **100% Local** - All AI processing via Ollama (no external API calls)
- 🚀 **Backwards Compatible** - Works exactly like jscpd when AI features are disabled

## Quick Start

### Installation

```bash
npm install -g jscpd-ai
```

### Basic Usage (Traditional Mode)

```bash
jscpd-ai /path/to/source
```

### AI-Enhanced Mode

```bash
# Install Ollama first (see setup guide)
curl -fsSL https://ollama.com/install.sh | sh
ollama pull codellama:7b

# Run with AI features
jscpd-ai /path/to/source --ai --ai-refactor --ai-semantic
```

## What's Different from jscpd?

| Feature | jscpd | jscpd-ai |
|---------|-------|----------|
| Duplicate Detection | ✅ | ✅ |
| 150+ Language Support | ✅ | ✅ |
| Multiple Reporters | ✅ | ✅ |
| **AI Semantic Analysis** | ❌ | ✅ |
| **Refactoring Suggestions** | ❌ | ✅ |
| **Enhanced Dart Support** | Basic | ✅ Advanced |
| **Local AI Processing** | ❌ | ✅ |
| **Natural Language Explanations** | ❌ | ✅ |

## AI Features in Detail

### Semantic Similarity Analysis

Traditional duplicate detection misses functionally identical code with different syntax. AI analysis catches these:

```typescript
// These are functionally identical but textually different
// Traditional tools might miss this, jscpd-ai catches it

// Version 1
function getUserName(user: User): string {
  return user?.name ?? 'Anonymous';
}

// Version 2
function extractUserName(u: User): string {
  if (u && u.name) {
    return u.name;
  }
  return 'Anonymous';
}
```

**AI Output**:
```json
{
  "similarityScore": 95,
  "confidence": 0.89,
  "functionallyEquivalent": true,
  "reasoning": "Both functions perform null-safe user name extraction with fallback"
}
```

### Smart Refactoring Suggestions

Get AI-powered recommendations:

```json
{
  "type": "extract-function",
  "confidence": 0.87,
  "description": "Extract validation logic into shared utility",
  "suggestedCode": "export function validateUser(user: User) { ... }",
  "reasoning": "This validation appears 5 times across auth and profile modules",
  "risks": ["May affect error handling in ProfileComponent"]
}
```

### Enhanced Dart/Flutter Support

Special detection for Flutter patterns:

```dart
// Detects similar widget structures
Widget buildUserCard() {
  return Card(
    child: Column(
      children: [Text(user.name), Text(user.email)]
    )
  );
}

Widget buildProfileCard() {  // AI recognizes structural similarity
  return Card(
    child: Column(
      children: [Text(profile.name), Text(profile.email)]
    )
  );
}
```

## Installation

### Global Installation

```bash
npm install -g jscpd-ai
```

### Local Project Installation

```bash
npm install --save-dev jscpd-ai
```

### Setup Ollama (for AI Features)

See [OLLAMA_SETUP.md](./OLLAMA_SETUP.md) for detailed installation guide.

**Quick setup**:

```bash
# Install Ollama
curl -fsSL https://ollama.com/install.sh | sh

# Install recommended model (3.8GB)
ollama pull codellama:7b

# Verify
ollama run codellama:7b "test"
```

## Usage

### Traditional Mode (No AI)

Works exactly like jscpd:

```bash
jscpd-ai /path/to/code
```

### AI-Enhanced Mode

```bash
# Basic AI scan
jscpd-ai /path/to/code --ai

# With refactoring suggestions
jscpd-ai /path/to/code --ai --ai-refactor

# With semantic analysis
jscpd-ai /path/to/code --ai --ai-semantic

# Full AI features
jscpd-ai /path/to/code --ai --ai-semantic --ai-refactor --ai-explain

# Generate AI report
jscpd-ai /path/to/code --ai --reporters ai -o ./reports/

# Custom model
jscpd-ai /path/to/code --ai --ai-model "deepseek-coder:6.7b"
```

### Configuration File

Create `.jscpd-ai.json` in your project root:

```json
{
  "threshold": 5,
  "minLines": 5,
  "minTokens": 50,
  "ignore": ["**/*.test.ts", "**/node_modules/**"],
  "reporters": ["console", "html", "ai"],
  "output": "./reports",
  "ollama": {
    "enabled": true,
    "host": "http://localhost:11434",
    "model": "codellama:7b",
    "timeout": 30000,
    "features": {
      "semanticSimilarity": true,
      "refactoringSuggestions": true,
      "explanations": true
    }
  },
  "dart": {
    "detectFlutterPatterns": true,
    "analyzeNullSafety": true
  }
}
```

## CLI Options

### Traditional Options

All original jscpd options are supported:

```bash
-l, --min-lines [number]       Minimum duplicate line count
-k, --min-tokens [number]      Minimum duplicate token count
-t, --threshold [number]       Error threshold
-r, --reporters [string]       Reporters to use
-o, --output [string]          Output directory
-m, --mode [string]            Detection mode (strict|mild|weak)
-f, --format [string]          Format filter
-i, --ignore [string]          Ignore pattern
-s, --silent                   Silent mode
```

### New AI Options

```bash
--ai                           Enable AI features
--ai-model [string]            Ollama model (default: codellama:7b)
--ai-host [string]             Ollama host (default: http://localhost:11434)
--ai-semantic                  Enable semantic similarity analysis
--ai-refactor                  Generate refactoring suggestions
--ai-explain                   Generate explanations
--ai-report [string]           Path for AI report
```

## Programming API

### Traditional API

```typescript
import { jscpd } from 'jscpd-ai';

const clones = await jscpd([
  '',
  '',
  __dirname + '/fixtures',
  '-m',
  'weak',
  '--silent'
]);

console.log(clones);
```

### AI-Enhanced API

```typescript
import { detectClones } from 'jscpd-ai';
import { OllamaService } from '@jscpd-ai/ollama-service';

const ollama = new OllamaService();

const clones = await detectClones({
  path: [__dirname + '/src'],
  silent: true,
  reporters: ['ai'],
  ollama: {
    enabled: true,
    model: 'codellama:7b'
  }
});

// Access AI analysis
clones.forEach(clone => {
  if (clone.semanticAnalysis) {
    console.log('Similarity:', clone.semanticAnalysis.similarityScore);
    console.log('Suggestion:', clone.refactoringSuggestion);
  }
});
```

### Using Ollama Service Directly

```typescript
import { OllamaService } from '@jscpd-ai/ollama-service';

const ollama = new OllamaService({
  model: 'deepseek-coder:6.7b',
  host: 'http://localhost:11434'
});

// Check availability
const isAvailable = await ollama.checkAvailability();

// Analyze similarity
const analysis = await ollama.analyzeSementicSimilarity(
  code1,
  code2,
  'typescript'
);

// Generate refactoring
const suggestion = await ollama.generateRefactoringSuggestion(
  duplicates,
  'typescript'
);
```

## Packages

| Package | Version | Description |
|---------|---------|-------------|
| [jscpd-ai](apps/jscpd) | [![npm](https://img.shields.io/npm/v/jscpd-ai.svg?style=flat-square)](https://www.npmjs.com/package/jscpd-ai) | Main CLI and API |
| [@jscpd-ai/core](packages/core) | [![npm](https://img.shields.io/npm/v/@jscpd-ai/core.svg?style=flat-square)](https://www.npmjs.com/package/@jscpd-ai/core) | Core detection algorithm |
| [@jscpd-ai/finder](packages/finder) | [![npm](https://img.shields.io/npm/v/@jscpd-ai/finder.svg?style=flat-square)](https://www.npmjs.com/package/@jscpd-ai/finder) | File detection |
| [@jscpd-ai/tokenizer](packages/tokenizer) | [![npm](https://img.shields.io/npm/v/@jscpd-ai/tokenizer.svg?style=flat-square)](https://www.npmjs.com/package/@jscpd-ai/tokenizer) | Code tokenization with Dart enhancements |
| [@jscpd-ai/ollama-service](packages/ollama-service) | [![npm](https://img.shields.io/npm/v/@jscpd-ai/ollama-service.svg?style=flat-square)](https://www.npmjs.com/package/@jscpd-ai/ollama-service) | **NEW** Ollama integration |
| [@jscpd-ai/ai-reporter](packages/ai-reporter) | [![npm](https://img.shields.io/npm/v/@jscpd-ai/ai-reporter.svg?style=flat-square)](https://www.npmjs.com/package/@jscpd-ai/ai-reporter) | **NEW** AI-enhanced reports |
| [@jscpd-ai/html-reporter](packages/html-reporter) | [![npm](https://img.shields.io/npm/v/@jscpd-ai/html-reporter.svg?style=flat-square)](https://www.npmjs.com/package/@jscpd-ai/html-reporter) | HTML reports |

## Supported Languages

150+ languages including:

**Enhanced**: Dart (with Flutter patterns), TypeScript, JavaScript, Python, Java, C#, Go, Rust, Kotlin, Swift

**Full List**: See [supported_formats.md](supported_formats.md)

## Examples

### CI/CD Integration

```yaml
# GitHub Actions
name: Code Quality
on: [push, pull_request]
jobs:
  duplicates:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-node@v3
      - run: npm install -g jscpd-ai
      - run: jscpd-ai ./src --threshold 10 --reporters console,json
```

### Pre-commit Hook

```json
{
  "husky": {
    "hooks": {
      "pre-commit": "jscpd-ai ./src --threshold 5 --silent"
    }
  }
}
```

### With AI in CI/CD

```yaml
# Self-hosted runner with Ollama
- name: Setup Ollama
  run: |
    curl -fsSL https://ollama.com/install.sh | sh
    ollama pull codellama:7b

- name: Analyze with AI
  run: jscpd-ai ./src --ai --ai-refactor --threshold 5
```

## Performance

### Without AI
- **Small** (< 100 files): < 1 second
- **Medium** (1000 files): 2-5 seconds
- **Large** (10,000 files): 10-30 seconds

### With AI
- Depends on model and hardware
- **7B models**: +2-5 seconds per clone analyzed
- **13B models**: +5-10 seconds per clone analyzed
- GPU acceleration: 2-3x faster

**Tip**: Use AI only for important scans, traditional mode for rapid feedback.

## Who Uses jscpd-ai?

- All original jscpd users can use jscpd-ai as drop-in replacement
- Teams wanting local AI code analysis
- Flutter/Dart projects needing better pattern detection
- Privacy-focused organizations

## Original jscpd Users

- [GitHub Super Linter](https://github.com/github/super-linter)
- [Code-Inspector](https://www.code-inspector.com/)
- [Mega-Linter](https://nvuillam.github.io/mega-linter/)
- [Codacy](http://docs.codacy.com/getting-started/supported-languages-and-tools/)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md)

Start development:

```bash
git clone https://github.com/moinsen-dev/jscpd2025
cd jscpd2025
pnpm install
pnpm dev  # Watch mode
pnpm build
pnpm test
```

## Documentation

- [Implementation Plan](IMPLEMENTATION_PLAN.md)
- [Ollama Setup Guide](OLLAMA_SETUP.md)
- [Supported Formats](supported_formats.md)
- [API Documentation](apps/jscpd/README.md)

## Credits

**Original jscpd**: [Andrey Kucherenko](https://github.com/kucherenko/jscpd)

**AI Enhancements**: This fork adds AI-powered analysis while maintaining full compatibility with the original jscpd.

## License

[MIT](LICENSE) © Andrey Kucherenko (original jscpd), AI enhancements by fork contributors

---

## Quick Links

- 📖 [Full Documentation](https://github.com/moinsen-dev/jscpd2025/wiki)
- 🤖 [Ollama Setup](OLLAMA_SETUP.md)
- 🐛 [Report Issues](https://github.com/moinsen-dev/jscpd2025/issues)
- 💬 [Discussions](https://github.com/moinsen-dev/jscpd2025/discussions)
- ⭐ [Star on GitHub](https://github.com/moinsen-dev/jscpd2025)

**Privacy First**: Your code never leaves your machine. All AI processing is local via Ollama.
