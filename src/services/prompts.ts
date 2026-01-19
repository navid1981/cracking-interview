// Prompt Manager with Custom Prompts Support

export enum PromptTemplate {
  AlgorithmOptimal = 'algorithm_optimal',
  AlgorithmBeginner = 'algorithm_beginner',
  SystemDesign = 'system_design',
  CodeReview = 'code_review',
  ExplainConcept = 'explain_concept',
}

export enum ProgrammingLanguage {
  Java = 'Java',
  Python = 'Python',
  JavaScript = 'JavaScript',
  Cpp = 'C++',
  Swift = 'Swift',
}

export interface CustomPrompt {
  id: string;
  name: string;
  systemPrompt: string;
  userPrompt: string;
  supportsLanguage: boolean;
}

const GENERAL_SYSTEM_PROMPT = `You are an expert technical assistant helping with coding interview preparation.

RESPONSE FORMAT:
Always structure your response with these markers:

EXPLANATION_START
[Provide your explanation here]
EXPLANATION_END

SOLUTION_START
[Provide the solution code without markdown blocks]
SOLUTION_END

RULES:
- Be clear and concise
- Focus on understanding, not just answers
- Provide optimal solutions when asked
- Explain complexity (time/space)`;

// Default templates with placeholders for customization
const DEFAULT_USER_TEMPLATES: Record<PromptTemplate, string> = {
  [PromptTemplate.AlgorithmOptimal]: `Solve this {LANGUAGE} algorithm problem.

Requirements:
- Provide optimal time/space complexity solution
- Include complexity analysis (O notation)
- Write production-ready, clean code
- Explain your approach briefly

{CONTENT}`,

  [PromptTemplate.AlgorithmBeginner]: `Explain and solve this {LANGUAGE} algorithm problem for a beginner.

Requirements:
- Use simple, clear language
- Explain each step of your approach
- Include examples to illustrate the solution
- Provide clean, well-commented code

{CONTENT}`,

  [PromptTemplate.SystemDesign]: `Design a scalable system to solve this problem.

Requirements:
- Provide high-level architecture
- Break down into components
- Discuss scalability considerations
- Explain trade-offs and alternatives

{CONTENT}`,

  [PromptTemplate.CodeReview]: `Review this code and provide comprehensive feedback.

Requirements:
- Identify bugs and potential issues
- Suggest improvements and optimizations
- Provide performance tips
- Recommend best practices

{CONTENT}`,

  [PromptTemplate.ExplainConcept]: `Explain this technical concept clearly.

Requirements:
- Provide clear, understandable explanation
- Include real-world examples
- Discuss common use cases
- Mention related concepts

{CONTENT}`,
};

// Custom Prompts Manager
export class CustomPromptsManager {
  private static STORAGE_KEY = 'custom_prompts';

  static getAll(): CustomPrompt[] {
    const saved = localStorage.getItem(this.STORAGE_KEY);
    return saved ? JSON.parse(saved) : [];
  }

  static save(prompt: CustomPrompt): void {
    const prompts = this.getAll();
    const existing = prompts.findIndex(p => p.id === prompt.id);
    
    if (existing >= 0) {
      prompts[existing] = prompt;
    } else {
      prompts.push(prompt);
    }
    
    localStorage.setItem(this.STORAGE_KEY, JSON.stringify(prompts));
  }

  static delete(id: string): void {
    const prompts = this.getAll().filter(p => p.id !== id);
    localStorage.setItem(this.STORAGE_KEY, JSON.stringify(prompts));
  }

  static getById(id: string): CustomPrompt | null {
    return this.getAll().find(p => p.id === id) || null;
  }
}

export function buildPrompt(
  template: PromptTemplate | string,
  language: ProgrammingLanguage,
  content?: string
): string {
  let systemPrompt = GENERAL_SYSTEM_PROMPT;
  let userTemplate = '';

  // Check if it's a custom prompt ID
  if (!Object.values(PromptTemplate).includes(template as PromptTemplate)) {
    const customPrompt = CustomPromptsManager.getById(template);
    if (customPrompt) {
      systemPrompt = customPrompt.systemPrompt;
      userTemplate = customPrompt.userPrompt;
    }
  } else {
    // Built-in template
    userTemplate = DEFAULT_USER_TEMPLATES[template as PromptTemplate];
  }

  let finalPrompt = systemPrompt + '\n\n';
  
  if (userTemplate) {
    finalPrompt += userTemplate
      .replace(/{LANGUAGE}/g, language)
      .replace(/{CONTENT}/g, content || '')
      .replace(/{PROBLEM}/g, content || '');
  } else if (content) {
    finalPrompt += '\nQuestion:\n' + content;
  }

  return finalPrompt;
}

export function getTemplateLabel(template: PromptTemplate | string): string {
  if (!Object.values(PromptTemplate).includes(template as PromptTemplate)) {
    const customPrompt = CustomPromptsManager.getById(template);
    return customPrompt ? `📝 ${customPrompt.name}` : 'Custom';
  }

  switch (template as PromptTemplate) {
    case PromptTemplate.AlgorithmOptimal:
      return 'Algorithm - Optimal';
    case PromptTemplate.AlgorithmBeginner:
      return 'Algorithm - Beginner';
    case PromptTemplate.SystemDesign:
      return 'System Design';
    case PromptTemplate.CodeReview:
      return 'Code Review';
    case PromptTemplate.ExplainConcept:
      return 'Explain Concept';
  }
}

export function supportsLanguageSelection(template: PromptTemplate | string): boolean {
  if (!Object.values(PromptTemplate).includes(template as PromptTemplate)) {
    const customPrompt = CustomPromptsManager.getById(template);
    return customPrompt ? customPrompt.supportsLanguage : false;
  }

  return template === PromptTemplate.AlgorithmOptimal || 
         template === PromptTemplate.AlgorithmBeginner;
}

export function isBuiltInTemplate(template: string): boolean {
  return Object.values(PromptTemplate).includes(template as PromptTemplate);
}

export function getAllTemplates(): Array<{ id: string; label: string; isCustom: boolean }> {
  const builtIn = Object.values(PromptTemplate).map(t => ({
    id: t,
    label: getTemplateLabel(t),
    isCustom: false,
  }));

  const custom = CustomPromptsManager.getAll().map(p => ({
    id: p.id,
    label: `📝 ${p.name}`,
    isCustom: true,
  }));

  return [...builtIn, ...custom];
}

export function getDefaultSystemPrompt(): string {
  return GENERAL_SYSTEM_PROMPT;
}

export function getDefaultUserTemplate(template: PromptTemplate): string {
  return DEFAULT_USER_TEMPLATES[template] || '';
}
