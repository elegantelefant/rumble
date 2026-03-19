# Getting Started with Elefant Rumble

Welcome to Elefant Rumble — your private, on-device legal assistant. Everything runs locally on your computer. Your documents and conversations never leave your machine.

Rumble runs on **macOS**, **Windows**, and **Linux**.

---

## What You Need

- At least 8 GB of RAM (16 GB recommended for larger models)
- About 5 GB of free disk space (for the app + a language model)

### Platform requirements

| Platform | Version |
|----------|---------|
| macOS | 10.15 Catalina or later (Apple Silicon or Intel) |
| Windows | Windows 10 (version 1803) or later |
| Linux | Ubuntu 22.04, Fedora 38, or equivalent (x64) |

---

## Step 1: Install Ollama

Rumble uses Ollama to run AI models privately on your computer. It's free and takes about 2 minutes.

1. Open your web browser and go to **https://ollama.com/download**
2. Download the installer for your platform
3. Install and run Ollama

### macOS
Download the .dmg, open it, and drag Ollama to your Applications folder. Open Ollama — a small llama icon appears in your menu bar.

### Windows
Download the .exe installer and run it. Ollama runs in the background — look for the llama icon in the system tray (bottom-right of your screen).

### Linux
Open a terminal and run:
```
curl -fsSL https://ollama.com/install.sh | sh
```
Then start the service: `ollama serve`

---

## Step 2: Download a Language Model

Rumble needs a language model to work. Open a terminal and run:

```
ollama pull llama3.2
```

**How to open a terminal:**
- **macOS:** Search for "Terminal" in Spotlight (Cmd+Space)
- **Windows:** Search for "PowerShell" in the Start menu
- **Linux:** Ctrl+Alt+T or search for "Terminal" in your app launcher

Wait for the download to finish (~2 GB). You can close the terminal when done.

---

## Step 3: Install Elefant Rumble

Download the installer for your platform from the link you were sent.

### macOS
1. Open the `.dmg` file
2. Drag **Elefant - Rumble** into your **Applications** folder
3. Open the app from Applications

**If macOS says the app can't be opened:**
This happens because the app isn't signed with Apple yet (coming in a future release).
1. Right-click (or Control-click) on **Elefant - Rumble** in Applications
2. Click **Open** from the menu
3. In the dialog that appears, click **Open** again

You only need to do this once.

### Windows
1. Run the `.msi` or `.exe` installer
2. Follow the prompts — the app installs to your Programs folder
3. Open **Elefant - Rumble** from the Start menu

**If Windows Defender shows a warning:**
Click "More info" then "Run anyway". This happens because the app isn't code-signed yet.

### Linux

**Ubuntu/Debian (.deb):**
```
sudo dpkg -i elefant-rumble_0.1.0_amd64.deb
```

**AppImage (any distro):**
```
chmod +x Elefant-Rumble_0.1.0_amd64.AppImage
./Elefant-Rumble_0.1.0_amd64.AppImage
```

---

## Step 4: Open the App

1. Make sure Ollama is running:
   - **macOS:** Look for the llama icon in your menu bar. If missing, open Ollama from Applications.
   - **Windows:** Look for the llama icon in the system tray. If missing, open Ollama from the Start menu.
   - **Linux:** Run `ollama serve` in a terminal if it isn't already running.
2. Open **Elefant - Rumble**
3. The app window should appear. A tray icon also appears in your menu bar / system tray.

---

## Your First 5 Minutes

Here's a quick walkthrough to see Rumble in action:

### Upload and review a document
1. Click **Document Review** in the sidebar
2. Click **Browse Files** and pick any PDF, DOCX, or TXT file
3. Rumble will analyze the document and produce an initial summary
4. Type a follow-up question in the chat box — e.g. "What are the key obligations?"

### Draft a document
1. Click **Document Draft** in the sidebar
2. Pick a template (try "Non-Disclosure Agreement")
3. Fill in the party names and duration
4. Click **Generate Draft**

### Try a research question
1. Click **Research** in the sidebar
2. Type a question in the prompt area — e.g. "What are the elements of negligence in common law?"
3. Click **Start Research**
4. Rumble will produce a structured answer with citations

---

## Using the App

Rumble has five main tools, accessible from the sidebar on the left:

### Document Review
Upload a PDF, DOCX, or TXT file by dragging it into the app or clicking **Browse Files**. Rumble will analyze the document and let you ask follow-up questions about it — like having a conversation about the document's contents.

### Research Assistant
Ask a legal research question and Rumble will produce a structured analysis with citations. Each research session is saved as a thread you can return to later.

### Document Draft
Choose a template (Employment Agreement, NDA, or Service Contract), fill in the details, and generate a first draft. You can export to Word or PDF.

### Translation
Paste text in one language and translate it to another. Rumble supports English, French, German, Spanish, and Chinese. These are draft-quality translations — always have them reviewed by a certified translator before filing.

### Settings
Configure AI providers, manage templates, and adjust how the app looks and feels. By default, everything runs through Ollama on your computer.

---

## Tips

- **Everything is private.** Your documents and conversations stay on your machine. Nothing is sent to the cloud unless you explicitly configure a hosted AI provider in Settings.
- **Keyboard shortcuts:** Use Cmd+K (macOS) or Ctrl+K (Windows/Linux) to open the shortcut palette for quick navigation.
- **First response is slow?** That's normal — Ollama loads the model into memory on first use (~30 seconds). After that, responses take a few seconds.
- **Want faster or better responses?** In Settings, add an OpenAI or Anthropic API key under "Providers & API keys". Hosted models are faster but send data to their servers.

---

## Troubleshooting

| What's happening | What to do |
|---|---|
| App says "sidecar not running" | Make sure Ollama is running (check for the llama icon in your menu bar / system tray) |
| App won't open (macOS) | Right-click the app → Open → click Open in the dialog |
| App won't open (Windows) | Click "More info" → "Run anyway" on the Defender warning |
| No models show up | Open a terminal and run `ollama pull llama3.2` |
| Responses are very slow | Normal for the first message. If it stays slow, try a smaller model: `ollama pull llama3.2:1b` |
| App crashes on launch | Restart your computer and try again. If it persists, check system logs for errors mentioning "elefant" |

---

## Uninstalling

### macOS
Drag **Elefant - Rumble** from Applications to the Trash. To also remove data: delete `~/Library/Application Support/com.ielegante.rumble/`

### Windows
Open Settings → Apps → find "Elefant - Rumble" → click Uninstall

### Linux
**Debian/Ubuntu:** `sudo apt remove elefant-rumble`
**AppImage:** Delete the AppImage file

To remove Ollama and models separately, see https://ollama.com/docs/uninstall

---

## Getting Help

If something isn't working, please share:
1. What you were doing when the problem occurred
2. Any error messages you saw
3. Your operating system and version

Send this information to your Elefant contact and we'll help you get sorted.
