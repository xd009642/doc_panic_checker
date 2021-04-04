use proc_macro2::Span;
use quote::ToTokens;
use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::PathBuf;
use syn::spanned::Spanned;
use syn::*;

#[derive(Clone)]
pub struct AstWalker {
    filename: PathBuf,
    source_code: String,
}

pub struct PanicLocation {
    ident: String,
    span: Span,
}

impl fmt::Display for PanicLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {}:{}",
            self.ident,
            self.span.start().line,
            self.span.end().line
        )
    }
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
        Ok(Self::new_with_source(filename, source_code))
    }

    fn new_with_source(filename: PathBuf, source_code: String) -> Self {
        Self {
            filename,
            source_code,
        }
    }

    pub fn process(&self) -> Vec<PanicLocation> {
        let mut result = vec![];
        if contains_panicky_words(&self.source_code) {
            if let Ok(file) = parse_file(&self.source_code) {
                self.process_items(&file.items, None, &mut result);
            }
        }
        result
    }

    fn process_items(
        &self,
        items: &[Item],
        namespace: Option<String>,
        result: &mut Vec<PanicLocation>,
    ) {
        for item in items.iter() {
            if !self.span_has_panics(item.span()) {
                continue;
            }
            match *item {
                Item::Mod(ref i) if is_public(&i.vis) => {
                    self.process_module(i, namespace.as_ref(), result)
                }
                Item::Fn(ref i) if is_public(&i.vis) => {
                    self.process_fn(i, namespace.as_ref(), result)
                }
                Item::Trait(ref i) if is_public(&i.vis) => {
                    self.process_trait(i, namespace.as_ref(), result)
                }
                Item::Impl(ref i) => self.process_impl(i, namespace.as_ref(), result),
                Item::Macro(ref _i) => {}
                Item::Macro2(ref i) if is_public(&i.vis) => {}
                _ => {}
            }
        }
    }

    fn process_module(
        &self,
        module: &ItemMod,
        namespace: Option<&String>,
        result: &mut Vec<PanicLocation>,
    ) {
        if let Some(items) = &module.content {
            let ident = if let Some(namespace) = namespace {
                format!("{}::{}", namespace, module.ident)
            } else {
                format!("{}", module.ident)
            };
            self.process_items(&items.1, Some(ident), result);
        }
    }

    fn process_fn(
        &self,
        func: &ItemFn,
        namespace: Option<&String>,
        result: &mut Vec<PanicLocation>,
    ) {
        if !self.span_has_panics(func.block.span()) {
            return;
        }
        let comment = self.find_doc_comment(func.span());
        let ident = if let Some(namespace) = namespace {
            format!("{}::{}", namespace, func.sig.ident)
        } else {
            func.sig.ident.to_string()
        };
        self.check_docs(&comment, &ident, func.span(), result);
    }

    fn check_docs(&self, comment: &str, ident: &str, span: Span, result: &mut Vec<PanicLocation>) {
        if !warns_about_panics(comment) {
            result.push(PanicLocation {
                span: span,
                ident: ident.to_string(),
            });
        }
    }

    fn process_trait(
        &self,
        item_trait: &ItemTrait,
        namespace: Option<&String>,
        result: &mut Vec<PanicLocation>,
    ) {
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

            self.check_docs(&comment, &ident, method.span(), result);
        }
    }

    fn process_impl(
        &self,
        imp: &ItemImpl,
        namespace: Option<&String>,
        result: &mut Vec<PanicLocation>,
    ) {
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

            self.check_docs(&comment, &ident, method.span(), result);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undocumented_panics() {
        let naughty_code = r#"
                /// Nothing to see here 
                pub fn foobar() {
                    panic!("mwhahahahaha");
                }
            "#
        .to_string();

        let ast_walker = AstWalker::new_with_source(PathBuf::from("bad_code.rs"), naughty_code);

        let panik = ast_walker.process();
        assert_eq!(panik.len(), 1);
        assert_eq!(panik[0].ident, "foobar");
    }
}
