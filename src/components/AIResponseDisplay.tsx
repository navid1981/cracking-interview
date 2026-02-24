// AI Response Display with syntax highlighting

import { useState, useEffect } from 'react';
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

  // Handle truncated responses: EXPLANATION_START present but no EXPLANATION_END
  if (!explanation && !solutionMatch) {
    const partialExplMatch = response.match(/EXPLANATION_START\s*([\s\S]*)/);
    if (partialExplMatch) {
      const afterMarker = partialExplMatch[1].trim();
      // Check for a code block inside the partial explanation
      const codeInPartial = afterMarker.match(/```(?:\w+)?\s*([\s\S]*?)```/);
      if (codeInPartial) {
        const codeIdx = afterMarker.indexOf('```');
        explanation = afterMarker.substring(0, codeIdx).trim();
        solution = codeInPartial[1].trim();
      } else {
        explanation = afterMarker;
      }
    }
  }

  // Handle truncated SOLUTION_START without SOLUTION_END
  if (!solution) {
    const partialSolMatch = response.match(/SOLUTION_START\s*([\s\S]*)/);
    if (partialSolMatch) {
      solution = partialSolMatch[1].trim();
      // Also extract explanation if not already found
      if (!explanation) {
        const beforeSol = response.substring(0, response.indexOf('SOLUTION_START'));
        explanation = beforeSol.replace(/EXPLANATION_START|EXPLANATION_END/g, '').trim();
      }
    }
  }

  // Fallback: strip any remaining markers before generic parsing
  if (!explanation && !solution) {
    const cleaned = response
      .replace(/EXPLANATION_START|EXPLANATION_END|SOLUTION_START|SOLUTION_END/g, '')
      .trim();

    const codeBlockMatch = cleaned.match(/```(?:\w+)?\s*([\s\S]*?)```/);
    if (codeBlockMatch) {
      solution = codeBlockMatch[1].trim();
      const codeBlockIndex = cleaned.indexOf('```');
      explanation = cleaned.substring(0, codeBlockIndex).trim();
    }

    if (!solution && cleaned.includes('class Solution')) {
      const parts = cleaned.split('class Solution');
      if (parts.length > 1) {
        explanation = parts[0].trim();
        solution = 'class Solution' + parts[1].trim();
      }
    }

    if (!explanation && !solution) {
      explanation = cleaned;
    }
  }

  return { explanation, solution };
}

// Render basic markdown: **bold**, *italic*, `inline code`, line breaks
function renderMarkdown(text: string): string {
  return text
    .replace(/\*\*(.*?)\*\*/g, '<strong>$1</strong>')
    .replace(/__(.*?)__/g, '<strong>$1</strong>')
    .replace(/\*(.*?)\*/g, '<em>$1</em>')
    .replace(/`([^`]+)`/g, '<code class="inline-code">$1</code>')
    .replace(/\n/g, '<br/>');
}

interface Props {
  response: string;
  language?: string;
}

export default function AIResponseDisplay({ response, language = 'java' }: Props) {
  const [showExplanation, setShowExplanation] = useState(true);
  const [explanationVisible, setExplanationVisible] = useState(true);
  const [showSolution, setShowSolution] = useState(true);
  const [copied, setCopied] = useState(false);

  // Reset visibility whenever a new response arrives
  useEffect(() => {
    setShowExplanation(true);
    setShowSolution(true);
    setExplanationVisible(true);
  }, [response]);

  if (!response) return null;

  const { explanation, solution } = parseResponse(response);

  const handleCopy = async () => {
    await navigator.clipboard.writeText(solution);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="ai-response">
      {explanation && showExplanation && (
        <div className="explanation-section">
          <div className="section-header">
            <button
              className="section-collapse-btn"
              onClick={() => setExplanationVisible(v => !v)}
              title={explanationVisible ? 'Collapse' : 'Expand'}
            >
              <span className={`collapse-arrow ${explanationVisible ? 'expanded' : 'collapsed'}`}>▶</span>
              <h3>📄 Explanation</h3>
            </button>
            <button
              className="section-close-btn"
              onClick={() => setShowExplanation(false)}
              title="Close"
            >
              ✕
            </button>
          </div>
          {explanationVisible && (
            <div
              className="explanation-content"
              dangerouslySetInnerHTML={{ __html: renderMarkdown(explanation) }}
            />
          )}
        </div>
      )}

      {solution && showSolution && (
        <div className="solution-section">
          <div className="section-header">
            <h3>⚡ Solution</h3>
            <div className="section-header-actions">
              <button
                onClick={handleCopy}
                className={`copy-btn ${copied ? 'copied' : ''}`}
              >
                {copied ? '✅ Copied!' : '📋 Copy Code'}
              </button>
              <button
                className="section-close-btn"
                onClick={() => setShowSolution(false)}
                title="Close"
              >
                ✕
              </button>
            </div>
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
