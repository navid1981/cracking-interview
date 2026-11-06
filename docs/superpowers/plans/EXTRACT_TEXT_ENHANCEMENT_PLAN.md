# Extract-Text Feature Enhancement — Investigation & Plan

**Status:** Planning complete, implementation not started.
**Audience:** Human maintainer + any AI agent picking up implementation of this work.
**Purpose:** Document what's broken in the current Chrome-tab text extraction feature, why (with evidence), what we decided to do about it, and — just as importantly — what we considered and explicitly rejected, so nobody re-litigates those paths without re-reading this first.

---

## 1. Background

The app's "text mode" lets a user point at a Chrome tab (a coding question, an assessment, etc.) and have the app extract the page's text, build a prompt, and send it to an AI model for a solution. The AI's output quality is entirely bounded by what gets extracted. This doc investigates how well that extraction actually works across real target platforms, and proposes a prioritized fix plan.

This was triggered by the user asking what the extractor outputs for a specific assessment URL and feature feedback; investigation expanded to a general audit of the extraction feature once scope was confirmed.

## 2. Current implementation (as of this writing)

Call chain for Chrome-tab text mode:

1. **`src/App.tsx:1908-1914`** — on solve, calls `activate_tab`, sleeps a **flat 300ms**, then calls `extract_tab_text`.
2. **`src-tauri/src/main.rs:593`** — `extract_tab_text(tab_id)` → `chrome::execute_javascript(&tab_id, "document.body.innerText")`.
3. **`src-tauri/src/chrome/mod.rs:124`** — `execute_javascript()` runs `Runtime.evaluate` over CDP against **the top frame only** (WS mode: `chrome/mod.rs` WS handle path; HTTP fallback: same file, `execute_javascript_http`). No iframe descent, no readiness wait beyond the caller's flat sleep.
4. **`src/services/prompts.ts:359`** — `buildPrompt(template, language, content)` interpolates the raw extracted string directly into `{CONTENT}` / `{PROBLEM}` placeholders (lines 387-392). **Zero post-processing** — no whitespace cleanup, no dedup, no chrome stripping.
5. Result is sent to the AI either via the BYO-key path (`query_ai` → `ai/gemini.rs`) or the proxy path (`query_ai_via_proxy` → `../../../supabase/functions/ai-proxy/index.ts` → OpenRouter).

Image/vision plumbing (relevant to the proposed fix, see §5.2) already exists but is **hardcoded for exactly one image**:
- `src-tauri/src/ai/gemini.rs:216` `query_with_image()` — single `inline_data` block.
- `src-tauri/src/main.rs:1206-1248` `query_ai_via_proxy_with_image()` — single `image_url` content part, built from a file path read off disk.
- `supabase/functions/ai-proxy/index.ts:44` — the message `content` type is already `Array<{type, text?, image_url?}>`, i.e. **the server already accepts multiple content parts and needs no changes**; it's a pass-through to OpenRouter.

## 3. Test methodology

Used Playwright to drive a real browser to each target URL, then ran `document.body.innerText` (and supporting diagnostics — iframe/image/canvas enumeration, editor-class detection) to reproduce exactly what the app's extractor would see. This is a faithful reproduction of the app's own mechanism (CDP `Runtime.evaluate` on the main frame), not a guess.

## 4. Test results

### 4.1 Criteria Corp (ondemandassessment.com) — Question 1/10, multiple-choice text

**Output:**
```
Question 1 / 10




Choose the word that is most nearly opposite to the word FAST

	
Large

	
Cold

	
Slow

	
Afraid

	
Hard

Submit Answer
```

**Verdict:** Mostly usable. Issues: raw tab/blank-line whitespace noise; no answer-option labels (1/2/3/4 or A/B/C/D) so the AI's answer can't be reliably mapped back to a clickable option; captures the `Submit Answer` button label as if it were content.

### 4.2 Criteria Corp — Question 7/10, image-based spatial reasoning

**Output:**
```
Question 7 / 10




 Enlarge Image

Which box should replace the question mark (?) to complete the pattern?

	
A


	
B


	
C


	
D


	
E

Submit Answer
```

**Verdict: Critical, silent failure.** The entire puzzle — the pattern grid and all five answer-choice boxes — is a single composite image (`01-grey.png`, 1424×792px, hosted at `criteria-corp-object-store.s3.us-east-1.amazonaws.com`, confirmed publicly fetchable without session auth via plain `curl` → HTTP 200) with **empty `alt=""`**. The extracted text is grammatically complete and plausible-looking ("Which box completes the pattern? A B C D E") but contains **zero actual visual information**. This is more dangerous than an obvious failure because nothing signals the extraction was incomplete — the AI will likely answer confidently and wrongly, and the user has no reason to suspect why.

### 4.3 LeetCode — "Two Sum" problem page (Monaco editor)

**Verdict: Critical.** 5,613 characters captured, the large majority irrelevant: nav bar, login/premium upsell, **duplicated tab labels** (`Editorial` x2, `Solutions` x2, `Submissions` x2, `Test Result` x2 — responsive mobile+desktop DOM both present), a list of **20 unrelated "Similar Questions"**, and a full discussion thread (usernames, dates, vote counts, community guidelines text). The actual starter code (`class Solution { ... }`) that the user needs to see **was completely absent** — confirmed via `document.body.innerText.includes('class Solution')` → `false`, despite the Monaco editor being present and rendering 7 visible `.view-line` elements. Monaco virtualizes its DOM; `innerText` does not traverse it usefully.

### 4.4 HackerRank — "Solve Me First" problem page (Monaco editor)

> **Correction (review pass, 2026-10-10):** the first draft said "CodeMirror" and guessed the text gaps came from CSS/JS templating. Both were wrong. Verified: HackerRank uses **Monaco** (`.monaco-editor` present, `window.monaco` exposed, one `cpp` model, 309 chars), and the gaps are **server-pre-rendered MathJax SVG** (see below).

**Verdict: Moderate — partially works, with specific defects.** Unlike LeetCode, the starter code (`#include <cmath>` ... full C++ stub) **was** captured, but only because the file is short enough (20 lines) that Monaco rendered every line. Longer stubs would be cut off the same way as on LeetCode. Other defects:
- Monaco's line-number gutter shows up as a separate block of numbers (`1\n2\n3...20`) *before* the code instead of next to each line.
- A cookie-consent banner adds noise at the very top of the extraction.
- **The problem statement has holes** (`"Complete the function  to compute..."`, `"Return ."`). Each hole is a `<span class="MathJax_SVG">` that holds an inline `<svg>` of raw `<path>` glyph outlines (12 of them on this page). There is **no recoverable text**: no `script[type="math/tex"]` source, no `aria-label`, no `<title>`, and no `window.MathJax` runtime (it was rendered server-side). That makes it visual-only content, the same class of problem as §4.2, just small and inline. In this case the impact is limited because the missing identifiers (e.g. `solveMeFirst`) also appear in the editor code.
- 4 third-party iframes (a chat/demo-request widget, a reCAPTCHA anchor, 2 empty). None held question content, but they show how common third-party iframes are on these platforms.

### 4.5 Codility — "BinaryGap" lesson page

**Verdict: Mostly fine for this specific page state, but surfaced a separate serious risk.** The editor itself hadn't loaded yet (gated behind a "START" button click), so what's on the page is static text — and it's good: the problem statement AND the function signature (`class Solution { public int solution(int N); }`) came through cleanly. Downsides: ~35 lines of sidebar lesson-list noise (17 unrelated lessons) and footer/newsletter/copyright noise.

**Separately important finding:** this page embeds a Twitter widget iframe (`platform.twitter.com/widgets/...`) containing **327,026 characters** of text. This was discovered while testing whether iframe content could be a source of missed question material. It's concrete proof that a naive "traverse all iframes and merge their text" fix would be actively dangerous — it would have blown up this single extraction by two orders of magnitude with zero relevant content.

### 4.6 General-exam element test (synthetic page, 2026-10-10)

The 5 live sites cover mostly coding platforms, but this app is meant for **any** online exam (English and language tests, LMS quizzes in Moodle/Canvas/Google Forms, certification exams). To check the elements those exams use without touching real exams, I built a local test page with one of each and ran `document.body.innerText` on it:

| Element (typical exam use) | What `innerText` produced | Impact |
|---|---|---|
| `<input type=text>` inside a sentence (fill-in-the-blank / cloze) | `She  in London since 2015` | **The blank and the typed answer both vanish.** The AI can't tell a blank was ever there. |
| `<select>` inside a sentence (choose the right word) | `If I \nChoose...\nwas\nwere\nam\n you` | **Every option is dumped in the middle of the sentence**, and the selected one isn't marked. The sentence is garbled. |
| `<textarea>` (essay / writing task) | *(nothing)* | **The user's draft is dropped entirely.** |
| `contenteditable` div (rich-text answer) | kept | OK |
| `<table>` (matching, grids) | tab-separated rows | Mostly OK, but any dropdown inside a cell is garbled as above |
| MathML `<math>` | `𝑥\n+\n3\n=\n7`, one symbol per line | Readable but broken up |
| `<details>` collapsed panel (reading passage behind "Show passage") | only the summary line | **The passage is missing**: `innerText` skips content that isn't rendered. Passages hidden in tabs or accordions are affected the same way. |
| **Same-origin `<iframe>`** (question embedded in a frame) | *(nothing)* | **Question 8 lost entirely** |
| **Shadow DOM** (web-component widgets) | *(nothing)* | **Question 10 lost entirely** |
| `<audio>` (listening section) | only the text near it | Expected; this is the app's audio mode, not text mode |

**Conclusion:** `innerText` is the wrong foundation for general exams. Cleaning up its output afterwards can't bring back blanks, typed answers, which option is selected, or content inside frames, shadow DOM and collapsed panels. The extractor needs to **walk the DOM itself** (see the Phase 2 revision).

### 4.7 Figma and Miro — canvas-based whiteboards (2026-10-10)

Interviews sometimes use whiteboard tools for system-design or design tasks, so I tested both, using only their public galleries (no login).

**Miro**: tested a real public board (the "Sailboat Retrospective" template, opened through its live-embed link from the public template page).
- The board is drawn on **7 `<canvas>` elements** (main one 1200×863). There are no text nodes, no SVG `<text>`, and no shadow DOM holding the board content.
- `innerText` (612 chars) contains **only accessibility UI text** ("Skip to first object on the board", "Explore board content", ...) plus the board title. **None of the board's actual content** (sticky notes, labels) comes through.
- Miro has a built-in "Explore board content" button that's supposed to build a read-only text outline of the board. I triggered it both by click and by keyboard focus + Enter. In this view-only embed it only added **"No objects on the board."** So the outline is empty here and can't be relied on as a text source. It *might* fill in for a logged-in user in the full editor; that's **unverified**.
- `textContent` is 89,993 chars, but nearly all of it is inline `<script>` source. Using `textContent` instead of `innerText` would be strictly worse.

**Figma**: tested logged in (2026-10-10), in two new test files the user agreed to create in their Drafts: one Design file and one FigJam board, each with sample text (a question, options, a labeled box, sticky notes).
- Community file pages (`figma.com/community/file/...`) are ordinary HTML landing pages, and published **Figma Sites** (`*.figma.site`) are **plain HTML that extracts cleanly**.
- The editor (Design and FigJam) **draws everything on a canvas**, but unlike Miro, **the text does reach `innerText`**. Figma keeps a hidden 1×15 px `div[role=document].focus-target` (its screen-reader/clipboard mirror) that holds the text of **every text layer on the page**:
  - Design file: `"API GatewayA) PostgreSQL   B) Redis   C) Cassandra   D) MongoDBQuestion 3: Design a URL shortener. Which database fits best?⁨⁩"`
  - FigJam: `"Interview task: scale to 10M usersShard by user_idCache layer (Redis)Load balancer⁨⁩"`
- Properties verified: content is the same whatever is selected (nothing / one layer / select-all); it **still includes layers scrolled out of view**; it's **one flat text node with no child elements**, so there are **no separators between layers**; order is **reverse creation order**, not layout order; it ends with invisible Unicode bidi-isolate characters (`U+2068 U+2069`).
- Other `innerText` content is Figma UI noise: the Agents/AI side panel, onboarding tips, FigJam's "Screenreader support for the board is currently disabled…" banner, the timer, the Share button.
- So the **words are recoverable but the structure isn't**: separate sticky notes run together ("Shard by user_idCache layer"), and which box connects to which, or what sits where, is lost completely.
- **Unverified:** (1) whether a view-only shared link (e.g. a candidate viewing an interviewer's file) also gets this mirror; (2) whether turning on Figma's own screen-reader support (FigJam suggests it in its banner) gives a structured per-layer tree with separators. That's a setting the user would have to change; the app shouldn't change it.

**Conclusion:**
- **Miro:** text extraction gets nothing, so it needs the visual path: the Phase 0 detector sees a large canvas filling the content area and sends **a screenshot of that canvas** along with whatever text exists (board title, etc.).
- **Figma/FigJam:** use **both**. Read the `focus-target` mirror for the exact words (it beats OCR on precision and also covers off-screen layers), and send the canvas screenshot for layout. In the text, mark the mirror as "unordered text layers from a design canvas", strip the bidi characters, and drop the Figma UI panel text.
- **Limitation for both:** only the **visible part of the board** gets captured as an image, so the user has to pan/zoom to the relevant area before solving. The app should say this when it detects a whiteboard (e.g. "Whiteboard detected — capturing the visible area").

### 4.8 Summary table

| # | Site | Content type | Primary defect | Severity |
|---|---|---|---|---|
| 1 | Criteria Corp Q1 | MCQ text | Whitespace noise, no option labels | Low |
| 2 | Criteria Corp Q7 | Image-only spatial reasoning | **Silent total content loss** | **Critical** |
| 3 | LeetCode | Coding (Monaco) | **Starter code 100% missing** + heavy noise | **Critical** |
| 4 | HackerRank | Coding (Monaco) | Gutter numbers, banner noise, MathJax-SVG holes in statement | Moderate |
| 5 | Codility | Coding (static problem page) | Sidebar/footer noise; **iframe content-bomb risk discovered** | Low (page) / informs architecture |
| 6 | Miro (public board) | Canvas whiteboard | **Board content 100% invisible**; only accessibility UI text extracted | **Critical** (needs visual path) |
| 7 | Figma / FigJam (editor, logged in) | Canvas design/whiteboard | Words recoverable from a hidden mirror element, but no separators, reverse order, no layout | Moderate (text + screenshot) |

## 5. Root causes (traced to code, not inferred)

1. **No content isolation** — grabs raw `document.body`; no attempt to locate a main-content region before falling back to everything (nav, ads, discussion, upsell all get mixed in).
2. **Image/canvas/SVG content is invisible to `innerText`, and fails *silently*.** This is the single highest-severity issue because the failure mode is indistinguishable from success — a complete-looking prompt with no actual content.
3. **No code-editor awareness.** Monaco only renders the lines currently on screen, so `innerText` drops code (LeetCode: the stub is missing entirely) or gets it only when the file happens to fit (HackerRank), and its line-number gutter comes out as a separate block of numbers. Both tested coding platforms use Monaco.
4. **Single top-frame only**, and — per the Codility finding — naively fixing this by traversing all iframes would be actively harmful without strict scoping.
5. **Zero post-processing** in `buildPrompt()` — no whitespace collapse, no dedup of duplicate responsive DOM nodes, no chrome stripping.
6. **Fixed 300ms wait** after `activate_tab`, no readiness or DOM-mutation-quiet check — risk of extracting from a partially-rendered SPA.
7. **Form structure is lost.** Radio/checkbox answer options come out as bare words with no labels and no indication of which option is selected (§4.1). The first draft listed this as a defect, but no phase addressed it.
8. **No output size cap.** Whatever `innerText` returns goes straight into the prompt. A page with a long discussion thread, or a large embedded widget, can blow up the token count.

## 6. Proposed plan (prioritized)

> **Review pass (2026-10-10) changes:** added Phase 0 (a quick fix for the critical silent failure using the existing single-image path), filled in the Phase 1 mechanism with verified findings, added MCQ structure and an output size cap to Phase 2, changed how Phase 4 captures `<img>` content, and added Phase 5 (move the extraction script into its own file and test it against saved fixtures).

### Phase 0 — Visual questions: send text + the original image, or text + a tab screenshot (quick win)

> **Final design (agreed 2026-10-10):** the text is always extracted. If the question also has visual content, add **one** picture: the **original image file** when there is one, otherwise a **tab screenshot**. The earlier "element-bounds screenshot" idea is dropped as unnecessary complexity.

The most severe finding (§4.2) shouldn't have to wait for the full multi-image work in Phase 4. Most of the plumbing already exists:
- `query_ai_via_proxy_with_image` (`main.rs:1208`) and `query_with_image` (`ai/gemini.rs:216`) already accept **a prompt plus one image**. The prompt carries the extracted text, so "text + image" already works for the single-image case. **No N-image changes are needed for this phase.**

**Step 1: always extract the text** (Phases 1–2).

**Step 2: detect visual content inside the content area.** Ignore small things (icons, logos, avatars, tracking pixels). Suggested threshold: rendered area ≥ ~40,000 px² (about 200×200).

**Step 3: choose what to send:**

| Page contains | Sent to the AI |
|---|---|
| Text only | **Text only**, exactly as today (no extra cost) |
| A large `<img>` (an actual image file), e.g. Criteria Corp Q7 | **Text + the original image bytes**, read from Chrome's own copy via CDP `Page.getResourceContent(frameId, url)`. That gives full resolution (Criteria Corp's image is 1424×792 natively but shown scaled down) and works on login-gated images because it reuses what the user's session already loaded, with no new download. *Verified 2026-10-10 (see note below the table).* If the resource isn't available, fall back to the tab screenshot. |
| Drawn content with no file: a large `<canvas>` or `<svg>` (charts, diagrams), or a canvas whiteboard (Miro, Figma/FigJam) | **Text + a tab screenshot** using the existing `capture_tab_screenshot` path. If the element is below the visible area, scroll it into view (`scrollIntoView`) just before capturing. |
| **Almost no question text found and no pictures detected** (decision 2026-10-10) | **Automatically switch to a tab screenshot** instead of sending a nearly empty prompt. The small amount of text found (e.g. a page title) can go along with it. "Almost nothing" = the extracted text, after removing navigation/UI junk, is below a small threshold (suggested ~80 characters; tune on the saved test pages). Covers cases no other rule catches, e.g. exams shown as PDF in Chrome's viewer. |

> **`Page.getResourceContent` verification (2026-10-10):** tested on a public Wikipedia article whose images come from a different domain (`thumb.wikimedia.org`), the same cross-domain setup as Criteria Corp's S3-hosted image. After `Page.enable` → `Page.getResourceTree` (image URLs were listed among the frame's resources) → `Page.getResourceContent(frameId, url)`, all 3 images returned real PNG files (`base64Encoded: true`, data starting with the PNG signature `iVBORw0KGgo`, ~28–45 KB each) in **1–2 ms**, with no new network request. The bytes are the **original file**, not the on-screen size: one image shown at 352×349 came back as its 500×495 source, and another shown at 151×174 came back as 250×289. **Not verified:** a login-gated image (it should work, since the bytes come from what the logged-in tab already loaded, but no such page was available to test), and images loaded inside iframes, which need that iframe's own `frameId`.

Why the text is still sent with the picture: a screenshot only covers what's visible, and the AI misreads small numbers/characters in images more often than in text. The text supplies the full question, every option, off-screen parts and exact numbers; the picture supplies what has no text form. Charts often keep their axis labels and values as real SVG `<text>`, and the DOM walker picks those up too.

Platform-specific notes:
- **Whiteboards:** a screenshot only captures the visible area, so show the user a short notice ("Whiteboard detected — capturing the visible area") so they can pan/zoom first.
- **Hidden screen-reader text on canvas apps (general rule, merged in 2026-10-10):** when the page is drawn on a canvas, also include any text the page keeps for screen readers (tiny or off-screen elements that aren't `display:none`). The Phase 2 walker already picks this up on any site, because these elements are part of the normal DOM. No platform-specific extraction code is needed. Label this text as "unordered text from a drawn canvas" so the AI relies on the screenshot for positions.
  - **Confirmed:** Figma/FigJam (`div[role=document].focus-target` holds every text layer, §4.7). The only Figma-specific part is cleanup: strip the bidi characters (`U+2068 U+2069`) and drop the Figma UI panel text.
  - **Checked, empty:** Miro's "Explore board content" outline showed "No objects on the board" in view-only mode (§4.7).
  - **Not tested:** Google Docs and other canvas-based apps.
- **Known limit:** small inline math SVG like HackerRank's (§4.4) falls under the size threshold and is skipped. That's acceptable because the missing identifiers usually also appear in the editor code (Phase 1).

### Phase 1 — Editor-aware extraction (highest priority)
Read code through the editor's own JS API instead of the rendered DOM. **Verified on both tested coding platforms (2026-10-10):**

| Platform | Editor | `window.monaco` | Models found | Result |
|---|---|---|---|---|
| LeetCode | Monaco | exposed | 2: `cpp` (98 chars, full `class Solution {...}` stub) + an empty `plaintext` model | ✅ full stub recovered (missing entirely from `innerText` today) |
| HackerRank | Monaco | exposed | 1: `cpp` (309 chars) | ✅ full stub recovered, no gutter numbers |

Implementation details from the verification:
- **Don't use `getModels()[0]` blindly.** LeetCode has an extra empty `plaintext` model. Prefer the models attached to visible editors (`monaco.editor.getEditors().map(e => e.getModel())`), then drop empty models and `plaintext` models when a code-language model exists. If there are several, label each one with its language.
- Fallback when `window.monaco` isn't exposed: read the `.monaco-editor textarea` value (it matched 98/98 chars on LeetCode). Treat it as last resort only, because Monaco's textarea can hold just a window of text around the cursor in large files.
- CodeMirror 5 (`.CodeMirror` → `el.CodeMirror.getValue()`), CodeMirror 6 (`.cm-content` → `el.cmView.view.state.doc.toString()`) and Ace (`ace.edit(el).getValue()`) are **not seen on any tested site yet**. Include them as detection branches, but mark them unverified until a test site uses them.
- Output the code as a separate, clearly labeled block (e.g. `--- Code editor (cpp) ---`) and exclude the editor's DOM subtree from the main `innerText` pass, so the gutter numbers and partial lines don't show up twice.

### Phase 2 — Custom DOM walker replaces `innerText` (ship alongside Phase 1, same code path)

> **Revised for general exams (§4.6):** this phase used to be "cleanup applied to `innerText` output." It is now "build the text by walking the DOM directly," because `innerText` throws away information no cleanup step can restore.

The walker visits the content root's nodes in document order and writes out text, handling each element type explicitly.

> **Decision (2026-10-10): extract the question, not the user's answers.** The user's typed or selected answers are **never** included. They can mislead the AI (it may go along with a wrong answer), and exams sometimes pre-select a default option that isn't the user's choice. The question's *shape* is still kept: where each blank is, and every available choice. **The one exception is code editors** (Phase 1): their content is always included, because the starter code and any code the user has written are part of what the AI has to work on.

- **Text nodes:** written out with whitespace collapsed. Elements hidden with `display:none` or `visibility:hidden` are skipped, **but** content inside a collapsed `<details>` or an inactive tab panel is still included and marked (e.g. `[collapsed section]`). Exams hide reading passages this way, and the user needs them answered on.
- **`<input type=text>` / number / etc.:** a plain `[blank]` written **at its position in the sentence**. Any value the user typed is ignored.
- **`<select>`:** `[choose: was | were | am]`, listing every option (placeholder entries like "Choose..." dropped), with nothing marked as selected.
- **Radio/checkbox groups:** options marked `( ) Joyful` (single choice) or `[ ] Joyful` (multi-select), with nothing marked as selected. No A/B/C letters are invented (changed 2026-10-22): the page does not show them, so an answer like "C" could not be matched to what the user sees. This fixes §4.1.
- **`<textarea>` and `contenteditable` answer boxes:** left out entirely, including any draft the user typed. The question text around them already says what to write.
- **MathML:** written as linear text (`x + 3 = 7`) from its text content, joined on one line.
- **Open shadow roots:** walked into whenever an element has a `shadowRoot`.
- **Same-origin iframes:** walked into, subject to the guardrails in the iframe section below.
- **Editors (Phase 1)** and **large images/canvas (Phase 0/4):** replaced with a placeholder (`[code editor: see below]`, `[image 1]`) so the AI knows where they sit in the page.
- **Then post-processing:** dedupe adjacent identical lines (LeetCode responsive duplicates), then apply the output cap (suggested ~12k chars, cut from the end so the question survives, log when it happens).
- **Content root:** try `<main>`, `[role=main]`, `article`, `form` before falling back to `document.body`.

### Phase 3 — Readiness check
Replace the flat 300ms sleep (`App.tsx:1912`) with a short DOM-mutation-quiet poll: extract as soon as the page has had no DOM changes for a brief moment, with a **hard cap of 300ms** (decision 2026-10-10: the solve flow must never be slower than it is today). On an already-loaded page this reads almost immediately, so it's faster than the current fixed wait; on a slow page it behaves like today. Bundle with Phase 1/2 since it's in the same call path.

### Phase 4 — Up to 4 pictures per question (in scope now)

> **Decision (2026-10-10):** moved from "later" into the first round, with a **maximum of 4 pictures** per question.

Generalizes Phase 0 from one picture to several, for questions that use multiple images (e.g. one image per answer option, or two charts to compare).

- **Same capture rules as Phase 0:** original image bytes via `Page.getResourceContent` for each large `<img>`, the tab screenshot for drawn content. A page needs at most one tab screenshot, since it already covers everything visible.
- **Choosing which pictures (when there are more than 4):** keep only candidates inside the main-content region from Phase 2, skip small items (icons, logos, avatars, tracking pixels), then take the 4 largest, sent in page order. Each picture gets a placeholder in the text (`[image 1]` … `[image 4]`) at its position, so the AI knows which picture belongs to which part of the question.
- **Send alongside extracted text, not instead of it:** one multi-part message (text + 1–4 images).
- **Requires extending both AI paths from single-image to N-image:**
  - `src-tauri/src/ai/gemini.rs:216` — Gemini's `contents[].parts[]` already supports multiple `inline_data` blocks natively; needs the function signature to accept `&[Vec<u8>]` instead of `&[u8]`.
  - `src-tauri/src/main.rs:1206-1248` — OpenRouter's `content[]` array already supports multiple `image_url` parts natively; needs `image_path: String` → `image_paths: Vec<String>` (or base64 payloads directly, avoiding the temp-file-per-screenshot pattern).
  - `../../../supabase/functions/ai-proxy/index.ts` — **no change needed**, already typed for an array of content parts (confirmed, §2). Consider an image-count/size check there as well, so the cap can't be bypassed by a modified client.
- **Cost:** each picture adds to the AI request cost, so a 4-picture question costs noticeably more than a text-only one. **Decided 2026-10-10: the same cap of 4 applies to free and Pro users.**

### Phase 5 — Move the extraction script into its own file and test it against saved pages (do this first, alongside Phase 1)
Today the extraction logic is an inline string in `main.rs:593`. With Phases 0–2 it grows to a few hundred lines of JS with editor detection, content isolation and form labeling. That shouldn't live in a Rust string literal.
- Put it in a standalone file (e.g. `../../../src-tauri/src/chrome/extract.js`) and load it with `include_str!`. Have it return **structured JSON** (`{ text, code: [{lang, value}], images: [...], truncated, source }`) instead of one flat string, so the frontend can build the prompt and choose between the text and image paths.
- **Test against saved pages, not live ones.** Save each regression page once (CDP `Page.captureSnapshot` → MHTML, or a saved HTML copy plus a small editor stub), then run `extract.js` against those copies in Playwright and diff the output against approved results. This gives repeatable, offline regression testing.
- **Why the saved copies matter:** testing on live assessment links has side effects. The Criteria Corp link used in this investigation is a real, stateful practice session: across the test runs it moved from Q1 to Q7. Repeated live testing burns through real assessments and can't be repeated exactly.

### Optional — Extract only the user's selected text
If the user has highlighted text on the page (`window.getSelection().toString()` is non-empty), extract only that. It costs almost nothing to add and gives the user exact control whenever the automatic heuristics pick the wrong region.

> **Removed (2026-10-10):** the user decided not to build this. Kept here only so it isn't re-proposed without that context.

### Iframe traversal — now in scope, with guardrails (revised)

> **Reversed (2026-10-10):** the first draft left iframe traversal out of scope because the coding sites only had widget iframes. General exams change that: LMS platforms commonly embed quiz content in frames (e.g. LTI tools inside Moodle/Canvas, embedded Google Forms), and the §4.6 test confirms such content is **completely invisible** to the current extractor.

Required guardrails, because the Codility Twitter iframe (327,026 chars, §4.5) proves naive traversal is dangerous:
- **Denylist** known third-party widget hosts: recaptcha, chilipiper, platform.twitter.com, intercom, hotjar (`_hjSafeContext` iframes appeared on Criteria Corp), and ad/analytics domains.
- **Per-frame size cap** (e.g. skip any frame over ~20k chars) on top of the global output cap.
- **Visibility filter**: skip frames that are hidden or under ~100×100 px rendered.
- **Same-origin frames** can be walked directly from the parent page's script. **Cross-origin frames** (out-of-process in Chrome) can't be reached from the page. They need CDP itself: list the frames (`Page.getFrameTree` / `Target.setAutoAttach` with `flatten`) and run the same `extract.js` in each attached frame. Build the same-origin part first; add cross-origin only once a real test platform needs it.

Original reasoning, kept for context:
Evidence doesn't support building this. In both sites that had iframes (HackerRank §4.4, Codility §4.5), the real question content was in the main frame — the iframes held a chat widget, reCAPTCHA, and (worst case) a 327,026-character Twitter embed. We have a demonstrated danger and zero demonstrated need. Revisit only if a real test case surfaces where content is actually trapped in a same-origin iframe; if so, scope any fix to same-origin-only + a hard size cap + a denylist of known third-party widget domains (recaptcha, chilipiper, platform.twitter.com, etc.).

## 7. Approaches considered and rejected (read this before re-proposing these)

| Rejected approach | Why |
|---|---|
| **Switch to screenshot mode *instead of* text when text extraction looks thin** | Partly adopted, partly rejected (revised 2026-10-10). The tab screenshot **is** used, but only for drawn content with no image file (charts, canvas whiteboards), and it is sent **together with** the extracted text, never instead of it (Phase 0). Rejected parts: dropping the text (a screenshot misses off-screen content and the AI misreads small print) and using a screenshot for real image files (the original file is sharper). |
| **Element-bounds screenshot** (screenshot cropped to one element) | Proposed in an earlier draft for canvas/SVG. Dropped 2026-10-10: the tab screenshot plus the text covers the same cases with code that already exists. |
| **Fetch `<img src>` URLs directly from the Rust backend** | Confirmed technically possible for the one site tested (Criteria Corp's S3-hosted image returned HTTP 200 unauthenticated), but not generalizable: fails on cookie/session-gated images (backend fetch carries no browser session), and has no answer at all for canvas, SVG, or CSS `background-image` content (no URL exists to fetch). CDP element-screenshot (Phase 4) is a superset of this capability. |
| **Pass the image URL as plain text in the prompt and instruct the model to "fetch" it** | Does not work with this app's architecture. The AI is called via single-shot REST completion with no tool-calling/browsing loop — the model cannot retrieve external resources mid-request. It will either refuse, ignore the instruction, or (worst case) hallucinate a plausible-sounding description of imagined image content with full confidence. This would be a *worse* failure mode than the current one, because it looks like a deliberate, reasoned answer rather than an obvious gap. |
| **Pass the image URL via the structured `image_url` content-part field (provider fetches server-side)** | A real, legitimate mechanism (distinct from the above — the provider's infra does a real fetch, not the LLM "deciding" to browse) and technically simpler to implement than byte capture. Not adopted as the primary path because: (1) unverified whether all 4 routed models behind OpenRouter honor an external URL vs. requiring inline base64 — would need per-model testing before relying on it; (2) inherits the same cookie/auth blind spot as direct backend fetch; (3) still has no answer for canvas/SVG/background-image cases. Could be revisited as a cheap optimization for the narrow case of known-public `<img src>` URLs, with CDP screenshot as the fallback for everything else — but adds a second code path for marginal benefit, not worth the complexity right now. |
| **Naive "traverse all iframes and merge text"** | Directly disproven as dangerous by the Codility test (§4.5) — a single unrelated third-party widget iframe contained 327,026 characters, which would have overwhelmed the actual question content and blown up token cost. No test case so far shows real question content trapped in an iframe that the main frame misses. |

## 8. Regression test battery

Run these against **saved copies** (Phase 5), not live URLs, before and after each phase ships:

1. Criteria Corp MCQ text question (§4.1)
2. **Criteria Corp image-based spatial reasoning question (§4.2) — the critical case, must not regress**
3. LeetCode "Two Sum" (§4.3) — verify starter code now appears, verify noise reduction
4. HackerRank "Solve Me First" (§4.4) — verify gutter/code interleaving improves
5. Codility "BinaryGap" (§4.5) — verify sidebar noise reduction; if iframe handling is ever added, verify the Twitter-widget case is excluded

**Optional additions for broader coverage (not blocking, nice to have):**
- **The §4.6 synthetic general-exam page.** Keep it as a permanent saved test copy; every row of that table should pass.
- **Real general-exam platforms (recommended before Phase 2 is finished):** Google Forms quiz (form inputs/radio groups), the Moodle public demo (`school.moodledemo.net`, which resets regularly so it's safe to modify) for LMS quiz layouts including cloze and drag-and-drop, and Microsoft Forms.
- **Known cases not covered by any phase yet:** exams shown as **PDF in Chrome's built-in viewer** (text isn't in the page; now caught by the Phase 0 "almost no text → tab screenshot" fallback), **canvas-drawn documents** such as Google Docs (body text is mostly canvas), and **drag-and-drop** items whose state lives only in JS. For these, the visual-content path (Phase 0/4) is the fallback.
- A Google Form (different DOM shape: form inputs/radio groups, good generic-platform control case)
- A true/false or free-text/essay-response question type (not yet tested — all MCQ/coding so far)
- CodeSignal or a logged-in Codility session with the live editor actually loaded (both attempts in this round hit auth/URL walls; would need valid test credentials)

## 9. Open decisions before implementation (need a decision, not blocking the plan itself)

1. ~~Image count cap per query~~ **Decided 2026-10-10: maximum 4.**
2. ~~Should free-tier users get a lower image cap than Pro?~~ **Decided 2026-10-10: no. Free and Pro users get the same cap of 4 pictures per question.**
3. Exact main-content selector heuristic and chrome-stripping denylist — start generic (`main`/`[role=main]`/`article`) or invest in per-platform selectors for the 5 tested sites specifically?
4. Visual-content trigger threshold for Phase 0 (suggested ≥ ~40,000 px² rendered area with empty alt). Tune it against the saved Criteria Corp Q7 copy.
5. Output cap size (suggested ~12k chars).

## 10. Implementation status (2026-10-10)

**Phase A and Phase B are implemented.** Cross-origin iframes remain optional/later.

Where things live:
- `../../../src-tauri/src/chrome/extract.js`: the page reader (DOM walker, editors, choices, frames, visuals, 300ms readiness wait). Returns JSON.
- `../../../src-tauri/src/chrome/mod.rs`: `extract_page`, `get_resource_images` (Chrome cache via `Page.getResourceContent`), `fit_image_bytes` (size budget, transparent→white JPEG), `scroll_drawing_into_view`.
- `../../../src-tauri/src/main.rs`: commands `extract_tab_text` (now returns the JSON), `get_tab_images`, `scroll_drawing_into_view`; `query_ai_with_image` / `query_ai_via_proxy_with_image` now take `image_paths` (max 4; total image budget 4.5 MB split across the pictures).
- `../../../src/services/tabExtraction.ts`: result types plus the "when to add a screenshot" rules; `src/App.tsx` text-mode flow.
- Tests: `yarn test:extract` (10 saved example pages in `../../../tests/extract/fixtures`, run in headless Chrome), `cargo test` (image fitting), and `cargo test -- --ignored chrome_integration` (real Rust code against headless Chrome: extraction, exact original image bytes from the cache, chart scroll).

Extra rules added during implementation (found while testing live sites):
- **Link-list removal:** blocks with ≥5 links making up ≥75% of their text are skipped (Codility's lesson menu and footer have no nav/footer tags). Never applied to blocks containing choices, blanks, dropdowns or editors.
- `sup`/`sub` become `^`/`_` (LeetCode `10^4`, `O(n^2)`).
- Email/password/phone fields, search boxes and type-ahead pickers (`role=combobox`) are not shown as `[blank]`.
- Common action buttons (Submit, Run, Share on X, ...) are dropped.

Live checks (2026-10-10): LeetCode (full cpp stub + CodeMirror test-case editor recovered; nav and Similar Questions removed), HackerRank (full stub via the Monaco API; it blocks headless Chrome, so it was checked in the visible browser), Codility (sidebar, top bar and footer links removed).

**Not verified yet:** the full app flow end to end (Tauri app + AI call), the WS ("chrome://inspect toggle") connection mode for page images (the HTTP mode is covered by the integration test), and a login-gated image.

## 11. Implementation phases (final, 2026-10-10)

**Implementation Phase A — Better text reading** (one PR; everything lives in the new extraction script)
- Phase 5: move the extraction logic into `extract.js` (structured JSON output) and set up the saved test pages + automated comparison. Do this first so every other change is testable.
- Phase 1: code editors (Monaco via its API, picking the right model).
- Phase 2: custom DOM walker (blanks, choices, hidden passages, shadow DOM, no user answers except code, junk removal, output cap).
- Same-origin iframes with guardrails (denylist, size cap, visibility filter).
- Phase 3: readiness wait, hard cap 300ms.

**Implementation Phase B — Pictures** (one PR; builds on Phase A's JSON output)
- Phase 0 + Phase 4: detect visual content; send text + up to **4** pictures (free and Pro alike). Image files come from Chrome's own copy via `Page.getResourceContent` (verified); drawn content (charts, whiteboards) uses the tab screenshot.
- Automatic switch to the tab screenshot when almost no text and no pictures are found.
- Hidden screen-reader text on canvas apps (e.g. Figma) included with the screenshot.
- Extend both AI paths (`ai/gemini.rs`, `main.rs` proxy command) from one image to up to 4. The Supabase proxy needs no change.

**Optional later — only if a real exam needs it**
- Cross-origin iframes via CDP frame attachment.

## 12. Monaco without `window.monaco`, and other project files (2026-10-21)

Found on CoderPad (React app, Monaco bundled, no `window.monaco`): the editor only draws the visible lines, so the DOM fallback cut the file off.

- **Editor via React fiber** (`reactMonaco` in `extract.js`): walk the React fiber tree (time-boxed to 150ms) for objects with `getModel()` + `getDomNode()`, match them to the on-screen editor by DOM node, and read `getValue()`. `source` = `react-fiber`. Verified on a live CoderPad pad (full file recovered).
- **Other files the editor holds:** all models (`monaco.editor.getModels()` if exposed, otherwise the model service found through the editor's own properties) are added as `--- Project file <path> (<language>) ---` blocks after the active editor. Skipped: empty files, `node_modules`/`dist`/`build`, lockfiles, `.min.*`, `.map`, `.svg`. Caps: 10k chars per file, 30k total, 20 files; the rest is reported as `[N more project files not included]`.
- Nothing is scrolled, clicked or copied. Generic: any site using Monaco benefits; sites with a single model are unchanged.
- Test: `../../../tests/extract/fixtures/monaco-react-multifile.html`.

## 13. Cross-origin frames via the debugger (2026-10-22)

Found on a HackerRank assessment: the problem statement, file tree and editor live in one full-size frame served from another site (`*.hrcdn.net`). The page script cannot read it (`skippedFrames[].reason = cross-origin`), so the prompt had no question and no code.

- A frame from another *site* is its own DevTools target of type `iframe` whose parent (`parentFrameId` over WS, `parentId` over HTTP `/json/list`) is the tab. Verified locally in both modes' listings and by running a script inside it.
- `extract_page` (`chrome/mod.rs`): if the page reports cross-origin skips, list the tab's child frame targets, run the same `extract.js` inside each, and append the result as `--- Embedded frame (host) ---`. Code blocks keep counting after the parent's (`CODE_NUMBER_OFFSET` in the script is replaced per frame).
- Guardrails: widget denylist (same hosts as the page script), http(s) only, viewport at least 100x100, empty frames dropped, max 4 frames, depth 2, 6 s total timeout. Any failure keeps the parent-page result.
- Not handled: **same-site** cross-origin frames (different port or subdomain of the same site) share a process, so they are not separate targets; images/canvases inside embedded frames; the WebSocket (chrome://inspect toggle) mode was not tested live.
- Test: the second half of `cargo test -- --ignored chrome_integration` (two local servers on different sites).
