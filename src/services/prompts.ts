// Prompt Manager with Custom Prompts Support

export enum PromptTemplate {
  AlgorithmOptimal = 'algorithm_optimal',
  AlgorithmBeginner = 'algorithm_beginner',
  SystemDesign = 'system_design',
  CodeReview = 'code_review',
  ExplainConcept = 'explain_concept',
  VerbalInterviewAudio = 'verbal_interview_audio',
}

export enum ProgrammingLanguage {
  Java = 'Java',
  Python = 'Python',
  JavaScript = 'JavaScript',
  Cpp = 'C++',
  Swift = 'Swift',
  Go = 'Go',
  PHP = 'PHP',
  Ruby = 'Ruby',
  SQL = 'SQL',
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
Always structure your response using these exact markers:

EXPLANATION_START
[Your explanation — 2-3 short paragraphs maximum]
EXPLANATION_END

SOLUTION_START
[Raw code only — no markdown fences, no prose, no comments explaining approach]
SOLUTION_END

STRICT RULES:
- Do NOT repeat or restate the problem/question
- Keep explanations concise: maximum 2-3 short paragraphs
- In SOLUTION block: write ONLY raw code — never use \`\`\` markdown code fences
- Include time and space complexity at the end of the explanation
- If the answer is not code (e.g. system design), use SOLUTION block for the structured answer
- Always include both EXPLANATION_START/END and SOLUTION_START/END markers, even if one section is brief`;

const DEFAULT_SYSTEM_PROMPTS: Partial<Record<PromptTemplate, string>> = {
  // Revert Audio template to the original "coding interview" system prompt format
  // (includes SOLUTION_START / SOLUTION_END markers).
  [PromptTemplate.VerbalInterviewAudio]: GENERAL_SYSTEM_PROMPT,
};

// Default templates with placeholders for customization
const DEFAULT_USER_TEMPLATES: Record<PromptTemplate, string> = {
  [PromptTemplate.AlgorithmOptimal]: `Solve this {LANGUAGE} algorithm problem with the most optimal approach.

Requirements:
- Jump straight into the approach — name the technique (e.g. DP, two pointers, sliding window)
- Explain the key insight in 1-2 paragraphs, then state time/space complexity
- Write clean, production-ready code with no unnecessary comments
- Do NOT restate or summarize the problem

{CONTENT}`,

  [PromptTemplate.AlgorithmBeginner]: `Explain and solve this {LANGUAGE} algorithm problem for a beginner.

Requirements:
- Explain the key intuition in 2-3 short paragraphs using simple language
- Walk through the approach with a small example if helpful
- State time/space complexity
- Write clean code with brief inline comments on non-obvious lines only
- Do NOT restate or summarize the problem

{CONTENT}`,

  [PromptTemplate.SystemDesign]: `Design a scalable system for this problem.

Requirements:
- In EXPLANATION: summarize the approach, key trade-offs, and scaling strategy (2-3 paragraphs max)
- In SOLUTION: provide the structured design using this format:
  1. Requirements (functional + non-functional)
  2. High-Level Architecture (components and data flow)
  3. Core Components (with brief responsibility descriptions)
  4. Data Model (key entities and relationships)
  5. Scaling Strategy (bottlenecks and solutions)
- Do NOT restate or summarize the problem

{CONTENT}`,

  [PromptTemplate.CodeReview]: `Review this code and provide actionable feedback.

Requirements:
- In EXPLANATION: summarize the top issues found — bugs, performance problems, anti-patterns (2-3 paragraphs max)
- In SOLUTION: provide the improved/fixed version of the code with the issues resolved
- Focus on: correctness, performance, readability, and best practices
- Do NOT restate or summarize the problem

{CONTENT}`,

  [PromptTemplate.ExplainConcept]: `Explain this technical concept clearly.

Requirements:
- In EXPLANATION: provide a clear explanation in 2-3 paragraphs — what it is, why it matters, and when to use it
- In SOLUTION: provide a practical code example demonstrating the concept (if applicable), or a structured summary with key points
- Include a real-world analogy if it helps understanding
- Do NOT restate or summarize the problem

{CONTENT}`,

  [PromptTemplate.VerbalInterviewAudio]: `You will receive an AUDIO recording containing a verbal interview question.

Your tasks:
1) Transcribe the question briefly (key details, constraints, terminology only — not word-for-word).
2) Answer with a strong, structured response appropriate for the role/context.
3) If ambiguous, state your assumptions and provide a best-effort answer.

Requirements:
- In EXPLANATION: put the transcription and your structured answer (2-3 paragraphs max)
- In SOLUTION: if the question involves coding, provide the code; otherwise provide a concise structured answer with key points
- Keep the tone professional and confident
- Use bullet points where helpful

Note: The audio is attached; you must use it. Do not ask the user to paste the audio.`,
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
    const builtInId = template as PromptTemplate;
    // Allow per-template overrides stored in localStorage
    const overriddenSystem = localStorage.getItem(`custom_${builtInId}_system`);
    const overriddenUser = localStorage.getItem(`custom_${builtInId}_user`);
    systemPrompt = overriddenSystem || DEFAULT_SYSTEM_PROMPTS[builtInId] || GENERAL_SYSTEM_PROMPT;
    userTemplate = overriddenUser || DEFAULT_USER_TEMPLATES[builtInId];
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
    case PromptTemplate.VerbalInterviewAudio:
      return 'Verbal Interview';
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

export function getDefaultSystemPromptForTemplate(template: PromptTemplate): string {
  return DEFAULT_SYSTEM_PROMPTS[template] || GENERAL_SYSTEM_PROMPT;
}

export function getDefaultUserTemplate(template: PromptTemplate): string {
  return DEFAULT_USER_TEMPLATES[template] || '';
}
