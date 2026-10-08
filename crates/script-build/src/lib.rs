//! Script compilation for CLI tools and Cargo build scripts, never the runtime.

use std::borrow::Cow;
use std::path::Path;

use anyhow::{Result, bail};
use oxc::allocator::Allocator;
use oxc::ast::ast::{JSXElement, JSXFragment};
use oxc::ast_visit::Visit;
use oxc::codegen::Codegen;
use oxc::parser::Parser;
use oxc::semantic::SemanticBuilder;
use oxc::span::SourceType;
use oxc::transformer::{JsxOptions, TransformOptions, Transformer};

/// JSX is allowed in both `.js` and `.jsx` modules. Ordinary JS stays verbatim.
/// The automatic runtime imports helpers from `deflorta/jsx-runtime`.
///
/// # Errors
///
/// Returns an error if parsing, semantic validation, or JSX transformation fails.
pub fn compile_jsx<'s>(id: &str, source: &'s str) -> Result<Cow<'s, str>> {
    if !source.contains('<') {
        return Ok(Cow::Borrowed(source));
    }
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::mjs().with_jsx(true)).parse();
    if !parsed.diagnostics.is_empty() {
        bail!("{}", diagnostics(id, source, &parsed.diagnostics));
    }
    let mut program = parsed.program;
    let mut detector = JsxDetector(false);
    detector.visit_program(&program);
    if !detector.0 {
        return Ok(Cow::Borrowed(source));
    }
    let built = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&program);
    if !built.diagnostics.is_empty() {
        bail!("{}", diagnostics(id, source, &built.diagnostics));
    }
    let options = TransformOptions {
        jsx: JsxOptions {
            import_source: Some("deflorta".into()),
            ..JsxOptions::enable()
        },
        ..TransformOptions::default()
    };
    let transformed = Transformer::new(&allocator, Path::new(id), &options)
        .build_with_scoping(built.semantic.into_scoping(), &mut program);
    if !transformed.diagnostics.is_empty() {
        bail!("{}", diagnostics(id, source, &transformed.diagnostics));
    }
    Ok(Cow::Owned(Codegen::new().build(&program).code))
}

fn diagnostics(id: &str, source: &str, errors: &[oxc::diagnostics::OxcDiagnostic]) -> String {
    errors
        .iter()
        .map(|error| {
            let offset = error
                .labels
                .first()
                .map_or(0, |label| label.offset() as usize);
            let prefix = &source[..offset];
            let line = prefix.bytes().filter(|&byte| byte == b'\n').count() + 1;
            let column = prefix
                .rsplit('\n')
                .next()
                .unwrap_or_default()
                .chars()
                .count()
                + 1;
            format!("{id}:{line}:{column}: {}", error.message)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

struct JsxDetector(bool);

impl<'a> Visit<'a> for JsxDetector {
    fn visit_jsx_element(&mut self, _: &JSXElement<'a>) {
        self.0 = true;
    }

    fn visit_jsx_fragment(&mut self, _: &JSXFragment<'a>) {
        self.0 = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_runtime_handles_fragments_spreads_and_keys() {
        let output = compile_jsx("screen.jsx", "export const Screen = () => <><View {...props} key={id}><Text>Hello {name}</Text></View></>;").unwrap();
        assert!(output.contains("deflorta/jsx-runtime"));
        // A key after a spread uses the classic helper from the import source.
        assert!(output.contains("createElement"));
        assert!(!output.contains("<View"));
        assert!(output.contains("Hello "));
    }

    #[test]
    fn preserves_plain_js_and_rejects_invalid_jsx() {
        let source = "export const less = (a, b) => a < b;";
        assert!(matches!(
            compile_jsx("main.js", source).unwrap(),
            Cow::Borrowed(_)
        ));
        let error = compile_jsx("broken.jsx", "export const Screen = () => <View>;").unwrap_err();
        assert!(error.to_string().contains("broken.jsx:1:"));
    }
}
