# JSCPD 2025 - AI-Enhanced Implementation Plan

## Project Overview

**Name**: `jscpd-ai` (or alternative: `smartdupe`, `ai-cpd`)
**Purpose**: Fork of jscpd with AI-enhanced duplicate detection and refactoring suggestions
**AI Backend**: Ollama (local, open-source)
**License**: MIT (same as original)
**Target**: NPM publication as standalone package

---

## Core Enhancements

### Phase 1: Foundation (Week 1)
**Goal**: Set up the fork with new branding and enhanced Dart support

#### 1.1 Rebranding
- [ ] Choose final package name: `jscpd-ai`
- [ ] Update all package.json files in monorepo
- [ ] Update README and documentation
- [ ] Update scope: `@jscpd-ai/core`, `@jscpd-ai/finder`, etc.
- [ ] Update imports and references

#### 1.2 Enhanced Dart Support
- [ ] Review current Dart tokenization (uses Prism.js)
- [ ] Add Dart-specific patterns:
  - Null safety operators (`?.`, `??`, `!`)
  - Async/await patterns
  - Extension methods
  - Mixins and abstract classes
- [ ] Test with real Flutter/Dart projects
- [ ] Add Dart examples to fixtures

---

### Phase 2: Ollama Integration (Week 2)
**Goal**: Create AI infrastructure using local Ollama models

#### 2.1 Ollama Service Module
Create new package: `@jscpd-ai/ollama-service`

```typescript
// Core functionality:
- Connection management
- Model selection (codellama, deepseek-coder, etc.)
- Request batching
- Error handling & fallbacks
- Configuration management
```

#### 2.2 Features
- [ ] Ollama client wrapper
- [ ] Health check and model verification
- [ ] Streaming support for large code blocks
- [ ] Configurable timeout and retry logic
- [ ] Support for multiple Ollama instances (load balancing)

#### 2.3 Configuration
```json
{
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
  }
}
```

---

### Phase 3: AI-Enhanced Detection (Week 3)
**Goal**: Semantic similarity beyond text matching

#### 3.1 Semantic Similarity Engine
Create new package: `@jscpd-ai/semantic`

**Algorithm**:
1. Extract duplicates using traditional Rabin-Karp (fast)
2. For borderline cases (40-70% match), use AI analysis
3. Compare AST structures when available
4. Generate similarity scores with explanations

**Implementation**:
```typescript
interface SemanticAnalysis {
  similarityScore: number; // 0-100
  confidence: number; // 0-1
  reasoning: string;
  functionallyEquivalent: boolean;
  refactoringPotential: 'high' | 'medium' | 'low';
}
```

#### 3.2 Cross-Language Detection
- [ ] Detect same logic in different languages
- [ ] Example: Python function ↔ TypeScript function
- [ ] Use AI to understand algorithmic similarity
- [ ] Suggest creating shared library

---

### Phase 4: Smart Refactoring (Week 4)
**Goal**: Don't just detect - suggest fixes

#### 4.1 Refactoring Suggestion Engine
Create new package: `@jscpd-ai/refactor`

**Features**:
- [ ] Extract function suggestions
- [ ] Extract class suggestions
- [ ] Create shared module recommendations
- [ ] Generate actual refactored code
- [ ] Impact analysis (what will break)

**Output Format**:
```json
{
  "duplicates": [
    {
      "id": "dup-001",
      "locations": [...],
      "suggestion": {
        "type": "extract-function",
        "confidence": 0.85,
        "extractionPoint": "src/utils/common.ts",
        "generatedCode": "export function validateUser(user) { ... }",
        "replacements": [
          {
            "file": "src/auth.ts",
            "line": 45,
            "replacement": "validateUser(user)"
          }
        ],
        "reasoning": "This validation logic is repeated 5 times...",
        "risks": ["May affect error handling in module X"]
      }
    }
  ]
}
```

---

### Phase 5: AI-Friendly Features (Week 5)
**Goal**: Make tool perfect for AI assistants

#### 5.1 Enhanced Output Formats
- [ ] Add `--format ai-json` mode
- [ ] Include full context for each duplicate
- [ ] Add prompts for AI refactoring
- [ ] Include file relationship graphs
- [ ] Generate refactoring scripts

#### 5.2 AI Context Export
```json
{
  "project": {
    "language": "typescript",
    "framework": "react",
    "patterns": ["hooks", "components"]
  },
  "duplicates": [...],
  "aiPrompts": {
    "refactor": "Refactor these 5 duplicates into a shared utility...",
    "analyze": "Explain why this duplication exists...",
    "fix": "Apply the following refactoring..."
  }
}
```

#### 5.3 Natural Language CLI
- [ ] Add `--ask` mode: `jscpd-ai --ask "Find duplicates in auth logic"`
- [ ] Interactive mode with conversation
- [ ] Plain English reports

---

### Phase 6: Performance & Scale (Week 6)
**Goal**: Handle large AI-generated codebases

#### 6.1 Optimizations
- [ ] Incremental analysis (only changed files)
- [ ] Parallel processing with worker threads
- [ ] Smart caching (hash-based)
- [ ] Streaming results for large repos
- [ ] Memory-efficient processing

#### 6.2 Configuration
```json
{
  "performance": {
    "incremental": true,
    "parallel": true,
    "workers": 4,
    "cache": true,
    "cacheDir": ".jscpd-ai-cache"
  }
}
```

---

## Technical Architecture

### New Package Structure
```
packages/
├── core/              (enhanced)
├── finder/            (enhanced)
├── tokenizer/         (enhanced - better Dart)
├── ollama-service/    (NEW)
├── semantic/          (NEW)
├── refactor/          (NEW)
├── ai-reporter/       (NEW)
└── ...existing reporters
```

### Data Flow
```
Source Code
    ↓
Tokenizer (enhanced Dart support)
    ↓
Traditional Detection (Rabin-Karp)
    ↓
Semantic Analysis (Ollama) ← Only for borderline cases
    ↓
Refactoring Suggestions (Ollama)
    ↓
AI-Friendly Output
```

---

## CLI Interface

### New Commands
```bash
# Traditional mode (backwards compatible)
jscpd-ai /path/to/code

# AI-enhanced mode
jscpd-ai /path/to/code --ai

# With refactoring suggestions
jscpd-ai /path/to/code --ai --suggest

# Apply refactoring
jscpd-ai /path/to/code --ai --suggest --apply

# Interactive mode
jscpd-ai /path/to/code --interactive

# Natural language query
jscpd-ai /path/to/code --ask "Find duplicates in authentication"

# AI-friendly output
jscpd-ai /path/to/code --ai --format ai-json -o duplicates.json
```

### Configuration File
```json
// .jscpd-ai.json
{
  "threshold": 5,
  "minLines": 5,
  "minTokens": 50,
  "ignore": ["**/*.test.ts", "**/*.spec.ts"],
  "ollama": {
    "enabled": true,
    "host": "http://localhost:11434",
    "model": "codellama:7b"
  },
  "ai": {
    "semanticSimilarity": true,
    "refactoringSuggestions": true,
    "confidence": 0.7
  },
  "dart": {
    "analyzeNullSafety": true,
    "detectFlutterPatterns": true
  }
}
```

---

## Ollama Models Recommendation

### Recommended Models
1. **codellama:7b** - Fast, good for similarity detection
2. **deepseek-coder:6.7b** - Excellent for code understanding
3. **starcoder2:7b** - Multi-language support
4. **codegemma:7b** - Good balance of speed/quality

### Installation Guide
```bash
# Install Ollama
curl -fsSL https://ollama.com/install.sh | sh

# Pull recommended model
ollama pull codellama:7b

# Test
ollama run codellama:7b "Explain this code: function add(a,b) { return a + b; }"
```

---

## Dart-Specific Enhancements

### Patterns to Detect
1. **Widget Duplication** (Flutter)
   ```dart
   // Detect similar widget builds
   Widget buildCard() { ... }
   Widget buildUserCard() { ... }  // Same structure, different name
   ```

2. **State Management Patterns**
   ```dart
   // Detect duplicated BLoC/Provider patterns
   ```

3. **Null Safety Patterns**
   ```dart
   // Detect similar null-checking logic
   final value = data?.field ?? defaultValue;
   ```

4. **Extension Methods**
   ```dart
   // Detect duplicated extensions
   extension StringExtension on String { ... }
   ```

---

## Testing Strategy

### Test Suite
1. **Unit Tests**
   - Each package has 80%+ coverage
   - Mock Ollama responses
   - Test edge cases

2. **Integration Tests**
   - End-to-end duplication detection
   - With/without Ollama
   - Large codebases (1000+ files)

3. **Real-World Tests**
   - Flutter apps
   - TypeScript projects
   - Multi-language repos
   - Monorepos

### Test Fixtures
```
fixtures/
├── dart/
│   ├── flutter-app/
│   ├── null-safety/
│   └── duplicates/
├── ai-generated/
│   ├── copilot-code/
│   └── chatgpt-code/
└── multi-language/
    ├── python-typescript/
    └── java-kotlin/
```

---

## Documentation

### Required Docs
1. **README.md** - Quick start, AI features
2. **OLLAMA_SETUP.md** - Detailed Ollama configuration
3. **DART_SUPPORT.md** - Dart-specific features
4. **API.md** - Programmatic usage
5. **EXAMPLES.md** - Real-world examples
6. **CONTRIBUTING.md** - How to contribute

---

## Release Plan

### Version 1.0.0 (MVP)
- ✅ Rebranded as jscpd-ai
- ✅ Enhanced Dart support
- ✅ Ollama integration
- ✅ Basic semantic similarity
- ✅ AI-friendly JSON output

### Version 1.1.0
- Refactoring suggestions
- Interactive mode
- Natural language queries

### Version 1.2.0
- Cross-language detection
- Advanced Dart/Flutter patterns
- Performance optimizations

### Version 2.0.0
- IDE extensions (VSCode, IntelliJ)
- GitHub Actions
- Cloud service option

---

## Success Metrics

### Technical
- 95%+ accuracy in duplicate detection
- <5 second analysis for 1000 files
- Works offline (Ollama)
- 80%+ test coverage

### User Experience
- Setup in <5 minutes
- Clear, actionable reports
- AI features optional (fallback to traditional)
- Backwards compatible with jscpd configs

---

## Timeline

**Total: 6 weeks to MVP**

| Week | Focus | Deliverable |
|------|-------|-------------|
| 1 | Foundation | Rebranded, enhanced Dart |
| 2 | AI Integration | Ollama service working |
| 3 | Semantic Detection | AI-powered similarity |
| 4 | Refactoring | Smart suggestions |
| 5 | UX Polish | AI-friendly outputs |
| 6 | Testing & Docs | Ready for NPM |

---

## Risk Mitigation

### Risk 1: Ollama Not Installed
**Solution**: Graceful fallback to traditional detection, clear setup docs

### Risk 2: Slow AI Analysis
**Solution**: Only use AI for borderline cases, add caching, parallel processing

### Risk 3: Inaccurate Suggestions
**Solution**: Confidence scores, user review required, opt-in feature

### Risk 4: Breaking Changes
**Solution**: Maintain backwards compatibility, gradual feature rollout

---

## Next Steps

1. ✅ Approve this plan
2. Choose final package name
3. Start Phase 1: Rebranding
4. Set up development environment
5. Begin implementation

---

**Ready to build? Let's make duplicate detection intelligent! 🚀**
