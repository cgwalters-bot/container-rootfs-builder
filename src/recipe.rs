//! Blueprint recipe parsing and validation.

use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Blueprint {
    #[serde(default)]
    packages: Vec<Package>,
    customizations: Option<Customizations>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Package {
    name: String,
    version: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Customizations {
    sshkey: Option<Vec<serde::de::IgnoredAny>>,
}

pub(crate) fn parse(source: &str, extension: Option<&str>) -> Result<Vec<String>> {
    let blueprint: Blueprint = match extension {
        Some("toml") => toml::from_str(source).context("parsing TOML blueprint")?,
        Some("ncl") => nickel_lang_core::deserialize::from_str(source)
            .map_err(|e| anyhow::anyhow!("evaluating Nickel recipe: {e}"))?,
        _ => bail!("recipe must have a .toml or .ncl extension"),
    };
    if blueprint.customizations.is_some_and(|x| x.sshkey.is_some()) {
        bail!(
            "blueprint customizations.sshkey is unsupported: source-backed planning cannot safely apply it"
        )
    }
    blueprint
        .packages
        .into_iter()
        .map(|p| {
            valid_package(&p.name)?;
            let value = match p.version {
                Some(version) => {
                    valid_package(&version)?;
                    format!("{}-{version}", p.name)
                }
                None => p.name,
            };
            Ok(value)
        })
        .collect()
}

pub(crate) fn read(path: &Path) -> Result<Vec<String>> {
    let source = std::fs::read_to_string(path)
        .with_context(|| format!("reading recipe {}", path.display()))?;
    parse(
        &source,
        path.extension().and_then(|extension| extension.to_str()),
    )
}

fn valid_package(value: &str) -> Result<()> {
    if value.is_empty()
        || !value.bytes().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'+' | b'-' | b':' | b'~')
        })
    {
        bail!("invalid package or version {value:?}")
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use anyhow::Result;
    use tempfile::NamedTempFile;

    use super::{parse, read};

    #[test]
    fn reads_recipes() -> Result<()> {
        let mut recipe = NamedTempFile::with_suffix(".toml")?;
        writeln!(recipe, "[[packages]]\nname=\"bash\"\nversion=\"5.2\"")?;
        assert_eq!(read(recipe.path())?, ["bash-5.2"]);
        writeln!(recipe, "\n[[packages]]\nname=\"bad\"\nversion=\"?\"")?;
        assert!(read(recipe.path()).is_err());
        Ok(())
    }

    #[test]
    fn parses_recipes_without_io() -> Result<()> {
        assert_eq!(
            parse(
                "[[packages]]\nname = \"bash\"\nversion = \"5.2\"\n",
                Some("toml")
            )?,
            ["bash-5.2"]
        );
        assert!(parse("[[packages]]\nname = \"bad value\"\n", Some("toml")).is_err());
        assert!(parse("{}", Some("json")).is_err());
        Ok(())
    }
}
