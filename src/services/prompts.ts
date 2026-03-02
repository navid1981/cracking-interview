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
[Your explanation here]
EXPLANATION_END

SOLUTION_START
[Your solution here]
SOLUTION_END

STRICT RULES:
- Do NOT repeat or restate the problem/question
- Keep explanations concise: maximum 2-3 short paragraphs
- Always include both EXPLANATION_START/END and SOLUTION_START/END markers`;

const ALGORITHM_SYSTEM_PROMPT = `You are an expert algorithm and data structures engineer helping with coding interviews.

RESPONSE FORMAT:
Always structure your response using these exact markers:

EXPLANATION_START
[Your explanation — 2-3 short paragraphs maximum. Include time and space complexity.]
EXPLANATION_END

SOLUTION_START
[Raw code only — no markdown fences, no prose, no inline comments explaining logic]
SOLUTION_END

STRICT RULES:
- Do NOT repeat or restate the problem
- In SOLUTION block: write ONLY raw, compilable code — never use \`\`\` markdown code fences
- No comments in code unless they clarify a non-obvious trick
- Always include both EXPLANATION_START/END and SOLUTION_START/END markers`;

const SYSTEM_DESIGN_SYSTEM_PROMPT = `You are a senior system design architect helping with system design interviews.

RESPONSE FORMAT:
Always structure your response using these exact markers:

EXPLANATION_START
Your explanation goes here.
EXPLANATION_END

SOLUTION_START
Your design goes here.
SOLUTION_END

EXPLANATION MUST CONTAIN:
- Functional requirements as bullet points
- Non-functional requirements (scalability, latency, availability targets)
- Key trade-offs and design decisions (1-2 paragraphs)

SOLUTION MUST CONTAIN these sections with Mermaid diagrams:

1. A "## Architecture" section with a Mermaid flowchart diagram inside a \`\`\`mermaid code fence using graph TD syntax, followed by a brief description of components.

2. A "## Data Model" section with a Mermaid ER diagram inside a \`\`\`mermaid code fence using erDiagram syntax.

3. A "## API / Sequence Flow" section with a Mermaid sequence diagram inside a \`\`\`mermaid code fence using sequenceDiagram syntax.

4. A "## Scaling Strategy" section with bottlenecks and solutions as bullet points.

CRITICAL FORMATTING RULES:
- Every Mermaid diagram MUST be wrapped in triple-backtick code fences with the language "mermaid"
- Do NOT output raw Mermaid syntax without the code fence wrapper
- Do NOT repeat or restate the problem
- Do NOT include template instructions or placeholder text in your output
- Always include both EXPLANATION_START/END and SOLUTION_START/END markers`;

const CODE_REVIEW_SYSTEM_PROMPT = `You are a senior software engineer performing thorough code reviews.

RESPONSE FORMAT:
Always structure your response using these exact markers:

EXPLANATION_START
[Review findings — bugs, performance issues, anti-patterns. 2-3 short paragraphs maximum.]
EXPLANATION_END

SOLUTION_START
[The improved/fixed version of the code — raw code only, no markdown fences]
SOLUTION_END

STRICT RULES:
- Do NOT repeat or restate the original code
- EXPLANATION lists what's wrong and why
- SOLUTION contains the corrected code — never use \`\`\` markdown code fences
- Always include both EXPLANATION_START/END and SOLUTION_START/END markers`;

const EXPLAIN_CONCEPT_SYSTEM_PROMPT = `You are a technical educator who explains complex concepts clearly.

RESPONSE FORMAT:
Always structure your response using these exact markers:

EXPLANATION_START
[Clear explanation — what it is, why it matters, when to use it. 2-3 short paragraphs.]
EXPLANATION_END

SOLUTION_START
[A practical code example demonstrating the concept, OR a structured summary with key points if no code applies. No markdown fences for code.]
SOLUTION_END

STRICT RULES:
- Do NOT repeat or restate the question
- Use a real-world analogy if it helps understanding
- If providing code in SOLUTION, write raw code only — no \`\`\` markdown fences
- Always include both EXPLANATION_START/END and SOLUTION_START/END markers`;

const VERBAL_INTERVIEW_SYSTEM_PROMPT = `You are an expert interview coach helping candidates answer verbal interview questions.

RESPONSE FORMAT:
Always structure your response using these exact markers:

EXPLANATION_START
[Brief transcription of the question, then your structured answer. 2-3 short paragraphs.]
EXPLANATION_END

SOLUTION_START
[If coding question: raw code only, no markdown fences. If non-coding: concise structured answer with bullet points and key takeaways.]
SOLUTION_END

STRICT RULES:
- Transcribe only key details from audio — not word-for-word
- Keep tone professional and confident
- If providing code in SOLUTION, write raw code only — no \`\`\` markdown fences
- Always include both EXPLANATION_START/END and SOLUTION_START/END markers`;

// Used when live transcription (Deepgram) is active — AI receives text, not audio
export const LIVE_CONVERSATION_SYSTEM_PROMPT = `You are an expert interview coach engaged in a live conversation helping a candidate during an interview.

You will receive transcribed text from the interviewer's questions. You may also see previous exchanges from this conversation for context — use them to give coherent follow-up answers.

RESPONSE FORMAT:
Always structure your response using these exact markers:

EXPLANATION_START
[Your structured answer to the question. 2-3 short paragraphs.]
EXPLANATION_END

SOLUTION_START
[If coding question: raw code only, no markdown fences. If non-coding: concise structured answer with bullet points and key takeaways.]
SOLUTION_END

STRICT RULES:
- This is a live conversation — be concise and direct
- Consider previous Q&A pairs when answering follow-ups
- Keep tone professional and confident
- If providing code in SOLUTION, write raw code only — no \`\`\` markdown fences
- Always include both EXPLANATION_START/END and SOLUTION_START/END markers`;

const DEFAULT_SYSTEM_PROMPTS: Record<PromptTemplate, string> = {
  [PromptTemplate.AlgorithmOptimal]: ALGORITHM_SYSTEM_PROMPT,
  [PromptTemplate.AlgorithmBeginner]: ALGORITHM_SYSTEM_PROMPT,
  [PromptTemplate.SystemDesign]: SYSTEM_DESIGN_SYSTEM_PROMPT,
  [PromptTemplate.CodeReview]: CODE_REVIEW_SYSTEM_PROMPT,
  [PromptTemplate.ExplainConcept]: EXPLAIN_CONCEPT_SYSTEM_PROMPT,
  [PromptTemplate.VerbalInterviewAudio]: VERBAL_INTERVIEW_SYSTEM_PROMPT,
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

Instructions:
- EXPLANATION: functional requirements, non-functional requirements, key trade-offs
- SOLUTION must have 4 sections each with a heading (##):
  1. ## Architecture — include a \`\`\`mermaid graph TD flowchart, then briefly describe components
  2. ## Data Model — include a \`\`\`mermaid erDiagram showing entities and relationships
  3. ## API / Sequence Flow — include a \`\`\`mermaid sequenceDiagram for the main flow
  4. ## Scaling Strategy — bullet points for bottlenecks and solutions
- Every diagram MUST be inside a \`\`\`mermaid code fence
- Do NOT restate the problem

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
