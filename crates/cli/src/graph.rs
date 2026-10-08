use std::collections::{BTreeSet, HashMap, HashSet};

use anyhow::{Context, Result, bail};
use oxc::allocator::Allocator;
use oxc::ast::ast::{
    Declaration, ExportDefaultDeclarationKind, IdentifierReference, ImportDeclarationSpecifier,
    ModuleExportName, Program, Statement,
};
use oxc::parser::Parser;
use oxc::semantic::{Semantic, SemanticBuilder, SymbolId};
use oxc::span::{GetSpan, SourceType, Span};

use crate::report::Report;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ImportName {
    Name(String),
    Namespace,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    Module(usize),
    External(String),
    /// Could not be resolved or loaded (already reported).
    Missing,
}

#[derive(Debug)]
pub struct Import {
    pub request: usize,
    pub name: ImportName,
    pub span: Span,
}

#[derive(Debug)]
pub enum Export {
    Local(SymbolId),
    /// `export default <expression>` and anonymous default functions/classes.
    Default,
    Reexport {
        request: usize,
        name: ImportName,
        span: Span,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    Local(usize, SymbolId),
    Default(usize),
    Namespace(usize),
    External(String, String),
    ExternalNamespace(String),
}

pub struct Module<'a> {
    pub id: String,
    pub source: &'a str,
    pub program: &'a Program<'a>,
    pub semantic: Semantic<'a>,
    /// Parse or semantic errors were reported; the module is not analyzed.
    pub broken: bool,
    /// Import/export requests: specifier, span and resolution, in source order.
    pub requests: Vec<(String, Span, Source)>,
    pub imports: HashMap<SymbolId, Import>,
    pub exports: HashMap<String, Export>,
    pub stars: Vec<usize>,
    pub links: HashMap<SymbolId, Target>,
}

impl Module<'_> {
    pub fn symbol_of(&self, ident: &IdentifierReference) -> Option<SymbolId> {
        let reference = ident.reference_id.get()?;
        self.semantic.scoping().get_reference(reference).symbol_id()
    }

    pub fn is_top_level(&self, symbol: SymbolId) -> bool {
        let scoping = self.semantic.scoping();
        scoping.symbol_scope_id(symbol) == scoping.root_scope_id()
    }
}

pub trait Loader {
    fn load(&self, id: &str) -> Result<String>;
}

impl Loader for deflorta_assets::GameFiles {
    fn load(&self, id: &str) -> Result<String> {
        Ok(self.read_to_string(id)?)
    }
}

pub struct Builtins;

impl Loader for Builtins {
    fn load(&self, id: &str) -> Result<String> {
        let file = deflorta_assets::BUILTIN_MODULES
            .iter()
            .find(|(name, _)| *name == id)
            .map(|(_, file)| *file)
            .ok_or_else(|| anyhow::anyhow!("unknown built-in module '{id}'"))?;
        let path = crate::distribution::template()?.join("runtime").join(file);
        std::fs::read_to_string(&path).with_context(|| format!("cannot read {}", path.display()))
    }
}

pub struct Graph<'a> {
    allocator: &'a Allocator,
    pub modules: Vec<Module<'a>>,
    /// Module indices in evaluation order (dependencies first).
    pub order: Vec<usize>,
    index: HashMap<String, usize>,
    pub report: Report,
    /// Built-in export names used when linking namespace re-exports.
    pub builtins: HashMap<String, HashSet<String>>,
}

impl<'a> Graph<'a> {
    /// Loads `entries` and everything they import. Built-in specifiers are
    /// external unless they are themselves entries.
    pub fn build(allocator: &'a Allocator, loader: &dyn Loader, entries: &[&str]) -> Self {
        let mut graph = Self {
            allocator,
            modules: Vec::new(),
            order: Vec::new(),
            index: HashMap::new(),
            report: Report::default(),
            builtins: HashMap::new(),
        };
        for entry in entries {
            if let Err(err) = graph.load(loader, entry) {
                graph
                    .report
                    .error(format!("cannot load '{entry}': {err:#}"));
            }
        }
        graph
    }

    pub fn module_id(&self, index: usize) -> &str {
        &self.modules[index].id
    }

    fn load(&mut self, loader: &dyn Loader, id: &str) -> Result<usize> {
        if let Some(&index) = self.index.get(id) {
            return Ok(index);
        }
        let source_text = loader.load(id)?;
        let source = self
            .allocator
            .alloc_str(&deflorta_script_build::compile_jsx(id, &source_text)?);
        let index = self.modules.len();
        self.index.insert(id.to_owned(), index);
        self.parse(id, source);
        let specifiers: Vec<(String, Span)> = self.modules[index]
            .requests
            .iter()
            .map(|(specifier, span, _)| (specifier.clone(), *span))
            .collect();
        for (k, (specifier, span)) in specifiers.into_iter().enumerate() {
            let source = match deflorta_assets::resolve_specifier(id, &specifier) {
                Ok(resolved) if deflorta_assets::is_builtin_module(&resolved) => {
                    Source::External(resolved)
                }
                Ok(resolved) => match self.load(loader, &resolved) {
                    Ok(module) => Source::Module(module),
                    Err(err) => {
                        self.report.error_at(
                            id,
                            span,
                            format!("cannot load '{specifier}': {err:#}"),
                        );
                        Source::Missing
                    }
                },
                Err(err) => {
                    self.report.error_at(id, span, format!("{err:#}"));
                    Source::Missing
                }
            };
            self.modules[index].requests[k].2 = source;
        }
        self.order.push(index);
        Ok(index)
    }

    fn parse(&mut self, id: &str, source: &'a str) {
        let parsed = Parser::new(self.allocator, source, SourceType::mjs()).parse();
        let mut broken = !parsed.diagnostics.is_empty();
        for error in parsed.diagnostics {
            self.push_oxc(id, &error);
        }
        let program: &'a Program<'a> = self.allocator.alloc(parsed.program);
        let built = SemanticBuilder::new()
            .with_check_syntax_error(true)
            .with_build_nodes(true)
            .build(program);
        if !broken {
            broken = !built.diagnostics.is_empty();
            for error in built.diagnostics {
                self.push_oxc(id, &error);
            }
        }
        let mut module = Module {
            id: id.to_owned(),
            source,
            program,
            semantic: built.semantic,
            broken,
            requests: Vec::new(),
            imports: HashMap::new(),
            exports: HashMap::new(),
            stars: Vec::new(),
            links: HashMap::new(),
        };
        collect_module_syntax(&mut module, &mut self.report);
        self.modules.push(module);
    }

    fn push_oxc(&mut self, id: &str, error: &oxc::diagnostics::OxcDiagnostic) {
        let span = error.labels.first().map_or_else(Span::default, |label| {
            Span::sized(label.offset(), label.len())
        });
        let help = error.help.as_ref().map(ToString::to_string);
        self.report.push(
            crate::report::Severity::Error,
            error.message.to_string(),
            Some((id, span)),
            help,
        );
    }

    pub fn link(&mut self, builtins: &HashMap<String, HashSet<String>>) {
        self.builtins.clone_from(builtins);
        let mut errors = Vec::new();
        for m in 0..self.modules.len() {
            let mut links = HashMap::new();
            for (&symbol, import) in &self.modules[m].imports {
                let module = &self.modules[m];
                let scoping = module.semantic.scoping();
                for &reference in scoping.get_resolved_reference_ids(symbol) {
                    let reference = scoping.get_reference(reference);
                    if reference.is_write() {
                        errors.push((
                            m,
                            module.semantic.reference_span(reference),
                            "imported bindings are read-only".to_owned(),
                        ));
                    }
                }
                match self.resolve_import(m, symbol, builtins) {
                    Ok(Some(target)) => {
                        links.insert(symbol, target);
                    }
                    Ok(None) => {}
                    Err(message) => errors.push((m, import.span, message)),
                }
            }
            for export in self.modules[m].exports.values() {
                if let Export::Reexport { span, .. } = export
                    && let Err(message) =
                        self.resolve_reexport(m, export, builtins, &mut HashSet::new())
                {
                    errors.push((m, *span, message));
                }
            }
            self.modules[m].links = links;
        }
        for (m, span, message) in errors {
            let id = self.modules[m].id.clone();
            self.report.error_at(&id, span, message);
        }
    }

    fn resolve_import(
        &self,
        m: usize,
        symbol: SymbolId,
        builtins: &HashMap<String, HashSet<String>>,
    ) -> Result<Option<Target>, String> {
        let import = &self.modules[m].imports[&symbol];
        self.resolve_request(
            m,
            import.request,
            &import.name,
            builtins,
            &mut HashSet::new(),
        )
    }

    fn resolve_request(
        &self,
        m: usize,
        request: usize,
        name: &ImportName,
        builtins: &HashMap<String, HashSet<String>>,
        visited: &mut HashSet<(usize, String)>,
    ) -> Result<Option<Target>, String> {
        match (&self.modules[m].requests[request].2, name) {
            (Source::Missing, _) => Ok(None),
            // A module that failed to parse has unreliable exports (already reported).
            (Source::Module(k), _) if self.modules[*k].broken => Ok(None),
            (Source::Module(k), ImportName::Namespace) => Ok(Some(Target::Namespace(*k))),
            (Source::Module(k), ImportName::Name(name)) => {
                self.resolve_export(*k, name, builtins, visited).map(Some)
            }
            (Source::External(e), ImportName::Namespace) => {
                Ok(Some(Target::ExternalNamespace(e.clone())))
            }
            (Source::External(e), ImportName::Name(name)) => {
                if builtins.get(e).is_some_and(|names| names.contains(name)) {
                    Ok(Some(Target::External(e.clone(), name.clone())))
                } else {
                    Err(format!("'{e}' has no export named '{name}'"))
                }
            }
        }
    }

    fn resolve_reexport(
        &self,
        m: usize,
        export: &Export,
        builtins: &HashMap<String, HashSet<String>>,
        visited: &mut HashSet<(usize, String)>,
    ) -> Result<Option<Target>, String> {
        match export {
            Export::Reexport { request, name, .. } => {
                self.resolve_request(m, *request, name, builtins, visited)
            }
            _ => Ok(None),
        }
    }

    pub fn resolve_export(
        &self,
        m: usize,
        name: &str,
        builtins: &HashMap<String, HashSet<String>>,
        visited: &mut HashSet<(usize, String)>,
    ) -> Result<Target, String> {
        let module = &self.modules[m];
        let missing = || format!("'{}' has no export named '{name}'", module.id);
        if !visited.insert((m, name.to_owned())) {
            return Err(format!("circular re-export of '{name}' in '{}'", module.id));
        }
        if let Some(export) = module.exports.get(name) {
            return match export {
                Export::Local(symbol) if module.imports.contains_key(symbol) => {
                    let import = &module.imports[symbol];
                    self.resolve_request(m, import.request, &import.name, builtins, visited)?
                        .ok_or_else(missing)
                }
                Export::Local(symbol) => Ok(Target::Local(m, *symbol)),
                Export::Default => Ok(Target::Default(m)),
                Export::Reexport { .. } => self
                    .resolve_reexport(m, export, builtins, visited)?
                    .ok_or_else(missing),
            };
        }
        if name == "default" {
            return Err(missing());
        }
        let mut found: Option<Target> = None;
        for &request in &module.stars {
            let target = match &module.requests[request].2 {
                Source::Module(k) => self
                    .resolve_export(*k, name, builtins, &mut visited.clone())
                    .ok(),
                Source::External(e) => builtins
                    .get(e)
                    .is_some_and(|names| names.contains(name))
                    .then(|| Target::External(e.clone(), name.to_owned())),
                Source::Missing => None,
            };
            match (&found, target) {
                (Some(existing), Some(target)) if *existing != target => {
                    return Err(format!(
                        "'{name}' is exported by more than one `export *` in '{}'",
                        module.id
                    ));
                }
                (None, Some(target)) => found = Some(target),
                _ => {}
            }
        }
        found.ok_or_else(missing)
    }

    /// Every name module `m` exports (for namespace objects), except ambiguous ones.
    pub fn export_names(
        &self,
        m: usize,
        builtins: &HashMap<String, HashSet<String>>,
        visited: &mut HashSet<usize>,
    ) -> BTreeSet<String> {
        let module = &self.modules[m];
        let mut names: BTreeSet<String> = module.exports.keys().cloned().collect();
        if !visited.insert(m) {
            return names;
        }
        for &request in &module.stars {
            let star: BTreeSet<String> = match &module.requests[request].2 {
                Source::Module(k) => self.export_names(*k, builtins, visited),
                Source::External(e) => builtins
                    .get(e)
                    .map_or_default(|n| n.iter().cloned().collect()),
                Source::Missing => BTreeSet::new(),
            };
            names.extend(star.into_iter().filter(|name| name != "default"));
        }
        names.retain(|name| {
            self.resolve_export(m, name, builtins, &mut HashSet::new())
                .is_ok()
        });
        names
    }
}

fn export_name(name: &ModuleExportName) -> String {
    name.name().to_string()
}

fn request(module: &mut Module, specifier: &str, span: Span) -> usize {
    module
        .requests
        .push((specifier.to_owned(), span, Source::Missing));
    module.requests.len() - 1
}

fn collect_module_syntax(module: &mut Module, report: &mut Report) {
    let program = module.program;
    for statement in &program.body {
        match statement {
            Statement::ImportDeclaration(decl) => {
                let r = request(module, decl.source.value.as_str(), decl.source.span);
                for specifier in decl.specifiers.iter().flatten() {
                    let (local, name) = match specifier {
                        ImportDeclarationSpecifier::ImportSpecifier(s) => {
                            (&s.local, ImportName::Name(export_name(&s.imported)))
                        }
                        ImportDeclarationSpecifier::ImportDefaultSpecifier(s) => {
                            (&s.local, ImportName::Name("default".into()))
                        }
                        ImportDeclarationSpecifier::ImportNamespaceSpecifier(s) => {
                            (&s.local, ImportName::Namespace)
                        }
                    };
                    module.imports.insert(
                        local.symbol_id(),
                        Import {
                            request: r,
                            name,
                            span: specifier.span(),
                        },
                    );
                }
            }
            Statement::ExportFromDeclaration(decl) => {
                let r = request(module, decl.source.value.as_str(), decl.source.span);
                for spec in &decl.specifiers {
                    module.exports.insert(
                        export_name(&spec.exported),
                        Export::Reexport {
                            request: r,
                            name: ImportName::Name(export_name(&spec.local)),
                            span: spec.span,
                        },
                    );
                }
            }
            Statement::ExportDeclaration(decl) => {
                for (name, symbol) in declaration_bindings(&decl.declaration) {
                    module.exports.insert(name, Export::Local(symbol));
                }
            }
            Statement::ExportNamedDeclaration(decl) => {
                for spec in &decl.specifiers {
                    let symbol = match &spec.local {
                        ModuleExportName::IdentifierReference(ident) => module.symbol_of(ident),
                        _ => None,
                    };
                    match symbol {
                        Some(symbol) => {
                            module
                                .exports
                                .insert(export_name(&spec.exported), Export::Local(symbol));
                        }
                        None => report.error_at(
                            &module.id,
                            spec.local.span(),
                            format!("exported name '{}' is not declared", spec.local.name()),
                        ),
                    }
                }
            }
            Statement::ExportDefaultDeclaration(decl) => {
                let export = match &decl.declaration {
                    ExportDefaultDeclarationKind::FunctionDeclaration(f) => {
                        f.id.as_ref()
                            .map_or(Export::Default, |id| Export::Local(id.symbol_id()))
                    }
                    ExportDefaultDeclarationKind::ClassDeclaration(c) => {
                        c.id.as_ref()
                            .map_or(Export::Default, |id| Export::Local(id.symbol_id()))
                    }
                    _ => Export::Default,
                };
                module.exports.insert("default".into(), export);
            }
            Statement::ExportAllDeclaration(decl) => {
                let r = request(module, decl.source.value.as_str(), decl.source.span);
                match &decl.exported {
                    Some(name) => {
                        module.exports.insert(
                            export_name(name),
                            Export::Reexport {
                                request: r,
                                name: ImportName::Namespace,
                                span: decl.span,
                            },
                        );
                    }
                    None => module.stars.push(r),
                }
            }
            _ => {}
        }
    }
}

pub fn declaration_bindings(declaration: &Declaration) -> Vec<(String, SymbolId)> {
    match declaration {
        Declaration::VariableDeclaration(var) => var
            .declarations
            .iter()
            .flat_map(|d| d.id.get_binding_identifiers())
            .map(|id| (id.name.to_string(), id.symbol_id()))
            .collect(),
        Declaration::FunctionDeclaration(f) => {
            f.id.iter()
                .map(|id| (id.name.to_string(), id.symbol_id()))
                .collect()
        }
        Declaration::ClassDeclaration(c) => {
            c.id.iter()
                .map(|id| (id.name.to_string(), id.symbol_id()))
                .collect()
        }
        _ => Vec::new(),
    }
}

pub fn builtin_exports() -> Result<HashMap<String, HashSet<String>>> {
    let allocator = Allocator::default();
    let ids: Vec<&str> = deflorta_assets::BUILTIN_MODULES
        .iter()
        .map(|(id, _)| *id)
        .collect();
    let graph = Graph::build(&allocator, &Builtins, &ids);
    if graph.report.has_errors() {
        bail!("{}", graph.report.render(&crate::project::sources(&graph)));
    }
    Ok(graph
        .modules
        .iter()
        .map(|module| (module.id.clone(), module.exports.keys().cloned().collect()))
        .collect())
}
