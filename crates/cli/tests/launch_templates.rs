//! The launch-template commands and `launch-instance --launch-template`
//! against a local server that records every request: what they send, how
//! a template's versions page, and the references they refuse before
//! sending anything.

mod common;

use common::{failure, recording, success};
use fake_server::{Received, Response};
use serde_json::{json, Value};

const TYPE: &str = "//compute.origincloudapis.eu/instanceTypes/gp1.core.2";
const BIGGER_TYPE: &str = "//compute.origincloudapis.eu/instanceTypes/gp1.core.4";
const FAMILY: &str = "//compute.origincloudapis.eu/imageFamilies/debian-12";
const KEY_PAIR: &str = "//compute.origincloudapis.eu/projects/p/keyPairs/k";
const TEMPLATE: &str = "//compute.origincloudapis.eu/projects/p/launchTemplates/web";
const OPERATION: &str = r#"{"name":"projects/p/zones/z/operations/op-1"}"#;

fn sent(request: &Received) -> Value {
    serde_json::from_slice(&request.body).unwrap()
}

/// `create-launch-template` sends the recipe as version 1, its references
/// as the full names given, and the tags beside it.
#[tokio::test(flavor = "multi_thread")]
async fn create_launch_template_sends_the_recipe_and_its_tags() {
    let created = r#"{"name":"projects/p/launchTemplates/web@1","version":1,"defaultVersion":1}"#;
    let server = recording(move |_| Response::json(created)).await;
    let recipe = format!("instance_type={TYPE},image_family={FAMILY},key_pair={KEY_PAIR}");
    let printed = success(
        &server
            .run(
                "compute",
                &[
                    "create-launch-template",
                    "--project",
                    "p",
                    "--launch-template-data",
                    &recipe,
                    "--tag",
                    "origin:name=web",
                ],
            )
            .await,
    );
    assert_eq!(printed["version"], 1, "{printed}");

    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/v1beta1/projects/p/launchTemplates");
    assert_eq!(
        sent(&requests[0]),
        json!({
            "launchTemplateData": {
                "instanceType": TYPE,
                "imageFamily": FAMILY,
                "keyPair": KEY_PAIR,
            },
            "tags": {"origin:name": "web"},
        })
    );

    // A relative name or an id in the recipe is refused, and nothing is sent.
    let relative =
        format!("instance_type={TYPE},image_family={FAMILY},key_pair=projects/p/keyPairs/k");
    let description = failure(
        &server
            .run(
                "compute",
                &[
                    "create-launch-template",
                    "--project",
                    "p",
                    "--launch-template-data",
                    &relative,
                ],
            )
            .await,
    );
    assert!(
        description.contains("is a relative name; give the resource's full name"),
        "{description}"
    );
    assert_eq!(server.requests().len(), 1);
}

/// `list-launch-template-versions` asks again with each page's token until
/// the last page and prints every version as one list, newest first.
#[tokio::test(flavor = "multi_thread")]
async fn list_launch_template_versions_pages_through_every_page() {
    let server = recording(|request| {
        Response::json(match request.query_values("pageToken").first().map(String::as_str) {
            None => {
                r#"{"launchTemplates":[{"name":"projects/p/launchTemplates/web@3","version":3},
                    {"name":"projects/p/launchTemplates/web@2","version":2}],"nextPageToken":"t2"}"#
            }
            Some("t2") => {
                r#"{"launchTemplates":[{"name":"projects/p/launchTemplates/web@1","version":1}]}"#
            }
            Some(other) => panic!("unexpected page token {other}"),
        })
    })
    .await;
    let printed = success(
        &server
            .run(
                "compute",
                &[
                    "list-launch-template-versions",
                    "--launch-template",
                    TEMPLATE,
                ],
            )
            .await,
    );
    let versions: Vec<i64> = printed["launchTemplates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["version"].as_i64().unwrap())
        .collect();
    assert_eq!(versions, [3, 2, 1], "{printed}");

    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    for request in &requests {
        assert_eq!(request.method, "GET");
        assert_eq!(
            request.path,
            "/v1beta1/projects/p/launchTemplates/web:listVersions"
        );
    }
    assert_eq!(requests[1].query_values("pageToken"), ["t2"]);
}

/// `launch-instance` launches from a named version of a template, with
/// the fields given overriding that version's: the override needs none of
/// the keys a recipe requires, and its references are full names.
#[tokio::test(flavor = "multi_thread")]
async fn launch_instance_launches_from_a_named_version_with_an_override() {
    let server = recording(|_| Response::json(OPERATION)).await;
    let server_ref = &server;
    let launch = |template: String, data: String| async move {
        server_ref
            .run(
                "compute",
                &[
                    "launch-instance",
                    "--project",
                    "p",
                    "--zone",
                    "z",
                    "--launch-template",
                    &template,
                    "--launch-template-data",
                    &data,
                    "--no-wait",
                ],
            )
            .await
    };
    let named = format!("{TEMPLATE}@2");
    let printed = success(&launch(named.clone(), format!("instance_type={BIGGER_TYPE}")).await);
    assert_eq!(printed, serde_json::from_str::<Value>(OPERATION).unwrap());

    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert!(
        requests[0]
            .path
            .starts_with("/v1beta1/projects/p/zones/z/instances"),
        "{}",
        requests[0].path
    );
    let body = sent(&requests[0]);
    assert_eq!(body["parent"], "projects/p/zones/z");
    assert_eq!(body["launchTemplate"], named);
    // Only what the override sets is sent: the version's fields are the
    // server's to fill in.
    assert_eq!(
        body["launchTemplateData"],
        json!({"instanceType": BIGGER_TYPE})
    );
    assert!(!body["requestId"].as_str().unwrap().is_empty());

    // `@default` and `@latest` are sent as given; so is a bare name.
    for template in [
        format!("{TEMPLATE}@latest"),
        format!("{TEMPLATE}@default"),
        TEMPLATE.to_owned(),
    ] {
        success(
            &launch(
                template.clone(),
                format!("login_user=ops,instance_type={TYPE}"),
            )
            .await,
        );
        let requests = server.requests();
        assert_eq!(sent(requests.last().unwrap())["launchTemplate"], template);
    }

    // A relative name or an id for the template, or for a reference in the
    // override, is refused before anything is sent.
    let before = server.requests().len();
    let description = failure(&launch("web@2".to_owned(), format!("instance_type={TYPE}")).await);
    assert!(
        description
            .contains("--launch-template: \"web@2\" is an id; give the resource's full name"),
        "{description}"
    );
    let description =
        failure(&launch(named, "instance_type=instanceTypes/gp1.core.4".to_owned()).await);
    assert!(description.contains("is a relative name"), "{description}");
    let description =
        failure(&launch(format!("{TEMPLATE}@next"), format!("instance_type={TYPE}")).await);
    assert!(
        description.contains("is neither a number, `default` nor `latest`"),
        "{description}"
    );
    // Another resource's name takes no version.
    let description = failure(
        &server
            .run(
                "compute",
                &[
                    "launch-instance",
                    "--project",
                    "p",
                    "--zone",
                    "z",
                    "--launch-template-data",
                    &format!("instance_type={TYPE},key_pair={KEY_PAIR}@2,image_family={FAMILY}"),
                    "--no-wait",
                ],
            )
            .await,
    );
    assert!(description.contains("key_pair"), "{description}");
    assert_eq!(server.requests().len(), before);
}

const SUBNET: &str = "//network.origincloudapis.eu/projects/p/zones/z/subnets/web";
const GROUP: &str = "//network.origincloudapis.eu/projects/p/regions/r/securityGroups/web";
const OTHER_GROUP: &str = "//network.origincloudapis.eu/projects/p/regions/r/securityGroups/db";

/// `launch-instance`'s flags for the common launch build the recipe without
/// JSON: the instance type, image, key pair, and the subnet and security
/// groups of one network interface the launch creates.
#[tokio::test(flavor = "multi_thread")]
async fn launch_instance_flags_build_the_recipe() {
    let server = recording(|_| Response::json(OPERATION)).await;
    let server_ref = &server;
    let launch = |extra: &[&str]| {
        let args: Vec<String> = [
            "launch-instance",
            "--project",
            "p",
            "--zone",
            "z",
            "--no-wait",
        ]
        .iter()
        .chain(extra)
        .map(|a| (*a).to_owned())
        .collect();
        async move {
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            server_ref.run("compute", &args).await
        }
    };
    success(
        &launch(&[
            "--instance-type",
            TYPE,
            "--image-family",
            FAMILY,
            "--key-pair",
            KEY_PAIR,
            "--subnet",
            SUBNET,
            "--security-group",
            GROUP,
            "--security-group",
            OTHER_GROUP,
        ])
        .await,
    );
    assert_eq!(
        sent(&server.requests()[0])["launchTemplateData"],
        json!({
            "instanceType": TYPE,
            "imageFamily": FAMILY,
            "keyPair": KEY_PAIR,
            "networkInterfaces": [{"spec": {
                "subnet": SUBNET,
                "securityGroups": [GROUP, OTHER_GROUP],
            }}],
        })
    );

    // On top of a template, and beside the recipe's other keys.
    success(
        &launch(&[
            "--launch-template",
            TEMPLATE,
            "--launch-template-data",
            "login_user=ops",
            "--instance-type",
            BIGGER_TYPE,
        ])
        .await,
    );
    let body = sent(&server.requests()[1]);
    assert_eq!(body["launchTemplate"], TEMPLATE);
    assert_eq!(
        body["launchTemplateData"],
        json!({"loginUser": "ops", "instanceType": BIGGER_TYPE})
    );

    let before = server.requests().len();
    // The recipe's own key for the same field, or the interfaces it lists,
    // is refused, and so are an image and a family together, a name that
    // isn't a full name, and a security group that is another type's.
    let description = failure(
        &launch(&[
            "--launch-template-data",
            &format!("instance_type={TYPE}"),
            "--instance-type",
            BIGGER_TYPE,
        ])
        .await,
    );
    assert!(
        description.contains("--instance-type can't be combined"),
        "{description}"
    );
    let description = failure(
        &launch(&[
            "--launch-template-data-json",
            r#"{"networkInterfaces":[{"networkInterface":"//network.origincloudapis.eu/projects/p/zones/z/networkInterfaces/n"}]}"#,
            "--subnet",
            SUBNET,
        ])
        .await,
    );
    assert!(
        description.contains("--subnet can't be combined"),
        "{description}"
    );
    let image = "//compute.origincloudapis.eu/projects/p/zones/z/images/i";
    let output = launch(&["--image", image, "--image-family", FAMILY]).await;
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("cannot be used with"), "{stderr}");
    let description = failure(&launch(&["--subnet", "web"]).await);
    assert!(description.contains("subnet"), "{description}");
    assert_eq!(server.requests().len(), before);
}
