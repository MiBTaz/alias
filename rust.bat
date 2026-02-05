@echo off
set "BASE_DIR=%~dp0"
cd /d "%BASE_DIR%"
@echo off
setlocal
:: Set the policy song and dance for the current process tree
set "PSExecutionPolicyPreference=Bypass"
set "REGBACK=%TEMP%\autorun_backup.xml"

powershell -Command "Get-ChildItem alias_lib,alias_wrapper,alias_win32,alias_hybrid -Recurse -File | ForEach-Object { '///////////////////////////////////'; '///// ' + $_.FullName; '///////////////////////////////////'; Get-Content $_.FullName }" > \tmp\alias.txt

:: Simple batch to reset the mutex reliably for test runs.
:: not only do the tests need a reset, so does auto run. since the tests can't reliably
:: restore the entry, needs to be done pre and post. 
echo Resetting semaphonre . . .
powershell -Command "$p='HKCU:\Software\Microsoft\Command Processor'; if(!(Test-Path '%REGBACK%')){Get-Item $p | Export-Clixml '%REGBACK%'}; $s='HKCU:\Software\AliasTool\Backup'; if(Test-Path $s){Remove-Item $s -Recurse -Force}; New-Item $s -Force; New-ItemProperty $s -Name 'ActiveCount' -Value 0 -PropertyType DWord"

echo Running cargo check . . .
cargo check
echo [RUNNER] Starting Cargo %*...
cargo %*
set "EXIT_VAL=%errorlevel%"

:: reg restore
if exist "%REGBACK%" (
    powershell -Command "$saved=Import-Clixml '%REGBACK%'; Set-ItemProperty 'HKCU:\Software\Microsoft\Command Processor' -Name 'AutoRun' -Value $saved.Property.AutoRun -Force; Remove-Item '%REGBACK%'"
)


exit /b %EXIT_VAL%
