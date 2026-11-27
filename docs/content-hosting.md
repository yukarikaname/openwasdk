# How a composition tree reaches a window

A program that uses only `Microsoft.UI.Composition` has no window. Something must
place its visual graph in an HWND; in the Windows App SDK that is
`Microsoft.UI.Content`, through `DesktopChildSiteBridge`. No XAML object appears
and `Microsoft.ui.xaml.dll` never loads. Numbers are 2.7.0 metadata, via `comptrace`.

## The call sequence

```cpp
#include <winrt/Microsoft.UI.Composition.h>
#include <winrt/Microsoft.UI.Content.h>
#include <Microsoft.UI.Interop.h>            // GetWindowIdFromWindow
using namespace winrt::Microsoft::UI;

Composition::Compositor compositor;               // the only visual factory
auto root = compositor.CreateContainerVisual();
root.RelativeSizeAdjustment({ 1.0f, 1.0f });       // IVisual2: fill the site
auto island = Content::ContentIsland::Create(root);
WindowId id;
GetWindowIdFromWindow(hwnd, &id);                  // FrameworkUdk, not WinRT
auto bridge = Content::DesktopChildSiteBridge::Create(compositor, id);
bridge.Connect(island);                            // DesktopSiteBridge
bridge.ResizePolicy(Content::ContentSizePolicy::ResizeContentToParentWindow);
bridge.Show();                                     // then pump messages
```

`Connect`, `Show`, `Hide` and `MoveAndResize` are on `IDesktopSiteBridge`, not on
`IDesktopChildSiteBridge`: the child bridge subclasses `DesktopSiteBridge`, and its
own interface has only `ResizePolicy` and `SiteView`. All four are on the base
interface, which a replacement must answer even though `Create` returns the
derived class. `Create` makes a `WS_CHILD` HWND under the given parent,
owns that window's `ContentSite` (one-to-one with the island), and
`ResizeContentToParentWindow` makes the site track the parent client area.

The root is sized by `RelativeSizeAdjustment` (`IVisual2`,
`492a7995-0c5c-5993-a283-52e4da3050ee`), which `opencomp` answers but returns
`E_NOTIMPL` for, so this frame fails today before reaching `Content`.

## The interfaces

| Interface | IID | N | The part that matters |
| --- | --- | --- | --- |
| `IDesktopChildSiteBridgeStatics` | `ab6b82de-6a47-5de3-a860-613c8db679ab` | 1 | `Create(Compositor, WindowId)` |
| `IDesktopChildSiteBridge` | `b2f2ff7b-1825-51b0-b80b-7599889c569f` | 3 | `ResizePolicy`, `SiteView` |
| `IDesktopSiteBridge` | `f0ae8750-905c-50a2-8a12-4545c6245bb4` | 12 | `Connect`, `Show`, `Hide`, `WindowId` |
| `IContentSiteBridge` | `faaab99e-a42b-549c-92df-3b6d6e1e368b` | 4 | `DispatcherQueue`, `OverrideScale` |
| `IContentIslandStatics` | `7b9eb7cc-8c43-5e0a-ab23-ab48628fd223` | 5 | `Create(Visual)`, `GetByVisual` |
| `IContentIsland` | `5b2504ba-361c-50aa-bd6e-4122c6d93889` | 26 | `ActualSize`, `IsConnected`, `RequestSize` |
| `IContentSiteView` | `2d5d8dd5-358e-5b05-993b-b2666d1786b3` | 14 | `ActualSize`, `ClientSize`, `RasterizationScale` |
| `IContentSite` | `996c60c4-02b2-5eef-93b0-dd6b1ec2fd7b` | 26 | host side; never called by the client |

Composition: `ICompositor` `95213c13-c4cb-57de-b267-d21ab901ae38` (23),
`IContainerVisual` (1), `IVisual` (33), `IVisual2` (4); environment objects 6 each.

## Size of the problem

Of the namespace's 51 interfaces and 214 methods, a first frame calls thirteen: the
five bridge calls, island `Create`, `ActualSize` and `IsConnected`, site-view
`ActualSize`, `ClientSize`, `RasterizationScale`, `AppWindowId`/`DisplayId`. The
other 201 can answer `E_NOTIMPL` for milestone 3: the `ContentSite` host half (33,
only the bridge calls it), `ChildSiteLink`, popup hosting, automation, event args,
deferrals and `ContentCoordinateConverter` (7).

## What the platform must supply

- An HWND and a message pump: the bridge makes the child window, but the parent
  and the loop are the host's, and there is no WinRT loop here.
- `WindowId` conversion. `Microsoft.UI.Interop.h` `LoadLibrary`s
  `Microsoft.Internal.FrameworkUdk.dll` and rejects the result unless all six
  `Windowing_*` exports exist, so a stub with only the two window conversions
  makes `GetWindowIdFromWindow` fail.
- DPI. The site environment (`RasterizationScale`, `DisplayScale`) plus
  `OverrideScale` is the whole story; nothing queries a monitor.
- No D3D device. No `Content` method takes one: the bridge needs a `Compositor`
  and a `WindowId`. D3D enters composition through `ICompositorInterop::
  CreateGraphicsDevice` (`fab19398-6d19-4d8a-b752-8f096c396069`) and
  `ICompositorSwapChainInterop` (`fc084699-67d8-40e1-ade7-08901d84ffda`), about
  surfaces. Someone still owes a swapchain for the child HWND, but the API does
  not say who. There is no `ICompositorDesktopInterop`: that header holds
  drawing-surface, graphics-device and swapchain interop only.

## What is unknown

- How the island's pixels reach the child HWND. The candidate is
  `DCompPrivates_CreateSharedVisual` in `dcompi.dll`, which `dwmcorei.dll` consumes.
- Whether `Show()` is needed after `Connect`, and whether the site pushes the
  initial size without a `WM_SIZE`.
- Which private interfaces the bridge queries internally. The sample never sees
  one, but both DLLs reference `IContentSiteBridgeEndpointConnectionPrivate`-like
  identifiers absent from the metadata.
- Whether `SiteView` can be a plain WinRT object or must be tied to the renderer,
  and whether this works under Wine while the child window is not composed.

Nobody has implemented it: the eight `Microsoft.UI.Content` references in
`Microsoft.ui.xaml.dll` are the only trace of this layer in the packages, and the
Wine and `wine-uwp` trees here contain no `ContentIsland`,
`DesktopChildSiteBridge` or `IContentSite`, only `dcomp` visuals and
`dcomp.spec`. The interfaces are countable; their behaviour is not.
