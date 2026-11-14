$ErrorActionPreference = 'Stop'
$executable = (Resolve-Path (Join-Path $PSScriptRoot '../target/release/localdrop.exe')).Path
$process = Start-Process -FilePath $executable -WindowStyle Hidden -PassThru
try {
    $listening = $false
    for ($attempt = 0; $attempt -lt 40; $attempt++) {
        $process.Refresh()
        if ($process.HasExited) { throw "LocalDrop exited during startup: $($process.ExitCode)" }
        $listener = Get-NetTCPConnection -State Listen -LocalPort 45831 -ErrorAction SilentlyContinue |
            Where-Object OwningProcess -eq $process.Id
        if ($listener) { $listening = $true; break }
        Start-Sleep -Milliseconds 250
    }
    if (-not $listening) { throw 'LocalDrop failed to start its local server (possible loader or GUI initialization error).' }
    $controls = $process.Modules | Where-Object ModuleName -ieq 'comctl32.dll'
    if (-not $controls -or $controls.FileName -notmatch '(?i)\\WinSxS\\') {
        throw 'Common Controls v6 was not loaded from WinSxS.'
    }
    Write-Output "Startup passed: native UI initialized, Common Controls v6 loaded, TCP 45831 listening."
} finally {
    if (-not $process.HasExited) { Stop-Process -Id $process.Id -Force }
}
