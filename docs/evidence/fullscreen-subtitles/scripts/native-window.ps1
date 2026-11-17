param([int]$AppPid,[int]$Width=1296,[int]$Height=839)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$all=[System.Windows.Automation.AutomationElement]::RootElement.FindAll([System.Windows.Automation.TreeScope]::Children,[System.Windows.Automation.Condition]::TrueCondition)
$windows=@($all | Where-Object {$_.Current.ProcessId -eq $AppPid -and $_.Current.Name -eq '영상'})
if ($windows.Count -ne 1) {throw 'Player window missing or ambiguous'}
$window=$windows[0]
$pattern=$window.GetCurrentPattern([System.Windows.Automation.WindowPattern]::Pattern)
$pattern.SetWindowVisualState([System.Windows.Automation.WindowVisualState]::Normal)
$transform=$window.GetCurrentPattern([System.Windows.Automation.TransformPattern]::Pattern)
$transform.Move(0,0)
$transform.Resize($Width,$Height)
Start-Sleep -Milliseconds 300
$r=$window.Current.BoundingRectangle
@{pid=$AppPid;hwnd=$window.Current.NativeWindowHandle;state=$pattern.Current.WindowVisualState.ToString();geometry=@($r.X,$r.Y,$r.Width,$r.Height)} | ConvertTo-Json -Compress
