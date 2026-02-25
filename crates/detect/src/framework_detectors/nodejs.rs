use crate::ids::{FrameworkId, FrameworkMeta, LanguageId};
use crate::types::{Dependency, FrameworkContribution};
use std::collections::BTreeMap;

const JS: LanguageId = LanguageId::new("javascript");
const TS: LanguageId = LanguageId::new("typescript");

// ── Server Frameworks ──────────────────────────────────────────────────────

const EXPRESS: FrameworkId = FrameworkId::new("express");
inventory::submit! { FrameworkMeta { slug: "express", display_name: "Express", aliases: &[] } }

// Express needs special handling: exclude projects where express is used as a
// transitive dependency by a higher-level framework (Angular SSR, NestJS, etc.).
pub struct ExpressDetector;

impl crate::traits::FrameworkDetector for ExpressDetector {
    fn id(&self) -> FrameworkId {
        EXPRESS
    }

    fn compatible_languages(&self) -> &[LanguageId] {
        &[JS, TS]
    }

    fn detect(&self, deps: &[Dependency]) -> bool {
        let has_express = deps.iter().any(|d| d.name == "express");
        if !has_express {
            return false;
        }
        // Exclude projects where express is used by a higher-level framework
        !deps.iter().any(|d| {
            d.name == "@angular/ssr"
                || d.name == "@nguniversal/express-engine"
                || d.name == "@nestjs/core"
        })
    }

    fn contribution(&self, _deps: &[Dependency]) -> FrameworkContribution {
        FrameworkContribution {
            framework: EXPRESS,
            default_ports: vec![3000],
            health_endpoints: vec!["/health".into(), "/healthz".into(), "/ping".into()],
            env_vars: BTreeMap::new(),
            runtime_packages: vec![],
            runtime_command: None,
            runtime_env: BTreeMap::new(),
            workdir: None,
            extra_copy: vec![],
        }
    }
}

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(ExpressDetector))
}

const NEXTJS: FrameworkId = FrameworkId::new("nextjs");
inventory::submit! { FrameworkMeta { slug: "nextjs", display_name: "Next.js", aliases: &[] } }

super::simple_detector!(
    NextJsDetector,
    NEXTJS,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "next"),
    vec![3000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(NextJsDetector))
}

const NESTJS: FrameworkId = FrameworkId::new("nestjs");
inventory::submit! { FrameworkMeta { slug: "nestjs", display_name: "NestJS", aliases: &[] } }

super::simple_detector!(
    NestJsDetector,
    NESTJS,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "@nestjs/core"),
    vec![3000],
    vec!["/health".into(), "/healthz".into()],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(NestJsDetector))
}

const FASTIFY: FrameworkId = FrameworkId::new("fastify");
inventory::submit! { FrameworkMeta { slug: "fastify", display_name: "Fastify", aliases: &[] } }

super::simple_detector!(
    FastifyDetector,
    FASTIFY,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "fastify"),
    vec![3000],
    vec!["/health".into()],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(FastifyDetector))
}

const HONO: FrameworkId = FrameworkId::new("hono");
inventory::submit! { FrameworkMeta { slug: "hono", display_name: "Hono", aliases: &[] } }

super::simple_detector!(
    HonoDetector,
    HONO,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "hono"),
    vec![3000],
    vec!["/health".into(), "/healthz".into()],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(HonoDetector))
}

const KOA: FrameworkId = FrameworkId::new("koa");
inventory::submit! { FrameworkMeta { slug: "koa", display_name: "Koa", aliases: &[] } }

super::simple_detector!(
    KoaDetector,
    KOA,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "koa"),
    vec![3000],
    vec!["/health".into(), "/healthz".into()],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(KoaDetector))
}

const H3: FrameworkId = FrameworkId::new("h3");
inventory::submit! { FrameworkMeta { slug: "h3", display_name: "H3", aliases: &[] } }

super::simple_detector!(
    H3Detector,
    H3,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "h3"),
    vec![3000],
    vec!["/health".into(), "/healthz".into()],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(H3Detector))
}

const NITRO: FrameworkId = FrameworkId::new("nitro");
inventory::submit! { FrameworkMeta { slug: "nitro", display_name: "Nitro", aliases: &[] } }

super::simple_detector!(
    NitroDetector,
    NITRO,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "nitropack"),
    vec![3000],
    vec!["/health".into(), "/healthz".into()],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(NitroDetector))
}

// ── SSR / Full-stack Frameworks ────────────────────────────────────────────

const NUXT: FrameworkId = FrameworkId::new("nuxt");
inventory::submit! { FrameworkMeta { slug: "nuxt", display_name: "Nuxt", aliases: &[] } }

super::simple_detector!(
    NuxtDetector,
    NUXT,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| {
        d.name == "nuxt" || d.name == "nuxt3" || d.name == "nuxt-edge" || d.name == "nuxt-nightly"
    }),
    vec![3000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(NuxtDetector))
}

const REMIX: FrameworkId = FrameworkId::new("remix");
inventory::submit! { FrameworkMeta { slug: "remix", display_name: "Remix", aliases: &[] } }

super::simple_detector!(
    RemixDetector,
    REMIX,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "@remix-run/dev"),
    vec![3000],
    vec!["/healthcheck".into()],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(RemixDetector))
}

const REACT_ROUTER: FrameworkId = FrameworkId::new("react-router");
inventory::submit! { FrameworkMeta { slug: "react-router", display_name: "React Router", aliases: &[] } }

super::simple_detector!(
    ReactRouterDetector,
    REACT_ROUTER,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "@react-router/dev"),
    vec![3000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(ReactRouterDetector))
}

const ASTRO: FrameworkId = FrameworkId::new("astro");
inventory::submit! { FrameworkMeta { slug: "astro", display_name: "Astro", aliases: &[] } }

super::simple_detector!(
    AstroDetector,
    ASTRO,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "astro"),
    vec![4321],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(AstroDetector))
}

const SVELTEKIT: FrameworkId = FrameworkId::new("sveltekit");
inventory::submit! { FrameworkMeta { slug: "sveltekit", display_name: "SvelteKit", aliases: &[] } }

super::simple_detector!(
    SvelteKitDetector,
    SVELTEKIT,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "@sveltejs/kit"),
    vec![3000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(SvelteKitDetector))
}

const GATSBY: FrameworkId = FrameworkId::new("gatsby");
inventory::submit! { FrameworkMeta { slug: "gatsby", display_name: "Gatsby", aliases: &[] } }

super::simple_detector!(
    GatsbyDetector,
    GATSBY,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "gatsby"),
    vec![8000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(GatsbyDetector))
}

const SOLID_START: FrameworkId = FrameworkId::new("solid-start");
inventory::submit! { FrameworkMeta { slug: "solid-start", display_name: "SolidStart", aliases: &[] } }

super::simple_detector!(
    SolidStartDetector,
    SOLID_START,
    &[JS, TS],
    |deps: &[Dependency]| {
        deps.iter()
            .any(|d| d.name == "@solidjs/start" || d.name == "solid-start")
    },
    vec![3000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(SolidStartDetector))
}

const REDWOODJS: FrameworkId = FrameworkId::new("redwoodjs");
inventory::submit! { FrameworkMeta { slug: "redwoodjs", display_name: "RedwoodJS", aliases: &[] } }

super::simple_detector!(
    RedwoodJsDetector,
    REDWOODJS,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "@redwoodjs/core"),
    vec![8910],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(RedwoodJsDetector))
}

const HYDROGEN: FrameworkId = FrameworkId::new("hydrogen");
inventory::submit! { FrameworkMeta { slug: "hydrogen", display_name: "Hydrogen", aliases: &[] } }

super::simple_detector!(
    HydrogenDetector,
    HYDROGEN,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "@shopify/hydrogen"),
    vec![3000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(HydrogenDetector))
}

const TANSTACK_START: FrameworkId = FrameworkId::new("tanstack-start");
inventory::submit! { FrameworkMeta { slug: "tanstack-start", display_name: "TanStack Start", aliases: &[] } }

super::simple_detector!(
    TanStackStartDetector,
    TANSTACK_START,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "@tanstack/router-plugin"),
    vec![3000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(TanStackStartDetector))
}

const BLITZ: FrameworkId = FrameworkId::new("blitz");
inventory::submit! { FrameworkMeta { slug: "blitz", display_name: "Blitz", aliases: &[] } }

super::simple_detector!(
    BlitzDetector,
    BLITZ,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "blitz"),
    vec![3000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(BlitzDetector))
}

// ── Frontend Build / SSG Frameworks ────────────────────────────────────────

const VITE: FrameworkId = FrameworkId::new("vite");
inventory::submit! { FrameworkMeta { slug: "vite", display_name: "Vite", aliases: &[] } }

// Vite needs special handling: exclude meta-frameworks that bundle Vite internally.
pub struct ViteDetector;

impl crate::traits::FrameworkDetector for ViteDetector {
    fn id(&self) -> FrameworkId {
        VITE
    }

    fn compatible_languages(&self) -> &[LanguageId] {
        &[JS, TS]
    }

    fn detect(&self, deps: &[Dependency]) -> bool {
        let has_vite = deps.iter().any(|d| d.name == "vite");
        if !has_vite {
            return false;
        }
        // Exclude if a more specific Vite-based framework is present
        const VITE_FRAMEWORKS: &[&str] = &[
            "@sveltejs/kit",
            "astro",
            "@solidjs/start",
            "solid-start",
            "vitepress",
            "nuxt",
            "nuxt3",
            "nuxt-edge",
            "nuxt-nightly",
            "@remix-run/dev",
            "@react-router/dev",
            "@tanstack/router-plugin",
            "@shopify/hydrogen",
        ];
        !deps
            .iter()
            .any(|d| VITE_FRAMEWORKS.contains(&d.name.as_str()))
    }

    fn contribution(&self, _deps: &[Dependency]) -> FrameworkContribution {
        FrameworkContribution {
            framework: VITE,
            default_ports: vec![5173],
            health_endpoints: vec![],
            env_vars: BTreeMap::new(),
            runtime_packages: vec![],
            runtime_command: None,
            runtime_env: BTreeMap::new(),
            workdir: None,
            extra_copy: vec![],
        }
    }
}

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(ViteDetector))
}

const CREATE_REACT_APP: FrameworkId = FrameworkId::new("create-react-app");
inventory::submit! { FrameworkMeta { slug: "create-react-app", display_name: "Create React App", aliases: &[] } }

super::simple_detector!(
    CreateReactAppDetector,
    CREATE_REACT_APP,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "react-scripts"),
    vec![3000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(CreateReactAppDetector))
}

const ANGULAR: FrameworkId = FrameworkId::new("angular");
inventory::submit! { FrameworkMeta { slug: "angular", display_name: "Angular", aliases: &[] } }

// Angular needs special handling: exclude SSR projects (detected by AngularSsrDetector).
pub struct AngularDetector;

impl crate::traits::FrameworkDetector for AngularDetector {
    fn id(&self) -> FrameworkId {
        ANGULAR
    }

    fn compatible_languages(&self) -> &[LanguageId] {
        &[JS, TS]
    }

    fn detect(&self, deps: &[Dependency]) -> bool {
        let has_angular = deps.iter().any(|d| d.name == "@angular/cli");
        if !has_angular {
            return false;
        }
        // Exclude Angular SSR projects (handled by AngularSsrDetector)
        !deps
            .iter()
            .any(|d| d.name == "@angular/ssr" || d.name == "@nguniversal/express-engine")
    }

    fn contribution(&self, _deps: &[Dependency]) -> FrameworkContribution {
        FrameworkContribution {
            framework: ANGULAR,
            default_ports: vec![4200],
            health_endpoints: vec![],
            env_vars: BTreeMap::new(),
            runtime_packages: vec![],
            runtime_command: None,
            runtime_env: BTreeMap::new(),
            workdir: None,
            extra_copy: vec![],
        }
    }
}

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(AngularDetector))
}

const ANGULAR_SSR: FrameworkId = FrameworkId::new("angular-ssr");
inventory::submit! { FrameworkMeta { slug: "angular-ssr", display_name: "Angular SSR", aliases: &[] } }

super::simple_detector!(
    AngularSsrDetector,
    ANGULAR_SSR,
    &[JS, TS],
    |deps: &[Dependency]| {
        deps.iter()
            .any(|d| d.name == "@angular/ssr" || d.name == "@nguniversal/express-engine")
    },
    vec![4000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(AngularSsrDetector))
}

const VUE_CLI: FrameworkId = FrameworkId::new("vue-cli");
inventory::submit! { FrameworkMeta { slug: "vue-cli", display_name: "Vue CLI", aliases: &[] } }

super::simple_detector!(
    VueCliDetector,
    VUE_CLI,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "@vue/cli-service"),
    vec![8080],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(VueCliDetector))
}

const PREACT_CLI: FrameworkId = FrameworkId::new("preact-cli");
inventory::submit! { FrameworkMeta { slug: "preact-cli", display_name: "Preact CLI", aliases: &[] } }

super::simple_detector!(
    PreactCliDetector,
    PREACT_CLI,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "preact-cli"),
    vec![8080],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(PreactCliDetector))
}

const EMBER: FrameworkId = FrameworkId::new("ember");
inventory::submit! { FrameworkMeta { slug: "ember", display_name: "Ember", aliases: &[] } }

super::simple_detector!(
    EmberDetector,
    EMBER,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "ember-cli"),
    vec![4200],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(EmberDetector))
}

const SVELTE: FrameworkId = FrameworkId::new("svelte");
inventory::submit! { FrameworkMeta { slug: "svelte", display_name: "Svelte", aliases: &[] } }

super::simple_detector!(
    SvelteDetector,
    SVELTE,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "sirv-cli"),
    vec![5000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(SvelteDetector))
}

const DOCUSAURUS: FrameworkId = FrameworkId::new("docusaurus");
inventory::submit! { FrameworkMeta { slug: "docusaurus", display_name: "Docusaurus", aliases: &[] } }

super::simple_detector!(
    DocusaurusDetector,
    DOCUSAURUS,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "@docusaurus/core"),
    vec![3000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(DocusaurusDetector))
}

const ELEVENTY: FrameworkId = FrameworkId::new("eleventy");
inventory::submit! { FrameworkMeta { slug: "eleventy", display_name: "Eleventy", aliases: &[] } }

super::simple_detector!(
    EleventyDetector,
    ELEVENTY,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "@11ty/eleventy"),
    vec![8080],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(EleventyDetector))
}

const VITEPRESS: FrameworkId = FrameworkId::new("vitepress");
inventory::submit! { FrameworkMeta { slug: "vitepress", display_name: "VitePress", aliases: &[] } }

super::simple_detector!(
    VitePressDetector,
    VITEPRESS,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "vitepress"),
    vec![5173],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(VitePressDetector))
}

const VUEPRESS: FrameworkId = FrameworkId::new("vuepress");
inventory::submit! { FrameworkMeta { slug: "vuepress", display_name: "VuePress", aliases: &[] } }

super::simple_detector!(
    VuePressDetector,
    VUEPRESS,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "vuepress"),
    vec![8080],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(VuePressDetector))
}

const STENCIL: FrameworkId = FrameworkId::new("stencil");
inventory::submit! { FrameworkMeta { slug: "stencil", display_name: "Stencil", aliases: &[] } }

super::simple_detector!(
    StencilDetector,
    STENCIL,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "@stencil/core"),
    vec![3333],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(StencilDetector))
}

const PARCEL: FrameworkId = FrameworkId::new("parcel");
inventory::submit! { FrameworkMeta { slug: "parcel", display_name: "Parcel", aliases: &[] } }

super::simple_detector!(
    ParcelDetector,
    PARCEL,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "parcel"),
    vec![1234],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(ParcelDetector))
}

const HEXO: FrameworkId = FrameworkId::new("hexo");
inventory::submit! { FrameworkMeta { slug: "hexo", display_name: "Hexo", aliases: &[] } }

super::simple_detector!(
    HexoDetector,
    HEXO,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "hexo"),
    vec![4000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(HexoDetector))
}

const GRIDSOME: FrameworkId = FrameworkId::new("gridsome");
inventory::submit! { FrameworkMeta { slug: "gridsome", display_name: "Gridsome", aliases: &[] } }

super::simple_detector!(
    GridsomeDetector,
    GRIDSOME,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "gridsome"),
    vec![8080],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(GridsomeDetector))
}

const BRUNCH: FrameworkId = FrameworkId::new("brunch");
inventory::submit! { FrameworkMeta { slug: "brunch", display_name: "Brunch", aliases: &[] } }

super::simple_detector!(
    BrunchDetector,
    BRUNCH,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "brunch"),
    vec![3333],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(BrunchDetector))
}

const STORYBOOK: FrameworkId = FrameworkId::new("storybook");
inventory::submit! { FrameworkMeta { slug: "storybook", display_name: "Storybook", aliases: &[] } }

super::simple_detector!(
    StorybookDetector,
    STORYBOOK,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "storybook"),
    vec![6006],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(StorybookDetector))
}

const SANITY: FrameworkId = FrameworkId::new("sanity");
inventory::submit! { FrameworkMeta { slug: "sanity", display_name: "Sanity", aliases: &[] } }

super::simple_detector!(
    SanityDetector,
    SANITY,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "sanity"),
    vec![3333],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(SanityDetector))
}

// ── Legacy / Niche Frameworks ──────────────────────────────────────────────

const SAPPER: FrameworkId = FrameworkId::new("sapper");
inventory::submit! { FrameworkMeta { slug: "sapper", display_name: "Sapper", aliases: &[] } }

super::simple_detector!(
    SapperDetector,
    SAPPER,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "sapper"),
    vec![3000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(SapperDetector))
}

const SABER: FrameworkId = FrameworkId::new("saber");
inventory::submit! { FrameworkMeta { slug: "saber", display_name: "Saber", aliases: &[] } }

super::simple_detector!(
    SaberDetector,
    SABER,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "saber"),
    vec![3000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(SaberDetector))
}

const UMIJS: FrameworkId = FrameworkId::new("umijs");
inventory::submit! { FrameworkMeta { slug: "umijs", display_name: "UmiJS", aliases: &[] } }

super::simple_detector!(
    UmiJsDetector,
    UMIJS,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "umi"),
    vec![8000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(UmiJsDetector))
}

const DOJO: FrameworkId = FrameworkId::new("dojo");
inventory::submit! { FrameworkMeta { slug: "dojo", display_name: "Dojo", aliases: &[] } }

super::simple_detector!(
    DojoDetector,
    DOJO,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "@dojo/framework"),
    vec![9999],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(DojoDetector))
}

const POLYMER: FrameworkId = FrameworkId::new("polymer");
inventory::submit! { FrameworkMeta { slug: "polymer", display_name: "Polymer", aliases: &[] } }

super::simple_detector!(
    PolymerDetector,
    POLYMER,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "polymer-cli"),
    vec![8080],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(PolymerDetector))
}

// ── Ionic / Angular Variants ───────────────────────────────────────────────

const IONIC_ANGULAR: FrameworkId = FrameworkId::new("ionic-angular");
inventory::submit! { FrameworkMeta { slug: "ionic-angular", display_name: "Ionic Angular", aliases: &[] } }

super::simple_detector!(
    IonicAngularDetector,
    IONIC_ANGULAR,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "@ionic/angular"),
    vec![8100],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(IonicAngularDetector))
}

const IONIC_REACT: FrameworkId = FrameworkId::new("ionic-react");
inventory::submit! { FrameworkMeta { slug: "ionic-react", display_name: "Ionic React", aliases: &[] } }

super::simple_detector!(
    IonicReactDetector,
    IONIC_REACT,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "@ionic/react"),
    vec![8100],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(IonicReactDetector))
}

const SCULLY: FrameworkId = FrameworkId::new("scully");
inventory::submit! { FrameworkMeta { slug: "scully", display_name: "Scully", aliases: &[] } }

super::simple_detector!(
    ScullyDetector,
    SCULLY,
    &[JS, TS],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "@scullyio/init"),
    vec![1668],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(ScullyDetector))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::FrameworkDetector;
    use crate::types::DepScope;

    fn dep(name: &str) -> Dependency {
        Dependency {
            name: name.to_string(),
            version: None,
            scope: DepScope::Runtime,
            is_internal: false,
        }
    }

    #[test]
    fn angular_spa_detected_without_ssr() {
        let detector = AngularDetector;
        assert!(detector.detect(&[dep("@angular/cli"), dep("@angular/core")]));
    }

    #[test]
    fn angular_spa_excluded_when_angular_ssr_present() {
        let detector = AngularDetector;
        assert!(!detector.detect(&[
            dep("@angular/cli"),
            dep("@angular/core"),
            dep("@angular/ssr"),
        ]));
    }

    #[test]
    fn angular_spa_excluded_when_nguniversal_present() {
        let detector = AngularDetector;
        assert!(!detector.detect(&[
            dep("@angular/cli"),
            dep("@angular/core"),
            dep("@nguniversal/express-engine"),
        ]));
    }

    #[test]
    fn angular_ssr_detected_with_angular_ssr_package() {
        let detector = AngularSsrDetector;
        assert!(detector.detect(&[dep("@angular/ssr"), dep("@angular/core")]));
    }

    #[test]
    fn angular_ssr_detected_with_nguniversal() {
        let detector = AngularSsrDetector;
        assert!(detector.detect(&[dep("@nguniversal/express-engine"), dep("@angular/core"),]));
    }

    #[test]
    fn angular_ssr_not_detected_without_ssr_deps() {
        let detector = AngularSsrDetector;
        assert!(!detector.detect(&[dep("@angular/cli"), dep("@angular/core")]));
    }
}
