// Gate C: microsoft/microsoft-ui-reactor's hello world, which is the smallest
// program that is declarative C# and needs no XAML of its own. The ladder's
// question is "does the .NET side of a WinUI app start under Wine", so the
// markup here is a heading and a counter rather than anything the platform
// would have to be trusted for.
using Microsoft.UI.Reactor;
using Microsoft.UI.Reactor.Core;
using static Microsoft.UI.Reactor.Factories;

namespace GateC;

static class Program
{
    // WinUI activates WinRT classes, which needs an apartment: the thread has to
    // be STA before the first one is created. Top-level statements cannot carry
    // the attribute, so the entry point is written out.
    [STAThread]
    static void Main()
    {
        WinRT.ComWrappersSupport.InitializeComWrappers();
        ReactorApp.Run<Counter>("Gate C");
    }
}

class Counter : Component
{
    public override Element Render()
    {
        var (count, setCount) = UseState(0);

        return VStack(
            8,
            Heading($"Gate C: {count}"),
            Button("Count", () => setCount(count + 1))
        );
    }
}
