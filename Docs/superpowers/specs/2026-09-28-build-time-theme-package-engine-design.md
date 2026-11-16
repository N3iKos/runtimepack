# Build-Time Theme Package Engine — Design

**Status:** Design approved in conversation; implementation not authorized by this document.
**Date:** 2026-09-28
**Scope:** RuntimePack host-side CLI/theme library, generated Android overlay, and the three bundled overlay themes. This design does not add another runtime family or modify game content.

## Decisions and boundary

The host manages installable theme packages. A build names an explicit nonempty ordered list of theme IDs and one default ID contained in that list. Only these packages enter the generated APK. On first launch, the selected default is active; the overlay's Select Theme lists only packages in that APK, allows switching at runtime, and persists the player's selection in app-specific storage. If an update removes the previously selected theme, selection returns to that build's default. No theme package may be installed from within the APK. CLI installation, removal, import, export, duplication and editing operate on the host catalog; a future GUI calls the same Theme Engine rather than implementing new packaging rules.

The host validates and packages themes before compiling Android. The generated app does not discover or select runtime backends. Theme choice changes only the visual overlay, not the game engine. Game input files remain immutable.

## Package contract

Each theme has a self-contained directory:

```text
theme-name/
  theme.json
  tokens.css
  components.css
  layout.css
  effects.css
  assets/
```

`theme.json` declares stable unique ID, display name, package version, compatible `themeApi` range, stylesheet references, and a manifest of local assets with logical IDs, relative paths, media types and integrity hashes. The four CSS files have distinct responsibilities: semantic/design tokens, component-state styling, allowed overlay layout, and visual effects/motion. A package may omit effects by supplying an empty file; it must not depend on another installed theme. The host validates the schema, compatibility range, referenced component IDs/tokens and files; incompatible packages, missing or invalid assets, unresolved references, unsafe CSS or duplicate IDs fail the build with a stable `BLD-xxx` diagnostic naming theme, file and cause. Never silently swap in another theme. The theme package format and Theme API have independent versions: schema governs serialization, `themeApi` governs overlay component contracts. Theme API major incompatibility fails; minor capabilities must be explicitly negotiated and unknown required capabilities fail.

Themes can configure color, typography, spacing, dimensions, borders, opacity, shadows, layout positions and density, icon mappings, textures/fonts/backgrounds, interaction states (including focus/disabled/status), transitions/spring parameters, controller visuals, overlay effects, and component variants. New visual effects use documented CSS/custom-property or declarative host capabilities; they cannot inject executable behavior. Unsupported requested effects fail validation rather than being represented as working.

## Theme API and visual isolation

Core owns semantic overlay markup, accessibility labels, input routing, modal focus and stacking, and stable names for components and states. Core's CSS contains only functional rules required for positioning, input hit-testing, visibility and accessibility. It must not encode a particular built-in theme's accent, font, texture, shape or glow. Shared component contracts define selectors/slots for toolbar, editor, VPad, cheat, diagnostic, inputs, badges and status elements. All components use semantic design tokens and component tokens; theme CSS may override component slots and layout within documented bounds. DOM status changes are class/data-state transitions, not direct hardcoded `.style.color`/`.style.background` writes. Functional inline geometry (e.g. joystick position) remains allowed and separate from theme presentation.

Each package is parsed and validated at build time. Its selectors are rewritten or verified to remain inside the overlay theme root and cannot target the game DOM or another theme. At runtime only the active package's stylesheets are attached, in deterministic order `tokens.css` → `components.css` → `layout.css` → `effects.css`; the previous package sheets are removed before the new package is applied. The overlay root carries the selected theme ID. Custom property names are scoped to that root and namespaced by Theme API; package selectors cannot target outside it. Functional core rules are protected by the validator, not merely by CSS load order. Built-in themes follow the identical package path; no special hardcoded `clean`/`matte`/`retro` branches in overlay components. For fully customizable visual presentation, component-specific tokens cover non-color properties too; raw scoped CSS supplements tokens without changing the component API.

## Asset Resolver and safety

Theme CSS references assets by declared logical ID; host Asset Resolver resolves them to packaged local paths and rewrites URLs to the overlay's controlled local origin. Validate canonical paths remain within the package, media type, resource existence and size against explicit build limits, and integrity before packaging. Reject traversal, symlink escape, external/network URLs, `@import`, `javascript:`/`data:` URLs, executable files, inline scripts and arbitrary JavaScript. CSS selectors, at-rules, asset URLs and potentially expensive effects require a bounded validation policy; do not claim CSS is a sandbox by itself. Preview and Android use the same resolved asset mapping. Third-party fonts/images/icons require provenance and redistribution review; no license is inferred from file availability. Future behavior extensions belong in a separate Plugin API with its own security and approval decision.

## CLI, compatibility and migration

The host library exports catalog operations (validate, install, list, remove, import, export, duplicate, edit metadata/assets safely) plus build resolution from IDs to immutable package snapshots. CLI exposes these same operations and accepts selected theme IDs and default ID as build inputs; both selections are recorded in build manifest/provenance. Import/export preserve package manifest and file integrity. Remove never changes previously built APKs; an installed package is not mutated in place during a build. GUI later calls this library. No floating dependency or `latest` theme version in reproducible production builds.

Migration is incremental: (1) schema, API contract, validator, resolver and catalog with synthetic packages; (2) move VPad, Cheat and Diagnostic injected CSS and hardcoded presentation mutations to contract-based component styling; (3) package Minimalist Matte, Liquid Glass and Retro OSD independently, preserving Retro's orange identity while removing leaked cyan; (4) replace the dotted touch tail with a continuous renderer only as an explicit Theme API visual-effect capability; (5) integrate selected packages into generated Android output and Select Theme. Each stage retains an executable regression path. Liquid Glass must be visually distinct at panel, control, icon-container, layout and state levels, not only blur/gradient values. Matte uses navy toolbar and consistent dark-blue accents. Panel stacking is functional core behavior, not a theme override.

## Verification and acceptance

Tests cover schema/version rejection, invalid CSS selectors/URLs and paths, missing/oversized/unlicensed assets, exact build-time selection, default inclusion, deterministic output, safe import/export/duplicate/remove, switching/persistence/fallback, and unchanged runtime backend and game source. Overlay tests cover token and state coverage for VPad/Cheat/Diagnostic, no cross-theme style bleed, orange Retro accent, navy Matte toolbar/editor, Liquid Glass optics and readable text, tap feedback restricted to game surface, continuous swipe tail, and topmost active panel. Preview and Android device tests inspect actual computed styles and screenshots across different game backgrounds and screen densities; Node/Gradle builds alone are not evidence of visual or runtime correctness. Android builds for autonomous validation use `--flavor beta` only. No APK build runs without user confirmation.

## Outstanding prerequisite

The canonical docs contain no existing Theme API contract, so implementation must first register the new interface and diagnostics in the appropriate project specifications. Legal permission for distributing any game/engine assets remains a separate gate and is not changed by theming.
