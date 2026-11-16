@echo off
setlocal
title RuntimePack - Interactive Build Wizard

where python >nul 2>&1
if %ERRORLEVEL% EQU 0 goto USE_PYTHON

where py >nul 2>&1
if %ERRORLEVEL% EQU 0 goto USE_PY

echo ===============================================================================
echo [ERROR]: Python 3 tidak ditemukan di sistem Anda!
echo ===============================================================================
echo.
echo RuntimePack Wizard memerlukan Python 3 untuk antarmuka interaktif.
echo Silakan unduh dan instal Python dari: https://www.python.org/downloads/
echo (Pastikan centang "Add python.exe to PATH" saat menginstal).
echo.
echo Tekan sembarang tombol untuk keluar...
pause >nul
exit /b 1

:USE_PYTHON
python "%~dp0wizard.py" %*
exit /b %ERRORLEVEL%

:USE_PY
py "%~dp0wizard.py" %*
exit /b %ERRORLEVEL%
