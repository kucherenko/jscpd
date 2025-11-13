/**
 * AI Reporter for jscpd-ai
 * Generates AI-enhanced reports with semantic analysis and refactoring suggestions
 */

import { IClone, IOptions, IReporter, IStatistic } from '@jscpd-ai/core';
import { OllamaService, SemanticAnalysis, RefactoringSuggestion } from '@jscpd-ai/ollama-service';
import { writeFile } from 'fs-extra';
import { join } from 'path';

export interface AIReportOptions {
  output?: string;
  format?: 'json' | 'markdown' | 'html';
  includeRefactoringSuggestions?: boolean;
  includeSemanticAnalysis?: boolean;
  includeExplanations?: boolean;
  minSimilarityForAI?: number; // Only use AI for similarities above this threshold (40-70%)
}

export interface AIEnhancedClone extends IClone {
  semanticAnalysis?: SemanticAnalysis;
  refactoringSuggestion?: RefactoringSuggestion;
  explanation?: string;
  aiProcessed?: boolean;
}

export interface AIReport {
  summary: {
    totalClones: number;
    totalFiles: number;
    totalLines: number;
    duplicatePercentage: number;
    aiAnalyzedClones: number;
  };
  clones: AIEnhancedClone[];
  refactoringSummary?: {
    highPriority: number;
    mediumPriority: number;
    lowPriority: number;
    totalSuggestions: number;
  };
  recommendations: string[];
  generatedAt: string;
  ollamaModel?: string;
}

export class AIReporter implements IReporter {
  private ollamaService: OllamaService | null = null;
  private options: AIReportOptions;
  private clones: AIEnhancedClone[] = [];

  constructor(options: AIReportOptions = {}) {
    this.options = {
      format: 'json',
      includeRefactoringSuggestions: true,
      includeSemanticAnalysis: true,
      includeExplanations: false,
      minSimilarityForAI: 40,
      ...options,
    };

    // Try to initialize Ollama service
    this.initializeOllama();
  }

  private async initializeOllama(): Promise<void> {
    try {
      this.ollamaService = new OllamaService();
      const isAvailable = await this.ollamaService.checkAvailability();
      if (!isAvailable) {
        console.warn('Ollama is not available. AI features will be disabled.');
        this.ollamaService = null;
      }
    } catch (error) {
      console.warn('Failed to initialize Ollama:', error);
      this.ollamaService = null;
    }
  }

  getName(): string {
    return 'ai';
  }

  async attach(eventEmitter: any): Promise<void> {
    eventEmitter.on('CLONE_FOUND', this.onCloneFound.bind(this));
    eventEmitter.on('END', this.onEnd.bind(this));
  }

  private async onCloneFound(clone: IClone): Promise<void> {
    const enhancedClone: AIEnhancedClone = { ...clone, aiProcessed: false };

    // Check if we should use AI for this clone
    if (this.shouldUseAI(clone)) {
      try {
        if (this.options.includeSemanticAnalysis && this.ollamaService) {
          enhancedClone.semanticAnalysis = await this.analyzeSemantics(clone);
        }

        if (this.options.includeRefactoringSuggestions && this.ollamaService) {
          enhancedClone.refactoringSuggestion = await this.generateRefactoring(clone);
        }

        if (this.options.includeExplanations && this.ollamaService) {
          enhancedClone.explanation = await this.explainDuplication(clone);
        }

        enhancedClone.aiProcessed = true;
      } catch (error) {
        console.error('AI processing failed for clone:', error);
      }
    }

    this.clones.push(enhancedClone);
  }

  private shouldUseAI(clone: IClone): boolean {
    // Only use AI for borderline cases where traditional detection might need extra confidence
    // or for generating refactoring suggestions
    if (!this.ollamaService) {
      return false;
    }

    // Always use AI if enabled and Ollama is available
    return true;
  }

  private async analyzeSemantics(clone: IClone): Promise<SemanticAnalysis | undefined> {
    if (!this.ollamaService || !clone.duplicationA || !clone.duplicationB) {
      return undefined;
    }

    try {
      const code1 = clone.duplicationA.sourceCode || '';
      const code2 = clone.duplicationB.sourceCode || '';
      const language = clone.format || 'unknown';

      return await this.ollamaService.analyzeSementicSimilarity(code1, code2, language);
    } catch (error) {
      console.error('Semantic analysis failed:', error);
      return undefined;
    }
  }

  private async generateRefactoring(clone: IClone): Promise<RefactoringSuggestion | undefined> {
    if (!this.ollamaService || !clone.duplicationA || !clone.duplicationB) {
      return undefined;
    }

    try {
      const duplicates = [
        {
          code: clone.duplicationA.sourceCode || '',
          file: clone.duplicationA.sourceId || 'unknown',
          line: clone.duplicationA.start?.line || 0,
        },
        {
          code: clone.duplicationB.sourceCode || '',
          file: clone.duplicationB.sourceId || 'unknown',
          line: clone.duplicationB.start?.line || 0,
        },
      ];

      const language = clone.format || 'unknown';
      return await this.ollamaService.generateRefactoringSuggestion(duplicates, language);
    } catch (error) {
      console.error('Refactoring suggestion failed:', error);
      return undefined;
    }
  }

  private async explainDuplication(clone: IClone): Promise<string | undefined> {
    if (!this.ollamaService || !clone.duplicationA) {
      return undefined;
    }

    try {
      const code = clone.duplicationA.sourceCode || '';
      const locations = [
        `${clone.duplicationA.sourceId}:${clone.duplicationA.start?.line}`,
        `${clone.duplicationB?.sourceId}:${clone.duplicationB?.start?.line}`,
      ];
      const language = clone.format || 'unknown';

      return await this.ollamaService.explainDuplication(code, locations, language);
    } catch (error) {
      console.error('Explanation generation failed:', error);
      return undefined;
    }
  }

  private async onEnd(statistic: IStatistic): Promise<void> {
    const report = this.generateReport(statistic);

    // Save report
    if (this.options.output) {
      await this.saveReport(report);
    }

    // Print summary
    this.printSummary(report);
  }

  private generateReport(statistic: IStatistic): AIReport {
    const aiAnalyzedClones = this.clones.filter(c => c.aiProcessed).length;

    const refactoringSummary = this.calculateRefactoringSummary();

    const recommendations = this.generateRecommendations(statistic, refactoringSummary);

    return {
      summary: {
        totalClones: this.clones.length,
        totalFiles: statistic.formats ? Object.keys(statistic.formats).length : 0,
        totalLines: statistic.total?.lines || 0,
        duplicatePercentage: statistic.percentage || 0,
        aiAnalyzedClones,
      },
      clones: this.clones,
      refactoringSummary,
      recommendations,
      generatedAt: new Date().toISOString(),
      ollamaModel: this.ollamaService?.getConfig().model,
    };
  }

  private calculateRefactoringSummary() {
    let highPriority = 0;
    let mediumPriority = 0;
    let lowPriority = 0;

    this.clones.forEach(clone => {
      if (clone.refactoringSuggestion) {
        const potential = clone.refactoringSuggestion.type;
        if (potential === 'extract-function' || potential === 'extract-class') {
          highPriority++;
        } else if (potential === 'create-module') {
          mediumPriority++;
        } else {
          lowPriority++;
        }
      }
    });

    return {
      highPriority,
      mediumPriority,
      lowPriority,
      totalSuggestions: highPriority + mediumPriority + lowPriority,
    };
  }

  private generateRecommendations(statistic: IStatistic, refactoringSummary: any): string[] {
    const recommendations: string[] = [];

    if (statistic.percentage && statistic.percentage > 10) {
      recommendations.push(
        `High duplication detected (${statistic.percentage.toFixed(1)}%). Consider refactoring.`
      );
    }

    if (refactoringSummary.highPriority > 0) {
      recommendations.push(
        `${refactoringSummary.highPriority} high-priority refactoring opportunities identified.`
      );
    }

    if (refactoringSummary.totalSuggestions > 5) {
      recommendations.push(
        'Multiple refactoring opportunities detected. Consider addressing them incrementally.'
      );
    }

    if (!this.ollamaService) {
      recommendations.push(
        'Install Ollama and enable AI features for advanced analysis and refactoring suggestions.'
      );
    }

    return recommendations;
  }

  private async saveReport(report: AIReport): Promise<void> {
    const outputPath = this.options.output || 'jscpd-ai-report.json';

    try {
      if (this.options.format === 'json') {
        await writeFile(outputPath, JSON.stringify(report, null, 2));
      } else if (this.options.format === 'markdown') {
        const markdown = this.generateMarkdownReport(report);
        await writeFile(outputPath.replace('.json', '.md'), markdown);
      }

      console.log(`\nAI report saved to: ${outputPath}`);
    } catch (error) {
      console.error('Failed to save report:', error);
    }
  }

  private generateMarkdownReport(report: AIReport): string {
    let markdown = `# JSCPD-AI Code Duplication Report\n\n`;
    markdown += `Generated: ${report.generatedAt}\n`;
    if (report.ollamaModel) {
      markdown += `AI Model: ${report.ollamaModel}\n`;
    }
    markdown += `\n## Summary\n\n`;
    markdown += `- Total Clones: ${report.summary.totalClones}\n`;
    markdown += `- Total Files: ${report.summary.totalFiles}\n`;
    markdown += `- Duplicate Percentage: ${report.summary.duplicatePercentage.toFixed(2)}%\n`;
    markdown += `- AI-Analyzed Clones: ${report.summary.aiAnalyzedClones}\n`;

    if (report.refactoringSummary) {
      markdown += `\n## Refactoring Summary\n\n`;
      markdown += `- High Priority: ${report.refactoringSummary.highPriority}\n`;
      markdown += `- Medium Priority: ${report.refactoringSummary.mediumPriority}\n`;
      markdown += `- Low Priority: ${report.refactoringSummary.lowPriority}\n`;
    }

    if (report.recommendations.length > 0) {
      markdown += `\n## Recommendations\n\n`;
      report.recommendations.forEach(rec => {
        markdown += `- ${rec}\n`;
      });
    }

    markdown += `\n## Detailed Clones\n\n`;
    report.clones.forEach((clone, index) => {
      markdown += `### Clone ${index + 1}\n\n`;
      markdown += `- Format: ${clone.format}\n`;
      markdown += `- Lines: ${clone.linesCount}\n`;

      if (clone.semanticAnalysis) {
        markdown += `\n**AI Analysis:**\n`;
        markdown += `- Similarity Score: ${clone.semanticAnalysis.similarityScore}/100\n`;
        markdown += `- Confidence: ${(clone.semanticAnalysis.confidence * 100).toFixed(0)}%\n`;
        markdown += `- Functionally Equivalent: ${clone.semanticAnalysis.functionallyEquivalent ? 'Yes' : 'No'}\n`;
        markdown += `- Reasoning: ${clone.semanticAnalysis.reasoning}\n`;
      }

      if (clone.refactoringSuggestion) {
        markdown += `\n**Refactoring Suggestion:**\n`;
        markdown += `- Type: ${clone.refactoringSuggestion.type}\n`;
        markdown += `- Confidence: ${(clone.refactoringSuggestion.confidence * 100).toFixed(0)}%\n`;
        markdown += `- Description: ${clone.refactoringSuggestion.description}\n`;
        markdown += `- Reasoning: ${clone.refactoringSuggestion.reasoning}\n`;
      }

      markdown += `\n---\n\n`;
    });

    return markdown;
  }

  private printSummary(report: AIReport): void {
    console.log('\n╔════════════════════════════════════════════════╗');
    console.log('║        JSCPD-AI Report Summary                 ║');
    console.log('╠════════════════════════════════════════════════╣');
    console.log(`║ Total Clones: ${String(report.summary.totalClones).padEnd(33)}║`);
    console.log(`║ Duplicate %:  ${String(report.summary.duplicatePercentage.toFixed(2) + '%').padEnd(33)}║`);
    console.log(`║ AI Analyzed:  ${String(report.summary.aiAnalyzedClones).padEnd(33)}║`);

    if (report.refactoringSummary && report.refactoringSummary.totalSuggestions > 0) {
      console.log('╠════════════════════════════════════════════════╣');
      console.log(`║ High Priority Refactoring: ${String(report.refactoringSummary.highPriority).padEnd(19)}║`);
      console.log(`║ Medium Priority:           ${String(report.refactoringSummary.mediumPriority).padEnd(19)}║`);
      console.log(`║ Low Priority:              ${String(report.refactoringSummary.lowPriority).padEnd(19)}║`);
    }

    console.log('╚════════════════════════════════════════════════╝');

    if (report.recommendations.length > 0) {
      console.log('\nRecommendations:');
      report.recommendations.forEach((rec, i) => {
        console.log(`  ${i + 1}. ${rec}`);
      });
    }
  }
}

export default AIReporter;
