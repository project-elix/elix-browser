param([Parameter(Mandatory=$true)][string]$Executable,[string[]]$BrowserArguments=@(),[int]$Seconds=60,[string]$Output='performance.csv',[double]$MaxIdleCpuPercent=10,[double]$MaxWorkingSetMB=2000)
$ErrorActionPreference='Stop'
# Use a clean profile and identical URLs/workload for each browser. Never reuse a running process.
$launch=@{FilePath=$Executable;PassThru=$true}
if($BrowserArguments.Count -gt 0){$launch.ArgumentList=$BrowserArguments}
$process = Start-Process @launch
$rows=@(); $previous=@{}; $logical=[Environment]::ProcessorCount
try {
 for ($i=0;$i -lt $Seconds;$i+=2) {
  Start-Sleep -Seconds 2
  $all=Get-CimInstance Win32_Process
  $ids=[System.Collections.Generic.HashSet[int]]::new();[void]$ids.Add($process.Id)
  do {$count=$ids.Count;foreach($p in $all){if($ids.Contains([int]$p.ParentProcessId)){[void]$ids.Add([int]$p.ProcessId)}}}while($ids.Count -gt $count)
  $cpu=0.0;$ram=0L;$found=0
  foreach($id in $ids){$p=Get-Process -Id $id -ErrorAction SilentlyContinue;if($p){$found++;$ram+=$p.WorkingSet64;if($previous.ContainsKey($id)){$cpu+=[Math]::Max(0,($p.CPU-$previous[$id])/2/$logical*100)};$previous[$id]=$p.CPU}}
  if($found -eq 0){throw 'Browser exited during performance test'}
  $rows += [PSCustomObject]@{elapsed_seconds=$i+2;cpu_percent=$cpu;working_set_mb=$ram/1MB;processes=$found}
 }
 $rows|Export-Csv -NoTypeInformation -Path $Output
 $steady=$rows|Select-Object -Skip 5
 if(!$steady){throw 'At least 12 seconds required'}
 $avg=($steady|Measure-Object cpu_percent -Average).Average;$peak=($steady|Measure-Object working_set_mb -Maximum).Maximum
 if($avg -gt $MaxIdleCpuPercent){throw "CPU budget exceeded: $avg percent"}
 if($peak -gt $MaxWorkingSetMB){throw "Memory budget exceeded: $peak MB"}
 Write-Output "PASS: mean CPU $avg %, peak summed working set $peak MB. These are budget checks, not comparative performance claims."
} finally {if(!$process.HasExited){Stop-Process -Id $process.Id -ErrorAction SilentlyContinue}}
