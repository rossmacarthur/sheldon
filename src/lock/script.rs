use anyhow::{Context as ResultExt, Result};
use serde::Serialize;
use std::collections::BTreeMap;

use crate::context::Context;
use crate::lock::file::LockedPlugin;
use crate::lock::LockedConfig;

#[derive(Serialize)]
struct Data<'a> {
    data_dir: &'a str,
    name: &'a str,
    dir: Option<&'a str>,
    file: Option<&'a str>,
    hooks: Option<&'a BTreeMap<String, String>>,
}

impl LockedConfig {
    /// Generate the script.
    pub fn script(&self, ctx: &Context) -> Result<String> {
        // Compile the templates
        let mut templates = handlebars::Handlebars::new();
        templates.set_strict_mode(true);
        templates.register_escape_fn(handlebars::no_escape);
        for (name, template) in &self.templates {
            templates
                .register_template_string(name, &template.value)
                .with_context(s!("failed to compile template `{}`", name))?;
        }

        let mut script = String::new();

        for plugin in &self.plugins {
            match plugin {
                LockedPlugin::External(plugin) => {
                    for name in &plugin.apply {
                        let dir_as_str = plugin
                            .dir()
                            .to_str()
                            .context("plugin directory is not valid UTF-8")?;

                        // Data to use in template rendering
                        let mut data = Data {
                            data_dir: self
                                .ctx
                                .data_dir()
                                .to_str()
                                .context("data directory is not valid UTF-8")?,
                            name: &plugin.name,
                            dir: Some(dir_as_str),
                            file: None,
                            hooks: plugin.hooks.as_ref(),
                        };

                        if self.templates.get(name.as_str()).unwrap().each {
                            for file in &plugin.files {
                                let as_str =
                                    file.to_str().context("plugin file is not valid UTF-8")?;
                                data.file = Some(as_str);
                                script.push_str(
                                    &templates
                                        .render(name, &data)
                                        .with_context(s!("failed to render template `{}`", name))?,
                                );
                                script.push('\n');
                            }
                        } else {
                            script.push_str(
                                &templates
                                    .render(name, &data)
                                    .with_context(s!("failed to render template `{}`", name))?,
                            );
                            script.push('\n');
                        }
                    }
                    status_v!(ctx, "Rendered", &plugin.name);
                }
                LockedPlugin::Inline(plugin) => {
                    let data = Data {
                        data_dir: self
                            .ctx
                            .data_dir()
                            .to_str()
                            .context("data directory is not valid UTF-8")?,
                        name: &plugin.name,
                        dir: None,
                        file: None,
                        hooks: plugin.hooks.as_ref(),
                    };
                    script.push_str(
                        &templates
                            .render_template(&plugin.raw, &data)
                            .with_context(s!(
                                "failed to render inline plugin `{}`",
                                &plugin.name
                            ))?,
                    );
                    script.push('\n');
                    status_v!(ctx, "Inlined", &plugin.name);
                }
            }
        }

        Ok(script)
    }
}
