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
