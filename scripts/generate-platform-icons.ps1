$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$source = [Drawing.Bitmap]::new((Join-Path $PSScriptRoot '../ios/LocalDrop/Assets.xcassets/AppIcon.appiconset/AppIcon.png'))
function Icon-Png([int]$size) {
    $bitmap = [Drawing.Bitmap]::new($size,$size,[Drawing.Imaging.PixelFormat]::Format24bppRgb)
    $g = [Drawing.Graphics]::FromImage($bitmap)
    $g.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.DrawImage($source,0,0,$size,$size); $g.Dispose()
    $stream = [IO.MemoryStream]::new(); $bitmap.Save($stream,[Drawing.Imaging.ImageFormat]::Png)
    $bytes = $stream.ToArray(); $stream.Dispose(); $bitmap.Dispose(); return ,$bytes
}
$files = @{'LocalDropIcon60.png'=60;'LocalDropIcon60@2x.png'=120;'LocalDropIcon60@3x.png'=180;'LocalDropIcon76.png'=76;'LocalDropIcon76@2x.png'=152;'LocalDropIcon83@2x.png'=167}
foreach ($entry in $files.GetEnumerator()) { [IO.File]::WriteAllBytes((Join-Path $PSScriptRoot "../ios/LocalDrop/$($entry.Key)"),(Icon-Png $entry.Value)) }
$sizes = @(16,32,48,64,128,256)
$frames = @($sizes | ForEach-Object { ,(Icon-Png $_) })
$stream = [IO.MemoryStream]::new(); $writer = [IO.BinaryWriter]::new($stream)
$writer.Write([UInt16]0); $writer.Write([UInt16]1); $writer.Write([UInt16]$sizes.Count)
$offset = 6+16*$sizes.Count
for ($i=0;$i -lt $sizes.Count;$i++) {
    $dimension = if ($sizes[$i] -eq 256) {0} else {$sizes[$i]}
    $writer.Write([byte]$dimension); $writer.Write([byte]$dimension); $writer.Write([byte]0); $writer.Write([byte]0)
    $writer.Write([UInt16]1); $writer.Write([UInt16]24); $writer.Write([UInt32]$frames[$i].Length); $writer.Write([UInt32]$offset)
    $offset += $frames[$i].Length
}
foreach ($frame in $frames) { $writer.Write([byte[]]$frame) }
$writer.Flush(); [IO.File]::WriteAllBytes((Join-Path $PSScriptRoot '../windows/app.ico'),$stream.ToArray())
$writer.Dispose(); $source.Dispose()
