// alias_lib/src/lib.rs

use std::{env, fmt};
use std::fs;
use std::io;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
// use crate::ShowFeature::On;

#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => {
        #[cfg(any(debug_assertions, test))]
        {
            eprintln!("[TRACE] {}", format!($($arg)*));
        }
    };
}

#[macro_export]
macro_rules! voice {
    // 1. Direct "Off" call
    ($level:ident, Off, Off) => {
        $crate::Verbosity {
            level: $crate::VerbosityLevel::$level,
            show_icons: $crate::ShowIcons::Off,
            show_tips: $crate::ShowTips::Off,
            display_tip: None,
            in_startup: false,
        }
    };
    // 2. General case
    ($level:ident, $icons:expr, $tips:expr) => {{
        let tips_setting = $tips;
        let icons_setting = $icons;
        $crate::Verbosity {
            level: $crate::VerbosityLevel::$level,
            show_icons: icons_setting,
            show_tips: tips_setting,
            in_startup: false,
            // We store the OPTION of the tip string here, once.
            display_tip: match tips_setting {
                $crate::ShowTips::On => Some($crate::get_random_tip()),
                $crate::ShowTips::Off => None,
                $crate::ShowTips::Random => $crate::random_tip_show(),
            },
        }
    }};
}

#[macro_export]
#[doc(hidden)]
macro_rules! to_bool {
    // These match specific expressions before falling back to the generic .is_on()
    (On) => { true };
    (Off) => { false };
    (ShowIcons::On) => { true };
    (ShowIcons::Off) => { false };
    (ShowTips::On) => { true };
    (ShowTips::Off) => { false };
    (ShowFeature::On) => { true };
    (ShowFeature::Off) => { false };
    ($val:expr) => { $val.is_on() };
}

macro_rules! impl_voice_macro {
    ($macro_name:ident, $method:ident, $default_icon:ident, $d:tt) => {
        #[macro_export]
        macro_rules! $macro_name {
            // 1. Icon + Format String + Args
            ($v:expr, $icon:expr, $fmt:literal, $d($d arg:tt)+) => {{
                let msg = format!($fmt, $d($d arg)+);
                let formatted = $v.icon_format($icon, &msg);
                $v.$method(&formatted)
            }};

            // 2. Icon + Static String
            ($v:expr, $icon:expr, $msg:expr) => {{
                let formatted = $v.icon_format($icon, $msg);
                $v.$method(&formatted)
            }};

            // 3. Default Icon + Format String + Args
            ($v:expr, $fmt:literal, $d($d arg:tt)+) => {{
                let msg = format!($fmt, $d($d arg)+);
                let formatted = $v.icon_format($crate::AliasIcon::$default_icon, &msg);
                $v.$method(&formatted)
            }};

            // 4. Default Icon + Static String
            ($v:expr, $msg:expr) => {{
                let formatted = $v.icon_format($crate::AliasIcon::$default_icon, $msg);
                $v.$method(&formatted)
            }};
        }
    };
}
// Generate the suite
impl_voice_macro!(say,     say,     Say,     $);
impl_voice_macro!(whisper, whisper, Whisper, $);
impl_voice_macro!(shout,   shout,   Shout,   $);
impl_voice_macro!(scream,  scream,  Scream,  $);
impl_voice_macro!(text,    text,    Text,    $);


#[macro_export]
macro_rules! failure {
    // system errors and logs via e
    ($verbosity:expr, $err:expr) => {
        Box::new($crate::AliasError {
            message: $verbosity.icon_format($crate::AliasIcon::Fail, &$err.to_string()),
            code: $err.raw_os_error().unwrap_or(1) as u8,
        })
    };
    // This handles: (verbosity, ErrorCode::MissingName, "Error message")
    ($verbosity:expr, $code:expr, $($arg:tt)+) => {
        Box::new($crate::AliasError {
            message: $verbosity.icon_format($crate::AliasIcon::Fail, &format!($($arg)+)),
            code: $code as u8,
        })
    };
}

#[derive(Debug)]
pub struct AliasError {
    pub message: String,
    pub code: u8,
}

impl std::fmt::Display for AliasError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for AliasError {}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd)]
#[repr(u8)] // Ensures the enum is stored as a single byte
pub enum ErrorCode {
    Generic = 1,
    Syntax = 2,
    MissingFile = 3,
    Registry = 5,
    AccessDenied = 6,
    MissingName = 7,
}

#[derive(Debug, Clone)]
pub struct Task {
    pub action: AliasAction,
}

pub struct TaskQueue {
    tasks: Vec<Task>,
}

impl TaskQueue {
    pub fn new() -> Self {
        Self {
            tasks: Vec::with_capacity(4),
        }
    }

    pub fn push(&mut self, action: AliasAction) {
        self.tasks.push(Task { action });
    }

    pub fn clear(&mut self) {
        self.tasks.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    pub fn get(&self, index: usize) -> Option<&Task> {
        self.tasks.get(index)
    }
}

impl IntoIterator for TaskQueue {
    type Item = Task;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.tasks.into_iter()
    }
}

// --- Shared Constants ---
pub const ENV_ALIAS_FILE: &str = "ALIAS_FILE";
pub const ENV_ALIAS_OPTS: &str = "ALIAS_OPTS";
pub const ENV_EDITOR: &str = "EDITOR";
pub const ENV_VISUAL: &str = "VISUAL";
pub const DEFAULT_ALIAS_FILENAME: &str = "aliases.doskey";
pub const FALLBACK_EDITOR: &str = "notepad";
pub const REG_CURRENT_USER: &str = "HKCU";
pub const PATH_SEPARATOR: &str = "\\";
pub const REG_SUBKEY: &str = "Software\\Microsoft\\Command Processor";
pub const REG_AUTORUN_KEY: &str = "AutoRun";

// --- Output Identity Logic (The Matrix) ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(usize)]
pub enum VerbosityLevel {
    Mute = 0,   // Total silence
    Silent = 1, // Whisper/Data only
    Normal = 2, // Standard use
    Loud = 3,   // Audit/Verbose
}

#[derive(Debug, Clone, Copy)]
#[repr(usize)]
pub enum AliasIcon {
    None    = 0,  Win32 = 1,  Doskey = 2,  Disk        = 3,  Alert    = 4,
    Success = 5,  Info  = 6,  Say    = 7,  Whisper     = 8,  Shout    = 9,
    Scream  = 10, Fail  = 11, Hint   = 12, Environment = 13, Ok       = 14,
    Tools   = 15, File  = 16, Path   = 17, Text        = 18, Question = 19,
    _VariantCount,
}

pub const ICON_TYPES: usize = AliasIcon::_VariantCount as usize;

pub static ICON_MATRIX: [[&str; 2]; ICON_TYPES] = [
    ["", ""], // None
    ["W",  "⚡"], // Win32
    ["K",  "🔑"], // Doskey
    ["D",  "💽"], // Disk
    ["!!", "⚠️"], // Alert
    ["OK", "✨"], // Success
    ["I",  "ℹ️"], // Info
    ["-",  "📜"], // Say
    ["_",  "➢"], // Whisper
    ["!",  "🚫"], // Shout
    ["!!", "⛔"], // Scream
    ["X",  "❌"], // Fail
    ["H",  "💡"], // Hint
    ["E",  "♻️"], // Env
    ["+",  "✅"], // OK
    ["#",  "🛠️"], // Tools
    ["F",  "📁"], // File
    ["P",  "🛣️"], // Path
    ["T",  "️💬"], // Text
    ["?",  "🤔"], // Question
];


#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Verbosity {
    pub level: VerbosityLevel,
    pub show_icons: ShowIcons,
    pub show_tips: ShowTips,
    pub display_tip: Option<&'static str>,
    pub in_startup: bool,
}

impl Verbosity {
    pub fn is_silent(&self) -> bool {
        self.level == VerbosityLevel::Silent
    }
    pub fn normal() -> Self {
        Self {
            level: VerbosityLevel::Normal,
            show_icons: ShowFeature::On,
            show_tips: ShowTips::Random, // Default to random tips
            display_tip: random_tip_show(),
            in_startup: false,
        }
    }

    pub fn loud() -> Self {
        Self {
            level: VerbosityLevel::Loud,
            show_icons: ShowFeature::On,
            show_tips: ShowTips::On, // Always show tips in Loud mode
            display_tip: random_tip_show(),
            in_startup: false,
        }
    }

    pub fn silent() -> Self {
        Self {
            level: VerbosityLevel::Silent,
            show_icons: ShowFeature::Off,
            show_tips: ShowTips::Off,
            display_tip: None,
            in_startup: false,
        }
    }
    pub fn mute() -> Self {
        Self {
            level: VerbosityLevel::Mute,
            show_icons: ShowFeature::Off,
            show_tips: ShowTips::Off,
            display_tip: None,
            in_startup: false,
        }
    }

    pub fn get_icon_str(&self, id: AliasIcon) -> &'static str {
        ICON_MATRIX[id as usize][self.show_icons as usize]
    }

    pub fn icon_format(&self, icon: AliasIcon, msg: &str) -> String {
        if !self.show_icons.is_on() || msg.is_empty() {
            return msg.to_string();
        }
        format!("{} {}", self.get_icon_str(icon), msg)
    }

    pub fn tip(&self, msg: Option<&str>) {
        if self.show_tips == ShowTips::Off { return }
        if let Some(m) = msg {
            // We use Hint icon (💡) for tips
            let formatted = self.icon_format(AliasIcon::Hint, m);
            self.say("\n");
            self.say(&formatted);
        }
    }
    pub fn show_audit(&self) -> bool { self.level >= VerbosityLevel::Normal }
    pub fn show_xmas_lights(&self) -> bool { self.show_icons.is_on() && self.show_audit() }

    pub fn text(&self, msg: &str) -> String {
        msg.to_string()
    }

    pub fn whisper(&self, msg: &str) {
        // Keep your level check, just add the "Empty String" skip
        if msg.is_empty() || self.level < VerbosityLevel::Silent { return }
        println!("{}", msg);
    }

    pub fn say(&self, msg: &str) {
        if msg.is_empty() || self.level < VerbosityLevel::Normal { return }
        println!("{}", msg);
    }

    pub fn shout(&self, msg: &str) {
        if msg.is_empty() || self.level <= VerbosityLevel::Mute { return }
        println!("{}", msg);
    }

    pub fn scream(&self, msg: &str) {
        if msg.is_empty() { return } // Even a scream needs words!
        eprintln!("{}", msg);
    }

    pub fn audit(&self, msg: &str) {
        if msg.is_empty() { return }
        if self.level == VerbosityLevel::Loud {
            println!("{}", self.icon_format(AliasIcon::Info, msg));
        }
    }
    pub fn property(&self, label: &str, value: &str, width: usize, wdf: (bool, bool, bool)) {
        if self.level == VerbosityLevel::Mute { return; }

        // Pad the label to the left, then the value
        let line = format!("{:<label_width$}: {}", label, value, label_width = width);
        let (w, d, f) = wdf;

        // --- THE OPTIONAL LOGIC ---
        // Only build the audit string if at least one state is true
        let audit_block = if w || d || f {
            let spacer = if self.show_icons.is_on() { "  " } else { " " };
            let w_m = if w { self.get_icon_str(AliasIcon::Win32) } else { spacer };
            let d_m = if d { self.get_icon_str(AliasIcon::Doskey) } else { spacer };
            let f_m = if f { self.get_icon_str(AliasIcon::File)   } else { spacer };
            format!(" [{}{}{}]", w_m, d_m, f_m)
        } else {
            String::new() // Return empty string if no audit info exists
        };

        println!("{}{}", line, audit_block);
    }
    pub fn align(&self, name: &str, value: &str, width: usize, wdf: (bool, bool, bool)) {
        if self.level == VerbosityLevel::Mute { return; }

        let display_val = if value.is_empty() { "<EMPTY>" } else { value };
        let line = format!("{}={}", name, display_val);
        let (w, d, f) = wdf;

        // --- THE FIX ---
        // Emojis (show_icons) occupy 2 terminal columns.
        // ASCII letters/spaces occupy 1.
        let spacer = if self.show_icons.is_on() { "  " } else { " " };

        let w_m = if w { self.get_icon_str(AliasIcon::Win32) } else { spacer };
        let d_m = if d { self.get_icon_str(AliasIcon::Doskey) } else { spacer };
        let f_m = if f { self.get_icon_str(AliasIcon::File)   } else { spacer };

        print!("{:width$} [{}{}{}]", line, w_m, d_m, f_m, width = width);
    }
}

// --- Data Structures ---

#[derive(Debug, Clone, Copy)]
pub enum HelpMode { Short, Full }

#[derive(Debug, PartialEq, Clone)]
pub enum AliasAction {
    Set(SetOptions),
    Query(String),
    Edit(Option<String>),
    Remove(String),
    Unalias(String),
    Clear,
    Help,
    Reload,
    Setup,
    ShowAll,
    Which,
    Invalid,
}

impl fmt::Display for AliasAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid => write!(f, "Unrecognized or malformed command"),
            Self::Set(opts) => write!(f, "Set alias: {}", opts.name),
            Self::Remove(name) => write!(f, "Remove alias: {}", name),
            Self::Reload => write!(f, "Reload configuration"),
            Self::Query(name) => write!(f, "Querying alias {}: ", name),
            Self::Edit(path) => match path {
                Some(exe) => write!(f, "Editing alias file with: {}", exe),
                None => write!(f, "Editing alias file with default editor"),
            },
            Self::Unalias(alias) => write!(f, "Set alias: {}", alias),
            Self::Clear => write!(f, "Clear aliases"),
            Self::Help => write!(f, "Display help"),
            Self::Setup => write!(f, "Setup autorun registry entry"),
            Self::ShowAll => write!(f, "Show all aliases"),
            Self::Which => write!(f, "Run diagnostics"),
//            _ => write!(f, "Unknown action"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Ord, PartialOrd, Eq, Hash)]
pub enum ShowFeature {
    On = 1,
    Off = 0,
}

impl ShowFeature {
    pub fn is_on(&self) -> bool {
        matches!(self, Self::On)
    }
}

impl std::ops::Not for ShowFeature {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            Self::On => Self::Off,
            Self::Off => Self::On,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShowTips {
    On,
    Off,
    Random,
}
impl ShowTips {
    pub fn is_on(&self) -> bool {
        matches!(self, Self::On)
    }
    pub fn random(&self) -> bool {
        matches!(self, Self::Random)
    }
}


pub type ShowIcons = ShowFeature;
pub type DisplayTip  = ShowFeature;

#[derive(Debug, PartialEq, Clone)]
pub struct SetOptions {
    pub name: String,
    pub value: String,
    pub volatile: bool,
    pub force_case: bool,
}

#[derive(Debug, Clone, Default)]
pub struct PurgeReport {
    pub cleared: Vec<String>,
    pub failed: Vec<(String, u32)>,
}
impl PurgeReport {
    pub fn is_fully_clean(&self) -> bool {
        self.failed.is_empty()
    }
}

// alias_lib/src/diagnostics.rs (or inside lib.rs)

pub struct DiagnosticReport {
    pub binary_path: Option<PathBuf>,
    pub resolved_path: PathBuf,
    pub env_file: String,
    pub env_opts: String,
    pub file_exists: bool,
    pub is_readonly: bool,
    pub drive_responsive: bool,
    pub registry_status: RegistryStatus,
    pub api_status: Option<String>, // "CONNECTED" vs "SPAWNER"
}

pub enum RegistryStatus {
    Synced,
    Mismatch(String),
    NotFound,
}

#[derive(Debug, Clone)]
pub struct AliasEntryMesh {
    pub name: String,
    pub os_value: Option<String>,
    pub file_value: Option<String>,
}
impl AliasEntryMesh {
    pub fn is_empty_definition(&self) -> bool {
        self.os_value.is_none() && self.file_value.is_none()
    }
}

pub trait AliasProvider {
    // --- 1. THE ATOMIC "HANDS" (Platform must implement these) ---
    fn raw_set_macro(name: &str, value: Option<&str>) -> io::Result<bool>;
    fn raw_reload_from_file(path: &Path) -> io::Result<()>;
    fn get_all_aliases(verbosity: Verbosity) -> io::Result<Vec<(String, String)>>;
    fn write_autorun_registry(cmd: &str, v: Verbosity) -> io::Result<()>;
    fn read_autorun_registry() -> String;
    // --- 2. THE CENTRALIZED LOGIC (Default implementations) ---
    fn purge_ram_macros(verbosity: Verbosity) -> io::Result<PurgeReport> {
        let mut report = PurgeReport::default();
        // Use Self:: to call the atomic hands
        let before = Self::get_all_aliases(verbosity)?;

        for (name, _) in &before {
            // We use raw_set_macro with None to delete
            if Self::raw_set_macro(name, None)? {
                report.cleared.push(name.clone());
            }
        }

        let after = Self::get_all_aliases(verbosity)?;
        for (name, _) in after {
            if let Some(pos) = report.cleared.iter().position(|x| x == &name) {
                report.cleared.remove(pos);
                report.failed.push((name, 0));
            }
        }
        Ok(report)
    }
    fn reload_full(path: &Path, verbosity: Verbosity) -> Result<(), Box<dyn std::error::Error>> {
        // Call our own purge logic
        Self::purge_ram_macros(verbosity)?;

        let content = std::fs::read_to_string(path).map_err(|e| failure!(verbosity, e))?;
        let count = content.lines()
            .filter(|l| !l.trim().is_empty() && !l.trim().starts_with(';'))
            .count();

        // Call the engine
        Self::raw_reload_from_file(path)?;

        say!(verbosity, AliasIcon::Success, "Reload: {} macros injected.", count);
        Ok(())
    }
    fn install_autorun(verbosity: Verbosity) -> io::Result<()> {
        // 1. Check for the Global Source of Truth (Env Var)
        let env_var = std::env::var("ALIAS_FILE").ok();
        let mut startup_args = String::from("--startup");

        // We check this now to see if we need to ask the user for a path
        if let Some(path) = env_var {
            say!(verbosity, AliasIcon::Info, "Detected ALIAS_FILE in environment: {}", path);
            // Env is present, so the AutoRun command stays lean: "alias.exe --startup"
        } else {
            say!(verbosity, AliasIcon::Question, "ALIAS_FILE environment variable not found.");

            // 2. Interactive Prompt
            print!("  > Enter path to store aliases (leave blank for default): ");
            let _ = std::io::stdout().flush();
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            let input = input.trim();

            if !input.is_empty() {
                let path = PathBuf::from(input);

                // 3. Validation & Creation (The 0-byte Touch)
                if !path.exists() {
                    std::fs::File::create(&path)?;
                    say!(verbosity, AliasIcon::File, "Created new alias file: {}", path.display());
                }

                // Canonicalize so the Registry doesn't get a relative path like ".\ali.txt"
                let abs_path = std::fs::canonicalize(&path).unwrap_or(path);

                // Explicitly bake the file path into the AutoRun command
                startup_args = format!("--file \"{}\" --startup", abs_path.display());
            } else {
                // Defaulting logic (e.g., %USERPROFILE%\.aliases.txt)
                say!(verbosity, AliasIcon::Info, "Proceeding with default path resolution.");
            }
        }

        // 4. Construct the Final Command
        let exe_path = std::env::current_exe()?;
        let our_cmd = format!("\"{}\" {}", exe_path.display(), startup_args);

        // 5. Final Write
        Self::write_autorun_registry(&our_cmd, verbosity)
    }

    fn query_alias(name: &str, verbosity: Verbosity) -> Vec<String>;
    fn set_alias(opts: SetOptions, path: &Path, verbosity: Verbosity) -> io::Result<()>;
    fn run_diagnostics(path: &Path, verbosity: Verbosity) -> Result<(), Box<dyn std::error::Error>>;
    fn alias_show_all(verbosity: Verbosity) -> Result<(), Box<dyn std::error::Error>>;
}

#[cfg(test)]
pub fn get_alias_directory() -> Result<std::path::PathBuf, String> {
    std::env::current_exe()
        .map(|p| p.parent().unwrap_or(&p).to_path_buf())
        .map_err(|e| e.to_string())
}

// --- Main Runner ---
pub fn run<P: AliasProvider>(mut args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    // 1. ENV Injection (Standard)
    if let Ok(opts) = env::var(ENV_ALIAS_OPTS) {
        let extra: Vec<String> = opts.split_whitespace()
            .map(String::from)
            .filter(|opt| matches!(opt.as_str(),
              "--quiet" | "--temp" | "--tips" | "--no-tips" | "--icons" | "--no-icons" | "--force"))
            .collect();
        args.splice(1..1, extra);
    }

    // 2. Parse intent (The TaskQueue + Voice)
    let (mut queue, verbosity, cli_path) = parse_arguments(&args);

    // 3. Resolve Path
    let path = cli_path.or_else(get_alias_path).ok_or_else(|| {
        failure!(verbosity, ErrorCode::MissingFile, "Error: No alias file found.")
    })?;

    // 4. THE STARTUP HYDRATION
    // If the shell is just waking up, we MUST pull from disk.
    if verbosity.in_startup {
        // We dispatch Reload immediately. This fills the RAM from your .txt file.
        // We use the provider <P> to ensure it's the real Win32 logic.
        dispatch::<P>(AliasAction::Reload, verbosity.clone(), &path)?;

        // If the ONLY thing the user typed was '--startup',
        // we're done. No need for ShowAll.
        if queue.is_empty() {
            return Ok(());
        }
    }

    // 5. THE FALLBACK (Non-startup empty call)
    if queue.is_empty() {
        queue.push(AliasAction::ShowAll);
    }

    // 6. EXECUTION LOOP
    // Now we process whatever else was in the queue (like that xcd=dir payload)
    for task in queue {
        dispatch::<P>(task.action, verbosity.clone(), &path)?;
    }

    // 7. Success Tip
    if let Some(tip_text) = verbosity.display_tip {
        say!(verbosity, AliasIcon::None, "\n");
        say!(verbosity, AliasIcon::Info, tip_text);
    }

    Ok(())
}

pub fn dispatch<P: AliasProvider>(
    action: AliasAction,
    verbosity: Verbosity,
    path: &Path
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        AliasAction::Clear => {
            whisper!(verbosity, "Purging RAM macros...");
            let report = P::purge_ram_macros(verbosity)?;
            if !report.cleared.is_empty() {
                say!(verbosity, AliasIcon::Info,"Removed {} aliases.", report.cleared.len());
            }
        }
        AliasAction::Reload => P::reload_full(path, verbosity)?,
        AliasAction::ShowAll => P::alias_show_all(verbosity)?,
        AliasAction::Query(term) => {
            for line in P::query_alias(&term, verbosity) {
                verbosity.whisper(&line);
            }
        }
        AliasAction::Unalias(raw_name) => {
            let name = raw_name.split('=').next().unwrap_or(&raw_name).trim();
            if !name.is_empty() {
                let opts = SetOptions {
                    name: name.to_string(),
                    value: String::new(), // The "unset" trigger
                    volatile: true,       // Memory only
                    force_case: false,
                };
                P::set_alias(opts, path, verbosity)?;
                say!(verbosity, AliasIcon::Win32,"Removed alias {}", name);
            } else {
                return Err(failure!(verbosity, ErrorCode::MissingName, "Error: need an alias to remove"));
            }
        }
        AliasAction::Remove(raw_name) => {
            let name = raw_name.split('=').next().unwrap_or(&raw_name).trim();
            if !name.is_empty() {
                let opts = SetOptions {
                    name: name.to_string(),
                    value: String::new(), // The "unset" trigger
                    volatile: false,       // Memory only
                    force_case: false,
                };
                P::set_alias(opts, path, verbosity)?;
                say!(verbosity, AliasIcon::File,"Removed alias {}", name);
            } else {
                return Err(failure!(verbosity, ErrorCode::MissingName, "Error: need an alias to remove"));
            }
        }
        AliasAction::Set(opts) => P::set_alias(opts, path, verbosity)?, // Fixed Missing Arm
        AliasAction::Edit(custom_editor) => {
            open_editor(path, custom_editor, verbosity)?;
            P::reload_full(path, verbosity)?;
        }
        AliasAction::Which => {
            P::alias_show_all(verbosity)?;
            say!(verbosity, AliasIcon::None, "\n");
            P::run_diagnostics(path, verbosity)?;
        },
        AliasAction::Setup => P::install_autorun(verbosity)?,
        AliasAction::Help => print_help(verbosity, HelpMode::Full, Some(path)),
        AliasAction::Invalid => {
            scream!(verbosity, AliasIcon::Fail, "Invalid command: {}", AliasAction::Invalid);
            print_help(verbosity, HelpMode::Short, Some(path));
        }
    }
    Ok(())
}

// --- Audit & Mesh Logic ---

pub fn mesh_logic(os_list: Vec<(String, String)>, file_list: Vec<(String, String)>) -> Vec<AliasEntryMesh> {
    let mut mesh_list: Vec<AliasEntryMesh> = os_list
        .into_iter()
        .map(|(n, v)| AliasEntryMesh {
            name: n,
            os_value: Some(v),
            file_value: None,
        })
        .collect();

    for (f_name, f_val) in file_list {
        if let Some(existing) = mesh_list.iter_mut().find(|e| e.name == f_name) {
            existing.file_value = Some(f_val);
        } else {
            mesh_list.push(AliasEntryMesh {
                name: f_name,
                os_value: None,
                file_value: Some(f_val),
            });
        }
    }
    mesh_list
}

pub fn perform_audit(os_pairs: Vec<(String, String)>, verbosity: Verbosity) -> Result<(), Box<dyn std::error::Error>> {
    let file_pairs = dump_alias_file(verbosity)?;
    let mesh = mesh_logic(os_pairs, file_pairs);
    display_audit(&mesh, verbosity);
    Ok(())
}

pub fn display_audit(mesh_list: &[AliasEntryMesh], verbosity: Verbosity) {
    let mut desync_detected = false;
    let max_len = mesh_list.iter()
        .map(|e| {
            let val = e.os_value.as_deref().unwrap_or("<MISSING>");
            format!("{}={}", e.name, val).len()
        })
        .max().unwrap_or(20);

    for entry in mesh_list {
        // 1. Check for corruption in the Name before alignment
        let mut corruption_note = String::new();
        if !is_valid_name(&entry.name) {
            corruption_note = String::from(" !! CORRUPT: Alias contains illegal characters ");
            desync_detected = true;
        }

        let os_val = entry.os_value.as_deref().unwrap_or("");

        // 2. Align the base entry
        verbosity.align(
            &entry.name,
            os_val,
            max_len + 5,
            (entry.os_value.is_some(), false, entry.file_value.is_some())
        );

        // 3. Print the corruption note if the name is illegal
        if !corruption_note.is_empty() {
            print!("{}", corruption_note);
        }
        println!(); // Ensure the line terminates if align doesn't

        // 4. Check for standard value discrepancies
        if let (Some(os), Some(fi)) = (&entry.os_value, &entry.file_value) {
            if os != fi {
                verbosity.shout(&format!("Desync for {}: File has '{}'", entry.name, fi));
                desync_detected = true;
            }
        }
    }

    if desync_detected && verbosity.show_audit() {
        say!(verbosity, AliasIcon::None, "\n");
        say!(verbosity, AliasIcon::Info, "Tip: Run `alias --reload` to fix corrupted or out-of-sync macros.");
    }
}

pub fn perform_triple_audit(
    verbosity: Verbosity,
    win32_pairs: Vec<(String, String)>,
    mut wrap_pairs: Vec<(String, String)>,
    mut file_pairs: Vec<(String, String)>
) {
    let mut desync_detected = false;

    // 1. Calculate max width for perfect vertical alignment of the [WDF] block
    // We add 5 to give a small breathing room buffer
    let max_len = win32_pairs.iter()
        .chain(wrap_pairs.iter())
        .chain(file_pairs.iter())
        .map(|(n, v)| format!("{}={}", n, v).len())
        .max()
        .unwrap_or(35) + 5;

    say!(verbosity, AliasIcon::Info, "Triple Audit [W=Win32, D=Doskey, F=File]\n");

    // 2. PRIMARY LOOP: Win32 is the Current Kernel State
    for (name, w_val) in win32_pairs {
        let d_idx = wrap_pairs.iter().position(|(n, _)| n == &name);
        let f_idx = file_pairs.iter().position(|(n, _)| n == &name);

        let d_val = d_idx.map(|i| wrap_pairs.remove(i).1);
        let f_val = f_idx.map(|i| file_pairs.remove(i).1);
        // Check for corruption BEFORE alignment
        let mut corruption_note = String::new();
        if !is_valid_name(&name) {
            corruption_note = String::from(" !! CORRUPT: Alias contains illegal characters ");
            desync_detected = true;
        }

        // Align the base entry
        verbosity.align(&name, &w_val, max_len, (true, d_val.is_some(), f_val.is_some()));

        if !corruption_note.is_empty() {
            print!("{}", corruption_note);
        }

        // Align the base entry
        // verbosity.align(&name, &w_val, max_len, (true, d_val.is_some(), f_val.is_some()));

        // Check for value discrepancies
        if let Some(dv) = d_val {
            if w_val != dv {
                print!(" !! Doskey: '{}'", dv);
                desync_detected = true;
            }
        }
        if let Some(fv) = f_val {
            if w_val != fv {
                print!(" !! File: '{}'", fv);
                desync_detected = true;
            }
        }
        println!(); // Terminate the line
    }

    // 3. PHANTOM LOOP: In Doskey (Legacy/Wrapper) but missing from Win32 Kernel
    for (name, d_val) in wrap_pairs {
        let f_idx = file_pairs.iter().position(|(n, _)| n == &name);
        let f_val = f_idx.map(|i| file_pairs.remove(i).1);

        verbosity.align(&name, &d_val, max_len, (false, true, f_val.is_some()));
        print!(" <- PHANTOM: In Doskey, not Win32.");
        println!();
        desync_detected = true;
    }

    // 4. PENDING LOOP: In the .doskey file but not loaded into the OS
    for (name, f_val) in file_pairs {
        verbosity.align(&name, &f_val, max_len, (false, false, true));
        print!(" <- PENDING: In File, not OS.");
        println!();
        desync_detected = true;
    }

    // 5. Final Footer
    if desync_detected {
        say!(verbosity, AliasIcon::None, "\n");
        say!(verbosity, AliasIcon::Info, "Tip: Out of sync? Run `alias --reload` to align RAM with your config file.");
    }
}

// --- Utility Functions ---
// Progressively looser checks, can pick up anywhere in the chain
pub fn is_valid_name(name: &str) -> bool {
    // 1. Basic whitespace and emptiness checks
    if name.is_empty() || name.contains(' ') || name.trim() != name { return false; }

    // 2. THE BLACKLIST: Reject notorious shell animals
    // Includes quotes, colons (drive letters), carets (cmd escape), and redirections
    let notorious_animals = ['"', '\'', ':', '^', '&', '|', '<', '>', '(', ')'];
    if name.contains(&notorious_animals[..]) { return false; }

    // 3. Ensure the first character isn't a digit or punctuation
    // We allow alphabetic (including Kanji) or underscores
    let first = name.chars().next().unwrap();
    if  first.is_alphabetic() || first == '_' {
        return is_valid_name_permissive(name);
    }
    false
}

// Broader match to more allow 2 bytes
pub fn is_valid_name_permissive(name: &str) -> bool {
    if name.is_empty() || name.contains(' ') || name.trim() != name { return false; }
    let first = name.chars().next().unwrap();
    if first.is_alphabetic() || first == '_' {
         return is_valid_name_loose(name)
    }
    false
}

// very permissive
pub fn is_valid_name_loose(name: &str) -> bool {
    if name.is_empty() { return false; }
    let first = name.chars().next().unwrap();
    first.is_alphabetic() || first.is_ascii_digit()
}

pub fn get_alias_path() -> Option<PathBuf> {
    if let Ok(val) = env::var(ENV_ALIAS_FILE) {
        let p = PathBuf::from(val);
        return Some(if p.is_dir() { p.join(DEFAULT_ALIAS_FILENAME) } else { p });
    }
    ["APPDATA", "USERPROFILE"].iter()
        .filter_map(|var| env::var(var).ok().map(PathBuf::from))
        .map(|base| base.join("alias_tool").join(DEFAULT_ALIAS_FILENAME))
        .find(|p| p.parent().map_or(false, |parent| parent.exists()))
}

pub fn dump_alias_file(verbosity: Verbosity) -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    let path = get_alias_path().ok_or_else(|| {
        failure!(verbosity, ErrorCode::MissingFile, "Could not locate the alias configuration file.")
    })?;

    let content = std::fs::read_to_string(path).map_err(|e| failure!(verbosity, e))?;

    let pairs = content.lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with(';'))
        .filter_map(|line| line.split_once('=').map(|(n, v)| (n.to_string(), v.to_string())))
        .collect();

    Ok(pairs)
}

pub fn open_editor(path: &Path, override_ed: Option<String>, verbosity: Verbosity) -> io::Result<()> {
    let ed = override_ed.or_else(|| env::var(ENV_VISUAL).ok()).or_else(|| env::var(ENV_EDITOR).ok())
        .unwrap_or_else(|| FALLBACK_EDITOR.to_string());

    verbosity.say(&format!("Launching {}...", ed));
    if Command::new(&ed).arg(path).status().is_err() {
        Command::new(FALLBACK_EDITOR).arg(path).status()?;
    }
    Ok(())
}

pub fn parse_alias_args(args: &[String]) -> (AliasAction, Verbosity, Option<PathBuf>) {
    let mut voice = Verbosity::loud();
    let mut volatile = false;
    let mut force_case = false;
    let mut custom_path: Option<PathBuf> = None;
    let mut pivot_index = args.len();
    let mut skip_next = false;

    // --- STEP 1: FLAG CONSUMPTION ---
    for (i, arg) in args.iter().enumerate().skip(1) {
        if skip_next {
            skip_next = false;
            continue;
        }

        let low_arg = arg.to_lowercase();
        if low_arg.starts_with("--") {
            match low_arg.as_str() {
                "--startup"  => {voice = voice!(Mute, Off, Off); voice.in_startup = true;}
                "--quiet"    => { voice.level = VerbosityLevel::Silent; voice.show_icons = ShowFeature::Off; }
                "--temp"     => volatile = true,
                "--force"    => force_case = true,
                "--tips"     => { voice.show_tips = ShowTips::On; voice.display_tip = Some(get_random_tip()); },
                "--no-tips"  => voice.show_tips = ShowTips::Off,
                "--icons"    => voice.show_icons = ShowFeature::On,
                "--no-icons" => voice.show_icons = ShowFeature::Off,
                "--file"     => {
                    if let Some(path_str) = args.get(i + 1) {
                        custom_path = Some(PathBuf::from(path_str));
                        skip_next = true;
                    }
                }
                "--unalias" | "--remove" => {
                    // Found a command, pivot here so Step 2 catches it in f_args
                    pivot_index = i;
                    break;
                }
                _ => { pivot_index = i; break; }
            }
        } else {
            pivot_index = i;
            break;
        }
    }

    // --- STEP 2: PAYLOAD EXTRACTION ---
    let f_args: Vec<String> = if pivot_index < args.len() {
        args[pivot_index..].to_vec()
    } else {
        Vec::new()
    };

    // --- STEP 3: ACTION MAPPING ---
    if f_args.is_empty() {
        return (AliasAction::ShowAll, voice, custom_path);
    }

    let first = f_args[0].to_lowercase();

    // 1. Check for Editor Commands
    if first.starts_with("--edalias") || first.starts_with("--edaliases") {
        let editor = if let Some((_, ed)) = first.split_once('=') {
            Some(ed.to_string())
        } else {
            None
        };
        return (AliasAction::Edit(editor), voice, custom_path);
    }

    // 2. Check for Deletion Commands (MOVED UP)
    if first == "--unalias" {
        let target = f_args.get(1).cloned().unwrap_or_default();
        return (AliasAction::Unalias(target), voice, custom_path);
    }

    if first == "--remove" {
        let target = f_args.get(1).cloned().unwrap_or_default();
        return (AliasAction::Remove(target), voice, custom_path);
    }

    // 3. Check for Resolved Commands (Setup, Reload, etc.)
    if let Some(action) = resolve_command(&first) {
        return (action, voice, custom_path);
    }

    // 4. Validate Name (Only for standard Set/Query)
    if !is_valid_name_loose(&first) {
        return (AliasAction::Invalid, voice, custom_path);
    }

    // --- STEP 4: SET vs QUERY SPLIT ---
    let input = f_args.join(" ");
    let delim = if input.contains('=') { '=' } else { ' ' };

    if let Some((n, v)) = input.split_once(delim) {
        (AliasAction::Set(SetOptions {
            name: n.trim().to_string(),
            value: v.trim().to_string(),
            volatile,
            force_case,
        }), voice, custom_path)
    } else {
        (AliasAction::Query(f_args[0].clone()), voice, custom_path)
    }
}

pub fn parse_arguments(args: &[String]) -> (TaskQueue, Verbosity, Option<PathBuf>) {
    let mut queue = TaskQueue::new();
    let mut voice = Verbosity::loud();
    let mut custom_path: Option<PathBuf> = None;
    let mut volatile = false;
    let mut force_case = false;
    let mut pivot_index = args.len();
    let mut skip_next = false;

    // --- STEP 1: FLAG HARVESTING ---
    for (i, arg) in args.iter().enumerate().skip(1) {
        if skip_next { skip_next = false; continue; }

        let low_arg = arg.to_lowercase();
        match low_arg.as_str() {
            // the punch out
            "--help" => {
                queue.clear();
                queue.push(AliasAction::Help);
                return (queue, voice, custom_path);
            }
            // Configs
            "--startup"  => { voice = voice!(Mute, Off, Off); voice.in_startup = true; }
            "--quiet"    => { voice.level = VerbosityLevel::Silent; voice.show_icons = ShowFeature::Off; }
            "--temp"     => volatile = true,
            "--force"    => force_case = true,
            "--tips"     => { voice.show_tips = ShowTips::On; voice.display_tip = Some(get_random_tip()); },
            "--no-tips"  => voice.show_tips = ShowTips::Off,
            "--icons"    => voice.show_icons = ShowFeature::On,
            "--no-icons" => voice.show_icons = ShowFeature::Off,
            "--file"     => {
                if let Some(path_str) = args.get(i + 1) {
                    custom_path = Some(PathBuf::from(path_str));
                    skip_next = true;
                    pivot_index = i + 2;
                } else {
                    scream!(voice, AliasIcon::Alert, "--file requires a path");
                    pivot_index = i + 1;
                }
            }
            // Actions
            // Inside Step 1 match block
            "--edalias" | "--edaliases" => {
                let editor = arg.split_once('=').map(|(_, ed)| ed.to_string());
                queue.push(AliasAction::Edit(editor));
                pivot_index = i + 1;
            }
            "--reload" => { queue.push(AliasAction::Reload); pivot_index = i + 1; }
            "--which"  => { queue.push(AliasAction::Which);  pivot_index = i + 1; }
            "--unalias" | "--remove" => {
                if let Some(target) = args.get(i + 1) {
                    if is_valid_name(target) {
                        let action = if low_arg == "--remove" { AliasAction::Remove(target.clone()) }
                        else { AliasAction::Unalias(target.clone()) };
                        queue.push(action);
                    } else {
                        scream!(voice, AliasIcon::Alert, "Invalid name: '{}'", target);
                    }
                    skip_next = true;
                    pivot_index = i + 2;
                }
            }
            // THE PIVOT BRANCH
            _ => {
                if arg.starts_with("--") {
                    scream!(voice, AliasIcon::Alert, "Unknown option: {}", arg);
                    continue; // Skip typo, keep looking
                }

                // If it's a naked string, check if the "Name" part is valid
                let potential_name = arg.split('=').next().unwrap_or(arg);
                if is_valid_name(potential_name) {
                    pivot_index = i; // Valid start of command!
                    break;
                } else {
                    scream!(voice, AliasIcon::Alert, "Illegal command start: '{}'", arg);
                    queue.push(AliasAction::Invalid);
                    continue;
                }
            }
        }
    }

    // --- STEP 2: PAYLOAD HARVESTING ---
    let f_args = &args[pivot_index..];
    if !f_args.is_empty() {
        let raw_line = f_args.join(" ");
        if let Some((n, v)) = raw_line.split_once('=') {
            let name = n.trim();
            if is_valid_name(name) {
                queue.push(AliasAction::Set(SetOptions {
                    name: name.to_string(),
                    value: v.to_string(), // 59-byte literal preservation
                    volatile,
                    force_case,
                }));
            }
        } else {
            queue.push(AliasAction::Query(f_args[0].clone()));
        }
    }

    (queue, voice, custom_path)
}


pub fn update_disk_file(verbosity: Verbosity, name: &str, value: &str, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut pairs = if path.exists() {
        parse_macro_file(path, verbosity)?
    } else {
        Vec::new()
    };

    // Update or Remove
    if let Some(pos) = pairs.iter().position(|(n, _)| n == name) {
        if value.is_empty() { pairs.remove(pos); }
        else { pairs[pos].1 = value.to_string(); }
    } else if !value.is_empty() {
        pairs.push((name.to_string(), value.to_string()));
    }

    // --- TRANSACTIONAL WRITE ---
    let tmp_path = path.with_extension("doskey.tmp");
    {
        let mut content = String::new();
        for (n, v) in pairs {
            content.push_str(&format!("{}={}\n", n, v));
        }
        fs::write(&tmp_path, content).map_err(|e| failure!(verbosity, e))?;
    }

    // Atomic Rename: If this fails, the original file is untouched
    fs::rename(&tmp_path, path).map_err(|e| failure!(verbosity, e))?;

    Ok(())
}

pub fn parse_macro_file(path: &Path, verbosity: Verbosity) -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path).map_err(|e| failure!(verbosity, e))?;

    let pairs = content.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .filter_map(|l| l.split_once('='))
        // 1. We keep the valid name check, but ONLY trim the key.
        // 2. We do NOT trim the value 'v' beyond the initial line trim.
        .filter(|(k, v)| is_valid_name(k.trim()) && !v.is_empty())
        .map(|(k, v)| (k.trim().to_string(), v.to_string()))
        .collect();

    Ok(pairs)
}

pub fn resolve_command(cmd: &str) -> Option<AliasAction> {
    match cmd {
        "--clear"  => Some(AliasAction::Clear),
        "--reload" => Some(AliasAction::Reload),
        "--setup"  => Some(AliasAction::Setup),
        "--which"  => Some(AliasAction::Which),

        "--help" | "-h" | "/?" => Some(AliasAction::Help),
        _ => None,
    }
}

pub fn query_alias_file(name: &str, path: &Path, verbosity: Verbosity) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut results = Vec::new();
    let search = format!("{}=", name.to_lowercase());

    // Read the file (The Source of Truth)
    let content = std::fs::read_to_string(path).map_err(|e| failure!(verbosity, e))?;

    let found = content
        .lines()
        .find(|line| line.to_lowercase().starts_with(&search));

    if let Some(line) = found {
        results.push(line.to_string());
    } else if !verbosity.is_silent() {
        // Only push the "Not Known" message if we aren't in Silent/DataOnly mode
        results.push(format!("{} is not a known alias in the config file.", name));
    }

    Ok(results)
}

pub fn is_path_healthy(path: &Path) -> bool {
    path.exists() && path.is_file()
}

pub fn calculate_new_file_state(original_content: &str, name: &str, value: &str) -> String {
    let mut lines: Vec<String> = original_content
        .lines()
        .map(|l| l.to_string())
        .collect();

    let search = format!("{}=", name.to_lowercase());
    let mut found = false;

    for line in lines.iter_mut() {
        if line.to_lowercase().starts_with(&search) {
            if value.is_empty() {
                line.clear(); // Mark for removal
            } else {
                *line = format!("{}={}", name, value);
            }
            found = true;
            break;
        }
    }

    if !found && !value.is_empty() {
        lines.push(format!("{}={}", name, value));
    }

    lines.into_iter()
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

// alias_lib/src/lib.rs


pub fn render_diagnostics(report: DiagnosticReport, verbosity: Verbosity) {
    whisper!(verbosity, AliasIcon::Tools, "--- Alias Tool Diagnostics ---");
    let w = 15; // Width for the label column
    let none = (false, false, false);

    if let Some(p) = report.binary_path {
        verbosity.property("Binary Loc", &p.to_string_lossy(), w, none);
    }

    verbosity.property("File Var", &report.env_file, w, none);
    verbosity.property("Env Var", &report.env_opts, w, none);
    verbosity.property("Resolved", &report.resolved_path.to_string_lossy(), w, none);

    let file_status = if !report.file_exists {
        text!(verbosity, AliasIcon::Fail, "MISSING")
    } else {
        text!(verbosity, AliasIcon::Ok, "WRITABLE")
    };
    verbosity.property("File Status", &file_status, w, none);

    if report.drive_responsive {
        // Here we use the icons to show "Win32, Doskey, File" are all happy
        verbosity.property("Drive", "RESPONSIVE", w, none);
    }
}

pub fn is_drive_responsive(path: &Path) -> bool {
    // Attempt a tiny 1-byte read to verify the handle is actually alive
    std::fs::File::open(path).and_then(|mut f| {
        let mut buf = [0; 1];
        f.read(&mut buf)
    }).is_ok()
}

pub fn perform_autorun_install<P: AliasProvider>(
    // provider: &P, <--- REMOVE THIS
    verbosity: Verbosity
) -> io::Result<()> {
    let path = get_alias_path().ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "No alias file found. Set ALIAS_FILE.")
    })?;

    let exe_path = std::env::current_exe()?;
    let our_cmd = format!("\"{}\" --reload --file \"{}\"", exe_path.display(), path.display());

    // This is now a static call to the type P
    P::write_autorun_registry(&our_cmd, verbosity)
}


pub fn get_random_tip() -> &'static str {
    let tips = [
        "Tired of Notepad? Set 'EDITOR=code' in your env to use VS Code for --edalias.",
        "Tired of Notepad? Set 'VISUAL=code' in your env to use VS Code for --edalias.",
        "Use --temp to keep an alias in RAM only—it vanishes when you close the window.",
        "The Audit (alias --which) checks if your File, and the system are in sync.",
        "You can use $* in your values! e.g., 'alias g=git $*' passes all args to git.",
        "Hate icons? Set 'ALIAS_OPTS=--no-icons' in your system environment to hide them.",
        "Too noisy? Set 'ALIAS_OPTS=--quiet' to reduce the output.",
        "Run 'alias --reload' to force-sync your current session with your config file.",
        "alias --setup hooks into the registry so your macros 'just work' in every window.",
        "Type 'alias <name>' (without an '=') to see what a specific macro does.",
        "Atomic saving ensures your alias file is never corrupted by a mid-write crash.",
        "Put flags in 'ALIAS_OPTS' to set global defaults like --quiet or --no-tips.",
        "--which finds 'Phantom' aliases—RAM macros no longer in your config file.",
        "Setting an alias to empty (alias x=) is a shortcut for the --remove command.",
        "A 'Pending' audit status means your file changed but you haven't run --reload.",
        "Diagnostics do a 1-byte 'heartbeat' to verify your drive is responsive.",
        "Deletions ignore case to prevent messy 'G=git' and 'g=git' duplicates.",
        "--setup hooks the AutoRun registry so macros work in every new window.",
        "The tool detects 'Read-Only' files and warns you before attempting a strike.",
        "Use --file <path> to use a custom alias list without changing env vars.",
        "The Triple Audit aligns icons in a vertical [WDF] block for easy scanning.",
        "Checking registry sync ensures your AutoRun points to the right alias.exe.",
        "Use a filename 'set ALIAS_FILE=' in your env to use a default aliases file",
        "Tired of resetting your aliases every time? try --setup",
        "The tool uses an 'Atomic Rename' when saving, so your alias file is never corrupted if a crash occurs during a write.",
        "Flags placed in the 'ALIAS_OPTS' env var are auto-injected, letting you set global defaults like --quiet or --no-tips.",
        "Running --which identifies 'Phantom' aliases—macros stuck in your RAM that no longer exist in your config file.",
        "Setting an alias to an empty value (e.g., 'alias x=') is a fast shortcut for the permanent --remove command.",
        "The 'Triple Audit' compares the Win32 Kernel, Doskey wrapper, and your File to find every possible synchronization gap.",
        "A 'Pending' status in the audit means you've saved a change to your file but haven't run 'alias --reload' to activate it.",
        "The tool performs a 1-byte 'heartbeat' read on your config file to verify your drive is actually alive and responsive.",
        "Case-insensitivity is enforced during deletions to prevent messy duplicates like 'G=git' and 'g=git' in your file.",
        "The --setup flag hooks your aliases into the 'AutoRun' registry so they are available in every new console window.",
        "The tool automatically detects if your alias file is 'Read-Only' and will warn you before attempting a strike.",
        "You can use --file <path> to temporarily use a different alias collection without changing your environment variables.",
        "Diagnostics check your registry sync status to ensure your AutoRun command matches the current location of alias.exe.",
        "Transactional logic ensures that if a file write fails, the RAM state isn't updated, keeping your system in a known state.",
        "The 'Triple Audit' alignment uses 2-column spacing for icons to ensure the [WDF] block stays perfectly vertical.",
        "The tool identifies 'Legacy Wrappers' vs 'Win32 Kernel' aliases to help debug issues with different terminal types.",
        "Your current binary location is tracked in diagnostics to help you find where alias.exe is actually running from.",
    ];

    use std::time::{SystemTime, UNIX_EPOCH};
    // let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let seed = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    tips[(seed % tips.len() as u128) as usize]
}

pub fn random_tip_show() -> Option<&'static str> {
    let seed = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    if seed % 10 == 0 {
        return Some(get_random_tip());
    }
    None
}

pub fn print_help(verbosity: Verbosity, mode: HelpMode, path: Option<&Path>) {
    // 1. Header with Icon
    shout!(verbosity, AliasIcon::Info, "ALIAS (Rust) - High-speed alias management");
    say!(verbosity, AliasIcon::None, r#"
USAGE:
  alias                       List active macros
  alias                       List active macros
  alias <name>                Search for a specific macro
  alias <name>=[value]        Set or delete (if empty) a macro
  alias <name> [value]        Set a macro (alternate syntax)

"#);
    if let HelpMode::Short = mode {
        return
    }
    shout!(verbosity, AliasIcon::None, r#"
FLAGS:
  --help                  Show this help menu
  --quiet                 Suppress success output & icons
  --edalias[=EDITOR]      Open alias file in editor
  --file <path>           Specify a custom .doskey file
  --force                 Bypass case-sensitivity checks
  --reload                Force reload from file
  --remove                Remove a specific alias from the alias file
  --setup                 Install AutoRun registry hook
  --[no-]tips             Enable/Disable tips
  --unalias               Remove a specific alias from memory
  --temp                  Set alias in RAM only (volatile)
  --which                 Run diagnostics & Triple Audit
"#);

    // 3. Environment (Keep these separate to use the Constants)
    shout!(verbosity, AliasIcon::Environment, "ENVIRONMENT:");
    shout!(verbosity, AliasIcon::None, "  {:<15} Path to your .doskey file", ENV_ALIAS_FILE);
    shout!(verbosity, AliasIcon::None, "  {:<15} Default flags (e.g. \"--quiet\")", ENV_ALIAS_OPTS);


    // 4. Footer Status
    if let Some(p) = path {
        shout!(verbosity, "");
        shout!(verbosity, AliasIcon::File, &format!("CURRENT FILE: {}", p.display()));
    } else {
        shout!(verbosity, "");
        shout!(verbosity, "CURRENT FILE: None (Set ALIAS_FILE to fix)");
    }
//    verbosity.tip(random_tip_show());
}

