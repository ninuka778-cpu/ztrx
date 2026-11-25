# ztrx launcher: runs the game for normal play (no scripted presses, no capture).
param(
    [Parameter(Mandatory = $true)][string]$Game,
    [string]$Cache = "$env:LOCALAPPDATA\ztrx\cache",
    [string]$Data = "$env:LOCALAPPDATA\ztrx\data",
    [string]$Exe = "$PSScriptRoot\..\target\release\victory.exe",
    [ValidateSet('performance', 'balanced', 'compatibility')]
    [string]$Performance = 'performance',
    [string]$Resolution = ''
)
foreach ($v in "VICTORY_KEYS", "VICTORY_CAPTURE", "VICTORY_CAPTURE_FRAMES", "VICTORY_DUMP_TARGETS", "VICTORY_GPU_TIMING") {
    Remove-Item "Env:$v" -ErrorAction SilentlyContinue
}
if ($Resolution -and $Resolution -notmatch '^\d+[xX]\d+$') {
    throw "-Resolution must look like 1280x720"
}
if ($PSBoundParameters.ContainsKey('Performance')) {
    $env:VICTORY_PERFORMANCE = $Performance
} elseif (-not $env:VICTORY_PERFORMANCE) {
    $env:VICTORY_PERFORMANCE = $Performance
}
if ($Resolution) {
    $env:VICTORY_DISPLAY_MODE = $Resolution
}
New-Item -ItemType Directory -Force $Cache, $Data | Out-Null
$env:VICTORY_CACHE_DIR = $Cache
$env:VICTORY_DATA_DIR = $Data
Write-Host "ztrx: performance=${env:VICTORY_PERFORMANCE}; resolution override=$(if (${env:VICTORY_DISPLAY_MODE}) { ${env:VICTORY_DISPLAY_MODE} } else { '(profile default)' })"
& $Exe run $Game --execute 2>&1 | Out-File "$env:TEMP\ztrx.log" -Encoding utf8
Write-Host "Exited with $LASTEXITCODE; log: $env:TEMP\ztrx.log"
