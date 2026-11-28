#!/bin/sh
# Register the WinRT classes wine-uwp's dcomp serves.
#
# Wine's combase resolves an activatable class by looking it up under
# `HKLM\Software\Microsoft\WindowsRuntime\ActivatableClassId`, so a DLL that exports
# `DllGetActivationFactory` is still invisible until something names it there. The lifted
# DWM core asks for `Windows.UI.Composition.Compositor` -- the *system* composition API,
# not `Microsoft.UI.Composition` -- and without the entry combase answers
# `REGDB_E_CLASSNOTREG` and XAML faults later on a null device.
#
# usage: register-winrt.sh <wine-binary> [prefix]
set -e

wine=${1:?usage: register-winrt.sh <wine-binary> [prefix]}
[ -n "$2" ] && export WINEPREFIX=$2

classes='
Windows.UI.Composition.Compositor
Windows.UI.Composition.CompositionCapabilities
Windows.UI.Composition.CompositionEasingFunction
Windows.UI.Composition.CompositionEffectSourceParameter
Windows.UI.Composition.CompositionIsland
Windows.UI.Composition.CompositionObject
Windows.UI.Composition.CompositionPath
Windows.UI.Composition.Effects.SceneLightingEffect
'

for class in $classes; do
	"$wine" reg add "HKLM\\Software\\Microsoft\\WindowsRuntime\\ActivatableClassId\\$class" \
		/v DllPath /t REG_SZ /d 'C:\windows\system32\dcomp.dll' /f >/dev/null 2>&1
done
echo "registered $(echo $classes | wc -w | tr -d ' ') classes against dcomp.dll"
