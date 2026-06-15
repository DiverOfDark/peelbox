use super::*;

/// Known package prefixes that need version resolution.
/// Maps the generic name to the prefix used for Wolfi lookup.
const VERSIONABLE_PACKAGES: &[(&str, &str)] = &[
    ("rust", "rust"),
    ("nodejs", "nodejs"),
    ("python", "python"),
    ("openjdk", "openjdk"),
    ("go", "go"),
    ("ruby", "ruby"),
    ("php", "php"),
    ("elixir", "elixir"),
    ("erlang", "erlang"),
    ("dotnet-sdk", "dotnet"),
    ("dotnet-runtime", "dotnet"),
    ("maven", "maven"),
    ("gradle", "gradle"),
    ("zig", "zig"),
    ("bazel", "bazel"),
    ("postgresql", "postgresql"),
    ("libpq", "libpq"),
];

/// Packages where the very latest version often lacks broad ecosystem
/// compatibility (many libraries don't publish wheels/packages for the
/// bleeding-edge release). For these, prefer the second-latest minor version
/// when no explicit version is pinned — matching PaaS defaults (Heroku, Railway).
const PREFER_STABLE_PACKAGES: &[(&str, usize)] = &[
    ("python", 2), // Many libraries publish wheels late; N-2 has broadest support
    ("elixir", 1),
    ("erlang", 2), // Erlang 27+ has escript compilation issues with popular packages (e.g. simplifile)
];

/// Resolve generic package names to versioned Wolfi package names.
pub(crate) fn resolve_wolfi_packages(packages: &mut [String], wolfi: &WolfiPackageIndex) {
    for pkg in packages.iter_mut() {
        // Skip if already exists in Wolfi (versioned or generic like "build-base")
        if wolfi.has_package(pkg) {
            continue;
        }

        // Handle special cases first
        if pkg == "pip" {
            // pip → py3.X-pip (derive from python version in same package list)
            // We'll handle this in a second pass
            continue;
        }

        // Check if this is a versionable package
        if let Some((_, prefix)) = VERSIONABLE_PACKAGES
            .iter()
            .find(|(name, _)| *name == pkg.as_str())
        {
            let resolved = if let Some((_, offset)) =
                PREFER_STABLE_PACKAGES.iter().find(|(p, _)| *p == *prefix)
            {
                wolfi.get_stable_version_at_offset(prefix, *offset)
            } else {
                wolfi.get_latest_version(prefix)
            };
            if let Some(resolved) = resolved {
                debug!(from = %pkg, to = %resolved, "Resolved Wolfi package version");
                *pkg = resolved;
            }
        } else if pkg.ends_with("-dev") {
            // Handle e.g. erlang-dev → erlang-28-dev
            let base = pkg.strip_suffix("-dev").unwrap();
            if let Some((_, prefix)) = VERSIONABLE_PACKAGES.iter().find(|(name, _)| *name == base) {
                let resolved = if let Some((_, offset)) =
                    PREFER_STABLE_PACKAGES.iter().find(|(p, _)| *p == *prefix)
                {
                    wolfi.get_stable_version_at_offset(prefix, *offset)
                } else {
                    wolfi.get_latest_version(prefix)
                };
                if let Some(resolved) = resolved {
                    let dev_pkg = format!("{}-dev", resolved);
                    if wolfi.has_package(&dev_pkg) {
                        debug!(from = %pkg, to = %dev_pkg, "Resolved Wolfi dev package version");
                        *pkg = dev_pkg;
                    }
                }
            } else if base.starts_with("ruby-") {
                // ruby-2.6-dev → ruby-3.0-dev (EOL version fallback)
                let versions = wolfi.get_versions("ruby");
                if let Some(oldest) = versions.last() {
                    let resolved = format!("ruby-{}-dev", oldest);
                    debug!(from = %pkg, to = %resolved, "Resolved unavailable Ruby -dev version to oldest Wolfi package");
                    *pkg = resolved;
                }
            }
        } else if pkg.starts_with("openjdk-") && !pkg.contains("-jre") {
            // openjdk-17 → check it exists, if not try with wolfi
            if !wolfi.has_package(pkg) {
                // Try finding the exact version
                let version = pkg.strip_prefix("openjdk-").unwrap_or("");
                let available = wolfi.get_versions("openjdk");
                if let Some(resolved) = wolfi.match_version("openjdk", version, &available) {
                    *pkg = resolved;
                }
            }
        } else if pkg.starts_with("dotnet-") && pkg.ends_with("-sdk") {
            // dotnet-8-sdk → already versioned, check existence
            if !wolfi.has_package(pkg) {
                // Try resolving: dotnet-sdk → dotnet-X-sdk
                if let Some(latest) = wolfi.get_latest_version("dotnet") {
                    let ver = latest.strip_prefix("dotnet-").unwrap_or("8");
                    *pkg = format!("dotnet-{}-sdk", ver);
                }
            }
        } else if pkg.starts_with("dotnet-") && pkg.ends_with("-runtime") && !wolfi.has_package(pkg)
        {
            if let Some(latest) = wolfi.get_latest_version("dotnet") {
                let ver = latest.strip_prefix("dotnet-").unwrap_or("8");
                *pkg = format!("dotnet-{}-runtime", ver);
            }
        } else if pkg.starts_with("aspnet-") && pkg.ends_with("-runtime") && !wolfi.has_package(pkg)
        {
            // aspnet-6-runtime → aspnet-X-runtime (resolve to latest available)
            if let Some(latest) = wolfi.get_latest_version("dotnet") {
                let ver = latest.strip_prefix("dotnet-").unwrap_or("8");
                *pkg = format!("aspnet-{}-runtime", ver);
            }
        } else if pkg.starts_with("go-") && !wolfi.has_package(pkg) {
            // go-1.18 → resolve to latest available Go version
            // Go is backward-compatible, so old code builds fine with newer compilers.
            if let Some(latest) = wolfi.get_latest_version("go") {
                debug!(from = %pkg, to = %latest, "Resolved old Go version to latest Wolfi package");
                *pkg = latest;
            }
        } else if pkg.starts_with("python-")
            && pkg[7..].chars().next().is_some_and(|c| c.is_ascii_digit())
            && !wolfi.has_package(pkg)
        {
            // python-2.7 → resolve to preferred stable Python version
            // Python 2 isn't in Wolfi; fall back to the stable Python 3.x
            // (N-2 offset matching PREFER_STABLE_PACKAGES for broadest support).
            if let Some(resolved) = wolfi.get_stable_version_at_offset("python", 2) {
                debug!(from = %pkg, to = %resolved, "Resolved unavailable Python version to stable Wolfi package");
                *pkg = resolved;
            }
        } else if pkg.starts_with("ruby-")
            && !pkg.ends_with("-dev")
            && pkg[5..].chars().next().is_some_and(|c| c.is_ascii_digit())
            && !wolfi.has_package(pkg)
        {
            // ruby-2.6 → resolve to minimum available Ruby version
            // EOL Ruby versions aren't in Wolfi; fall back to oldest available.
            let versions = wolfi.get_versions("ruby");
            if let Some(oldest) = versions.last() {
                let resolved = format!("ruby-{}", oldest);
                debug!(from = %pkg, to = %resolved, "Resolved unavailable Ruby version to oldest Wolfi package");
                *pkg = resolved;
            }
        }
    }

    // Second pass: resolve pip based on python version
    let python_version = packages
        .iter()
        .find(|p| p.starts_with("python-"))
        .and_then(|p| p.strip_prefix("python-"))
        .map(String::from);

    if let Some(py_ver) = python_version {
        for pkg in packages.iter_mut() {
            if pkg == "pip" {
                let pip_pkg = format!("py{}-pip", py_ver);
                if wolfi.has_package(&pip_pkg) {
                    *pkg = pip_pkg;
                } else {
                    // Try just the major.minor
                    let short_ver = py_ver.split('.').take(2).collect::<Vec<_>>().join(".");
                    let pip_pkg = format!("py{}-pip", short_ver);
                    if wolfi.has_package(&pip_pkg) {
                        *pkg = pip_pkg;
                    }
                }
            }
        }
    }

    // Third pass: resolve Ruby-related packages with version context
    // ruby is already resolved to e.g. ruby-3.4 by VERSIONABLE_PACKAGES
    let ruby_version = packages
        .iter()
        .find(|p| {
            p.starts_with("ruby-") && p[5..].chars().next().is_some_and(|c| c.is_ascii_digit())
        })
        .and_then(|p| p.strip_prefix("ruby-"))
        .map(String::from);

    if let Some(rb_ver) = ruby_version {
        for pkg in packages.iter_mut() {
            if pkg == "bundler" || pkg == "ruby-bundler" {
                // ruby-bundler → ruby3.4-bundler (note: no dash between ruby and version)
                let bundler_pkg = format!("ruby{}-bundler", rb_ver);
                if wolfi.has_package(&bundler_pkg) {
                    *pkg = bundler_pkg;
                }
            } else if pkg == "ruby-dev" {
                // ruby-dev → ruby-3.4-dev
                let dev_pkg = format!("ruby-{}-dev", rb_ver);
                if wolfi.has_package(&dev_pkg) {
                    *pkg = dev_pkg;
                }
            }
        }
    }
}
