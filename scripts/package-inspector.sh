#!/usr/bin/env bash
# Keep .NET-generated files and NuGet state outside authored source.
set -Eeuo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
export DOTNET_CLI_HOME="$root/.work/tool-state/dotnet"
export DOTNET_SKIP_FIRST_TIME_EXPERIENCE=1
export DOTNET_CLI_TELEMETRY_OPTOUT=1
export DOTNET_GENERATE_ASPNET_CERTIFICATE=false
export NUGET_PACKAGES="$root/.work/cache/nuget"
export TMPDIR="$root/.work/tool-state/dotnet/tmp"
mkdir -p "$DOTNET_CLI_HOME" "$NUGET_PACKAGES" "$TMPDIR"
project="$root/research/tools/package-inspector/PackageInspector.csproj"
artifacts="$root/.work/build-package-inspector"
dotnet build "$project" --configuration Release --artifacts-path "$artifacts" --nologo >&2
exec dotnet "$artifacts/bin/PackageInspector/release/PackageInspector.dll" "$@"
