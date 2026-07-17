# Silicon Labs SDK Project Generator

A fast Rust reimplementation of a subset of the Silicon Labs [`slc-cli`](https://www.silabs.com/software-and-tools/simplicity-studio/configurator-command-line-interface),
targeting the [SLC specification 1.2](https://siliconlabs.github.io/slc-specification/1.2/).
Usable both as a command-line tool and as a library.

HIGHLY EXPERIMENTAL. No guarantees are made about compatibility with Silicon
Labs' official code generator.

## Installation

```
cargo install slcgen
```

The crate is published as `slcgen`; it installs a binary named `slc`.

## Command-line usage

```
slc generate --sdk <path> [--output <dir>] <project.slcp>
```

- `--sdk <path>` — the SDK `.slcs` file, or a directory containing one.
- `--output <dir>` — output directory (defaults to the project's directory).

`generate` parses the SDK and project, resolves the project's components against
the SDK, and writes the `autogen/` and `config/` trees under the output
directory.

## Library usage

Add the dependency:

```
cargo add slcgen
```

The crate is `slcgen`, but the library is imported as `slc`:

```rust
use slc::{ParsedProject, Project, SDK};

let sdk = SDK::parse("path/to/sdk.slcs")?;
let project = Project::parse("path/to/project.slcp")?;

let resolved = project.resolve_components(&sdk)?;
let parsed = ParsedProject::new(&sdk, &project, &resolved);
let written = parsed.generate("path/to/output")?;

println!("wrote {} file(s)", written.len());
```

`resolve_components` returns structured errors on resolution failure, and
`generate` returns the list of paths it wrote.

## What it does

- Parses `.slcs` (SDK), `.slcc` (component), and `.slcp` (project) files,
  including the `!!omap` ordered-map form real SDK components ship in.
- Resolves component dependencies per the SLC 1.2 algorithm: required/provided/
  conflicting feature sets, the provide fixpoint, single-candidate auto-add,
  `recommends`-based disambiguation, and the `allow_multiple` duplicate-provide
  rule. Resolution failures are reported as structured errors, not panics.
- Generates config files (with the project-level `configuration` overrides
  applied to `#define` values on first copy, never overwriting existing files),
  renders `template_file` templates, and expands instantiable components
  (`{{instance}}` path substitution plus the `INSTANCE`/`{{instance}}` content
  transforms).

Template rendering is pure Rust ([minijinja](https://docs.rs/minijinja)); there
is no external runtime dependency. A small shim backs the jinja2 `list.append()`
mutation idiom that core SDK templates (e.g. `sl_event_handler.c`) use for
deduplication.

## Tests

```
cargo test
```

An opt-in test parses every component of a real SDK; point it at a checkout:

```
SLC_TEST_SDK=/path/to/simplicity_sdk cargo test --test real_sdk -- --ignored
```

## Known limitations

Not yet implemented: SDK extensions (`.slce`) and component `from`, `post_build`
emission, `toolchain_settings`/`other_file` output, component validation
(`validation_helper`/`validation_library`), workspaces (`.slcw`), SDK upgrade
(`.slcu`), and `{{instance}}` substitution inside `template_contribution` values.
These keys are parsed where present so they are not silently lost.
