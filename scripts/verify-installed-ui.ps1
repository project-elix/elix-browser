param(
  [Parameter(Mandatory=$true)][string]$Installer,
  [Parameter(Mandatory=$true)][string]$OutputDirectory,
  [string]$InstallDirectory = (Join-Path $env:RUNNER_TEMP 'ELIX-signed-ui-review')
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient,UIAutomationTypes,System.Windows.Forms,System.Drawing
New-Item -ItemType Directory -Force $OutputDirectory | Out-Null
function Save-Desktop([string]$Name) {
  $screen = [Windows.Forms.Screen]::PrimaryScreen.Bounds
  $image = [Drawing.Bitmap]::new($screen.Width,$screen.Height)
  $graphics = [Drawing.Graphics]::FromImage($image)
  try {
    $graphics.CopyFromScreen($screen.Location,[Drawing.Point]::Empty,$screen.Size)
    $image.Save((Join-Path $OutputDirectory $Name),[Drawing.Imaging.ImageFormat]::Png)
  } finally { $graphics.Dispose(); $image.Dispose() }
}
$dest = $InstallDirectory
$install = Start-Process $Installer -ArgumentList @('/S', "/D=$dest") -PassThru
if (!$install.WaitForExit(120000) -or $install.ExitCode -ne 0) { throw 'Installer failed' }
$app = Start-Process "$dest/elix-browser.exe" -PassThru
$clock = [Diagnostics.Stopwatch]::StartNew()
$ready = $false
try {
  $addressCondition = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,'Adressefelt')
  $searchCondition = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,('S'+[char]0x00f8+'k fra startsiden'))
  while ($clock.Elapsed.TotalSeconds -lt 120) {
    $app.Refresh()
    if ($app.HasExited) { throw 'Production browser exited during startup' }
    if ($app.MainWindowHandle -ne [IntPtr]::Zero) {
      $window = [Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
      $address = $window.FindFirst([Windows.Automation.TreeScope]::Descendants,$addressCondition)
      $search = $window.FindFirst([Windows.Automation.TreeScope]::Descendants,$searchCondition)
      if ($address -and $search -and $address.Current.IsEnabled -and !$address.Current.IsOffscreen) {
        $ready = $true
        $elements = $window.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)
        @($elements | ForEach-Object { $_.Current.Name }) | Set-Content -Encoding UTF8 (Join-Path $OutputDirectory 'accessible-names.txt')
        break
      }
    }
    Save-Desktop 'startup-latest.png'
    Start-Sleep -Seconds 2
  }
  Save-Desktop 'signed-ready.png'
  if (!$ready) { throw 'Address bar and start-page search were not usable within 120 seconds' }
  @{ready=$ready;seconds=$clock.Elapsed.TotalSeconds;product_version=(Get-Item "$dest/elix-browser.exe").VersionInfo.ProductVersion} | ConvertTo-Json | Set-Content (Join-Path $OutputDirectory 'ready-result.json')
  Write-Output "PASS: installed production browser address bar and start-page search are usable after $([Math]::Round($clock.Elapsed.TotalSeconds,1)) seconds"
} finally {
  Save-Desktop 'startup-final.png'
  if (!$app.HasExited) {
    [void]$app.CloseMainWindow()
    if (!$app.WaitForExit(15000)) { Stop-Process -Id $app.Id -Force; $app.WaitForExit() }
  }
}
