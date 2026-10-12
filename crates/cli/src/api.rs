use std::collections::{HashMap, HashSet};

use oxc::ast::AstKind;
use oxc::ast::ast::{
    Argument, AssignmentExpression, BindingPattern, CallExpression, Expression,
    IdentifierReference, NewExpression, ObjectExpression, ObjectPropertyKind, PropertyKind,
    SimpleAssignmentTarget, StaticMemberExpression, TaggedTemplateExpression, UnaryExpression,
    UnaryOperator, UpdateExpression, VariableDeclarator,
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
    /// Values written to `store` or `persistent` that JSON cannot round-trip.
    pub unserializable: Vec<(String, Loc)>,
    /// Label code that loading or rollback would not replay faithfully.
    pub replay: Vec<(String, Loc)>,
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

pub fn analyze(graph: &Graph) -> Facts {
    let builtins = &graph.builtins;
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
            builtins,
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
            builtins,
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
    builtins: &HashMap<String, HashSet<String>>,
    callee: &Expression,
) -> Callee {
    match expression_target(graph, m, builtins, callee) {
        Some(Target::External(module, name)) => match api_name(&module, &name) {
            Some(name) if name == "nvlNarrator" => Callee::Character,
            Some(name) => Callee::Api(name),
            None => Callee::Other,
        },
        Some(Target::Local(k, symbol)) if characters.contains(&(k, symbol)) => Callee::Character,
        _ => match callee.get_inner_expression() {
            Expression::StaticMemberExpression(member) => {
                match classify(graph, m, characters, builtins, &member.object) {
                    Callee::Api(name) => Callee::Method(name, member.property.name.to_string()),
                    _ => Callee::Other,
                }
            }
            _ => Callee::Other,
        },
    }
}

fn expression_target(
    graph: &Graph,
    m: usize,
    builtins: &HashMap<String, HashSet<String>>,
    expression: &Expression,
) -> Option<Target> {
    match expression {
        Expression::Identifier(ident) => target_of(graph, m, ident),
        Expression::StaticMemberExpression(member) => {
            let property = member.property.name.as_str();
            match expression_target(graph, m, builtins, &member.object)? {
                Target::Namespace(k) => graph
                    .resolve_export(k, property, builtins, &mut HashSet::new())
                    .ok(),
                Target::ExternalNamespace(module) => {
                    Some(Target::External(module, property.to_owned()))
                }
                _ => None,
            }
        }
        Expression::ParenthesizedExpression(inner) => {
            expression_target(graph, m, builtins, &inner.expression)
        }
        _ => None,
    }
}

struct CharacterFinder<'g, 'a> {
    graph: &'g Graph<'a>,
    module: usize,
    characters: &'g mut HashSet<(usize, SymbolId)>,
    builtins: &'g HashMap<String, HashSet<String>>,
}

impl<'a> Visit<'a> for CharacterFinder<'_, 'a> {
    fn visit_variable_declarator(&mut self, it: &VariableDeclarator<'a>) {
        if let (BindingPattern::BindingIdentifier(id), Some(Expression::CallExpression(call))) =
            (&it.id, &it.init)
            && matches!(
                classify(self.graph, self.module, self.characters, self.builtins, &call.callee),
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
    builtins: &'g HashMap<String, HashSet<String>>,
    facts: &'g mut Facts,
    /// Nesting depth inside `label()` bodies.
    label_depth: u32,
}

fn argument<'b, 'a>(args: &'b [Argument<'a>], index: usize) -> Option<&'b Expression<'a>> {
    args.get(index).and_then(Argument::as_expression)
}

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

/// What a member chain such as `store.flags.met` ultimately writes to.
#[derive(Clone, Copy)]
enum Owner {
    /// The saved `store` or `persistent` object.
    Saved(&'static str),
    /// A module-level binding, which loading and rollback do not restore.
    Module(usize, SymbolId),
}

/// Methods that change an array, Map, or Set in place.
const MUTATORS: &[&str] = &[
    "push",
    "pop",
    "shift",
    "unshift",
    "splice",
    "sort",
    "reverse",
    "fill",
    "copyWithin",
    "set",
    "add",
    "delete",
    "clear",
];

const CLOCK: &str =
    "story code that depends on the clock replays differently after loading or rollback";

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
            if let Expression::ObjectExpression(object) = choice {
                self.text(property(object, "label"));
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
            "playMovie" => self.file(arg(0), FileKind::Video),
            "imageDissolve" => self.file(arg(0), FileKind::Image),
            "Image" | "Video" => {
                if let Some(Expression::ObjectExpression(props)) = arg(0) {
                    self.component_files(name, props);
                }
            }
            "jsx" | "jsxs" | "createElement" => {
                if let Some(component) = arg(0)
                    && let Callee::Api(component) = classify(
                        self.graph,
                        self.module,
                        self.characters,
                        self.builtins,
                        component,
                    )
                    && let Some(Expression::ObjectExpression(props)) = arg(1)
                {
                    self.component_files(&component, props);
                }
            }
            "defaults" => {
                if let Some(values) = arg(0) {
                    self.write(Some(Owner::Saved("store")), Some(values), values.span());
                }
            }
            "setTimer" => self.timer(span),
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

    fn component_files(&mut self, name: &str, props: &ObjectExpression<'a>) {
        match name {
            "Image" => {
                self.file(property(props, "src"), FileKind::Image);
                self.file(property(props, "hoverSrc"), FileKind::Image);
            }
            "Video" => self.file(property(props, "src"), FileKind::Video),
            _ => {}
        }
    }

    fn is_global(&self, expression: &Expression, name: &str) -> bool {
        matches!(expression, Expression::Identifier(ident)
            if ident.name == name && self.module().symbol_of(ident).is_none())
    }

    fn timer(&mut self, span: Span) {
        if self.label_depth > 0 {
            self.facts.replay.push((
                "timers in a label are not replayed by loading or rollback; await pause(seconds) instead".into(),
                self.loc(span),
            ));
        }
    }

    fn global_name(&self, expression: &Expression<'a>) -> Option<&'a str> {
        match expression.get_inner_expression() {
            Expression::Identifier(ident) if self.module().symbol_of(ident).is_none() => {
                Some(ident.name.as_str())
            }
            _ => None,
        }
    }

    fn owner(&self, expression: &Expression<'a>) -> Option<Owner> {
        let mut expression = expression.get_inner_expression();
        loop {
            match expression_target(self.graph, self.module, self.builtins, expression) {
                Some(Target::External(module, name)) => {
                    return match api_name(&module, &name).as_deref() {
                        Some("store") => Some(Owner::Saved("store")),
                        Some("persistent") => Some(Owner::Saved("persistent")),
                        _ => None,
                    };
                }
                Some(Target::Local(k, symbol)) => {
                    return (!self.is_alias(k, symbol)).then_some(Owner::Module(k, symbol));
                }
                Some(_) => return None,
                None => {}
            }
            expression = expression
                .as_member_expression()?
                .object()
                .get_inner_expression();
        }
    }

    /// A binding initialized from another reference (`const s = store`) may
    /// alias anything, so writes through it are not attributed to the module.
    fn is_alias(&self, k: usize, symbol: SymbolId) -> bool {
        let semantic = &self.graph.modules[k].semantic;
        matches!(semantic.symbol_declaration(symbol).kind(),
        AstKind::VariableDeclarator(d) if d.init.as_ref().is_some_and(|init| {
            let init = init.get_inner_expression();
            matches!(init, Expression::Identifier(_)) || init.is_member_expression()
        }))
    }

    fn target_owner(&self, target: &SimpleAssignmentTarget<'a>) -> Option<Owner> {
        match target {
            SimpleAssignmentTarget::AssignmentTargetIdentifier(ident) => {
                let symbol = self.module().symbol_of(ident)?;
                self.module()
                    .is_top_level(symbol)
                    .then_some(Owner::Module(self.module, symbol))
            }
            _ => self.owner(target.as_member_expression()?.object()),
        }
    }

    /// Records a write to `owner`: `value` must survive JSON when saved, and
    /// label code must not keep story state in module bindings.
    fn write(&mut self, owner: Option<Owner>, value: Option<&Expression<'a>>, span: Span) {
        match owner {
            Some(Owner::Saved(object)) => {
                let mut found = Vec::new();
                if let Some(value) = value {
                    self.unserializable(value, &mut found);
                }
                for (reason, span) in found {
                    self.facts.unserializable.push((
                        format!("{reason}; values in {object} must be JSON-serializable"),
                        self.loc(span),
                    ));
                }
            }
            Some(Owner::Module(k, symbol)) if self.label_depth > 0 => {
                let name = self.graph.modules[k].semantic.scoping().symbol_name(symbol);
                self.facts.replay.push((
                    format!("module variable '{name}' is not saved, so loading and rollback do not restore changes made in a label; keep story state in store"),
                    self.loc(span),
                ));
            }
            _ => {}
        }
    }

    /// Collects the parts of `value` that would not survive the JSON round
    /// trip of a save, with why.
    fn unserializable(&self, value: &Expression<'a>, found: &mut Vec<(String, Span)>) {
        let value = value.get_inner_expression();
        let reason = match value {
            Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => {
                "functions are not saved".into()
            }
            Expression::ClassExpression(_) => "classes are not saved".into(),
            Expression::RegExpLiteral(_) => "a regular expression is saved as {}".into(),
            Expression::BigIntLiteral(_) => "a BigInt makes saving fail".into(),
            Expression::Identifier(ident) => match self.global_name(value) {
                Some(name @ ("NaN" | "Infinity")) => format!("{name} is saved as null"),
                Some(_) => return,
                None => match self.callable(ident) {
                    Some(reason) => reason.into(),
                    None => return,
                },
            },
            Expression::UnaryExpression(unary)
                if unary.operator == UnaryOperator::UnaryNegation =>
            {
                return self.unserializable(&unary.argument, found);
            }
            Expression::NewExpression(new) => match self.global_name(&new.callee) {
                Some("Array" | "Object" | "String" | "Number" | "Boolean") => return,
                Some("Date") => "a Date is saved as a string; save a timestamp instead".into(),
                Some(name @ ("Map" | "Set" | "WeakMap" | "WeakSet" | "RegExp" | "Promise")) => {
                    format!("a {name} is saved as {{}}; use a plain object or array")
                }
                _ => "a class instance is saved as a plain object without its methods".into(),
            },
            Expression::CallExpression(call)
                if self.global_name(&call.callee) == Some("Symbol") =>
            {
                "symbols are not saved".into()
            }
            Expression::ObjectExpression(object) => {
                for p in &object.properties {
                    if let ObjectPropertyKind::ObjectProperty(p) = p
                        && p.kind == PropertyKind::Init
                    {
                        self.unserializable(&p.value, found);
                    }
                }
                return;
            }
            Expression::ArrayExpression(array) => {
                for element in array.elements.iter().filter_map(|e| e.as_expression()) {
                    self.unserializable(element, found);
                }
                return;
            }
            Expression::ConditionalExpression(c) => {
                self.unserializable(&c.consequent, found);
                return self.unserializable(&c.alternate, found);
            }
            Expression::LogicalExpression(l) => {
                self.unserializable(&l.left, found);
                return self.unserializable(&l.right, found);
            }
            _ => return,
        };
        found.push((reason, value.span()));
    }

    /// Whether `ident` names a function or class declared in the game's code.
    fn callable(&self, ident: &IdentifierReference<'a>) -> Option<&'static str> {
        let (k, symbol) = match target_of(self.graph, self.module, ident) {
            Some(Target::Local(k, symbol)) => (k, symbol),
            Some(_) => return None,
            None => (self.module, self.module().symbol_of(ident)?),
        };
        let semantic = &self.graph.modules[k].semantic;
        let flags = semantic.scoping().symbol_flags(symbol);
        let initializer = match semantic.symbol_declaration(symbol).kind() {
            AstKind::VariableDeclarator(d) if flags.is_const_variable() => d.init.as_ref(),
            _ => None,
        };
        if flags.is_class() {
            return Some("classes are not saved");
        }
        if flags.is_function() {
            return Some("functions are not saved");
        }
        match initializer.map(Expression::get_inner_expression) {
            Some(Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_)) => {
                Some("functions are not saved")
            }
            Some(Expression::ClassExpression(_)) => Some("classes are not saved"),
            _ => None,
        }
    }

    /// Method calls that change their object: `store.items.push(…)` or
    /// `Object.assign(store, …)`.
    fn mutating_call(&mut self, call: &CallExpression<'a>) {
        let Some(member) = call.callee.get_inner_expression().as_member_expression() else {
            return;
        };
        let method = member.static_property_name();
        let (target, skip) =
            if method == Some("assign") && self.is_global(member.object(), "Object") {
                let Some(target) = argument(&call.arguments, 0) else {
                    return;
                };
                (target, 1)
            } else if method.is_some_and(|m| MUTATORS.contains(&m)) {
                (member.object(), 0)
            } else {
                return;
            };
        match self.owner(target) {
            owner @ Some(Owner::Module(..)) => self.write(owner, None, call.span),
            owner => {
                for value in call
                    .arguments
                    .iter()
                    .skip(skip)
                    .filter_map(Argument::as_expression)
                {
                    self.write(owner, Some(value), value.span());
                }
            }
        }
    }
}

impl<'a> Visit<'a> for Analyzer<'_, 'a> {
    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        let mut in_label = false;
        match classify(
            self.graph,
            self.module,
            self.characters,
            self.builtins,
            &it.callee,
        ) {
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
            Callee::Other => {
                if self.is_global(&it.callee, "setTimeout") {
                    self.timer(it.span);
                }
                self.mutating_call(it);
            }
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
            classify(
                self.graph,
                self.module,
                self.characters,
                self.builtins,
                &it.tag
            ),
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
                self.facts.replay.push((
                    "Math.random() gives different results after loading or rollback; use random() or randInt()".into(),
                    self.loc(it.span),
                ));
            } else if self.is_global(&it.object, "Date") {
                self.facts.replay.push((CLOCK.into(), self.loc(it.span)));
            }
        }
        walk::walk_static_member_expression(self, it);
    }

    fn visit_new_expression(&mut self, it: &NewExpression<'a>) {
        if self.label_depth > 0 && self.is_global(&it.callee, "Date") {
            self.facts.replay.push((CLOCK.into(), self.loc(it.span)));
        }
        walk::walk_new_expression(self, it);
    }

    fn visit_assignment_expression(&mut self, it: &AssignmentExpression<'a>) {
        if let Some(target) = it.left.as_simple_assignment_target() {
            self.write(self.target_owner(target), Some(&it.right), it.span);
        }
        walk::walk_assignment_expression(self, it);
    }

    fn visit_update_expression(&mut self, it: &UpdateExpression<'a>) {
        self.write(self.target_owner(&it.argument), None, it.span);
        walk::walk_update_expression(self, it);
    }

    fn visit_unary_expression(&mut self, it: &UnaryExpression<'a>) {
        if it.operator == UnaryOperator::Delete
            && let Some(member) = it.argument.get_inner_expression().as_member_expression()
        {
            self.write(self.owner(member.object()), None, it.span);
        }
        walk::walk_unary_expression(self, it);
    }

    fn visit_import_expression(&mut self, it: &oxc::ast::ast::ImportExpression<'a>) {
        self.facts.dynamic_imports.push(self.loc(it.span));
        walk::walk_import_expression(self, it);
    }
}
