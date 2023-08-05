use anyhow::{Context as ResultExt, Error, Result};
use once_cell::sync::Lazy;
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::Mutex;

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
    pub fn script(&self, ctx: &Context, warnings: &mut Vec<Error>) -> Result<String> {
        static USED_GET: Lazy<Mutex<bool>> = Lazy::new(|| Mutex::new(false));

        let mut engine = upon::Engine::new();

        engine.add_filter(
            "get",
            |map: &BTreeMap<String, upon::Value>, key: &str| -> Option<upon::Value> {
                *USED_GET.lock().unwrap() = true;
                map.get(key).cloned()
            },
        );
        engine.add_filter("nl", |mut v: upon::Value| -> upon::Value {
            if let upon::Value::String(s) = &mut v {
                if !s.ends_with('\n') {
                    s.push('\n');
                }
            }
            v
        });

        // Compile the templates
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
                    let data = ExternalData {
                        name: &plugin.name,
                        dir: plugin
                            .dir()
                            .to_str()
                            .context("plugin directory is not valid UTF-8")?,
                        files,
                        hooks: &plugin.hooks,
                    };

                    for name in &plugin.apply {
                        let out = &engine
                            .get_template(name)
                            .unwrap()
                            .render(&data)
                            .to_string()
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
                    let data = upon::value! {
                        name: &plugin.name,
                        hooks: &plugin.hooks,
                    };
                    let out = engine
                        .compile(&plugin.raw)
                        .with_context(|| {
                            format!("failed to compile inline plugin `{}`", &plugin.name)
                        })?
                        .render(&data)
                        .to_string()
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

        if *USED_GET.lock().unwrap() {
            warnings.push(Error::msg(
                "use of deprecated filter `get` in [templates], please use the `?.` operator \
                 instead.\nFor example: `{{ hooks | get: \"pre\" | nl }}` can be written `{{ \
                 hook?.pre | nl }}`",
            ));
        }

        Ok(script)
    }
}
