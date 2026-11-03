//! The GraphQL API's read of the signed-in user, which `origin id login`
//! makes once after a user login: the platform applies a user's pending
//! organization invites on that read.

use crate::client_auth::ConfigAuthenticator;
use crate::graphql::Error as ClientError;
use crate::graphql::GenericGraphQLClient;
use graphql_client::*;
use url::Url;

pub type Result<T> = std::result::Result<T, ClientError>;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/iam/query_current_user.graphql"
)]
pub(crate) struct CurrentUser;

pub(crate) struct IamServiceClient {
    client: GenericGraphQLClient<ConfigAuthenticator>,
}

impl IamServiceClient {
    pub(crate) fn new(api_url: Url) -> Self {
        Self {
            client: GenericGraphQLClient::new(api_url, ConfigAuthenticator::default()),
        }
    }

    /// Reads the signed-in user, which applies their pending invites.
    pub(crate) async fn current_user(&self) -> Result<current_user::ResponseData> {
        self.client
            .call::<CurrentUser>(current_user::Variables)
            .await
    }
}
