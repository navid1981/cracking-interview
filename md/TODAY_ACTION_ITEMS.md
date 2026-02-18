# ✅ TODAY'S ACTION ITEMS - Before LLC Approval

**Priority tasks to complete while waiting for LLC approval.**

---

## 🔥 Critical (Do First)

### 1. Deploy Supabase Edge Functions to Production
**Time: 15 minutes**

```bash
cd /Users/nsalehvaziri/cracking-interview

# Link your project (if not already)
supabase link --project-ref uudwpcjxbwtszhhcgybj

# Deploy all functions
supabase functions deploy ai-proxy
supabase functions deploy create-checkout
supabase functions deploy create-billing-portal
supabase functions deploy notification
supabase functions deploy stripe-webhook

# Verify deployment
supabase functions list
```

**Expected result:** All 5 functions show as deployed

---

### 2. Update Stripe Webhook URL
**Time: 5 minutes**

1. Go to: https://dashboard.stripe.com/webhooks
2. Find your webhook endpoint
3. Click "..." → Update details
4. Change URL from:
   ```
   https://uudwpcjxbwtszhhcgybj.supabase.co/functions/v1/stripe-webhook-test
   ```
   To:
   ```
   https://uudwpcjxbwtszhhcgybj.supabase.co/functions/v1/stripe-webhook
   ```
5. Save changes
6. Verify these events are enabled:
   - ✅ `customer.subscription.created`
   - ✅ `customer.subscription.updated`
   - ✅ `customer.subscription.deleted`
   - ✅ `invoice.payment_succeeded`
   - ✅ `invoice.payment_failed`

---

### 3. Test App in Development Mode
**Time: 30 minutes**

```bash
npm run tauri dev
```

**Test checklist:**
```
□ Sign up new user
□ Sign in existing user
□ Free user: 3 AI calls work
□ Paywall appears after 3 calls
□ Stripe checkout opens
□ Test subscription (use test card: 4242 4242 4242 4242)
□ Pro user: Can make AI calls
□ All 6 prompts work
□ Text input works
□ Screenshot capture works
□ Audio recording works (Command+3)
□ Chrome CDP connects
□ Stealth mode works (Command+Shift+H)
□ All global hotkeys work
□ Announcement appears after login
□ Announcement dismisses
□ Settings save correctly
```

**If anything fails, fix before proceeding!**

---

## 🎯 Important (Do Today)

### 4. Verify Stripe Configuration
**Time: 10 minutes**

**Check if you're in Test or Production mode:**
1. Go to: https://dashboard.stripe.com
2. Look at toggle in top-right corner
3. Switch to **Production** mode (if releasing to real users)

**Get Production Keys** (if using production):
1. Go to: https://dashboard.stripe.com/apikeys
2. Reveal "Secret key" (starts with `sk_live_...`)
3. Copy it

**Update Supabase Edge Function Secrets:**
1. Go to Supabase Dashboard: https://supabase.com/dashboard/project/uudwpcjxbwtszhhcgybj/settings/functions
2. Add/Update secrets:
   ```
   STRIPE_SECRET_KEY = sk_live_... (your production key)
   STRIPE_WEBHOOK_SECRET = whsec_... (from webhook endpoint)
   ```

**Create Production Subscription Product:**
1. Dashboard → Products → Add Product
2. Name: "CrackingInterview Pro"
3. Price: $29/month (or your price)
4. Copy **Price ID** (starts with `price_...`)
5. Update in your code where price ID is used

---

### 5. Test Build Process
**Time: 5 minutes**

```bash
# Clean previous builds
rm -rf src-tauri/target/release

# Try building (will fail at signing - expected)
npm run tauri build
```

**Expected result:**
- Vite builds successfully
- Rust compiles without errors
- Fails at code signing (no certificate yet)

**If build fails before signing step:**
- Fix compilation errors
- Check dependencies
- Verify icons exist

---

### 6. Take Screenshots
**Time: 20 minutes**

Launch app and take screenshots of:
1. Main interface (status screen)
2. Settings → Prompts
3. Settings → AI Config
4. Settings → HotKeys
5. AI response with code syntax highlighting
6. Subscription page

**Use for:**
- GitHub release
- Website/landing page
- Social media
- Product Hunt

**Save to:** `./screenshots/` directory

---

## 📝 Nice to Have (If Time)

### 7. Prepare Marketing Copy
**Time: 30 minutes**

Write short descriptions:

**One-liner (for social media):**
```
CrackingInterview - AI-powered coding interview assistant with stealth mode
```

**Short (for GitHub):**
```
Practice coding interviews with AI assistance. Features stealth mode, 
multiple input methods (text/screenshot/audio), and support for major 
AI models. Perfect for LeetCode, system design, and technical interviews.
```

**Full (for website):**
- See `./md/RELEASE_NOTES_v1.0.0.md` for inspiration

---

### 8. Set Up Support Email
**Time: 15 minutes**

**Option 1: Email Forwarding (Free)**
1. Go to your domain registrar
2. Set up forwarding: support@crackinginterview.org → your-personal-email

**Option 2: Google Workspace ($6/month)**
1. Sign up: https://workspace.google.com
2. Add domain: crackinginterview.org
3. Create: support@crackinginterview.org

**Option 3: Zoho Mail (Free for 5 users)**
1. Sign up: https://www.zoho.com/mail/
2. Add domain
3. Create support email

---

### 9. Create Landing Page (If you have a domain)
**Time: 1-2 hours**

**Quick options:**

**Option A: GitHub Pages (Free)**
```bash
# In your repo
mkdir docs
cd docs
# Create index.html with download link
git add .
git commit -m "Add landing page"
git push

# Enable GitHub Pages in repo settings
```

**Option B: Carrd (Free/Paid)**
- https://carrd.co
- Drag-and-drop builder
- Custom domain support

**Option C: Simple HTML**
Create `landing.html` with:
- Hero section with app name
- Key features (3-5 bullets)
- Screenshots
- Download button
- Pricing
- Support email

---

## 📊 Progress Tracking

**Mark as done when completed:**

### Critical (Required)
- [ ] Edge functions deployed ⚠️
- [ ] Stripe webhook updated ⚠️
- [ ] App tested in dev mode ⚠️

### Important (Highly Recommended)
- [ ] Stripe production configured
- [ ] Build process verified
- [ ] Screenshots taken

### Nice to Have
- [ ] Marketing copy written
- [ ] Support email set up
- [ ] Landing page created

---

## ⏰ Time Estimate

**Critical tasks:** 50 minutes
**Important tasks:** 35 minutes  
**Nice to have:** 2-3 hours

**Minimum required:** ~1 hour for critical tasks

---

## 🚨 Blockers to Watch For

### Problem: Supabase CLI not linked
**Solution:**
```bash
supabase link --project-ref uudwpcjxbwtszhhcgybj
# Enter your access token from Supabase Dashboard
```

### Problem: Edge function deployment fails
**Solution:**
- Check you're logged in: `supabase login`
- Check function has no syntax errors
- Try deploying via Supabase Dashboard instead

### Problem: Stripe test card doesn't work
**Solution:**
- Use: 4242 4242 4242 4242
- Any future expiry
- Any CVC
- If still fails, check Stripe is in correct mode (test/production)

### Problem: Build fails with Rust errors
**Solution:**
```bash
cd src-tauri
cargo clean
cargo build --release
# Check error messages
```

---

## 🎯 When You're Done

Once critical tasks are complete:
1. ✅ Your app is ready for production
2. ✅ Just waiting for LLC approval
3. ✅ When approved: Follow `./md/LLC_APPROVAL_GUIDE.md`
4. ✅ Build → Notarize → Ship! (~30 min)

---

## 📞 Quick Help

**Stuck on Supabase?**
- Docs: https://supabase.com/docs/guides/functions
- Dashboard: https://supabase.com/dashboard

**Stuck on Stripe?**
- Docs: https://stripe.com/docs/webhooks
- Test mode: Always test first!

**Questions?**
- Check: `./md/COMPLETE_RELEASE_ROADMAP.md` for detailed guide
- Check: `./md/QUICK_REFERENCE.md` for commands

---

**🚀 Get started now! The sooner you complete these, the faster you can ship when LLC is approved!**

