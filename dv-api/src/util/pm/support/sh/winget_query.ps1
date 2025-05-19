$packageIdsArray = $pkgs -split ' '
$uninstalledPackages = @()
foreach ($packageId in $packageIdsArray) {
    $result = winget list --id $packageId
    if (!($result -match $packageId)) {
        $uninstalledPackages += $packageId
    }
}
$uninstalledPackages = $uninstalledPackages -join ' '
Write-Output $uninstalledPackages
