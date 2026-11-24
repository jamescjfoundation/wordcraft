//! Extension data ([`wordcraft_doc::ext`]) on documents, paragraphs and runs: it counts as
//! content, survives the native JSON, and is a mark (not formatting) on runs.

use serde_json::json;
use wordcraft_doc::props::{CharProps, ParaProps};
use wordcraft_doc::{Document, Ext};

fn ext(key: &str, value: serde_json::Value) -> Ext {
    let mut e = Ext::new();
    e.set(key, value).unwrap();
    e
}

#[test]
fn document_ext_counts_as_content_and_round_trips() {
    let a = Document::new();
    let mut b = a.clone();
    assert!(a.same_content(&b));
    b.ext = ext("app.version", json!({"v": 2}));
    assert!(!a.same_content(&b), "a change to extension data is a change to the document");
    let text = serde_json::to_string(&b).unwrap();
    let back: Document = serde_json::from_str(&text).unwrap();
    assert_eq!(back.ext.get("app.version"), Some(&json!({"v": 2})));
}

#[test]
fn empty_ext_is_left_out_of_the_json() {
    let text = serde_json::to_string(&Document::new()).unwrap();
    assert!(!text.contains("\"ext\""), "documents without extension data are written as before");
    let p = serde_json::to_string(&ParaProps::default()).unwrap();
    assert_eq!(p, "{}");
}

#[test]
fn para_ext_round_trips_and_is_not_formatting() {
    let p = ParaProps { ext: ext("script.scene_number", json!("12A")), ..Default::default() };
    let back: ParaProps = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
    assert_eq!(back, p);
    assert!(p.formatting().ext.is_empty());
    let mut styled = ParaProps::default();
    styled.overlay(&p);
    assert!(styled.ext.is_empty(), "paragraph extension data is never inherited");
}

#[test]
fn run_ext_is_a_mark() {
    let c = CharProps { bold: Some(true), ext: ext("script.note", json!(4)), ..Default::default() };
    assert_eq!(c.cleared().ext, c.ext, "Clear Formatting keeps marks");
    assert!(c.cleared().bold.is_none());
    assert!(c.formatting().ext.is_empty());
    let base = CharProps { ext: ext("script.note", json!(4)), ..Default::default() };
    assert!(c.clone().minus(&base).ext.is_empty());
    let mut over = CharProps::default();
    over.overlay(&c);
    assert_eq!(over.ext, c.ext);
}
