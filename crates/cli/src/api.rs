//! Static analysis of how game code uses the scripting API: labels, images,
//! files, translatable text and non-deterministic story code.
//!
//! Calls are recognized through the linked module graph, so aliased imports
//! (`import { say as s }`), namespace imports and re-exports from other game
//! modules are followed. Only literal arguments can be checked.

use std::collections::{HashMap, HashSet};

use oxc::ast::ast::{
    Argument, ArrayExpressionElement, BindingPattern, CallExpression, Expression,
    IdentifierReference, NewExpression, ObjectExpression, ObjectPropertyKind,
    StaticMemberExpression, TaggedTemplateExpression, VariableDeclarator,
};
use oxc::ast_visit::{Visit, walk};
use oxc::semantic::SymbolId;
use oxc::span::{GetSpan, Span};

use crate::bundle::static_string;
use crate::graph::{Graph, Module, Target};

#[derive(Clone, Copy, Debug)]
pub struct Loc {
    pub module: usize,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    Image,
    Audio,
    Video,
    Text,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageUse {
    /// `scene(name)`: a declared image or `images/<name>.png`.
    Background,
    /// `show(name)`: a layered image with attributes, or like a background.
    Sprite,
    Preload,
}

/// Attributes of a layered image; `None` when they are not literal.
pub type Attributes = Option<HashSet<String>>;

#[derive(Default)]
pub struct Facts {
    pub labels: Vec<(String, Loc)>,
    pub label_refs: Vec<(String, Loc)>,
    /// `jump()`/`call()` with a computed label name.
    pub dynamic_label_refs: bool,
    pub images: HashMap<String, Loc>,
    pub layered: HashMap<String, Attributes>,
    /// `image()`/`layeredImage()` with a computed name.
    pub dynamic_images: bool,
    pub image_uses: Vec<(String, ImageUse, Loc)>,
    pub files: Vec<(String, FileKind, Loc)>,
    /// Translatable strings, in source order.
    pub strings: Vec<(String, Loc)>,
    /// Translatable positions whose text is computed (template substitutions).
    pub dynamic_strings: Vec<Loc>,
    pub nondeterministic: Vec<(&'static str, Loc)>,
    pub dynamic_imports: Vec<Loc>,
}

/// The public API name of an engine export (`deflorta/story`'s `sceneStatement`
/// is the public `scene`), or None for exports that are not API functions.
fn api_name(module: &str, name: &str) -> Option<String> {
    match (module, name) {
        ("deflorta/story", "sceneStatement") | ("deflorta/scene", "setScene") => {
            Some("scene".into())
        }
        ("deflorta/scene", "scene") => None,
        _ => Some(name.to_owned()),
    }
}

enum Callee {
    Api(String),
    /// A method of an API object: `music.play`.
    Method(String, String),
    /// A speaker created with `character()`.
    Character,
    Other,
}

/// Analyzes every module of a linked graph.
pub fn analyze(graph: &Graph) -> Facts {
    let mut characters = HashSet::new();
    for &m in &graph.order {
        let module = &graph.modules[m];
        if module.broken {
            continue;
        }
        let mut finder = CharacterFinder {
            graph,
            module: m,
            characters: &mut characters,
        };
        finder.visit_program(module.program);
    }
    let mut facts = Facts::default();
    for &m in &graph.order {
        let module = &graph.modules[m];
        if module.broken {
            continue;
        }
        let mut analyzer = Analyzer {
            graph,
            module: m,
            characters: &characters,
            facts: &mut facts,
            label_depth: 0,
        };
        analyzer.visit_program(module.program);
    }
    facts
}

fn target_of(graph: &Graph, m: usize, ident: &IdentifierReference) -> Option<Target> {
    let module = &graph.modules[m];
    let symbol = module.symbol_of(ident)?;
    module.links.get(&symbol).cloned().or_else(|| {
        module
            .is_top_level(symbol)
            .then_some(Target::Local(m, symbol))
    })
}

fn classify(
    graph: &Graph,
    m: usize,
    characters: &HashSet<(usize, SymbolId)>,
    callee: &Expression,
) -> Callee {
    match callee {
        Expression::Identifier(ident) => match target_of(graph, m, ident) {
            Some(Target::External(module, name)) => match api_name(&module, &name) {
                Some(name) if name == "nvlNarrator" => Callee::Character,
                Some(name) => Callee::Api(name),
                None => Callee::Other,
            },
            Some(Target::Local(k, symbol)) if characters.contains(&(k, symbol)) => {
                Callee::Character
            }
            _ => Callee::Other,
        },
        Expression::StaticMemberExpression(member) => {
            let Expression::Identifier(object) = &member.object else {
                return Callee::Other;
            };
            let property = member.property.name.as_str();
            match target_of(graph, m, object) {
                Some(Target::ExternalNamespace(module)) => {
                    api_name(&module, property).map_or(Callee::Other, |name| {
                        if name == "nvlNarrator" {
                            Callee::Character
                        } else {
                            Callee::Api(name)
                        }
                    })
                }
                Some(Target::External(module, name)) => api_name(&module, &name)
                    .map_or(Callee::Other, |name| {
                        Callee::Method(name, property.to_owned())
                    }),
                _ => Callee::Other,
            }
        }
        Expression::ParenthesizedExpression(inner) => {
            classify(graph, m, characters, &inner.expression)
        }
        _ => Callee::Other,
    }
}

struct CharacterFinder<'g, 'a> {
    graph: &'g Graph<'a>,
    module: usize,
    characters: &'g mut HashSet<(usize, SymbolId)>,
}

impl<'a> Visit<'a> for CharacterFinder<'_, 'a> {
    fn visit_variable_declarator(&mut self, it: &VariableDeclarator<'a>) {
        if let (BindingPattern::BindingIdentifier(id), Some(Expression::CallExpression(call))) =
            (&it.id, &it.init)
            && matches!(
                classify(self.graph, self.module, self.characters, &call.callee),
                Callee::Api(name) if name == "character"
            )
        {
            self.characters.insert((self.module, id.symbol_id()));
        }
        walk::walk_variable_declarator(self, it);
    }
}

struct Analyzer<'g, 'a> {
    graph: &'g Graph<'a>,
    module: usize,
    characters: &'g HashSet<(usize, SymbolId)>,
    facts: &'g mut Facts,
    /// Nesting depth inside `label()` bodies.
    label_depth: u32,
}

fn argument<'b, 'a>(args: &'b [Argument<'a>], index: usize) -> Option<&'b Expression<'a>> {
    args.get(index).and_then(Argument::as_expression)
}

/// The value of a non-computed `key: value` property in an object literal.
fn property<'b, 'a>(object: &'b ObjectExpression<'a>, key: &str) -> Option<&'b Expression<'a>> {
    object.properties.iter().find_map(|p| match p {
        ObjectPropertyKind::ObjectProperty(p)
            if !p.computed && p.key.static_name().as_deref() == Some(key) =>
        {
            Some(&p.value)
        }
        _ => None,
    })
}

impl<'a> Analyzer<'_, 'a> {
    const fn loc(&self, span: Span) -> Loc {
        Loc {
            module: self.module,
            span,
        }
    }

    fn module(&self) -> &Module<'a> {
        &self.graph.modules[self.module]
    }

    fn file(&mut self, expression: Option<&Expression<'a>>, kind: FileKind) {
        if let Some(expression) = expression
            && let Some(path) = static_string(expression)
        {
            self.facts
                .files
                .push((path.to_owned(), kind, self.loc(expression.span())));
        }
    }

    /// Text shown through `_()`: collected when literal, noted when computed.
    fn text(&mut self, expression: Option<&Expression<'a>>) {
        let Some(expression) = expression else { return };
        if let Some(text) = static_string(expression) {
            self.facts
                .strings
                .push((text.to_owned(), self.loc(expression.span())));
        } else if matches!(expression, Expression::TemplateLiteral(_)) {
            self.facts.dynamic_strings.push(self.loc(expression.span()));
        }
    }

    fn image_use(&mut self, expression: Option<&Expression<'a>>, kind: ImageUse) {
        if let Some(expression) = expression
            && let Some(name) = static_string(expression)
        {
            self.facts
                .image_uses
                .push((name.to_owned(), kind, self.loc(expression.span())));
        }
    }

    fn label_ref(&mut self, expression: Option<&Expression<'a>>) {
        if let Some(expression) = expression {
            match static_string(expression) {
                Some(name) => self
                    .facts
                    .label_refs
                    .push((name.to_owned(), self.loc(expression.span()))),
                None => self.facts.dynamic_label_refs = true,
            }
        }
    }

    fn choices(&mut self, expression: Option<&Expression<'a>>) {
        let Some(Expression::ArrayExpression(array)) = expression else {
            return;
        };
        for element in &array.elements {
            let Some(choice) = element.as_expression() else {
                continue;
            };
            match choice {
                Expression::ArrayExpression(pair) => {
                    self.text(
                        pair.elements
                            .first()
                            .and_then(ArrayExpressionElement::as_expression),
                    );
                }
                Expression::ObjectExpression(object) => self.text(property(object, "text")),
                other => self.text(Some(other)),
            }
        }
    }

    fn layered_image(&mut self, args: &[Argument<'a>]) {
        let Some(tag) = argument(args, 0).and_then(static_string) else {
            self.facts.dynamic_images = true;
            return;
        };
        let mut attributes = Some(HashSet::new());
        let Some(Expression::ArrayExpression(layers)) = argument(args, 1) else {
            self.facts.layered.insert(tag.to_owned(), None);
            return;
        };
        for layer in &layers.elements {
            let Some(Expression::ObjectExpression(layer)) = layer.as_expression() else {
                attributes = None;
                continue;
            };
            self.file(property(layer, "src"), FileKind::Image);
            if let Some(attribute) = property(layer, "attribute") {
                match (static_string(attribute), attributes.as_mut()) {
                    (Some(name), Some(set)) => {
                        set.insert(name.to_owned());
                    }
                    (None, _) => attributes = None,
                    _ => {}
                }
            }
            if let Some(options) = property(layer, "options") {
                let Expression::ObjectExpression(options) = options else {
                    attributes = None;
                    continue;
                };
                for option in &options.properties {
                    match option {
                        ObjectPropertyKind::ObjectProperty(p) if !p.computed => {
                            if let (Some(name), Some(set)) =
                                (p.key.static_name(), attributes.as_mut())
                            {
                                set.insert(name.into_owned());
                            }
                            self.file(Some(&p.value), FileKind::Image);
                        }
                        _ => attributes = None,
                    }
                }
            }
        }
        self.facts.layered.insert(tag.to_owned(), attributes);
    }

    fn api_call(&mut self, name: &str, args: &[Argument<'a>], span: Span) {
        let arg = |i| argument(args, i);
        match name {
            "label" => match arg(0).and_then(static_string) {
                Some(label) => self.facts.labels.push((label.to_owned(), self.loc(span))),
                None => self.facts.dynamic_label_refs = true,
            },
            "jump" | "call" | "newGame" => self.label_ref(arg(0)),
            "scene" => self.image_use(arg(0), ImageUse::Background),
            "show" => self.image_use(arg(0), ImageUse::Sprite),
            "preload" => {
                for i in 0..args.len() {
                    self.image_use(arg(i), ImageUse::Preload);
                }
            }
            "image" => {
                match arg(0).and_then(static_string) {
                    Some(name) => {
                        self.facts.images.insert(name.to_owned(), self.loc(span));
                    }
                    None => self.facts.dynamic_images = true,
                }
                self.file(arg(1), FileKind::Image);
            }
            "layeredImage" => self.layered_image(args),
            "playMovie" | "video" => self.file(arg(0), FileKind::Video),
            "img" | "imageDissolve" => self.file(arg(0), FileKind::Image),
            "imageButton" => {
                self.file(arg(0), FileKind::Image);
                self.file(arg(1), FileKind::Image);
            }
            "voice" => self.file(arg(0), FileKind::Audio),
            "readText" => self.file(arg(0), FileKind::Text),
            "configure" => {
                if let Some(Expression::ObjectExpression(options)) = arg(0) {
                    self.file(property(options, "menuBackground"), FileKind::Image);
                    self.file(property(options, "menuVideo"), FileKind::Video);
                    self.text(property(options, "title"));
                }
            }
            "_" | "prompt" | "character" => {
                if arg(0).and_then(static_string).is_some() {
                    self.text(arg(0));
                }
            }
            "say" => {
                if args.len() >= 2 {
                    if arg(0).and_then(static_string).is_some() {
                        self.text(arg(0));
                    }
                    self.text(arg(1));
                    if let Some(Expression::ObjectExpression(options)) = arg(2) {
                        self.file(property(options, "voice"), FileKind::Audio);
                    }
                } else {
                    self.text(arg(0));
                }
            }
            "menu" => {
                if matches!(arg(0), Some(Expression::ArrayExpression(_))) {
                    self.choices(arg(0));
                } else {
                    self.text(arg(0));
                    self.choices(arg(1));
                }
            }
            _ => {}
        }
    }

    fn is_global(&self, expression: &Expression, name: &str) -> bool {
        matches!(expression, Expression::Identifier(ident)
            if ident.name == name && self.module().symbol_of(ident).is_none())
    }
}

impl<'a> Visit<'a> for Analyzer<'_, 'a> {
    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        let mut in_label = false;
        match classify(self.graph, self.module, self.characters, &it.callee) {
            Callee::Api(name) => {
                in_label = name == "label";
                self.api_call(&name, &it.arguments, it.span);
            }
            Callee::Method(object, method) => {
                if matches!(
                    (object.as_str(), method.as_str()),
                    ("music" | "sound", "play")
                ) {
                    self.file(argument(&it.arguments, 0), FileKind::Audio);
                }
            }
            Callee::Character => {
                self.text(argument(&it.arguments, 0));
                if let Some(Expression::ObjectExpression(options)) = argument(&it.arguments, 1) {
                    self.file(property(options, "voice"), FileKind::Audio);
                }
            }
            Callee::Other => {}
        }
        if in_label {
            self.label_depth += 1;
        }
        walk::walk_call_expression(self, it);
        if in_label {
            self.label_depth -= 1;
        }
    }

    fn visit_tagged_template_expression(&mut self, it: &TaggedTemplateExpression<'a>) {
        if matches!(
            classify(self.graph, self.module, self.characters, &it.tag),
            Callee::Character
        ) {
            // Speakers join tagged templates with String.raw, so the raw text is the key.
            if it.quasi.expressions.is_empty() {
                if let Some(quasi) = it.quasi.quasis.first() {
                    self.facts
                        .strings
                        .push((quasi.value.raw.to_string(), self.loc(it.quasi.span)));
                }
            } else {
                self.facts.dynamic_strings.push(self.loc(it.quasi.span));
            }
        }
        walk::walk_tagged_template_expression(self, it);
    }

    fn visit_static_member_expression(&mut self, it: &StaticMemberExpression<'a>) {
        if self.label_depth > 0 {
            if self.is_global(&it.object, "Math") && it.property.name == "random" {
                self.facts
                    .nondeterministic
                    .push(("Math.random() gives different results after loading or rollback; use random() or randInt()", self.loc(it.span)));
            } else if self.is_global(&it.object, "Date") {
                self.facts
                    .nondeterministic
                    .push(("story code that depends on the clock replays differently after loading or rollback", self.loc(it.span)));
            }
        }
        walk::walk_static_member_expression(self, it);
    }

    fn visit_new_expression(&mut self, it: &NewExpression<'a>) {
        if self.label_depth > 0 && self.is_global(&it.callee, "Date") {
            self.facts
                .nondeterministic
                .push(("story code that depends on the clock replays differently after loading or rollback", self.loc(it.span)));
        }
        walk::walk_new_expression(self, it);
    }

    fn visit_import_expression(&mut self, it: &oxc::ast::ast::ImportExpression<'a>) {
        self.facts.dynamic_imports.push(self.loc(it.span));
        walk::walk_import_expression(self, it);
    }
}
