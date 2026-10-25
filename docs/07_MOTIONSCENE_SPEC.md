# ShotClutch — MotionScene Specification

Baseline 0.1 · 2026-10-26 · Native subset in M4; isolated web fallback in M5.

## Purpose and contract

MotionScene is a deterministic programmatic graphics source for the ShotClutch timeline. It is a compiler/runtime and scene contract, independent from the editor UI framework. It can power titles, lower thirds, diagrams, charts, overlays, and eventually richer web-authored graphics.

The native semantic contract is:

```ts
render(t: SceneTime, context: SceneContext): SceneIR
```

For fixed scene code, parameters, assets, seed, runtime, fonts, environment, and requested time, evaluation must produce equivalent output regardless of previous requests. Calling frame 100, then frame 1, then frame 100 must not depend on accumulated animation state. Sequential playback is an optimization, never a semantic prerequisite.

Exact scene time is passed as rational ticks. Numerical animation helpers may derive finite seconds locally for interpolation; timeline boundaries retain exact ticks. Determinism has two levels: identical normalized SceneIR for the same pinned evaluator, and pixel equivalence within declared renderer/platform tolerances. Cross-GPU bit-identical output is not assumed.

## Authored asset and clip instance

An authored asset specifies ID/revision, kind `native` or `web`, API version, source/entry point, lockfile, duration, dimensions, rational frame rate, working color/alpha policy, parameters, seed, referenced media/fonts, capability requirements, and provenance. A clip instance references that revision and provides timeline placement, trim, supported speed map, and instance parameters.

Changing a scene creates a new revision and invalidates affected preview/verification caches. Existing exports remain tied to their original revision. Imported code is untrusted and does not execute on project open without the appropriate trust policy.

## Native authoring API

TS/JS with an optional JSX authoring layer is the baseline direction; the embedded runtime and compiler are unresolved decisions. The final API must compile or evaluate into serializable SceneIR without requiring a DOM or a frontend framework. Authoring helpers include pure interpolation, keyframe curves, easing, analytical springs, seeded noise, text layout parameters, and composition transforms.

Illustrative proposed API:

```ts
export function render(t: SceneTime, ctx: SceneContext): SceneIR {
  const elapsed = ctx.time.toSeconds(t);
  const progress = Math.max(0, Math.min(1, elapsed / 0.4));
  const eased = 1 - Math.pow(1 - progress, 3);
  return {
    version: 1,
    nodes: [{
      id: "title",
      kind: "text",
      text: ctx.params.title,
      fontId: ctx.fonts.primary,
      fontSize: 72,
      position: [120, 700 + 80 * (1 - eased)],
      opacity: eased
    }]
  };
}
```

The example is conceptual rather than an existing SDK. It has no hidden state and can evaluate any timestamp directly. Context contains immutable parameters, declared assets, deterministic helper functions, render dimensions, quality tier, and capability information. Context does not contain unrestricted filesystem, network, process, or wall-clock APIs.

## SceneIR v0.1 subset

| Node/feature | Minimum contract |
|---|---|
| Group | Ordered children, local transform, opacity, clip bounds |
| Text | String, font identity, size, shaping/language inputs, layout bounds, alignment, fill |
| Shape/path | Declared geometry/path commands, fill, stroke, explicit units |
| Image | Asset reference, source rectangle, fit behavior, transform, alpha |
| Clip/mask | Bounded geometric clipping for the qualified subset |
| Transform | Finite 2D translation, scale, rotation, anchor, explicit composition order |
| Animation | Time-evaluated parameters; keyframes/easing/analytical spring helpers |

Video surfaces are reserved in the schema and supplied by the native media engine; broad support follows engine qualification. Blur, glow, gradients, advanced masks, particles, 3D, and arbitrary shaders require capability declarations and separate qualification. Unsupported nodes return a structured error or an explicit export fallback plan. They are never silently ignored.

SceneIR nodes have stable local IDs, finite numeric values, bounded arrays, and explicit asset references. Tree depth, node count, path complexity, text length, and output size are quota-controlled. Cycles and executable callbacks inside the IR are invalid. Layout has explicit units and coordinate origin; v0.1 uses top-left pixel coordinates at the authored canvas size, with a documented scaling transform to the requested output.

## Native evaluation and rendering

1. Validate the asset manifest and granted capabilities.
2. Resolve locked dependencies and fonts before evaluation.
3. Evaluate a time sample in a bounded isolated worker.
4. Validate/normalize SceneIR and compute its digest.
5. Render supported nodes through the native render graph and WGPU path.
6. Return a surface with timestamp, dimensions, color/alpha metadata, revision, and quality.

Evaluation and rendering are separately cacheable. No scene may run on the UI thread or real-time audio callback. Motion blur, when added, requests deterministic subframe samples with explicit shutter settings; a scene must not read “current playback time” to choose samples.

## Web fallback contract

The web path is explicit `kind: web`; failure of a native node does not automatically execute arbitrary web code. A web scene contains a locked bundle and exposes a ShotClutch-specific adapter:

```ts
interface WebSceneAdapter {
  initialize(context: WebSceneContext): Promise<void>;
  renderFrame(time: SceneTime): Promise<void>;
  dispose(): Promise<void>;
}
```

This proposed interface is ShotClutch's internal contract, not a claim of compatibility with a referenced project. A bounded Chromium-based worker initializes fonts/assets, freezes nondeterministic clocks, evaluates a requested time, waits for explicit readiness, and captures an RGBA surface. HTML/CSS/SVG/Canvas/WebGL or packaged libraries may be used only within declared capabilities and supported runtime versions.

Seeking must reset or reconstruct state so output is independent of request history. Timers, random sources, media playback, asynchronous asset loads, and GPU completion need deterministic controls. Bundles unable to satisfy the contract are unsupported for deterministic export and require an explicit prerendered asset workflow.

Ordinary source footage stays in the native engine. Any web-scene media input is an explicit host-provided frame/image resource with bounded transfer cost; a scene cannot open timeline media files directly or run its own unsupervised playback clock. The web worker returns graphics to the native compositor with explicit alpha and color conversions.

## Sandbox and resource policy

Disable network, arbitrary file access, subprocesses, native modules, and dynamic dependency downloads by default. Resolve dependencies from pinned approved bundles. Grant access only to manifest-listed asset handles. Imported scene content cannot request broader permissions through text inside the scene.

Enforce evaluation time, wall-time, memory, GPU allocation, output size, node/DOM complexity, and concurrent worker quotas. Kill and restart a runaway worker; show a labeled preview error. Final export fails with a scene-specific diagnostic unless the user has selected a documented baked replacement. A crashed runtime must not take down the editor or lose project state.

## Cache identity and fidelity

The cache key includes scene revision/code hash, dependency/font hashes, parameters, seed, exact sample time, dimensions, pixel ratio, quality tier, color/alpha policy, runtime/build identity, renderer/backend identity where pixel output depends on it, and all referenced source-frame identities. Draft frames cannot satisfy final-quality requests. Persist source and provenance independently of cache entries.

Native text/transforms can map to destination constructs where supported. Otherwise export samples keyframes or bakes a scene with alpha. Web scenes generally require baking unless a destination adapter has a verified equivalent. Record the loss of programmability in the fidelity report; preserve authored sources in the package when permitted.

## Qualification and unresolved decisions

Test randomized seek order, repeated frames, parallel workers, save/reopen, seed changes, missing fonts, modified assets, duration boundaries, fractional time samples, unsupported nodes, quota violations, blocked network/file access, and crash recovery. Compare native rendering with scene goldens; compare web frames only within a pinned environment and declared tolerance.

Open choices are the JS runtime, TS/JSX compilation pipeline, exact native node subset implementation, font packaging, Chromium embedding/distribution, and web GPU/color behavior. These are tracked as DEC-003, DEC-004, and DEC-010 in [17](17_DEVELOPMENT_TRACKER.md); they do not reopen the accepted deterministic/native-plus-web architecture.
