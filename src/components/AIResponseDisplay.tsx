// AI Response Display with syntax highlighting and Mermaid diagram rendering

import { useState, useEffect, useRef, useCallback } from 'react';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism';
import mermaid from 'mermaid';

mermaid.initialize({
  startOnLoad: false,
  theme: 'default',
  securityLevel: 'loose',
  fontFamily: 'system-ui, -apple-system, sans-serif',
});

interface ParsedResponse {
  explanation: string;
  solution: string;
}

// A solution block is either a mermaid diagram, a code block, or plain text
interface SolutionBlock {
  type: 'mermaid' | 'code' | 'text';
  content: string;
  language?: string;
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

/**
 * Parse the solution text into blocks: mermaid diagrams, code blocks, and plain text.
 */
function parseSolutionBlocks(solution: string): SolutionBlock[] {
  const blocks: SolutionBlock[] = [];
  // Match ```mermaid ... ``` and ```lang ... ``` blocks
  const fenceRegex = /```(\w+)?\s*\n?([\s\S]*?)```/g;
  let lastIndex = 0;
  let match;

  while ((match = fenceRegex.exec(solution)) !== null) {
    // Text before this block
    if (match.index > lastIndex) {
      const text = solution.substring(lastIndex, match.index).trim();
      if (text) blocks.push({ type: 'text', content: text });
    }

    const lang = (match[1] || '').toLowerCase();
    const content = match[2].trim();

    if (lang === 'mermaid') {
      blocks.push({ type: 'mermaid', content });
    } else {
      blocks.push({ type: 'code', content, language: lang || undefined });
    }

    lastIndex = match.index + match[0].length;
  }

  // Remaining text after last block
  if (lastIndex < solution.length) {
    const text = solution.substring(lastIndex).trim();
    if (text) {
      // Check if the remaining text looks like code (no mermaid blocks found and it's raw code)
      if (blocks.length === 0 && !text.includes('```')) {
        blocks.push({ type: 'code', content: text });
      } else {
        blocks.push({ type: 'text', content: text });
      }
    }
  }

  // If no blocks were parsed at all (raw code without fences), treat entire solution as code
  if (blocks.length === 0 && solution.trim()) {
    blocks.push({ type: 'code', content: solution.trim() });
  }

  return blocks;
}

// Render basic markdown: **bold**, *italic*, `inline code`, headings, line breaks
function renderMarkdown(text: string): string {
  return text
    .replace(/^### (.*$)/gm, '<h4 style="margin: 12px 0 6px; font-size: 14px;">$1</h4>')
    .replace(/^## (.*$)/gm, '<h3 style="margin: 14px 0 8px; font-size: 15px;">$1</h3>')
    .replace(/^# (.*$)/gm, '<h2 style="margin: 16px 0 8px; font-size: 16px;">$1</h2>')
    .replace(/\*\*(.*?)\*\*/g, '<strong>$1</strong>')
    .replace(/__(.*?)__/g, '<strong>$1</strong>')
    .replace(/\*(.*?)\*/g, '<em>$1</em>')
    .replace(/`([^`]+)`/g, '<code class="inline-code">$1</code>')
    .replace(/\n/g, '<br/>');
}

function MermaidDiagram({ chart }: { chart: string }) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [svg, setSvg] = useState<string>('');
  const [error, setError] = useState<string>('');

  const renderChart = useCallback(async () => {
    try {
      const id = `mermaid-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
      const { svg: renderedSvg } = await mermaid.render(id, chart);
      setSvg(renderedSvg);
      setError('');
    } catch (e: any) {
      console.error('[Mermaid] Render error:', e);
      setError(e.message || 'Failed to render diagram');
      setSvg('');
    }
  }, [chart]);

  useEffect(() => {
    renderChart();
  }, [renderChart]);

  if (error) {
    return (
      <div className="mermaid-error">
        <pre style={{ fontSize: '12px', color: '#999', whiteSpace: 'pre-wrap' }}>{chart}</pre>
      </div>
    );
  }

  return (
    <div
      ref={containerRef}
      className="mermaid-diagram"
      dangerouslySetInnerHTML={{ __html: svg }}
    />
  );
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
  const solutionBlocks = parseSolutionBlocks(solution);
  const hasMermaid = solutionBlocks.some(b => b.type === 'mermaid');
  const codeOnlyBlocks = solutionBlocks.filter(b => b.type === 'code');
  const copyText = codeOnlyBlocks.map(b => b.content).join('\n\n');

  const handleCopy = async () => {
    await navigator.clipboard.writeText(copyText || solution);
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
            <h3>{hasMermaid ? '🏗️ Design' : '⚡ Solution'}</h3>
            <div className="section-header-actions">
              {copyText && (
                <button
                  onClick={handleCopy}
                  className={`copy-btn ${copied ? 'copied' : ''}`}
                >
                  {copied ? '✅ Copied!' : '📋 Copy Code'}
                </button>
              )}
              <button
                className="section-close-btn"
                onClick={() => setShowSolution(false)}
                title="Close"
              >
                ✕
              </button>
            </div>
          </div>

          {hasMermaid ? (
            // Mixed content: text, diagrams, and optional code blocks
            <div className="solution-content-mixed">
              {solutionBlocks.map((block, i) => {
                if (block.type === 'mermaid') {
                  return <MermaidDiagram key={i} chart={block.content} />;
                }
                if (block.type === 'code') {
                  return (
                    <div key={i} className="solution-code-block">
                      <SyntaxHighlighter
                        language={(block.language || language).toLowerCase()}
                        style={vscDarkPlus}
                        showLineNumbers={true}
                        customStyle={{
                          margin: 0,
                          background: '#1e1e1e',
                          fontSize: '13px',
                          padding: '12px',
                          borderRadius: '8px',
                        }}
                        codeTagProps={{
                          style: {
                            whiteSpace: 'pre-wrap',
                            wordBreak: 'keep-all',
                            overflowWrap: 'break-word',
                          }
                        }}
                      >
                        {block.content}
                      </SyntaxHighlighter>
                    </div>
                  );
                }
                // Text block
                return (
                  <div
                    key={i}
                    className="solution-text-block"
                    dangerouslySetInnerHTML={{ __html: renderMarkdown(block.content) }}
                  />
                );
              })}
            </div>
          ) : (
            // Pure code solution (algorithm, code review, etc.)
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
          )}
        </div>
      )}
    </div>
  );
}
