$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$assetRoot = Join-Path $PSScriptRoot '../ios/LocalDrop/Assets.xcassets'
$iconRoot = Join-Path $assetRoot 'AppIcon.appiconset'
[IO.Directory]::CreateDirectory($iconRoot) | Out-Null
$image = [Drawing.Bitmap]::new(1024,1024,[Drawing.Imaging.PixelFormat]::Format24bppRgb)
$graphics = [Drawing.Graphics]::FromImage($image)
$graphics.SmoothingMode = [Drawing.Drawing2D.SmoothingMode]::AntiAlias
$background = [Drawing.Drawing2D.LinearGradientBrush]::new([Drawing.Rectangle]::new(0,0,1024,1024),[Drawing.Color]::FromArgb(10,18,36),[Drawing.Color]::FromArgb(23,70,91),45)
$graphics.FillRectangle($background,0,0,1024,1024)
function Draw-Arrow($points,$color) {
    $pen = [Drawing.Pen]::new($color,76)
    $pen.StartCap = [Drawing.Drawing2D.LineCap]::Round
    $pen.EndCap = [Drawing.Drawing2D.LineCap]::Round
    $pen.LineJoin = [Drawing.Drawing2D.LineJoin]::Round
    $graphics.DrawLines($pen,[Drawing.Point[]]$points)
    $pen.Dispose()
}
Draw-Arrow @([Drawing.Point]::new(240,362),[Drawing.Point]::new(755,362)) ([Drawing.Color]::White)
Draw-Arrow @([Drawing.Point]::new(640,250),[Drawing.Point]::new(755,362),[Drawing.Point]::new(640,474)) ([Drawing.Color]::White)
Draw-Arrow @([Drawing.Point]::new(784,662),[Drawing.Point]::new(269,662)) ([Drawing.Color]::FromArgb(85,226,219))
Draw-Arrow @([Drawing.Point]::new(384,550),[Drawing.Point]::new(269,662),[Drawing.Point]::new(384,774)) ([Drawing.Color]::FromArgb(85,226,219))
$graphics.Dispose(); $background.Dispose()
$images = @()
foreach ($size in @(20,29,40,60)) {
    foreach ($scale in @(2,3)) { $images += @{idiom='iphone';size="$($size)x$size";scale="$($scale)x";filename="icon-$size-$scale.png"} }
}
foreach ($size in @(20,29,40,76)) {
    foreach ($scale in @(1,2)) { $images += @{idiom='ipad';size="$($size)x$size";scale="$($scale)x";filename="icon-$size-$scale.png"} }
}
$images += @{idiom='ipad';size='83.5x83.5';scale='2x';filename='icon-83.5-2.png'}
$images += @{idiom='ios-marketing';size='1024x1024';scale='1x';filename='AppIcon.png'}
foreach ($item in $images) {
    $pixels = [int](([double]($item.size.Split('x')[0])) * [double]($item.scale.TrimEnd('x')))
    $scaled = [Drawing.Bitmap]::new($pixels,$pixels,[Drawing.Imaging.PixelFormat]::Format24bppRgb)
    $g = [Drawing.Graphics]::FromImage($scaled)
    $g.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.DrawImage($image,0,0,$pixels,$pixels); $g.Dispose()
    $scaled.Save((Join-Path $iconRoot $item.filename),[Drawing.Imaging.ImageFormat]::Png); $scaled.Dispose()
}
$image.Dispose()
@{images=$images;info=@{version=1;author='xcode'}} | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $iconRoot 'Contents.json') -Encoding utf8
@{info=@{version=1;author='xcode'}} | ConvertTo-Json -Depth 3 | Set-Content (Join-Path $assetRoot 'Contents.json') -Encoding utf8
