# Validation on real screens (milestone J6)

Status: **to do** — synthetic fixtures pass; real-device validation needs captures from a phone.

## Procedure

1. Pick 5 screens of a real app (prefer screens with lists, icons, forms and images).
2. Capture them with `a11y-report` (scan) on the device. Do not commit screenshots containing real
   people's names, photos or messages; anonymize or use a test account.
3. On the same device and screens, run Google's **Accessibility Scanner** and export its results.
4. Compare finding by finding and fill the table below. Every divergence must be explained.

| Screen | Element | a11y-report | Accessibility Scanner | Explanation |
|---|---|---|---|---|
| | | | | |

## Expected differences (to confirm)

- **Touch target**: Accessibility Scanner uses 48 dp for everything; a11y-report reports < 24 dp as
  major (WCAG 2.5.8) and 24–48 dp as minor (Android guideline).
- **Contrast**: Accessibility Scanner reports text contrast from its own colour extraction; ratios
  may differ by a few hundredths. a11y-report truncates ratios (4.499 → 4.49) and reports busy
  backgrounds as `info` / low confidence instead of failing them.
- **Item descriptions**: Accessibility Scanner flags duplicate descriptions with a different
  grouping; a11y-report groups interactive elements by normalized accessible name per screen.
- **Text fields**: missing labels on `EditText` are low confidence here because hints and
  `labelFor` are not in uiautomator dumps.

## Benchmark

`cargo bench -p a11y-report-cli` — 100 synthetic 1080×2400 screens, analysis + JSON + HTML with
thumbnails: **≈ 0.48 s** on the development laptop (target: < 2 s).
