param(
  [Parameter(Mandatory=$true)][string]$Installer,
  [Parameter(Mandatory=$true)][string]$OutputDirectory,
  [string]$InstallDirectory = (Join-Path $env:RUNNER_TEMP 'ELIX-signed-ui-review'),
  [switch]$VerifyStable1Downloads
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
  if ($VerifyStable1Downloads) {
    $links = python scripts/stable1-download.py links
    if ($LASTEXITCODE -ne 0) { throw 'Could not obtain a valid fresh GitHub release asset' }
    # Send each chord in one Win32 input packet. WinForms SendKeys can deliver
    # a bare "v" when its modifier events overlap WebView2 focus changes.
    Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
public static class ElixAcceptanceKeys {
  [StructLayout(LayoutKind.Sequential)]
  struct Keyboard { public ushort key, scan; public uint flags, time; public UIntPtr extra; }
  [StructLayout(LayoutKind.Sequential)]
  struct Mouse { public int x, y; public uint data, flags, time; public UIntPtr extra; }
  [StructLayout(LayoutKind.Explicit)]
  struct Payload { [FieldOffset(0)] public Mouse mouse; [FieldOffset(0)] public Keyboard keyboard; }
  [StructLayout(LayoutKind.Sequential)]
  struct Input { public uint type; public Payload payload; }
  [DllImport("user32.dll", SetLastError=true)]
  static extern uint SendInput(uint count, Input[] inputs, int size);
  static Input Key(ushort key, bool released) {
    return new Input { type=1, payload=new Payload { keyboard=new Keyboard { key=key, flags=released ? 2u : 0u } } };
  }
  public static void ReplaceFromClipboard() {
    Input[] keys = { Key(0x11,false), Key(0x41,false), Key(0x41,true),
                     Key(0x56,false), Key(0x56,true), Key(0x11,true) };
    if (SendInput((uint)keys.Length, keys, Marshal.SizeOf<Input>()) != keys.Length)
      throw new Win32Exception(Marshal.GetLastWin32Error());
  }
}
'@
    $shell = New-Object -ComObject WScript.Shell
    $profileRoot = Join-Path $env:LOCALAPPDATA 'no.projectelix.browser'
    $count = 0
    foreach ($url in ($links | ConvertFrom-Json)) {
      $count++
      if (!$shell.AppActivate($app.Id)) { throw 'Could not focus the installed browser' }
      $address = $window.FindFirst([Windows.Automation.TreeScope]::Descendants, $addressCondition)
      Set-Clipboard -Value $url
      $value = $address.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern)
      $readyToSubmit = $false
      for ($attempt = 1; $attempt -le 3 -and !$readyToSubmit; $attempt++) {
        $address.SetFocus()
        $limit = [DateTime]::UtcNow.AddSeconds(3)
        while (!$address.Current.HasKeyboardFocus -and [DateTime]::UtcNow -lt $limit) { Start-Sleep -Milliseconds 100 }
        if (!$address.Current.HasKeyboardFocus) { continue }
        [ElixAcceptanceKeys]::ReplaceFromClipboard()
        $limit = [DateTime]::UtcNow.AddSeconds(3)
        while ($value.Current.Value -ne $url -and [DateTime]::UtcNow -lt $limit) { Start-Sleep -Milliseconds 100 }
        $readyToSubmit = $value.Current.Value -ceq $url
      }
      if (!$readyToSubmit) { throw "Could not enter complete asset address $count in the focused field" }
      [Windows.Forms.SendKeys]::SendWait('{ENTER}')
      python scripts/stable1-download.py verify $profileRoot $count
      if ($LASTEXITCODE -ne 0) { throw "Installed browser download $count failed" }
      $failedAttachment = [Windows.Automation.PropertyCondition]::new(
        [Windows.Automation.AutomationElement]::NameProperty,
        ('Siden kunne ikke lastes inn (nettverksfeil 9). Pr'+[char]0x00f8+'v igjen.'))
      $limit = [DateTime]::UtcNow.AddSeconds(4)
      do {
        $banner = $window.FindFirst([Windows.Automation.TreeScope]::Descendants, $failedAttachment)
        if (!$banner) { break }
        Start-Sleep -Milliseconds 200
      } while ([DateTime]::UtcNow -lt $limit)
      if ($banner) { throw 'A completed attachment still shows a navigation failure in ELIX' }

    }
    if ($count -ne 2) { throw 'Both the release link and fresh CDN link must pass' }
    Write-Output 'PASS: ordinary address-bar navigation downloaded both the GitHub release and its fresh signed CDN URL; both file digests matched.'
  }

} finally {
  Save-Desktop 'startup-final.png'
  if (!$app.HasExited) {
    [void]$app.CloseMainWindow()
    if (!$app.WaitForExit(15000)) { Stop-Process -Id $app.Id -Force; $app.WaitForExit() }
  }
}
