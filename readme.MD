---

# alias.exe

**The fast, safe, and modern alias manager for Windows CMD.**

`alias` is a suite of tiny, statically-linked Rust utilities designed to replace messy batch files and manual `doskey` entries. It uses a **Triple-Audit Hybrid Engine** to ensure your shell macros are consistent across the Win32 Kernel, the active process, and the disk.

---

## 🏗 The Multi-Binary Suite

When you compile the project, three distinct binaries are produced to suit different system needs:

| Binary | Layer | Best For... |
| --- | --- | --- |
| `alias.exe` | **Hybrid** | **Daily Use.** The primary orchestrator that audits RAM, Process, and Disk simultaneously. |
| `alias_win32.exe` | **Kernel** | Restricted environments where only direct `kernel32.dll` access is reliable. |
| `alias_wrapper.exe` | **Process** | Compatibility mode for systems where `doskey.exe` behavior must be strictly mimicked. |

---

## ✨ Features

* **Hybrid Sync:** Triple-verifies aliases between **Win32 RAM**, **Doskey Process**, and your **Macro File**.
* **Static & Tiny:** ~283KB binaries. No DLLs, no runtimes, zero-cost abstractions.
* **Deep Diagnostics:** Integrated `--which` mode checks drive health, registry sync, and drive latency.
* **Resilient Memory:** **Loop & Grow** buffer management prevents crashes during high-density or international (UTF-16) alias reads.
* **Safety First:** Prevents dangerous flag collisions and validates naming conventions.
* **Live Reload:** Instantly pushes your disk file into the current shell session.

---

## 🛠 Technical Architecture: The Triple-Audit

To maintain "Intellectual Honesty" in your shell environment, the Hybrid Engine queries three distinct layers to find desyncs:

1. **Win32 RAM (Kernel)**: Direct `kernel32.dll` calls (`GetConsoleAliasesW`) read exactly what the Windows console subsystem sees.
2. **Doskey Wrapper (Process)**: A secondary check that queries the `doskey.exe` process to ensure the wrapper layer hasn't desynced.
3. **Local Disk (Persistence)**: Resolves your `.doskey` macro file to ensure survival across reboots.

If a layer fails (e.g., Access Denied to Kernel memory), the system **Screams** (captures the OS Error Code) and **Shouts** (reports the specific failure) but continues the audit using the remaining healthy sources.

---

## 🚀 Quick Start

1. Place your preferred `alias.exe` in your `\bin\` folder.
2. Set your alias file location (e.g., via `setx` or in your profile):
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
| `alias` | List all current macros (runs Triple-Audit) |
| `alias name=value` | Create/Update an alias |
| `alias name=` | Delete an alias |
| `alias --reload` | Sync the shell with your `ALIAS_FILE` |
| `alias --edalias` | Open your aliases in your `%EDITOR%` |
| `alias --which` | Run system diagnostics (checks drive health/latency) |

---

## ⚡ Power User: Sub-shell Navigation (`ncd`)

If you use a custom navigator (like `ncd.exe`) for path jumps, use this specific pattern to handle the Windows sub-shell correctly. The quoting around the executable path is critical for resolving virtual (`subst`) drives.

Note: due to cmd quoting difficulties, the best method to store this macro is with alias --edalias The code below is the unquoted, ready to copy&paste into your aliases version.  

```cmd
cd=FOR /F "tokens=*" %i IN ('"v:\lbin\ncd.exe" $*') DO @(set "OLDPWD=%CD%" & chdir /d "%i")

```

### 👻 Troubleshooting: "The system cannot find the drive specified"

If you encounter this error while using `cd` on virtual or `subst` drives:

1. **Quote the Path:** Ensure the executable path is quoted inside the `FOR` loop: `'"v:\bin\tool.exe"'`.
2. **Check Latency:** Run `alias --which`. If a drive is marked as **HUNG** or **TIMEOUT**, the sub-shell spawned by the `FOR` loop may fail to resolve the mapping before the process terminates.

---

## ⚙️ Build Info

* **Language:** Rust 2021 Edition.
* **Target:** `x86_64-pc-windows-msvc`.
* **OS Support:** Windows 10/11 CMD (PowerShell and Bash not supported).

---

**Since you have three separate binaries now, would you like me to create a simple `build.bat` or `justfile` that compiles all three and moves them into a central `release/` folder automatically?**