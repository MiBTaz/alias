# alias.exe

**The fast, safe, and modern alias manager for Windows CMD.**

`alias` is a production-grade suite of statically-linked Rust utilities designed to replace messy batch files and manual `doskey` entries. It utilizes a **Triple-Audit Hybrid Engine** to ensure shell macros remain 1:1 across the Win32 Kernel, the active process, and disk persistence.

---

## 🏗 The Multi-Binary Workspace

The project is structured as a modular workspace to ensure strict crate boundaries.

|**Binary**|**Layer**|**Description**|
|---|---|---|
|**`alias.exe`**|**Hybrid**|The primary orchestrator that attempts high-speed Win32 API calls with automatic fallbacks.|
|**`alias_win32.exe`**|**Kernel**|Specialized for direct `kernel32.dll` interop and console subsystem access.|
|**`alias_wrapper.exe`**|**Process**|Compatibility mode for strictly mimicking `doskey.exe` subprocess behavior.|
|**`alias_nuke.exe`**|**Scrub**|A dedicated utility for low-level environment "nuking."|

---

## ✨ Quantified Features

- **100% Test Fidelity**: Verified by a suite of **401 integration tests** across both Debug and Release profiles.

- **High-Speed Merging**: Logic utilizes a `HashMap` for identity checks during RAM-to-disk synchronization.

- **Ultra-Lightweight**: Release binaries are optimized to **~500KB** using LTO and symbol stripping.

- **Triple-Audit Sync**: Simultaneously verifies state across **Win32 RAM**, **Doskey Cache**, and the **Local File**.

- **Resilient Parsing**: Implements a **"Literal Mirror"** strategy to preserve internal quotes and fix the "XCD Tail Murder" bug.

- **Hardened Security**: The `is_valid_name` gatekeeper prevents illegal shell character injections (`|`, `&`, `!`) and reserved names (CON, AUX, PRN, etc.).

- **Command Stacking**: Options are processed sequentially.

    - `alias --reload --file my.aliases`: Resets macros then loads the file.

    - `alias --file my.aliases --reload`: Loads aliases then immediately resets them (effectively wiping them).


---

## 🛠 Technical Architecture: The "Shared Brain"

To maintain stability, all core logic resides in `alias_lib`, while the individual binaries act as thin wrappers to specific dedicated contexts.

### The Triple-Audit Layers

1. **Win32 RAM (Kernel)**: Uses `GetConsoleAliasesW` to read the true state of the Windows console subsystem.

2. **Doskey Wrapper (Process)**: Queries the `doskey.exe` cache to identify "ghost" aliases that the API might miss.

3. **Local Disk (Persistence)**: Ensures survival across reboots by managing your `.doskey` macro file.


The system uses **Reference-Based Verbosity** to avoid unnecessary memory clones while reporting diagnostics.

---

## 🛠 Usage & Flags

### Primary Actions

|**Command**|**Description**|
|---|---|
|`alias`|List all macros and perform a Triple-Audit.|
|`<name>`|Query/Lookup a specific alias definition.|
|`<name>=<value>`|Create or update an alias (subject to current Macro State).|
|`--remove <name>`|Delete alias from both File and RAM.|
|`--unalias <name>`|Drop alias from current session (RAM) only.|
|`--show-all`|List fully hydrated mapping (File + RAM).|
|`--edalias[=EXE]`|Open active file in editor (`VISUAL` -> `EDITOR` -> `notepad`).|
|`--reload`|Force re-sync of Win32 environment strings from file.|
|`--which`|Deep-audit sync status across all backends and check drive latency.|
|`--`|Stop processing flags (treat remainder as name/value pair).|

### Macro State & Toggles

|**Flag**|**Default**|**Description**|
|---|---|---|
|`--temp` / `--no-temp`|`--no-temp`|Force Volatility (RAM only) vs. Persistence.|
|`--case` / `--no-case`|`--no-case`|Force Case-Sensitivity vs. Case-Insensitive.|
|`--file <path>`|_Varies_|Redirect action to a specific alias file.|
|`--quiet` / `--no-quiet`|`--no-quiet`|Suppress metadata/whisper output.|
|`--icons` / `--no-icons`|`--icons`|Toggle ANSI glyphs/icons in output.|
|`--tips` / `--no-tips`|`--tips`|Toggle random usage hint injection (10% chance).|

### System & Boot

| **Flag**              | **Description**                                    |
|-----------------------|----------------------------------------------------|
| `--setup`             | Initialize Windows Registry AutoRun hooks.         |
| `--startup`           | Execute boot-time hydration (called via AutoRun).  |
| `--clear`             | Nuke all aliases in the active context.            |
| `--license`           | Show Licensing.                                    |
| `--version` / `--ver` | Show full build metadata vs. short version string. |


---

## 🌍 Environment Variables

|**Variable**|**Description**|
|---|---|
|**`ALIAS_FILE`**|Default alias file name (Default: `aliases.doskey`).|
|**`ALIAS_OPTS`**|Override default options/flags globally.|
|**`VISUAL` / `EDITOR`**|Primary/Secondary editor for `--edalias`.|
|**`PATHEXT` / `PATH`**|Used to resolve and validate editor short-names.|
|**`USERPROFILE`**|Primary fallback path if CWD is locked.|
|**`APPDATA`**|Secondary fallback path for configuration.|

---

## ⚡ Power User: Sub-shell Navigation (`ncd`)

The **"Literal but Clean"** parser ensures that complex directory navigation macros—like those using `ncd.exe`—do not suffer from trailing quote truncation.

> **Note:** Multi-quote commands are often "murdered" by the CMD shell. The most reliable way to load complex strings is via an external file using `--file`.

DOS

```
cd=FOR /F "tokens=*" %i IN ('"ncd.exe" $*') DO @(set "OLDPWD=%CD%" & chdir /d "%i")
```

### 👻 Troubleshooting: Ghost Aliases

If you find aliases that won't disappear, it is likely a "ghost" entry starting with a corrupt quote. The `alias_hybrid` engine detects these and uses the Win32 API to target and purge them specifically. Running `--clear` or `--reload` will scrub these from the session.

---

## ⚙️ Build Info

- **Dependencies**: Hardened with `rustix`, `winreg`, and `serial_test` for thread-safe registry and API operations.

- **Language**: Rust 2024 Edition (Workspace Resolver 3).

- **Target**: `x86_64-pc-windows-msvc`.

- **License**: PolyForm Noncommercial License 1.0.0.


---
