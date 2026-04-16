$ErrorActionPreference = "Stop"

cargo install cargo-wix --locked
winget install --id WiXToolset.WiXToolset -e --accept-package-agreements --accept-source-agreements --force
cargo wix init
cargo wix -b "C:\Program Files (x86)\WiX Toolset v3.14\bin" --nocapture

Get-ChildItem -Path .\target\wix -Filter *.msi | Select-Object Name,FullName,Length,LastWriteTime
