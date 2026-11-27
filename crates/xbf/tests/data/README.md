# XBF test fixtures

Both files are real XBF 2.1 images carved from
`runtimes-framework/win-x64/native/Microsoft.UI.Xaml.Controls.pri` in the
NuGet package `Microsoft.WindowsAppSDK.WinUI` 2.7.0.

The theme XAML for WinUI ships compiled into a PRI, not as loose `.xbf`
files, so these are the two smallest standalone images embedded in that PRI.
Each is the exact image (12 byte prefix plus metadata and node sections).

| file | PRI offset | metadata | node stream | total | contents |
| --- | --- | --- | --- | --- | --- |
| `winui-theme-resources-small.xbf` | 0x294ac | 1146 | 214 | 1372 | TimePickerFlyoutPresenter theme constants |
| `winui-theme-resources.xbf` | 0x29a08 | 1328 | 378 | 1718 | TreeView/ComboBox/DatePicker theme resources, including a Style |

The smallest image is used for the primary parse test; the second one adds a
`Style` custom runtime data blob and a longer node stream.  Their contents
are MIT-licensed together with microsoft/microsoft-ui-xaml, from which the
Windows App SDK binaries are built.
