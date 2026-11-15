param([Parameter(Mandatory=$true)][string]$Path)
$ErrorActionPreference='Stop'
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class FlyxResourceReader {
 [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] public static extern IntPtr LoadLibraryEx(string file,IntPtr reserved,uint flags);
 [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] public static extern IntPtr FindResource(IntPtr module,IntPtr name,IntPtr type);
 [DllImport("kernel32.dll")] public static extern IntPtr LoadResource(IntPtr module,IntPtr resource);
 [DllImport("kernel32.dll")] public static extern IntPtr LockResource(IntPtr resource);
 [DllImport("kernel32.dll")] public static extern uint SizeofResource(IntPtr module,IntPtr resource);
 [DllImport("kernel32.dll")] public static extern bool FreeLibrary(IntPtr module);
}
'@
$absolute=(Resolve-Path -LiteralPath $Path).Path
$module=[FlyxResourceReader]::LoadLibraryEx($absolute,[IntPtr]::Zero,2)
if($module -eq [IntPtr]::Zero){throw 'Cannot load EXE resources'}
try {
 $resource=[FlyxResourceReader]::FindResource($module,[IntPtr]1,[IntPtr]24)
 if($resource -eq [IntPtr]::Zero){throw 'EXE manifest is missing'}
 $size=[FlyxResourceReader]::SizeofResource($module,$resource)
 $memory=[FlyxResourceReader]::LockResource([FlyxResourceReader]::LoadResource($module,$resource))
 $bytes=[byte[]]::new($size)
 [Runtime.InteropServices.Marshal]::Copy($memory,$bytes,0,$size)
 $manifest=[Text.Encoding]::UTF8.GetString($bytes)
 if($manifest -notmatch 'requestedExecutionLevel\s+level="requireAdministrator"'){throw 'EXE does not require administrator'}
 Write-Output 'Embedded release manifest verified: requireAdministrator, uiAccess=false.'
} finally {
 [void][FlyxResourceReader]::FreeLibrary($module)
}
