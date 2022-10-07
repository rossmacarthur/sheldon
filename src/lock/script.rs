use anyhow::{Context as ResultExt, Result};
use maplit::hashmap;

use crate::context::Context;
use crate::lock::file::LockedPlugin;
use crate::lock::LockedConfig;

impl LockedConfig {
    /// Generate the script.
    pub fn script(&self, ctx: &Context) -> Result<String> {
        // Compile the templates
        let mut engine = upon::Engine::new();
        for (name, template) in &self.templates {
            engine
                .add_template(name, &template.value)
                .with_context(s!("failed to compile template `{}`", name))?;
        }

        macro_rules! render {
            ($name:expr, $data:expr) => {
                &engine
                    .get_template($name)
                    .unwrap()
                    .render($data)
                    .with_context(s!("failed to render template `{}`", $name))?
            };
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
                        let mut data = hashmap! {
                            "data_dir" => self
                                .ctx
                                .data_dir()
                                .to_str()
                                .context("data directory is not valid UTF-8")?,
                            "name" => &plugin.name,
                            "dir" => dir_as_str,
                        };

                        if self.templates.get(name.as_str()).unwrap().each {
                            for file in &plugin.files {
                                let as_str =
                                    file.to_str().context("plugin file is not valid UTF-8")?;
                                data.insert("file", as_str);
                                script.push_str(render!(name, &data));
                                script.push('\n');
                            }
                        } else {
                            script.push_str(render!(name, &data));
                            script.push('\n');
                        }
                    }
                    status_v!(ctx, "Rendered", &plugin.name);
                }
                LockedPlugin::Inline(plugin) => {
                    let data = hashmap! {
                        "data_dir" => self
                            .ctx
                            .data_dir()
                            .to_str()
                            .context("data directory is not valid UTF-8")?,
                        "name" => &plugin.name,
                    };
                    script.push_str(
                        &engine
                            .compile(&plugin.raw)
                            .with_context(s!("failed to compile inline plugin `{}`", &plugin.name))?
                            .render(&data)
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
