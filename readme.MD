
---

# alias.exe

**The fast, safe, and modern alias manager for Windows CMD.**

`alias` is a production-grade suite of statically-linked Rust utilities designed to replace messy batch files and manual `doskey` entries. It utilizes a **Triple-Audit Hybrid Engine** to ensure shell macros remain 1:1 across the Win32 Kernel, the active process, and disk persistence.

---

## 🏗 The Multi-Binary Workspace

The project is structured as a modular workspace to ensure strict crate boundaries and platform-agnostic core logic.

| Binary | Layer | Description |
| --- | --- | --- |
| **`alias.exe`** | **Hybrid** | The primary orchestrator (from `alias_hybrid`) that attempts high-speed Win32 API calls with automatic `doskey` fallbacks.

|
| **`alias_win32.exe`** | **Kernel** | Specialized for direct `kernel32.dll` interop and direct console subsystem access.

|
| **`alias_wrapper.exe`** | **Process** | Legacy compatibility mode for strictly mimicking `doskey.exe` subprocess behavior.

|
| **`alias_nuke.exe`** | **Scrub** | A dedicated utility for low-level environment "nuking," physically clearing macros from the kernel.

|

---

## ✨ Quantified Features

* **100% Test Fidelity**: Verified by a suite of **401 integration tests** across both Debug and Release profiles.


* **High-Speed Merging**: Logic utilizes a `HashMap` for  identity checks during RAM-to-disk synchronization.


* **Ultra-Lightweight**: Release binaries are optimized to ~700KB using LTO and symbol stripping.


* **Triple-Audit Sync**: Simultaneously verifies state across **Win32 RAM**, **Doskey Cache**, and the **Local File**.


* **Resilient Parsing**: Implements a "Literal Mirror" strategy to preserve internal quotes and fix the "XCD Tail Murder" bug.


* **Hardened Security**: The `is_valid_name` gatekeeper prevents illegal shell character injections (`|`, `&`, `!`) as well as reserved names, such as CON AUX or PRN.


* **Commands stack**: in the order given ```alias --reload --file my.aliases``` will reset your macros and load the ones in file.aliases. ```alias --file my.aliases --reload``` will load the aliases and then reset them

---

## 🛠 Technical Architecture: The "Shared Brain"

To maintain stability, all core logic resides in `alias_lib`, while the individual binaries act as thin wrappers.

### The Triple-Audit Layers

1.
**Win32 RAM (Kernel)**: Uses `GetConsoleAliasesW` to read the true state of the Windows console subsystem.


2.
**Doskey Wrapper (Process)**: Queries the `doskey.exe` cache to identify "ghost" aliases that the API might miss.


3.
**Local Disk (Persistence)**: Ensures survival across reboots by managing your `.doskey` macro file.



The system now uses **Reference-Based Verbosity** to avoid unnecessary memory clones while reporting diagnostics.

---

## 📂 Usage & Flags

| Command | Description |
| --- | --- |
| `alias` | List all macros and perform a Triple-Audit.

|
| `alias name=value` | Create/Update an alias with strict name validation.

|
| `alias name=` | Perform a "Forensic Purge" to remove an alias from all layers.

|
| `alias --reload` | Instantly sync current RAM with your `ALIAS_FILE`. |
| `alias --temp` | <br>**Volatile Mode**: Set an alias for the current session RAM only.

|
| `alias --force` | Preserve case-sensitivity during alias creation.

|
| `alias --which` | Run multi-layer diagnostics, including drive latency and API responsiveness.

|

---

## ⚡ Power User: Sub-shell Navigation (`ncd`)

The new **"Literal but Clean"** parser ensures that complex directory navigation macros—like those using `ncd.exe`—do not suffer from trailing quote truncation.
Note: multi quote commands still get murdered by cmd. The most reliable way to load them is via --file
```cmd
cd=FOR /F "tokens=*" %i IN ('"v:\lbin\ncd.exe" $*') DO @(set "OLDPWD=%CD%" & chdir /d "%i")

```

### 👻 Troubleshooting: Ghost Aliases

If you find aliases that won't disappear, it is likely a "ghost" entry starting with a corrupt quote. The `alias_hybrid` engine now detects these "ghosts" and uses the Win32 API to target and purge them specifically. --clear (or --reload) will remove them. 

---

## ⚙️ Build Info

*
**Language**: Rust 2024 Edition (Workspace Resolver 3).


*
**Dependencies**: Hardened with `rustix`, `winreg`, and `serial_test` for thread-safe registry and API operations.


* **Target**: `x86_64-pc-windows-msvc`.

---

