use anyhow::{Context as ResultExt, Result};
use serde::Serialize;
use std::collections::BTreeMap;

use crate::context::Context;
use crate::lock::file::LockedPlugin;
use crate::lock::LockedConfig;

#[derive(Debug, Serialize)]
struct ExternalData<'a> {
    name: &'a str,
    dir: &'a str,
    files: Vec<&'a str>,
    hooks: &'a BTreeMap<String, String>,
}

impl LockedConfig {
    /// Generate the script.
    pub fn script(&self, ctx: &Context) -> Result<String> {
        // Compile the templates
        let mut engine = upon::Engine::new();
        engine.add_filter("contains", |map: BTreeMap<String, _>, key: String| {
            map.contains_key(&key)
        });
        for (name, template) in &self.templates {
            engine
                .add_template(name, template)
                .with_context(|| format!("failed to compile template `{name}`"))?;
        }

        let mut script = String::new();

        for plugin in &self.plugins {
            match plugin {
                LockedPlugin::External(plugin) => {
                    // Data to use in template rendering
                    let mut files = Vec::new();
                    for f in &plugin.files {
                        files.push(f.to_str().context("plugin directory is not valid UTF-8")?);
                    }
                    let empty_hooks = BTreeMap::new();
                    let data = ExternalData {
                        name: &plugin.name,
                        dir: plugin
                            .dir()
                            .to_str()
                            .context("plugin directory is not valid UTF-8")?,
                        files,
                        hooks: plugin.hooks.as_ref().unwrap_or(&empty_hooks),
                    };

                    for name in &plugin.apply {
                        let out = &engine
                            .get_template(name)
                            .unwrap()
                            .render(&data)
                            .with_context(|| format!("failed to render template `{name}`"))?;
                        script.push_str(out);
                        if !out.ends_with('\n') {
                            script.push('\n');
                        }
                    }
                    ctx.log_verbose_status("Rendered", &plugin.name);
                }
                LockedPlugin::Inline(plugin) => {
                    // Data to use in template rendering
                    let empty_hooks = BTreeMap::new();
                    let data = upon::value! {
                        name: &plugin,
                        hooks: plugin.hooks.as_ref().unwrap_or(&empty_hooks),
                    };
                    let out = engine
                        .compile(&plugin.raw)
                        .with_context(|| {
                            format!("failed to compile inline plugin `{}`", &plugin.name)
                        })?
                        .render(&data)
                        .with_context(|| {
                            format!("failed to render inline plugin `{}`", &plugin.name)
                        })?;
                    script.push_str(&out);
                    if !out.ends_with('\n') {
                        script.push('\n');
                    }
                    ctx.log_verbose_status("Inlined", &plugin.name);
                }
            }
        }

        Ok(script)
    }
}
