/**
 * Ollama Service for jscpd-ai
 * Provides local AI-powered code analysis using Ollama
 */

import fetch from 'node-fetch';

export interface OllamaConfig {
  host: string;
  model: string;
  timeout: number;
  enabled: boolean;
  features?: {
    semanticSimilarity?: boolean;
    refactoringSuggestions?: boolean;
    explanations?: boolean;
  };
}

export const defaultOllamaConfig: OllamaConfig = {
  host: 'http://localhost:11434',
  model: 'codellama:7b',
  timeout: 30000,
  enabled: true,
  features: {
    semanticSimilarity: true,
    refactoringSuggestions: true,
    explanations: true,
  },
};

export interface OllamaResponse {
  model: string;
  created_at: string;
  response: string;
  done: boolean;
  context?: number[];
  total_duration?: number;
  load_duration?: number;
  prompt_eval_count?: number;
  eval_count?: number;
  eval_duration?: number;
}

export interface SemanticAnalysis {
  similarityScore: number; // 0-100
  confidence: number; // 0-1
  reasoning: string;
  functionallyEquivalent: boolean;
  refactoringPotential: 'high' | 'medium' | 'low';
}

export interface RefactoringSuggestion {
  type: 'extract-function' | 'extract-class' | 'create-module' | 'inline' | 'other';
  confidence: number;
  description: string;
  suggestedCode?: string;
  reasoning: string;
  risks?: string[];
}

export class OllamaService {
  private config: OllamaConfig;
  private isAvailable: boolean | null = null;

  constructor(config: Partial<OllamaConfig> = {}) {
    this.config = { ...defaultOllamaConfig, ...config };
  }

  /**
   * Check if Ollama is available and running
   */
  async checkAvailability(): Promise<boolean> {
    if (!this.config.enabled) {
      this.isAvailable = false;
      return false;
    }

    try {
      const response = await fetch(`${this.config.host}/api/tags`, {
        method: 'GET',
        headers: { 'Content-Type': 'application/json' },
        // @ts-ignore
        timeout: 5000,
      });

      if (response.ok) {
        const data = await response.json();
        this.isAvailable = true;

        // Check if the configured model is available
        const models = data.models || [];
        const hasModel = models.some((m: any) => m.name === this.config.model);

        if (!hasModel) {
          console.warn(
            `Warning: Model ${this.config.model} not found. Available models:`,
            models.map((m: any) => m.name).join(', ')
          );
        }

        return true;
      }

      this.isAvailable = false;
      return false;
    } catch (error) {
      this.isAvailable = false;
      return false;
    }
  }

  /**
   * Generate completion using Ollama
   */
  async generate(prompt: string, options: any = {}): Promise<string> {
    if (this.isAvailable === false) {
      throw new Error('Ollama is not available');
    }

    if (this.isAvailable === null) {
      await this.checkAvailability();
      if (!this.isAvailable) {
        throw new Error('Ollama is not available');
      }
    }

    try {
      const response = await fetch(`${this.config.host}/api/generate`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          model: this.config.model,
          prompt,
          stream: false,
          options: {
            temperature: options.temperature || 0.2,
            top_p: options.top_p || 0.9,
            ...options,
          },
        }),
        // @ts-ignore
        timeout: this.config.timeout,
      });

      if (!response.ok) {
        throw new Error(`Ollama request failed: ${response.statusText}`);
      }

      const data = (await response.json()) as OllamaResponse;
      return data.response;
    } catch (error) {
      if (error instanceof Error) {
        throw new Error(`Ollama generation failed: ${error.message}`);
      }
      throw error;
    }
  }

  /**
   * Analyze semantic similarity between two code blocks
   */
  async analyzeSementicSimilarity(code1: string, code2: string, language: string): Promise<SemanticAnalysis> {
    if (!this.config.features?.semanticSimilarity) {
      throw new Error('Semantic similarity analysis is disabled');
    }

    const prompt = `You are a code analysis expert. Compare these two ${language} code blocks and determine their semantic similarity.

Code Block 1:
\`\`\`${language}
${code1}
\`\`\`

Code Block 2:
\`\`\`${language}
${code2}
\`\`\`

Analyze:
1. Similarity Score (0-100): How similar are they functionally?
2. Confidence (0-1): How confident are you in this analysis?
3. Functionally Equivalent (yes/no): Do they do the same thing?
4. Refactoring Potential (high/medium/low): Should they be refactored?
5. Reasoning: Brief explanation (2-3 sentences)

Respond in JSON format:
{
  "similarityScore": <number>,
  "confidence": <number>,
  "functionallyEquivalent": <boolean>,
  "refactoringPotential": "<high|medium|low>",
  "reasoning": "<string>"
}`;

    try {
      const response = await this.generate(prompt, { temperature: 0.1 });
      const jsonMatch = response.match(/\{[\s\S]*\}/);

      if (jsonMatch) {
        const analysis = JSON.parse(jsonMatch[0]);
        return {
          similarityScore: analysis.similarityScore || 0,
          confidence: analysis.confidence || 0,
          reasoning: analysis.reasoning || 'No reasoning provided',
          functionallyEquivalent: analysis.functionallyEquivalent || false,
          refactoringPotential: analysis.refactoringPotential || 'low',
        };
      }

      // Fallback if JSON parsing fails
      return {
        similarityScore: 50,
        confidence: 0.5,
        reasoning: 'Failed to parse AI response',
        functionallyEquivalent: false,
        refactoringPotential: 'low',
      };
    } catch (error) {
      console.error('Semantic similarity analysis failed:', error);
      throw error;
    }
  }

  /**
   * Generate refactoring suggestions for duplicated code
   */
  async generateRefactoringSuggestion(
    duplicates: Array<{ code: string; file: string; line: number }>,
    language: string
  ): Promise<RefactoringSuggestion> {
    if (!this.config.features?.refactoringSuggestions) {
      throw new Error('Refactoring suggestions are disabled');
    }

    const duplicatesText = duplicates
      .map((d, i) => `Location ${i + 1} (${d.file}:${d.line}):\n\`\`\`${language}\n${d.code}\n\`\`\``)
      .join('\n\n');

    const prompt = `You are a refactoring expert. Analyze these duplicated ${language} code blocks and suggest the best refactoring approach.

${duplicatesText}

Provide:
1. Type: extract-function, extract-class, create-module, inline, or other
2. Confidence (0-1): How confident are you in this suggestion?
3. Description: Brief description of the refactoring
4. Suggested Code: The refactored code (if applicable)
5. Reasoning: Why this refactoring is recommended
6. Risks: Potential risks or considerations

Respond in JSON format:
{
  "type": "<type>",
  "confidence": <number>,
  "description": "<string>",
  "suggestedCode": "<string>",
  "reasoning": "<string>",
  "risks": ["<string>", ...]
}`;

    try {
      const response = await this.generate(prompt, { temperature: 0.2 });
      const jsonMatch = response.match(/\{[\s\S]*\}/);

      if (jsonMatch) {
        const suggestion = JSON.parse(jsonMatch[0]);
        return {
          type: suggestion.type || 'other',
          confidence: suggestion.confidence || 0.5,
          description: suggestion.description || 'No description provided',
          suggestedCode: suggestion.suggestedCode,
          reasoning: suggestion.reasoning || 'No reasoning provided',
          risks: suggestion.risks || [],
        };
      }

      return {
        type: 'other',
        confidence: 0.5,
        description: 'Failed to parse refactoring suggestion',
        reasoning: 'AI response could not be parsed',
      };
    } catch (error) {
      console.error('Refactoring suggestion generation failed:', error);
      throw error;
    }
  }

  /**
   * Explain why code duplication might exist
   */
  async explainDuplication(code: string, locations: string[], language: string): Promise<string> {
    if (!this.config.features?.explanations) {
      throw new Error('Explanations are disabled');
    }

    const locationsText = locations.join('\n- ');

    const prompt = `You are a code quality expert. This ${language} code is duplicated in multiple locations:

Code:
\`\`\`${language}
${code}
\`\`\`

Locations:
- ${locationsText}

Explain in 2-3 sentences:
1. Why this duplication might exist
2. The potential issues it causes
3. How it should be addressed`;

    try {
      return await this.generate(prompt, { temperature: 0.3 });
    } catch (error) {
      console.error('Explanation generation failed:', error);
      return 'Failed to generate explanation';
    }
  }

  /**
   * Batch analyze multiple code pairs for similarity
   */
  async batchAnalyzeSimilarity(
    pairs: Array<{ code1: string; code2: string }>,
    language: string
  ): Promise<SemanticAnalysis[]> {
    const results: SemanticAnalysis[] = [];

    // Process in batches to avoid overwhelming Ollama
    const batchSize = 5;
    for (let i = 0; i < pairs.length; i += batchSize) {
      const batch = pairs.slice(i, i + batchSize);
      const batchResults = await Promise.all(
        batch.map(pair => this.analyzeSementicSimilarity(pair.code1, pair.code2, language))
      );
      results.push(...batchResults);
    }

    return results;
  }

  /**
   * Get recommended models for different tasks
   */
  static getRecommendedModels(): {
    [key: string]: { name: string; description: string; size: string };
  } {
    return {
      fast: {
        name: 'codellama:7b',
        description: 'Fast, good for similarity detection',
        size: '3.8GB',
      },
      balanced: {
        name: 'deepseek-coder:6.7b',
        description: 'Excellent code understanding',
        size: '3.8GB',
      },
      quality: {
        name: 'codellama:13b',
        description: 'Higher quality analysis',
        size: '7.4GB',
      },
      multilang: {
        name: 'starcoder2:7b',
        description: 'Multi-language support',
        size: '4.0GB',
      },
    };
  }

  /**
   * Update configuration
   */
  updateConfig(config: Partial<OllamaConfig>): void {
    this.config = { ...this.config, ...config };
    this.isAvailable = null; // Reset availability check
  }

  /**
   * Get current configuration
   */
  getConfig(): OllamaConfig {
    return { ...this.config };
  }
}

export default OllamaService;
