# CrackingInterview Video Scripts

Scripts ready to paste into ElevenLabs for AI voiceover generation. Each section includes **[SCREEN]** directions for what to show while the narration plays.

---

## Video 1: Homepage Introduction (~5 minutes)

### Section 1: Hook + Overview (0:00 – 0:30)

**[SCREEN]** Show a split view: on the left, a coding interview screen with a hard problem. On the right, the CrackingInterview app with a solution appearing.

> Ever frozen during a coding interview, staring at a problem you know you could solve with just a little more time? CrackingInterview is your invisible AI-powered interview coach. It sits right on your screen, extracts problems from your browser, solves them instantly, and even listens to your verbal interviews in real time. It works on Mac and Windows, and it's designed to stay completely out of the way while giving you exactly the help you need.

---

### Section 2: Audio / Verbal Interview Mode (0:30 – 1:50)

**[SCREEN]** Show a Zoom meeting window on the left side of the screen with a fake interviewer (AI-generated avatar or a profile picture with camera off). The CrackingInterview app sits on the right side, partially overlapping. In the app, switch to the Audio input source. Click on the language selector — show the full dropdown list of supported languages (scroll through: English, Spanish, French, German, Persian, Chinese, Japanese, Portuguese, and more). Select English. Click the Record button. The "interviewer" on Zoom speaks (use Google Translate text-to-speech playing as system audio): "Can you explain how you would implement a rate limiter?" — show live transcription appearing in real time in the CrackingInterview app as the Zoom audio plays. The silence detection countdown triggers, and the AI response streams in with a detailed explanation and code. The interviewer asks a follow-up through Zoom: "How would you handle distributed rate limiting across multiple servers?" — transcription flows in again, and the AI response builds on the previous answer with full context (demonstrating agent memory). Then the interviewer asks a behavioral question: "Tell me about a time you optimized a system for scale." The AI responds using context from the uploaded resume, referencing the candidate's actual past experience and projects — all while the Zoom call continues naturally in the background.

> Here's the most powerful feature of CrackingInterview — built-in audio mode for verbal interviews. Pick your language — the app supports over 30 languages, including English, Spanish, French, German, Persian, Chinese, Japanese, and many more. Click Record and the app starts listening through your microphone, transcribing the interviewer's questions in real time. When the interviewer stops speaking, the app detects the silence, shows a brief countdown, and sends the transcribed question to the AI. You get a structured answer with explanation and code. But here's what makes this truly game-changing — the AI remembers everything. When the interviewer asks a follow-up, the AI builds on its previous response with full context. It doesn't start from scratch — it understands the entire conversation flow. Every follow-up question gets a smarter, more context-aware answer because the AI carries the full history of your interview. It's like having a coach who's been listening to every word. And it gets even better. Upload your resume in the settings, and the AI knows your background. When the interviewer asks a behavioral question like "Tell me about a time you optimized a system," the AI pulls from your actual experience — your projects, your roles, your achievements. It answers as you, with your story. This is a real-time AI interview partner that knows your history and follows the conversation like a human coach sitting right next to you.

---

### Section 3: Text Extraction + AI Solving (1:50 – 2:30)

**[SCREEN]** In the CrackingInterview app, click the "Open Chrome" button — Chrome launches automatically. In Chrome, manually navigate to a LeetCode problem (e.g., Two Sum). Switch back to the CrackingInterview app and click the refresh icon next to the input source dropdown — the LeetCode tab now appears in the list. Open Prompt Settings, show the "Algorithm Optimal" prompt selected, and set the language to Python. Back in the main app window, press the Solve hotkey. Show the AI response streaming in with a clean, formatted solution. Scroll down through the full response.

> CrackingInterview also excels at text-based coding problems. Click the Open Chrome button right in the app — it launches Chrome for you. Navigate to any coding problem, like this first Missing Positive problem on LeetCode. Back in the app, hit the refresh icon to detect your tab. Open the prompt settings — select Algorithm Optimal for the most efficient solution, and choose Python as your language. Press the hotkey. CrackingInterview extracts the problem text directly from your Chrome tab and sends it to the AI. A complete solution streams in — optimized approach, time and space complexity, and clean Python code, ready to use.

---

### Section 4: System Design Interviews (2:30 – 3:20)

**[SCREEN]** Switch the prompt to "System Design". Show a Miro board open in the browser with a system design question like "Design a URL Shortener" written on it. Back in the CrackingInterview app, switch the input source to Screenshot mode. Press the Screenshot hotkey — show the screen capture overlay appear, drag to select the Miro board area containing the question. The captured image is sent to the AI. Show the AI response with a Mermaid architecture diagram rendering beautifully, followed by structured sections.

> CrackingInterview isn't just for algorithm problems — and it works with more than just text. Switch to the System Design prompt. Your interviewer has shared a Miro board with the question on it. No problem. Switch the input to Screenshot mode or just press the hotkey to capture screen shot from your whole chrome tab — the app reads the question directly from the image. In seconds, the AI generates a complete system design response with a visual architecture diagram. These diagrams render right inside the app using Mermaid, so you can see the system components, data flow, and connections at a glance. Below the diagram, you get structured sections covering the API design, database schema, and scaling strategy. It's like having a senior engineer walk you through the design on a whiteboard.

---

### Section 5: Customizable Prompts & Settings (3:20 – 3:55)

**[SCREEN]** Open Settings, go to the Prompts tab. Show the list of built-in prompts. Click to edit the "Verbal Interview" prompt — show the system prompt and user prompt fields with a `{resume}` placeholder already visible in the prompt body. Then go to the Resume tab (or resume upload area), drag in a PDF resume — show it being uploaded and parsed. Switch back to the Prompts tab to show the placeholder is now populated with the resume content. Then show the AI Models tab with model selection.

> Every interview is different, and CrackingInterview lets you customize how the AI responds. Open Settings and go to the Prompts tab. You'll find built-in prompts for algorithm problems, system design, verbal interviews, and more. Each one is fully editable. Look at the Verbal Interview prompt — you can add a resume placeholder in system prompt. Upload your resume in the upload document, and the app automatically injects it into every verbal interview response. The AI now knows your background, your experience, and your skills — so its answers are tailored specifically to you. In the AI Models tab, you can choose between multiple AI providers. Pro users get access to premium models from OpenAI, Google, and Anthropic for the highest quality responses.

---

### Section 6: Stealth Features (3:55 – 4:50)

**[SCREEN]** This entire section uses a persistent side-by-side layout. **Left side (~60%)**: your actual screen showing HackerRank's coding environment in Chrome with CrackingInterview floating on top. **Right side (~40%)**: the Zoom screen-share preview window, which only shows HackerRank — CrackingInterview is completely invisible. Both views are visible simultaneously throughout the entire section so the viewer can constantly compare what you see vs. what the interviewer sees.

The section opens with this split already in place — Zoom is running, screen share is active, stealth mode is on. On the left, CrackingInterview has an AI response visible, floating on top of HackerRank. On the right, the Zoom preview shows only HackerRank. First, briefly show the Settings App tab and drag the transparency slider from 100% down to about 30% — the app becomes see-through and the HackerRank code editor is now visible through it. The Zoom side still shows nothing. Now start typing code into HackerRank — the mouse cursor is inside Chrome, the keyboard focus is on HackerRank's code editor, and CrackingInterview stays floating on top, never going behind. The Zoom side continues to show only HackerRank with the code you're typing. Press a global hotkey to scroll down the AI response — the left side scrolls, the right side shows nothing. Press another hotkey to move the CrackingInterview window to a different corner — the left side shows it moving, the right side shows nothing. Press another hotkey to toggle visibility off and on — the left side shows the app disappearing and reappearing, the right side never changes. Throughout all of this, the mouse cursor and keyboard focus never leave HackerRank.

> CrackingInterview is designed to be invisible when you need it to be — and completely undetectable by interview platforms. Watch the left side of your screen. That's what you see — the AI solution floating right on top of your coding environment. Now look at the right side. That's what your interviewer sees through Zoom screen share — just HackerRank. Nothing else. The app is invisible. Let's make it even better. Adjust the transparency, and now you can read the AI response and see the code editor right through it. Start typing your solution directly into HackerRank. The app stays on top — always. It never drops behind your browser. And your mouse, your keyboard, your focus — it all stays inside HackerRank the entire time. You never click on CrackingInterview. You never switch tabs. You never leave the page. That matters, because platforms like HackerRank, CodeSignal, and CoderPad track exactly that — tab switches, focus loss, leaving the browser window. That's how they flag cheating. With CrackingInterview, none of those signals ever fire. Watch — I'm scrolling through the AI response with a hotkey. Left side scrolls. Right side? Nothing. I'm moving the window to a different corner. Left side moves. Right side? Still just HackerRank. I'm toggling the app window on and off. Gone, back, gone, back. Right side never changes. Every action is controlled through global hotkeys that work system-wide without ever shifting your focus. As far as the interview platform is concerned, you never left. Transparent, always on top, zero focus changes, invisible to screen share — this is what undetectable looks like.

---

### Section 7: Call to Action (4:50 – 5:05)

**[SCREEN]** Show the crackinginterview.org website with the download button and pricing section.

> CrackingInterview is available now for Mac and Windows. Get started with the free tier, or go Pro for full access to premium AI models, audio interviews, and unlimited features. Download it today at crackinginterview.org, and walk into your next interview with confidence.

---
---

## Video 2: YouTube Ad (~45 seconds)

**[SCREEN]** directions are inline. Fast-paced cuts, energetic background music throughout.

---

**[0:00 – 0:05] HOOK**

**[SCREEN]** Dark screen, then the CrackingInterview app window fades in next to a coding interview.

> What if you had an AI coach sitting right next to you during your coding interview?

---

**[0:05 – 0:10] PROBLEM**

**[SCREEN]** Quick zoom into a hard LeetCode problem in Chrome. Title clearly visible.

> A hard LeetCode problem just dropped. Clock is ticking.

---

**[0:10 – 0:17] SOLVE**

**[SCREEN]** Hotkey press with a visual key indicator. AI response streams in instantly — clean code, clear explanation.

> One hotkey. The problem is extracted, analyzed, and solved — with optimized code and full explanation. Instantly.

---

**[0:17 – 0:25] AUDIO MODE**

**[SCREEN]** Audio waveform appears. Live transcription text flowing in. AI response appears.

> Verbal interview? CrackingInterview listens in real time, transcribes the question, and delivers a structured answer before you even finish thinking.

---

**[0:25 – 0:35] SYSTEM DESIGN + STEALTH**

**[SCREEN]** Quick flash of a Mermaid architecture diagram rendering. Then a dramatic split: left side shows your screen with CrackingInterview visible, right side shows the Zoom screen-share preview — the app is completely invisible.

> System design with auto-generated architecture diagrams. And stealth mode? Your interviewer's screen share sees nothing. Completely invisible.

---

**[0:35 – 0:45] CTA**

**[SCREEN]** App logo centered. Website URL appears. "Mac & Windows" badge. "Free to Start" text.

> CrackingInterview. Your unfair advantage in every coding interview. Download free at crackinginterview.org.

---
---

## Full Production Guide

This guide walks you through the entire process from raw screen recordings to finished, published videos.

---

### Step 1: Set Up Your Tools

| Tool | Purpose | Cost | Link |
|------|---------|------|------|
| **OBS Studio** | Screen recording (high quality, free) | Free | [obsproject.com](https://obsproject.com) |
| **ElevenLabs** | AI voiceover from script text | Free (10 min/mo) or $5/mo | [elevenlabs.io](https://elevenlabs.io) |
| **Descript** | Video editing, merging, captions, export | $24/mo Hobbyist | [descript.com](https://descript.com) |
| **Canva** | Intro/outro title cards, thumbnails | Free | [canva.com](https://canva.com) |
| **Google Gemini** | AI video review & feedback | Free | [gemini.google.com](https://gemini.google.com) |

**Install these before you begin:**
1. Download and install OBS Studio
2. Create accounts on ElevenLabs, Descript, Canva, and Google Gemini
3. (Optional) Install Descript desktop app for screen recording — but OBS gives better quality

---

### Step 2: Record Screen Footage

Record each section as a **separate screen recording**. This makes editing much easier — you can re-record one section without affecting others.

#### OBS Studio Setup

1. Open OBS Studio
2. Go to **Settings > Video**: set Base Resolution and Output Resolution both to **1920x1080**
3. Go to **Settings > Output**: set Recording Format to **MP4**, Encoder to **x264**, Quality to **High Quality**
4. Add a source: **Display Capture** (for full screen) or **Window Capture** (for specific window)
5. Click **Start Recording** when ready, **Stop Recording** when done
6. Recordings save to your Videos folder by default

#### Before Recording — Clean Up Your Screen

- Set a clean desktop wallpaper (solid dark color or subtle gradient)
- Hide Chrome bookmarks bar: press `Cmd+Shift+B` (Mac) or `Ctrl+Shift+B` (Windows)
- Close all unnecessary tabs and apps
- Turn off notification popups (Do Not Disturb mode)
- Pre-load all demo tabs (LeetCode problem, system design question)
- Pre-type any inputs you'll need

#### What to Record (one recording per section)

| File Name | What to Record | Duration |
|-----------|---------------|----------|
| `01-overview.mp4` | App window appearing next to a coding problem | ~30s |
| `02-audio.mp4` | Zoom meeting with fake interviewer on left + CrackingInterview app on right > Select Audio source > language dropdown > select English > Click Record > Interviewer "speaks" via Google Translate system audio > Live transcription > AI response > Follow-up question showing agent memory > Behavioral question answered using resume context (see setup guide below) | ~80s |
| `03-solve.mp4` | Click "Open Chrome" in app > Chrome opens > navigate to LeetCode Two Sum > back in app refresh input source > Prompt Settings: Algorithm Optimal + Python > press Solve hotkey > AI streams solution > scroll through response | ~40s |
| `04-system-design.mp4` | Switch to System Design prompt > show Miro board with "Design a URL Shortener" question > switch to Screenshot mode > press Screenshot hotkey > drag to capture Miro board area > AI response with Mermaid diagram | ~50s |
| `05-settings.mp4` | Open Settings > Prompts tab > Open "Verbal Interview" prompt > show {resume} placeholder > go to Resume tab > upload a PDF resume > back to Prompts to show placeholder populated > AI Models tab > Model selection | ~30s |
| `06-stealth.mp4` | Persistent side-by-side: left = your screen (HackerRank + CrackingInterview on top), right = Zoom screen-share preview (only HackerRank visible). Transparency slider > type code in HackerRank > app stays on top > global hotkeys scroll/move/toggle without focus change > Zoom side never shows the app (see recording guide below) | ~55s |
| `07-cta.mp4` | crackinginterview.org website with download button and pricing | ~15s |

#### How to Record the Audio/Zoom Interview Scene (Section 2)

This is the most important demo in the entire video — it sells the core product experience. The goal is to make it look and feel like a real Zoom interview.

**How to create a fake Zoom meeting with a second person:**

You need two Zoom accounts so the meeting shows two participants (you + the "interviewer"). Here are three approaches, from simplest to most polished:

**Method A: Second Zoom Account — Camera Off (easiest, free)**

1. Create a second free Zoom account using a different email (e.g., a Gmail alias like `yourname+interviewer@gmail.com`)
2. On the second account, go to Zoom profile settings at [zoom.us/profile](https://zoom.us/profile):
   - Set the display name to something like **"Sarah Chen"** or **"David Kim — Engineering Manager"**
   - Upload a professional-looking profile picture (use a stock photo from [unsplash.com](https://unsplash.com) or generate one with [thispersondoesnotexist.com](https://thispersondoesnotexist.com))
3. From your main account, start a Zoom meeting and copy the invite link
4. Open an **incognito/private browser window**, go to the invite link, and join from the second account. Keep the camera OFF on this second account
5. The Zoom window now shows two participant tiles: your tile + the "interviewer" tile with their name and profile photo. This looks exactly like a real interview where the interviewer has their camera off (very common in real interviews)
6. Arrange the Zoom window so the interviewer tile is prominently visible

**Method B: Second Account + OBS Virtual Camera (more polished, free)**

This makes the fake interviewer appear with a **live webcam feed** in Zoom — looks even more real. OBS Virtual Camera turns any OBS scene into a fake webcam that other apps (like Zoom) can use as a camera input.

**Step 1: Get a stock video of a person**

You need a short video of someone sitting at a desk, looking at the screen, nodding occasionally — the kind of thing an interviewer looks like on Zoom. Download one of these:
- [Pexels — "person video call"](https://pexels.com/search/videos/person%20video%20call/) — free, no sign-up
- [Pexels — "woman office laptop"](https://pexels.com/search/videos/woman%20office%20laptop/) — free
- Search for "person on zoom call" or "person listening at desk" on Pexels or [Pixabay](https://pixabay.com/videos/)
- Or record a friend sitting at a desk for 2-3 minutes — just looking at the screen, nodding, occasionally glancing down. No talking needed

Save the video file somewhere accessible, e.g. `~/Videos/fake-interviewer.mp4`

**Step 2: Set up OBS Studio with the video**

1. Open **OBS Studio** (download from [obsproject.com](https://obsproject.com) if not installed)
2. Look at the bottom of the OBS window — you'll see a row of panels: **Scenes**, **Sources**, **Audio Mixer**, **Scene Transitions**, and **Controls**

**Create a Scene:**

3. In the **Scenes** panel (bottom-left), click the **+** button
4. A dialog pops up asking for the scene name — type `Fake Interviewer` and click **OK**
5. The new scene appears in the list and is now selected (highlighted)

**Add the video as a Media Source:**

6. In the **Sources** panel (directly to the right of Scenes), click the **+** button
7. A dropdown menu appears with a list of source types. Select **Media Source** (sometimes called "Media Source (FFmpeg)")
8. A dialog asks "Create new" or "Add existing" — leave **"Create new"** selected
9. Type a name: `Interviewer Video` → click **OK**
10. The **Properties** dialog opens — this is where you load the video file:

    | Setting | Value |
    |---------|-------|
    | **Local File** | ✅ Check this box (it should be checked by default) |
    | **Input** | Click **Browse** → navigate to your video file (e.g. `~/Videos/fake-interviewer.mp4`) → select it → click **Open** |
    | **Loop** | ✅ **Check this box** — critical! This makes the video repeat endlessly so the "interviewer" stays on screen for the entire recording |
    | **Restart playback when source becomes active** | ✅ Check this box |
    | **Use hardware decoding when available** | ✅ Leave checked (default) |
    | All other settings | Leave at defaults |

11. Click **OK** to close the Properties dialog

**Resize the video to fill the canvas:**

12. The video now appears in the OBS preview area (the big black rectangle in the center). It might appear small or off-center
13. Right-click on the video in the preview → **Transform** → **Fit to screen**
    - This stretches the video to fill the entire OBS canvas
    - Alternatively: right-click → **Transform** → **Stretch to screen** if "Fit" leaves black bars
14. The OBS preview should now show the person from your video filling the entire frame — this is exactly what Zoom will see as the "webcam" feed

**Verify it's playing:**

15. You should see the video playing in the OBS preview. If it's frozen on a black frame, right-click the `Interviewer Video` source in the Sources panel → **Media Controls** → click the **play ▶** button
16. The video should now be looping continuously in the preview

**Step 3: Start the OBS Virtual Camera**

1. In OBS, go to the menu bar: **Tools** → **Start Virtual Camera**
   - On Mac: if this is your first time, macOS may ask for camera permissions — allow it
   - The button in the bottom-right control panel also shows **Start Virtual Camera** — you can click either one
2. The OBS status bar at the bottom now shows "Virtual Camera Active" — OBS is now broadcasting the fake interviewer video as a webcam feed that any app can use

**Step 4: Join Zoom with the fake camera**

1. From your main Zoom account, start a meeting and copy the invite link
2. Open an **incognito/private browser window** and go to the invite link to join from your second Zoom account
3. When the Zoom browser client (or Zoom app) asks to select a camera, choose **"OBS Virtual Camera"** from the camera dropdown
   - If you're already in the meeting: click the **^** arrow next to the camera icon in the Zoom toolbar → select **"OBS Virtual Camera"**
   - If OBS Virtual Camera doesn't appear, make sure Step 3 is done (Virtual Camera is running in OBS) and try restarting Zoom
4. The Zoom meeting now shows your stock video playing as the "interviewer's" live webcam. It looks like a real person on camera

**Step 5: Verify the layout**

1. Your Zoom window should now show two tiles: **you** (main account) and the **"interviewer"** (second account with the OBS Virtual Camera feed)
2. On your main account, you can turn your own camera off if you don't want your face in the recording — the focus should be on the interviewer tile + the CrackingInterview app
3. Position everything for the recording: Zoom on the left, CrackingInterview on the right

**Important notes:**
- You need **two separate instances** of Zoom running — the easiest way is to use the Zoom desktop app for your main account and the Zoom **web client** (in an incognito browser) for the interviewer account
- The stock video doesn't need audio — you're providing the interviewer's "voice" separately via Google Translate or ElevenLabs MP3s
- The lip movement won't match the audio, but this is fine — the viewer's attention will be on the CrackingInterview app transcribing and generating answers, not scrutinizing the interviewer's face
- If the video looks choppy in Zoom, in OBS go to **Settings > Video** and set the output resolution to **1280x720** and FPS to **30** — this is plenty for a Zoom webcam feed

**Method C: AI-Generated Talking Avatar (most polished, paid)**

1. Use [HeyGen](https://heygen.com) (free trial: 3 videos) or [D-ID](https://d-id.com) (free trial: 5 min)
2. Type the three interviewer questions as the script text
3. Choose a professional-looking avatar and generate a video of them "asking" the questions
4. Play this video using OBS Virtual Camera (same as Method B step 2-4), or simply overlay the generated video on top of the Zoom tile area in post-production using Descript

---

**Full recording setup (works with any method above):**

1. **Prepare the interviewer audio** — pre-type all three questions in Google Translate ([translate.google.com](https://translate.google.com)):
   - "Can you explain how you would implement a rate limiter?"
   - "How would you handle distributed rate limiting across multiple servers?"
   - "Tell me about a time you optimized a system for scale."
   - You'll click the **speaker icon** to play each one as system audio during recording
   - *Better option*: Pre-generate the questions as MP3s using ElevenLabs with a different voice than your narration — sounds much more natural than Google Translate
2. **Route system audio to CrackingInterview**: The app needs to hear the interviewer audio as microphone input. On Mac, install **BlackHole** ([existential.audio/blackhole](https://existential.audio/blackhole/) — free). In System Settings > Sound, set output to BlackHole. Then in CrackingInterview, set the audio input to BlackHole. This routes whatever plays on your speakers into the app as if it were a mic
3. **Screen layout**: Position Zoom on the left ~60% of the screen, CrackingInterview app on the right ~40%, slightly overlapping
4. **Upload your resume** in CrackingInterview Settings > Resume tab before recording
5. **Record with OBS** using Display Capture to capture the full screen

**Recording flow:**

1. Start OBS recording
2. Show the Zoom meeting with two participants — the "interviewer" is visible
3. In CrackingInterview, switch to Audio, select English, click Record
4. Play the first question audio (Google Translate speaker icon or pre-made MP3). The audio plays through the system → BlackHole routes it to CrackingInterview → live transcription appears in the app. To the viewer, it looks like the Zoom interviewer asked the question
5. Wait for silence detection countdown → AI response streams in
6. Pause 3-4 seconds, then play the second question (follow-up) → AI builds on the previous answer
7. Pause, then play the third question (behavioral) → AI uses resume context in its response
8. Stop recording

**Tips:**
- Practice the full flow 1-2 times before the real recording to nail the timing
- Keep 3-4 seconds of silence between questions so the app's silence detection triggers cleanly
- If using Method A (camera off), the viewer won't question it — most real interviewers keep their camera off
- Keep Zoom's participant count and meeting timer visible to sell the realism
- If using ElevenLabs for the interviewer voice, pick a voice that sounds different from your narration voice (e.g., use a female voice for the interviewer if your narrator is male)

---

#### How to Record the Stealth/Zoom Scene (Section 6)

This is the most important demo. Here are two methods:

**Method A: Side-by-side with iPhone (recommended)**
1. Open CrackingInterview with an AI response visible on your Mac
2. Enable Stealth Mode in Settings > App
3. Open Zoom, start a solo meeting, click **Share Screen** and share your desktop
4. Position your iPhone to record your Mac's screen showing CrackingInterview clearly visible
5. On your iPhone, also capture the Zoom meeting window showing the shared screen where the app is invisible
6. In editing, place the two views side by side with labels: "Your Screen" / "What Interviewer Sees"

**Method B: OBS with two scenes**
1. In OBS, set up **Display Capture** (shows everything including CrackingInterview)
2. Start Zoom screen share
3. Also add a **Window Capture** of the Zoom meeting window (shows the shared view without the app)
4. Record both, then in Descript place them side by side

---

### Step 3: Generate AI Voiceover with ElevenLabs

Generate the voiceover for each section from the narration scripts above. Each section's text (the `>` quoted paragraphs) is what you paste into ElevenLabs.

#### ElevenLabs Step-by-Step

1. Go to [elevenlabs.io](https://elevenlabs.io) and sign in (or create a free account)
2. Click **Text to Speech** in the left sidebar
3. **Choose a voice**: Click the voice dropdown at the top
   - Recommended voices: **"Josh"** (confident, professional male) or **"Rachel"** (clear, warm female)
   - Click the play icon next to each voice to preview before choosing
   - Pick one voice and use it for ALL sections to keep the videos consistent
4. **Adjust voice settings** (click the settings gear icon next to the voice):
   - **Stability**: Set to **0.50** (more variation = more natural; higher = more robotic)
   - **Clarity + Similarity Enhancement**: Set to **0.75** (higher = clearer pronunciation)
   - **Style Exaggeration**: Leave at **0** for professional tone
   - **Speaker Boost**: Turn **ON** for clearer output
5. **Generate each section one at a time**:

#### Video 1 — Section-by-Section Audio Generation

For each section below, paste ONLY the narration text (not the [SCREEN] directions) into the ElevenLabs text box, click **Generate**, listen to the preview, then click the **download icon** to save as MP3.

| ElevenLabs File to Save | Script Section to Paste |
|--------------------------|------------------------|
| `V1-01-hook.mp3` | Section 1: "Ever frozen during a coding interview..." (full paragraph) |
| `V1-02-audio.mp3` | Section 2: "Here's the most powerful feature of CrackingInterview..." (full paragraph) |
| `V1-03-solve.mp3` | Section 3: "CrackingInterview also excels at text-based..." (full paragraph) |
| `V1-04-sysdesign.mp3` | Section 4: "CrackingInterview isn't just for algorithm problems..." (full paragraph) |
| `V1-05-settings.mp3` | Section 5: "Every interview is different..." (full paragraph) |
| `V1-06-stealth.mp3` | Section 6: "CrackingInterview is designed to be invisible..." (full paragraph) |
| `V1-07-cta.mp3` | Section 7: "CrackingInterview is available now..." (full paragraph) |

#### Video 2 — Section-by-Section Audio Generation

For Video 2, use slightly **faster speed**: in ElevenLabs settings, increase the speed slider to **1.1x** for a more energetic, ad-like delivery.

| ElevenLabs File to Save | Script Section to Paste |
|--------------------------|------------------------|
| `V2-01-hook.mp3` | "What if you had an AI coach sitting right next to you during your coding interview?" |
| `V2-02-problem.mp3` | "A hard LeetCode problem just dropped. Clock is ticking." |
| `V2-03-solve.mp3` | "One hotkey. The problem is extracted, analyzed, and solved — with optimized code and full explanation. Instantly." |
| `V2-04-audio.mp3` | "Verbal interview? CrackingInterview listens in real time, transcribes the question, and delivers a structured answer before you even finish thinking." |
| `V2-05-stealth.mp3` | "System design with auto-generated architecture diagrams. And stealth mode? Your interviewer's screen share sees nothing. Completely invisible." |
| `V2-06-cta.mp3` | "CrackingInterview. Your unfair advantage in every coding interview. Download free at crackinginterview.org." |

#### Troubleshooting ElevenLabs

- If a word is mispronounced (e.g., "Mermaid", "LeetCode"), try spelling it phonetically: "Mer-maid", "Leet-Code"
- If the pacing feels rushed, add a period or `...` to create a natural pause
- If you run out of free characters (10,000/mo on free plan), upgrade to $5/mo Starter for 30,000 characters
- Save all MP3 files in one folder, e.g., `~/Videos/CrackingInterview/audio/`

---

### Step 4: Create Intro & Outro Cards in Canva

1. Go to [canva.com](https://canva.com)
2. Create a new design: choose **Video** (1920x1080)
3. **Intro card** (3-5 seconds): App name "CrackingInterview" + tagline "Your AI Interview Coach" + logo if you have one. Use a dark background with clean white text.
4. **Outro card** (5-7 seconds): "Download Free at crackinginterview.org" + "Available on Mac & Windows" + logo. Include a subtle call-to-action arrow or button graphic.
5. **Export as MP4** (Canva lets you export short animations as video clips)
6. Save as `intro-card.mp4` and `outro-card.mp4`

---

### Step 5: Edit & Merge Everything in Descript

This is where you combine all recordings, voiceover clips, and cards into the final videos.

#### Initial Setup

1. Download and install [Descript](https://descript.com) desktop app
2. Create a new Project: name it "CrackingInterview Videos"
3. Create two **Compositions**: "Homepage Video" and "YouTube Ad"

#### Assembling Video 1 (Homepage ~5:15 min)

1. **Import all files**: Drag in all your screen recordings (`01-overview.mp4` through `07-cta.mp4`), all voiceover audio (`V1-01-hook.mp3` through `V1-07-cta.mp3`), and the Canva cards
2. **Build the timeline in order**:
   - Drag `intro-card.mp4` to the beginning
   - For each section: drag the voiceover MP3 first, then place the matching screen recording on the video track above it
   - Drag `outro-card.mp4` to the end
3. **Sync video to audio**: Descript aligns by transcript. You can also manually trim screen recordings to match voiceover length — just drag the edges of clips to shorten or extend
4. **Add text overlays**: For hotkey demonstrations, add text labels like "Press Cmd+Shift+S" using Descript's text tool (click the "T" button)
5. **Add transitions**: Between sections, add a simple cross-dissolve (right-click between clips > Add Transition)
6. **Add captions**: Click **Edit > Add captions**. Descript auto-generates word-by-word captions. Choose a style (white text with dark background works well)
7. **Review the full video**: Play it start to finish. Trim any dead air or awkward pauses by selecting the silence in the transcript and deleting it
8. **Export**: File > Export > MP4, Resolution: 1080p, Quality: High

#### Assembling Video 2 (YouTube Ad ~45 sec)

1. Same process but with the V2 audio files and corresponding screen footage
2. **Add background music**: In Descript, click **Add Stock Music** (built into Hobbyist plan). Search for "tech", "upbeat", or "corporate". Set volume to about 20-30% so it doesn't overpower the voiceover
3. **Faster pacing**: Trim any gaps between sections to zero. The cuts should be immediate — no pauses
4. **Add zoom animations**: Select a clip, click **Effects > Ken Burns** or manually keyframe a slow zoom-in to make static screen recordings feel dynamic
5. **Export**: Same as above, 1080p MP4

#### Descript Keyboard Shortcuts You'll Use

| Action | Shortcut (Mac) |
|--------|----------------|
| Split clip | `Cmd+Shift+K` |
| Delete selected | `Delete` |
| Undo | `Cmd+Z` |
| Play/Pause | `Space` |
| Add marker | `M` |

---

### Step 6: AI Video Review & Feedback

After recording each raw section (and again after final assembly), use AI tools to get feedback and catch issues you might miss.

#### Recommended AI Tools for Video Review

| Tool | Best For | How to Use | Cost |
|------|----------|-----------|------|
| **Google Gemini** | Reviewing raw footage and final videos | Upload video file directly to chat | Free |
| **ChatGPT (GPT-4o)** | Detailed feedback on pacing, messaging, clarity | Upload video file to chat | $20/mo |
| **Claude** | Script and narration feedback | Paste script text for written review | Free / $20/mo |
| **Descript AI** | Auto-remove filler words, find dead air, generate summaries | Built-in features while editing | Included ($24/mo) |

#### How to Use Google Gemini for Video Review (Recommended — Free)

1. Go to [gemini.google.com](https://gemini.google.com)
2. Click the **attachment/upload icon** in the chat input
3. Upload your video file (MP4, up to 2GB on free tier)
4. Wait for it to process (may take 30-60 seconds for a 5-minute video)

**Prompts to use at each stage:**

**After recording raw screen footage (before adding voiceover):**
> I'm creating a product demo video for a desktop application called CrackingInterview. This is a raw screen recording for [Section Name - e.g., "the AI problem-solving feature"]. The voiceover narration will be added later. Please review:
> 1. Is the UI clearly visible and easy to follow?
> 2. Is the mouse movement smooth and deliberate, or does it look rushed/lost?
> 3. Are there any moments with dead time or unnecessary pauses I should trim?
> 4. Would a viewer understand what feature is being demonstrated?
> 5. Any suggestions for what I should re-record?

**After assembling the final video with voiceover:**
> This is my finished product demo video for CrackingInterview, a desktop app that helps with coding interviews. Please review the complete video and provide feedback on:
> 1. Pacing — are any sections too fast or too slow?
> 2. Does the voiceover match what's shown on screen?
> 3. Is the flow logical? Would a first-time viewer understand the product?
> 4. Are the captions readable and well-timed?
> 5. How effective is the hook in the first 5 seconds?
> 6. Rate the video 1-10 for a homepage product demo and explain why.
> 7. What one change would improve it the most?

**For the YouTube Ad specifically:**
> This is a 45-second YouTube ad for CrackingInterview. Review it as if you were a marketing professional:
> 1. Does the hook grab attention in the first 3 seconds? (YouTube lets viewers skip at 5s)
> 2. Is the value proposition clear within 10 seconds?
> 3. Is the pacing energetic enough for an ad?
> 4. Is the call-to-action compelling?
> 5. Would you click through after watching this? Why or why not?

#### How to Use ChatGPT (GPT-4o) for Video Review

1. Go to [chatgpt.com](https://chatgpt.com) (requires Plus, $20/mo)
2. Click the **attachment icon** and upload your video
3. Use the same prompts as above — GPT-4o provides more detailed, structured feedback than Gemini in many cases

#### Using Claude for Script Review (Before Recording)

Before you start recording, you can paste the script sections into Claude and ask:
> Review this voiceover script for a product demo video. Is the language clear and concise? Are there any sentences that are too long or awkward to say aloud? Suggest improvements for a professional, confident tone.

---

### Step 7: Publish

#### Homepage Video

1. Upload the final MP4 to **YouTube** (Public or Unlisted)
2. Use the title, description, tags, and thumbnail guide below
3. Copy the YouTube embed code and paste it into your website's homepage HTML
4. Alternatively, host the video directly (e.g., upload to your web hosting or use a service like Vimeo for cleaner embeds without YouTube branding)

**YouTube Title:**

```
CrackingInterview — AI That Solves Coding Interviews in Real Time (Live Demo)
```

**YouTube Description (copy-paste ready):**

```
CrackingInterview is an invisible AI-powered interview coach that sits on your screen and helps you ace coding interviews in real time. Available for Mac and Windows.

🔗 Download free: https://crackinginterview.org

⏱ TIMESTAMPS:
0:04 — What is CrackingInterview?
0:32 — Audio Mode: Real-time verbal interview with AI memory & resume context
2:14 — Text Extraction: Solve LeetCode problems instantly
3:01 — System Design: Auto-generated architecture diagrams
3:52 — Custom Prompts, Resume Upload & AI Model Selection
4:44 — Stealth Mode: Invisible to screen share, always on top, zero focus changes
6:44 — Download & Pricing

🎯 WHAT YOU'LL SEE IN THIS VIDEO:
• Live audio interview demo — the AI listens, transcribes, and answers in real time
• Follow-up questions with full conversation memory (agent context)
• Behavioral questions answered using your uploaded resume
• One-hotkey problem solving from LeetCode, HackerRank, and any browser tab
• System design responses with Mermaid architecture diagrams
• Stealth mode: invisible to Zoom screen share, HackerRank, CodeSignal, and CoderPad
• Always-on-top window with adjustable transparency
• Global hotkeys — control everything without changing browser focus

🛡 STEALTH FEATURES:
• Invisible to screen sharing, screenshots, and screen recordings
• Always-on-top — never drops behind your browser
• Global hotkeys — scroll, move, toggle without leaving the interview platform
• Zero tab switches, zero focus loss — anti-cheat systems detect nothing
• Adjustable transparency — read AI responses while coding through the window

🌐 SUPPORTED LANGUAGES:
Audio mode supports 30+ languages including English, Spanish, French, German, Persian, Chinese, Japanese, Portuguese, and many more.

🤖 AI MODELS:
Works with OpenAI, Google Gemini, and Anthropic Claude. Free tier included — Pro unlocks premium models.

📥 DOWNLOAD:
https://crackinginterview.org
Available on Mac and Windows. Free to start.

#CodingInterview #LeetCode #HackerRank #InterviewPrep #AI #CrackingInterview #SystemDesign #SoftwareEngineering #TechInterview #CodingInterviewHelp
```

**YouTube Tags (paste into the tags field):**

```
cracking interview, coding interview help, AI interview coach, leetcode solver, hackerrank cheating, coding interview AI, system design interview, verbal interview AI, real time interview help, invisible interview app, stealth mode interview, interview screen share invisible, coding interview tool, AI coding assistant, leetcode AI, hackerrank AI, codesignal, coderbyte, software engineer interview, tech interview prep, coding bootcamp, interview anxiety, coding interview tips, AI resume interview, behavioral interview AI
```

---

#### Homepage Video — Thumbnail Design

Create in **Canva** at **1280x720** (YouTube standard). The thumbnail needs to communicate the core value in under 2 seconds.

**Recommended layout — side-by-side stealth proof:**

| Element | Details |
|---------|---------|
| **Background** | Dark gradient (dark blue/black to deep purple) — looks premium and techy |
| **Left side** | Screenshot of your app floating on top of a HackerRank coding environment — the AI response clearly visible. Add a subtle green glow or border around the app window |
| **Right side** | Screenshot of the Zoom screen-share preview showing only HackerRank — no app visible. Add a red "X" or "INVISIBLE" stamp over where the app would be |
| **Divider** | A thin vertical line or "VS" badge between the two sides |
| **Top text (large, bold)** | `AI INTERVIEW COACH` — white or bright green, bold sans-serif (Montserrat or Inter), at least 60pt |
| **Bottom text (smaller)** | `Invisible to Screen Share • Always On Top • Real-Time AI` — white, 24-30pt |
| **Corner badge** | `FREE DOWNLOAD` in a bright green or yellow pill/badge shape — bottom-right corner |
| **App logo** | Small CrackingInterview logo in the top-left or top-right corner |

**Alternative thumbnail — audio mode focus:**

| Element | Details |
|---------|---------|
| **Background** | Screenshot of the Zoom meeting with the fake interviewer on the left, CrackingInterview on the right with an AI response visible |
| **Overlay text (large)** | `IT LISTENS. IT ANSWERS.` — bold white text with dark shadow |
| **Subtitle** | `Real-Time AI for Verbal Interviews` — smaller, below the main text |
| **Audio waveform graphic** | A stylized green/blue waveform across the bottom — signals audio/voice |
| **Corner badge** | `FREE • Mac & Windows` |

**Thumbnail tips:**
- Test both designs — upload one, check the click-through rate in YouTube Studio after a few days, then try the other
- Avoid small text — thumbnails appear tiny on mobile. If you can't read it at phone size, make it bigger
- Use no more than 5-7 words of text total on the thumbnail
- High contrast matters: bright text on dark background performs best on YouTube
- No faces needed since this is a software product — the app screenshot IS the visual hook

---

#### YouTube Ad

1. Upload to YouTube as Public
2. To run as an ad, go to [ads.google.com](https://ads.google.com) and create a Video campaign
3. Campaign type: **Video > Drive conversions**
4. Select your uploaded YouTube video
5. Target audience: software engineers, job seekers, coding bootcamp students
6. Set your daily budget (start with $10-20/day to test)

---

### Quick Reference: Full Workflow Checklist

- [ ] Install OBS Studio
- [ ] Create ElevenLabs account
- [ ] Create Descript account ($24/mo)
- [ ] Create Canva account (free)
- [ ] Clean desktop and prepare demo content
- [ ] Record Section 1 → save as `01-overview.mp4`
- [ ] Record Section 2 → save as `02-audio.mp4`
- [ ] Record Section 3 → save as `03-solve.mp4`
- [ ] Record Section 4 → save as `04-system-design.mp4`
- [ ] Record Section 5 → save as `05-settings.mp4`
- [ ] Record Section 6 → save as `06-stealth.mp4`
- [ ] Record Section 7 → save as `07-cta.mp4`
- [ ] Upload raw recordings to Gemini for feedback → re-record if needed
- [ ] Generate V1 voiceover clips (7 MP3 files) in ElevenLabs
- [ ] Generate V2 voiceover clips (6 MP3 files) in ElevenLabs
- [ ] Create intro + outro cards in Canva → export as MP4
- [ ] Import everything into Descript
- [ ] Assemble Video 1 (Homepage) → add captions + text overlays → export
- [ ] Assemble Video 2 (YouTube Ad) → add music + fast cuts → export
- [ ] Upload final videos to Gemini for review → make adjustments
- [ ] Upload Video 1 to YouTube → embed on crackinginterview.org
- [ ] Upload Video 2 to YouTube → set up Google Ads campaign
- [ ] Create thumbnails in Canva
