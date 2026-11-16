# Architecture

## 1. Architectural style

RuntimePack uses:

- ports-and-adapters on host side;
- backend plugin architecture for engines;
- immutable build planning;
- app-side service boundaries for content, input, saves, diagnostics;
- build-time runtime selection.

## 2. Host component graph

```text
CLI
 |
 v
CommandService
 |
 +--> DoctorService
 +--> InspectService
 |      |
 |      +--> Ingestor
 |      +--> DetectorRegistry
 |      +--> AnalyzerRegistry
 |
 +--> BuildService
        |
        +--> Ingestor
        +--> DetectorRegistry
        +--> AnalyzerRegistry
        +--> RuntimeResolver
        +--> BuildPlanner
        +--> ContentPacker
        +--> AndroidProjectGenerator
        +--> GradleDriver
        +--> Signer
        +--> ArtifactReporter
```

## 3. Pure planning boundary

All decisions should be calculated before build side effects.

```text
Input + Config + RuntimeRegistry
       |
       v
    BuildPlan   <--- serializable
       |
       v
 side-effecting execution
```

`BuildPlan` must be serializable so it can be reviewed/debugged.

## 4. Core interfaces

### Detector

```rust
trait Detector {
    fn id(&self) -> &'static str;
    fn probe(&self, source: &GameSourceIndex) -> DetectionCandidate;
}
```

### Analyzer

```rust
trait Analyzer {
    fn supports(&self, engine: EngineId) -> bool;
    fn analyze(
        &self,
        source: &GameSourceIndex,
        engine: &EngineDescriptor
    ) -> CompatibilityReport;
}
```

### RuntimeResolver

```rust
trait RuntimeResolver {
    fn resolve(
        &self,
        engine: &EngineDescriptor,
        report: &CompatibilityReport,
        registry: &RuntimeRegistry,
        policy: &BuildPolicy
    ) -> Result<RuntimePlan, ResolveError>;
}
```

### ContentTransform

```rust
trait ContentTransform {
    fn plan(&self, input: &ContentGraph) -> TransformPlan;
    fn execute(&self, plan: &TransformPlan, out: &Path) -> Result<()>;
}
```

## 5. Android component graph

```text
BootstrapActivity
       |
       v
RuntimeApplication
       |
       +--> BuildManifestLoader
       +--> ContentService
       +--> SaveService
       +--> InputService
       +--> DiagnosticsService
       +--> RuntimeHost
                |
                +--> RgssBackend
                +--> RpgMakerWebBackend
                +--> RenPyBackend
                +--> TyranoWebBackend
```

## 6. Dependency direction

Allowed:

```text
CLI -> core
engine adapters -> core interfaces
Android backends -> Android runtime API
packager -> schemas
```

Forbidden:

```text
core -> concrete RPG Maker class
core -> JoiPlay implementation
RGSS backend -> Ren'Py backend
engine detector -> Android Activity
Android Activity -> host CLI
```

## 7. Repository layout

```text
runtimepack/
├── Cargo.toml
├── cli/
├── core/
│   ├── ingest/
│   ├── detection/
│   ├── compatibility/
│   ├── runtime_registry/
│   ├── build_plan/
│   ├── diagnostics/
│   └── provenance/
├── engines/
│   ├── rgss/
│   ├── rpgmaker_web/
│   ├── renpy/
│   └── tyrano/
├── pack/
│   ├── container/
│   ├── crypto/
│   └── compression/
├── android/
│   ├── shell/
│   ├── runtime-api/
│   ├── content/
│   ├── input/
│   └── saves/
├── runtimes/
│   ├── manifests/
│   └── build-recipes/
├── compatibility-db/
├── schemas/
├── tests/
│   ├── fixtures/
│   ├── integration/
│   └── device/
└── third_party/
```

## 8. Why not a universal generated APK?

A JoiPlay-style launcher has to dynamically host multiple runtime families. RuntimePack resolves the runtime at build time.

Benefits:

- smaller output;
- fewer dependencies and licenses per game;
- smaller attack surface;
- fewer Android permissions;
- simpler startup path;
- clearer compatibility report.

A debug-only universal APK may be introduced for internal testing but is not a product output requirement.

## 9. Compatibility overlay architecture

```text
logical read
  |
  +--> PatchLayer
  |      |
  |      +-- game-specific replacement
  |
  +--> BaseContentLayer
  |
  +--> GeneratedCompatibilityLayer
```

The original source remains untouched.

## 10. Version boundaries

There are four independent versions:

```text
RuntimePack CLI version
Runtime API version
Content container version
Backend/runtime package version
```

They must not be collapsed into one version number.
