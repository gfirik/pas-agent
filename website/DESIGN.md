# PAS-Agent Design System

The design system for the PAS-Agent website (`website/`). Tokens live in
`website/src/styles/global.css`; if this document and the CSS disagree, the CSS wins and
this document should be fixed.

## 1. Identity

PAS-Agent should feel like a quiet, confident piece of developer infrastructure: a
well-documented open-source tool that respects your time.

- **It is:** precise, local, calm, technical. The product speaks through its utility.
  Think of a well-written README, not a sales deck.
- **It is not:** an AI startup landing page, a SaaS funnel, a robot showcase, or a template.
- **Visual character:** warm paper, ink-black type, hairline rules, one amber accent,
  generous negative space.
- **Tone:** "This solves a real problem", never "This will change everything".

## 2. Principles

1. **Typography is the primary visual system.** Size, weight, and spacing do the work.
2. **Restraint.** If an element can be removed and the page still works, remove it.
3. **Precision.** Alignment and spacing are exact; hairlines replace shadows.
4. **Technical details are visual language.** Terminal prompts, file paths, and real
   `session.json` fragments are the imagery.
5. **Honesty.** Every command, flag, and file shown on the site must exist in the CLI.
   Unbuilt features are labelled as coming next, never shown as working.
6. **Motion serves comprehension**, and the page is complete without it.
7. **Local-first, even for the site:** no third-party requests at runtime.

## 3. Color

| Token | Value | Use |
|---|---|---|
| `--color-page-cream` | `#faf9f6` | Page background, card surfaces |
| `--color-ink-black` | `#111111` | Primary text, primary buttons, code windows |
| `--color-warm-stone` | `#585858` | Secondary text, inactive nav and TOC links (6.8:1) |
| `--color-rule-black` | `#dedbd6` | Hairline borders, the vertical page rules |
| `--color-linen-beige` | `#f1eee9` | Pills, inline code, hover washes |
| `--color-ash-gray` | `#a0a0a0` | Decorative ticks; dim text **on dark only** |
| `--color-amber-signal` | `#fcaa2d` | Accent fills: highlighter marks, active borders, commands on dark |
| `--color-amber-ink` | `#9a5b00` | Amber **text** on light backgrounds (5.1:1) |

Rules:

- Amber is rationed: at most one accent word per heading, plus active states.
- Never set `--color-amber-signal` as text on cream (1.8:1). Use `.color-accent`
  (amber-ink) for text, `.mark` (highlighter stroke) for heading words, and
  `.color-accent-on-dark` inside code windows.
- Body text is never lighter than `--color-warm-stone` on cream.

## 4. Typography

- **Sans:** Instrument Sans 400 / 500 / 600
- **Mono:** JetBrains Mono 400, for UI chrome, labels, buttons, and code. Ligatures are off, so
  `-->` renders as typed.
- Both fonts are self-hosted through `@fontsource`, imported in `components/Head.astro`.

| Class | Size / line height | Tracking | Use |
|---|---|---|---|
| `.text-display` | 64 / 1.1 (40 on mobile) | -3.2px | Hero headline |
| `.text-heading-lg` | 40 / 1.1 (32 on mobile) | -1.6px | Section headings |
| `.text-subheading` | 20 / 1.06, weight 500 | -0.6px | Card and step titles |
| `.text-body` | 18 / 1.5 | -0.72px | Lead paragraphs |
| `.text-body-sm` | 16 / 1.5 | -0.64px | Body copy |
| `.text-mono-sm` | 12 / 1.2 | +0.36px | Buttons, pills, terminal |
| `.text-mono-xs` | 10 / 1.2, uppercase for labels | +0.4px | Eyebrows, step numbers |

Headings are weight 400; hierarchy comes from size and tight tracking, not boldness.
Prose is capped at 640px (`.max-w-prose`); docs articles at 760px.

## 5. Layout

- **Landing page:** `.container` is 1200px max with 80px side padding (20px under 768px), with
  1px vertical rules on both sides that run the full page height. Sections are separated by
  full-width hairlines, with 96px vertical padding (56px on mobile).
- **Docs:** three columns (220px sidebar | content | 180px TOC) inside a 1440px frame.
  The TOC hides under 1100px, and the sidebar becomes a toggle menu under 768px.
- **Radii:** 4px for boxes and buttons, 2px for small chips, pill (800px) for agent tags only.
- **Elevation:** none. Separation comes from hairlines and surface color, never shadows.

## 6. Components

- **Buttons:** `.btn` is mono 12px with 14×16 padding and a 4px radius. `.btn-primary` is ink on cream;
  `.btn-ghost` has a 1px ink border. Hover lowers opacity; focus shows a 2px amber-ink outline.
- **Code window:** ink background, three muted dots, a hairline header, and an optional file label.
  Used for the terminal demo and `session.json` snippets. The terminal transcript is rendered
  in the HTML, and JavaScript only re-types it.
- **Landscape box:** a photograph with a light sepia filter behind a code window. This is the one
  place photography is used, always self-hosted in `public/images/`.
- **Agent pills:** plain text names in mono on linen. No third-party logos.
- **Scrollytelling:** a sticky code window on the left with numbered steps on the right; each
  step carries its snippet in `data-code`. Stacks on mobile.
- **FAQ:** native `<details>` with a CSS-only +/− icon.
- **Docs pagination:** bordered previous/next cards at the end of each article.

## 7. Motion

- **Entrance:** `.anim` fades up 16px over 500ms using `cubic-bezier(0.16, 1, 0.3, 1)`, with
  `.anim-delay-1..3` staggered by 100ms. Each element animates once.
- **Intro curtain (landing page, first visit only):** "PORTABLE / AGENT / SESSIONS" streams in
  as large mono caps with amber-ink initials. The other letters fade out and the initials settle
  into "P A S" (a FLIP transition), then the cream panel lifts like a curtain, about 2.6s in total.
  Any key, click, or tap skips it. It never plays with reduced motion or without JavaScript, a 5s
  failsafe prevents it from ever blocking the page, and the hero's entrance waits for it. It's
  remembered in `localStorage` (`pas-intro-seen`); append `?intro` to replay it.
- **Hover:** 120–200ms changes to color, opacity, or border only.
- **No:** parallax, floating or looping decoration, 3D, bouncing.
- **Without JavaScript:** `.anim` only hides content when `<html>` has the `js` class, so
  everything is visible without scripts.
- **Reduced motion:** entrance animations, the typing effect, and scrolly transitions are
  disabled, and all content is shown immediately.

## 8. Accessibility

- WCAG AA contrast for all text (see the ratios in §3).
- A skip link on every page, semantic landmarks, and one `<h1>` per page with headings in order.
- Visible `:focus-visible` outlines. The mobile docs menu uses `aria-expanded` and
  `aria-controls`, and Escape closes it.
- Decorative elements (dots, ticks, logo mark) are `aria-hidden`.

## 9. What We Don't Use

Gradients and glow, orbs, robots or brain imagery, glassmorphism, heavy shadows, stock photos
of people or tech, third-party logos, fake metrics or testimonials, pricing tables,
newsletter pop-ups, chat widgets, cookie banners, emoji decoration, or copy that
promises features the CLI doesn't have.

## 10. Checklist Before Shipping a Page

1. Does every command, flag, and file shown exist in the current CLI?
2. Is the page readable and complete with JavaScript off and with reduced motion on?
3. Does all text pass AA contrast?
4. Does it hold up at 390px wide?
5. Does it make any third-party request? (It shouldn't.)
6. Is anything there only because "landing pages usually have it"?
