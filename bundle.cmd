@echo off
setlocal
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul
set TEMP=%LOCALAPPDATA%\Temp
set TMP=%LOCALAPPDATA%\Temp
set PATH=%USERPROFILE%\.cargo\bin;%PATH%
cd /d C:\Users\Lenovo\Desktop\mlai-gui
npx tauri build --ci
