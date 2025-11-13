# JSCPD-AI Features Guide

This document provides a comprehensive guide to using the AI-enhanced features in jscpd-ai.

## Table of Contents

- [Overview](#overview)
- [AI Features](#ai-features)
- [Usage Examples](#usage-examples)
- [Configuration](#configuration)
- [API Usage](#api-usage)
- [Best Practices](#best-practices)

## Overview

JSCPD-AI extends the traditional code duplicate detection with AI-powered semantic analysis using local Ollama models. All processing happens on your machine for maximum privacy.

## AI Features

### 1. Semantic Similarity Analysis

Detects functionally similar code that looks different syntactically.

**What it detects:**
- Same logic with different variable names
- Same algorithm with different implementations
- Refactored code that serves the same purpose
- Cross-language similarities

**Example:**

```typescript
// Function 1
async function fetchUserData(id: string) {
  const response = await fetch(`/api/users/${id}`);
  if (!response.ok) throw new Error('Failed');
  return await response.json();
}

// Function 2 - Functionally identical
async function getUserInfo(userId: string) {
  const result = await fetch(`/api/users/${userId}`);
  if (result.status !== 200) throw new Error('Failed');
  return result.json();
}
```

**AI Output:**
```json
{
  "similarityScore": 92,
  "confidence": 0.88,
  "functionallyEquivalent": true,
  "refactoringPotential": "high",
  "reasoning": "Both functions perform async user data fetching with error handling"
}
```

### 2. Refactoring Suggestions

AI analyzes duplicates and suggests specific refactoring strategies.

**Suggestion Types:**
- `extract-function`: Extract common code into a shared function
- `extract-class`: Create a class for related methods
- `create-module`: Move to a shared utility module
- `inline`: Remove unnecessary abstraction
- `other`: Custom suggestions

**Example Output:**

```json
{
  "type": "extract-function",
  "confidence": 0.87,
  "description": "Extract user validation logic into shared utility",
  "suggestedCode": "export function validateUser(user: User): ValidationResult { ... }",
  "reasoning": "This validation pattern appears in 5 different components",
  "risks": [
    "May affect error handling in ProfileComponent",
    "Consider updating tests for ValidationService"
  ]
}
```

### 3. Duplication Explanations

AI explains why duplication exists and suggests solutions.

**Example:**

```
This form validation logic is duplicated across the login and registration
components because both require email validation. Consider creating a shared
validation utility in src/utils/validators.ts. This will reduce maintenance
burden and ensure consistent validation rules across the application.
```

### 4. Enhanced Dart/Flutter Support

Special analysis for Flutter widget patterns:

**Detected Patterns:**
- Similar widget builds
- Repeated state management patterns
- Common setState usage
- Deeply nested widget trees
- Null safety patterns

**Example Analysis:**

```dart
// Widget 1
Widget buildUserCard(User user) {
  return Card(
    child: Column(
      children: [
        Text(user.name),
        Text(user.email),
        ElevatedButton(...)
      ]
    )
  );
}

// Widget 2
Widget buildProfileCard(Profile profile) {
  return Card(
    child: Column(
      children: [
        Text(profile.name),
        Text(profile.email),
        ElevatedButton(...)
      ]
    )
  );
}
```

**AI Suggestion:**
```json
{
  "type": "extract-function",
  "description": "Extract card widget with common structure",
  "suggestedCode": "Widget buildInfoCard(String name, String email, VoidCallback onTap)",
  "reasoning": "Same widget structure used for both user and profile displays",
  "dartSpecific": {
    "widgetSimilarity": 0.95,
    "usesNullSafety": true,
    "isStateful": false
  }
}
```

## Usage Examples

### Basic AI Scan

```bash
jscpd-ai /path/to/code --ai
```

This enables AI features with default settings.

### Semantic Analysis Only

```bash
jscpd-ai /path/to/code --ai --ai-semantic
```

Analyzes semantic similarity without generating suggestions.

### Refactoring Suggestions

```bash
jscpd-ai /path/to/code --ai --ai-refactor
```

Generates refactoring suggestions for all duplicates.

### Full AI Analysis

```bash
jscpd-ai /path/to/code --ai --ai-semantic --ai-refactor --ai-explain
```

Enables all AI features:
- Semantic similarity analysis
- Refactoring suggestions
- Natural language explanations

### With Custom Model

```bash
jscpd-ai /path/to/code --ai --ai-model "deepseek-coder:6.7b"
```

Use a different Ollama model (better for code understanding).

### Generate AI Report

```bash
jscpd-ai /path/to/code --ai --reporters ai -o ./reports/
```

Creates detailed AI report in JSON format.

### Markdown Report

```bash
jscpd-ai /path/to/code --ai --reporters ai --ai-report ./report.md
```

## Configuration

### Configuration File

Create `.jscpd-ai.json`:

```json
{
  "threshold": 5,
  "minLines": 5,
  "minTokens": 50,
  "ignore": ["**/*.test.ts", "**/node_modules/**"],
  "reporters": ["console", "ai"],
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
    "analyzeNullSafety": true,
    "ignoreNullSafetyDifferences": false,
    "minWidgetSimilarity": 0.7
  }
}
```

### Ollama Configuration

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
      "explanations": false
    }
  }
}
```

**Options:**
- `enabled`: Enable/disable AI features
- `host`: Ollama server URL
- `model`: Model name (see [Ollama Setup](OLLAMA_SETUP.md))
- `timeout`: Request timeout in milliseconds
- `features`: Enable specific AI features

### Dart Configuration

```json
{
  "dart": {
    "detectFlutterPatterns": true,
    "analyzeNullSafety": true,
    "ignoreNullSafetyDifferences": false,
    "minWidgetSimilarity": 0.7
  }
}
```

**Options:**
- `detectFlutterPatterns`: Detect Flutter widget patterns
- `analyzeNullSafety`: Analyze null safety usage
- `ignoreNullSafetyDifferences`: Ignore differences in null safety operators
- `minWidgetSimilarity`: Minimum similarity for widget structure matching (0-1)

## API Usage

### Basic API

```typescript
import { detectClones } from 'jscpd-ai';

const clones = await detectClones({
  path: ['./src'],
  silent: true,
  reporters: ['ai'],
  ollama: {
    enabled: true,
    model: 'codellama:7b'
  }
});

console.log(`Found ${clones.length} duplicates`);
```

### With AI Analysis

```typescript
import { detectClones } from 'jscpd-ai';

const clones = await detectClones({
  path: ['./src'],
  silent: false,
  reporters: ['ai'],
  ollama: {
    enabled: true,
    model: 'deepseek-coder:6.7b',
    features: {
      semanticSimilarity: true,
      refactoringSuggestions: true
    }
  }
});

// Access AI analysis
for (const clone of clones) {
  if (clone.semanticAnalysis) {
    console.log(`Similarity: ${clone.semanticAnalysis.similarityScore}/100`);
    console.log(`Confidence: ${clone.semanticAnalysis.confidence}`);
    console.log(`Reasoning: ${clone.semanticAnalysis.reasoning}`);
  }

  if (clone.refactoringSuggestion) {
    console.log(`Suggestion: ${clone.refactoringSuggestion.type}`);
    console.log(`Code: ${clone.refactoringSuggestion.suggestedCode}`);
  }
}
```

### Using Ollama Service Directly

```typescript
import { OllamaService } from '@jscpd-ai/ollama-service';

const ollama = new OllamaService({
  model: 'codellama:7b',
  host: 'http://localhost:11434'
});

// Check if Ollama is available
const isAvailable = await ollama.checkAvailability();
if (!isAvailable) {
  console.error('Ollama not available');
  process.exit(1);
}

// Analyze similarity
const analysis = await ollama.analyzeSementicSimilarity(
  code1,
  code2,
  'typescript'
);

console.log('Similarity Score:', analysis.similarityScore);
console.log('Functionally Equivalent:', analysis.functionallyEquivalent);
console.log('Refactoring Potential:', analysis.refactoringPotential);

// Generate refactoring suggestion
const duplicates = [
  { code: code1, file: 'src/a.ts', line: 10 },
  { code: code2, file: 'src/b.ts', line: 20 }
];

const suggestion = await ollama.generateRefactoringSuggestion(
  duplicates,
  'typescript'
);

console.log('Suggestion Type:', suggestion.type);
console.log('Confidence:', suggestion.confidence);
console.log('Description:', suggestion.description);
console.log('Suggested Code:', suggestion.suggestedCode);
```

### Batch Processing

```typescript
import { OllamaService } from '@jscpd-ai/ollama-service';

const ollama = new OllamaService();

const pairs = [
  { code1: func1, code2: func2 },
  { code1: func3, code2: func4 },
  // ... more pairs
];

const results = await ollama.batchAnalyzeSimilarity(
  pairs,
  'typescript'
);

results.forEach((result, index) => {
  console.log(`Pair ${index + 1}:`);
  console.log(`  Similarity: ${result.similarityScore}`);
  console.log(`  Reasoning: ${result.reasoning}`);
});
```

## Best Practices

### 1. Model Selection

**For Quick Scans:**
```bash
jscpd-ai ./src --ai --ai-model "codellama:7b"
```

**For Production Analysis:**
```bash
jscpd-ai ./src --ai --ai-model "deepseek-coder:6.7b"
```

**For Best Quality:**
```bash
jscpd-ai ./src --ai --ai-model "codellama:13b"
```

### 2. Performance Optimization

**For Large Codebases:**
```json
{
  "ollama": {
    "timeout": 60000,
    "features": {
      "semanticSimilarity": true,
      "refactoringSuggestions": true,
      "explanations": false  // Disable for speed
    }
  }
}
```

**For CI/CD:**
```bash
# Use traditional detection in CI, AI for manual reviews
jscpd-ai ./src --threshold 10 --silent
```

### 3. Incremental Adoption

**Week 1: Traditional mode**
```bash
jscpd-ai ./src
```

**Week 2: Add semantic analysis**
```bash
jscpd-ai ./src --ai --ai-semantic
```

**Week 3: Add refactoring suggestions**
```bash
jscpd-ai ./src --ai --ai-semantic --ai-refactor
```

### 4. Team Workflow

1. **Developer**: Run traditional scan locally
   ```bash
   jscpd-ai ./src --silent
   ```

2. **Pre-commit**: Quick check
   ```bash
   jscpd-ai ./src --threshold 5 --silent
   ```

3. **CI/CD**: Full check
   ```bash
   jscpd-ai ./src --threshold 3 --reporters json -o ./reports
   ```

4. **Weekly Review**: AI analysis
   ```bash
   jscpd-ai ./src --ai --ai-refactor --reporters ai,html
   ```

### 5. Privacy & Security

✅ **Good:**
- Running Ollama locally
- Using self-hosted Ollama instances
- Analyzing open-source code

⚠️ **Consider:**
- Running on company network (firewall config)
- Very large models on shared machines
- Resource usage during business hours

❌ **Avoid:**
- Sending proprietary code to external APIs
- Using cloud Ollama instances for sensitive code
- Storing AI reports in public locations

## Troubleshooting

### AI Features Not Working

1. Check Ollama is running:
   ```bash
   curl http://localhost:11434/api/tags
   ```

2. Verify model is installed:
   ```bash
   ollama list
   ```

3. Test model directly:
   ```bash
   ollama run codellama:7b "test"
   ```

4. Enable verbose mode:
   ```bash
   jscpd-ai ./src --ai --verbose
   ```

### Slow Performance

1. Use smaller model:
   ```bash
   --ai-model "codellama:7b"
   ```

2. Disable explanations:
   ```json
   {
     "ollama": {
       "features": {
         "explanations": false
       }
     }
   }
   ```

3. Increase timeout:
   ```json
   {
     "ollama": {
       "timeout": 60000
     }
   }
   ```

### Inaccurate Results

1. Try a different model:
   ```bash
   --ai-model "deepseek-coder:6.7b"
   ```

2. Check confidence scores:
   ```typescript
   if (analysis.confidence > 0.8) {
     // High confidence
   }
   ```

3. Review AI reasoning:
   ```typescript
   console.log(analysis.reasoning);
   ```

## Examples

See the [examples directory](examples/) for complete working examples:

- `basic-ai-scan/` - Simple AI-enhanced scan
- `refactoring-workflow/` - Complete refactoring workflow
- `flutter-analysis/` - Flutter project analysis
- `ci-cd-integration/` - CI/CD pipeline example
- `custom-ollama/` - Custom Ollama configuration

## Resources

- [Ollama Setup Guide](OLLAMA_SETUP.md)
- [Implementation Plan](IMPLEMENTATION_PLAN.md)
- [API Documentation](apps/jscpd/README.md)
- [GitHub Issues](https://github.com/moinsen-dev/jscpd2025/issues)

---

**Questions?** Open an issue or discussion on GitHub!
