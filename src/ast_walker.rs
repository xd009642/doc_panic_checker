use proc_macro2::Span;
use quote::ToTokens;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use syn::spanned::Spanned;
use syn::*;
use tracing::warn;

pub struct AstWalker {
    filename: PathBuf,
    source_code: String,
}

fn contains_panicky_words(source_code: &str) -> bool {
    let panicky_words = &["panic", "unwrap", "expect", "todo", "unimplemented"];
    source_code
        .lines()
        .map(|x| x.trim_start())
        .filter(|trimmed| !trimmed.starts_with("///") || !trimmed.starts_with("//"))
        .any(|x| panicky_words.iter().any(|panik| x.contains(panik)))
}

fn warns_about_panics(comment: &str) -> bool {
    !comment.is_empty() && comment.contains("panic")
}

impl AstWalker {
    pub fn new(filename: PathBuf) -> io::Result<Self> {
        let mut file = File::open(&filename)?;
        let mut source_code = String::new();
        file.read_to_string(&mut source_code)?;
        Ok(Self {
            filename,
            source_code,
        })
    }

    pub fn process(&self) {
        if contains_panicky_words(&self.source_code) {
            if let Ok(file) = parse_file(&self.source_code) {
                self.process_items(&file.items, None);
            }
        }
    }

    fn process_items(&self, items: &[Item], namespace: Option<String>) {
        for item in items.iter() {
            if !self.span_has_panics(item.span()) {
                continue;
            }
            match *item {
                Item::Mod(ref i) if is_public(&i.vis) => self.process_module(i),
                Item::Fn(ref i) if is_public(&i.vis) => self.process_fn(i, namespace.as_ref()),
                Item::Trait(ref i) if is_public(&i.vis) => {
                    self.process_trait(i, namespace.as_ref())
                }
                Item::Impl(ref i) => self.process_impl(i, namespace.as_ref()),
                Item::Macro(ref _i) => {}
                Item::Macro2(ref i) if is_public(&i.vis) => {}
                _ => {}
            }
        }
    }

    fn process_module(&self, module: &ItemMod) {
        if let Some(items) = &module.content {
            self.process_items(&items.1, Some(module.ident.to_string()));
        }
    }

    fn process_fn(&self, func: &ItemFn, namespace: Option<&String>) {
        if !self.span_has_panics(func.block.span()) {
            return;
        }
        let comment = self.find_doc_comment(func.span());
        let ident = if let Some(namespace) = namespace {
            format!("{}::{}", namespace, func.sig.ident)
        } else {
            func.sig.ident.to_string()
        };
        self.check_docs(&comment, &ident, func.span());
    }

    fn check_docs(&self, comment: &str, ident: &str, span: Span) {
        if !warns_about_panics(comment) {
            warn!("In span: {}:{}", span.start().line, span.end().line);
            warn!(
                "Function {} in {} potentially has an undocumented panic",
                ident,
                self.filename.display()
            );
        }
    }

    fn process_trait(&self, item_trait: &ItemTrait, namespace: Option<&String>) {
        for default_method in item_trait
            .items
            .iter()
            .filter(|x| matches!(x, TraitItem::Method(m) if m.default.is_some()))
        {
            let method = if let TraitItem::Method(ref m) = default_method {
                m
            } else {
                unreachable!()
            };
            if !self.span_has_panics(method.default.as_ref().unwrap().span()) {
                continue;
            }
            let comment = self.find_doc_comment(method.span());
            let ident = if let Some(namespace) = namespace {
                format!("{}::{}::{}", namespace, item_trait.ident, method.sig.ident)
            } else {
                format!("{}::{}", item_trait.ident, method.sig.ident)
            };

            self.check_docs(&comment, &ident, method.span());
        }
    }

    fn process_impl(&self, imp: &ItemImpl, namespace: Option<&String>) {
        for method in imp
            .items
            .iter()
            .filter(|x| matches!(x, ImplItem::Method(_)))
        {
            let method = if let ImplItem::Method(m) = method {
                m
            } else {
                unreachable!()
            };
            if !self.span_has_panics(method.block.span()) {
                continue;
            }
            let comment = self.find_doc_comment(method.span());
            let self_ty = imp.self_ty.to_token_stream().to_string();
            let ident = if let Some(namespace) = namespace {
                format!("{}::{}::{}", namespace, self_ty, method.sig.ident)
            } else {
                format!("{}::{}", self_ty, method.sig.ident)
            };

            self.check_docs(&comment, &ident, method.span());
        }
    }

    fn find_doc_comment(&self, span: Span) -> String {
        let start = span.start().line - 1;
        let end = span.end().line - 1;
        let lines = self.source_code.lines().collect::<Vec<&str>>();

        let mut doc_comment = vec![];
        for i in start..end {
            let trimmed = lines[i].trim();
            if trimmed.starts_with("///") {
                doc_comment.push(trimmed);
            } else {
                break;
            }
        }
        doc_comment.join("\n").to_lowercase()
    }

    fn span_has_panics(&self, span: Span) -> bool {
        let start = span.start().line - 1;
        let end = (span.end().line - 1) - start;
        self.source_code
            .lines()
            .skip(start)
            .take(end)
            .any(contains_panicky_words)
    }
}

fn is_public(visibility: &Visibility) -> bool {
    matches!(visibility, &Visibility::Public(_))
}
