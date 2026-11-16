//! The output formats, run as a person or a script runs `origin`: which
//! one is in effect, and what `table` and `name` print for a list, a
//! resource, an empty list, a delete and a failure. The tests run `origin` on a pipe,
//! where `json` is the default, as every other test file relies on.

mod common;

use common::{origin, recording, stdout};
use fake_server::{Received, Response};

const ZONES: &str = r#"{"zones":[
    {"name":"zones/z-a","displayName":"Zone A","region":"regions/r","services":["pvs"]},
    {"name":"zones/z-b","region":"regions/r"}]}"#;
const ZONE: &str = r#"{"name":"zones/z-a","displayName":"Zone A","region":"regions/r","services":["pvs","compute"]}"#;

fn location(request: &Received) -> Response {
    match request.path.as_str() {
        "/v1beta1/zones" => Response::json(ZONES),
        "/v1beta1/zones/z-a" => Response::json(ZONE),
        _ => Response::json(
            r#"{"error":{"code":404,"status":"NOT_FOUND","message":"no such zone"}}"#,
        )
        .with_status(404),
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// What `origin` printed on standard error but the notice of `--endpoint`,
/// which every command run against the server prints first.
fn err(output: &std::process::Output) -> String {
    text(&output.stderr)
        .lines()
        .filter(|line| !line.starts_with("Sending the `origin "))
        .map(|line| format!("{line}\n"))
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_pipe_gets_json_unless_a_format_is_chosen() {
    let server = recording(location).await;
    let printed = stdout(&server.run("location", &["list-zones"]).await);
    assert_eq!(printed["zones"][0]["name"], "zones/z-a");

    let table = server.run("location", &["list-zones", "-o", "table"]).await;
    assert!(text(&table.stdout).starts_with("NAME"), "{table:?}");

    // The variable, then `--output` over it.
    let env = [("ORIGINCLOUD_OUTPUT_FORMAT", "table")];
    let table = server.run_with("location", &["list-zones"], &env).await;
    assert!(text(&table.stdout).starts_with("NAME"), "{table:?}");
    let json = server
        .run_with("location", &["list-zones", "--output", "json"], &env)
        .await;
    assert_eq!(stdout(&json)["zones"][1]["name"], "zones/z-b");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_profile_s_format_holds_on_a_pipe_too() {
    let server = recording(location).await;
    let home = server.home.path();
    let set = origin(
        home,
        &["config", "profile-set", "output-format", "name"],
        &[],
    )
    .await;
    assert_eq!(set.status.code(), Some(0), "{set:?}");

    let names = server.run("location", &["list-zones"]).await;
    assert_eq!(text(&names.stdout), "zones/z-a\nzones/z-b\n");

    let refused = origin(
        home,
        &["config", "profile-set", "output-format", "yaml"],
        &[],
    )
    .await;
    // Refused in the profile's format too: as an error.
    assert_eq!(refused.status.code(), Some(1));
    assert_eq!(
        err(&refused),
        "error: unknown output format \"yaml\": expected table, wide, json or name\n"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn table_prints_a_list_as_a_table_and_a_resource_as_its_fields() {
    let server = recording(location).await;
    let list = server.run("location", &["list-zones", "-o", "table"]).await;
    assert_eq!(list.status.code(), Some(0));
    assert_eq!(
        text(&list.stdout),
        "\
NAME   DISPLAY NAME   REGION   SERVICES
z-a    Zone A         r        pvs
z-b                   r
"
    );
    assert_eq!(err(&list), "");

    let one = server
        .run("location", &["get-zone", "--zone", "z-a", "-o", "table"])
        .await;
    assert_eq!(
        text(&one.stdout),
        "\
name         zones/z-a
displayName  Zone A
region       regions/r
services     pvs, compute
"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_table_fits_columns() {
    let server = recording(location).await;
    let narrow = server
        .run_with(
            "location",
            &["list-zones", "-o", "table"],
            &[("COLUMNS", "24")],
        )
        .await;
    assert_eq!(
        text(&narrow.stdout),
        "\
NAME   DISPLAY NAME
z-a    Zone A
z-b
"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn name_prints_the_whole_names_for_the_resource_flags() {
    let server = recording(location).await;
    let names = server.run("location", &["list-zones", "-o", "name"]).await;
    assert_eq!(text(&names.stdout), "zones/z-a\nzones/z-b\n");
    let name = server
        .run("location", &["get-zone", "--zone", "z-a", "-o", "name"])
        .await;
    assert_eq!(text(&name.stdout), "zones/z-a\n");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_empty_list_is_said_on_standard_error() {
    let server = recording(|_| Response::json("{}")).await;
    let empty = server.run("location", &["list-zones", "-o", "table"]).await;
    assert_eq!(empty.status.code(), Some(0));
    assert_eq!(text(&empty.stdout), "");
    assert_eq!(err(&empty), "No zones.\n");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failure_is_an_error_on_standard_error_in_table_and_json_on_a_pipe() {
    let server = recording(location).await;
    let failed = server
        .run("location", &["get-zone", "--zone", "z-x", "-o", "table"])
        .await;
    assert_eq!(failed.status.code(), Some(1));
    assert_eq!(text(&failed.stdout), "");
    assert_eq!(err(&failed), "error: NOT_FOUND: no such zone\n");

    let failed = server.run("location", &["get-zone", "--zone", "z-x"]).await;
    let description = common::failure(&failed);
    assert!(description.contains("no such zone"), "{description}");
}

#[tokio::test(flavor = "multi_thread")]
async fn colour_is_only_where_it_is_asked_for_off_a_terminal() {
    let server = recording(location).await;
    let plain = server.run("location", &["list-zones", "-o", "table"]).await;
    assert!(!text(&plain.stdout).contains('\u{1b}'));

    let coloured = server
        .run(
            "location",
            &["list-zones", "-o", "table", "--color", "always"],
        )
        .await;
    assert!(
        text(&coloured.stdout).starts_with("\u{1b}[1mNAME"),
        "{coloured:?}"
    );

    // `json` is never coloured.
    let json = server
        .run("location", &["list-zones", "--color", "always"])
        .await;
    assert!(!text(&json.stdout).contains('\u{1b}'));
}

const VOLUMES: &str = r#"{"volumes":[
    {"name":"projects/p/zones/z/volumes/v1","sizeGib":8,"state":"IN_USE",
     "storageClass":"storageClasses/gp1","urn":"urn:v1","tags":{"env":"prod"}},
    {"name":"projects/p/zones/z/volumes/v2","sizeGib":16,"state":"ERROR",
     "storageClass":"storageClasses/gp1","stateReason":"no capacity"}]}"#;

/// A delete's operation, done.
const DELETED: &str = r#"{"name":"projects/p/zones/z/operations/op-1","done":true,
    "response":{"@type":"type.googleapis.com/google.protobuf.Empty"}}"#;

#[tokio::test(flavor = "multi_thread")]
async fn a_list_shows_its_view_s_columns_and_wide_the_rest() {
    let server = recording(|_| Response::json(VOLUMES)).await;
    let args = ["list-volumes", "--project", "p", "--zone", "z"];
    let table = server.pvs(&[&args[..], &["-o", "table"]].concat()).await;
    assert_eq!(
        text(&table.stdout),
        "\
NAME   STATE    SIZE GIB   STORAGE CLASS   RESIZE STATE
v1     IN_USE   8          gp1
v2     ERROR    16         gp1
"
    );

    let wide = server.pvs(&[&args[..], &["-o", "wide"]].concat()).await;
    let wide = text(&wide.stdout);
    let header = wide.lines().next().unwrap();
    assert!(
        header.starts_with("NAME   STATE    SIZE GIB   STORAGE CLASS   RESIZE STATE   URN"),
        "{header}"
    );
    assert!(
        header.contains("STATE REASON") && header.contains("TAGS"),
        "{header}"
    );
    assert!(
        wide.contains("no capacity") && wide.contains("env=prod"),
        "{wide}"
    );
}

/// Role bindings, the caller's own among them (`organizations/-`), are
/// listed by member and role, a role by its id rather than its whole name.
#[tokio::test(flavor = "multi_thread")]
async fn role_bindings_are_listed_by_member_and_role() {
    const BINDINGS: &str = r#"{"roleBindings":[
        {"name":"organizations/o/roleBindings/rb-1","urn":"urn:origin:cloud:iam::o::roleBinding/rb-1",
         "member":"user:ada@example.com","role":"roles/owner","creator":"platform"},
        {"name":"organizations/o/roleBindings/rb-2","member":"serviceAccount:sa-1",
         "role":"roles/viewer","project":"projects/p1","creator":"user:ada@example.com"}]}"#;
    let server = recording(|_| Response::json(BINDINGS)).await;
    let table = server
        .run(
            "iam",
            &["list-role-bindings", "--organization", "-", "-o", "table"],
        )
        .await;
    assert_eq!(
        text(&table.stdout),
        "\
NAME   MEMBER                 ROLE     PROJECT       CREATE TIME
rb-1   user:ada@example.com   owner
rb-2   serviceAccount:sa-1    viewer   projects/p1
"
    );
    assert_eq!(
        server.requests()[0].path,
        "/v1beta1/organizations/-/roleBindings"
    );
}

/// Service accounts are listed with the member a role binding names them
/// by, their state and the client id they log in with; a deleted one
/// (`--show-deleted`) says so in its state.
#[tokio::test(flavor = "multi_thread")]
async fn service_accounts_are_listed_by_member_state_and_client_id() {
    const ACCOUNTS: &str = r#"{"serviceAccounts":[
        {"name":"organizations/o/serviceAccounts/sa-1","displayName":"CI deploy",
         "member":"serviceAccount:sa-1","clientId":"c-7f3a","state":"ACTIVE",
         "creator":"user:ada@example.com"},
        {"name":"organizations/o/serviceAccounts/sa-2","displayName":"Old backup",
         "member":"serviceAccount:sa-2","clientId":"c-91be","state":"DELETED",
         "creator":"user:ada@example.com","deleteTime":"2026-01-02T00:00:00Z"}]}"#;
    let server = recording(|_| Response::json(ACCOUNTS)).await;
    let args = ["list-service-accounts", "--organization", "o"];
    let table = server
        .run("iam", &[&args[..], &["-o", "table"]].concat())
        .await;
    assert_eq!(
        text(&table.stdout),
        "\
NAME   DISPLAY NAME   STATE     MEMBER                CLIENT ID   CREATE TIME
sa-1   CI deploy      ACTIVE    serviceAccount:sa-1   c-7f3a
sa-2   Old backup     DELETED   serviceAccount:sa-2   c-91be
"
    );

    let wide = server
        .run("iam", &[&args[..], &["-o", "wide"]].concat())
        .await;
    let header = text(&wide.stdout).lines().next().unwrap().to_owned();
    assert!(header.contains("DELETE TIME"), "{header}");
}

/// Roles are listed by their id and the name the console shows them by,
/// with what holding them allows; their permissions are in `-o wide`.
#[tokio::test(flavor = "multi_thread")]
async fn roles_are_listed_by_id_display_name_and_description() {
    const ROLES: &str = r#"{"roles":[
        {"name":"roles/owner","urn":"urn:origin:cloud:iam:::role/owner","displayName":"Owner",
         "description":"Full access, including granting roles.",
         "includedPermissions":["permissions/iam.roleBindings.create","permissions/compute.instances.create"]},
        {"name":"roles/billing.administrator","urn":"urn:origin:cloud:iam:::role/billing.administrator",
         "displayName":"Billing Administrator","description":"Manages billing accounts.",
         "includedPermissions":["permissions/billing.accounts.update"]}]}"#;
    let server = recording(|_| Response::json(ROLES)).await;
    let table = server.run("iam", &["list-roles", "-o", "table"]).await;
    assert_eq!(
        text(&table.stdout),
        "\
NAME                    DISPLAY NAME            DESCRIPTION
owner                   Owner                   Full access, including granting roles.
billing.administrator   Billing Administrator   Manages billing accounts.
"
    );

    let wide = server.run("iam", &["list-roles", "-o", "wide"]).await;
    let wide = text(&wide.stdout);
    assert!(wide.contains("INCLUDED PERMISSIONS"), "{wide}");
    assert!(
        wide.contains("iam.roleBindings.create, compute.instances.create"),
        "{wide}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn an_empty_list_says_where_it_looked_and_how_to_make_one() {
    let server = recording(|_| Response::json("{}")).await;
    let empty = server
        .pvs(&[
            "list-volumes",
            "--project",
            "p",
            "--zone",
            "z",
            "-o",
            "table",
        ])
        .await;
    assert_eq!(empty.status.code(), Some(0));
    assert_eq!(text(&empty.stdout), "");
    assert_eq!(
        err(&empty),
        "No volumes in project p, zone z. Create one with `origin pvs create-volume`.\n"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_delete_says_what_it_deleted_and_json_says_success() {
    let server = recording(|_| Response::json(DELETED)).await;
    let args = [
        "delete-volume",
        "--project",
        "p",
        "--zone",
        "z",
        "--volume",
        "v1",
        "--yes",
    ];
    let table = server.pvs(&[&args[..], &["-o", "table"]].concat()).await;
    assert_eq!(table.status.code(), Some(0), "{table:?}");
    assert_eq!(text(&table.stdout), "✓ Deleted volume v1.\n");

    let json = server.pvs(&args).await;
    assert_eq!(stdout(&json), serde_json::json!({"code": "success"}));
}

/// An RFC 3339 time `hours` before now, which the table shows relative to
/// now: `3h ago`, or `5d ago` for 120 hours.
fn ago(hours: i64) -> String {
    (chrono::Utc::now() - chrono::Duration::hours(hours))
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// The table `origin <group> <args> -o table` prints for `body`, and the
/// header `-o wide` prints.
async fn tables(body: String, group: &str, args: &[&str]) -> (String, String) {
    let server = recording(move |_| Response::json(&body)).await;
    let table = server.run(group, &[args, &["-o", "table"]].concat()).await;
    let wide = server.run(group, &[args, &["-o", "wide"]].concat()).await;
    let wide = text(&wide.stdout)
        .lines()
        .next()
        .unwrap_or_default()
        .to_owned();
    (text(&table.stdout), wide)
}

/// Audit log entries are listed by when, who, what, to what and how it
/// went; the details are in `-o wide`.
#[tokio::test(flavor = "multi_thread")]
async fn audit_log_entries_are_listed_by_time_actor_action_and_outcome() {
    let entries = format!(
        r#"{{"auditLogEntries":[
        {{"name":"organizations/o/auditLogEntries/e-1","actor":"user:ada@example.com",
         "action":"iam.role.assign","target":"user:bob@example.com","outcome":"SUCCEEDED",
         "occurTime":"{}","details":{{"role":"roles/viewer"}}}},
        {{"name":"organizations/o/auditLogEntries/e-2","project":"projects/p1",
         "actor":"serviceAccount:sa-1","action":"compute.instance.create",
         "outcome":"DENIED","occurTime":"{}"}}]}}"#,
        ago(3),
        ago(120)
    );
    let (table, wide) = tables(
        entries,
        "iam",
        &["list-audit-log-entries", "--organization", "o"],
    )
    .await;
    assert_eq!(
        table,
        "\
NAME   OCCUR TIME   ACTOR                  ACTION                    TARGET                 OUTCOME     PROJECT
e-1    3h ago       user:ada@example.com   iam.role.assign           user:bob@example.com   SUCCEEDED
e-2    5d ago       serviceAccount:sa-1    compute.instance.create                          DENIED      projects/p1
"
    );
    assert!(wide.contains("DETAILS"), "{wide}");
}

/// Federated credentials are listed by the issuer whose tokens they trust;
/// the audience and conditions are in `-o wide`.
#[tokio::test(flavor = "multi_thread")]
async fn federated_credentials_are_listed_by_issuer_and_state() {
    let credentials = format!(
        r#"{{"federatedCredentials":[
        {{"name":"organizations/o/serviceAccounts/sa-1/federatedCredentials/fc-1",
         "displayName":"GitHub deploy","issuer":"https://token.actions.githubusercontent.com",
         "audience":"https://sts.origincloudapis.eu","conditions":{{"repository_id":"42"}},
         "state":"ACTIVE","creator":"user:ada@example.com","createTime":"{}"}}]}}"#,
        ago(120)
    );
    let (table, wide) = tables(
        credentials,
        "iam",
        &[
            "list-federated-credentials",
            "--organization",
            "o",
            "--service-account",
            "sa-1",
        ],
    )
    .await;
    assert_eq!(
        table,
        "\
NAME   DISPLAY NAME    ISSUER                                        STATE    CREATE TIME
fc-1   GitHub deploy   https://token.actions.githubusercontent.com   ACTIVE   5d ago
"
    );
    assert!(wide.contains("CONDITIONS"), "{wide}");
}

/// Hosted zones are listed by their domain, where they are answered and
/// whether they are; the name servers and tags are in `-o wide`.
#[tokio::test(flavor = "multi_thread")]
async fn hosted_zones_are_listed_by_domain_visibility_and_state() {
    let zones = format!(
        r#"{{"hostedZones":[
        {{"name":"projects/p/hostedZones/z-1","dnsName":"example.com","visibility":"PUBLIC",
         "state":"ACTIVE","nameServers":["ns1.origincloud.eu","ns2.origincloud.eu"],
         "challenge":{{"name":"_origin-challenge.example.com","value":"abc"}},
         "createTime":"{}","tags":{{"env":"prod"}}}},
        {{"name":"projects/p/hostedZones/z-2","dnsName":"corp","visibility":"PRIVATE",
         "state":"ACTIVE","createTime":"{}"}}]}}"#,
        ago(120),
        ago(3)
    );
    let (table, wide) = tables(zones, "dns", &["list-hosted-zones", "--project", "p"]).await;
    assert_eq!(
        table,
        "\
NAME   DNS NAME      VISIBILITY   STATE    CREATE TIME
z-1    example.com   PUBLIC       ACTIVE   5d ago
z-2    corp          PRIVATE      ACTIVE   3h ago
"
    );
    assert!(wide.contains("NAME SERVERS"), "{wide}");
}

/// Billing accounts are listed by name, whether they are the default and
/// where invoices go; the contact person is in `-o wide`, and the address
/// and payment method in the account's own fields (`get-billing-account`).
#[tokio::test(flavor = "multi_thread")]
async fn billing_accounts_are_listed_by_name_default_and_invoice_email() {
    const ACCOUNTS: &str = r#"{"billingAccounts":[
        {"name":"organizations/o/billingAccounts/ba-1","displayName":"default","defaultAccount":true,
         "invoiceEmail":"billing@example.com","vatId":"DE123456789","contactPerson":"Ada Lovelace",
         "billingAddress":{"street":"Hauptstr. 1","city":"Berlin","countryCode":"DE"},
         "paymentMethod":{"invoice":{}}},
        {"name":"organizations/o/billingAccounts/ba-2","displayName":"Research",
         "invoiceEmail":"research@example.com"}]}"#;
    let (table, wide) = tables(
        ACCOUNTS.to_owned(),
        "billing",
        &["list-billing-accounts", "--organization", "o"],
    )
    .await;
    assert_eq!(
        table,
        "\
NAME   DISPLAY NAME   DEFAULT ACCOUNT   INVOICE EMAIL          VAT ID
ba-1   default        true              billing@example.com    DE123456789
ba-2   Research       false             research@example.com
"
    );
    assert!(wide.contains("CONTACT PERSON"), "{wide}");
}

/// A project's usage is listed by interval, resource, metric and quantity;
/// the product and billing account are in `-o wide`.
#[tokio::test(flavor = "multi_thread")]
async fn usage_is_listed_by_interval_resource_metric_and_quantity() {
    let usage = format!(
        r#"{{"usageRecords":[
        {{"resourceKind":"compute.instance","resourceId":"i-1","metric":"running_seconds",
         "productKey":"compute.gp1.core.2","quantity":"3600","startTime":"{}","endTime":"{}",
         "billingAccount":"organizations/o/billingAccounts/ba-1"}}]}}"#,
        ago(5),
        ago(4)
    );
    let (table, wide) = tables(
        usage,
        "billing",
        &[
            "query-project-usage",
            "--project",
            "p",
            "--start-time",
            "2026-11-01T00:00:00Z",
            "--end-time",
            "2026-12-01T00:00:00Z",
        ],
    )
    .await;
    assert_eq!(
        table,
        "\
START TIME   END TIME   RESOURCE KIND      RESOURCE ID   METRIC            QUANTITY
5h ago       4h ago     compute.instance   i-1           running_seconds   3600
"
    );
    assert!(wide.contains("PRODUCT KEY"), "{wide}");
}

/// Support cases are listed by subject and state, with when they were
/// opened and last changed; the description is in `-o wide`.
#[tokio::test(flavor = "multi_thread")]
async fn support_cases_are_listed_by_subject_state_and_when_they_changed() {
    let cases = format!(
        r#"{{"cases":[
        {{"name":"organizations/o/cases/c-1","project":"projects/p1","subject":"Volume stuck attaching",
         "description":"Since this morning the volume v1 stays ATTACHING.\nIt worked yesterday.",
         "contactEmail":"ada@example.com","state":"IN_PROGRESS",
         "createTime":"{}","updateTime":"{}"}}]}}"#,
        ago(120),
        ago(3)
    );
    let (table, wide) = tables(cases, "support", &["list-cases", "--organization", "o"]).await;
    assert_eq!(
        table,
        "\
NAME   SUBJECT                  STATE         PROJECT   CREATE TIME   UPDATE TIME
c-1    Volume stuck attaching   IN_PROGRESS   p1        5d ago        3h ago
"
    );
    assert!(wide.contains("DESCRIPTION"), "{wide}");
}
