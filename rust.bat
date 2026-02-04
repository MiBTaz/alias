@echo off
set "BASE_DIR=%~dp0"
cd /d "%BASE_DIR%"
set "split1=///////////////////////////////////"
set "split2=/////"
echo Aggregate src . . .
\bin\find alias_lib alias_wrapper alias_win32 alias_hybrid -type f -exec echo %split1% ^; -exec echo -E "%split2%    {}" ^;  -exec echo %split1% ^; -exec cat {} ^; -exec \bin\sync ^; > \tmp\alias.txt 2>nul

:: Simple batch to reset the mutex reliably for test runs.
:: The reason for tis necessity is rust has no post run system,
:: so if the a test fails, the dstor doesn't run. si the mutex
:: needs ro be reset reliably outside of thr build system.
echo Resetting semaphonre . . .
 reg add "HKCU\Software\AliasTool\Backup" /v ActiveCount /t REG_DWORD /d 0 /f >nul 2>&1
:: Ensure the path exists so reg add doesn't complain, then zero the count
echo ensuring base registery entry exists . . .
reg add "HKCU\Software\AliasTool\Backup" /v ActiveCount /t REG_DWORD /d 0 /f >nul 2>&1
echo Running cargo check . . .
cargo check
cargo %*


