Add-Type -AssemblyName System.Drawing
$folder=(Resolve-Path "$PSScriptRoot/../../outputs/diversity").Path
$report=Get-Content -LiteralPath (Join-Path $folder 'diversity-report.json') -Raw | ConvertFrom-Json
$font=[System.Drawing.Font]::new('Segoe UI',10)
$brush=[System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(30,38,35))
try {
foreach($style in @('nebula','bloom','contour')) {
    $rows=@($report.art | Where-Object {$_.style -eq $style})
    $sheet=[System.Drawing.Bitmap]::new(1120,432)
    $graphics=[System.Drawing.Graphics]::FromImage($sheet)
    $graphics.Clear([System.Drawing.Color]::FromArgb(224,228,218))
    $graphics.InterpolationMode=[System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    for($i=0;$i -lt $rows.Count;$i++) {
        $x=($i%4)*280; $y=[math]::Floor($i/4)*216
        $source=[System.Drawing.Image]::FromFile((Join-Path $folder $rows[$i].file))
        try {$graphics.DrawImage($source,[int]$x,[int]$y,280,182)} finally {$source.Dispose()}
        $graphics.DrawString(($rows[$i].source+' / '+$rows[$i].composition),$font,$brush,[single]($x+7),[single]($y+188))
    }
    $sheet.Save((Join-Path $folder ($style+'-contact.png')),[System.Drawing.Imaging.ImageFormat]::Png)
    $graphics.Dispose();$sheet.Dispose()
}
$rows=@($report.artTakes)
if($rows.Count -eq 12) {
    $sheet=[System.Drawing.Bitmap]::new(1120,648)
    $graphics=[System.Drawing.Graphics]::FromImage($sheet)
    $graphics.Clear([System.Drawing.Color]::FromArgb(224,228,218))
    $graphics.InterpolationMode=[System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    for($i=0;$i -lt $rows.Count;$i++) {
        $x=($i%4)*280; $y=[math]::Floor($i/4)*216
        $source=[System.Drawing.Image]::FromFile((Join-Path $folder $rows[$i].file))
        try {$graphics.DrawImage($source,[int]$x,[int]$y,280,182)} finally {$source.Dispose()}
        $graphics.DrawString($rows[$i].composition,$font,$brush,[single]($x+7),[single]($y+188))
    }
    $sheet.Save((Join-Path $folder 'composition-systems.png'),[System.Drawing.Imaging.ImageFormat]::Png)
    $graphics.Dispose();$sheet.Dispose()
}
}finally{$font.Dispose();$brush.Dispose()}
