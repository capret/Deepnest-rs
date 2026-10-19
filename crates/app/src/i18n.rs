//! English source messages are catalog keys; project data and file formats stay unchanged.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt, sync::OnceLock};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[default]
    #[serde(rename = "en")]
    English,
    #[serde(rename = "zh-CN")]
    SimplifiedChinese,
}

type Catalog = BTreeMap<String, String>;

macro_rules! trf {
    ($language:expr, $source:literal $(, $key:ident = $value:expr)* $(,)?) => {
        $language.format($source, &[$((stringify!($key), $value.to_string())),*])
    };
}
pub(crate) use trf;

impl Language {
    pub const ALL: [Self; 2] = [Self::English, Self::SimplifiedChinese];

    pub fn tr<'a>(self, source: &'a str) -> &'a str {
        self.catalog().get(source).map_or(source, String::as_str)
    }

    fn catalog(self) -> &'static Catalog {
        static ENGLISH: OnceLock<Catalog> = OnceLock::new();
        static CHINESE: OnceLock<Catalog> = OnceLock::new();
        let (catalog, source) = match self {
            Self::English => (&ENGLISH, include_str!("../locales/en.json")),
            Self::SimplifiedChinese => (&CHINESE, include_str!("../locales/zh-CN.json")),
        };
        catalog
            .get_or_init(|| serde_json::from_str(source).expect("valid bundled language catalog"))
    }

    pub fn format(self, source: &str, args: &[(&str, String)]) -> String {
        // Substitute once. A file name containing {count} must remain a file name.
        let mut rest = self.tr(source);
        let mut output = String::new();
        while let Some(start) = rest.find('{') {
            output.push_str(&rest[..start]);
            rest = &rest[start..];
            let Some(end) = rest.find('}') else { break };
            let name = &rest[1..end];
            if let Some((_, value)) = args.iter().find(|(key, _)| *key == name) {
                output.push_str(value);
            } else {
                output.push_str(&rest[..=end]);
            }
            rest = &rest[end + 1..];
        }
        output.push_str(rest);
        output
    }

    pub fn label<T>(self, value: T) -> Label<T> {
        Label {
            value,
            language: self,
        }
    }

    pub fn labels<T: Copy>(self, values: &[T]) -> Vec<Label<T>> {
        values.iter().map(|value| self.label(*value)).collect()
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::English => "English",
            Self::SimplifiedChinese => "简体中文",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Label<T> {
    pub value: T,
    language: Language,
}
impl<T: fmt::Display> fmt::Display for Label<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.language.tr(&self.value.to_string()))
    }
}

/// Keep notices as source + arguments so an existing notice also switches language.
#[derive(Debug, Clone)]
pub struct Text {
    source: String,
    args: Vec<(&'static str, String)>,
}
impl Text {
    pub fn with(source: &str, args: &[(&'static str, String)]) -> Self {
        Self {
            source: source.into(),
            args: args.to_vec(),
        }
    }
    pub fn render(&self, language: Language) -> String {
        language.format(&self.source, &self.args)
    }
}
impl From<&str> for Text {
    fn from(source: &str) -> Self {
        Self::with(source, &[])
    }
}
impl From<String> for Text {
    fn from(source: String) -> Self {
        Self {
            source,
            args: vec![],
        }
    }
}

// Non-UI consumers (the benchmark and geometry importers) keep English diagnostics.
impl fmt::Display for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render(Language::English))
    }
}
impl From<Text> for String {
    fn from(value: Text) -> Self {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn placeholders(value: &str) -> BTreeSet<&str> {
        value
            .split('{')
            .skip(1)
            .map(|s| s.split('}').next().unwrap())
            .collect()
    }

    #[test]
    fn every_language_has_complete_messages_and_matching_placeholders() {
        let english = Language::English.catalog();
        assert!(english.len() > 200);
        for language in Language::ALL {
            let catalog = language.catalog();
            assert_eq!(
                catalog.keys().collect::<Vec<_>>(),
                english.keys().collect::<Vec<_>>()
            );
            for (key, value) in catalog {
                assert!(!value.trim().is_empty(), "empty {language}: {key}");
                assert_eq!(placeholders(key), placeholders(value), "{language}: {key}");
            }
        }
    }

    #[test]
    fn notices_translate_live_without_rewriting_user_values() {
        let text = Text::with(
            "Project saved to {path}",
            &[("path", "D:/中文/{count}.deepnest".into())],
        );
        assert_eq!(
            text.render(Language::English),
            "Project saved to D:/中文/{count}.deepnest"
        );
        assert_eq!(
            text.render(Language::SimplifiedChinese),
            "项目已保存到 D:/中文/{count}.deepnest"
        );
        assert_eq!(
            Language::SimplifiedChinese.tr("Unknown diagnostic"),
            "Unknown diagnostic"
        );
    }
}
