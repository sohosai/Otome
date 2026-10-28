@echo off
setlocal EnableExtensions EnableDelayedExpansion

set "OTOME_EXE=%~dp0otome.exe"
if not exist "!OTOME_EXE!" set "OTOME_EXE=%~dp0target\release\otome.exe"
if not exist "!OTOME_EXE!" set "OTOME_EXE=%~dp0target\debug\otome.exe"

if not exist "!OTOME_EXE!" (
  echo otome.exe not found next to start.cmd or in target\release / target\debug.
  pause
  exit /b 1
)

set "NDI_FOUND="
for %%D in (
  "%~dp0."
  "%NDI_RUNTIME_DIR_V6%"
  "%NDI_RUNTIME_DIR_V5%"
  "C:\Program Files\NDI\NDI 6 Runtime\v6"
  "C:\Program Files\NDI\NDI 6 Tools\Runtime"
  "C:\Program Files\NDI\NDI 6 SDK\Bin\x64"
  "%~dp0target\release\deps"
  "%~dp0target\debug\deps"
) do (
  if not defined NDI_FOUND if not "%%~D"=="" if exist "%%~D\Processing.NDI.Lib.x64.dll" (
    set "PATH=%%~D;!PATH!"
    set "NDI_FOUND=1"
  )
)

if not defined NDI_FOUND (
  where Processing.NDI.Lib.x64.dll >nul 2>&1
  if errorlevel 1 (
    echo NDI Runtime not found.
    echo Install NDI Tools from https://ndi.video/tools/ and try again.
    pause
    exit /b 1
  )
)

"!OTOME_EXE!" --config ".\otome.toml" %*
exit /b %ERRORLEVEL%
