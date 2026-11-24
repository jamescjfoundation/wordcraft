//! Extension data: namespaced values an application attaches to a document, a paragraph or a
//! run of text, which the model carries, copies, compares, undoes and saves without understanding
//! them (like custom XML in Word).
//!
//! Keys are namespaced by the application or plug-in that owns them (`"script.scene_number"`,
//! `"manuscript.status"`), so two owners never collide. Values are JSON, shared behind an [`Arc`]
//! so copying a document for an undo step does not copy them.
//!
//! On a run of text, extension data is a *mark* like a hyperlink, not formatting: Clear
//! Formatting keeps it ([`crate::props::CharProps::cleared`]) and
//! [`crate::props::CharProps::formatting`] leaves it out. On a paragraph it is never inherited
//! from a style. Formats that cannot hold it (DOCX, RTF, ...) drop it; the native JSON keeps it.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// The longest key accepted by [`Ext::set`].
pub const MAX_EXT_KEY_LEN: usize = 128;

/// A map of namespaced extension values. Empty by default, and left out of the native JSON when
/// empty, so documents without extension data read and write exactly as before.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Ext(BTreeMap<String, Arc<serde_json::Value>>);

/// Why [`Ext::set`] refused a key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExtKeyError {
    /// The key has no `namespace.` prefix, or the namespace or name is empty.
    NotNamespaced,
    /// The key is longer than [`MAX_EXT_KEY_LEN`].
    TooLong,
}

impl std::fmt::Display for ExtKeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExtKeyError::NotNamespaced => write!(f, "an extension key must be `namespace.name`"),
            ExtKeyError::TooLong => write!(f, "an extension key may be at most {MAX_EXT_KEY_LEN} bytes"),
        }
    }
}

impl std::error::Error for ExtKeyError {}

/// Whether `key` is a valid extension key: `namespace.name`, both parts non-empty, ASCII
/// letters, digits, `_`, `-` and `.` only.
pub fn valid_ext_key(key: &str) -> Result<(), ExtKeyError> {
    if key.len() > MAX_EXT_KEY_LEN {
        return Err(ExtKeyError::TooLong);
    }
    let Some((ns, name)) = key.split_once('.') else { return Err(ExtKeyError::NotNamespaced) };
    let ok = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.');
    if ok(ns) && ok(name) { Ok(()) } else { Err(ExtKeyError::NotNamespaced) }
}

impl Ext {
    pub fn new() -> Ext {
        Ext::default()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.0.get(key).map(|v| v.as_ref())
    }
    pub fn contains(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }
    /// Set `key` to `value`; a `null` value removes the key.
    pub fn set(&mut self, key: &str, value: serde_json::Value) -> Result<(), ExtKeyError> {
        valid_ext_key(key)?;
        if value.is_null() {
            self.0.remove(key);
        } else {
            self.0.insert(key.to_owned(), Arc::new(value));
        }
        Ok(())
    }
    pub fn remove(&mut self, key: &str) -> Option<serde_json::Value> {
        self.0.remove(key).map(Arc::unwrap_or_clone)
    }
    /// Remove every key in `namespace` (the part before the first `.`).
    pub fn clear_namespace(&mut self, namespace: &str) {
        self.0.retain(|k, _| k.split_once('.').map(|(ns, _)| ns) != Some(namespace));
    }
    pub fn iter(&self) -> impl Iterator<Item = (&str, &serde_json::Value)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v.as_ref()))
    }
    /// Keys of `patch` replace the same keys of `self`; other keys stay.
    pub fn overlay(&mut self, patch: &Ext) {
        for (k, v) in &patch.0 {
            self.0.insert(k.clone(), Arc::clone(v));
        }
    }
    /// Drop the keys whose value equals the same key's value in `base`.
    pub fn minus(&mut self, base: &Ext) {
        self.0.retain(|k, v| base.0.get(k) != Some(v));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn keys_must_be_namespaced() {
        let mut e = Ext::new();
        assert_eq!(e.set("plain", json!(1)), Err(ExtKeyError::NotNamespaced));
        assert_eq!(e.set(".x", json!(1)), Err(ExtKeyError::NotNamespaced));
        assert_eq!(e.set("a b.c", json!(1)), Err(ExtKeyError::NotNamespaced));
        assert_eq!(e.set(&format!("a.{}", "x".repeat(200)), json!(1)), Err(ExtKeyError::TooLong));
        assert!(e.set("script.scene_number", json!("12A")).is_ok());
        assert_eq!(e.get("script.scene_number"), Some(&json!("12A")));
    }

    #[test]
    fn null_removes_and_namespaces_clear() {
        let mut e = Ext::new();
        e.set("a.x", json!(1)).unwrap();
        e.set("a.y", json!(2)).unwrap();
        e.set("b.x", json!(3)).unwrap();
        e.set("a.x", serde_json::Value::Null).unwrap();
        assert!(!e.contains("a.x"));
        e.clear_namespace("a");
        assert_eq!(e.len(), 1);
        assert!(e.contains("b.x"));
    }

    #[test]
    fn overlay_and_minus_work_key_by_key() {
        let mut base = Ext::new();
        base.set("a.x", json!(1)).unwrap();
        base.set("a.y", json!(2)).unwrap();
        let mut patch = Ext::new();
        patch.set("a.y", json!(5)).unwrap();
        let mut all = base.clone();
        all.overlay(&patch);
        assert_eq!(all.get("a.x"), Some(&json!(1)));
        assert_eq!(all.get("a.y"), Some(&json!(5)));
        all.minus(&base);
        assert_eq!(all, patch);
    }

    #[test]
    fn serializes_as_a_plain_map() {
        let mut e = Ext::new();
        e.set("a.x", json!({"k": [1, 2]})).unwrap();
        let s = serde_json::to_string(&e).unwrap();
        assert_eq!(s, r#"{"a.x":{"k":[1,2]}}"#);
        let back: Ext = serde_json::from_str(&s).unwrap();
        assert_eq!(back, e);
    }
}
