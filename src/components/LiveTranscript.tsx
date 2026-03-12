import { useEffect, useRef } from 'react';

interface Props {
  interimText: string;
  finalText: string;
  isTranscribing: boolean;
  silenceCountdown: number | null;
}

export default function LiveTranscript({
  interimText,
  finalText,
  isTranscribing,
  silenceCountdown,
}: Props) {
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (containerRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
    }
  }, [interimText, finalText]);

  const displayText = finalText + (interimText ? (finalText ? ' ' : '') + interimText : '');

  return (
    <div className="live-transcript">
      <div className="live-transcript-header">
        <div className="live-transcript-status">
          {isTranscribing && <span className="pulse-dot" />}
          <span className="live-transcript-label">
            {isTranscribing
              ? silenceCountdown !== null
                ? `Silence detected — sending in ${silenceCountdown}s…`
                : 'Listening…'
              : 'Transcription stopped'}
          </span>
        </div>
      </div>
      <div className="live-transcript-body" ref={containerRef}>
        {displayText ? (
          <p className="live-transcript-text">
            {finalText}
            {interimText && (
              <span className="live-transcript-interim">
                {finalText ? ' ' : ''}{interimText}
              </span>
            )}
          </p>
        ) : (
          <p className="live-transcript-placeholder">
            Waiting for speech…
          </p>
        )}
      </div>
    </div>
  );
}
