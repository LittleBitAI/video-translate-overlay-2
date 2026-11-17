param([int]$AppPid, [int]$Port, [int]$RootPid)
$ErrorActionPreference='Stop'
$process=Get-CimInstance Win32_Process -Filter "ProcessId = $AppPid"
if (!$process) { throw 'Expected application process is absent' }
$listeners=@(Get-NetTCPConnection -State Listen -LocalPort $Port)
$owners=@($listeners.OwningProcess | Select-Object -Unique)
if ($owners.Count -ne 1) { throw 'Listener owner is missing or ambiguous' }
$chain=@()
$cursor=[int]$owners[0]
for ($i=0; $i -lt 12; $i++) {
  $entry=Get-CimInstance Win32_Process -Filter "ProcessId = $cursor"
  if (!$entry) { throw 'Listener ancestry is incomplete' }
  $chain+=@{pid=$cursor;parent=[int]$entry.ParentProcessId;image=$entry.ExecutablePath;command=$entry.CommandLine;created=$entry.CreationDate.ToUniversalTime().ToString('o')}
  if ($cursor -eq $RootPid) { break }
  $cursor=[int]$entry.ParentProcessId
}
if ($cursor -ne $RootPid) { throw 'Listener belongs to another process tree' }
$listenerProcess=Get-Process -Id $owners[0]
@{pid=$AppPid;rootPid=$RootPid;image=$process.ExecutablePath;created=$process.CreationDate.ToUniversalTime().ToString('o');port=$Port;listeners=$listeners.LocalAddress;ancestry=$chain;listenerVersion=$listenerProcess.FileVersionInfo.FileVersion} | ConvertTo-Json -Depth 8 -Compress
