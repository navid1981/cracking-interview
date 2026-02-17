// AI Response Display with syntax highlighting

import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism';

interface ParsedResponse {
  explanation: string;
  solution: string;
}

function parseResponse(response: string): ParsedResponse {
  let explanation = '';
  let solution = '';


  const explanationMatch = response.match(/EXPLANATION_START\s*([\s\S]*?)\s*EXPLANATION_END/);
  const solutionMatch = response.match(/SOLUTION_START\s*([\s\S]*?)\s*SOLUTION_END/);

  if (explanationMatch) {
    explanation = explanationMatch[1].trim();
  }

  if (solutionMatch) {
    solution = solutionMatch[1].trim();
  }

  if (!solution) {
    const codeBlockMatch = response.match(/```(?:\w+)?\s*([\s\S]*?)```/);
    if (codeBlockMatch) {
      solution = codeBlockMatch[1].trim();
      const codeBlockIndex = response.indexOf('```');
      explanation = response.substring(0, codeBlockIndex).trim();
    }
  }

  if (!solution && response.includes('class Solution')) {
    const parts = response.split('class Solution');
    if (parts.length > 1) {
      explanation = parts[0].trim();
      solution = 'class Solution' + parts[1].trim();
    }
  }

  if (!explanation && !solution) {
    explanation = response;
  }


  return { explanation, solution };
}

interface Props {
  response: string;
  language?: string;
}

export default function AIResponseDisplay({ response, language = 'java' }: Props) {
  if (!response) return null;

  const { explanation, solution } = parseResponse(response);

  return (
    <div className="ai-response">
      {explanation && (
        <div className="explanation-section">
          <div className="section-header">
            <h3>📄 Explanation</h3>
          </div>
          <div className="explanation-content">
            {explanation}
          </div>
        </div>
      )}

      {solution && (
        <div className="solution-section">
          <div className="section-header">
            <h3>⚡ Solution</h3>
            <button 
              onClick={() => navigator.clipboard.writeText(solution)}
              className="copy-btn"
            >
              📋 Copy Code
            </button>
          </div>
          <div className="solution-content">
            <SyntaxHighlighter
              language={language.toLowerCase()}
              style={vscDarkPlus}
              showLineNumbers={true}
              customStyle={{
                margin: 0,
                background: 'transparent',
                fontSize: '13px',
                padding: 0,
              }}
              codeTagProps={{
                style: {
                  whiteSpace: 'pre-wrap',
                  wordBreak: 'keep-all',
                  overflowWrap: 'break-word',
                }
              }}
            >
              {solution}
            </SyntaxHighlighter>
          </div>
        </div>
      )}
    </div>
  );
}
