//! Bundles a game's modules into one ES module.
//!
//! Modules are concatenated in evaluation order with their top-level scopes
//! merged ("scope hoisting"). Import bindings are replaced by the names of the
//! declarations they resolve to, top-level names that would collide are
//! renamed, `export` syntax is removed, and modules imported as namespaces get
//! frozen namespace objects with live getters. Imports of the engine's
//! built-in modules are kept, deduplicated, at the top. The result is
//! minified with oxc.

use std::collections::{HashMap, HashSet};
use std::fmt::Write;

use anyhow::{Context, Result, bail};
use oxc::allocator::Allocator;
use oxc::ast::ast::{
    AssignmentTargetPropertyIdentifier, BindingPattern, BindingProperty,
    ExportDefaultDeclarationKind, Expression, ObjectProperty, Statement,
};
use oxc::ast_visit::{Visit, walk};
use oxc::codegen::{Codegen, CodegenOptions, CommentOptions};
use oxc::minifier::{CompressOptions, Minifier, MinifierOptions};
use oxc::parser::Parser;
use oxc::semantic::SymbolId;
use oxc::span::{GetSpan, SourceType, Span};
use oxc::str::Str;

use crate::api;
use crate::graph::{Export, Graph, Source, Target};

/// ECMAScript reserved words, which generated names must avoid.
const RESERVED_WORDS: &[&str] = &[
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "implements",
    "import",
    "in",
    "instanceof",
    "interface",
    "let",
    "new",
    "null",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "yield",
    "arguments",
    "eval",
    "undefined",
];

fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$')
        && chars.all(|c| c.is_alphanumeric() || c == '_' || c == '$')
}

/// Turns arbitrary text (file names, export names) into an identifier base.
fn identifier_base(text: &str) -> String {
    let mut base: String = text
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '$' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if base.is_empty() || base.starts_with(|c: char| c.is_ascii_digit()) {
        base.insert(0, '_');
    }
    if RESERVED_WORDS.contains(&base.as_str()) {
        base.push('_');
    }
    base
}

fn module_stem(id: &str) -> &str {
    let file = id.rsplit('/').next().unwrap_or(id);
    file.split('.').next().unwrap_or(file)
}

fn quote(text: &str) -> String {
    serde_json::to_string(text).expect("strings serialize")
}

/// Final top-level names in the bundle.
struct Names {
    used: HashSet<String>,
    /// Names that must not be kept as-is: declared in a nested scope or global.
    shadowed: HashSet<String>,
    /// Every name in the program; generated names avoid all of them.
    taken: HashSet<String>,
    locals: HashMap<(usize, SymbolId), String>,
    defaults: HashMap<usize, String>,
    namespaces: HashMap<usize, String>,
    /// `(module, export name)`; `*` is the namespace import.
    externals: HashMap<(String, String), String>,
    pending_namespaces: Vec<usize>,
}

impl Names {
    fn new(graph: &Graph) -> Self {
        // Generated namespace objects refer to these globals before any game
        // declarations run. Hoisted game bindings must not capture them.
        let mut shadowed: HashSet<String> = ["Object", "Symbol"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        let mut taken = HashSet::new();
        for module in &graph.modules {
            let scoping = module.semantic.scoping();
            let root = scoping.root_scope_id();
            for symbol in scoping.symbol_ids() {
                let name = scoping.symbol_name(symbol).to_owned();
                if scoping.symbol_scope_id(symbol) != root {
                    shadowed.insert(name.clone());
                }
                taken.insert(name);
            }
            for name in scoping.root_unresolved_references().keys() {
                shadowed.insert(name.to_string());
                taken.insert(name.to_string());
            }
        }
        Self {
            used: HashSet::new(),
            shadowed,
            taken,
            locals: HashMap::new(),
            defaults: HashMap::new(),
            namespaces: HashMap::new(),
            externals: HashMap::new(),
            pending_namespaces: Vec::new(),
        }
    }

    /// Keeps `base` when it is free, otherwise derives an unused `base$n`.
    fn pick(&mut self, base: &str) -> String {
        let base = identifier_base(base);
        if !self.used.contains(&base) && !self.shadowed.contains(&base) {
            self.used.insert(base.clone());
            return base;
        }
        let name = (1..=self.used.len() + self.taken.len() + 1)
            .map(|n| format!("{base}${n}"))
            .find(|name| !self.used.contains(name) && !self.taken.contains(name))
            .expect("an unused name exists");
        self.used.insert(name.clone());
        name
    }

    fn name_of(&mut self, graph: &Graph, target: &Target) -> String {
        match target {
            Target::Local(m, symbol) => self.locals[&(*m, *symbol)].clone(),
            Target::Default(m) => self.defaults[m].clone(),
            Target::Namespace(m) => {
                if let Some(name) = self.namespaces.get(m) {
                    return name.clone();
                }
                let name = self.pick(&format!("{}_ns", module_stem(graph.module_id(*m))));
                self.namespaces.insert(*m, name.clone());
                self.pending_namespaces.push(*m);
                name
            }
            Target::External(module, export) => self.external(module, export, export),
            Target::ExternalNamespace(module) => {
                self.external(module, "*", &format!("{}_ns", module.replace('/', "_")))
            }
        }
    }

    fn external(&mut self, module: &str, export: &str, base: &str) -> String {
        let key = (module.to_owned(), export.to_owned());
        if let Some(name) = self.externals.get(&key) {
            return name.clone();
        }
        let name = self.pick(base);
        self.externals.insert(key, name.clone());
        name
    }
}

/// A text replacement in a module's source.
struct Edit {
    start: u32,
    end: u32,
    text: String,
    /// Removes a whole statement; edits inside it are dropped.
    removes: bool,
}

impl Edit {
    fn replace(span: Span, text: impl Into<String>) -> Self {
        Self {
            start: span.start,
            end: span.end,
            text: text.into(),
            removes: false,
        }
    }

    fn range(start: u32, end: u32, text: impl Into<String>) -> Self {
        Self::replace(Span::new(start, end), text)
    }

    const fn remove(span: Span) -> Self {
        Self {
            start: span.start,
            end: span.end,
            text: String::new(),
            removes: true,
        }
    }
}

/// Identifiers written as shorthand properties (`{ a }`), which must become
/// `{ a: renamed }` when renamed.
#[derive(Default)]
struct Shorthands(HashSet<Span>);

impl<'a> Visit<'a> for Shorthands {
    fn visit_object_property(&mut self, it: &ObjectProperty<'a>) {
        if it.shorthand {
            self.0.insert(it.value.span());
        }
        walk::walk_object_property(self, it);
    }

    fn visit_binding_property(&mut self, it: &BindingProperty<'a>) {
        if it.shorthand {
            let ident = match &it.value {
                BindingPattern::BindingIdentifier(id) => Some(id.span),
                BindingPattern::AssignmentPattern(pattern) => match &pattern.left {
                    BindingPattern::BindingIdentifier(id) => Some(id.span),
                    _ => None,
                },
                _ => None,
            };
            self.0.extend(ident);
        }
        walk::walk_binding_property(self, it);
    }

    fn visit_assignment_target_property_identifier(
        &mut self,
        it: &AssignmentTargetPropertyIdentifier<'a>,
    ) {
        self.0.insert(it.binding.span);
        walk::walk_assignment_target_property_identifier(self, it);
    }
}

/// Bundles the linked graph into a single module; minifies when asked.
pub fn bundle(
    graph: &Graph,
    builtins: &HashMap<String, HashSet<String>>,
    minify: bool,
) -> Result<String> {
    if graph.report.has_errors() {
        bail!("the game has errors; run `deflorta check` for details");
    }
    if !api::analyze(graph).dynamic_imports.is_empty() {
        bail!("dynamic import() is not supported; use a static import");
    }
    let mut names = Names::new(graph);

    // Top-level declarations keep their names where possible, in evaluation order.
    for &m in &graph.order {
        let module = &graph.modules[m];
        let scoping = module.semantic.scoping();
        let mut bindings: Vec<(SymbolId, String)> = scoping
            .get_bindings(scoping.root_scope_id())
            .iter()
            .map(|(name, symbol)| (*symbol, name.to_string()))
            .filter(|(symbol, _)| !module.imports.contains_key(symbol))
            .collect();
        bindings.sort_by_key(|(symbol, _)| scoping.symbol_span(*symbol).start);
        for (symbol, name) in bindings {
            let name = names.pick(&name);
            names.locals.insert((m, symbol), name);
        }
        if matches!(module.exports.get("default"), Some(Export::Default)) {
            let name = names.pick(&format!("{}_default", module_stem(&module.id)));
            names.defaults.insert(m, name);
        }
    }

    let mut body = String::new();
    for &m in &graph.order {
        let text = emit_module(graph, m, &mut names)?;
        // A module may omit its final semicolon. Keep a leading `(` or `[` in
        // the next module from continuing the preceding expression.
        let _ = writeln!(body, "// {}\n{}\n;", graph.modules[m].id, text.trim_end());
    }

    let mut namespaces = String::new();
    let mut emitted = HashSet::new();
    while let Some(m) = names.pending_namespaces.pop() {
        if !emitted.insert(m) {
            continue;
        }
        let mut getters = String::new();
        for export in graph.export_names(m, builtins, &mut HashSet::new()) {
            let Ok(target) = graph.resolve_export(m, &export, builtins, &mut HashSet::new()) else {
                continue;
            };
            let value = names.name_of(graph, &target);
            let key = if is_identifier(&export) {
                export
            } else {
                quote(&export)
            };
            let _ = write!(getters, " get {key}() {{ return {value}; }},");
        }
        let _ = writeln!(
            namespaces,
            "const {} = Object.freeze({{ __proto__: null, [Symbol.toStringTag]: \"Module\",{getters} }});",
            names.namespaces[&m]
        );
    }

    let header = external_imports(graph, &names);
    let code = format!("{header}{namespaces}{body}");
    if minify {
        minify_module(&code)
    } else {
        validate(&code)?;
        Ok(code)
    }
}

/// Import statements for built-in modules, in first-use order.
fn external_imports(graph: &Graph, names: &Names) -> String {
    let mut modules: Vec<&str> = Vec::new();
    for &m in &graph.order {
        for (_, _, source) in &graph.modules[m].requests {
            if let Source::External(e) = source
                && !modules.contains(&e.as_str())
            {
                modules.push(e);
            }
        }
    }
    let mut out = String::new();
    for module in modules {
        let mut named_module: Vec<(&str, &str)> = names
            .externals
            .iter()
            .filter(|((m, export), _)| m == module && export != "*")
            .map(|((_, export), local)| (export.as_str(), local.as_str()))
            .collect();
        named_module.sort_unstable();
        let namespace = names.externals.get(&(module.to_owned(), "*".to_owned()));
        let specifier = quote(module);
        if !named_module.is_empty() {
            let list: Vec<String> = named_module
                .iter()
                .map(|(export, local)| {
                    let export = if is_identifier(export) {
                        (*export).to_owned()
                    } else {
                        quote(export)
                    };
                    if export == *local {
                        export
                    } else {
                        format!("{export} as {local}")
                    }
                })
                .collect();
            let _ = writeln!(out, "import {{ {} }} from {specifier};", list.join(", "));
        }
        if let Some(namespace) = namespace {
            let _ = writeln!(out, "import * as {namespace} from {specifier};");
        }
        if named_module.is_empty() && namespace.is_none() {
            let _ = writeln!(out, "import {specifier};");
        }
    }
    out
}

fn emit_module(graph: &Graph, m: usize, names: &mut Names) -> Result<String> {
    let module = &graph.modules[m];
    let program = module.program;
    let mut edits = Vec::new();

    if let Some(hashbang) = &program.hashbang {
        edits.push(Edit::remove(hashbang.span));
    }
    for statement in &program.body {
        match statement {
            Statement::ImportDeclaration(_)
            | Statement::ExportAllDeclaration(_)
            | Statement::ExportNamedDeclaration(_)
            | Statement::ExportFromDeclaration(_) => {
                edits.push(Edit::remove(statement.span()));
            }
            Statement::ExportDeclaration(decl) => {
                edits.push(Edit::range(
                    decl.span.start,
                    decl.declaration.span().start,
                    "",
                ));
            }
            Statement::ExportDefaultDeclaration(decl) => {
                let start = decl.span.start;
                match &decl.declaration {
                    ExportDefaultDeclarationKind::FunctionDeclaration(f) => {
                        edits.push(Edit::range(start, f.span.start, ""));
                        if f.id.is_none() {
                            let name = &names.defaults[&m];
                            edits.push(Edit::range(
                                f.params.span.start,
                                f.params.span.start,
                                format!(" {name}"),
                            ));
                        }
                    }
                    ExportDefaultDeclarationKind::ClassDeclaration(c) if c.id.is_some() => {
                        edits.push(Edit::range(start, c.span.start, ""));
                    }
                    kind => {
                        let span = kind.span();
                        let name = &names.defaults[&m];
                        edits.push(Edit::range(start, span.start, format!("const {name} = ")));
                        edits.push(Edit::range(span.end, decl.span.end, ";"));
                    }
                }
            }
            _ => {}
        }
    }

    let mut shorthands = Shorthands::default();
    shorthands.visit_program(program);
    let scoping = module.semantic.scoping();
    let rename = |edits: &mut Vec<Edit>, span: Span, from: &str, to: &str| {
        let text = if shorthands.0.contains(&span) {
            format!("{from}: {to}")
        } else {
            to.to_owned()
        };
        edits.push(Edit::replace(span, text));
    };
    let reference_spans = |symbol: SymbolId| -> Vec<Span> {
        scoping
            .get_resolved_reference_ids(symbol)
            .iter()
            .map(|&id| module.semantic.reference_span(scoping.get_reference(id)))
            .collect()
    };

    for (name, &symbol) in scoping.get_bindings(scoping.root_scope_id()) {
        if module.imports.contains_key(&symbol) {
            continue;
        }
        let to = &names.locals[&(m, symbol)];
        if to == name.as_str() {
            continue;
        }
        let mut spans = vec![scoping.symbol_span(symbol)];
        spans.extend(scoping.symbol_redeclarations(symbol).iter().map(|r| r.span));
        spans.extend(reference_spans(symbol));
        for span in spans {
            rename(&mut edits, span, name.as_str(), to);
        }
    }
    let mut imports: Vec<(&SymbolId, &Target)> = module.links.iter().collect();
    imports.sort_by_key(|(symbol, _)| scoping.symbol_span(**symbol).start);
    for (&symbol, target) in imports {
        let to = names.name_of(graph, target);
        let from = scoping.symbol_name(symbol);
        for span in reference_spans(symbol) {
            rename(&mut edits, span, from, &to);
        }
    }

    apply_edits(module.source, edits).with_context(|| format!("bundling '{}'", module.id))
}

fn apply_edits(source: &str, mut edits: Vec<Edit>) -> Result<String> {
    let removed: Vec<(u32, u32)> = edits
        .iter()
        .filter(|e| e.removes)
        .map(|e| (e.start, e.end))
        .collect();
    edits.retain(|e| {
        e.removes
            || !removed
                .iter()
                .any(|&(start, end)| start <= e.start && e.end <= end)
    });
    edits.sort_by_key(|e| (e.start, e.end));
    let mut out = String::with_capacity(source.len());
    let mut position = 0;
    for edit in edits {
        let (start, end) = (edit.start as usize, edit.end as usize);
        if start < position {
            bail!("internal error: overlapping edits at byte {start}");
        }
        out.push_str(&source[position..start]);
        out.push_str(&edit.text);
        position = end;
    }
    out.push_str(&source[position..]);
    Ok(out)
}

fn parse_errors(errors: &[oxc::diagnostics::OxcDiagnostic], code: &str) -> anyhow::Error {
    let rendered: Vec<String> = errors
        .iter()
        .map(|e| {
            e.clone()
                .render_with_source_code(oxc::diagnostics::NamedSource::new(
                    "bundle.js",
                    code.to_owned(),
                ))
        })
        .collect();
    anyhow::anyhow!(
        "internal error: the bundle is invalid\n{}",
        rendered.join("\n")
    )
}

fn validate(code: &str) -> Result<()> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, code, SourceType::mjs()).parse();
    if !parsed.diagnostics.is_empty() {
        return Err(parse_errors(&parsed.diagnostics, code));
    }
    Ok(())
}

fn minify_module(code: &str) -> Result<String> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, code, SourceType::mjs()).parse();
    if !parsed.diagnostics.is_empty() {
        return Err(parse_errors(&parsed.diagnostics, code));
    }
    let mut program = parsed.program;
    let minified = Minifier::new(MinifierOptions {
        compress: Some(CompressOptions::smallest()),
        ..MinifierOptions::default()
    })
    .minify(&allocator, &mut program);
    Ok(Codegen::new()
        .with_options(CodegenOptions {
            minify: true,
            comments: CommentOptions::disabled(),
            ..CodegenOptions::default()
        })
        .with_scoping(minified.scoping)
        .build(&program)
        .code)
}

/// Is `expression` a string without substitutions? Returns its value.
pub fn static_string<'a>(expression: &'a Expression<'a>) -> Option<&'a str> {
    match expression {
        Expression::StringLiteral(s) => Some(s.value.as_str()),
        Expression::TemplateLiteral(t) if t.expressions.is_empty() => {
            t.quasis.first()?.value.cooked.as_ref().map(Str::as_str)
        }
        _ => None,
    }
}
