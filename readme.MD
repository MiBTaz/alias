---

# alias.exe

**The fast, safe, and modern alias manager for Windows CMD.**

`alias` is a 283KB, statically-linked Rust utility designed to replace messy batch files and manual `doskey` entries. It manages your macros in a single text file, handles environment variables, and ensures your shell tools are always responsive.

---

## ✨ Features

* **Static & Tiny:** A single `283KB` binary. No DLLs, no runtimes.
* **Safety First:** Prevents dangerous flag collisions (like accidentally aliasing `-f`).
* **Deep Diagnostics:** Integrated `--which` mode checks drive health and registry sync.
* **Live Reload:** Instantly syncs your disk file to your current shell session.
* **Power-User Friendly:** Built-in logic for `%EDITOR%` and `%ALIAS_FILE%` resolution.

---

## 🚀 Quick Start

1. Place `alias.exe` in your `\bin\` folder.
2. Set your alias file location (e.g., in your profile or via `setx`):
```cmd
set ALIAS_FILE=V:\lbin\aliases

```


3. **Bootstrap your shell:**
```cmd
alias alias="v:\lbin\alias.exe" $*
alias aliases=alias $*
alias --reload

```



---

## 📂 Usage

| Command | Description |
| --- | --- |
| `alias` | List all current macros |
| `alias name=value` | Create/Update an alias |
| `alias name=` | Delete an alias |
| `alias --reload` | Sync the shell with your `ALIAS_FILE` |
| `alias --edalias` | Open your aliases in your `%EDITOR%` |
| `alias --which` | Run system diagnostics (checks drive latency) |

---

## ⚡ Power Integration: Navigating with `ncd`

If you use a custom navigator (like `ncd.exe`) for glob-matching and path jumps, use this specific pattern to handle the Windows sub-shell correctly.

> **Note:** The quoting around the executable path is critical for resolving virtual (`subst`) drives.

```cmd
alias cd=FOR /F "tokens=*" %i IN ('"v:\lbin\ncd.exe" $*') DO @(set "OLDPWD=%CD%" & chdir /d "%i")

```

### 👻 Troubleshooting: "The system cannot find the drive specified"

If you encounter this error while using `cd` on virtual or `subst` drives:

1. **Quote the Path:** Ensure the executable path is quoted inside the `FOR` loop: `'"v:\bin\tool.exe"'`.
2. **Check Latency:** Run `alias --which`. If a drive is marked as **HUNG** or **TIMEOUT**, the sub-shell spawned by the `FOR` loop may fail to resolve the drive mapping.

---

## ⚙️ Technical Details

* **Language:** Rust (Zero-cost abstractions, memory safe).
* **Memory Management:** Strictly scoped; no segfaults or leaks.
* **Performance:** Optimized for high-frequency CLI use.

---

## 📜 License

MIT License - Copyright (c) 2025.

---
