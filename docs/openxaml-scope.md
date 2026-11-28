# What openxaml has to be

Measured with `comptrace summary|interfaces|classes|methods|exports|strings` over
the 2.7.0 metadata (`Microsoft.UI.Xaml.winmd`, `Microsoft.UI.winmd`) and the
shipped x64 binaries (`Microsoft.ui.xaml.dll`, `Microsoft.UI.Xaml.Internal.dll`).
The XAML source layout was read from `microsoft/microsoft-ui-xaml` (MIT).
Everything else is marked unknown.

## 1. The surface the file must answer

`Microsoft.ui.xaml.dll` is the WinRT server for the XAML classes *and* the
Content classes; `Microsoft.UI.Xaml.Internal.dll` is a text engine and is not on
that ABI. A replacement is one activation table covering three namespaces, plus
eight exports.

| namespace | interfaces | methods | classes |
| --- | --- | --- | --- |
| `Microsoft.UI.Xaml.Controls` | 745 | 4,732 | 336 |
| `Microsoft.UI.Xaml.Automation.Peers` | 211 | 218 | 106 |
| `Microsoft.UI.Xaml.Controls.Primitives` | 160 | 935 | 75 |
| `Microsoft.UI.Xaml.Media.Animation` | 150 | 424 | 87 |
| `Microsoft.UI.Xaml` | 146 | 861 | 79 |
| `Microsoft.UI.Xaml.Media` | 126 | 379 | 63 |
| other 16 namespaces | 322 | 1,389 | 154 |
| **`Microsoft.UI.Xaml.winmd`** | **1,860** | **8,938** | **900** |

| fact | count |
| --- | --- |
| classes with an activation factory | 247 (49 in `Controls`, 71 in `Media.Animation`) |
| static interfaces (names the table must also answer) | 396 |
| `Microsoft.UI.Content` classes / interfaces / methods | 19 / 51 / 214 |
| Content classes with an activation factory | 0 — all created by static `Create` |
| `Microsoft.UI.Windowing` names the XAML DLL activates | 2 (`AppWindow`, `DisplayArea`) |
| private `Microsoft.UI.Xaml.Internal.*` names the DLL activates | 46 |

The DLL exports eight names, not one: `DllGetActivationFactory`,
`DllCanUnloadNow`, `DllMain`, `XamlCheckProcessRequirements`,
`OverrideXamlMetadataProvider`, `OverrideXamlResourcePropertyBag`,
`GetErrorContextIndex`, `GetHeapHandle`. The last five are not WinRT and not in
any `.winmd`. Which of them the generated app entry point and the XAML hosting
layer call, and in what order: unknown. They exist, so a drop-in must define
them.

`Microsoft.UI.Xaml.Internal.dll` is 872 KB with 186 exports, 182 of them `Ls*`
(Line Services), and imports only the CRT and `KERNEL32`. It is loaded by name at
run time, not linked, so it is no ABI obligation. Whether its Line Services work
without private XAML callbacks: unknown.

## 2. The smallest thing that runs

Reactor is the roadmap's first target and it is verifiable. `Dsl.cs` declares
207 `public static` factory overloads over 118 return types, 91 of them element
types. It is not XAML-free: `ReactorApplication`, an `Application` and
`IXamlMetadataProvider`, calls `InitializeComponent()`, which loads one 344-byte
`ReactorApplication.xaml` whose only content is `<XamlControlsResources/>`; and
`ReactorHostControl` is a `ContentControl`.

| sample | elements | bootstrap |
| --- | --- | --- |
| hello world | `TextBlock`, `TextBox`, `VStack`, `HStack`; `Padding`, `FontSize`, `Bold`, `Width` | `Application.Start`, `Window` + `AppWindow`, `DispatcherQueue` |
| static page | + `Grid`, `Border`, `Button`, `Image`; `Style`/`Setter`, `ResourceDictionary` | + templates and theme dictionaries |

The ABI they touch, counting whole interface families (`IUIElement*`,
`IFrameworkElement*`, `IControl*`, `IPanel*`, `ITextBlock*`, `IButton*`,
`IResourceDictionary*`, `IStyle*`, `ISetter*`, `IDataTemplate*`, `IBrush*`,
`IApplication*`, `IWindow*`, with `*Statics`, `*Overrides`, `*Protected`):
76 interfaces and 1,008 methods. By group: Application+Window 50 methods,
DependencyObject+UIElement+FrameworkElement 432, Control+ContentControl+Panel
148, Grid+StackPanel+Border+Page 100, TextBlock+TextBox 216, Button+ButtonBase
20, resources and styles 27, brushes 15.

That is 11% of the classes and 11% of the methods, excluding input, focus,
automation, XAML's own internal interfaces and the dependency-property metadata
behind each method. The .NET host Reactor needs under Wine is not counted.

## 3. XBF

The format is not undocumented. The reader, writer and dump tool are in the MIT
tree and the wire layout is small:

| part | content |
| --- | --- |
| header | `58 42 46 00`, `uint32` metadata size, `uint32` node size; the version is the first item of the metadata stream |
| splitting | metadata at 12, node stream at 12 + metadata size; both are bounds-checked against the file size |
| metadata tables | string, assembly (provider kind `native`/`managed`/`system`/`parser`/`alternate`), type-namespace, type, property, XML-namespace, hash |
| node stream | 12 node kinds (`StartObject` … `LineInfoAbsolute`), 11 pre-converted value kinds (`IsBoolTrue`, `IsFloat`, `IsThickness`, `IsGridLength`, …) |
| identity | framework types, properties and events are stable indices, not names |
| stable tables | `StableXbfIndexes.g.h`: 984 types, 2,476 properties, 340 events |
| versions | `XbfV1`, `m_uMajorBinaryFileVersion == 2`; XbfKit names these 1, 2, 2.1 |
| dialect | not recorded; XBF2 does not say `Windows.UI.Xaml` or `Microsoft.UI.Xaml` |

| project | language | state |
| --- | --- | --- |
| `microsoft-ui-xaml` `dxaml/xcp/tools/{GenXbfDLL,DumpXbf,XbfParser}`, `core/Parser` | C++/C# | authoritative reader + writer, MIT |
| `WalkingCat/XbfDump` | C++ | dumper, 22 stars |
| `TeamGnome/XbfDecompiler` | C# | decompiler, 12 stars |
| `0x5bfa/XbfKit` | C# | decompile **and** recompile, XBF1/2/2.1, WUX and MUX, used by MrmTool |
| `riverar/xbfdump` | C# | calls the dump facility inside the SDK's `genxbf.dll` |
| Uno (`Uno.UI`) | C# | behaviour reference for XAML; an XBF decoder there: not verified |

An implementation needs, in order: the stable index tables (port the generated
header or map onto our own ids), both metadata readers, the node-stream reader
with its version-gated encodings, type resolution through
`IXamlMetadataProvider`, and a resource map to find the file.

The format cannot be skipped permanently, because the controls' templates are
XBF. `Microsoft.ui.xaml.dll` carries `themeresources.xbf`, `Styles.xbf` and
`Framework ThemeResources.xbf`, referenced as
`Microsoft.UI.Xaml;component/themes/themeresources.xbf`. A `Button` with no
template renders nothing.

For the first app the parser is not on the critical path: the DLL loads XBF
itself through `IApplicationStatics.LoadComponent(IInspectable, Uri)` — the call
generated `InitializeComponent` emits — and parses text through
`IXamlReaderStatics.Load(HSTRING)`. With the official DLL under Wine that already
happens; `openxaml` is needed only when we replace it. The static
`XamlBinaryWriter.Write` (two `IVector<Uri>` and an `IXamlMetadataProvider`)
compiles XAML to XBF at run time, which gives us a corpus to diff against
`DumpXbf`.

## 4. Dependency order

| before | why | state |
| --- | --- | --- |
| `openudk` | the XAML DLL loads only if its 43 C imports resolve | 6 `Windowing_*` real, 143 stubs |
| Content + window + swapchain | `Window` reaches the screen through `ContentIsland`/`DesktopChildSiteBridge` | a bridge creates a D3D11 swapchain for the HWND it was given and draws it with D2D at 60 Hz; `frames_presented()` and `tests/gate_a.rs` prove a frame reached the window on Windows and under Wine. How the *shipped* stack gets island pixels into a child HWND (`DCompPrivates_CreateSharedVisual`) is still unknown, and need not be ours |
| renderer primitives | a page is rectangles, rounded rectangles and text | rectangle, rounded rectangle and textured quad; ellipses, paths and arbitrary geometry: absent; anti-aliasing: rounded corners only |
| text | a static page's content is text | absent; XAML itself imports `DWrite.dll` and `d2d1.dll`, so the port will call them rather than shape glyphs |
| surface interop | XAML may hand pixels over as surfaces | bitmaps, CPU surfaces, GPU textures, and `BeginDraw` returns a real D2D context |
| animations | visual states animate | scalar, vector, colour, step easing and groups reach the node; implicit animations `E_NOTIMPL` |
| `openxaml` | only needed once we replace the DLL, not to run it | XBF → element tree with a support table that errors by name, dependency properties with inheritance and a dirty flag, measure/arrange for StackPanel, Grid, Border, TextBlock. No controls, templates or text yet |

## 4b. Where a control's template actually is

Measured, so the next step does not start from the wrong place.

The theme dictionary inside `Microsoft.UI.Xaml.Controls.pri` is reachable (`xbf::pri`): seven
XBF documents, one of them a `ResourceDictionary` with `Default`/`Light`/`HighContrast` keys and
202 `ControlTemplate` elements. A page's `Button` is a `Style` there, and that Style carries
`custom=Style_v3` plus a substream index (Button: `substream=70`).

That substream is **not** the setter list. It is sixteen nodes:

    PushScope
    SetValueFromThemeResource  property=Setter_Value value="ButtonRevealBackground"
    SetValueFromThemeResource  property=Setter_Value value="ButtonForeground"
    ...

so building it as an element stream fails at node 1 ("property has no current element") and
giving the stream a parent would only produce nonsense: there is no Setter element to own the
property.

The setters live in the style's **custom runtime data**, and its layout is written down in the
engine's own parser (`dxaml/xcp/components/deferral/StyleCustomRuntimeDataSerializer.cpp` and the
C# reference in `dxaml/xcp/tools/XbfParser/WidgetSpinner/XBF/StyleCustomRuntimeData.cs`):

- `Style_v3` = vector of `StyleSetterEssence` (7-bit flags, then a resolved property or a
  name+declaring type, then a string, a constant or a stream-offset token chosen by the flags),
  followed by a vector of conditionally-declared objects (token + predicate list).
- The flags say which of theme-resource, static-resource, string, container, object, self-token
  or mutable the essence is, so the *template* setter is the one whose property is
  `ControlTemplate`/`FrameworkElement.Template` and whose value is an object token.

What this means for the roadmap: reading a style needs the custom-data bytes, which the model
currently discards (it keeps only the kind name and the substream index). Retaining them and
parsing `Style_v3` is the prerequisite for template instantiation; `Builder::new_with_owner` is
not.

## 4c. Control templates are instantiated

The chain is now read end to end: `Style` -> setter list -> `Control_Template` ->
`ControlTemplate` -> its deferred body -> an element tree. `openxaml::theme` keeps those
bodies by type, the loader gives a control the body when the page leaves it empty, and the
render example feeds the theme from `Microsoft.UI.Xaml.Controls.pri` (108 templates from the
Gallery's own copy).

Two rules keep it honest:

- a template body that cannot be built leaves the control its empty box instead of failing
  the page -- one control without chrome is a worse picture, a page that fails is no picture;
- the template is Microsoft's: nothing here draws a Button by hand, it instantiates what the
  framework ships.

## 5. Milestones

| # | step | exit criterion |
| --- | --- | --- |
| 0 | XBF reader crate | reads 20 XBFs written by `XamlBinaryWriter`, including `themeresources.xbf`; every field matches `DumpXbf`; a truncated file returns an error, not a panic |
| 1 | activation table | a table-driven test activates all 900 names and 247 factories and `QueryInterface`s every IID in the metadata; no failure, no unknown IID in `comptrace iids` of the client |
| 2 | property system + layout, no pixels | an 800×600 `Grid`/`StackPanel`/`Border`/`TextBlock` tree measures and arranges without throwing; 12 hand-computed rectangles match `ActualWidth`/`ActualHeight`/offset exactly |
| 3 | window and first frame | a program on our DLLs opens an HWND, shows a background rectangle and one text run; a screenshot check finds the expected colour at three sampled pixels; exit code 0 |
| 4 | templates from XBF | `themeresources.xbf` applies to a `Button`: template border and content appear at the template's geometry, and the log has no `E_NOTIMPL` on the template path |

Step 2 is the cost: 1,008 methods, each backed by a dependency property. Step 3
is the risk: it needs the Content gap closed, and that behaviour is unwritten.

## A caveat on the tool survey

The XBF facts above were read from files fetched by path (`dxaml/xcp/tools/DumpXbf`,
`dxaml/xcp/core/Parser`, `StableXbfIndexes.g.h`). The survey of *other* XBF tooling names the
projects that could be reached and checked one by one (`WalkingCat/XbfDump`,
`TeamGnome/XbfDecompiler`, `0x5bfa/XbfKit`, `riverar/xbfdump`); it is not a proven-exhaustive
list, and Uno's XBF handling is unverified rather than absent.

## Unknowns

1. Who calls `XamlCheckProcessRequirements`, `OverrideXamlMetadataProvider` and
   `OverrideXamlResourcePropertyBag`, and with what contract. The names are
   measured; the callers are in neither the metadata nor the source we read.
2. How an island's pixels reach the child HWND
   (`DCompPrivates_CreateSharedVisual` is the only candidate), and therefore
   whether step 3 can present without a D3D11 shared texture.
3. Whether app-compiled XBF names the 46 private `Microsoft.UI.Xaml.Internal.*`
   classes in a way that forces us to keep those names. Reading real XBFs from
   `XamlBinaryWriter` should answer it; not done.
4. Whether `themeresources.xbf` ships as a loose file or a PRI entry. The loader
   accepts both; the package was not inspected here.
