// Prompt Manager - ported from PromptManager.swift

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

export function buildPrompt(
  template: PromptTemplate,
  language: ProgrammingLanguage,
  content?: string
): string {
  let finalPrompt = GENERAL_SYSTEM_PROMPT + '\n\n';

  switch (template) {
    case PromptTemplate.AlgorithmOptimal:
      finalPrompt += `Solve this ${language} algorithm problem.

Requirements:
- Provide optimal time/space complexity solution
- Include complexity analysis (O notation)
- Write production-ready, clean code
- Explain your approach briefly`;
      break;

    case PromptTemplate.AlgorithmBeginner:
      finalPrompt += `Explain and solve this ${language} algorithm problem for a beginner.

Requirements:
- Use simple, clear language
- Explain each step of your approach
- Include examples to illustrate the solution
- Provide clean, well-commented code`;
      break;

    case PromptTemplate.SystemDesign:
      finalPrompt += `Design a scalable system to solve this problem.

Requirements:
- Provide high-level architecture
- Break down into components
- Discuss scalability considerations
- Explain trade-offs and alternatives`;
      break;

    case PromptTemplate.CodeReview:
      finalPrompt += `Review this code and provide comprehensive feedback.

Requirements:
- Identify bugs and potential issues
- Suggest improvements and optimizations
- Provide performance tips
- Recommend best practices`;
      break;

    case PromptTemplate.ExplainConcept:
      finalPrompt += `Explain this technical concept clearly.

Requirements:
- Provide clear, understandable explanation
- Include real-world examples
- Discuss common use cases
- Mention related concepts`;
      break;
  }

  if (content) {
    finalPrompt += '\n\nQuestion:\n' + content;
  }

  return finalPrompt;
}

export function getTemplateLabel(template: PromptTemplate): string {
  switch (template) {
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

export function supportsLanguageSelection(template: PromptTemplate): boolean {
  return template === PromptTemplate.AlgorithmOptimal || 
         template === PromptTemplate.AlgorithmBeginner;
}
