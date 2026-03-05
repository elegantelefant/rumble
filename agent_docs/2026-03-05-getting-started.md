# Getting Started with Elefant Ivory

Welcome to Elefant Ivory — your private, on-device legal assistant. Everything runs locally on your Mac. Your documents and conversations never leave your computer.

---

## What You Need

- A Mac with Apple Silicon (M1, M2, M3, or M4)
- macOS 14 (Sonoma) or later
- At least 8 GB of RAM (16 GB recommended for larger models)
- About 5 GB of free disk space (for the app + a language model)

---

## Step 1: Install Ollama

Ivory uses Ollama to run AI models privately on your Mac. It's free and takes about 2 minutes.

1. Open your web browser and go to **https://ollama.com/download**
2. Click **Download for macOS**
3. Open the downloaded file and drag Ollama to your Applications folder
4. Open Ollama from your Applications folder — you'll see a small llama icon appear in your menu bar (top-right of your screen)

That's it — Ollama runs quietly in the background.

---

## Step 2: Download a Language Model

Ivory needs a language model to work. Open the **Terminal** app (search for "Terminal" in Spotlight with Cmd+Space) and paste this command:

```
ollama pull llama3.2
```

Press Enter and wait for the download to finish. This is about 2 GB and may take a few minutes depending on your internet speed.

You can close Terminal when it's done.

---

## Step 3: Install Elefant Ivory

1. Download `Elefant-Ivory-0.1.0-aarch64.zip` from the link you were given
2. Double-click the zip file to unzip it
3. Drag **Elefant - Ivory** into your **Applications** folder

### If macOS says the app can't be opened

This happens because the app isn't signed with Apple yet (it will be in a future release). To open it:

1. Right-click (or Control-click) on **Elefant - Ivory** in Applications
2. Click **Open** from the menu
3. In the dialog that appears, click **Open** again

You only need to do this once. After that, the app opens normally.

---

## Step 4: Open the App

1. Make sure you see the Ollama icon (llama) in your menu bar. If not, open Ollama from Applications first.
2. Open **Elefant - Ivory** from your Applications folder
3. The app window should appear with "Elefant - Ivory" in the title bar
4. A small icon also appears in your menu bar

---

## Using the App

Ivory has five main tools, accessible from the sidebar on the left:

### Document Review
Upload a PDF, DOCX, or TXT file by dragging it into the app or clicking **Browse Files**. Ivory will analyze the document and let you ask follow-up questions about it — like having a conversation about the document's contents.

### Research Assistant
Ask a legal research question and Ivory will produce a structured analysis with citations. Each research session is saved as a thread you can return to later.

### Document Draft
Choose a template (Employment Agreement, NDA, or Service Contract), fill in the details, and generate a first draft. You can export to Word or PDF.

### Translation
Paste text in one language and translate it to another. Ivory supports English, French, German, Spanish, and Chinese. These are draft-quality translations — always have them reviewed by a certified translator before filing.

### Settings
Configure AI providers, manage templates, and adjust how the app looks and feels. By default, everything runs through Ollama on your computer.

---

## Tips

- **Everything is private.** Your documents and conversations stay on your Mac. Nothing is sent to the cloud unless you explicitly configure a hosted AI provider in Settings.
- **Click any toast notification** (the small messages that appear at the bottom-right) to dismiss it early.
- **Use Cmd+K** to open the shortcut palette for quick navigation.
- **First response is slow?** That's normal — Ollama needs to load the model into memory the first time. Subsequent responses are faster.

---

## Troubleshooting

| What's happening | What to do |
|---|---|
| App says "sidecar not running" | Make sure Ollama is running (look for the llama icon in your menu bar) |
| App won't open at all | Right-click the app → Open → click Open in the dialog |
| No models show up | Open Terminal and run `ollama pull llama3.2` |
| Responses are very slow | This is normal for the first message. If it stays slow, try a smaller model: `ollama pull llama3.2:1b` |
| App crashes on launch | Restart your Mac and try again. If it persists, check Console.app for errors mentioning "elefant" |

---

## Getting Help

If something isn't working, please share:
1. What you were doing when the problem occurred
2. Any error messages you saw
3. Your macOS version (Apple menu → About This Mac)

Send this information to your Elefant contact and we'll help you get sorted.
