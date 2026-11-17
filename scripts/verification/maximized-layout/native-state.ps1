param([int]$AppPid,[ValidateSet('Normal','Maximized')][string]$State)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$process=Get-Process -Id $AppPid
if ($process.Path -ne (Join-Path (Get-Location) 'target/debug/video-translate-overlay.exe')) {throw 'Unexpected native binary'}
$all=[System.Windows.Automation.AutomationElement]::RootElement.FindAll([System.Windows.Automation.TreeScope]::Children,[System.Windows.Automation.Condition]::TrueCondition)
$windows=@($all | Where-Object {$_.Current.ProcessId -eq $AppPid -and $_.Current.Name -eq '영상 번역 오버레이'})
if ($windows.Count -ne 1) {throw 'Main native window is missing or ambiguous'}
$window=$windows[0]
$pattern=$window.GetCurrentPattern([System.Windows.Automation.WindowPattern]::Pattern)
$pattern.SetWindowVisualState([System.Windows.Automation.WindowVisualState]::$State)
Start-Sleep -Milliseconds 500
if ($pattern.Current.WindowVisualState.ToString() -ne $State) {throw 'Window state did not change'}
$r=$window.Current.BoundingRectangle
@{pid=$AppPid;hwnd=$window.Current.NativeWindowHandle;state=$pattern.Current.WindowVisualState.ToString();geometry=@($r.X,$r.Y,$r.Width,$r.Height)} | ConvertTo-Json -Compress
