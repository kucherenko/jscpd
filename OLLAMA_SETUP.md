# Ollama Setup Guide for JSCPD-AI

This guide will help you set up Ollama for use with jscpd-ai's AI-powered features.

## What is Ollama?

Ollama is a lightweight, local AI model runtime that allows you to run large language models on your own machine. JSCPD-AI uses Ollama to provide:

- Semantic similarity analysis
- Intelligent refactoring suggestions
- Explanations for code duplication
- Context-aware code analysis

**Privacy**: All AI processing happens locally on your machine. No code is sent to external servers.

## Installation

### macOS and Linux

```bash
curl -fsSL https://ollama.com/install.sh | sh
```

### Windows

Download the installer from [ollama.com](https://ollama.com/download/windows)

### Docker

```bash
docker pull ollama/ollama
docker run -d -v ollama:/root/.ollama -p 11434:11434 --name ollama ollama/ollama
```

## Installing Models

JSCPD-AI works best with code-specialized models. Here are the recommended options:

### Quick Start (Recommended)

```bash
# Fast, 3.8GB - Good for most use cases
ollama pull codellama:7b
```

### Alternative Models

#### DeepSeek Coder (Best Code Understanding)
```bash
# 3.8GB - Excellent for code analysis
ollama pull deepseek-coder:6.7b
```

#### StarCoder2 (Multi-Language)
```bash
# 4.0GB - Great multi-language support
ollama pull starcoder2:7b
```

#### CodeGemma (Balanced)
```bash
# 5.0GB - Good balance of speed and quality
ollama pull codegemma:7b
```

#### CodeLlama 13B (Higher Quality)
```bash
# 7.4GB - Best quality, slower
ollama pull codellama:13b
```

### Check Installed Models

```bash
ollama list
```

## Verify Installation

Test that Ollama is working:

```bash
ollama run codellama:7b "Explain this code: function add(a,b) { return a + b; }"
```

You should see a response explaining the function.

## Configuration

### Default Configuration

JSCPD-AI uses these defaults:

```json
{
  "ollama": {
    "host": "http://localhost:11434",
    "model": "codellama:7b",
    "timeout": 30000,
    "enabled": true
  }
}
```

### Custom Configuration

Create a `.jscpd-ai.json` file in your project root:

```json
{
  "threshold": 5,
  "ignore": ["**/*.test.ts"],
  "ollama": {
    "enabled": true,
    "host": "http://localhost:11434",
    "model": "deepseek-coder:6.7b",
    "timeout": 45000,
    "features": {
      "semanticSimilarity": true,
      "refactoringSuggestions": true,
      "explanations": true
    }
  }
}
```

### Remote Ollama Instance

If running Ollama on a different machine:

```json
{
  "ollama": {
    "host": "http://192.168.1.100:11434",
    "model": "codellama:7b"
  }
}
```

## Usage with JSCPD-AI

### Basic AI-Enhanced Scan

```bash
jscpd-ai /path/to/code --ai
```

### With Refactoring Suggestions

```bash
jscpd-ai /path/to/code --ai --ai-refactor
```

### With Semantic Analysis

```bash
jscpd-ai /path/to/code --ai --ai-semantic
```

### Full AI Features

```bash
jscpd-ai /path/to/code --ai --ai-semantic --ai-refactor --ai-explain
```

### Generate AI Report

```bash
jscpd-ai /path/to/code --ai --reporters ai -o ./reports/
```

### Custom Model

```bash
jscpd-ai /path/to/code --ai --ai-model "deepseek-coder:6.7b"
```

### Custom Host

```bash
jscpd-ai /path/to/code --ai --ai-host "http://192.168.1.100:11434"
```

## Model Comparison

| Model | Size | Speed | Quality | Best For |
|-------|------|-------|---------|----------|
| codellama:7b | 3.8GB | ⚡⚡⚡ | ⭐⭐⭐ | General use, fast scans |
| deepseek-coder:6.7b | 3.8GB | ⚡⚡⚡ | ⭐⭐⭐⭐ | Code understanding |
| starcoder2:7b | 4.0GB | ⚡⚡ | ⭐⭐⭐⭐ | Multi-language projects |
| codegemma:7b | 5.0GB | ⚡⚡ | ⭐⭐⭐⭐ | Balanced performance |
| codellama:13b | 7.4GB | ⚡ | ⭐⭐⭐⭐⭐ | Production analysis |

## Performance Optimization

### For Large Codebases

```json
{
  "ollama": {
    "timeout": 60000,
    "features": {
      "semanticSimilarity": true,
      "refactoringSuggestions": true,
      "explanations": false  // Disable for faster processing
    }
  }
}
```

### For CI/CD Pipelines

Use the faster 7B models and disable explanations:

```bash
jscpd-ai /path/to/code --ai --ai-model "codellama:7b" --ai-refactor --silent
```

## Troubleshooting

### Ollama Not Available

**Error**: `Ollama is not available`

**Solutions**:
1. Check if Ollama is running:
   ```bash
   curl http://localhost:11434/api/tags
   ```

2. Start Ollama service:
   ```bash
   # macOS/Linux
   systemctl start ollama  # or
   ollama serve

   # Docker
   docker start ollama
   ```

3. Verify the host in your config:
   ```bash
   jscpd-ai /path/to/code --ai --ai-host "http://localhost:11434"
   ```

### Model Not Found

**Error**: `Model codellama:7b not found`

**Solution**:
```bash
ollama pull codellama:7b
```

### Timeout Errors

**Error**: `Ollama request failed: timeout`

**Solutions**:
1. Increase timeout:
   ```json
   {
     "ollama": {
       "timeout": 60000
     }
   }
   ```

2. Use a smaller model:
   ```bash
   jscpd-ai /path/to/code --ai --ai-model "codellama:7b"
   ```

3. Close other applications using the GPU/CPU

### Slow Performance

**Solutions**:
1. Use a smaller model (7B instead of 13B)
2. Disable explanations (only use refactoring suggestions)
3. Allocate more RAM to Ollama:
   ```bash
   # Linux
   export OLLAMA_MAX_LOADED_MODELS=1
   export OLLAMA_MAX_QUEUE=512
   ```

## Hardware Requirements

### Minimum

- **RAM**: 8GB
- **Disk**: 4GB free space
- **Model**: codellama:7b

### Recommended

- **RAM**: 16GB
- **Disk**: 10GB free space
- **Model**: deepseek-coder:6.7b or codellama:13b

### Optimal

- **RAM**: 32GB+
- **GPU**: NVIDIA with 8GB+ VRAM
- **Disk**: 20GB+ SSD
- **Model**: codellama:13b or larger

## GPU Acceleration

### NVIDIA (CUDA)

Ollama automatically uses CUDA if available. Verify:

```bash
ollama ps
```

Should show GPU usage.

### Apple Silicon (M1/M2/M3)

Ollama automatically uses Metal acceleration. No configuration needed.

### AMD (ROCm)

Supported on Linux. See [Ollama documentation](https://github.com/ollama/ollama/blob/main/docs/gpu.md).

## Running Without Ollama

JSCPD-AI will gracefully fallback to traditional duplicate detection if Ollama is not available:

```bash
jscpd-ai /path/to/code  # Works without AI features
```

To explicitly disable AI:

```json
{
  "ollama": {
    "enabled": false
  }
}
```

## Advanced Configuration

### Multiple Ollama Instances (Load Balancing)

Create a custom service configuration to handle multiple instances:

```typescript
import { OllamaService } from '@jscpd-ai/ollama-service';

const service1 = new OllamaService({ host: 'http://server1:11434' });
const service2 = new OllamaService({ host: 'http://server2:11434' });

// Use load balancer logic
```

### Custom Prompts

For advanced users, you can extend the OllamaService:

```typescript
import { OllamaService } from '@jscpd-ai/ollama-service';

class CustomOllamaService extends OllamaService {
  async customAnalysis(code: string): Promise<string> {
    return await this.generate(`Your custom prompt: ${code}`);
  }
}
```

## Best Practices

1. **Model Selection**: Start with `codellama:7b`, upgrade if needed
2. **Timeout**: Set timeout based on codebase size (30s small, 60s large)
3. **Features**: Enable only needed features for faster processing
4. **CI/CD**: Use smaller models and disable explanations
5. **Privacy**: Keep Ollama local for sensitive codebases

## Security & Privacy

- ✅ All processing is local
- ✅ No code sent to external servers
- ✅ No telemetry or tracking
- ✅ Full control over models and data

## Getting Help

If you encounter issues:

1. Check Ollama logs: `journalctl -u ollama` (Linux) or Console.app (macOS)
2. Test Ollama directly: `ollama run codellama:7b "test"`
3. Check jscpd-ai verbose output: `jscpd-ai /path/to/code --ai --verbose`
4. Report issues: [GitHub Issues](https://github.com/moinsen-dev/jscpd2025/issues)

## Resources

- [Ollama Documentation](https://github.com/ollama/ollama)
- [Available Models](https://ollama.com/library)
- [JSCPD-AI GitHub](https://github.com/moinsen-dev/jscpd2025)

---

**Need help?** Open an issue or check the [FAQ](https://github.com/moinsen-dev/jscpd2025/wiki/FAQ)
