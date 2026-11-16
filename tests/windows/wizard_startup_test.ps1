param(
    [Parameter(Mandatory = $true)][string]$WizardPath,
    [Parameter(Mandatory = $true)][string]$BinaryPath
)
$ErrorActionPreference = 'Stop'
$WizardPath = (Resolve-Path -LiteralPath $WizardPath).Path
$BinaryPath = (Resolve-Path -LiteralPath $BinaryPath).Path

function Invoke-WizardSmoke([string]$Path, [string]$Override) {
    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = $env:ComSpec
    $info.Arguments = '/d /c ""' + $Path + '""'
    $info.WorkingDirectory = Split-Path -Parent $Path
    $info.UseShellExecute = $false
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.EnvironmentVariables.Remove('RUNTIMEPACK_EXE')
    if ($Override) { $info.EnvironmentVariables['RUNTIMEPACK_EXE'] = $Override }
    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $info
    [void]$process.Start()
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    $process.StandardInput.WriteLine('0')
    $process.StandardInput.Close()
    if (-not $process.WaitForExit(10000)) {
        $process.Kill()
        throw 'Wizard startup exceeded 10 seconds.'
    }
    $result = [pscustomobject]@{
        ExitCode = $process.ExitCode
        Output = $stdout.Result + $stderr.Result
    }
    $process.Dispose()
    return $result
}

$normal = Invoke-WizardSmoke $WizardPath ''
if ($normal.ExitCode -ne 0 -or $normal.Output -notmatch 'RUNTIMEPACK ANDROID BUILD WIZARD' -or $normal.Output -match 'ERR-WIZ-001') {
    throw "Bundle startup failed (exit $($normal.ExitCode)):`n$($normal.Output)"
}
Write-Output 'PASS: Bundle binary reaches menu and exits normally.'

$override = Invoke-WizardSmoke $WizardPath $BinaryPath
if ($override.ExitCode -ne 0 -or $override.Output -notmatch 'RUNTIMEPACK ANDROID BUILD WIZARD') {
    throw "Explicit binary startup failed:`n$($override.Output)"
}
Write-Output 'PASS: RUNTIMEPACK_EXE reaches menu.'

$tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ('runtimepack-wizard-smoke-' + [guid]::NewGuid().ToString('N'))
[void][System.IO.Directory]::CreateDirectory($tempRoot)
$missingWizard = Join-Path $tempRoot 'RuntimePack-Wizard.bat'
Copy-Item -LiteralPath $WizardPath -Destination $missingWizard
$wizardPy = Join-Path (Split-Path -Parent $WizardPath) 'wizard.py'
if (Test-Path -LiteralPath $wizardPy) {
    Copy-Item -LiteralPath $wizardPy -Destination (Join-Path $tempRoot 'wizard.py')
}
$missing = Invoke-WizardSmoke $missingWizard ''
if ($missing.ExitCode -ne 1 -or $missing.Output -notmatch 'ERR-WIZ-001' -or $missing.Output -notmatch '1\.' -or $missing.Output -notmatch '2\.' -or $missing.Output -match 'RUNTIMEPACK ANDROID BUILD WIZARD') {
    throw "Missing-binary diagnostic failed:`n$($missing.Output)"
}
Write-Output 'PASS: Missing binary prints complete diagnostic and exits 1.'
Write-Output "Synthetic fixture retained at: $tempRoot"
