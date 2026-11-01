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
//! - A name field without a resource reference takes its pattern from the
//!   method's HTTP binding, each `*` named after the collection before it:
//!   `{name=projects/*/zones/*/operations/*}` is
//!   `projects/{project}/zones/{zone}/operations/{operation}`, given as
//!   `--operation`.
//! - A duration is given as `30s`, `2m` or `1m30s`.
//! - The methods of the operations package (`origincloud.longrunning`) are
//!   commands of every group whose methods answer with operations, sent to
//!   that group's host, which serves its operations. A method answering
//!   with the operation as it is after waiting up to its `timeout` is
//!   called again until the operation is done, or until `--timeout`, a
//!   deadline of the command's, passes; it exits with status 1 when the
//!   operation failed or isn't done.
//! - A name field without a resource reference takes its help from its
//!   pattern, not from its comment, which describes it as one string.

use std::cell::RefCell;
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
const DURATION: &str = "google.protobuf.Duration";

/// The service whose package holds every service's operation methods:
/// `origincloud.longrunning.{version}`. Each service serves them at its own
/// host, for the operations its methods answer with.
pub const OPERATIONS: &str = "longrunning";

/// The Rust type of a duration flag, and what its help says of its form.
const DURATION_TYPE: &str = "::origincloud_core::protobuf::Duration";
const DURATION_FORMAT: &str = "\n\nSuch as `30s` or `1m`.";

// `google.api.FieldBehavior` values.
const REQUIRED: i32 = 2;
const UNORDERED_LIST: i32 = 6;
const OUTPUT_ONLY: i32 = 3;
const IMMUTABLE: i32 = 5;
const IDENTIFIER: i32 = 8;

/// The generated commands of one group, and the methods left without one.
pub struct Generated {
    pub code: String,
    pub skipped: Vec<String>,
    /// Whether the group has the operation methods' commands.
    pub operations: bool,
    /// Whether a list of the group looks up the zones, at the location
    /// service, when given no `--zone`.
    pub lists_zones: bool,
}

/// What every method's generation shares.
struct Cx<'a> {
    pool: &'a DescriptorPool,
    comments: &'a Comments,
    policy: &'a Policy,
    /// The package whose methods these are.
    package: &'a str,
    /// The service the package names, as `commands.toml` does: `pvs`, or
    /// `longrunning` for the operation methods.
    own: &'a str,
    /// The group the commands are in, the service's name in its host and in
    /// a zone's `services`: `pvs`.
    service: &'a str,
    /// The SDK module of the package: `::origincloud_pvs::v1beta1`.
    sdk: String,
    /// For the operation methods, the expression of the group's endpoint,
    /// which serves its operations: the `&Service` of one of its clients.
    host: Option<String>,
    /// The functions parsing nested messages' `KEY=VALUE,...` forms, by
    /// name, each written once in the group's code.
    helpers: &'a RefCell<BTreeMap<String, String>>,
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
    /// Whether, given no `--zone`, it lists every zone the location service
    /// says offers the service.
    lists_zones: bool,
}

/// The service a package names: `pvs` for `origincloud.pvs.v1beta1`.
fn service_of(package: &str) -> &str {
    package.split('.').nth(1).unwrap_or_default()
}

/// The Rust SDK module of a package: `::origincloud_pvs::v1beta1`, and
/// `::origincloud_core::longrunning::v1beta1` for the operations package,
/// which the core crate holds.
fn sdk_module(package: &str) -> String {
    let version = package.rsplit('.').next().unwrap_or_default();
    let service = service_of(package);
    if service == OPERATIONS {
        format!("::origincloud_core::{service}::{version}")
    } else {
        format!("::origincloud_{service}::{version}")
    }
}

/// The client type of a service: `VolumeClient` for `VolumeService`.
fn client_name(service: &prost_reflect::ServiceDescriptor) -> String {
    format!(
        "{}Client",
        service
            .name()
            .strip_suffix("Service")
            .unwrap_or(service.name())
    )
}

/// The services of `package`, in file order.
fn services_of(pool: &DescriptorPool, package: &str) -> Vec<prost_reflect::ServiceDescriptor> {
    let mut files: Vec<_> = pool
        .files()
        .filter(|f| f.package_name() == package)
        .collect();
    files.sort_by(|a, b| a.name().cmp(b.name()));
    files.iter().flat_map(|f| f.services()).collect()
}

/// The commands of the group of `package`: one per method of its services,
/// and, when one of them answers with an operation, one per method of the
/// operations package `operations`.
pub fn generate(
    pool: &DescriptorPool,
    comments: &Comments,
    policy: &Policy,
    package: &str,
    operations: Option<&str>,
) -> Result<Generated> {
    let service = service_of(package);
    let fields = |package: &str| -> BTreeSet<String> {
        pool.all_messages()
            .filter(|m| m.package_name() == package)
            .flat_map(|m| m.fields().map(|f| f.name().to_owned()).collect::<Vec<_>>())
            .collect()
    };
    let helpers = RefCell::new(BTreeMap::new());
    let cx = Cx {
        pool,
        comments,
        policy,
        package,
        own: service,
        service,
        sdk: sdk_module(package),
        host: None,
        helpers: &helpers,
        resources: resources(pool),
        fields: fields(package),
    };

    let mut commands: Vec<Command> = Vec::new();
    let mut skipped = Vec::new();
    let services = services_of(pool, package);
    for s in &services {
        for method in s.methods() {
            if policy.no_command(service, method.name()) {
                continue;
            }
            match command(&cx, &method) {
                Ok(command) => commands.push(command),
                Err(why) => skipped.push(format!("{}: {why}", method.full_name())),
            }
        }
    }

    // The operation methods, at the host of the group's first service.
    let answers_with_operations = services
        .iter()
        .flat_map(|s| s.methods())
        .any(|m| operation_result(pool, &m).is_some());
    let mut with_operations = false;
    if let (true, Some(operations)) = (answers_with_operations, operations) {
        let ops = Cx {
            package: operations,
            own: OPERATIONS,
            sdk: sdk_module(operations),
            host: Some("&service(client.sdk())".to_owned()),
            fields: fields(operations),
            ..cx
        };
        for s in services_of(pool, operations) {
            for method in s.methods() {
                if policy.no_command(OPERATIONS, method.name()) {
                    continue;
                }
                let name = method.name().to_kebab_case();
                if commands.iter().any(|c| c.name == name) {
                    skipped.push(format!(
                        "{}: the service has a method of its own named {}",
                        method.full_name(),
                        method.name()
                    ));
                    continue;
                }
                match command(&ops, &method) {
                    Ok(command) => {
                        commands.push(command);
                        with_operations = true;
                    }
                    Err(why) => skipped.push(format!("{}: {why}", method.full_name())),
                }
            }
        }
    }

    let mut code = String::new();
    code.push_str(
        "#[allow(unused_imports)]\n\
         use crate::sdk::runtime::{\n\
             api_error, deadline_after, duration, json, list_json, names_any, pace, passed, success,\n\
             tags, wait, wait_for, waited, waiting, zones, ConfirmedClient, Context, ReadClient,\n\
         };\n\
         #[allow(unused_imports)]\n\
         use crate::confirm;\n\
         #[allow(unused_imports)]\n\
         use crate::handler::tag_args::TagArg;\n\
         use crate::output::SerializableOutput;\n\n",
    );
    code.push_str(&docs::attributes(
        &format!("The commands of every method of `{package}`."),
        "",
    ));
    code.push_str("#[derive(clap::Subcommand, Debug)]\npub enum Commands {\n");
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
        "pub async fn run(ctx: &Context<'_>, command: Commands) -> anyhow::Result<Box<dyn SerializableOutput>> {\n    match command {\n",
    );
    for c in &commands {
        writeln!(
            code,
            "        Commands::{}(args) => {}(ctx, args).await,",
            c.variant, c.handler
        )?;
    }
    code.push_str("    }\n}\n\n");
    // The group's endpoint: that of its first service's client, which every
    // client of the package shares, and which serves its operations.
    if let Some(first) = services.first() {
        code.push_str(&docs::attributes(
            "The endpoint the group's calls go to, which also serves its operations.",
            "",
        ));
        writeln!(
            code,
            "pub(crate) fn service(client: &::origincloud_core::Client) -> ::origincloud_core::Service {{ {}::{}::new(client).service().clone() }}\n",
            cx.sdk,
            client_name(first)
        )?;
    }
    for c in &commands {
        code.push_str(&c.code);
        code.push('\n');
    }
    for helper in helpers.borrow().values() {
        code.push_str(helper);
        code.push('\n');
    }
    // What the flags of nested messages use, for a group that has some:
    // every message field has a JSON flag.
    if code.contains("JsonArg<") {
        code.push_str(
            "#[allow(unused_imports)]\n\
             use crate::sdk::runtime::{\n\
                 json_arg, key_bool, key_duration, key_enum, key_scalar, key_values, required_keys,\n\
                 unknown_key, JsonArg,\n\
             };\n",
        );
    }

    Ok(Generated {
        code: unparse(&code)?,
        skipped,
        operations: with_operations,
        lists_zones: commands.iter().any(|c| c.lists_zones),
    })
}

/// `code`, formatted, once it parses.
fn unparse(code: &str) -> Result<String> {
    let file = syn::parse_file(code)
        .map_err(|e| anyhow::anyhow!("the generated code doesn't parse: {e}\n{code}"))?;
    Ok(prettyplease::unparse(&file))
}

/// A command group: `origin {service}`.
pub struct Group {
    pub service: String,
    /// Its line in `origin --help`.
    pub about: String,
    /// What `origin {service} --help` says below it.
    pub details: String,
    /// The host its calls go to, such as `pvs.origincloudapis.eu`.
    pub host: String,
    /// Whether, with a region, its calls go to the region's host,
    /// `pvs.{region}.origincloudapis.eu`, as the SDK's clients say.
    pub regional: bool,
    /// Whether a list of it looks up the zones at the location service.
    pub lists_zones: bool,
    /// Whether it is a service of the tests only, compiled with the
    /// `test-services` feature alone.
    pub test: bool,
}

/// The group of `package`: its help from `commands.toml`, or from the
/// comment of its only service.
pub fn group(
    pool: &DescriptorPool,
    comments: &Comments,
    policy: &Policy,
    package: &str,
    regional: bool,
    lists_zones: bool,
    test: bool,
) -> Group {
    let service = service_of(package);
    let services = services_of(pool, package);
    let host = services
        .iter()
        .find_map(|s| {
            let ext = pool.get_extension_by_name("google.api.default_host")?;
            let options = s.options();
            options
                .has_extension(&ext)
                .then(|| options.get_extension(&ext).as_str().map(str::to_owned))
                .flatten()
        })
        .unwrap_or_default();
    let (about, details) = match (policy.group(service), services.as_slice()) {
        (Some(g), _) => (g.about.clone(), g.details.clone()),
        (None, [only]) => {
            let fields = BTreeMap::new();
            let names = BTreeSet::new();
            let text = docs::help(
                comments.leading(only.parent_file().name(), only.path()),
                &Vocabulary {
                    flags: &fields,
                    fields: &names,
                },
            );
            let mut paragraphs = text.splitn(2, "\n\n");
            let first = paragraphs.next().unwrap_or_default();
            let summary = docs::summary(first);
            let joined = first.split_whitespace().collect::<Vec<_>>().join(" ");
            let rest_of_first = joined
                .strip_prefix(&summary)
                .unwrap_or_default()
                .trim_start_matches('.')
                .trim()
                .to_owned();
            let mut details: Vec<String> = Vec::new();
            if !rest_of_first.is_empty() {
                details.push(rest_of_first);
            }
            details.extend(paragraphs.next().map(str::to_owned));
            (summary, details.join("\n\n"))
        }
        _ => (format!("The {service} service"), String::new()),
    };
    Group {
        service: service.to_owned(),
        about: about.trim_end_matches('.').to_owned(),
        details,
        host,
        regional,
        lists_zones,
        test,
    }
}

/// The environment variable of a group's `--endpoint`:
/// `ORIGINCLOUD_PVS_ENDPOINT`. Each group has its own, so that redirecting
/// one service's calls leaves the others' alone.
fn endpoint_variable(service: &str) -> String {
    format!(
        "ORIGINCLOUD_{}_ENDPOINT",
        service.to_uppercase().replace('-', "_")
    )
}

/// The help of a group's `--endpoint`.
fn endpoint_doc(g: &Group, groups: &[Group]) -> String {
    let mut doc = String::from("Base URL to send the calls to, such as a test server's");
    if g.host.is_empty() {
        doc.push_str(", instead of the service's own host.");
    } else if g.regional {
        let (label, rest) = g.host.split_once('.').unwrap_or((&g.host, ""));
        let _ = write!(
            doc,
            ", instead of the service's own host: `https://{}`, or `https://{label}.{{region}}.{rest}` \
             when the profile names a `region`.",
            g.host
        );
    } else {
        let _ = write!(
            doc,
            ", instead of the service's own host, `https://{}`, the same in every region.",
            g.host
        );
    }
    doc.push_str(
        " It must be `https://`, or `http://` to this machine (`localhost`, `127.0.0.1`, \
         `[::1]`).",
    );
    if g.lists_zones {
        if let Some(location) = groups.iter().find(|l| l.service == "location") {
            let _ = write!(
                doc,
                " A list without `--zone` looks up the zones at the location service, not here; \
                 `{}` sends that lookup elsewhere.",
                endpoint_variable(&location.service)
            );
        }
    }
    doc
}

/// The groups, as subcommands of `origin` with their `--endpoint`, what the
/// runtime knows of each, and the code that runs a command of one.
pub fn services(groups: &[Group]) -> Result<String> {
    let mut code = String::new();
    code.push_str(
        "use crate::config::ClientProfile;\n\
         use crate::output::SerializableOutput;\n\
         use crate::sdk::runtime::{Context, Group};\n\n",
    );
    // A test service's group is compiled with the `test-services` feature
    // alone.
    let gate = |g: &Group| {
        if g.test {
            "#[cfg(feature = \"test-services\")]\n"
        } else {
            ""
        }
    };
    for g in groups {
        code.push_str(&docs::attributes(&format!("`origin {}`.", g.service), ""));
        code.push_str(gate(g));
        // A service's methods may all end alike (`GetRoute`, `UpdateRoute`).
        code.push_str("#[allow(clippy::enum_variant_names)]\n");
        writeln!(
            code,
            "pub mod {} {{ include!({:?}); }}",
            ident(&g.service),
            format!("{}.rs", g.service)
        )?;
        code.push_str(&docs::attributes(
            &format!("What the runtime knows of `origin {}`.", g.service),
            "",
        ));
        code.push_str(gate(g));
        writeln!(
            code,
            "pub(crate) static {}: Group = Group {{ name: {:?}, endpoint_variable: {:?}, regional: {}, service: {}::service }};",
            g.service.to_uppercase().replace('-', "_"),
            g.service,
            endpoint_variable(&g.service),
            g.regional,
            ident(&g.service)
        )?;
    }
    let listed = |with_tests: bool| -> Vec<String> {
        groups
            .iter()
            .filter(|g| with_tests || !g.test)
            .map(|g| format!("&{}", g.service.to_uppercase().replace('-', "_")))
            .collect()
    };
    if groups.iter().any(|g| g.test) {
        for (gate, with_tests) in [
            ("not(feature = \"test-services\")", false),
            ("feature = \"test-services\"", true),
        ] {
            code.push_str(&docs::attributes("Every generated group.", ""));
            writeln!(
                code,
                "#[cfg({gate})]\n#[allow(dead_code)]\npub(crate) static GROUPS: &[&Group] = &[{}];",
                listed(with_tests).join(", ")
            )?;
        }
    } else {
        code.push_str(&docs::attributes("Every generated group.", ""));
        writeln!(
            code,
            "#[allow(dead_code)]\npub(crate) static GROUPS: &[&Group] = &[{}];",
            listed(false).join(", ")
        )?;
    }
    code.push_str(&docs::attributes(
        "The command groups generated from the API definitions, one per service.",
        "",
    ));
    code.push_str("#[derive(clap::Subcommand, Debug)]\npub enum ServiceCommands {\n");
    for g in groups {
        let variant = g.service.to_upper_camel_case();
        let long = if g.details.is_empty() {
            format!("{}.", g.about)
        } else {
            format!("{}.\n\n{}", g.about, g.details)
        };
        let long = long
            .split("\n\n")
            .map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect::<Vec<_>>()
            .join("\n\n");
        if g.test {
            code.push_str("    #[cfg(feature = \"test-services\")]\n");
        }
        writeln!(
            code,
            "    #[command(name = {:?}, about = {:?}, long_about = {:?})]\n    {variant}({variant}Cli),",
            g.service, g.about, long
        )?;
    }
    code.push_str("}\n\n");
    for g in groups {
        let variant = g.service.to_upper_camel_case();
        code.push_str(&docs::attributes(
            &format!("`origin {}`: its options, and the command.", g.service),
            "",
        ));
        code.push_str(gate(g));
        writeln!(
            code,
            "#[derive(clap::Args, Debug)]\npub struct {variant}Cli {{"
        )?;
        code.push_str(&docs::attributes(&endpoint_doc(g, groups), "    "));
        writeln!(
            code,
            "    #[arg(long, value_name = \"URL\", env = {:?}, global = true, help_heading = \"Global options\")]\n    pub endpoint: Option<String>,\n    #[command(subcommand)]\n    pub command: {}::Commands,\n}}\n",
            endpoint_variable(&g.service),
            ident(&g.service)
        )?;
    }
    code.push_str(&docs::attributes("Runs `command`.", ""));
    code.push_str(
        "pub async fn run(profile: &ClientProfile, command: ServiceCommands) -> anyhow::Result<Box<dyn SerializableOutput>> {\n    match command {\n",
    );
    for g in groups {
        if g.test {
            code.push_str("        #[cfg(feature = \"test-services\")]\n");
        }
        writeln!(
            code,
            "        ServiceCommands::{}(cli) => {{ let ctx = Context::new(profile, &{}, cli.endpoint.as_deref())?; {}::run(&ctx, cli.command).await }}",
            g.service.to_upper_camel_case(),
            g.service.to_uppercase().replace('-', "_"),
            ident(&g.service)
        )?;
    }
    code.push_str("    }\n}\n");
    unparse(&code)
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

/// The resource a request's `name` field names: the one its reference
/// declares, or the one the method's HTTP binding implies.
fn named_resource(cx: &Cx<'_>, m: &MethodDescriptor, field: &FieldDescriptor) -> Option<Resource> {
    match reference(cx.pool, field) {
        Some((t, false)) => cx.resources.get(&t).cloned(),
        Some((_, true)) => None,
        None => http_resource(cx.pool, m, field.name()),
    }
}

/// The resource the HTTP binding of `m` implies for its path field
/// `field`, which declares no resource type: in
/// `/v1beta1/{name=projects/*/zones/*/operations/*}:wait`, each `*` is
/// named after the collection before it, in the singular, as
/// `projects/{project}/zones/{zone}/operations/{operation}`. `None` when
/// the binding doesn't have that shape.
fn http_resource(pool: &DescriptorPool, m: &MethodDescriptor, field: &str) -> Option<Resource> {
    let rule = message_extension(pool, &m.options(), "google.api.http")?;
    let path = ["get", "put", "post", "delete", "patch"]
        .iter()
        .filter(|verb| rule.has_field_by_name(verb))
        .find_map(|verb| rule.get_field_by_name(verb)?.as_str().map(str::to_owned))?;
    let template = path
        .split_once(&format!("{{{field}="))?
        .1
        .split_once('}')?
        .0;
    let pattern = pattern_of_template(template)?;
    let singular = leaf_of(&pattern);
    Some(Resource { pattern, singular })
}

/// `projects/*/zones/*` as `projects/{project}/zones/{zone}`; `None` for a
/// template that isn't collections each followed by one `*`.
fn pattern_of_template(template: &str) -> Option<String> {
    let segments: Vec<&str> = template.split('/').collect();
    if !segments.len().is_multiple_of(2) {
        return None;
    }
    let mut out = Vec::new();
    for pair in segments.chunks(2) {
        let (collection, star) = (pair[0], pair[1]);
        let literal = !collection.is_empty()
            && collection
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_');
        if !literal || star != "*" {
            return None;
        }
        out.push(collection.to_owned());
        out.push(format!("{{{}}}", singular(collection).to_snake_case()));
    }
    Some(out.join("/"))
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
    /// A repeated scalar, enum or resource name, as `--{singular} VALUE`,
    /// repeated.
    List(Elem),
    /// A resource name, from its id or whole name.
    Reference(Resource),
    /// `google.protobuf.Duration`, as `30s`, `2m` or `1m30s`.
    Duration,
    /// A nested message: `--{field} KEY=VALUE,...` and `--{field}-json`.
    Message(Nested),
    /// A repeated nested message: `--{singular} KEY=VALUE,...`, repeated,
    /// and `--{field}-json`.
    Messages(Nested),
}

/// An element of a repeated scalar field.
#[derive(Clone)]
enum Elem {
    Scalar(&'static str),
    Enum(String, Vec<String>),
    Reference(Resource),
}

/// A nested message's flags: the keys of its `KEY=VALUE,...` form, and its
/// JSON form, which every message field has.
#[derive(Clone)]
struct Nested {
    /// The message's full name, by which the JSON flag checks its value.
    full_name: String,
    /// The message's Rust type.
    rust: String,
    /// The keys, in field order.
    keys: Vec<Key>,
    /// The fields only the JSON form sets: messages, lists and maps.
    json_only: Vec<String>,
    /// The function parsing the `KEY=VALUE,...` form, when there is one:
    /// when a field can be a key and none that can't is required.
    parser: Option<String>,
}

/// One key of a nested message's `KEY=VALUE,...` form.
#[derive(Clone)]
struct Key {
    /// The field's name, which is the key.
    name: String,
    /// The field's Rust name.
    ident: String,
    kind: Kind2,
    required: bool,
    /// The field's comment.
    comment: String,
}

/// The Rust type of a number or a string.
fn scalar_type(kind: &Kind) -> Option<&'static str> {
    Some(match kind {
        Kind::String => "String",
        Kind::Int32 | Kind::Sint32 | Kind::Sfixed32 => "i32",
        Kind::Int64 | Kind::Sint64 | Kind::Sfixed64 => "i64",
        Kind::Uint32 | Kind::Fixed32 => "u32",
        Kind::Uint64 | Kind::Fixed64 => "u64",
        Kind::Float => "f32",
        Kind::Double => "f64",
        _ => return None,
    })
}

/// The Rust path of a type of package `package` in the SDK: its package's
/// module, then its path there.
fn sdk_type(package: &str, full_name: &str) -> Result<String, String> {
    if package == "google.protobuf" {
        let name = full_name.rsplit('.').next().unwrap_or_default();
        return Ok(format!("::origincloud_core::protobuf::{name}"));
    }
    if !package.starts_with("origincloud.") {
        return Err(format!(
            "{full_name} is in the package {package}, which the SDK has no module for"
        ));
    }
    Ok(format!(
        "{}::{}",
        sdk_module(package),
        rust_type_path(package, full_name)
    ))
}

fn enum_kind(e: &prost_reflect::EnumDescriptor) -> Result<(String, Vec<String>), String> {
    let values = e
        .values()
        .filter(|v| v.number() != 0)
        .map(|v| v.name().to_owned())
        .collect();
    Ok((sdk_type(e.package_name(), e.full_name())?, values))
}

fn kind(cx: &Cx<'_>, field: &FieldDescriptor) -> Result<Kind2, String> {
    if field.containing_oneof().is_some() {
        return Err(format!(
            "the field {} is in a oneof, or has explicit presence",
            field.name()
        ));
    }
    let resource = |t: &str| {
        cx.resources
            .get(t)
            .cloned()
            .ok_or_else(|| format!("the field {} refers to the unknown type {t}", field.name()))
    };
    let referenced = match reference(cx.pool, field) {
        Some((t, false)) => Some(resource(&t)?),
        _ => None,
    };
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
        if let Some(r) = referenced {
            return Ok(Kind2::List(Elem::Reference(r)));
        }
        return match field.kind() {
            Kind::Message(m) => Ok(Kind2::Messages(nested(cx, &m)?)),
            Kind::Enum(e) => {
                let (path, values) = enum_kind(&e)?;
                Ok(Kind2::List(Elem::Enum(path, values)))
            }
            other => scalar_type(&other)
                .map(|t| Kind2::List(Elem::Scalar(t)))
                .ok_or_else(|| format!("the list {} is a list of {other:?}", field.name())),
        };
    }
    if let Some(r) = referenced {
        return Ok(Kind2::Reference(r));
    }
    Ok(match field.kind() {
        Kind::Bool => Kind2::Bool,
        Kind::Message(m) if m.full_name() == DURATION => Kind2::Duration,
        Kind::Message(m) => Kind2::Message(nested(cx, &m)?),
        Kind::Enum(e) => {
            let (path, values) = enum_kind(&e)?;
            Kind2::Enum(path, values)
        }
        other => Kind2::Scalar(
            scalar_type(&other)
                .ok_or_else(|| format!("the field {} is a {other:?}", field.name()))?,
        ),
    })
}

/// The kind of a nested message's field as a key of its `KEY=VALUE,...`
/// form: a number, a string, a boolean, an enum, a duration or a resource
/// name; `None` for what only its JSON form sets.
fn key_kind(cx: &Cx<'_>, field: &FieldDescriptor) -> Option<Kind2> {
    if field.is_list() || field.is_map() || field.containing_oneof().is_some() {
        return None;
    }
    if let Some((t, false)) = reference(cx.pool, field) {
        return cx.resources.get(&t).cloned().map(Kind2::Reference);
    }
    match field.kind() {
        Kind::Bool => Some(Kind2::Bool),
        Kind::Message(m) if m.full_name() == DURATION => Some(Kind2::Duration),
        Kind::Message(_) => None,
        Kind::Enum(e) => enum_kind(&e).ok().map(|(p, v)| Kind2::Enum(p, v)),
        other => scalar_type(&other).map(Kind2::Scalar),
    }
}

/// A nested message's keys, and the function parsing its `KEY=VALUE,...`
/// form, which is added to the group's code once.
fn nested(cx: &Cx<'_>, m: &MessageDescriptor) -> Result<Nested, String> {
    let rust = sdk_type(m.package_name(), m.full_name())?;
    let mut keys = Vec::new();
    let mut json_only = Vec::new();
    let mut required = Vec::new();
    let mut required_json_only = false;
    // The well-known types (a timestamp, a struct) have no keys: their JSON
    // is their one form.
    if !m.package_name().starts_with("google.") {
        for f in m.fields() {
            let fb = behaviors(cx.pool, &f);
            if fb.contains(&OUTPUT_ONLY) {
                continue;
            }
            match key_kind(cx, &f) {
                Some(kind) => {
                    if fb.contains(&REQUIRED) {
                        required.push(f.name().to_owned());
                    }
                    keys.push(Key {
                        name: f.name().to_owned(),
                        ident: ident(f.name()),
                        kind,
                        required: fb.contains(&REQUIRED),
                        comment: docs::plain(cx.comments.leading(f.parent_file().name(), f.path())),
                    });
                }
                None => {
                    required_json_only |= fb.contains(&REQUIRED);
                    json_only.push(f.name().to_owned());
                }
            }
        }
    }
    let parser = (!keys.is_empty() && !required_json_only).then(|| {
        let local = rust_type_path(m.package_name(), m.full_name())
            .replace("::", "_")
            .to_snake_case();
        format!("parse_{local}")
    });
    if let Some(parser) = &parser {
        let mut arms = Vec::new();
        for key in &keys {
            let (name, id) = (&key.name, &key.ident);
            let value = match &key.kind {
                Kind2::Scalar("String") | Kind2::Reference(_) => "value.to_owned()".to_owned(),
                Kind2::Scalar(ty) => format!("key_scalar::<{ty}>({name:?}, value)?"),
                Kind2::Bool => format!("key_bool({name:?}, value)?"),
                Kind2::Duration => format!("Some(key_duration({name:?}, value)?)"),
                Kind2::Enum(path, values) => format!(
                    "key_enum({name:?}, value, &[{}], |v| {path}::from_str_name(v).map(|e| e as i32))?",
                    values.iter().map(|v| format!("{v:?}")).collect::<Vec<_>>().join(", ")
                ),
                _ => unreachable!("a key is a scalar, an enum, a duration or a name"),
            };
            arms.push(format!("{name:?} => out.{id} = {value},"));
        }
        let quoted = |names: &mut dyn Iterator<Item = &String>| {
            names
                .map(|n| format!("{n:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let all = quoted(&mut keys.iter().map(|k| &k.name));
        let json = quoted(&mut json_only.iter());
        let needed = quoted(&mut required.iter());
        let code = format!(
            "/// A `{rust}` from its `KEY=VALUE,...` form.\n\
             fn {parser}(text: &str) -> ::core::result::Result<{rust}, String> {{\n\
                 let pairs = key_values(text)?;\n\
                 let mut out = <{rust}>::default();\n\
                 for (key, value) in &pairs {{\n\
                     let value = value.as_str();\n\
                     match key.as_str() {{\n\
                         {}\n\
                         other => return Err(unknown_key(other, &[{all}], &[{json}])),\n\
                     }}\n\
                 }}\n\
                 required_keys(&pairs, &[{needed}])?;\n\
                 Ok(out)\n\
             }}\n",
            arms.join("\n")
        );
        cx.helpers.borrow_mut().insert(parser.clone(), code);
    }
    Ok(Nested {
        full_name: m.full_name().to_owned(),
        rust,
        keys,
        json_only,
        parser,
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
    /// Whether the handler completes resource names, such as those a
    /// nested message's keys give, without a name-part flag.
    needs_names: bool,
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
    // Only the operation methods answer with an operation they didn't
    // start, which they print as it is.
    if m.output().full_name() == OPERATION && result.is_none() && cx.host.is_none() {
        return Err("it answers with an operation but names no response_type".to_owned());
    }
    // Answering with the operation as it is after waiting up to `timeout`:
    // called again until it is done, unless `--timeout` is given.
    let waits = cx.host.is_some()
        && m.output().full_name() == OPERATION
        && result.is_none()
        && request
            .get_field_by_name("timeout")
            .is_some_and(|f| matches!(f.kind(), Kind::Message(ref k) if k.full_name() == DURATION));
    let client = client_name(m.parent_service());
    let destructive = m.name().starts_with("Delete");
    let update = request
        .get_field_by_name("update_mask")
        .is_some_and(|f| matches!(f.kind(), Kind::Message(ref k) if k.full_name() == FIELD_MASK));
    let creating = !update && request.fields().any(|f| body_message(cx, &f).is_some());
    let paging = request.get_field_by_name("page_token").is_some();
    let response = m.output();
    let list = paging.then(|| list_field(&response)).flatten();
    let excluded = |path: &str| cx.policy.excluded(cx.own, m.name(), path);
    let file = m.parent_file();
    let file = file.name();
    let comment = |field: &FieldDescriptor| {
        let mut text = cx
            .comments
            .leading(field.parent_message().parent_file().name(), field.path())
            .trim_end()
            .to_owned();
        if behaviors(cx.pool, field).contains(&UNORDERED_LIST) {
            text.push_str("\n\nThe API doesn't keep their order, and each must be unique.");
        }
        text
    };

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
            if let Some(r) = named_resource(cx, m, &field) {
                b.vocabulary
                    .insert("name".to_owned(), leaf_of(&r.pattern).to_kebab_case());
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
                if let Some(entry) = cx.policy.one_way(cx.own, m.name(), &path) {
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
        } else if name == "name" {
            if let Some(resource) = http_resource(cx.pool, m, &name) {
                // The field's comment describes the name as one string; the
                // flag takes it by its parts too, which its help says.
                let leaf = leaf_of(&resource.pattern);
                let what = format!("The {}.", resource.singular.replace('_', " "));
                let expr = leaf_name(&mut b, &resource, &leaf, &what)?;
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
        if name == "timeout" && waits {
            // The flag is a deadline of the command's: each call waits a
            // minute at most, as the API allows, and the command calls again
            // until the operation is done or the deadline passes.
            if let Some(flag) = b.flags.iter_mut().find(|f| f.ident == fid) {
                flag.doc = "How long to wait for the operation, such as `30s` or `5m`. Without \
                            it, the command waits as long as the operation takes. When it passes \
                            first, the command prints the operation as it is and exits with \
                            status 1."
                    .to_owned();
            }
        }
        b.request.push(init(&fid, &expr));
        if let Some(entry) = cx.policy.one_way(cx.own, m.name(), &name) {
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
    } else if waits {
        let _ = writeln!(
            call,
            "let client = ctx.client()?;\n\
             let deadline = args.timeout.map(deadline_after).transpose()?;\n\
             let mut request = request;\n\
             let mut announced = false;\n\
             let response = loop {{\n\
                 let started = ::std::time::Instant::now();\n\
                 if let Some(deadline) = deadline {{ request.timeout = Some(wait_for(deadline)); }}\n\
                 let response = call_{handler}(&client, request.clone()).await.map_err(api_error)?;\n\
                 if response.done || deadline.is_some_and(passed) {{ break response; }}\n\
                 if !announced {{ waiting(&response.name); announced = true; }}\n\
                 pace(started, deadline).await;\n\
             }};"
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
        _ if waits => tail.push_str("Ok(waited(&response))\n"),
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
                                 for zone in zones(ctx, {service:?}).await? {{\n\
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
    let uses_names = !b.components.is_empty() || !b.learn.is_empty() || b.needs_names;
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
    let endpoint = cx.host.as_deref().unwrap_or("client.sdk()");
    let sdk_call = format!(
        "{}::{client}::new({endpoint}).{handler}(request).await",
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
            .filter(|o| o.service == cx.own && o.method == m.name())
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
        let mut text = help(cx.comments.leading(file, m.path()));
        if text.is_empty() {
            text = format!("Calls `{}`.", m.name());
        }
        if cx.host.is_some() {
            let _ = write!(
                text,
                "\n\nThe operation is one an `origin {}` command answered with, such as the one \
                 `--no-wait` prints.",
                cx.service
            );
        }
        if waits {
            text.push_str(
                " Without `--timeout`, the command waits until the operation is done; Ctrl-C \
                 stops the waiting, not the operation. It prints the operation, and exits with \
                 status 1 when the operation failed, or isn't done when `--timeout` passes.",
            );
        }
        text
    };
    Ok(Command {
        name: m.name().to_kebab_case(),
        variant,
        handler,
        doc,
        code,
        lists_zones: zonal_parent.is_some(),
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
    if matches!(
        k,
        Kind2::Map | Kind2::List(_) | Kind2::Message(_) | Kind2::Messages(_)
    ) {
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
    } else if message.full_name() == OPERATION {
        "::origincloud_core::longrunning::Operation".to_owned()
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
fn flag_name(_cx: &Cx<'_>, field: &FieldDescriptor) -> String {
    if field.is_map() || field.is_list() {
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
        Kind2::List(elem) => {
            let one = ident(&singular(field.name()));
            let long = singular(field.name()).to_kebab_case();
            let mut attrs = Vec::new();
            required_attr(&mut attrs);
            let (ty, doc, expr) = list_elements(b, elem, &one, &long, doc);
            attrs.extend(list_attrs(elem));
            b.add_flag(Flag {
                ident: one.clone(),
                long,
                doc: format!("{doc}\n\nRepeat for more."),
                ty,
                attrs,
            })?;
            if matches!(elem, Elem::Reference(_)) {
                b.prelude.push(format!("let {fid} = {expr};"));
                fid
            } else {
                expr
            }
        }
        Kind2::Message(n) => {
            let json = format!("{fid}_json");
            let json_long = format!("{long}-json");
            let completions = completions(b, n, "item", &long);
            if let Some(parser) = &n.parser {
                let mut attrs = vec![
                    "value_name = \"KEY=VALUE,...\"".to_owned(),
                    format!("value_parser = {parser}"),
                    format!("conflicts_with = {json:?}"),
                ];
                if required {
                    attrs.push(format!("required_unless_present = {json:?}"));
                }
                b.add_flag(Flag {
                    ident: fid.clone(),
                    long: long.clone(),
                    doc: keyed_doc(doc, n, &json_long, false),
                    ty: format!("Option<{}>", n.rust),
                    attrs,
                })?;
            }
            b.add_flag(Flag {
                ident: json.clone(),
                long: json_long,
                doc: json_doc(doc, n, &long, false),
                ty: format!("Option<JsonArg<{}>>", n.rust),
                attrs: json_attrs(&n.rust, &n.full_name, required && n.parser.is_none()),
            })?;
            if n.parser.is_some() {
                if completions.is_empty() {
                    b.prelude.push(format!("let {fid} = args.{fid};"));
                } else {
                    b.prelude.push(format!(
                        "let mut {fid} = args.{fid}; if let Some(item) = &mut {fid} {{ {} }}",
                        completions.join(" ")
                    ));
                }
                b.prelude.push(format!(
                    "let {fid} = match args.{json} {{ Some(json) => Some(json.0), None => {fid} }};"
                ));
            } else {
                b.prelude
                    .push(format!("let {fid} = args.{json}.map(|json| json.0);"));
            }
            fid
        }
        Kind2::Messages(n) => {
            let one = ident(&singular(field.name()));
            let one_long = singular(field.name()).to_kebab_case();
            let json = format!("{fid}_json");
            let json_long = format!("{long}-json");
            let completions = completions(b, n, "item", &one_long);
            if let Some(parser) = &n.parser {
                let mut attrs = vec![
                    "value_name = \"KEY=VALUE,...\"".to_owned(),
                    format!("value_parser = {parser}"),
                    format!("conflicts_with = {json:?}"),
                ];
                if required {
                    attrs.push(format!("required_unless_present = {json:?}"));
                }
                b.add_flag(Flag {
                    ident: one.clone(),
                    long: one_long.clone(),
                    doc: keyed_doc(doc, n, &json_long, true),
                    ty: format!("Vec<{}>", n.rust),
                    attrs,
                })?;
            }
            b.add_flag(Flag {
                ident: json.clone(),
                long: json_long,
                doc: json_doc(doc, n, &one_long, true),
                ty: format!("Option<JsonArg<Vec<{}>>>", n.rust),
                attrs: json_attrs(
                    &format!("Vec<{}>", n.rust),
                    &n.full_name,
                    required && n.parser.is_none(),
                ),
            })?;
            if n.parser.is_some() {
                if completions.is_empty() {
                    b.prelude.push(format!("let {fid} = args.{one};"));
                } else {
                    b.prelude.push(format!(
                        "let mut {fid} = args.{one}; for item in &mut {fid} {{ {} }}",
                        completions.join(" ")
                    ));
                }
                b.prelude.push(format!(
                    "let {fid} = match args.{json} {{ Some(json) => json.0, None => {fid} }};"
                ));
            } else {
                b.prelude.push(format!(
                    "let {fid} = args.{json}.map(|json| json.0).unwrap_or_default();"
                ));
            }
            fid
        }
        Kind2::Duration => {
            let mut attrs = vec![
                "value_name = \"DURATION\"".to_owned(),
                "value_parser = duration".to_owned(),
            ];
            required_attr(&mut attrs);
            b.add_flag(Flag {
                ident: fid.clone(),
                long,
                doc: format!("{doc}{DURATION_FORMAT}"),
                ty: if required {
                    DURATION_TYPE.to_owned()
                } else {
                    format!("Option<{DURATION_TYPE}>")
                },
                attrs,
            })?;
            if required {
                format!("Some(args.{fid})")
            } else {
                format!("args.{fid}")
            }
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
        Kind2::Map | Kind2::List(_) => {
            let one = ident(&singular(name));
            let one_long = singular(name).to_kebab_case();
            let none = format!("no_{}", name.to_snake_case());
            let (ty, comment, expr, attrs, form) = match k {
                Kind2::List(elem) => {
                    let (ty, comment, expr) = list_elements(b, elem, &one, &one_long, comment);
                    (ty, comment, expr, list_attrs(elem), "Repeat")
                }
                _ => (
                    "Vec<TagArg>".to_owned(),
                    comment.to_owned(),
                    format!("tags(&args.{one})"),
                    vec!["value_name = \"KEY=VALUE\"".to_owned()],
                    "As KEY=VALUE; repeat",
                ),
            };
            b.add_flag(Flag {
                ident: one.clone(),
                long: one_long,
                doc: format!(
                    "{comment}\n\n{form} for more. Replaces all of them; `--no-{long}` removes them all. Left as they are when neither is given."
                ),
                ty,
                attrs,
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
            format!(
                "if args.{none} || !args.{one}.is_empty() {{ {} }}",
                set(&expr)
            )
        }
        Kind2::Message(n) | Kind2::Messages(n) => {
            let repeated = matches!(k, Kind2::Messages(_));
            let (one, one_long) = if repeated {
                (ident(&singular(name)), singular(name).to_kebab_case())
            } else {
                (fid.clone(), long.clone())
            };
            let json = format!("{fid}_json");
            let json_long = format!("{long}-json");
            let none = format!("no_{}", name.to_snake_case());
            let replaced = if repeated {
                format!(
                    "\n\nThe list is replaced whole: give every element to keep. `--no-{long}` \
                     removes them all. Left as it is when none of these is given."
                )
            } else {
                format!(
                    "\n\nIt is replaced whole: a key not given is reset. `--no-{long}` removes \
                     it. Left as it is when none of these is given."
                )
            };
            let completions = completions(b, n, "item", &one_long);
            let mut conflicts = vec![format!("{json:?}")];
            if let Some(parser) = &n.parser {
                b.add_flag(Flag {
                    ident: one.clone(),
                    long: one_long.clone(),
                    doc: format!("{}{replaced}", keyed_doc(comment, n, &json_long, repeated)),
                    ty: if repeated {
                        format!("Vec<{}>", n.rust)
                    } else {
                        format!("Option<{}>", n.rust)
                    },
                    attrs: vec![
                        "value_name = \"KEY=VALUE,...\"".to_owned(),
                        format!("value_parser = {parser}"),
                        format!("conflicts_with = {json:?}"),
                    ],
                })?;
                b.update_ids.push(arg_id(&one).to_owned());
                conflicts.push(format!("{:?}", arg_id(&one)));
            }
            let json_ty = if repeated {
                format!("Vec<{}>", n.rust)
            } else {
                n.rust.clone()
            };
            b.add_flag(Flag {
                ident: json.clone(),
                long: json_long,
                doc: format!("{}{replaced}", json_doc(comment, n, &one_long, repeated)),
                ty: format!("Option<JsonArg<{json_ty}>>"),
                attrs: json_attrs(&json_ty, &n.full_name, false),
            })?;
            b.add_flag(Flag {
                ident: none.clone(),
                long: format!("no-{long}"),
                doc: if repeated {
                    format!("Remove all of its {}.", name.replace('_', " "))
                } else {
                    format!("Remove its {}.", name.replace('_', " "))
                },
                ty: "bool".to_owned(),
                attrs: vec![format!("conflicts_with_all = [{}]", conflicts.join(", "))],
            })?;
            b.update_ids.push(json.clone());
            b.update_ids.push(none.clone());
            let (given, from_json) = if repeated {
                (format!("!args.{one}.is_empty()"), "json.0")
            } else {
                (format!("args.{one}.is_some()"), "Some(json.0)")
            };
            match &n.parser {
                Some(_) => {
                    let item = if completions.is_empty() {
                        format!("let items = args.{one};")
                    } else if repeated {
                        format!(
                            "let mut items = args.{one}; for item in &mut items {{ {} }}",
                            completions.join(" ")
                        )
                    } else {
                        format!(
                            "let mut items = args.{one}; if let Some(item) = &mut items {{ {} }}",
                            completions.join(" ")
                        )
                    };
                    format!(
                        "if args.{none} || {given} || args.{json}.is_some() {{ {item} {} }}",
                        set(&format!(
                            "match args.{json} {{ Some(json) => {from_json}, None => items }}"
                        ))
                    )
                }
                None => format!(
                    "if args.{none} || args.{json}.is_some() {{ {} }}",
                    set(&if repeated {
                        format!("args.{json}.map(|json| json.0).unwrap_or_default()")
                    } else {
                        format!("args.{json}.map(|json| json.0)")
                    })
                ),
            }
        }
        Kind2::Duration => {
            b.add_flag(Flag {
                ident: fid.clone(),
                long,
                doc: format!("{comment}{DURATION_FORMAT}{kept}"),
                ty: format!("Option<{DURATION_TYPE}>"),
                attrs: vec![
                    "value_name = \"DURATION\"".to_owned(),
                    "value_parser = duration".to_owned(),
                ],
            })?;
            b.update_ids.push(arg_id(&fid).to_owned());
            format!(
                "if let Some(value) = args.{fid} {{ {} }}",
                set("Some(value)")
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

/// A repeated scalar field's flag: its type, its help, and the expression
/// of its value, given the flag's field `one` and name `long`.
fn list_elements(
    b: &mut Build,
    elem: &Elem,
    one: &str,
    long: &str,
    doc: &str,
) -> (String, String, String) {
    match elem {
        Elem::Scalar(ty) => (
            format!("Vec<{ty}>"),
            doc.to_owned(),
            format!("args.{one}.clone()"),
        ),
        Elem::Enum(path, _) => (
            "Vec<String>".to_owned(),
            doc.to_owned(),
            format!(
                "args.{one}.iter().filter_map(|v| {path}::from_str_name(v)).map(|v| v as i32).collect()"
            ),
        ),
        Elem::Reference(resource) => {
            let leaf = leaf_of(&resource.pattern);
            b.add_components(&resource.pattern, Some(&leaf));
            b.learn.push(format!(
                "for value in &args.{one} {{ names.learn({:?}, {long:?}, Some(value.as_str()))?; }}",
                resource.pattern
            ));
            (
                "Vec<String>".to_owned(),
                reference_doc(doc, &resource.pattern, &leaf).replace("Its id", "Each by its id"),
                format!(
                    "args.{one}.iter().map(|v| names.name({:?}, Some(({long:?}, v.as_str())))).collect::<anyhow::Result<Vec<String>>>()?",
                    resource.pattern
                ),
            )
        }
    }
}

/// The attributes of a repeated scalar field's flag.
fn list_attrs(elem: &Elem) -> Vec<String> {
    match elem {
        Elem::Enum(_, values) => {
            let quoted: Vec<String> = values.iter().map(|v| format!("{v:?}")).collect();
            vec![format!("value_parser = [{}]", quoted.join(", "))]
        }
        Elem::Reference(resource) => {
            vec![format!(
                "value_name = {:?}",
                leaf_of(&resource.pattern).to_uppercase()
            )]
        }
        Elem::Scalar(_) => Vec::new(),
    }
}

/// The statements completing the resource names a nested message's keys
/// gave, on the message `var`, given with the flag `--{flag}`: an id is
/// completed with the command's `--project` and `--zone`, as everywhere.
fn completions(b: &mut Build, n: &Nested, var: &str, flag: &str) -> Vec<String> {
    let mut out = Vec::new();
    if n.parser.is_none() {
        return out;
    }
    for key in &n.keys {
        if let Kind2::Reference(resource) = &key.kind {
            let leaf = leaf_of(&resource.pattern);
            b.add_components(&resource.pattern, Some(&leaf));
            b.needs_names = true;
            let id = &key.ident;
            let label = format!("{flag} {}", key.name);
            out.push(format!(
                "if !{var}.{id}.is_empty() {{ {var}.{id} = names.name({:?}, Some(({label:?}, {var}.{id}.as_str())))?; }}",
                resource.pattern
            ));
        }
    }
    out
}

/// What a key's help adds about its value.
fn key_hint(kind: &Kind2) -> String {
    match kind {
        Kind2::Enum(_, values) => {
            let quoted: Vec<String> = values.iter().map(|v| format!("`{v}`")).collect();
            format!(" One of {}.", quoted.join(", "))
        }
        Kind2::Bool => " `true` or `false`.".to_owned(),
        Kind2::Duration => " Such as `30s` or `1m`.".to_owned(),
        Kind2::Reference(resource) => {
            let leaf = leaf_of(&resource.pattern);
            format!(" {}", reference_doc("", &resource.pattern, &leaf))
        }
        _ => String::new(),
    }
}

/// The help of a nested message's `KEY=VALUE,...` flag: the field's, then
/// the form and every key.
fn keyed_doc(comment: &str, n: &Nested, json_long: &str, repeated: bool) -> String {
    let mut doc = comment.trim_end().to_owned();
    if !doc.is_empty() {
        doc.push_str("\n\n");
    }
    doc.push_str(if repeated {
        "One element as comma-separated KEY=VALUE pairs; repeat the flag for more. The keys:"
    } else {
        "As comma-separated KEY=VALUE pairs. The keys:"
    });
    for key in &n.keys {
        let mut line = format!("\n\n`{}`", key.name);
        if key.required {
            line.push_str(", required");
        }
        line.push(':');
        if !key.comment.is_empty() {
            line.push(' ');
            line.push_str(&key.comment);
        }
        line.push_str(&key_hint(&key.kind));
        doc.push_str(&line);
    }
    let mut rest = Vec::new();
    if !n.json_only.is_empty() {
        let names: Vec<String> = n.json_only.iter().map(|f| format!("`{f}`")).collect();
        rest.push(format!(
            "{} {} set only through `--{json_long}`.",
            names.join(", "),
            if names.len() == 1 { "is" } else { "are" }
        ));
    }
    rest.push(format!(
        "A value with a comma in it goes through `--{json_long}` too."
    ));
    doc.push_str("\n\n");
    doc.push_str(&rest.join(" "));
    doc
}

/// The help of a message field's JSON flag.
fn json_doc(comment: &str, n: &Nested, keyed_long: &str, repeated: bool) -> String {
    let what = if repeated {
        "The whole list as the API's JSON, an array of objects"
    } else {
        "As the API's JSON, an object"
    };
    let mut doc = String::new();
    if n.parser.is_none() && !comment.trim().is_empty() {
        doc.push_str(comment.trim_end());
        doc.push_str("\n\n");
    }
    doc.push_str(&format!(
        "{what} whose fields are named in camel case, as the command prints them, or as the API \
         definitions name them; `@FILE` reads it from a file. A field the message doesn't have is \
         refused, at any depth. Resource names in it are sent as given: give whole names."
    ));
    if n.parser.is_some() {
        doc.push_str(&format!(" Instead of `--{keyed_long}`."));
    }
    doc
}

/// The attributes of a message field's JSON flag, whose value is a `ty`.
fn json_attrs(ty: &str, message: &str, required: bool) -> Vec<String> {
    let mut attrs = vec![
        "value_name = \"JSON\"".to_owned(),
        format!("value_parser = |text: &str| json_arg::<{ty}>(text, {message:?})"),
    ];
    if required {
        attrs.push("required = true".to_owned());
    }
    attrs
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

    #[test]
    fn a_binding_template_is_a_name_pattern() {
        assert_eq!(
            pattern_of_template("projects/*/zones/*/operations/*").as_deref(),
            Some("projects/{project}/zones/{zone}/operations/{operation}")
        );
        assert_eq!(
            pattern_of_template("storageClasses/*").as_deref(),
            Some("storageClasses/{storage_class}")
        );
        for bad in [
            "projects/*/zones",
            "*",
            "projects/**",
            "projects/*/zones/{zone}",
            "",
        ] {
            assert_eq!(pattern_of_template(bad), None, "{bad}");
        }
    }
}
