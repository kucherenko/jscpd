export type Tone = 'neutral' | 'warning';

export function badge(label: string, tone: Tone = 'neutral'): string {
  return `[${tone}] ${label}`;
}

export function pill(label: string): string {
  return `(${label})`;
}
