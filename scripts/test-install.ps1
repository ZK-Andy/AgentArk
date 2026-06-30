$ErrorActionPreference = "Stop"

$scriptPath = Join-Path $PSScriptRoot "install.ps1"
$scriptText = Get-Content -Raw -Path $scriptPath
$tokens = $null
$errors = $null
$ast = [System.Management.Automation.Language.Parser]::ParseInput($scriptText, [ref]$tokens, [ref]$errors)
if ($errors.Count -gt 0) {
    throw "install.ps1 has parse errors: $($errors[0].Message)"
}

$functionAst = $ast.Find({
    param($node)
    $node -is [System.Management.Automation.Language.FunctionDefinitionAst] -and
        $node.Name -eq "Test-AgentArkDockerEngine"
}, $true)
if (-not $functionAst) {
    throw "Test-AgentArkDockerEngine was not found"
}

$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("agentark-install-test-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $tmp | Out-Null
$oldPath = $env:PATH
try {
    Set-Content -Path (Join-Path $tmp "docker.cmd") -Encoding ASCII -Value @"
@echo off
echo fake docker daemon error 1>&2
exit /b 1
"@
    $env:PATH = "$tmp;$env:PATH"
    Invoke-Expression $functionAst.Extent.Text

    $result = Test-AgentArkDockerEngine
    if ($result -ne $false) {
        throw "Expected Test-AgentArkDockerEngine to return false for a failing docker daemon"
    }
} finally {
    $env:PATH = $oldPath
    Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Host "install.ps1 regression tests passed"
