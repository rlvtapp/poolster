//! Discoverable generated-artifact contracts; reexports preserve existing identity.
pub mod graphql {
    pub use crate::GraphqlClient;
}

pub mod events {
    pub use crate::{KafkaClient, KafkaOperationSymbols};
}
