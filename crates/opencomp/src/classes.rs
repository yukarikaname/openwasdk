//! The classes the activation manifest points at this DLL.
//!
//! Wine's combase treats a manifest entry the DLL does not serve as fatal -- the process
//! ends instead of returning a code the caller can handle -- so every name here needs a
//! factory, even when the factory can only refuse. The list is generated from the SDK's
//! own `AppxManifest.xml` (see `docs/gate-b.md`); keep it in step with the manifest or an
//! application dies on a class this crate has never heard of.

/// Every runtime class the activation manifest points at this DLL.
///
/// Wine's combase treats "the manifest names a class and the DLL does not serve it" as a
/// fatal error rather than `REGDB_E_CLASSNOTREG`, so a missing name ends the process
/// instead of returning a code the engine can act on. Serving the name and refusing the
/// activation is both honest and survivable; each one that gains a real implementation
/// moves out of `Unmodelled`.
pub(crate) const MANIFEST_CLASSES: &[&str] = &[
    "Microsoft.UI.Composition.AnimationController",
    "Microsoft.UI.Composition.CompositionAnimation",
    "Microsoft.UI.Composition.CompositionApiInformation",
    "Microsoft.UI.Composition.CompositionBrush",
    "Microsoft.UI.Composition.CompositionCapabilities",
    "Microsoft.UI.Composition.CompositionClip",
    "Microsoft.UI.Composition.CompositionDrawingSurface",
    "Microsoft.UI.Composition.CompositionEasingFunction",
    "Microsoft.UI.Composition.CompositionEffectSourceParameter",
    "Microsoft.UI.Composition.CompositionEngine",
    "Microsoft.UI.Composition.CompositionGeometry",
    "Microsoft.UI.Composition.CompositionGradientBrush",
    "Microsoft.UI.Composition.CompositionLight",
    "Microsoft.UI.Composition.CompositionObject",
    "Microsoft.UI.Composition.CompositionPath",
    "Microsoft.UI.Composition.CompositionProjectedShadowCasterCollection",
    "Microsoft.UI.Composition.CompositionShadow",
    "Microsoft.UI.Composition.CompositionShape",
    "Microsoft.UI.Composition.CompositionTransform",
    "Microsoft.UI.Composition.CompositionVirtualDrawingSurface",
    "Microsoft.UI.Composition.Compositor",
    "Microsoft.UI.Composition.ContainerVisual",
    "Microsoft.UI.Composition.Core.CompositorController",
    "Microsoft.UI.Composition.Diagnostics.CompositionDebugSettings",
    "Microsoft.UI.Composition.Effects.SceneLightingEffect",
    "Microsoft.UI.Composition.Experimental.ExpCompositionVisualSurface",
    "Microsoft.UI.Composition.Interactions.CompositionConditionalValue",
    "Microsoft.UI.Composition.Interactions.InteractionTracker",
    "Microsoft.UI.Composition.Interactions.InteractionTrackerInertiaModifier",
    "Microsoft.UI.Composition.Interactions.InteractionTrackerInertiaMotion",
    "Microsoft.UI.Composition.Interactions.InteractionTrackerInertiaNaturalMotion",
    "Microsoft.UI.Composition.Interactions.InteractionTrackerInertiaRestingValue",
    "Microsoft.UI.Composition.Interactions.InteractionTrackerVector2InertiaModifier",
    "Microsoft.UI.Composition.Interactions.InteractionTrackerVector2InertiaNaturalMotion",
    "Microsoft.UI.Composition.Interactions.VisualInteractionSource",
    "Microsoft.UI.Composition.KeyFrameAnimation",
    "Microsoft.UI.Composition.NaturalMotionAnimation",
    "Microsoft.UI.Composition.ScalarNaturalMotionAnimation",
    "Microsoft.UI.Composition.Scenes.SceneComponent",
    "Microsoft.UI.Composition.Scenes.SceneMaterial",
    "Microsoft.UI.Composition.Scenes.SceneMaterialInput",
    "Microsoft.UI.Composition.Scenes.SceneMesh",
    "Microsoft.UI.Composition.Scenes.SceneMeshRendererComponent",
    "Microsoft.UI.Composition.Scenes.SceneMetallicRoughnessMaterial",
    "Microsoft.UI.Composition.Scenes.SceneNode",
    "Microsoft.UI.Composition.Scenes.SceneObject",
    "Microsoft.UI.Composition.Scenes.ScenePbrMaterial",
    "Microsoft.UI.Composition.Scenes.SceneRendererComponent",
    "Microsoft.UI.Composition.Scenes.SceneSurfaceMaterialInput",
    "Microsoft.UI.Composition.Scenes.SceneVisual",
    "Microsoft.UI.Composition.SystemBackdrops.DesktopAcrylicController",
    "Microsoft.UI.Composition.SystemBackdrops.MicaController",
    "Microsoft.UI.Composition.SystemBackdrops.SystemBackdropConfiguration",
    "Microsoft.UI.Composition.Vector2NaturalMotionAnimation",
    "Microsoft.UI.Composition.Vector3NaturalMotionAnimation",
    "Microsoft.UI.Composition.Visual",
];
