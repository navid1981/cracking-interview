// Result of `extract_tab_text` (src-tauri/src/chrome/extract.js).
export interface ExtractedCode {
  number: number;
  language: string;
  source: string;
  value: string;
}

export interface ExtractedImage {
  index: number;
  src: string;
  alt: string;
  naturalWidth: number;
  naturalHeight: number;
  frameUrl: string;
  width: number;
  height: number;
  top: number;
  left: number;
}

export interface ExtractedDrawing {
  kind: string;
  width: number;
  height: number;
  top: number;
  left: number;
}

export interface TabExtraction {
  version: number;
  /** Final text for the prompt: page text with placeholders, then code-editor blocks. */
  text: string;
  pageTextLength: number;
  code: ExtractedCode[];
  images: ExtractedImage[];
  drawn: ExtractedDrawing[];
  truncated: boolean;
  root: string;
  framesRead: number;
  framesSkipped: number;
  url: string;
  title: string;
  error?: string;
}

/** Max pictures sent with one question (free and Pro alike). Mirrors MAX_AI_IMAGES in main.rs. */
export const MAX_QUESTION_IMAGES = 4;
/** Below this much page text (and with no images or code), the page is treated as visual-only. */
const MIN_QUESTION_TEXT = 80;
/** A canvas this large is treated as a whiteboard (only its visible area can be captured). */
const WHITEBOARD_MIN_AREA = 300_000;

/** Charts/whiteboards have no image file, and near-empty pages (e.g. PDFs) have no text. */
export function needsTabScreenshot(e: TabExtraction): boolean {
  if (e.drawn.length > 0) return true;
  return e.pageTextLength < MIN_QUESTION_TEXT && e.images.length === 0 && e.code.length === 0;
}

export function isWhiteboard(e: TabExtraction): boolean {
  return e.drawn.some((d) => d.kind === 'canvas' && d.width * d.height >= WHITEBOARD_MIN_AREA);
}
