// alias_lib/src/lib.rs

// --- Includes ---
use std::{env, fmt};
use std::fs;
use std::io;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::collections::HashMap;
use std::fs::File;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::str::FromStr;
use function_name::named;

// --- Macros ---
#[macro_export]
macro_rules! trace {
    // Branch 1: Single argument
    ($arg:expr) => {
        #[cfg(any(debug_assertions, test))]
        {
            // Changing {} to {:?} is the key.
            // It will now print "Query("cmd")" instead of just "cmd"
            eprintln!("[TRACE][{}] {:?}", function_name!(), $arg);
        }
    };
    // Branch 2: Format string
    ($fmt:expr, $($arg:tt)*) => {
        #[cfg(any(debug_assertions, test))]
        {
            eprintln!("[TRACE][{}] {}", function_name!(), format!($fmt, $($arg)*));
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
            in_setup: false,
            writer: None,
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
            in_setup: false,
            writer: None,
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

macro_rules! setup_failure {
    ($voice:ident, $queue:ident, $arg:expr) => {
        scream!($voice, AliasIcon::Alert, "Invalid in setup mode: {}", $arg);
        $queue.clear();

        $queue.push(AliasAction::Invalid);
        return ($queue, $voice);
    };
    ($voice:ident, $queue:ident, $arg:expr, $msg:expr) => {
        scream!($voice, AliasIcon::Alert, $msg);
        $queue.clear();
        $queue.push(AliasAction::Invalid);
        return ($queue, $voice);
    };
}

#[macro_export]
macro_rules! dispatch_failure {
    ($verbosity:expr, $variant:expr, $msg:expr) => {{
        let error_msg = $msg;
        // Trace uses the Debug/Display of the variant
        trace!("[dispatch] Logic Error: {} (Variant: {:?})", error_msg, $variant);

        // Scream for the user
        scream!($verbosity, AliasIcon::Fail, "Critical Error. Should be unreachable in dispatch: {}", error_msg);

        // Return the error. We use ErrorCode::Generic because we can't
        // cast a complex Enum to a u8, but we want the 'failure!' formatting.
        return Err($crate::failure!($verbosity, $crate::ErrorCode::Generic, "{}", error_msg));
    }};
}

// --- Shared Constants ---
#[allow(dead_code)]
const REG_CURRENT_USER: &str = "HKCU";
const PATH_SEPARATOR: &str = r"\";
pub const UNC_GLOBAL_PATH: &str = r"\\?\UNC\";
pub const UNC_PATH: &str = r"\\?\";
pub const REG_SUBKEY: &str = r"Software\Microsoft\Command Processor";
pub const REG_AUTORUN_KEY: &str = "AutoRun";
pub const ENV_ALIAS_FILE: &str = "ALIAS_FILE";
pub const ENV_ALIAS_OPTS: &str = "ALIAS_OPTS";
const ENV_EDITOR: &str = "EDITOR";
const ENV_VISUAL: &str = "VISUAL";
pub const DEFAULT_ALIAS_FILENAME: &str = "aliases.doskey";
const DEFAULT_APPDATA_ALIAS_DIR: &str = "alias_tool";
const FALLBACK_EDITOR: &str = "notepad";
pub const IO_RESPONSIVENESS_THRESHOLD: Duration = Duration::from_millis(500);
const PATH_RESPONSIVENESS_THRESHOLD: Duration = Duration::from_millis(50);
const DEFAULT_EXTS: &str = ".COM;.EXE;.BAT;.CMD;.VBS;.VBE;.JS;.JSE;.WSF;.WSH;.MSC";
const DEFAULT_EXTS_EXE: &str = ".COM;.EXE;.SCR";
pub const RESERVED_NAMES: &[&str] = &[
"CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4",
"COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2",
"LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9"
];
pub const MAX_ALIAS_FILE_SIZE: usize = 1_500_000;
pub const MAX_BINARY_FILE_SIZE: usize = 100_000_000;

// --- Structs ---
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
    UnknownFileType = 8,
}

#[derive(Debug, Clone)]
pub struct Task {
    pub action: AliasAction,
    pub path: PathBuf,
}
pub struct TaskQueue {
    pub tasks: Vec<Task>,
    action_path: String,
}
impl TaskQueue {
    pub fn new() -> Self {
        Self {
            tasks: Vec::with_capacity(4),
            action_path: "".to_string(),
        }
    }
    pub fn push_file(&mut self, action: AliasAction, path: PathBuf) {
        self.tasks.push(Task { action, path, });
    }
    pub fn push(&mut self, action: AliasAction) {
        self.push_file(action, PathBuf::new());
    }
    pub fn pushpath(&mut self, path: String) {
        self.action_path = path.clone();
    }
    pub fn getpath(&mut self) -> String {
        self.action_path.clone()
    }
    pub fn clear(&mut self) {
        self.tasks.clear();
        self.action_path.clear();
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
    pub fn pull(&mut self) -> Option<Task>  { // Quewue action
        if self.tasks.is_empty() {
            None
        } else {
            Some(self.tasks.remove(0))
        }
    }
    pub fn pop(&mut self) -> Option<Task> { // Stack action
        if self.tasks.is_empty() {
            None
        } else {
            self.tasks.pop()
        }
    }
    pub fn iter(&self) -> std::slice::Iter<'_, Task> {
        self.tasks.iter()
    }
}
impl std::ops::Index<usize> for TaskQueue {
    type Output = Task;

    fn index(&self, index: usize) -> &Self::Output {
        // Calling .get(index) is safer, but standard Index behavior
        // in Rust is to panic on out-of-bounds.
        &self.tasks[index]
    }
}
impl IntoIterator for TaskQueue {
    type Item = Task;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.tasks.into_iter()
    }
}

pub enum BinarySubsystem {
    Gui,
    Cui,
    Script,
    Unavail,
    Unknown,
}

pub struct BinaryProfile {
    pub exe: PathBuf,
    pub args: Vec<String>,
    pub subsystem: BinarySubsystem,
    pub is_32bit: bool,
}
impl BinaryProfile {
    /// A "Safe" constructor for when things go wrong
    pub fn fallback(name: &str) -> Self {
        Self {
            exe: PathBuf::from(name),
            args: Vec::new(),
            subsystem: BinarySubsystem::Cui,
            is_32bit: false,
        }
    }
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

#[derive(Debug, Clone, PartialEq, Default)]
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

#[derive(Debug, Clone, PartialEq, Default)]
pub enum RegistryStatus {
    #[default]
    Uninitialized,
    Synced,
    Mismatch(String),
    NotFound,
}
impl RegistryStatus {
    pub fn is_synced(&self) -> bool {
        matches!(self, Self::Synced)
    }
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

#[derive(Debug, PartialEq, Eq)]
pub enum ProviderType {
    NotLinked,   // Library isolation / Mock
    Win32,       // Native Windows console
    Wrapper,     // Cross-platform wrapper
    Hybrid,
    Custom(String)
}

#[derive(Clone)]
pub struct Verbosity {
    pub level: VerbosityLevel,
    pub show_icons: ShowIcons,
    pub show_tips: ShowTips,
    pub display_tip: Option<&'static str>,
    pub in_startup: bool,
    pub in_setup: bool,
    pub writer: Option<Arc<Mutex<dyn std::io::Write + Send>>>,
}
impl Verbosity {
    pub fn is_silent(&self) -> bool {
        self.level <= VerbosityLevel::Silent
    }
    pub fn normal() -> Self {
        Self {
            level: VerbosityLevel::Normal,
            show_icons: ShowFeature::On,
            show_tips: ShowTips::Random, // Default to random tips
            display_tip: random_tip_show(),
            in_startup: false,
            in_setup: false,
            writer: None,
        }
    }

    pub fn loud() -> Self {
        Self {
            level: VerbosityLevel::Loud,
            show_icons: ShowFeature::On,
            show_tips: ShowTips::On, // Always show tips in Loud mode
            display_tip: random_tip_show(),
            in_startup: false,
            in_setup: false,
            writer: None,
        }
    }

    pub fn silent() -> Self {
        Self {
            level: VerbosityLevel::Silent,
            show_icons: ShowFeature::Off,
            show_tips: ShowTips::Off,
            display_tip: None,
            in_startup: false,
            in_setup: false,
            writer: None,
        }
    }
    pub fn mute() -> Self {
        Self {
            level: VerbosityLevel::Mute,
            show_icons: ShowFeature::Off,
            show_tips: ShowTips::Off,
            display_tip: None,
            in_startup: false,
            in_setup: false,
            writer: None,
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

    pub fn show_audit(&self) -> bool { self.level >= VerbosityLevel::Normal }
    pub fn show_xmas_lights(&self) -> bool { self.show_icons.is_on() && self.show_audit() }

    fn emit(&self, msg: &str) -> bool {
        if let Some(ref arc_writer) = self.writer {
            if let Ok(mut buf) = arc_writer.lock() {
                let _ = writeln!(buf, "{}", msg);
                return true;
            }
        }
        false
    }
    fn emitln(&self, msg: &str) -> bool {
        self.emit(&format!("{}\n", msg))
    }

    pub fn text(&self, msg: &str) -> String {
        msg.to_string()
    }

    pub fn whisper(&self, msg: &str) {
        // Keep your level check, just add the "Empty String" skip
        if msg.is_empty() || self.level < VerbosityLevel::Silent { return }
        if !self.emitln(msg) { println!("{}", msg); }
    }

    pub fn say(&self, msg: &str) {
        if msg.is_empty() || self.level < VerbosityLevel::Normal { return }
        if !self.emitln(msg) { println!("{}", msg); }
    }

    pub fn shout(&self, msg: &str) {
        if msg.is_empty() || self.level <= VerbosityLevel::Mute { return }
        if !self.emitln(msg) { println!("{}", msg); }
    }

    pub fn scream(&self, msg: &str) {
        if msg.is_empty() { return } // Even a scream needs words!
        self.emitln(msg);
        eprintln!("{}", msg);
    }

    fn property(&self, label: &str, value: &str, width: usize, wdf: (bool, bool, bool)) {
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

        let msg = format!("{}{}", line, audit_block);
        if !self.emitln(&msg) { println!("{}", msg); }
    }

    fn align(&self, name: &str, value: &str, width: usize, wdf: (bool, bool, bool)) {
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

    #[allow(dead_code)]
    fn with_buffer(buffer: Vec<u8>) -> Self {
        Self {
            level: VerbosityLevel::Loud,    // Full data output for testing
            show_icons: ShowFeature::Off,  // No icons (cleaner string matching)
            show_tips: ShowTips::Off,      // No random tip noise
            display_tip: None,
            in_startup: false,
            in_setup: false,
            writer: Some(Arc::new(Mutex::new(buffer))),
        }
    }
}
impl std::fmt::Debug for Verbosity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Verbosity")
            .field("level", &self.level)
            .field("show_icons", &self.show_icons)
            .field("show_tips", &self.show_tips)
            .field("in_startup", &self.in_startup)
            .field("writer", &"Option<Arc<Mutex<dyn Write>>>") // Just print a string label
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(usize)]
pub enum VerbosityLevel {
    Mute = 0,   // Total silence
    Silent = 1, // Whisper/Data only
    Normal = 2, // Standard use
    Loud = 3,   // Audit/Verbose
}

#[derive(Debug, Clone, Copy)]
pub enum HelpMode { Short, Full }

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

#[derive(Debug, PartialEq, Clone)]
pub enum AliasAction {
    Clear,
    Edit(Option<String>),
    File,
    Fail,
    Force,
    Help,
    Icons,
    NoIcons,
    Invalid,
    Reload,
    Set(SetOptions),
    Setup,
    ShowAll,
    Startup,
    Temp,
    Tips,
    NoTips,
    Query(String),
    Quiet,
    Remove(String),
    Unalias(String),
    Which,
    Toggle(Box<AliasAction>, bool),
}
impl AliasAction {
    pub fn to_cli_args(&self) -> String {
        match self {
            AliasAction::Clear => "--clear".to_string(),
            AliasAction::Edit(None) => "--edalias".to_string(),
            AliasAction::Edit(Some(editor)) => format!("--edalias=\"{}\"", editor),
            AliasAction::Fail => { "".to_string() },
            AliasAction::File => { "--file".to_string() },
            AliasAction::Force => { "--force".to_string() },
            AliasAction::Help => { "--help".to_string() },
            AliasAction::Icons => { "--[no-]icons".to_string() },
            AliasAction::NoIcons => { "".to_string() }
            AliasAction::Invalid => { "".to_string() },
            AliasAction::Reload => "--reload".to_string(),
            AliasAction::Remove(name) => format!("--remove {}", name),
            AliasAction::Query(name) => name.clone(),
            AliasAction::Set(opts) => {
                let mut s = format!("{}={}", opts.name, opts.value);
                if opts.volatile { s.push_str(" --temp"); }
                if opts.force_case { s.push_str(" --force"); }
                s
            }
            AliasAction::Setup => { "--setup".to_string() },
            AliasAction::ShowAll => { "".to_string() },
            AliasAction::Unalias(name) => format!("--unalias {}", name),
            AliasAction::Which => "--which".to_string(),
            AliasAction::Startup => { "--startup".to_string() },
            AliasAction::Temp => { "--temp".to_string() },
            AliasAction::Tips => { "--[no-]tips".to_string() },
            AliasAction::NoTips => { "".to_string()}
            AliasAction::Quiet => { "--quiet".to_string() },
            AliasAction::Toggle(_, _) => {"".to_string()},
        }
    }
    pub fn requires_file(&self) -> bool {
        match self {
            | AliasAction::Edit(_)
            | AliasAction::File
            | AliasAction::Reload
            | AliasAction::Remove(_)
            | AliasAction::Set(_)
            | AliasAction::ShowAll
            => true,
            // Everything else (Help, Setup, Which, etc.) doesn't touch the d
            _ => false,
        }
    }
    pub fn intent(arg: &str) -> Self {
        arg.parse().unwrap_or(AliasAction::Invalid)
    }
    pub fn is_switch(arg: &str) -> bool {
        if !arg.starts_with("--") {return false;}
        // Reuse the existing FromStr logic
        match Self::from_str(arg) {
            Ok(action) => !matches!(action, AliasAction::Query(_)),
            // If it's Invalid, it's a malformed flag (e.g., --unknown),
            // so we still treat it as a boundary switch.
            Err(_) => true,
        }
    }
}
impl FromStr for AliasAction {
    type Err = (); // We use AliasAction::Invalid instead of hard errors

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let low = s.to_lowercase();

        // 1. HIGH PRIORITY: The Internal Toggle (Check this BEFORE general split)
        if low.starts_with("__internal_toggle=") {
            if let Some((_, right)) = low.split_once('=') {
                if let Some((name, state_str)) = right.split_once(':') {
                    let state = state_str.parse::<bool>().unwrap_or(false);
                    return Ok(Self::Toggle(
                        Box::new(Self::Query(name.to_string())),
                        state
                    ));
                }
            }
            return Ok(Self::Invalid);
        }

        // 2. Pre-process Negation
        let (is_negated, search_term) = if let Some(stripped) = low.strip_prefix("--no-") {
            (true, format!("--{}", stripped))
        } else {
            (false, low.clone())
        };

        // 3. Handle Key=Value Pairs
        if let Some((left, right)) = low.split_once('=') {
            return Ok(match left {
                "--edalias" | "--edaliases"     => Self::Edit(Some(right.trim_matches('"').to_string())),
                "--remove"                      => Self::Remove(right.to_string()),
                "--unalias"                     => Self::Unalias(right.to_string()),
                // This was the "Black Hole" - it was catching __internal_toggle
                _ if !left.starts_with('-') => Self::Set(SetOptions {
                    name: left.to_string(),
                    value: right.to_string(),
                    volatile: false,
                    force_case: false,
                }),
                _ => Self::Invalid,
            });
        }

        // 4. Single Match Block for all Standalone Flags
        Ok(match search_term.as_str() {
            // Toggable Flags
            "--icons" => if is_negated { Self::NoIcons } else { Self::Icons },
            "--tips"  => if is_negated { Self::NoTips } else { Self::Tips },

            // Standard Flags (Negation not supported/ignored)
            "--help"                       => if is_negated { Self::Invalid } else { Self::Help },
            "--reload"                     => if is_negated { Self::Invalid } else { Self::Reload },
            "--setup"                      => if is_negated { Self::Invalid } else { Self::Setup },
            "--startup"                    => if is_negated { Self::Invalid } else { Self::Startup },
            "--clear"                      => if is_negated { Self::Invalid } else { Self::Clear },
            "--which"                      => if is_negated { Self::Invalid } else { Self::Which },
            "--edalias" | "--edaliases"    => if is_negated { Self::Invalid } else { Self::Edit(None) },
            "--showall" | "--all"          => if is_negated { Self::Invalid } else { Self::ShowAll },
            "--file"                       => if is_negated { Self::Invalid } else { Self::File },
            "--quiet"                      => if is_negated { Self::Invalid } else { Self::Quiet },
            "--temp"                       => if is_negated { Self::Invalid } else { Self::Temp },
            "--force"                      => if is_negated { Self::Invalid } else { Self::Force },
            "--unalias"                    => if is_negated { Self::Invalid } else { Self::Unalias(String::new()) },
            "--remove"                     => if is_negated { Self::Invalid } else { Self::Remove(String::new()) },

            // Catch-alls
            _ if low.starts_with("--") => Self::Invalid,
            _                              => Self::Query(s.to_string()),
        })
    }
}
impl fmt::Display for AliasAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Help                   => write!(f, "--help"),
            Self::Reload                 => write!(f, "--reload"),
            Self::Setup                  => write!(f, "--setup"),
            Self::Clear                  => write!(f, "--clear"),
            Self::Which                  => write!(f, "--which"),
            Self::ShowAll                => write!(f, "--all"),
            Self::File                   => write!(f, "--file"),
            Self::Edit(Some(ed)) => write!(f, "--edalias={}", ed),
            Self::Edit(None)             => write!(f, "--edalias"),
            Self::Remove(t)      => write!(f, "--remove={}", t),
            Self::Unalias(t)     => write!(f, "--unalias={}", t),
            Self::Set(opt)   => write!(f, "{}={}", opt.name, opt.value),
            Self::Query(q)       => write!(f, "{}", q),
            Self::Invalid                => write!(f, "--invalid"),
            Self::Fail                   => write!(f, "--fail"),
            Self::Force                 => write!(f, "--force"),
            Self::Startup               => write!(f, "--startup"),
            Self::Temp                  => write!(f, "--temp"),
            Self::Quiet                 => write!(f, "--quiet"),
            Self::Icons                 => write!(f, "--icons"),
            Self::NoIcons               => write!(f, "--no-icons"),
            Self::Tips                  => write!(f, "--tips"),
            Self::NoTips                => write!(f, "--no-tips"),
            Self::Toggle(from, to) => write!(f, "__internal_toggle={}:{}", from, to),
        }
    }
}
impl AliasAction {
    pub fn error (&self) -> AliasErrorString<'_> { AliasErrorString(self) }
}
pub struct AliasErrorString<'a>(&'a AliasAction);
impl<'a> std::fmt::Display for AliasErrorString<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // You must match on self.0 because self is the wrapper struct
        match self.0 {
            AliasAction::Clear => write!(f, "Clear aliases error"),
            AliasAction::Edit(path) => match path {
                Some(exe) => write!(f, "Error editing alias file with: {}", exe),
                None => write!(f, "Error editing alias file with default editor"),
            },
            AliasAction::Fail => write!(f, "General Error"),
            AliasAction::File => write!(f, "Error loading file for actions or load"),
            AliasAction::Help => write!(f, "Display help"),
            AliasAction::Invalid => write!(f, "Unrecognized or malformed command"),
            AliasAction::Icons => write!(f, "Error setting icons"),
            AliasAction::NoIcons => write!(f, "Error unsetting icons"),
            AliasAction::Query(name) => write!(f, "Error querying alias {}: ", name),
            AliasAction::Reload => write!(f, "Error reloading configuration"),
            AliasAction::Remove(name) => write!(f, "Error removing alias: {}", name),
            AliasAction::Set(opts) => write!(f, "Error setting alias: {}", opts.name),
            AliasAction::Setup => write!(f, "Error setting up autorun registry entry"),
            AliasAction::ShowAll => write!(f, "Error showing all aliases"),
            AliasAction::Tips => write!(f, "Error setting tips"),
            AliasAction::NoTips => write!(f, "Error unsetting tips"),
            AliasAction::Unalias(alias) => write!(f, "Error unaliasing alias: {}", alias),
            AliasAction::Which => write!(f, "Error running diagnostics"),
            AliasAction::Force => write!(f, "Error setting/using force case"),
            AliasAction::Startup => write!(f, "Error setting/using statup mode"),
            AliasAction::Temp => write!(f, "Error setting/using process as memory only"),
            AliasAction::Quiet => write!(f, "Error setting/using quiet mode"),
            AliasAction::Toggle(from, to) => write!(f, "Error reverse mapping {} to {}", from, to),
        }
    }
}

// --- Indexes ---
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

pub const TIPS_ARRAY: &[&str] = &[
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

//////////////////////////////////////////////////////
////
//// --- Functions and Routines ---
////
//////////////////////////////////////////////////////

// --- Providers/Interface ---
pub trait AliasProvider {
    fn raw_set_macro(name: &str, value: Option<&str>) -> io::Result<bool>;
    fn raw_reload_from_file(verbosity: &Verbosity, path: &Path) -> io::Result<()>;
    fn get_all_aliases(verbosity: &Verbosity) -> io::Result<Vec<(String, String)>>;
    fn write_autorun_registry(cmd: &str, verbosity: &Verbosity) -> io::Result<()>;
    fn read_autorun_registry() -> String;
    fn purge_ram_macros(verbosity: &Verbosity) -> io::Result<PurgeReport>;
    fn purge_file_macros(verbosity: &Verbosity, path: &Path) -> io::Result<PurgeReport>;
    fn reload_full(verbosity: &Verbosity, path: &Path, clear: bool) -> Result<(), Box<dyn std::error::Error>> {
        // Call our own purge logic
        if clear { Self::purge_ram_macros(verbosity)?; }

        let content = std::fs::read_to_string(path).map_err(|e| failure!(verbosity, e))?;
        let count = content.lines().filter_map(is_data_line).count();

        // Call the engine
        Self::raw_reload_from_file(verbosity, path)?;

        say!(verbosity, AliasIcon::Success, "Reload: {} macros injected.", count);
        Ok(())
    }
    fn sanitize_path(original: &PathBuf) -> String {
        // Strip existing quotes and wrap in exactly ONE set of double quotes
        format!("\"{}\"", original.to_string_lossy().trim_matches('"'))
    }
    fn setup_alias(verbosity: &Verbosity, queue: &TaskQueue) -> io::Result<()> {
        let mut parts: Vec<String> = Vec::new();
        parts.push("--startup ".to_string());

        for task in &queue.tasks {
            // We skip Setup because it's the trigger, not the payload.
            if task.action == AliasAction::Setup { continue; }

            match &task.action {
                // Reconstruct the pivot exactly as it was resolved
                AliasAction::File => {
                    parts.push(format!("--file {}", Self::sanitize_path(&task.path)));                }
                // Trust the mapper for everything else
                _ => {
                    let cmd = task.action.to_cli_args();
                    if !cmd.is_empty() {
                        parts.push(format!("{}", cmd));
                    }
                }
            }
        }
        // Join with a single space - No trailing spaces, no double spaces.
        let reconstructed = parts.join(" ");
        Self::install_autorun(verbosity, &reconstructed)
    }
    fn install_autorun(verbosity: &Verbosity, payload: &str) -> io::Result<()> {
        // 1. & 2. Identity Resolution (Your excellent Audit logic)
        let current_exe_name = get_alias_exe_nofail(verbosity);
        let full_exe_path = get_alias_exe()?;

        let search_name = Path::new(&current_exe_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&current_exe_name);

        let current_canon = std::fs::canonicalize(&full_exe_path)
            .map(normalize_path)
            .unwrap_or_else(|_| normalize_path(full_exe_path.clone()));

        let call_identifier = if let Some(found_path) = find_executable(search_name) {
            let system_found_canon = std::fs::canonicalize(&found_path)
                .map(normalize_path)
                .unwrap_or_else(|_| normalize_path(found_path));

            if current_canon == system_found_canon {
                search_name.to_string()
            } else {
                format!("\"{}\"", current_canon)
            }
        } else {
            format!("\"{}\"", current_canon)
        };

        // --- ALIAS_FILE & Payload Logic ---
        let mut startup_command = String::new();

        // Priority 1: Did the user provide actions/pivots in the current command?
        if !payload.is_empty() {
            startup_command.push_str(payload);
        }
        // Priority 2: No payload, check environment
        else if std::env::var("ALIAS_FILE").is_ok() {
            // Environment exists, so "alias --startup" is enough
        }
        // Priority 3: No payload, no environment -> Prompt (The "Snafu" Safety Net)
        else {
            say!(verbosity, AliasIcon::Question, "ALIAS_FILE environment variable not found.");
            print!("  > Enter path to store aliases (leave blank for default): ");
            let _ = std::io::stdout().flush();
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            let input = input.trim();

            if !input.is_empty() {
                let path = PathBuf::from(input);
                if !path.exists() { std::fs::File::create(&path)?; }
                let abs_path = std::fs::canonicalize(&path).unwrap_or(path);
                startup_command = format!("--file \"{}\"", abs_path.display());
            }
        }

        // 4. Final Construction: Always append --startup at the end
        let our_cmd = if startup_command.is_empty() {
            format!("{} --startup", call_identifier)
        } else {
            // Keeping your logic of putting startup first
            format!("{} --startup {}", call_identifier, startup_command.trim())
        };

        // 5. Final Write
        Self::write_autorun_registry(&our_cmd, verbosity)
    }
    fn query_alias(name: &str, verbosity: &Verbosity) -> Vec<String>;
    fn set_alias(opts: SetOptions, path: &Path, verbosity: &Verbosity) -> io::Result<()>;
    fn run_diagnostics(path: &Path, verbosity: &Verbosity) -> Result<(), Box<dyn std::error::Error>>;
    fn alias_show_all(verbosity: &Verbosity) -> Result<(), Box<dyn std::error::Error>>;
    fn provider_type() -> ProviderType {
        ProviderType::NotLinked
    }
    fn is_api_responsive(_timeout: Duration) -> bool { true }
}

// --- Functions ---
// --- Main Runner ---
// Phase A: Calls parse_arguments to build the TaskQueue.
// Phase B (The Executor Loop): Iterates over every Task in the queue and passes it to dispatch.
// Special Case: Handles --setup separately before the loop
pub fn run<P: AliasProvider>(mut args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    // 1. ENV Injection (unchanged)
    if let Ok(opts) = env::var(ENV_ALIAS_OPTS) {
        let extra: Vec<String> = opts.split_whitespace()
            .map(String::from)
            .filter(|opt| matches!(opt.as_str(),
              "--quiet" | "--temp" | "--tips" | "--no-tips" | "--icons" | "--no-icons" | "--force"))
            .collect();
        args.splice(1..1, extra);
    }

    // 2. Parse intent
    // This now returns a queue where tasks have their own .path (some valid, some raw/Fail)
    let (mut queue, verbosity) = parse_arguments(&args);
    // Check if the very first intent is Setup
    if let Some(first_task) = queue.tasks.first() {
        if first_task.action == AliasAction::Setup {

            // Check for any "Chainsaw" damage in the queue
            let is_poisoned = queue.tasks.iter().any(|t|
                matches!(t.action, AliasAction::Fail | AliasAction::Invalid)
            );

            if is_poisoned {
                scream!(verbosity, AliasIcon::Alert, "Setup aborted: Command line contains invalid paths or actions.");
                return Ok(());
            }

            // Clean stack! Pass the original args (minus the app name) to the installer.
            // We bypass hydration, anchors, and the execution loop entirely.
            // In your run loop
            return <P>::setup_alias(&verbosity, &queue).map_err(|e| e.into());
        }
    }

    // 3. STEP 3 IS NOW THE "ANCHOR" RESOLUTION
    // We establish the default context for tasks that didn't get watermarked.
    // This is the "Final Anchor"
    let default_path = get_alias_path(&String::new()) // Or pass your custom_path string
        .unwrap_or_else(|| PathBuf::from(DEFAULT_ALIAS_FILENAME));

    // 4. THE STARTUP HYDRATION
    if verbosity.in_startup {
        // Startup always uses the default anchor
        dispatch::<P>(Task {action: AliasAction::Reload, path: default_path.clone()} , &verbosity)?;
        if queue.is_empty() { return Ok(()); }
    }

    // 5. THE FALLBACK
    if queue.is_empty() {
        queue.push_file (AliasAction::ShowAll, default_path.clone());
        queue.pushpath(default_path.clone().to_string_lossy().to_string());
    }

    // 6. EXECUTION LOOP (The Forensic Dispatcher)
    for task in queue.tasks {
        // 1. Resolve the target for this specific task
        let target_path = if task.path.as_os_str().is_empty() {
            &default_path
        } else {
            &task.path
        };

        // 2. THE SILENT SKIP
        // If it's a payload task (add, show, etc.) and it's marked Fail,
        // we just drop it. No noise. The "Chainsaw" stays quiet here.
        if task.action == AliasAction::Fail {
            continue;
        }

        // 3. THE AUDIT POINT (The Cursor)
        // Here is where we check the path for real.
        // This is the ONLY place we should scream if the pivot is bad.
        if task.action == AliasAction::File {
            if let Some(concrete_path) = resolve_viable_path(&target_path) {
                <P>::reload_full(&verbosity, &concrete_path, false)?;
                continue;
            } else {
                // THIS is the "Record" that matters.
                scream!(verbosity, AliasIcon::Alert, &format!("Block Rejected: Invalid path '{}'", target_path.display()));
                continue;
            }
        }

        // 4. THE DISPATCH
        // Only healthy, non-Fail, non-File tasks reach the provider.
        if let Err(e) = dispatch::<P>(task, &verbosity) {
            scream!(verbosity, AliasIcon::Alert, &format!("Action Failed: {}", e));
        }
    }

    if let Some(tip_text) = verbosity.display_tip {
        say!(verbosity, AliasIcon::None, "\n");
        say!(verbosity, AliasIcon::Info, tip_text);
    }

    Ok(())
}
// --- Argument --- Processing
// Logic Block 1: Flag Harvesting.
// Logic Block 2: Sticky Path / Context Resolution (Hydrating the Task.path).
// Logic Block 3: Greedy Payload Collection.
#[named]
pub fn parse_arguments(args: &[String]) -> (TaskQueue, Verbosity) {
    let mut queue = TaskQueue::new();
    let mut voice = Verbosity::loud();
    let mut custom_path: PathBuf = PathBuf::from("");
    let mut volatile = false;
    let mut force_case = false;
    let mut pivot_index = args.len();
    let mut skip_next = false;
    let mut saw_unknown = false;
    let mut is_literal = false;
    // --- STEP 1: FLAG HARVESTING ---
    for (i, arg) in args.iter().enumerate().skip(1) {
        if skip_next {
            skip_next = false;
            continue;
        }

        // 1. Keep the Bridge check first
        if arg == "--" {
            if voice.in_setup { setup_failure!(voice, queue, arg); }
            is_literal = true;
            pivot_index = i + 1;
            break;
        }

        // 2. The Semantic Trigger
        let trigger = AliasAction::intent(arg);
        match trigger {
            // Punch-out Intent
            AliasAction::Help => {
                queue.clear();
                queue.push(AliasAction::Help);
                return (queue, voice);
            }
            // Setup Intent
            AliasAction::Setup => {
                voice.in_setup = true;
                if !queue.is_empty() {
                    setup_failure!(voice, queue, "Error: --setup must be the first command.", arg);
                }
                queue.push(AliasAction::Setup);
                continue;
            }
            // Modifiers (These don't push to queue, just change local state)
            // You'll need to add "Quiet", "Temp", "Force", and "Startup" to your enum
            AliasAction::Icons   => { voice.show_icons = ShowFeature::On; continue; },
            AliasAction::NoIcons => { voice.show_icons = ShowFeature::Off; continue; },
            AliasAction::Tips => { voice.show_icons = ShowFeature::On; continue; },
            AliasAction::NoTips  => { voice.show_tips = ShowTips::Off; continue; },
            AliasAction::Quiet => { voice.level = VerbosityLevel::Silent; voice.show_icons = ShowFeature::Off; continue; },
            AliasAction::Temp => { volatile = true; continue; },
            AliasAction::Force => { force_case = true; continue; },
            AliasAction::Startup => {
                if voice.in_setup { setup_failure!(voice, queue, arg); }
                voice = voice!(Mute, Off, Off);
                voice.in_startup = true;
                continue;
            }
            // The Consuming Intent (Triggers your existing logic)
            AliasAction::File => {
                let mut invalidate = false;
                if let Some(path_str) = args.get(i + 1) {
                    // semaphore. always set. never blindly use.
                    custom_path = PathBuf::from(path_str);
                    let raw_p = PathBuf::from(path_str);
                    let resolved = resolve_viable_path(&raw_p);
                    if let Some(valid_p) = resolved {
                        custom_path = valid_p;
                        queue.pushpath(custom_path.to_string_lossy().to_string());
                    } else {
                        invalidate = true;
                    }
                    for task in queue.tasks.iter_mut().rev() {
                        if task.action == AliasAction::File { break; }

                        if !invalidate {
                            task.path = custom_path.clone();
                        } else {
                            if task.action.requires_file() {
                                task.action = AliasAction::Fail;
                            }
                        }
                    }
                    //let action = if invalidate { AliasAction::Fail } else { AliasAction::File };
                    queue.push_file(AliasAction::File, raw_p);
                    skip_next = true;
                    pivot_index = i + 2;
                } else {

                    scream!(voice, AliasIcon::Alert, "--file requires a path");
                    pivot_index = i + 1;
                }
                continue;
            }
            AliasAction::Unalias(_) | AliasAction::Remove(_) => {
                if voice.in_setup { setup_failure!(voice, queue, arg); }
                pivot_index = i + 1;
                if let Some(next) = args.get(pivot_index) {
                    let next_intent = AliasAction::intent(next);

                    // If the next thing is just a Query (a name), we harvest it
                    if matches!(next_intent, AliasAction::Query(_)) && is_valid_name(next) {
                        let harvested = if matches!(trigger, AliasAction::Unalias(_)) {
                            AliasAction::Unalias(next.to_string())
                        } else {
                            AliasAction::Remove(next.to_string())
                        };
                        queue.push(harvested);
                        skip_next = true;
                        pivot_index = i + 2;
                        continue;
                    }
                }
                scream!(voice, AliasIcon::Alert, "{} requires a valid target", arg);
                queue.push(AliasAction::Fail);
                continue;
            },
            // Task-generating Intents
            AliasAction::Reload => { queue.push(AliasAction::Reload); pivot_index = i + 1; continue;},
            AliasAction::Which => { queue.push(AliasAction::Which); pivot_index = i + 1; continue;},
            AliasAction::Clear => { queue.push(AliasAction::Clear); continue;},
            // Use the data captured by intent() for parameterized flags
            AliasAction::Edit(val) => {
                if voice.in_setup { setup_failure!(voice, queue, arg); }
                queue.push(AliasAction::Edit(val));
                pivot_index = i + 1;
                continue;
            }
            _ => {
                if voice.in_setup { setup_failure!(voice, queue, arg); }

                // If intent() marked it Invalid but it starts with --, it's a bad flag
                if matches!(trigger, AliasAction::Invalid) && arg.starts_with("--") {
                    scream!(voice, AliasIcon::Alert, "Unknown option: {}", arg);
                    saw_unknown = true;
                    pivot_index = i + 1;
                    queue.push_file(AliasAction::Invalid, PathBuf::from(""));
                    continue;
                }

                // Otherwise, check if it's the start of the payload
                let potential_name = arg.split('=').next().unwrap_or(arg);
                if is_valid_name(potential_name) {
                    pivot_index = i;
                    break; // PIVOT: Step 2
                } else {
                    scream!(voice, AliasIcon::Alert, "Illegal command start: '{}'", arg);
                    queue.push(AliasAction::Invalid);
                    pivot_index = i + 1;
                }
            }
        }
    }

    // --- STEP 2: PAYLOAD HARVESTING ---
    let mut i = pivot_index;
    while i < args.len() { // Change 'if' to 'while'

        // Use is_literal to decide if we should 'gobble' everything
        let (action, consumed) = parse_set_argument(&voice, &args[i..], volatile, force_case, is_literal);

        queue.push(action);

        // Ensure we actually move forward
        let move_by = if consumed == 0 { 1 } else { consumed };
        i += move_by;

        // CRITICAL: If we hit a flag like --temp, we need to update 'volatile'
        // for the NEXT task in the loop!
        if i < args.len() {
            let next_arg = args[i].to_lowercase();
            if next_arg == "--temp" {
                volatile = true;
                i += 1; // Skip the flag so the next harvest gets the name
            }
            // Add other "mid-stream" flags here if needed (--force, etc.)
        }
    }
    // --- STEP 3: THE RESOLUTION (The Sticky Sweep) ---
    trace!("Pre test: Custom path: {:?}", custom_path);
    let current_context = if custom_path.to_string_lossy().is_empty() {
        trace!("Pre test: set to \"\": {:?}", custom_path);
        get_alias_path("") // Safety Net
    } else {
        trace!("Pre test: set to : {:?}", custom_path);
        resolve_viable_path(&custom_path) // Strict Override
    };
    if let Some(p) = &current_context {
        queue.pushpath(p.display().to_string());
    }
    trace!("Pre test: Current Context: {:?}", current_context);

    trace!("STEP 3 START");
    trace!("  Custom Path: {:?}", custom_path);
    trace!("  Current Context: {:?}", current_context);
    trace!("  Resolved Context: {:?}", current_context);
    let mut i= queue.tasks.len();

    for task in queue.tasks.iter_mut().rev() {
        trace!("  Processing Task [{}]: {:?}", i - 1, task.action);
        i -= 1;
        if task.action == AliasAction::File { break; }

        // ONLY act if it's empty AND it actually needs a file
        if task.path.as_os_str().is_empty() && task.action.requires_file() {
            if let Some(ref valid) = current_context {
                trace!("    -> [FILL] Path injected: {:?}", valid);
                task.path = valid.clone();
            } else {
                trace!("    -> [FAIL] Anchor was None, marking task Fail");
                // Anchor is invalid (bad --file path or bad default)
                task.action = AliasAction::Fail;
                let _ = failure!(voice, ErrorCode::MissingFile, "Error: file name required for {}", task.action.to_cli_args());
            }
            trace!("    -> [IF/ELSE let logic complete] Path set to: {:?}", task.path);
        }
        trace!("    -> [IF/ELSE task logic complete] Path set to: {:?}", task.path);
    }
    trace!("STEP 3 COMPLETE");

    // 4, Finalize
    if queue.is_empty() && !saw_unknown {
        queue.push(AliasAction::ShowAll);
    }
    (queue, voice)
}
#[named]
pub fn parse_set_argument(
    _verbosity: &Verbosity, // Prefixed to clear warning
    f_args: &[String],
    volatile: bool,
    force_case: bool,
    is_gobble: bool,
) -> (AliasAction, usize) {
    if f_args.is_empty() { return (AliasAction::Invalid, 0); }

    let mut i = 0;
    let mut cmd_parts: Vec<String> = Vec::new();
    let mut saw_equals = false;

    // --- 1. NAME & INITIAL VALUE EXTRACTION ---
    // Using a more idiomatic 'let' to avoid the "unused assignment" warning
    let name = if let Some((n, v)) = f_args[i].split_once('=') {
        saw_equals = true;
        let first_val = v;
        if !first_val.is_empty() {
            cmd_parts.push(first_val.to_string());
        }
        i += 1;
        n.trim().to_string()
    } else {
        let n = f_args[i].clone();
        i += 1;
        // Check for the "Bridge" (=)
        if i < f_args.len() && f_args[i] == "=" {
            saw_equals = true;
            i += 1;
        }
        n
    };

    // --- 2. VALUE HARVESTING ---
    while i < f_args.len() {
        trace!("f_args[{}]={}", i, f_args[i]);
        let current = &f_args[i];
        trace!("gobble:{} AliasAction::is_switch(current):{} current:{} f_args[i]={}", is_gobble, AliasAction::is_switch(current), current, &f_args[i]);
        if !is_gobble && (AliasAction::is_switch(current) || current == "--") {
            break;
        }
        cmd_parts.push(current.clone());
        trace!("while. after push. len is {}", cmd_parts.len());
        i += 1;
    }
    trace!("Done...");
    // --- 3. THE LOGIC GATE ---
    if !is_gobble && !is_valid_name(&name) {
        trace!("AliasAction::Invalid");
        return (AliasAction::Invalid, i);
    }

    trace!("saw={}, cmd_parts.len={}", saw_equals, cmd_parts.len());
    // CASE A: The "Empty Strike" (name= or name =)
    if saw_equals && cmd_parts.is_empty() {
        trace!("AliasAction::Unalias");
        return if volatile { (AliasAction::Unalias(name), i) }
        else { (AliasAction::Remove(name), i) };
    }

    // CASE B: The Query (No equals sign and no values found)
    if !saw_equals && cmd_parts.is_empty() {
        trace!("AliasAction::Query");
        return (AliasAction::Query(name), i);
    }

    // CASE C: The Standard Set
    let settings = SetOptions {
        name,
        value: cmd_parts.join(" "),
        volatile,
        force_case,
    };
    trace!("AliasAction::Set");
    (AliasAction::Set(settings), i)
}
// --- Dipatcher, does what you think
// Matches on AliasAction and executes the specific command strategy.
#[named]
pub fn dispatch<P: AliasProvider>(task: Task, verbosity: &Verbosity, ) -> Result<(), Box<dyn std::error::Error>> {
    // Convenience reference to the baked-in path
    let path = &task.path;

    match task.action {
        AliasAction::Set(opts) => {
            // Path is guaranteed by the 'run' hydration
            P::set_alias(opts, path, verbosity)?;
        }

        AliasAction::Remove(raw_name) => {
            let name = raw_name.split('=').next().unwrap_or(&raw_name).trim();
            if !name.is_empty() {
                let opts = SetOptions {
                    name: name.to_string(),
                    value: String::new(),
                    volatile: false,
                    force_case: false,
                };
                P::set_alias(opts, path, verbosity)?;
                say!(verbosity, AliasIcon::File, "Removed alias '{}' from {}", name, path.display());
            } else {
                return Err(failure!(verbosity, ErrorCode::MissingName, "Error: name required"));
            }
        }

        AliasAction::Unalias(raw_name) => {
            let name = raw_name.split('=').next().unwrap_or(&raw_name).trim();
            if !name.is_empty() {
                let opts = SetOptions {
                    name: name.to_string(),
                    value: String::new(),
                    volatile: true,
                    force_case: false,
                };
                P::set_alias(opts, path, verbosity)?;
                say!(verbosity, AliasIcon::File, "Removed alias '{}' ", name);
            } else {
                return Err(failure!(verbosity, ErrorCode::MissingName, "Error: name required"));
            }
        }

        AliasAction::Edit(custom_editor) => {
            let report = P::purge_file_macros(verbosity, path)?;
            for (failed_name, _error_code) in report.failed {
                shout!(verbosity, AliasIcon::Fail, &format!("Ghost Warning: Failed to unset '{}' from memory.", failed_name));
            }
            open_editor(path, custom_editor, verbosity)?;
            // Immediate sync so the edits are live in RAM
            P::reload_full(verbosity, path, false)?;
        }

        AliasAction::Which => {
            P::alias_show_all(verbosity)?;
            say!(verbosity, AliasIcon::None, "\n");
            P::run_diagnostics(path, verbosity)?;
        },

        AliasAction::Reload => P::reload_full(verbosity, path, true)?,

        AliasAction::Query(term) => {
            for line in P::query_alias(&term, verbosity) {
                verbosity.whisper(&line);
            }
        }

        AliasAction::ShowAll => P::alias_show_all(verbosity)?,

        AliasAction::Clear => {
            whisper!(verbosity, "Purging RAM macros...");
            P::purge_ram_macros(verbosity)?;
        }

        AliasAction::Help => print_help(verbosity, HelpMode::Full, Some(path)),

        AliasAction::File  => {
            P::reload_full(verbosity, path, false)?;
        }

        AliasAction::Setup => {
            scream!(verbosity, AliasIcon::Alert, "Setup should never be dispatched (Handled seperately).");
            print_help(verbosity, HelpMode::Short, Some(path));
        }

        AliasAction::Fail => {
            scream!(verbosity, AliasIcon::Alert, "Failed command state.");
            print_help(verbosity, HelpMode::Short, Some(path));
        }

        AliasAction::Invalid => {
            scream!(verbosity, AliasIcon::Alert, "Invalid command state.");
            print_help(verbosity, HelpMode::Short, Some(path));
        }
        AliasAction::Force => {dispatch_failure!(verbosity, AliasAction::Force, "Metadata Leak: Parser state variant reached the executor.");}
        AliasAction::Startup => {dispatch_failure!(verbosity, AliasAction::Startup, "Metadata Leak: Parser state variant reached the executor.");}
        AliasAction::Temp => {dispatch_failure!(verbosity, AliasAction::Temp, "Metadata Leak: Parser state variant reached the executor.");}
        AliasAction::Quiet => {dispatch_failure!(verbosity, AliasAction::Quiet, "Metadata Leak: Parser state variant reached the executor.");}
        AliasAction::Icons => {dispatch_failure!(verbosity, AliasAction::Quiet, "Metadata Leak: Parser state variant reached the executor.");}
        AliasAction::NoIcons => {dispatch_failure!(verbosity, AliasAction::Quiet, "Metadata Leak: Parser state variant reached the executor.");}
        AliasAction::Tips => {dispatch_failure!(verbosity, AliasAction::Quiet, "Metadata Leak: Parser state variant reached the executor.");}
        AliasAction::NoTips => {dispatch_failure!(verbosity, AliasAction::Quiet, "Metadata Leak: Parser state variant reached the executor.");}
        AliasAction::Toggle(_, _) => {dispatch_failure!(verbosity, AliasAction::Quiet, "Metadata Leak: Parser state variant reached the executor.");}
    }
    Ok(())
}

//////////////////////////////////////////////////////
////
//// --- Local dispatched targets, and underloads ---
////
//////////////////////////////////////////////////////
pub fn open_editor(path: &Path, override_ed: Option<String>, verbosity: &Verbosity) -> io::Result<()> {
    // 1. Resolve Preference (Handles Priority, Resolution, and PE Identification)
    // This gives us the 'Soul' and the 'Intent'
    let mut profile = get_editor_preference(verbosity, &override_ed);

    // 2. Canonicalize the target file (The file the user wants to edit)
    let absolute_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

    // Safety Check
    if !is_file_accessible(&absolute_path) {
        return Err(io::Error::new(io::ErrorKind::Other, "Target file inaccessible."));
    }

    // 3. Prepare the Command Line
    // We append the target file to the end of the existing editor args
    profile.args.push(absolute_path.to_string_lossy().to_string());

    say!(verbosity, &format!("Launching {}...", profile.args[0]));

    // 4. Execution Triage
    let status = match profile.subsystem {
        BinarySubsystem::Gui => {
            // GUI apps (VS Code, Notepad++): Launch directly
            let mut cmd = Command::new(&profile.args[0]);
            if profile.is_32bit {
                cmd.env("__COMPAT_LAYER", "RunAsInvoker");
            }
            cmd.args(&profile.args[1..]).status()
        }
        _ => {
            // CUI/Scripts/Unknown: Host via cmd /C for better terminal handling
            let mut cmd = Command::new("cmd");
            if profile.is_32bit {
                cmd.env("__COMPAT_LAYER", "RunAsInvoker");
            }
            cmd.arg("/C")
                .args(&profile.args) // Includes args[0] and our newly pushed path
                .status()
        }
    };

    // 5. The Fail-Safe (If the primary choice failed)
    if status.is_err() || !status.unwrap().success() {
        whisper!(verbosity, AliasIcon::Alert, "Primary editor failed. Falling back to notepad...");
        Command::new("notepad")
            .arg(&absolute_path)
            .status()?;
    }

    Ok(())
}

fn print_help(verbosity: &Verbosity, mode: HelpMode, path: Option<&Path>) {
    let exe_name = get_alias_exe_nofail(verbosity);
    shout!(verbosity, AliasIcon::Info, "ALIAS ({}) - High-speed alias management", exe_name);
    say!(verbosity, AliasIcon::None, r#"
USAGE:
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
  --                      Stop processing arguments
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

//////////////////////////////////////////////////////
////
//// --- Reporting and Diagnostics ---
////
//////////////////////////////////////////////////////
pub fn mesh_logic(os_list: Vec<(String, String)>, file_list: Vec<(String, String)>) -> Vec<AliasEntryMesh> {
    // 1. Move OS entries into a Map for O(1) "Identity Checks"
    // We use a Map because we need to 'pluck' items out of it as we find them.
    let mut os_map: HashMap<String, String> = os_list.into_iter().collect();
    let mut mesh_list: Vec<AliasEntryMesh> = Vec::with_capacity(file_list.len() + os_map.len());

    // 2. PRIMARY PASS: Follow the File Order
    // This respects the user's visual organization (categories, groups, etc.)
    for (f_name, f_val) in file_list {
        // If it's in the OS map, we take the value and REMOVE it from the map
        let os_val = os_map.remove(&f_name);

        mesh_list.push(AliasEntryMesh {
            name: f_name,
            os_value: os_val,
            file_value: Some(f_val),
        });
    }

    // 3. SECONDARY PASS: Collect the "Ghosts"
    // Anything remaining in os_map exists in RAM but NOT in the file.
    // We append these to the end to ensure 100% consistency.
    for (o_name, o_val) in os_map {
        mesh_list.push(AliasEntryMesh {
            name: o_name,
            os_value: Some(o_val),
            file_value: None,
        });
    }

    mesh_list
}

pub fn perform_audit(os_pairs: Vec<(String, String)>, verbosity: &Verbosity) -> Result<(), Box<dyn std::error::Error>> {
    let file_pairs = dump_alias_file(verbosity)?;
    let mesh = mesh_logic(os_pairs, file_pairs);
    display_audit(&mesh, verbosity);
    Ok(())
}

pub fn perform_triple_audit(
    verbosity: &Verbosity,
    win32_pairs: Vec<(String, String)>,
    mut wrap_pairs: Vec<(String, String)>,
    mut file_pairs: Vec<(String, String)>
) {
    let mut desync_detected = false;

    // 1. THE "OVERCHECK" WIDTH CALCULATION
    // We calculate based on the RAW strings. If we trim here, alignment drifts.
    let max_len = win32_pairs.iter()
        .chain(wrap_pairs.iter())
        .chain(file_pairs.iter())
        .map(|(n, v)| n.len() + v.len() + 1)
        .max()
        .unwrap_or(35) + 5;

    say!(verbosity, AliasIcon::Info, "Triple Audit [W=Win32, D=Doskey, F=File]\n");

    // 2. PRIMARY PASS: Win32 Kernel (The "Live" Truth)
    for (name, w_val) in win32_pairs {
        // Pluck matches from other lists to avoid double-processing
        let d_val = wrap_pairs.iter().position(|(n, _)| n == &name).map(|i| wrap_pairs.remove(i).1);
        let f_val = file_pairs.iter().position(|(n, _)| n == &name).map(|i| file_pairs.remove(i).1);

        // DISPLAY RAW: This preserves the "cxd=ehat? haha no" mess exactly as it is
        verbosity.align(&name, &w_val, max_len, (true, d_val.is_some(), f_val.is_some()));

        // CHECK 1: Name Corruption (The serious work)
        if !is_valid_name(&name) {
            print!(" {}", text!(verbosity, AliasIcon::Fail, "!! CORRUPT NAME"));
            desync_detected = true;
        }

        // CHECK 2: Value Desync (Compare intent, but show the drift)
        if let Some(dv) = d_val {
            if !functional_cmp(&w_val, &dv) {
                print!(" {} D: '{}'", text!(verbosity, AliasIcon::Alert, "!!"), dv);
                desync_detected = true;
            }
        }
        if let Some(fv) = f_val {
            if !functional_cmp(&w_val, &fv) {
                print!(" {} F: '{}'", text!(verbosity, AliasIcon::Alert, "!!"), fv);
                desync_detected = true;
            }
        }
        println!();
    }

    // 3. SECONDARY PASS: Phantom Entries (In Doskey wrapper, but missing from Kernel)
    for (name, d_val) in wrap_pairs {
        let f_val = file_pairs.iter().position(|(n, _)| n == &name).map(|i| file_pairs.remove(i).1);

        verbosity.align(&name, &d_val, max_len, (false, true, f_val.is_some()));
        print!(" {}", text!(verbosity, AliasIcon::Alert, "<- PHANTOM (Not in Kernel)"));

        if !is_valid_name(&name) { print!(" !! CORRUPT"); }
        println!();
        desync_detected = true;
    }

    // 4. TERTIARY PASS: Pending Entries (In File, but not loaded into OS)
    for (name, f_val) in file_pairs {
        verbosity.align(&name, &f_val, max_len, (false, false, true));
        print!(" {}", text!(verbosity, AliasIcon::Alert, "<- PENDING (Not in RAM)"));

        if !is_valid_name(&name) { print!(" !! CORRUPT"); }
        println!();
        desync_detected = true;
    }

    // 5. THE SURVIVAL FOOTER
    if desync_detected {
        say!(verbosity, AliasIcon::None, "");
        say!(verbosity, AliasIcon::Info, "Tip: Run `alias --reload` to synchronize all layers.");
    }
}

pub fn display_audit(mesh_list: &[AliasEntryMesh], verbosity: &Verbosity) {
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

pub fn render_diagnostics(report: DiagnosticReport, verbosity: &Verbosity) {
    whisper!(verbosity, AliasIcon::Tools, "--- Alias Tool Diagnostics ---");
    let w = 15;
    let none = (false, false, false);

    // 1. IDENTITY & PATHS
    if let Some(p) = report.binary_path {
        verbosity.property("Binary Loc", &p.to_string_lossy(), w, none);
    }
    verbosity.property("File Var", &report.env_file, w, none);
    verbosity.property("Env Var", &report.env_opts, w, none);
    verbosity.property("Resolved", &report.resolved_path.to_string_lossy(), w, none);

    // 2. DISK INTEGRITY
    let file_status = if !report.file_exists {
        text!(verbosity, AliasIcon::Fail, "MISSING")
    } else if report.is_readonly {
        text!(verbosity, AliasIcon::Alert, "READ-ONLY")
    } else {
        text!(verbosity, AliasIcon::Ok, "WRITABLE")
    };
    verbosity.property("File Status", &file_status, w, none);

    let (d_icon, d_msg) = if report.drive_responsive {
        (AliasIcon::Ok, "RESPONSIVE")
    } else {
        (AliasIcon::Fail, "TIMEOUT / UNREACHABLE")
    };
    verbosity.property("Drive", &text!(verbosity, d_icon, "{}", d_msg), w, none);

    // 3. PERSISTENCE (Registry)
    let reg_msg = match report.registry_status {
        RegistryStatus::Uninitialized => text!(verbosity, AliasIcon::Shout, "Not checked"),
        RegistryStatus::Synced => text!(verbosity, AliasIcon::Ok, "SYNCED"),
        RegistryStatus::NotFound => text!(verbosity, AliasIcon::Alert, "NOT FOUND (Run --setup)"),
        RegistryStatus::Mismatch(ref v) => text!(verbosity, AliasIcon::Alert, "MISMATCH: {}", v),
    };
    verbosity.property("Registry", &reg_msg, w, none);

    // 4. LIVE KERNEL STATUS
    if let Some(api) = report.api_status {
        let api_icon = if api.contains("CONNECTED") {
            AliasIcon::Win32
        } else {
            AliasIcon::Fail
        };
        verbosity.property("Win32 API", &text!(verbosity, api_icon, "{}", api), w, none);
    }

    whisper!(verbosity, AliasIcon::Info, "Diagnostic check complete.");
}


//////////////////////////////////////////////////////
////
//// --- Utilities and Helpers ---
////
//////////////////////////////////////////////////////
pub fn is_path_healthy(path: &Path, threshold: usize) -> bool {
    let meta = match path.metadata() {
        Ok(m) => m,
        Err(_) => return false,
    };
    if !meta.is_file() {
        return false;
    }
    if meta.len() > threshold as u64 {
        return false;
    }

    true
}

pub fn is_data_line(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.trim();

    // 1. Skip empty or lines starting with non-alphanumeric (comments)
    if trimmed.is_empty() || !trimmed.chars().next().map_or(false, |c| c.is_alphanumeric() || c == '_') {
        return None;
    }

    // 2. Split once, trim the Name, leave the Value raw
    trimmed.split_once('=').map(|(n, v)| (n.trim(), v))
}

fn functional_cmp(a: &str, b: &str) -> bool {
    a.trim_matches('"') == b.trim_matches('"')
}

// Progressively looser checks, can pick up anywhere in the chain
pub fn is_valid_name(name: &str) -> bool {
    // 1. Basic whitespace and emptiness checks
    if name.is_empty() || name.contains(' ') || name.trim() != name { return false; }
    if RESERVED_NAMES.contains(&name.to_uppercase().as_str()) { return false; }
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
#[allow(dead_code)]
fn is_valid_name_ascii(name: &str) -> bool {
    if name.is_empty() { return false; }

    // An alias name with a space is technically "Corrupt"
    // because cmd.exe will never be able to trigger it.
    if name.contains(' ') || name.contains('=') {
        return false;
    }
    // Rugged check for control characters or non-printable ASCII
    name.chars().all(|c| c.is_ascii_graphic())
}

// Broader match to more allow 2 bytes
pub fn is_valid_name_permissive(name: &str) -> bool {
    if name.is_empty() || name.contains(' ') || name.trim() != name { return false; }
    let first = name.chars().next().unwrap();
    if (first.is_alphabetic() || first == '_') && name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
        return is_valid_name_loose(name)
    }
    false
}

// very permissive
pub fn is_valid_name_loose(name: &str) -> bool {
    if name.is_empty() { return false; }
    let first = name.chars().next().unwrap();
    first.is_alphabetic() || first.is_ascii_digit() || first == '_'
}

pub fn get_alias_path(current_file: &str) -> Option<PathBuf> {
    // 1. Priority One: Explicit override (passed from CLI)
    if !current_file.is_empty() {
        let target = PathBuf::from(current_file);
        if is_viable_path(&target) {
            return Some(target);
        }
        // If the explicit file isn't viable, we might want to fail hard,
        // but falling through to Env/Defaults is more resilient.
    }

    // 2. Priority Two: Environment Variable
    if let Ok(val) = env::var(ENV_ALIAS_FILE) {
        let p = PathBuf::from(val);
        let target = if p.is_dir() { p.join(DEFAULT_ALIAS_FILENAME) } else { p };

        if is_viable_path(&target) {
            return Some(target);
        }
    }

    // 3. Priority Three: Standard OS Locations (The Search)
    ["APPDATA", "USERPROFILE"].iter()
        .filter_map(|var| env::var(var).ok().map(PathBuf::from))
        .map(|base| base.join(DEFAULT_APPDATA_ALIAS_DIR).join(DEFAULT_ALIAS_FILENAME))
        .find(|p| {
            if !p.parent().map_or(false, |parent| parent.exists()) {
                return false;
            }
            is_viable_path(p)
        })
}

fn dump_alias_file(verbosity: &Verbosity) -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    let path = get_alias_path("").ok_or_else(|| {
        failure!(verbosity, ErrorCode::MissingFile, "Could not locate the alias configuration file.")
    })?;
    if !is_file_accessible(&path) {
        return Err(failure!(verbosity, ErrorCode::AccessDenied, "File is currently locked by another process."));
    }
    let content = std::fs::read_to_string(path).map_err(|e| failure!(verbosity, e))?;

    let pairs = content.lines()
        .filter_map(is_data_line) // Use the DRY helper
        .map(|(n, v)| (n.to_string(), v.to_string()))
        .collect();

    Ok(pairs)
}

pub fn get_editor_preference(verbosity: &Verbosity, editor: &Option<String>) -> BinaryProfile {
    // 1. Resolve to an owned String first (Fixes E0716)
    let raw_ed = editor.clone()
        .or_else(|| env::var(ENV_VISUAL).ok())
        .or_else(|| env::var(ENV_EDITOR).ok())
        .unwrap_or_else(|| FALLBACK_EDITOR.to_string());

    // drop normalizing issues. shlex and win don't mind /
    let normalized = raw_ed.replace(PATH_SEPARATOR, "/");

    // 2. Disassemble command from args (e.g., "code --wait")
    let args = shlex::split(&normalized)
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| vec![FALLBACK_EDITOR.to_string()]);

    // 3. Resolve the actual EXE path on disk
    let cmd_name = args.get(0).cloned().unwrap_or_else(|| FALLBACK_EDITOR.to_string());
    let exe_path = find_executable(&cmd_name).unwrap_or_else(|| PathBuf::from(&cmd_name));

    // 4. Identify the binary's soul (Subsystem/Bitness)
    let mut profile = identify_binary(verbosity, &exe_path)
        .unwrap_or_else(|_| BinaryProfile::fallback(&exe_path.to_string_lossy()));

    profile.args = args;
    profile
}

pub fn find_executable(name: &str) -> Option<PathBuf> {
    let p = PathBuf::from(name);

    // 1. If it's already a full path or exists locally, stop here.
    if p.is_file() {
        return Some(p);
    }

    // 2. Fetch PATHEXT and PATH
    let pathext_raw = env::var("PATHEXT").unwrap_or_else(|_| DEFAULT_EXTS.to_string());
    let extensions: Vec<&str> = pathext_raw.split(';').collect();

    // 3. The Search Gauntlet (The missing logic)
    if let Ok(path_var) = env::var("PATH") {
        for mut path_node in env::split_paths(&path_var) {
            path_node.push(name);

            // A. Exact match check (e.g., if the user provided "notepad.exe")
            if path_node.is_file() {
                return Some(path_node);
            }

            // B. Extension-appended check (e.g., for "notepad")
            for ext in &extensions {
                // TRAP FIX: Trim the dot so with_extension doesn't create ".."
                let clean_ext = ext.trim_start_matches('.');
                let with_ext = path_node.with_extension(clean_ext);
                if with_ext.is_file() {
                    return Some(with_ext);
                }
            }
        }
    }

    None // Explicitly return None so the caller knows the search failed
}

pub fn normalize_path(path: PathBuf) -> String {
    let s = path.to_string_lossy();

    // Check for the extended UNC prefix first
    if let Some(stripped) = s.strip_prefix(UNC_GLOBAL_PATH) {
        return format!("{}{}", PATH_SEPARATOR, stripped); // Turn \\?\UNC\server into \\server
    }

    // Otherwise just strip the standard extended prefix
    s.strip_prefix(UNC_PATH).unwrap_or(&s).to_string()
}
#[named]
pub fn update_disk_file(verbosity: &Verbosity, name: &str, value: &str, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Load existing data
    let mut pairs = {
        if path.exists() {
            parse_macro_file(path, verbosity)?
        } else {
            Vec::new()
        }
    }; // and DROP THE READ HABDLE

    // 2. Core Logic: Update, Remove, or Append
    if let Some(pos) = pairs.iter().position(|(n, _)| n == name) {
        if value.is_empty() {
            pairs.remove(pos);
        } else {
            pairs[pos].1 = value.to_string();
        }
    } else if !value.is_empty() {
        pairs.push((name.to_string(), value.to_string()));
    }

    // 3. --- TRANSACTIONAL WRITE ---
    let tmp_path = path.with_extension("tmp");

    // Build content string
    let content: String = pairs.iter()
        .map(|(n, v)| format!("{}={}", n, v))
        .collect::<Vec<_>>()
        .join("\n");

    // Attempt the write to temp file
    if let Err(e) = fs::write(&tmp_path, content) {
        return Err(failure!(verbosity, e).into());
    }

    // 4. ATOMIC SWAP
    // If the destination exists, rename will overwrite it on Windows 10/11
    trace!("path={:?}, tpath={:?}", path, tmp_path);
    if path.exists() && !is_file_accessible(path) {
        return Err(failure!(verbosity, ErrorCode::AccessDenied, "Cannot swap: Destination file is locked."));
    }
    if let Err(e) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(failure!(verbosity, e).into());
    }

    Ok(())
}

pub fn parse_macro_file(path: &Path, verbosity: &Verbosity) -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    if !is_file_accessible(path) {
        return Err(failure!(verbosity, ErrorCode::AccessDenied, "Lock detected during parse."));
    }
    let content = fs::read_to_string(path).map_err(|e| failure!(verbosity, e))?;

    let pairs = content.lines()
        .filter_map(is_data_line)
        .filter(|(n, _)| is_valid_name(n)) // Firewall: Drops anything not starting with alpha/underscore
        .map(|(n, v)| (n.to_string(), v.to_string())) // No more .trim() here!
        .collect();

    Ok(pairs)
}

pub fn query_alias_file(name: &str, path: &Path, verbosity: &Verbosity) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    if !is_file_accessible(path) {
        return Ok(vec![format!("Access denied: {} is currently busy.", name)]);
    }
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

pub fn parse_alias_line(line: &str) -> Option<(String, String)> {
    // 1. Clean the nulls from the Win32 buffer
    let line = line.trim_matches('\0');
    if line.is_empty() { return None; }

    // 2. Split on the FIRST equals sign.
    // If the entry is "local_test=echo...", raw_n becomes "\"local_test"
    let (raw_n, raw_v) = match line.split_once('=') {
        Some(pair) => pair,
        None => return None,
    };

    // 3. THE FIX: Only trim whitespace. DO NOT touch quotes.
    // We need the raw identity to match Win32 RAM exactly.
    let name = raw_n
        .trim_matches(|c: char| c.is_whitespace() || c == '\u{00A0}')
        .to_string();

    let value = raw_v
        .trim_matches(|c: char| c.is_whitespace() || c == '\u{00A0}')
        .to_string();

    if name.is_empty() { return None; }
    Some((name, value))
}

pub fn timeout_guard<F, R>(timeout: Duration, f: F) -> Option<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(timeout).ok()
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

pub fn identify_binary(_verbosity: &Verbosity, path: &Path) -> io::Result<BinaryProfile> {
    // 1. Initialize the profile
    let mut profile = BinaryProfile {
        exe: path.to_path_buf(),
        args: Vec::new(),
        subsystem: BinarySubsystem::Cui, // Default
        is_32bit: false,
    };

    // 2. The Gatekeeper (Retry logic/Accessibility)
    if !is_file_accessible(path) {
        profile.subsystem = BinarySubsystem::Unavail;
        return Ok(profile);
    }

    // 3. Consolidated Triage
    if let Some(ext_os) = path.extension() {
        let ext = ext_with_dot(ext_os);
        if is_script_extension(&ext) {
            // Only short-circuit if it's NOT a binary container
            if ext != ".exe" && ext != ".com" {
                profile.subsystem = BinarySubsystem::Script;
                return Ok(profile);
            }
        }
    }

    // 4. Delegate to the Deep Peeker (Only for binaries)
    if let Ok((sub, is_32)) = peek_pe_metadata(path) {
        profile.subsystem = sub;
        profile.is_32bit = is_32;
    }

    Ok(profile)
}

pub fn is_exe_extension(ext: &str) -> bool {
    let e = ext.to_lowercase();
    // Check our hardcoded binary list
    DEFAULT_EXTS_EXE.to_lowercase().split(';').any(|x| x == e)
}

pub fn is_script_extension(ext: &str) -> bool {
    let e_lower = ext.to_lowercase();

    // 1. Get the system list (which usually includes .EXE)
    let pathext = std::env::var("PATHEXT")
        .unwrap_or_else(|_| DEFAULT_EXTS.to_string())
        .to_lowercase();

    // 2. TRUE SCRIPTS = (Anything in PATHEXT) MINUS (Anything in our EXE list)
    pathext.split(';').any(|e| e == e_lower) && !is_exe_extension(&e_lower)
}

pub fn ext_with_dot(ext_os: &std::ffi::OsStr) -> String {
    let s = ext_os.to_string_lossy();
    if s.starts_with('.') {
        s.to_lowercase()
    } else {
        format!(".{}", s.to_lowercase())
    }
}

pub fn peek_pe_metadata(path: &Path) -> io::Result<(BinarySubsystem, bool)> {
    let mut file = File::open(path)?;
    let mut buffer = [0u8; 64];

    // Read DOS Header to find PE offset
    file.read_exact(&mut buffer)?;
    if &buffer[0..2] != b"MZ" {
        return Ok((BinarySubsystem::Unknown, false));
    }

    let pe_offset = u32::from_le_bytes([buffer[60], buffer[61], buffer[62], buffer[63]]) as u64;
    file.seek(SeekFrom::Start(pe_offset))?;

    let mut pe_sig = [0u8; 4];
    file.read_exact(&mut pe_sig)?;
    if &pe_sig != b"PE\0\0" {
        return Ok((BinarySubsystem::Unknown, false));
    }

    // Machine type check (32 vs 64 bit)
    let mut machine_buf = [0u8; 2];
    file.read_exact(&mut machine_buf)?;
    let is_32bit = u16::from_le_bytes(machine_buf) == 0x014c;

    // Subsystem check (GUI vs CUI)
    file.seek(SeekFrom::Current(18 + 68))?;
    let mut sub_buf = [0u8; 2];
    file.read_exact(&mut sub_buf)?;

    let subsystem = match u16::from_le_bytes(sub_buf) {
        2 => BinarySubsystem::Gui,
        3 => BinarySubsystem::Cui,
        _ => BinarySubsystem::Unknown,
    };

    Ok((subsystem, is_32bit))
}

#[named]
pub fn is_drive_responsive(path: &Path, timeout: Duration) -> bool {
    let path_buf = path.to_path_buf();
    trace!("[DRIVE_CHECK] Starting guard for: {:?}", path_buf);

    let result = match timeout_guard(timeout, move || {
        let meta = std::fs::metadata(&path_buf);
        let exists = meta.is_ok();
        trace!("  [GUARD_CLOSURE] Metadata result for {:?}: exists={}", path_buf, exists);
        meta.ok().map(|_| ())
    }) {
        Some(inner_opt) => {
            let success = inner_opt.is_some();
            trace!("  [DRIVE_CHECK] Guard returned in time. Path viable: {}", success);
            success
        }
        None => {
            trace!("  [DRIVE_CHECK] !! TIMEOUT !! Drive unresponsive");
            false
        }
    };

    trace!("[DRIVE_CHECK] Final verdict is {}", result);
    result
}

fn is_path_viable(path: &Path) -> bool {
    // 1. First Check: Is the hardware/OS actually responding for this path?
    // Use your 50ms timeout guard to prevent hangs and catch locks.
    if !is_drive_responsive(path, PATH_RESPONSIVENESS_THRESHOLD) {
        return false;
    }

    // 2. Second Check: Is the file state "healthy"?
    // Now we know the drive is awake and the file isn't hard-locked.
    is_path_healthy(path, MAX_ALIAS_FILE_SIZE)
}
#[named]
pub fn can_path_exist(path: &Path) -> bool {
    let dir_to_check = match path.parent().filter(|p| !p.as_os_str().is_empty()) {
        Some(p) => p.to_path_buf(),
        None => PathBuf::from("."),
    };
    trace!("CPE dir to check is now {:?}", dir_to_check);
    // Flatten the Option<Option<()>>
    timeout_guard(PATH_RESPONSIVENESS_THRESHOLD, move || {
        if !dir_to_check.exists() { return None; }
        std::fs::metadata(&dir_to_check).ok().map(|_| ())
    }).and_then(|inner| inner).is_some() // Use and_then to reach the real answer
}
#[named]
pub fn resolve_viable_path(path: &PathBuf) -> Option<PathBuf> {
    if is_viable_path(path) {
        let clean_str = path.canonicalize()
            .map(|p| normalize_path(p))
            .unwrap_or_else(|_| normalize_path(path.clone()));
        trace!("Resolved to {:?}", clean_str);
        Some(PathBuf::from(clean_str))
    } else {
        trace!("Didn't resolve.");
        None
    }
}
#[named]
fn is_viable_path(path: &Path) -> bool {
    // 1. Force Canonicalization
    // This turns short-names into long-names and validates the route
    let canonical = match path.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            // If it doesn't exist, we can't canonicalize yet.
            // Fall back to the "Can" check for new/mock files.
            trace!("can't canonize {:?}", path);
            return !path.exists() && can_path_exist(path);
        }
    };
    trace!("canonized to {:?}", canonical);
    // 2. If it exists and is canonical, run the Harsh check
    if canonical.exists() {
        is_file_accessible(&canonical)
    } else {
        can_path_exist(&canonical)
    }
}

pub fn is_file_accessible(path: &Path) -> bool {
    let mut retries = 3;

    while retries > 0 {
        if !is_path_viable(path) { retries -= 1; continue; }
        // Attempt to open the file with read permissions
        match std::fs::OpenOptions::new().read(true).open(path) {
            Ok(_) => return true, // File is free and accessible
            Err(e) => {
                match e.raw_os_error() {
                    Some(32) => { // ERROR_SHARING_VIOLATION
                        retries -= 1;
                        if retries > 0 {
                            sleep(PATH_RESPONSIVENESS_THRESHOLD);
                            continue;
                        }
                    }
                    _ => return false, // Path is missing or hard permission error
                }
            }
        }
    }
    false
}

pub fn random_num_bounded(limit: usize) -> usize {
    if limit == 0 { return 0 };

    // 1. Get the time (ms or ns)
    let now = SystemTime::now();
    let time_seed = now.duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos()) // Use nanos, even if "unreliable", it's more jitter than ms
        .unwrap_or(0);

    // 2. Use a static counter to guarantee change in tight loops
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);

    // 3. Mix in the memory address and the unique Thread ID
    let thread_id = std::thread::current().id();
    // We treat the ThreadID as a raw u64/u128
    let thread_seed = unsafe { std::mem::transmute::<std::thread::ThreadId, u64>(thread_id) } as u128;

    // Mix them using XOR and bit-shifting to ensure high/low bits all dance
    let final_seed = time_seed ^ (thread_seed << 32) ^ (count as u128);

    (final_seed % limit as u128) as usize
}

pub fn get_random_tip() -> &'static str {
    // 2. Use the global constant here
    let random_seed = random_num_bounded(TIPS_ARRAY.len());
    TIPS_ARRAY[random_seed]
}

pub fn random_tip_show() -> Option<&'static str> {
    let seed = random_num_bounded(usize::MAX);
    if seed % 10 == 0 {
        return Some(get_random_tip());
    }
    None
}

pub fn get_alias_exe() -> io::Result<std::path::PathBuf> {
    std::env::current_exe().map_err(|e| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("OS Error: Unable to locate the alias executable path: {}", e)
        )
    })
}

fn get_alias_exe_nofail(verbosity: &Verbosity) -> String {
    match get_alias_exe() {
        Ok(p) => p.file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "alias".to_string()),
        Err(e) => {
            // Observe and Report: Scream the issue but don't stop the train
            scream!(verbosity, AliasIcon::Scream, "Path Resolution Failed: {}", e);
            "alias".to_string()
        }
    }
}

