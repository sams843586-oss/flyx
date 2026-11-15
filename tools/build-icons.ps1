$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Drawing
$workspace=Split-Path -Parent $PSScriptRoot
$logo=[System.Drawing.Image]::FromFile((Join-Path $workspace 'promo/assets/logo.png'))
$logo.Save((Join-Path $workspace 'crates/app/icons/icon.png'),[System.Drawing.Imaging.ImageFormat]::Png)
$stream=[System.IO.File]::Create((Join-Path $workspace 'crates/app/icons/icon.ico'))
$writer=[System.IO.BinaryWriter]::new($stream)
$sizes=@(16,32,48,64,128,256)
$images=@()
foreach($size in $sizes){
    $bitmap=[System.Drawing.Bitmap]::new($size,$size)
    $graphics=[System.Drawing.Graphics]::FromImage($bitmap)
    $graphics.Clear([System.Drawing.Color]::Transparent)
    $graphics.InterpolationMode=[System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $graphics.DrawImage($logo,0,0,$size,$size)
    $memory=[System.IO.MemoryStream]::new()
    $bitmap.Save($memory,[System.Drawing.Imaging.ImageFormat]::Png)
    $images+= ,$memory.ToArray()
    $graphics.Dispose(); $bitmap.Dispose(); $memory.Dispose()
}
$writer.Write([uint16]0); $writer.Write([uint16]1); $writer.Write([uint16]$sizes.Count)
$offset=6+16*$sizes.Count
for($i=0;$i -lt $sizes.Count;$i++){
    $dim=if($sizes[$i] -eq 256){0}else{$sizes[$i]}
    $writer.Write([byte]$dim); $writer.Write([byte]$dim); $writer.Write([uint16]0)
    $writer.Write([uint16]1); $writer.Write([uint16]32)
    $writer.Write([uint32]$images[$i].Length); $writer.Write([uint32]$offset)
    $offset+=$images[$i].Length
}
foreach($bytes in $images){$writer.Write([byte[]]$bytes)}
$writer.Dispose(); $logo.Dispose()
Write-Output 'PNG and six-resolution ICO created with alpha transparency.'
