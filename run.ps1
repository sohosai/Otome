$runtimeCandidates = @(
    $env:NDI_RUNTIME_DIR_V6,
    $env:NDI_RUNTIME_DIR_V5,
    'C:\Program Files\NDI\NDI 6 Runtime\v6',
    'C:\Program Files\NDI\NDI 6 Tools\Runtime',
    'C:\Program Files\NDI\NDI 6 SDK\Bin\x64',
    (Join-Path $PSScriptRoot 'target\release\deps'),
    (Join-Path $PSScriptRoot 'target\debug\deps')
)

$runtimePaths = @()
foreach ($candidate in $runtimeCandidates) {
    if (-not $candidate) {
        continue
    }
    if (Test-Path (Join-Path $candidate 'Processing.NDI.Lib.x64.dll')) {
        $runtimePaths += $candidate
    }
}

if ($runtimePaths.Count -gt 0) {
    $env:PATH = (($runtimePaths -join ';') + ';' + $env:PATH)
}

$exe = Join-Path $PSScriptRoot 'otome.exe'
if (-not (Test-Path $exe)) {
    $exe = Join-Path $PSScriptRoot 'target\release\otome.exe'
}
if (-not (Test-Path $exe)) {
    $exe = Join-Path $PSScriptRoot 'target\debug\otome.exe'
}

& $exe @args
exit $LASTEXITCODE
