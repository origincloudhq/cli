//! A command per method: its flags, from the method's request message, and
//! the code that builds the request from them and calls the SDK.
//!
//! The scheme, which the README describes for users:
//!
//! - The command is the method's name in kebab case: `CreateVolume` is
//!   `create-volume`.
//! - A resource name is given as its parts. Each variable of a name
//!   pattern other than the resource's own id is a flag shared by every
//!   name the command builds (`--project`, `--zone`); the resource's id is
//!   the flag named after the resource (`--volume`), or after the field for
//!   a reference inside the request (`--source-snapshot`). That flag also
//!   takes a whole name, whose parts then fill in the shared flags.
//! - Each other field of the request, and of the resource it carries, is a
//!   flag named after it. Output-only fields are left out, and so is the
//!   resource's name, which its parts make.
//! - An update is a partial update: a flag given sets its field and adds
//!   it to `update_mask`, and at least one is required. A map or a list is
//!   replaced whole: its repeatable flag (`--tag KEY=VALUE`) and its
//!   `--no-` flag (`--no-tags`), which names the empty set, exclude each
//!   other, so emptying it is only reachable by name.
//! - A boolean takes its value explicitly (`--flag true|false`).
//! - A delete requires `--yes`, checked before any request is built.
//! - A method answering with an operation waits for it, unless
//!   `--no-wait`; a list method lists every page, unless `--page-size` or
//!   `--page-token` asks for one.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use anyhow::Result;
use heck::{ToKebabCase, ToLowerCamelCase, ToSnakeCase, ToUpperCamelCase};
use prost_reflect::{
    DescriptorPool, DynamicMessage, FieldDescriptor, Kind, MessageDescriptor, MethodDescriptor,
    Value,
};

use crate::docs::{self, Comments, Vocabulary};
use crate::policy::Policy;

const OPERATION: &str = "google.longrunning.Operation";
const EMPTY: &str = "google.protobuf.Empty";
const FIELD_MASK: &str = "google.protobuf.FieldMask";

// `google.api.FieldBehavior` values.
const REQUIRED: i32 = 2;
const OUTPUT_ONLY: i32 = 3;
const IMMUTABLE: i32 = 5;
const IDENTIFIER: i32 = 8;

/// The generated commands, and the methods left without one.
pub struct Generated {
    pub code: String,
    pub skipped: Vec<String>,
}

/// What every method's generation shares.
struct Cx<'a> {
    pool: &'a DescriptorPool,
    comments: &'a Comments,
    policy: &'a Policy,
    package: &'a str,
    /// The service's name in its host and in a zone's `services`: `pvs`.
    service: &'a str,
    /// The SDK module of the package: `::origincloud_pvs::v1beta1`.
    sdk: String,
    /// The name patterns and singular names of the package's resources, by
    /// resource type.
    resources: BTreeMap<String, Resource>,
    /// Every field name of the package, for the help text.
    fields: BTreeSet<String>,
}

#[derive(Clone)]
struct Resource {
    pattern: String,
    singular: String,
}

/// One command: its clap variant, its arguments struct and its handler.
struct Command {
    name: String,
    variant: String,
    handler: String,
    doc: String,
    code: String,
}

pub fn generate(
    pool: &DescriptorPool,
    comments: &Comments,
    policy: &Policy,
    package: &str,
) -> Result<Generated> {
    let version = package.rsplit('.').next().unwrap_or_default();
    let service = package.split('.').nth(1).unwrap_or_default();
    let cx = Cx {
        pool,
        comments,
        policy,
        package,
        service,
        sdk: format!("::origincloud_{service}::{version}"),
        resources: resources(pool),
        fields: pool
            .all_messages()
            .filter(|m| m.package_name() == package)
            .flat_map(|m| m.fields().map(|f| f.name().to_owned()).collect::<Vec<_>>())
            .collect(),
    };

    let mut files: Vec<_> = pool
        .files()
        .filter(|f| f.package_name() == package)
        .collect();
    files.sort_by(|a, b| a.name().cmp(b.name()));

    let mut commands = Vec::new();
    let mut skipped = Vec::new();
    for file in files {
        for service in file.services() {
            for method in service.methods() {
                match command(&cx, &method) {
                    Ok(command) => commands.push(command),
                    Err(why) => skipped.push(format!("{}: {why}", method.full_name())),
                }
            }
        }
    }

    let mut code = String::new();
    code.push_str(
        "#[allow(unused_imports)]\n\
         use super::runtime::{\n\
             api_error, json, list_json, names_any, success, tags, wait, zones, ConfirmedClient,\n\
             Context, ReadClient,\n\
         };\n\
         use crate::confirm;\n\
         use crate::handler::tag_args::TagArg;\n\
         use crate::output::SerializableOutput;\n\n",
    );
    code.push_str(&docs::attributes(
        &format!("The commands of every method of `{package}`."),
        "",
    ));
    code.push_str("#[derive(clap::Subcommand, Debug)]\npub enum PvsCommands {\n");
    for c in &commands {
        let long = c
            .doc
            .split("\n\n")
            .map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect::<Vec<_>>()
            .join("\n\n");
        writeln!(
            code,
            "    #[command(name = {:?}, about = {:?}, long_about = {:?})]\n    {}({}Args),",
            c.name,
            docs::summary(&c.doc),
            long,
            c.variant,
            c.variant
        )?;
    }
    code.push_str("}\n\n");
    code.push_str(&docs::attributes("Runs `command`.", ""));
    code.push_str(
        "pub async fn run(ctx: &Context<'_>, command: PvsCommands) -> anyhow::Result<Box<dyn SerializableOutput>> {\n    match command {\n",
    );
    for c in &commands {
        writeln!(
            code,
            "        PvsCommands::{}(args) => {}(ctx, args).await,",
            c.variant, c.handler
        )?;
    }
    code.push_str("    }\n}\n\n");
    for c in &commands {
        code.push_str(&c.code);
        code.push('\n');
    }

    let file = syn::parse_file(&code)
        .map_err(|e| anyhow::anyhow!("the generated code doesn't parse: {e}\n{code}"))?;
    Ok(Generated {
        code: prettyplease::unparse(&file),
        skipped,
    })
}

/// Every resource the pool declares, by type.
fn resources(pool: &DescriptorPool) -> BTreeMap<String, Resource> {
    let mut out = BTreeMap::new();
    for message in pool.all_messages() {
        let Some(resource) = message_extension(pool, &message.options(), "google.api.resource")
        else {
            continue;
        };
        let text = |name: &str| {
            resource
                .get_field_by_name(name)
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default()
        };
        let pattern = match resource.get_field_by_name("pattern").as_deref() {
            Some(Value::List(patterns)) => patterns
                .first()
                .and_then(|p| p.as_str().map(str::to_owned))
                .unwrap_or_default(),
            _ => String::new(),
        };
        let mut singular = text("singular");
        if singular.is_empty() {
            singular = message.name().to_snake_case();
        }
        out.insert(text("type"), Resource { pattern, singular });
    }
    out
}

fn message_extension(
    pool: &DescriptorPool,
    options: &DynamicMessage,
    name: &str,
) -> Option<DynamicMessage> {
    let ext = pool.get_extension_by_name(name)?;
    if !options.has_extension(&ext) {
        return None;
    }
    match options.get_extension(&ext).into_owned() {
        Value::Message(m) => Some(m),
        _ => None,
    }
}

fn behaviors(pool: &DescriptorPool, field: &FieldDescriptor) -> Vec<i32> {
    let Some(ext) = pool.get_extension_by_name("google.api.field_behavior") else {
        return Vec::new();
    };
    let options = field.options();
    if !options.has_extension(&ext) {
        return Vec::new();
    }
    match options.get_extension(&ext).into_owned() {
        Value::List(values) => values.iter().filter_map(Value::as_enum_number).collect(),
        _ => Vec::new(),
    }
}

/// The resource type a field refers to: `(type, is child_type)`.
fn reference(pool: &DescriptorPool, field: &FieldDescriptor) -> Option<(String, bool)> {
    let r = message_extension(pool, &field.options(), "google.api.resource_reference")?;
    for (name, child) in [("type", false), ("child_type", true)] {
        if let Some(t) = r
            .get_field_by_name(name)
            .and_then(|v| v.as_str().map(str::to_owned))
            .filter(|t| !t.is_empty())
        {
            return Some((t, child));
        }
    }
    None
}

/// The message an operation's result is, for a method answering with an
/// operation.
fn operation_result(pool: &DescriptorPool, m: &MethodDescriptor) -> Option<MessageDescriptor> {
    let info = message_extension(pool, &m.options(), "google.longrunning.operation_info")?;
    let name = info
        .get_field_by_name("response_type")?
        .as_str()?
        .to_owned();
    let name = if name.contains('.') {
        name
    } else {
        format!("{}.{name}", m.parent_file().package_name())
    };
    pool.get_message_by_name(&name)
}

/// The variables of a name pattern, in order: `projects/{project}/zones/{zone}`
/// has `project` and `zone`.
fn variables(pattern: &str) -> Vec<String> {
    pattern
        .split('/')
        .filter_map(|s| s.strip_prefix('{').and_then(|s| s.strip_suffix('}')))
        .map(str::to_owned)
        .collect()
}

/// The pattern of the parent of a resource with `pattern`: without its last
/// collection and id.
fn parent_pattern(pattern: &str) -> String {
    let segments: Vec<&str> = pattern.split('/').collect();
    segments[..segments.len().saturating_sub(2)].join("/")
}

const KEYWORDS: &[&str] = &[
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "crate",
    "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if", "impl",
    "in", "let", "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref",
    "return", "self", "static", "struct", "super", "trait", "true", "try", "type", "typeof",
    "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];

/// A Rust field name: `size_gib`, or `r#type` for a keyword.
fn ident(name: &str) -> String {
    let snake = name.to_snake_case();
    if KEYWORDS.contains(&snake.as_str()) {
        format!("r#{snake}")
    } else {
        snake
    }
}

/// The clap id of an argument: its field name without `r#`.
fn arg_id(ident: &str) -> &str {
    ident.trim_start_matches("r#")
}

/// The singular of a map's or a list's field name, for its repeatable
/// flag: `tags` is `tag`, `addresses` is `address`, `policies` is `policy`.
fn singular(name: &str) -> String {
    if let Some(stem) = name.strip_suffix("ies").filter(|s| !s.is_empty()) {
        return format!("{stem}y");
    }
    if let Some(stem) = name.strip_suffix("es") {
        if ["s", "x", "z", "ch", "sh"]
            .iter()
            .any(|end| stem.ends_with(end))
        {
            return stem.to_owned();
        }
    }
    if name.ends_with("ss") || name.ends_with("us") {
        return name.to_owned();
    }
    name.strip_suffix('s')
        .filter(|s| !s.is_empty())
        .unwrap_or(name)
        .to_owned()
}

/// How a field's value comes from the command line.
#[derive(Clone)]
enum Kind2 {
    /// A number or a string, as its Rust type.
    Scalar(&'static str),
    Bool,
    /// An enum, by the Rust path of its type and its values' names.
    Enum(String, Vec<String>),
    /// `map<string, string>`, as `--{singular} KEY=VALUE`.
    Map,
    /// `repeated string`, as `--{singular} VALUE`.
    List,
    /// A resource name, from its id or whole name.
    Reference(Resource),
}

fn kind(cx: &Cx<'_>, field: &FieldDescriptor) -> Result<Kind2, String> {
    if field.containing_oneof().is_some() {
        return Err(format!(
            "the field {} is in a oneof, or has explicit presence",
            field.name()
        ));
    }
    if let Some((t, false)) = reference(cx.pool, field) {
        let resource = cx
            .resources
            .get(&t)
            .ok_or_else(|| format!("the field {} refers to the unknown type {t}", field.name()))?;
        return Ok(Kind2::Reference(resource.clone()));
    }
    if field.is_map() {
        let Kind::Message(entry) = field.kind() else {
            return Err(format!("the map {} has no entry", field.name()));
        };
        let (k, v) = (entry.map_entry_key_field(), entry.map_entry_value_field());
        return if k.kind() == Kind::String && v.kind() == Kind::String {
            Ok(Kind2::Map)
        } else {
            Err(format!(
                "the map {} isn't map<string, string>",
                field.name()
            ))
        };
    }
    if field.is_list() {
        return if field.kind() == Kind::String {
            Ok(Kind2::List)
        } else {
            Err(format!("the list {} isn't a list of strings", field.name()))
        };
    }
    Ok(match field.kind() {
        Kind::String => Kind2::Scalar("String"),
        Kind::Int32 | Kind::Sint32 | Kind::Sfixed32 => Kind2::Scalar("i32"),
        Kind::Int64 | Kind::Sint64 | Kind::Sfixed64 => Kind2::Scalar("i64"),
        Kind::Uint32 | Kind::Fixed32 => Kind2::Scalar("u32"),
        Kind::Uint64 | Kind::Fixed64 => Kind2::Scalar("u64"),
        Kind::Float => Kind2::Scalar("f32"),
        Kind::Double => Kind2::Scalar("f64"),
        Kind::Bool => Kind2::Bool,
        Kind::Enum(e) => {
            let values = e
                .values()
                .filter(|v| v.number() != 0)
                .map(|v| v.name().to_owned())
                .collect();
            Kind2::Enum(
                format!("{}::{}", cx.sdk, rust_type_path(cx.package, e.full_name())),
                values,
            )
        }
        other => return Err(format!("the field {} is a {other:?}", field.name())),
    })
}

/// The path of a type relative to its package's module: `Volume`, or
/// `volume::State` for one nested in a message.
fn rust_type_path(package: &str, full_name: &str) -> String {
    let local = full_name
        .strip_prefix(package)
        .and_then(|rest| rest.strip_prefix('.'))
        .unwrap_or(full_name);
    let parts: Vec<&str> = local.split('.').collect();
    let (last, outer) = parts.split_last().expect("a type has a name");
    let mut path: Vec<String> = outer.iter().map(|m| m.to_snake_case()).collect();
    path.push(last.to_upper_camel_case());
    path.join("::")
}

/// One flag of a command.
struct Flag {
    ident: String,
    long: String,
    doc: String,
    ty: String,
    attrs: Vec<String>,
}

impl Flag {
    fn render(&self, out: &mut String) {
        out.push_str(&docs::attributes(&self.doc, "    "));
        let mut attrs = vec![format!("long = {:?}", self.long)];
        attrs.extend(self.attrs.iter().cloned());
        let _ = writeln!(
            out,
            "    #[arg({})]\n    pub {}: {},",
            attrs.join(", "),
            self.ident,
            self.ty
        );
    }
}

/// A command's flags and the statements building its request, as they are
/// gathered.
#[derive(Default)]
struct Build {
    flags: Vec<Flag>,
    /// The shared name parts, in order of first use.
    components: Vec<String>,
    /// Statements learning name parts from whole names given.
    learn: Vec<String>,
    /// Statements before the request.
    prelude: Vec<String>,
    /// Field initializers of the request.
    request: Vec<String>,
    /// The flags each request field is set by, for the help.
    vocabulary: BTreeMap<String, String>,
    /// The ids of the flags of an update, at least one of which is needed.
    update_ids: Vec<String>,
}

impl Build {
    fn add_flag(&mut self, flag: Flag) -> Result<(), String> {
        if self.flags.iter().any(|f| f.long == flag.long)
            || self
                .components
                .iter()
                .any(|c| c.to_kebab_case() == flag.long)
        {
            return Err(format!("two fields would both be the flag --{}", flag.long));
        }
        self.flags.push(flag);
        Ok(())
    }

    fn add_components(&mut self, pattern: &str, leaf: Option<&str>) {
        for var in variables(pattern) {
            if Some(var.as_str()) != leaf && !self.components.contains(&var) {
                self.components.push(var);
            }
        }
    }
}

fn component_doc(var: &str) -> String {
    match var {
        "project" => "The project, such as `my-project-123`. The profile's `default-project`, or \
                      `ORIGINCLOUD_PROJECT`, when not given."
            .to_owned(),
        "zone" => "The zone, such as `eu-central1-az1`. The profile's `default-zone`, or \
                   `ORIGINCLOUD_ZONE`, when not given."
            .to_owned(),
        other => format!("The `{{{other}}}` part of the resource names the command builds."),
    }
}

/// The help of a flag taking a resource name.
fn reference_doc(comment: &str, pattern: &str, leaf: &str) -> String {
    let others: Vec<String> = variables(pattern)
        .into_iter()
        .filter(|v| v != leaf)
        .map(|v| format!("`--{}`", v.to_kebab_case()))
        .collect();
    let how = if others.is_empty() {
        format!("Its id, or its whole name, `{pattern}`.")
    } else {
        format!(
            "Its id, completed with {}, or its whole name, `{pattern}`.",
            others.join(" and ")
        )
    };
    if comment.is_empty() {
        how
    } else {
        format!("{comment}\n\n{how}")
    }
}

fn leaf_of(pattern: &str) -> String {
    variables(pattern).pop().unwrap_or_default()
}

/// A one-way field a command sets: the flag, and what the table says.
struct OneWayUse {
    ident: String,
    long: String,
    /// Whether the flag's value is an `Option`, given or not.
    optional: bool,
    verb: String,
    consequence: String,
}

fn command(cx: &Cx<'_>, m: &MethodDescriptor) -> Result<Command, String> {
    if m.is_client_streaming() || m.is_server_streaming() {
        return Err("streaming methods aren't supported".to_owned());
    }
    let request = m.input();
    let result = operation_result(cx.pool, m);
    if m.output().full_name() == OPERATION && result.is_none() {
        return Err("it answers with an operation but names no response_type".to_owned());
    }
    let service = m.parent_service();
    let client = format!(
        "{}Client",
        service
            .name()
            .strip_suffix("Service")
            .unwrap_or(service.name())
    );
    let destructive = m.name().starts_with("Delete");
    let update = request
        .get_field_by_name("update_mask")
        .is_some_and(|f| matches!(f.kind(), Kind::Message(ref k) if k.full_name() == FIELD_MASK));
    let creating = !update && request.fields().any(|f| body_message(cx, &f).is_some());
    let paging = request.get_field_by_name("page_token").is_some();
    let response = m.output();
    let list = paging.then(|| list_field(&response)).flatten();
    let excluded = |path: &str| cx.policy.excluded(m.name(), path);
    let file = m.parent_file();
    let file = file.name();
    let comment = |field: &FieldDescriptor| cx.comments.leading(file, field.path()).to_owned();

    let mut b = Build::default();
    // The kind of resource the command acts on, and the expression of its
    // name, for a refusal.
    let mut target: Option<(String, String)> = None;
    let mut body_updates: Option<Vec<String>> = None;
    let mut one_way: Vec<OneWayUse> = Vec::new();
    // The parent pattern of a list that pages through every zone.
    let mut zonal_parent: Option<String> = None;

    // A first pass for the vocabulary of the help: which flag sets which
    // field.
    for field in request.fields() {
        if excluded(field.name()) {
            continue;
        }
        if let Some(body) = body_message(cx, &field) {
            for f in body.fields() {
                if !excluded(&format!("{}.{}", field.name(), f.name())) {
                    b.vocabulary.insert(f.name().to_owned(), flag_name(cx, &f));
                }
            }
        } else if field.name() == "name" {
            if let Some((t, false)) = reference(cx.pool, &field) {
                if let Some(r) = cx.resources.get(&t) {
                    b.vocabulary
                        .insert("name".to_owned(), leaf_of(&r.pattern).to_kebab_case());
                }
            }
        } else if field.name() != "parent" && field.name() != "update_mask" {
            b.vocabulary
                .insert(field.name().to_owned(), flag_name(cx, &field));
        }
    }
    let vocabulary = b.vocabulary.clone();
    let help = |text: &str| {
        docs::help(
            text,
            &Vocabulary {
                flags: &vocabulary,
                fields: &cx.fields,
            },
        )
    };

    for field in request.fields() {
        let name = field.name().to_owned();
        let fid = ident(&name);
        let required = behaviors(cx.pool, &field).contains(&REQUIRED);

        if (name == "update_mask" && update) || excluded(&name) {
            continue;
        }

        if let Some(body) = body_message(cx, &field) {
            let resource = resource_of(cx, &body)
                .ok_or_else(|| format!("{} declares no resource", body.full_name()))?;
            let body_ty = format!(
                "{}::{}",
                cx.sdk,
                rust_type_path(cx.package, body.full_name())
            );
            let local = ident(&name);
            let mut inits = Vec::new();
            let mut updates = Vec::new();
            for f in body.fields() {
                let path = format!("{name}.{}", f.name());
                let fb = behaviors(cx.pool, &f);
                if fb.contains(&OUTPUT_ONLY) || excluded(&path) {
                    continue;
                }
                if fb.contains(&IDENTIFIER) {
                    if !update {
                        continue;
                    }
                    let leaf = leaf_of(&resource.pattern);
                    let what = resource.singular.to_snake_case().replace('_', " ");
                    let expr =
                        leaf_name(&mut b, &resource, &leaf, &format!("The {what} to change."))?;
                    b.prelude.push(format!("let name = {expr};"));
                    target = Some((resource.singular.clone(), "name".to_owned()));
                    inits.push(format!("{}: name.clone()", ident(f.name())));
                    continue;
                }
                if update && fb.contains(&IMMUTABLE) {
                    continue;
                }
                let k = kind(cx, &f)?;
                let field_required = !update && fb.contains(&REQUIRED);
                if update {
                    updates.push(update_field(&mut b, &f, &k, &local, &help(&comment(&f)))?);
                } else {
                    let expr = value(
                        &mut b,
                        &f,
                        &k,
                        field_required,
                        creating,
                        &help(&comment(&f)),
                    )?;
                    inits.push(init(&ident(f.name()), &expr));
                }
                if let Some(entry) = cx.policy.one_way(m.name(), &path) {
                    one_way.push(one_way_use(
                        &mut b,
                        &f,
                        &k,
                        update || !field_required,
                        entry,
                    )?);
                }
            }
            let rest = if inits.len() < body.fields().count() {
                ", ..::core::default::Default::default()"
            } else {
                ""
            };
            let binding = if update { "let mut" } else { "let" };
            b.prelude.push(format!(
                "{binding} {local} = {body_ty} {{ {}{rest} }};",
                inits.join(", ")
            ));
            b.request.push(format!("{fid}: Some({local})"));
            body_updates = Some(updates);
            continue;
        }

        if let Some((t, child)) = reference(cx.pool, &field) {
            let resource = cx
                .resources
                .get(&t)
                .ok_or_else(|| format!("{name} refers to the unknown type {t}"))?
                .clone();
            if child {
                let pattern = parent_pattern(&resource.pattern);
                if pattern.is_empty() {
                    continue;
                }
                b.add_components(&pattern, None);
                if list.is_some() && variables(&pattern).iter().any(|v| v == "zone") {
                    // Set per zone, once the zones are known.
                    zonal_parent = Some(pattern);
                    b.request.push(format!("{fid}: String::new()"));
                } else {
                    b.prelude
                        .push(format!("let {fid} = names.name({pattern:?}, None)?;"));
                    b.request.push(fid);
                }
                continue;
            }
            if name == "name" {
                let leaf = leaf_of(&resource.pattern);
                let expr = leaf_name(&mut b, &resource, &leaf, &help(&comment(&field)))?;
                target = Some((resource.singular.clone(), "request.name".to_owned()));
                b.prelude.push(format!("let {fid} = {expr};"));
                b.request.push(fid);
                continue;
            }
        }

        let k = kind(cx, &field)?;
        let mut doc = help(&comment(&field));
        if name == "page_size" || name == "page_token" {
            doc.push_str("\n\nWithout `--page-size` or `--page-token`, every page is listed.");
            if zonal_parent.is_some() {
                doc.push_str(" Either needs `--zone`.");
            }
        }
        if name == "request_id" {
            doc.push_str(" One is generated when not given.");
        }
        let expr = value(&mut b, &field, &k, required, creating, &doc)?;
        b.request.push(init(&fid, &expr));
        if let Some(entry) = cx.policy.one_way(m.name(), &name) {
            one_way.push(one_way_use(&mut b, &field, &k, !required, entry)?);
        }
    }

    // The update's statements, and its mask.
    if update {
        let updates = body_updates.ok_or("an update without a resource")?;
        if b.update_ids.is_empty() {
            return Err("an update with no field to change".to_owned());
        }
        b.prelude
            .push("let mut paths: Vec<String> = Vec::new();".to_owned());
        b.prelude.extend(updates);
        b.request.push(
            "update_mask: Some(::origincloud_core::protobuf::FieldMask { paths })".to_owned(),
        );
    }
    if !one_way.is_empty() && (destructive || list.is_some()) {
        return Err("a delete or a list with one-way fields isn't supported".to_owned());
    }

    let request_ty = format!("{}::{}", cx.sdk, request.name());
    let total = request.fields().count();
    let rest = if b.request.len() < total {
        ", ..::core::default::Default::default()"
    } else {
        ""
    };
    let variant = m.name().to_owned();
    let handler = m.name().to_snake_case();
    let args = format!("{variant}Args");
    let what = |singular: &str| singular.to_snake_case().replace('_', " ");

    // The call: the client, and the wrapper the SDK is reached through.
    let output_ty = match &result {
        Some(r) => format!("::origincloud_core::Operation<{}>", type_of(cx, r)),
        None => type_of(cx, &response),
    };
    let mut call = String::new();
    if destructive {
        let (singular, target_expr) = target
            .clone()
            .ok_or_else(|| "a delete without a resource name".to_owned())?;
        let _ = writeln!(
            call,
            "let confirmed = confirm::require(args.confirmation, confirm::Destructive {{ verb: \"delete\", what: {:?}, target: &{target_expr}, consequence: \"It can't be restored.\" }})?;\n\
             let client = ctx.confirmed_client(confirmed)?;\n\
             let response = call_{handler}(&client, request).await.map_err(api_error)?;",
            what(&singular)
        );
    } else if !one_way.is_empty() {
        let (singular, target_expr) = target
            .clone()
            .ok_or_else(|| "a one-way change without a resource name".to_owned())?;
        call.push_str("let mut one_way: Vec<(&str, String, &str)> = Vec::new();\n");
        for u in &one_way {
            if u.optional {
                let _ = writeln!(
                    call,
                    "if let Some(value) = &args.{} {{ one_way.push(({:?}, format!(\"--{} {{value}}\"), {:?})); }}",
                    u.ident, u.verb, u.long, u.consequence
                );
            } else {
                let _ = writeln!(
                    call,
                    "one_way.push(({:?}, format!(\"--{} {{}}\", args.{}), {:?}));",
                    u.verb, u.long, u.ident, u.consequence
                );
            }
        }
        let _ = writeln!(
            call,
            "let response = if one_way.is_empty() {{\n\
                 let client = ctx.client()?;\n\
                 call_{handler}(&client, request).await\n\
             }} else {{\n\
                 let changes: Vec<&str> = one_way.iter().map(|o| o.1.as_str()).collect();\n\
                 let consequences: Vec<&str> = one_way.iter().map(|o| o.2).collect();\n\
                 let confirmed = confirm::require(args.confirmation, confirm::Destructive {{ verb: one_way[0].0, what: {:?}, target: &format!(\"{{}} ({{}})\", {target_expr}, changes.join(\", \")), consequence: &consequences.join(\" \") }})?;\n\
                 let client = ctx.confirmed_client(confirmed)?;\n\
                 call_{handler}_confirmed(&client, request).await\n\
             }}.map_err(api_error)?;",
            what(&singular)
        );
    } else if list.is_none() {
        let _ = writeln!(
            call,
            "let client = ctx.client()?;\nlet response = call_{handler}(&client, request).await.map_err(api_error)?;"
        );
    }

    // What the command prints.
    let mut tail = String::new();
    match (&result, &list) {
        (Some(r), _) => {
            b.flags.push(Flag {
                ident: "no_wait".to_owned(),
                long: "no-wait".to_owned(),
                doc: "Print the operation as soon as the API accepts it, instead of waiting \
                      until it is done."
                    .to_owned(),
                ty: "bool".to_owned(),
                attrs: Vec::new(),
            });
            tail.push_str(
                "let mut operation = response;\nif args.no_wait { return json(operation.raw()); }\n",
            );
            if r.full_name() == EMPTY {
                tail.push_str("wait(&mut operation).await?;\nOk(success())\n");
            } else {
                tail.push_str("let result = wait(&mut operation).await?;\njson(&result)\n");
            }
        }
        (None, Some(items)) => {
            let response_ty = format!("{}::{}", cx.sdk, response.name());
            let field = items.trim_start_matches("r#").to_lower_camel_case();
            let pages = format!(
                "let mut items = Vec::new();\n\
                 for parent in parents {{\n\
                     let mut request = request.clone();\n\
                     request.parent = parent;\n\
                     loop {{\n\
                         let page = call_{handler}(&client, request.clone()).await.map_err(api_error)?;\n\
                         items.extend(page.{items});\n\
                         if page.next_page_token.is_empty() {{ break; }}\n\
                         request.page_token = page.next_page_token;\n\
                     }}\n\
                 }}\n\
                 list_json(&{response_ty} {{ {items}: items, ..::core::default::Default::default() }}, {field:?})"
            );
            match &zonal_parent {
                Some(pattern) => {
                    let _ = writeln!(
                        tail,
                        "let client = ctx.client()?;\n\
                         let paging = args.page_size.is_some() || args.page_token.is_some();\n\
                         let parents: Vec<String> = match args.zone.as_deref() {{\n\
                             Some(_) => vec![names.name({pattern:?}, None)?],\n\
                             None => {{\n\
                                 if paging {{ anyhow::bail!(\"--page-size and --page-token list one page of one zone: they need --zone\"); }}\n\
                                 names.check_without({pattern:?}, \"zone\")?;\n\
                                 let mut parents = Vec::new();\n\
                                 for zone in zones(&client, {service:?}).await? {{\n\
                                     parents.push(names.with_part(\"zone\", &zone).name({pattern:?}, None)?);\n\
                                 }}\n\
                                 parents\n\
                             }}\n\
                         }};\n\
                         if paging {{\n\
                             let mut request = request;\n\
                             request.parent = parents.into_iter().next().unwrap_or_default();\n\
                             let page = call_{handler}(&client, request).await.map_err(api_error)?;\n\
                             return list_json(&page, {field:?});\n\
                         }}\n\
                         {pages}",
                        service = cx.service
                    );
                }
                None => {
                    let _ = writeln!(
                        tail,
                        "let client = ctx.client()?;\n\
                         if args.page_size.is_some() || args.page_token.is_some() {{\n\
                             let page = call_{handler}(&client, request).await.map_err(api_error)?;\n\
                             return list_json(&page, {field:?});\n\
                         }}\n\
                         let parents = [request.parent.clone()];\n\
                         {pages}"
                    );
                }
            }
        }
        (None, None) if response.full_name() == EMPTY => {
            tail.push_str("let _ = response;\nOk(success())\n");
        }
        (None, None) => match items_field(&response).filter(|_| m.name().starts_with("List")) {
            Some(items) => {
                let _ = writeln!(
                    tail,
                    "list_json(&response, {:?})",
                    items.trim_start_matches("r#").to_lower_camel_case()
                );
            }
            None => tail.push_str("json(&response)\n"),
        },
    }

    if destructive || !one_way.is_empty() {
        b.flags.push(Flag {
            ident: "confirmation".to_owned(),
            long: String::new(),
            doc: String::new(),
            ty: "confirm::Confirmation".to_owned(),
            attrs: Vec::new(),
        });
    }

    // The arguments struct.
    let mut code = String::new();
    code.push_str(&docs::attributes(
        &format!("The flags of `{}`.", m.name().to_kebab_case()),
        "",
    ));
    code.push_str("#[derive(clap::Args, Debug)]\n");
    if update {
        let ids: Vec<String> = b.update_ids.iter().map(|i| format!("{i:?}")).collect();
        let _ = writeln!(
            code,
            "#[command(group(clap::ArgGroup::new(\"changes\").required(true).multiple(true).args([{}])))]",
            ids.join(", ")
        );
    }
    let _ = writeln!(code, "pub struct {args} {{");
    for var in &b.components {
        let doc = if var == "zone" && zonal_parent.is_some() {
            "The zone to list, such as `eu-central1-az1`. Without it, every zone that offers \
             the service is listed, in catalog order; the profile's `default-zone` isn't used \
             for lists."
                .to_owned()
        } else {
            component_doc(var)
        };
        Flag {
            ident: ident(var),
            long: var.to_kebab_case(),
            doc,
            ty: "Option<String>".to_owned(),
            attrs: vec![format!("value_name = {:?}", var.to_uppercase())],
        }
        .render(&mut code);
    }
    for flag in &b.flags {
        if flag.ident == "confirmation" {
            code.push_str(
                "    #[command(flatten)]\n    pub confirmation: confirm::Confirmation,\n",
            );
        } else {
            flag.render(&mut code);
        }
    }
    code.push_str("}\n\n");

    // The handler.
    let used = !b.flags.is_empty() || !b.components.is_empty();
    let _ = writeln!(
        code,
        "async fn {handler}(ctx: &Context<'_>, {}: {args}) -> anyhow::Result<Box<dyn SerializableOutput>> {{",
        if used { "args" } else { "_args" }
    );
    let parts: Vec<String> = b
        .components
        .iter()
        .map(|v| format!("({v:?}, args.{}.as_deref())", ident(v)))
        .collect();
    let uses_names = !b.components.is_empty() || !b.learn.is_empty();
    if uses_names {
        let mutable = if b.learn.is_empty() { "" } else { "mut " };
        let _ = writeln!(
            code,
            "let {mutable}names = ctx.names(&[{}]);",
            parts.join(", ")
        );
        for l in &b.learn {
            code.push_str(l);
            code.push('\n');
        }
    }
    for p in &b.prelude {
        code.push_str(p);
        code.push('\n');
    }
    let _ = writeln!(
        code,
        "let request = {request_ty} {{ {}{rest} }};",
        b.request.join(", ")
    );
    code.push_str(&call);
    code.push_str(&tail);
    code.push_str("}\n\n");

    // The wrappers: the only places the SDK's method is called. A delete's
    // takes the confirmed client; a method with one-way fields has one for
    // each client, the unconfirmed one refusing a one-way change.
    let sdk_call = format!(
        "{}::{client}::new(client.sdk()).{handler}(request).await",
        cx.sdk
    );
    let wrapper = |name: &str, client_ty: &str, guard: &str| {
        format!(
            "async fn {name}(client: &{client_ty}, request: {request_ty}) -> ::core::result::Result<{output_ty}, ::origincloud_core::Error> {{\n{guard}{sdk_call}\n}}\n\n"
        )
    };
    if destructive {
        code.push_str(&wrapper(&format!("call_{handler}"), "ConfirmedClient", ""));
    } else if !one_way.is_empty() {
        let mask_paths: Vec<String> = cx
            .policy
            .one_way
            .iter()
            .filter(|o| o.method == m.name())
            .filter_map(|o| o.field.split_once('.').map(|(_, p)| format!("{p:?}")))
            .collect();
        let guard = if update && !mask_paths.is_empty() {
            format!(
                "if names_any(request.update_mask.as_ref(), &[{}]) {{ return Err(::origincloud_core::Error::InvalidRequest(\"this change can't be undone: it needs --yes\".to_owned())); }}\n",
                mask_paths.join(", ")
            )
        } else {
            String::new()
        };
        code.push_str(&wrapper(&format!("call_{handler}"), "ReadClient", &guard));
        code.push_str(&wrapper(
            &format!("call_{handler}_confirmed"),
            "ConfirmedClient",
            "",
        ));
    } else {
        code.push_str(&wrapper(&format!("call_{handler}"), "ReadClient", ""));
    }

    let doc = {
        let text = help(cx.comments.leading(file, m.path()));
        if text.is_empty() {
            format!("Calls `{}`.", m.name())
        } else {
            text
        }
    };
    Ok(Command {
        name: m.name().to_kebab_case(),
        variant,
        handler,
        doc,
        code,
    })
}

/// Records a one-way field the command sets, and says so in its flag's help.
fn one_way_use(
    b: &mut Build,
    field: &FieldDescriptor,
    k: &Kind2,
    optional: bool,
    entry: &crate::policy::OneWay,
) -> Result<OneWayUse, String> {
    if matches!(k, Kind2::Map | Kind2::List) {
        return Err(format!(
            "the one-way field {} is a map or a list, which isn't supported",
            field.name()
        ));
    }
    let id = ident(field.name());
    let flag = b
        .flags
        .iter_mut()
        .find(|f| f.ident == id)
        .ok_or_else(|| format!("the one-way field {} has no flag", field.name()))?;
    flag.doc.push_str(&format!(
        "\n\n{} Setting it needs `--yes`.",
        entry.consequence
    ));
    Ok(OneWayUse {
        ident: id,
        long: flag.long.clone(),
        optional,
        verb: entry.verb.clone(),
        consequence: entry.consequence.clone(),
    })
}

/// The Rust type of a method's output or an operation's result.
fn type_of(cx: &Cx<'_>, message: &MessageDescriptor) -> String {
    if message.full_name() == EMPTY {
        "::origincloud_core::protobuf::Empty".to_owned()
    } else {
        format!(
            "{}::{}",
            cx.sdk,
            rust_type_path(cx.package, message.full_name())
        )
    }
}

/// A field initializer, shortened when the value is a variable of the
/// field's name.
fn init(field: &str, expr: &str) -> String {
    if field == expr {
        field.to_owned()
    } else {
        format!("{field}: {expr}")
    }
}

/// How a flag's value of Rust type `ty` is taken from the arguments: cloned
/// unless it is `Copy`.
fn clone_of(ty: &str) -> &'static str {
    if ty == "String" {
        ".clone()"
    } else {
        ""
    }
}

/// The resource message a request carries, flattened into the command's
/// flags: a field whose type declares a resource.
fn body_message(cx: &Cx<'_>, field: &FieldDescriptor) -> Option<MessageDescriptor> {
    match field.kind() {
        Kind::Message(m)
            if !field.is_list() && !field.is_map() && resource_of(cx, &m).is_some() =>
        {
            Some(m)
        }
        _ => None,
    }
}

fn resource_of(cx: &Cx<'_>, message: &MessageDescriptor) -> Option<Resource> {
    let r = message_extension(cx.pool, &message.options(), "google.api.resource")?;
    let t = r.get_field_by_name("type")?.as_str()?.to_owned();
    cx.resources.get(&t).cloned()
}

/// The flag a field is set by.
fn flag_name(cx: &Cx<'_>, field: &FieldDescriptor) -> String {
    if (field.is_map() || field.is_list()) && reference(cx.pool, field).is_none() {
        singular(field.name()).to_kebab_case()
    } else {
        field.name().to_kebab_case()
    }
}

/// The repeated message field a list response pages through.
fn list_field(response: &MessageDescriptor) -> Option<String> {
    response.get_field_by_name("next_page_token")?;
    items_field(response)
}

/// The repeated message field of a response that lists something.
fn items_field(response: &MessageDescriptor) -> Option<String> {
    response
        .fields()
        .find(|f| f.is_list() && matches!(f.kind(), Kind::Message(_)))
        .map(|f| ident(f.name()))
}

/// Adds the flag of a resource's own name and returns the expression
/// composing it.
fn leaf_name(
    b: &mut Build,
    resource: &Resource,
    leaf: &str,
    comment: &str,
) -> Result<String, String> {
    let fid = ident(leaf);
    let long = leaf.to_kebab_case();
    b.add_components(&resource.pattern, Some(leaf));
    b.add_flag(Flag {
        ident: fid.clone(),
        long: long.clone(),
        doc: reference_doc(comment, &resource.pattern, leaf),
        ty: "String".to_owned(),
        attrs: vec![format!("value_name = {:?}", leaf.to_uppercase())],
    })?;
    b.learn.push(format!(
        "names.learn({:?}, {:?}, Some(args.{fid}.as_str()))?;",
        resource.pattern, long
    ));
    Ok(format!(
        "names.name({:?}, Some(({long:?}, args.{fid}.as_str())))?",
        resource.pattern
    ))
}

/// Adds the flag of a field of a create, or of the request itself, and
/// returns the expression of its value.
fn value(
    b: &mut Build,
    field: &FieldDescriptor,
    k: &Kind2,
    required: bool,
    creating: bool,
    doc: &str,
) -> Result<String, String> {
    let fid = ident(field.name());
    // A boolean a create sets is always stated, `true` or `false`: its
    // default reads the same as leaving it out.
    let bool_required = required || creating;
    let required_attr = |attrs: &mut Vec<String>| {
        if required {
            attrs.push("required = true".to_owned());
        }
    };
    let long = field.name().to_kebab_case();
    Ok(match k {
        Kind2::Scalar(ty) => {
            b.add_flag(Flag {
                ident: fid.clone(),
                long,
                doc: doc.to_owned(),
                ty: if required {
                    (*ty).to_owned()
                } else {
                    format!("Option<{ty}>")
                },
                attrs: Vec::new(),
            })?;
            if required {
                format!("args.{fid}{}", clone_of(ty))
            } else {
                format!("args.{fid}{}.unwrap_or_default()", clone_of(ty))
            }
        }
        Kind2::Bool => {
            b.add_flag(Flag {
                ident: fid.clone(),
                long,
                doc: doc.to_owned(),
                ty: if bool_required {
                    "bool"
                } else {
                    "Option<bool>"
                }
                .to_owned(),
                attrs: [
                    "value_name = \"true|false\"",
                    "action = clap::ArgAction::Set",
                    "required = true",
                ]
                .into_iter()
                .take(if bool_required { 3 } else { 2 })
                .map(str::to_owned)
                .collect(),
            })?;
            if bool_required {
                format!("args.{fid}")
            } else {
                format!("args.{fid}.unwrap_or_default()")
            }
        }
        Kind2::Enum(path, values) => {
            let quoted: Vec<String> = values.iter().map(|v| format!("{v:?}")).collect();
            b.add_flag(Flag {
                ident: fid.clone(),
                long,
                doc: doc.to_owned(),
                ty: if required { "String" } else { "Option<String>" }.to_owned(),
                attrs: vec![format!("value_parser = [{}]", quoted.join(", "))],
            })?;
            if required {
                format!("{path}::from_str_name(&args.{fid}).map_or(0, |v| v as i32)")
            } else {
                format!(
                    "args.{fid}.as_deref().and_then({path}::from_str_name).map_or(0, |v| v as i32)"
                )
            }
        }
        Kind2::Map => {
            let one = ident(&singular(field.name()));
            let mut attrs = vec!["value_name = \"KEY=VALUE\"".to_owned()];
            required_attr(&mut attrs);
            b.add_flag(Flag {
                ident: one.clone(),
                long: singular(field.name()).to_kebab_case(),
                doc: format!("{doc}\n\nAs KEY=VALUE; repeat for more."),
                ty: "Vec<TagArg>".to_owned(),
                attrs,
            })?;
            format!("tags(&args.{one})")
        }
        Kind2::List => {
            let one = ident(&singular(field.name()));
            let mut attrs = Vec::new();
            required_attr(&mut attrs);
            b.add_flag(Flag {
                ident: one.clone(),
                long: singular(field.name()).to_kebab_case(),
                doc: format!("{doc}\n\nRepeat for more."),
                ty: "Vec<String>".to_owned(),
                attrs,
            })?;
            format!("args.{one}.clone()")
        }
        Kind2::Reference(resource) => {
            let leaf = leaf_of(&resource.pattern);
            b.add_components(&resource.pattern, Some(&leaf));
            b.add_flag(Flag {
                ident: fid.clone(),
                long: long.clone(),
                doc: reference_doc(doc, &resource.pattern, &leaf),
                ty: if required { "String" } else { "Option<String>" }.to_owned(),
                attrs: vec![format!("value_name = {:?}", leaf.to_uppercase())],
            })?;
            if required {
                b.learn.push(format!(
                    "names.learn({:?}, {long:?}, Some(args.{fid}.as_str()))?;",
                    resource.pattern
                ));
                b.prelude.push(format!(
                    "let {fid} = names.name({:?}, Some(({long:?}, args.{fid}.as_str())))?;",
                    resource.pattern
                ));
            } else {
                b.learn.push(format!(
                    "names.learn({:?}, {long:?}, args.{fid}.as_deref())?;",
                    resource.pattern
                ));
                b.prelude.push(format!(
                    "let {fid} = match args.{fid}.as_deref() {{ Some(v) => names.name({:?}, Some(({long:?}, v)))?, None => String::new() }};",
                    resource.pattern
                ));
            }
            fid
        }
    })
}

/// Adds the flag of a field an update may change, and returns the statement
/// that sets it on the resource `body` and adds it to the mask when given.
fn update_field(
    b: &mut Build,
    field: &FieldDescriptor,
    k: &Kind2,
    body: &str,
    comment: &str,
) -> Result<String, String> {
    let name = field.name();
    let fid = ident(name);
    let long = name.to_kebab_case();
    let set = |expr: &str| format!("{body}.{fid} = {expr}; paths.push({name:?}.to_owned());");
    let kept = "\n\nLeft as it is when not given.";
    Ok(match k {
        Kind2::Scalar(ty) => {
            b.add_flag(Flag {
                ident: fid.clone(),
                long,
                doc: format!("{comment}{kept}"),
                ty: format!("Option<{ty}>"),
                attrs: Vec::new(),
            })?;
            b.update_ids.push(arg_id(&fid).to_owned());
            format!(
                "if let Some(value) = args.{fid}{} {{ {} }}",
                clone_of(ty),
                set("value")
            )
        }
        Kind2::Bool => {
            b.add_flag(Flag {
                ident: fid.clone(),
                long,
                doc: format!("{comment}{kept}"),
                ty: "Option<bool>".to_owned(),
                attrs: vec![
                    "value_name = \"true|false\"".to_owned(),
                    "action = clap::ArgAction::Set".to_owned(),
                ],
            })?;
            b.update_ids.push(arg_id(&fid).to_owned());
            format!("if let Some(value) = args.{fid} {{ {} }}", set("value"))
        }
        Kind2::Enum(path, values) => {
            let quoted: Vec<String> = values.iter().map(|v| format!("{v:?}")).collect();
            b.add_flag(Flag {
                ident: fid.clone(),
                long,
                doc: format!("{comment}{kept}"),
                ty: "Option<String>".to_owned(),
                attrs: vec![format!("value_parser = [{}]", quoted.join(", "))],
            })?;
            b.update_ids.push(arg_id(&fid).to_owned());
            format!(
                "if let Some(value) = args.{fid}.as_deref() {{ {} }}",
                set(&format!(
                    "{path}::from_str_name(value).map_or(0, |v| v as i32)"
                ))
            )
        }
        Kind2::Map | Kind2::List => {
            let is_map = matches!(k, Kind2::Map);
            let one = ident(&singular(name));
            let none = format!("no_{}", name.to_snake_case());
            b.add_flag(Flag {
                ident: one.clone(),
                long: singular(name).to_kebab_case(),
                doc: format!(
                    "{comment}\n\n{} Replaces all of them; `--no-{long}` removes them all. Left as they are when neither is given.",
                    if is_map { "As KEY=VALUE; repeat for more." } else { "Repeat for more." }
                ),
                ty: if is_map { "Vec<TagArg>" } else { "Vec<String>" }.to_owned(),
                attrs: if is_map {
                    vec!["value_name = \"KEY=VALUE\"".to_owned()]
                } else {
                    Vec::new()
                },
            })?;
            b.add_flag(Flag {
                ident: none.clone(),
                long: format!("no-{long}"),
                doc: format!("Remove all of its {}.", name.replace('_', " ")),
                ty: "bool".to_owned(),
                attrs: vec![format!("conflicts_with = {:?}", arg_id(&one))],
            })?;
            b.update_ids.push(arg_id(&one).to_owned());
            b.update_ids.push(none.clone());
            let expr = if is_map {
                format!("tags(&args.{one})")
            } else {
                format!("args.{one}.clone()")
            };
            format!(
                "if args.{none} || !args.{one}.is_empty() {{ {} }}",
                set(&expr)
            )
        }
        Kind2::Reference(resource) => {
            let leaf = leaf_of(&resource.pattern);
            b.add_components(&resource.pattern, Some(&leaf));
            b.add_flag(Flag {
                ident: fid.clone(),
                long: long.clone(),
                doc: format!("{}{kept}", reference_doc(comment, &resource.pattern, &leaf)),
                ty: "Option<String>".to_owned(),
                attrs: vec![format!("value_name = {:?}", leaf.to_uppercase())],
            })?;
            b.update_ids.push(arg_id(&fid).to_owned());
            b.learn.push(format!(
                "names.learn({:?}, {long:?}, args.{fid}.as_deref())?;",
                resource.pattern
            ));
            format!(
                "if let Some(value) = args.{fid}.as_deref() {{ {} }}",
                set(&format!(
                    "names.name({:?}, Some(({long:?}, value)))?",
                    resource.pattern
                ))
            )
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_patterns() {
        assert_eq!(
            variables("projects/{project}/zones/{zone}/volumes/{volume}"),
            ["project", "zone", "volume"]
        );
        assert_eq!(
            parent_pattern("projects/{project}/zones/{zone}/volumes/{volume}"),
            "projects/{project}/zones/{zone}"
        );
        assert_eq!(parent_pattern("storageClasses/{storage_class}"), "");
        assert_eq!(singular("tags"), "tag");
        assert_eq!(singular("addresses"), "address");
        assert_eq!(singular("policies"), "policy");
        assert_eq!(singular("boxes"), "box");
        assert_eq!(singular("names"), "name");
        assert_eq!(singular("status"), "status");
        assert_eq!(singular("data"), "data");
        assert_eq!(ident("type"), "r#type");
        assert_eq!(arg_id("r#type"), "type");
    }
}
