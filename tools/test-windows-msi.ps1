param(
  [Parameter(Mandatory)][string]$SourceRoot,
  [Parameter(Mandatory)][string]$MsiPath,
  [Parameter(Mandatory)][string]$ExpectedVersion
)
$ErrorActionPreference = 'Stop'
if ($env:CI -ne 'true') { throw 'Run installer smoke checks only on an isolated CI runner.' }
$SourceRoot = (Resolve-Path $SourceRoot).Path
$MsiPath = (Resolve-Path $MsiPath).Path
$work = Join-Path $env:RUNNER_TEMP 'crown-msi-smoke'
New-Item -ItemType Directory -Force $work | Out-Null
$installer = New-Object -ComObject WindowsInstaller.Installer
$database = $installer.GetType().InvokeMember('OpenDatabase', 'InvokeMethod', $null, $installer, @($MsiPath, 0))
function Get-MsiProperty([string]$name) {
  $query = "SELECT ``Value`` FROM ``Property`` WHERE ``Property`` = '$name'"
  $view = $database.GetType().InvokeMember('OpenView', 'InvokeMethod', $null, $database, @($query))
  $view.GetType().InvokeMember('Execute', 'InvokeMethod', $null, $view, @()) | Out-Null
  $record = $view.GetType().InvokeMember('Fetch', 'InvokeMethod', $null, $view, @())
  return $record.GetType().InvokeMember('StringData', 'GetProperty', $null, $record, @(1))
}
$product = Get-MsiProperty 'ProductCode'
if ((Get-MsiProperty 'ProductVersion') -ne $ExpectedVersion) { throw 'MSI version does not match the tag.' }
if ((Get-MsiProperty 'UpgradeCode') -ne '{BD9D3915-B839-48C7-AE79-AFBF60A6CD78}') { throw 'MSI upgrade identity changed.' }
Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class CrownMsiInfo {
  [DllImport("msi.dll", CharSet=CharSet.Unicode)]
  static extern uint MsiGetProductInfo(string code, string property, StringBuilder value, ref uint length);
  public static string Get(string code, string property) {
    var value = new StringBuilder(32768); uint length = 32768;
    uint result = MsiGetProductInfo(code, property, value, ref length);
    if (result != 0) throw new Exception("MSI property query failed: " + result);
    return value.ToString();
  }
}
'@
function Invoke-Msi([string]$mode, [string]$target, [string]$name) {
  $log = Join-Path $work "$name.log"
  $p = Start-Process "$env:WINDIR/System32/msiexec.exe" -ArgumentList @($mode, "`"$target`"", '/qn', '/norestart', '/l*v', "`"$log`"") -Wait -PassThru
  if ($p.ExitCode -notin @(0, 3010)) { throw "Windows Installer $name failed: $($p.ExitCode)" }
}
function Test-Launch([string]$path) {
  $p = Start-Process -FilePath $path -PassThru
  try {
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    while ([DateTime]::UtcNow -lt $deadline) {
      $p.Refresh()
      if ($p.HasExited) { throw "Packaged app exited before creating a window: $path" }
      if ($p.MainWindowHandle -ne 0) { return }
      Start-Sleep -Milliseconds 250
    }
    throw "Packaged app did not create a window: $path"
  } finally {
    $p.Refresh()
    if (!$p.HasExited) {
      $p.CloseMainWindow() | Out-Null
      if (!$p.WaitForExit(5000)) { Stop-Process -Id $p.Id -Force }
    }
  }
}
$installed = $false
try {
  Invoke-Msi '/i' $MsiPath 'install'
  $installed = $true
  $location = [CrownMsiInfo]::Get($product, 'InstallLocation')
  if (!$location -or [CrownMsiInfo]::Get($product, 'VersionString') -ne $ExpectedVersion) { throw 'MSI did not register its installation correctly.' }
  $app = Join-Path $location 'dw4-crown-mod.exe'
  if (!(Get-Item $app).VersionInfo.ProductVersion.StartsWith($ExpectedVersion)) { throw 'Installed executable version is wrong.' }
  Test-Launch $app
  $portable = Join-Path $work 'portable'
  New-Item -ItemType Directory -Force $portable | Out-Null
  Copy-Item (Join-Path $SourceRoot 'crates/dw4vhp-tauri/target/release/dw4-crown-mod.exe') $portable
  Test-Launch (Join-Path $portable 'dw4-crown-mod.exe')

  # Compile a test-only probe of the unchanged production updater. Its executable
  # runs in both locations so installation detection uses real Windows Installer state.
  $sourcePath = Join-Path $SourceRoot 'crates/dw4vhp-tauri/src/updates.rs'
  $source = [IO.File]::ReadAllText($sourcePath)
  $probe = @'
    #[test]
    #[ignore = "requires the isolated packaged MSI smoke runner"]
    fn packaged_msi_route_smoke() {
        let expected = std::env::var("CROWN_MSI_SMOKE_INSTALLED").unwrap() == "1";
        assert_eq!(is_msi_install(), expected);
        let offer = prepare(releases(), &Version::parse("0.9.0").unwrap(), is_msi_install()).unwrap();
        assert_eq!(offer.offer.method, if expected { "msi" } else { "browser" });
    }
'@
  [IO.File]::WriteAllText($sourcePath, $source.Insert($source.LastIndexOf('}'), $probe))
  $lines = & cargo test --release --lib --manifest-path (Join-Path $SourceRoot 'crates/dw4vhp-tauri/Cargo.toml') --features tauri/custom-protocol --target-dir (Join-Path $SourceRoot 'crates/dw4vhp-tauri/target') --locked --no-run --message-format=json
  if ($LASTEXITCODE -ne 0) { throw 'Could not build the updater route probe.' }
  $tests = @($lines | ForEach-Object {
    try { $entry = $_ | ConvertFrom-Json -ErrorAction Stop } catch { return }
    if ($entry.reason -eq 'compiler-artifact' -and $entry.target.name -eq 'dw4vhp_tauri' -and $entry.profile.test -and $entry.executable) { $entry.executable }
  })
  if ($tests.Count -ne 1) { throw 'Expected one updater route probe.' }
  foreach ($context in @(@{ dir = $location; installed = '1' }, @{ dir = $portable; installed = '0' })) {
    $probePath = Join-Path $context.dir 'updater-route-smoke.exe'
    Copy-Item $tests[0] $probePath
    $env:CROWN_MSI_SMOKE_INSTALLED = $context.installed
    $stdout = Join-Path $work "route-$($context.installed).stdout.log"
    $stderr = Join-Path $work "route-$($context.installed).stderr.log"
    $process = Start-Process -FilePath $probePath -ArgumentList @('packaged_msi_route_smoke', '--ignored', '--nocapture', '--color', 'never') -RedirectStandardOutput $stdout -RedirectStandardError $stderr -Wait -PassThru
    Get-Content $stdout
    Get-Content $stderr
    if ($process.ExitCode -ne 0 -or (Get-Content $stdout -Raw) -notmatch 'test result: ok\. 1 passed; 0 failed') {
      throw 'Packaged updater route probe did not execute and pass its test.'
    }
    Remove-Item $probePath -Force

  }
  @{ version = $ExpectedVersion; installedLaunch = $true; portableLaunch = $true; installedMsiRoute = $true; portableBrowserRoute = $true } |
    ConvertTo-Json | Set-Content (Join-Path $work 'result.json')
} finally {
  if ($installed) { Invoke-Msi '/x' $product 'uninstall' }
}
