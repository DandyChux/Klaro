<p align="center">
  <img src="public/vite.svg" width="80" alt="Klaro Logo" />
</p>

<h1 align="center">Klaro</h1>

<p align="center">
  <strong>A privacy-focused desktop app that detects and scrubs Personally Identifiable Information (PII) from your files — entirely offline.</strong>
</p>

<p align="center">
  Built with <a href="https://tauri.app">Tauri v2</a> · <a href="https://svelte.dev">Svelte 5</a> · <a href="https://www.rust-lang.org">Rust</a>
</p>

---

## What It Does

Klaro scans CSV, Excel (`.xlsx`, `.xls`), text, and JSON files for sensitive data like emails, phone numbers, credit card numbers, and names — then replaces them using your chosen scrubbing method. **All processing happens locally on your machine.** Nothing is sent to the cloud.

### Supported PII Types

| PII Type             | Detection Method     |
|----------------------|----------------------|
| Email Address        | Regex                |
| Phone Number         | Regex                |
| Social Security No.  | Regex + validation   |
| Credit Card Number   | Regex + Luhn check   |
| IP Address           | Regex                |
| Date of Birth        | Regex                |
| Street Address       | NER                  |
| Person Name          | NER                  |
| Passport Number      | Regex                |
| Driver's License     | Regex                |
| Bank Account Number  | Regex                |
| Organization Name    | NER                  |

### Scrubbing Methods

- **Mask** — Partially redact values (e.g. `j***@example.com`, `***-***-1234`)
- **Remove** — Replace with a labeled placeholder (e.g. `[EMAIL REMOVED]`)
- **Hash** — One-way SHA-256 hash (e.g. `#a1b2c3d4e5f6`)
- **Fake** — Generate realistic-looking replacement data using the [fake](https://crates.io/crates/fake) crate

## AI-Powered Name Detection

Klaro bundles a **DistilBERT NER model** (~261 MB) from Hugging Face ([elastic/distilbert-base-cased-finetuned-conll03-english](https://huggingface.co/elastic/distilbert-base-cased-finetuned-conll03-english)) to detect person names, organizations, and locations that regex alone can't catch. Inference runs via [Candle](https://github.com/huggingface/candle) with **Metal GPU acceleration on Apple Silicon** and CPU on other platforms.

## Supported File Formats

| Format | Extension       |
|--------|-----------------|
| CSV    | `.csv`          |
| Excel  | `.xlsx`, `.xls` |
| Text   | `.txt`          |
| JSON   | `.json`         |

## Lite vs Pro

Klaro ships in two tiers:

| Feature              | Lite                             | Pro           |
|----------------------|----------------------------------|---------------|
| Max file size        | 5 MB                             | Unlimited     |
| Max rows per file    | 10,000                           | Unlimited     |
| Max files per batch  | 3                                | Unlimited     |
| PII types            | Email, Phone, SSN, Credit Card   | All 12 types  |
| Batch download       | ✗                                | ✓             |
| XLSX export          | ✗                                | ✓             |

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) (2024 edition)
- [Bun](https://bun.sh/) (or Node.js)
- Platform dependencies for Tauri v2 — see the [Tauri prerequisites guide](https://v2.tauri.app/start/prerequisites/)

### Install Dependencies

```sh
bun install
```

### Download the NER Model

```sh
chmod +x download_model.sh
./download_model.sh
```

This downloads ~261 MB of model weights into `src-tauri/resources/models/ner/`.

### Run in Development

```sh
bun tauri dev
```

### Build for Production

```sh
# Pro (all features, NER, unlimited limits)
bun tauri build

# Lite (limited PII types and file size caps)
bun tauri build -- --no-default-features --features lite-release
```

## Architecture

```
Klaro/
├── src/                    # Svelte 5 frontend (TypeScript + Tailwind CSS)
│   ├── App.svelte          # Main app — stepper UI (Upload → Configure → Results)
│   └── lib/components/     # File upload, PII options, scrub method, results panel
├── src-tauri/src/          # Rust backend
│   ├── main.rs             # Tauri setup, NER initialization
│   ├── commands.rs         # Tauri command handlers (process_file, etc.)
│   ├── pii_detector.rs     # Regex + NER-based PII detection
│   ├── scrubber.rs         # Mask / Remove / Hash / Fake scrubbing
│   ├── ner.rs              # DistilBERT NER model (Candle + Metal)
│   ├── file_parser.rs      # CSV, XLSX, XLS, TXT, JSON parsing
│   ├── streaming.rs        # Streaming processor for large files
│   └── license.rs          # Lite/Pro feature gating
└── .github/workflows/      # CI: cross-platform builds (macOS, Linux, Windows)
```

### How Processing Works

1. **Upload** — Files are read as base64 in the frontend and sent to the Rust backend via Tauri commands.
2. **Parse** — The backend detects the file type and parses it into tabular rows or raw text.
3. **Detect** — Each cell/line is scanned with regex patterns. If NER-based PII types are selected (names, addresses, organizations), batches are run through the DistilBERT model.
4. **Scrub** — Detected PII matches are replaced in-place using the selected scrubbing method.
5. **Stream back** — Progress events are emitted to the frontend in real-time. For large files, a streaming processor handles data in chunks to keep memory usage low.
6. **Download** — Scrubbed files are saved via the Tauri file dialog.

## Tech Stack

| Layer             | Technology                                                                             |
|-------------------|----------------------------------------------------------------------------------------|
| Framework         | [Tauri v2](https://tauri.app)                                                          |
| Frontend          | [Svelte 5](https://svelte.dev) + TypeScript                                           |
| Styling           | [Tailwind CSS v4](https://tailwindcss.com) + [shadcn-svelte](https://shadcn-svelte.com) |
| Backend           | Rust (2024 edition)                                                                    |
| ML Inference      | [Candle](https://github.com/huggingface/candle) (DistilBERT)                          |
| File Parsing      | [csv](https://crates.io/crates/csv), [calamine](https://crates.io/crates/calamine)    |
| Package Manager   | [Bun](https://bun.sh)                                                                 |

## CI/CD

GitHub Actions builds both Lite and Pro versions for:

- **macOS** — ARM64 + x86_64
- **Linux** — x64
- **Windows** — x64

Releases are created as drafts on tag pushes (`v*`), or triggered manually via workflow dispatch.

## Recommended IDE Setup

[VS Code](https://code.visualstudio.com/) + [Svelte](https://marketplace.visualstudio.com/items?itemName=svelte.svelte-vscode) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## License

MIT — Chukwuma Okoroji