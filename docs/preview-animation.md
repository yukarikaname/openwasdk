# Animation in the preview

WinUI 3 animates in two ways, and only one of them is in a page's XBF.

## Storyboards in XBF

A `Storyboard` is a tree of animations; each names a target element, a property, a
duration and either a from/to pair or key frames:

    Storyboard
      DoubleAnimationUsingKeyFrames  Target=ContentPresenter Property=(UIElement.Opacity)
        DiscreteDoubleKeyFrame  t=0     v=1
        SplineDoubleKeyFrame    t=0.133 v=0   KeySpline 0.33,0.0 0.67,1.0
        DiscreteDoubleKeyFrame  t=1.3   v=0
        SplineDoubleKeyFrame    t=1.433 v=1

`openxaml::animation` reads that into tracks, samples a track at a time with the same
curves the composition API uses (`opencomp_core::Easing`), and writes the value onto the
element the target names. The render example exposes it:

    render page.xbf out.ppm 800 600 --state PointerOver --at 0.2
    render page.xbf out.ppm 800 600 --storyboard CopyToClipboardSuccessAnimation --at 0.4

`--state` reaches a storyboard through the `VisualState` that owns it; `--storyboard`
names it directly. Both live in the file's deferred streams, which is why the reader looks
at every substream rather than only the page's own tree.

What a track can move is what the element model stores: opacity, a brush colour, and a
render transform (`TranslateTransform`/`ScaleTransform`, applied at composition time and,
for the animation case, animated through `X`/`Y`/`ScaleX`/`ScaleY`). A target the tree does
not have is left alone.

## Composition animations

Page transitions, implicit show/hide animations and the toolkit's `ShowAnimations` sets are
created in code through `Compositor`. They are not in any XBF and the preview cannot run
them; that path belongs to `opencomp`'s composition API, where `Keyframes` and the `Easing`
curves above are the same sampler.

## What the Gallery has, measured

Four of the 195 compiled pages carry a storyboard:

    Styles/SelectorBar.xbf                    visual states, colour swaps
    Controls/CopyButton.xbf                   CopyToClipboardSuccessAnimation, opacity and scale
    Controls/HorizontalScrollContainer.xbf    CommonStates, colour swaps
    Samples/EasingFunction/EasingFunctionPage.xbf  a DoubleAnimation whose values are set in code

The motion storyboards target parts of app controls (`ContentPresenter`,
`CopyToClipboardTextScaleTransform`, `PART_ContainerRoot`), and those parts only exist once
the control's own style dictionary (`Controls/CopyButton.xbf`, `Styles/*.xbf`) is merged into
the theme. Until then the engine runs but has nothing to move on a page; the colour-swap
states that do land are the ones whose targets happen to exist.

Next step for motion: merge the application's resource dictionaries into the theme, then a
state or storyboard has the template parts its targets name.
