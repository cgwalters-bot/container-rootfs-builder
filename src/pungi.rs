//! Conservative literal checks for the selected Pungi configuration.

use anyhow::{Context, Result, bail};

pub(crate) fn validate(bytes: &[u8]) -> Result<()> {
    let text = std::str::from_utf8(bytes).context("Pungi entrypoint is not UTF-8")?;
    if text.contains("'''") || text.contains("\"\"\"") {
        bail!("Pungi literal check rejects triple-quoted strings")
    }
    let assignment = |key: &str, value: &str| -> bool {
        text.lines().any(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return false;
            }
            let pattern = format!("{key} = '{value}'");
            line == pattern || line == format!("{key} = \"{value}\"")
        })
    };
    let has_profile = text.lines().any(|line| {
        let line = line.trim();
        line == "'kiwi_profile': 'Container-Base-Generic',"
            || line == "\"kiwi_profile\": \"Container-Base-Generic\","
    });
    if !assignment("release_version", "45")
        || !assignment("kiwibuild_description_path", "Fedora.kiwi")
        || !has_profile
    {
        bail!("Pungi entrypoint lacks a non-commented expected setting")
    }
    Ok(())
}
