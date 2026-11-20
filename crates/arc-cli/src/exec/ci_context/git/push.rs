use super::load;
use std::path::Path;

pub(in crate::exec::ci_context) fn push(words: &[&str]) -> Option<(String, Vec<String>)> {
    let mut cwd = std::env::current_dir().ok()?;
    let mut configs = Vec::new();
    let mut index = 1;
    while let Some(&word) = words.get(index) {
        match word {
            "push" => {
                index += 1;
                break;
            }
            "-C" => {
                index += 1;
                let path = Path::new(*words.get(index)?);
                cwd = if path.is_absolute() {
                    path.to_owned()
                } else {
                    cwd.join(path)
                };
            }
            "-c" => {
                index += 1;
                let (key, value) = words.get(index)?.split_once('=')?;
                configs.push((key, value));
            }
            _ => return None,
        }
        index += 1;
    }
    if words.get(index.wrapping_sub(1)) != Some(&"push") {
        return None;
    }
    let mut selected = None;
    for &word in &words[index..] {
        if word.starts_with('-') {
            if !matches!(
                word,
                "-u" | "--set-upstream" | "--force-with-lease" | "--tags"
            ) {
                return None;
            }
        } else if word.is_empty() {
            return None;
        } else if selected.is_none() {
            selected = Some(word);
        }
    }
    let (cfg, branch) = load(&cwd, &configs)?;
    let selector = selected.unwrap_or("@bare-push");
    let destination = match selected {
        Some(value) => value.to_owned(),
        None => cfg.default_remote(branch.as_deref())?,
    };
    Some((selector.to_owned(), cfg.endpoints(&destination)?))
}
