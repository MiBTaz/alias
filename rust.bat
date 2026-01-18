@echo off
:: Simple batch to reset the mutex reliably for test runs.
:: The reason for tis necessity is rust has no post run system,
:: so if the a test fails, the dstor doesn't run. si the mutex
:: needs ro be reset reliably outside of thr build system.
echo Resetting semaphonre . . .
 reg add "HKCU\Software\AliasTool\Backup" /v ActiveCount /t REG_DWORD /d 0 /f >nul 2>&1
:: Ensure the path exists so reg add doesn't complain, then zero the count
echo ensuring base registery entry exists . . .
reg add "HKCU\Software\AliasTool\Backup" /v ActiveCount /t REG_DWORD /d 0 /f >nul 2>&1
cargo %*


