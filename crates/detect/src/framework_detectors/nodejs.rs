use crate::id_enums::{FrameworkId, LanguageId};
use crate::types::{Dependency, FrameworkContribution};
use std::collections::BTreeMap;

// ── Server Frameworks ──────────────────────────────────────────────────────

super::simple_detector!(
    ExpressDetector,
    FrameworkId::Express,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "express"),
    vec![3000],
    vec!["/health".into(), "/healthz".into(), "/ping".into()],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(ExpressDetector))
}

super::simple_detector!(
    NextJsDetector,
    FrameworkId::NextJs,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "next"),
    vec![3000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(NextJsDetector))
}

super::simple_detector!(
    NestJsDetector,
    FrameworkId::NestJs,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "@nestjs/core"),
    vec![3000],
    vec!["/health".into(), "/healthz".into()],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(NestJsDetector))
}

super::simple_detector!(
    FastifyDetector,
    FrameworkId::Fastify,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "fastify"),
    vec![3000],
    vec!["/health".into()],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(FastifyDetector))
}

super::simple_detector!(
    HonoDetector,
    FrameworkId::Hono,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "hono"),
    vec![3000],
    vec!["/health".into(), "/healthz".into()],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(HonoDetector))
}

super::simple_detector!(
    KoaDetector,
    FrameworkId::Koa,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "koa"),
    vec![3000],
    vec!["/health".into(), "/healthz".into()],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(KoaDetector))
}

super::simple_detector!(
    H3Detector,
    FrameworkId::H3,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "h3"),
    vec![3000],
    vec!["/health".into(), "/healthz".into()],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(H3Detector))
}

super::simple_detector!(
    NitroDetector,
    FrameworkId::Nitro,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
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

super::simple_detector!(
    NuxtDetector,
    FrameworkId::Nuxt,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| {
        d.name == "nuxt"
            || d.name == "nuxt3"
            || d.name == "nuxt-edge"
            || d.name == "nuxt-nightly"
    }),
    vec![3000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(NuxtDetector))
}

super::simple_detector!(
    RemixDetector,
    FrameworkId::Remix,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "@remix-run/dev"),
    vec![3000],
    vec!["/healthcheck".into()],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(RemixDetector))
}

super::simple_detector!(
    ReactRouterDetector,
    FrameworkId::ReactRouter,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "@react-router/dev"),
    vec![3000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(ReactRouterDetector))
}

super::simple_detector!(
    AstroDetector,
    FrameworkId::Astro,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "astro"),
    vec![4321],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(AstroDetector))
}

super::simple_detector!(
    SvelteKitDetector,
    FrameworkId::SvelteKit,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "@sveltejs/kit"),
    vec![3000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(SvelteKitDetector))
}

super::simple_detector!(
    GatsbyDetector,
    FrameworkId::Gatsby,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "gatsby"),
    vec![8000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(GatsbyDetector))
}

super::simple_detector!(
    SolidStartDetector,
    FrameworkId::SolidStart,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
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

super::simple_detector!(
    RedwoodJsDetector,
    FrameworkId::RedwoodJs,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "@redwoodjs/core"),
    vec![8910],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(RedwoodJsDetector))
}

super::simple_detector!(
    HydrogenDetector,
    FrameworkId::Hydrogen,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "@shopify/hydrogen"),
    vec![3000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(HydrogenDetector))
}

super::simple_detector!(
    TanStackStartDetector,
    FrameworkId::TanStackStart,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "@tanstack/router-plugin"),
    vec![3000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(TanStackStartDetector))
}

super::simple_detector!(
    BlitzDetector,
    FrameworkId::Blitz,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
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

// Vite needs special handling: exclude meta-frameworks that bundle Vite internally.
pub struct ViteDetector;

impl crate::traits::FrameworkDetector for ViteDetector {
    fn id(&self) -> FrameworkId {
        FrameworkId::Vite
    }

    fn compatible_languages(&self) -> &[LanguageId] {
        &[LanguageId::JavaScript, LanguageId::TypeScript]
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
            "@tanstack/router-plugin",
            "@shopify/hydrogen",
        ];
        !deps
            .iter()
            .any(|d| VITE_FRAMEWORKS.contains(&d.name.as_str()))
    }

    fn contribution(&self, _deps: &[Dependency]) -> FrameworkContribution {
        FrameworkContribution {
            framework: FrameworkId::Vite,
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

super::simple_detector!(
    CreateReactAppDetector,
    FrameworkId::CreateReactApp,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "react-scripts"),
    vec![3000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(CreateReactAppDetector))
}

super::simple_detector!(
    AngularDetector,
    FrameworkId::Angular,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "@angular/cli"),
    vec![4200],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(AngularDetector))
}

super::simple_detector!(
    VueCliDetector,
    FrameworkId::VueCli,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "@vue/cli-service"),
    vec![8080],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(VueCliDetector))
}

super::simple_detector!(
    PreactCliDetector,
    FrameworkId::PreactCli,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "preact-cli"),
    vec![8080],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(PreactCliDetector))
}

super::simple_detector!(
    EmberDetector,
    FrameworkId::Ember,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "ember-cli"),
    vec![4200],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(EmberDetector))
}

super::simple_detector!(
    SvelteDetector,
    FrameworkId::Svelte,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "sirv-cli"),
    vec![5000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(SvelteDetector))
}

super::simple_detector!(
    DocusaurusDetector,
    FrameworkId::Docusaurus,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "@docusaurus/core"),
    vec![3000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(DocusaurusDetector))
}

super::simple_detector!(
    EleventyDetector,
    FrameworkId::Eleventy,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "@11ty/eleventy"),
    vec![8080],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(EleventyDetector))
}

super::simple_detector!(
    VitePressDetector,
    FrameworkId::VitePress,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "vitepress"),
    vec![5173],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(VitePressDetector))
}

super::simple_detector!(
    VuePressDetector,
    FrameworkId::VuePress,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "vuepress"),
    vec![8080],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(VuePressDetector))
}

super::simple_detector!(
    StencilDetector,
    FrameworkId::Stencil,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "@stencil/core"),
    vec![3333],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(StencilDetector))
}

super::simple_detector!(
    ParcelDetector,
    FrameworkId::Parcel,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "parcel"),
    vec![1234],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(ParcelDetector))
}

super::simple_detector!(
    HexoDetector,
    FrameworkId::Hexo,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "hexo"),
    vec![4000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(HexoDetector))
}

super::simple_detector!(
    GridsomeDetector,
    FrameworkId::Gridsome,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "gridsome"),
    vec![8080],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(GridsomeDetector))
}

super::simple_detector!(
    BrunchDetector,
    FrameworkId::Brunch,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "brunch"),
    vec![3333],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(BrunchDetector))
}

super::simple_detector!(
    StorybookDetector,
    FrameworkId::Storybook,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "storybook"),
    vec![6006],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(StorybookDetector))
}

super::simple_detector!(
    SanityDetector,
    FrameworkId::Sanity,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
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

super::simple_detector!(
    SapperDetector,
    FrameworkId::Sapper,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "sapper"),
    vec![3000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(SapperDetector))
}

super::simple_detector!(
    SaberDetector,
    FrameworkId::Saber,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "saber"),
    vec![3000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(SaberDetector))
}

super::simple_detector!(
    UmiJsDetector,
    FrameworkId::UmiJs,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "umi"),
    vec![8000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(UmiJsDetector))
}

super::simple_detector!(
    DojoDetector,
    FrameworkId::Dojo,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "@dojo/framework"),
    vec![9999],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(DojoDetector))
}

super::simple_detector!(
    PolymerDetector,
    FrameworkId::Polymer,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
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

super::simple_detector!(
    IonicAngularDetector,
    FrameworkId::IonicAngular,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "@ionic/angular"),
    vec![8100],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(IonicAngularDetector))
}

super::simple_detector!(
    IonicReactDetector,
    FrameworkId::IonicReact,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "@ionic/react"),
    vec![8100],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(IonicReactDetector))
}

super::simple_detector!(
    ScullyDetector,
    FrameworkId::Scully,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "@scullyio/init"),
    vec![1668],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(ScullyDetector))
}
