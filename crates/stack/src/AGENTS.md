# STACK MODULE KNOWLEDGE BASE

## OVERVIEW
Unified registry for languages, build systems, frameworks, and monorepo orchestrators. Uses strongly-typed IDs with `Custom(String)` fallback for LLM discovery.

## STRUCTURE
```
src/stack/
├── language/       # Language traits & definitions
├── buildsystem/    # Build system traits (packages, commands)
├── framework/      # Framework-specific overrides
├── orchestrator/   # Monorepo/Workspace logic
└── registry.rs     # The central StackRegistry
```

## WHERE TO LOOK
| Component | Location | Responsibility |
|-----------|----------|----------------|
| LanguageDef | `src/stack/language/` | Manifest patterns & detection rules |
| BuildSystem | `src/stack/buildsystem/` | Templates for build/runtime stages |
| Framework | `src/stack/framework/` | Port/Health/Env defaults |
| Monorepo | `src/stack/orchestrator/` | Workspace topology & topological sort |

## CONVENTIONS
- **ID Safety**: Always use `LanguageId`, `BuildSystemId`, etc. for logic branching.
- **Custom Variant**: Handle `Custom` IDs gracefully for unknown technologies.
- **Trait-First**: Logic belongs in specific trait impls, not in the registry.
- **Manifest Aware**: Build systems should parse manifests (Cargo.toml, package.json) for versions.

## ANTI-PATTERNS
- **Registry Bloat**: Don't add tech-specific logic to `StackRegistry`.
- **Stringly-Typed**: Avoid using raw strings for technology identification.
- **Manual Mapping**: Relationships are defined by trait returns, not manual maps.
