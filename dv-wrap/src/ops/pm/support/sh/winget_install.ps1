$packageIdsArray = $pkgs -split ' '
if ($null -ne $env:https_proxy) {
    $proxy = $env:https_proxy
}
elseif ($null -ne $env:all_proxy) {
    $proxy = $env:all_proxy
}
else {
    $proxy = ""
}
if ($null -ne $proxy) {
    $proxy = "--proxy=$proxy"
}
foreach ($packageId in $packageIdsArray) {
    $cmd = "winget install --id $packageId $proxy"
    Write-Host "Running command: $cmd"
    Invoke-Expression $cmd
}