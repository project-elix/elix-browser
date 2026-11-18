# The two normal installers use their real Windows paths on a clean hosted runner.
param(
  [Parameter(Mandatory=$true)][string]$Installer,
  [switch]$RequireSignature
)
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:ELIX_RUNNER_ENVIRONMENT -ne 'github-hosted') {
  throw 'Use a disposable GitHub-hosted Windows runner.'
}
$version = (Get-Content 'package.json' -Raw | ConvertFrom-Json).version
if ($env:RELEASE_VERSION -and $env:RELEASE_VERSION -ne $version) { throw 'Candidate version differs from release request.' }
if (Test-Path "$Installer.sig") {
  node scripts/verify-installer-signature.mjs $Installer $version
  if ($LASTEXITCODE -ne 0) { throw 'Candidate installer signature verification failed.' }
} elseif ($RequireSignature) {
  throw 'Missing updater signature for the candidate installer.'
}
# Verify that upgrading across channels preserves the user's existing choice.
$existingChannel = if ($version -match '-dev\.') { 'stable' } else { 'developer' }
$root = Join-Path $env:LOCALAPPDATA 'no.projectelix.browser'
$out = Join-Path $env:RUNNER_TEMP 'profile-upgrade-review'
$installDir = Join-Path $env:RUNNER_TEMP 'ELIX-profile-upgrade-install'
if ((Test-Path $root) -or (Test-Path $out) -or (Test-Path $installDir)) {
  throw 'The runner already contains an ELIX profile or qualification directory.'
}
New-Item -ItemType Directory -Path $out | Out-Null
$old = Join-Path $out 'alpha9.exe'
$baseline = 'https://github.com/project-elix/elix-browser/releases/download/v0.1.4-dev.9/ELIX.Browser_0.1.4-dev.9_x64-setup.exe'
Invoke-WebRequest -Uri $baseline -OutFile $old
if ((Get-FileHash $old -Algorithm SHA256).Hash -ne 'bc03406019e6eb767aee5c571165f43a9b5e212a5b7ca2b30b72d96d4f3f6a92') {
  throw 'Published Alpha 9 installer digest mismatch.'
}
./scripts/verify-installed-ui.ps1 -Installer $old -OutputDirectory "$out/alpha9" -InstallDirectory $installDir
python scripts/stable1-profile.py seed $root --channel $existingChannel
if ($LASTEXITCODE -ne 0) { throw 'Unable to seed the Alpha 9 profile.' }
./scripts/verify-installed-ui.ps1 -Installer $Installer -OutputDirectory "$out/candidate" -InstallDirectory $installDir -VerifyReleaseDownloads
$installed = (Get-FileHash "$installDir/elix-browser.exe").Hash
$expected = (Get-FileHash 'src-tauri/target/release/elix-browser.exe').Hash
if ($installed -ne $expected) { throw 'Installed executable differs from the candidate build.' }
python scripts/stable1-profile.py verify $root --channel $existingChannel
if ($LASTEXITCODE -ne 0) { throw 'Upgrading the existing Alpha 9 profile failed.' }
$evidence = [ordered]@{
  source_sha = $env:GITHUB_SHA
  run_id = $env:GITHUB_RUN_ID
  run_attempt = $env:GITHUB_RUN_ATTEMPT
  installer_sha256 = (Get-FileHash $Installer -Algorithm SHA256).Hash
  exe_sha256 = $installed
  baseline = '0.1.4-dev.9'
  version = $version
  preserved_channel = $existingChannel
  profile_upgrade = 'passed'
  elix_34_github_release_and_fresh_cdn = 'passed'
}
if (Test-Path "$Installer.sig") {
  $evidence.signature = (Get-Content "$Installer.sig" -Raw).Trim()
}
$evidence | ConvertTo-Json | Set-Content -Encoding UTF8 "$out/profile-upgrade.json"
# Leave the following clean-install check an empty machine. Keep the synthetic
# upgraded profile with its screenshots in the isolated temp directory.
$remove = Start-Process "$installDir/uninstall.exe" -ArgumentList @('/S', "_?=$installDir") -PassThru
if (!$remove.WaitForExit(120000) -or $remove.ExitCode -ne 0) { throw 'Test uninstall failed.' }
if (Test-Path $root) { Move-Item $root "$out/upgraded-profile" }
