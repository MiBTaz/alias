// tests/shared_test_utils.rs

#[allow(unused_imports)]
use std::sync::Mutex;
#[allow(unused_imports)]
use std::io;
#[allow(unused_imports)]
use std::path::Path;
#[allow(unused_imports)]
use alias_lib::{AliasProvider, SetOptions, Verbosity, PurgeReport};
#[allow(unused_imports)]
use lazy_static::lazy_static;
use alias_lib::ProviderType;

// 1. SHARED MOCK STATE
lazy_static! {
    #[allow(dead_code)]
    pub static ref LAST_CALL: Mutex<Option<SetOptions>> = Mutex::new(None);
    #[allow(dead_code)]
    pub static ref MOCK_RAM: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());
}

// 2. SHARED INITIALIZATION LOGIC
pub fn global_test_setup() {
    unsafe {
        std::env::remove_var("ALIAS_FILE");
        std::env::remove_var("ALIAS_OPTS");
        std::env::remove_var("ALIAS_PATH");
    }
}

// 3. SHARED MOCK PROVIDER
#[allow(dead_code)]
pub fn get_captured_set() -> SetOptions {
    LAST_CALL.lock()
        .expect("Mutex poisoned")
        .take() // Clears it for the next test
        .expect("The dispatcher never called the provider!")
}

#[allow(dead_code)]
pub struct MockProvider;
// Create an alias to the actual provider being tested
// This allows the test file to just refer to "P"

#[allow(unused_imports)]
#[cfg(feature = "identity_win32")]
pub use alias_win32::Win32Provider as P;

#[allow(unused_imports)]
#[cfg(feature = "identity_wrapper")]
pub use alias_wrapper::WrapperLibraryInterface as P;

#[allow(unused_imports)]
#[cfg(feature = "identity_hybrid")]
pub use alias_hybrid::WrapperLibraryInterface as P;

#[allow(dead_code)]
impl AliasProvider for MockProvider {
    // 1. ATOMIC HANDS
    fn raw_set_macro(name: &str, value: Option<&str>) -> io::Result<bool> {
        let mut ram = MOCK_RAM.lock().unwrap();
        if value.is_none() {
            ram.retain(|(k, _)| k != name);
        } else {
            ram.push((name.to_string(), value.unwrap().to_string()));
        }
        Ok(true)
    }

    // This now returns the ACTUAL state of your fake system
    fn get_all_aliases(_: &Verbosity) -> io::Result<Vec<(String, String)>> {
        let ram = MOCK_RAM.lock().unwrap();
        Ok(ram.clone())
    }
    // MATCH: Path instead of str
    fn raw_reload_from_file(_: &Verbosity, _: &std::path::Path) -> io::Result<()> { Ok(()) }
    fn reload_full(_verbosity: &Verbosity, _file_path: &Path, _force: bool) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
    fn write_autorun_registry(_: &str, _: &Verbosity) -> io::Result<()> { Ok(()) }
    fn purge_ram_macros(v: &Verbosity) -> Result<PurgeReport, std::io::Error> {
        let mut report = PurgeReport::default();
        let aliases = Self::get_all_aliases(v)?;
        for (name, _) in aliases {
            Self::raw_set_macro(&name, None)?;
            report.cleared.push(name);
        }
        Ok(report)
    }
    fn purge_file_macros(_: &Verbosity, _: &Path) -> Result<PurgeReport, std::io::Error> { Ok(PurgeReport::default()) }
    // MATCH: Returns String directly, not Result
    fn read_autorun_registry() -> String { String::new() }

    // 2. REQUIRED TRAIT METHODS
    // MATCH: Returns Vec<String>, not Result
    fn query_alias(_: &str, _: &Verbosity) -> Vec<String> { vec![] }

    // MATCH: Param 1 is SetOptions, Param 2 is &Path
    fn set_alias(opts: SetOptions, _path: &Path, _v: &Verbosity) -> io::Result<()> {
        let mut call = LAST_CALL.lock().unwrap();
        *call = Some(opts); // This records the work dispatch did
        Ok(())
    }

    // MATCH: &Path and Result<(), Box<dyn Error>>
    fn run_diagnostics(_: &std::path::Path, v: &Verbosity) -> Result<(), Box<dyn std::error::Error>> {
        // If the test expects to see "WRITABLE", the provider MUST write it!
        v.say("✅ WRITABLE");
        Ok(())
    }

    // MATCH: Result<(), Box<dyn Error>>
    fn alias_show_all(_: &Verbosity) -> Result<(), Box<dyn std::error::Error>> { Ok(()) }

    fn install_autorun(_v: &Verbosity, _payload: &str) -> io::Result<()> { Ok(()) }

    fn provider_type() -> ProviderType {
        if cfg!(feature = "identity_wrapper") {
            ProviderType::Wrapper
        } else if cfg!(feature = "identity_hybrid") {
            ProviderType::Hybrid
        } else if cfg!(feature = "identity_win32") {
            ProviderType::Win32
        } else {
            ProviderType::NotLinked
        }
    }
}




