use std::sync::Arc;
use tokio::sync::OnceCell;
use serde::{Deserialize, Serialize};
use reqwest::{Client, header};

use crate::error::{Error, Result};

// Global Hasura client
static HASURA_CLIENT: OnceCell<Arc<HasuraClient>> = OnceCell::const_new();

pub struct HasuraClient {
    client: Client,
    endpoint: String,
    admin_secret: String,
}

#[derive(Debug, Serialize)]
struct GraphQLRequest {
    query: String,
    variables: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    operation_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphQLResponse<T> {
    data: Option<T>,
    errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Deserialize)]
struct GraphQLError {
    message: String,
    extensions: Option<serde_json::Value>,
}

impl HasuraClient {
    // Get a singleton instance of the Hasura client
    pub async fn get_instance() -> Result<Arc<Self>> {
        Ok(HASURA_CLIENT.get_or_init(|| async {
            let endpoint = std::env::var("NEXT_PUBLIC_HASURA_ENDPOINT")
                .expect("NEXT_PUBLIC_HASURA_ENDPOINT environment variable not set");
                
            let admin_secret = std::env::var("NEXT_PUBLIC_HASURA_ADMIN_SECRET")
                .expect("NEXT_PUBLIC_HASURA_ADMIN_SECRET environment variable not set");
            
            let mut headers = header::HeaderMap::new();
            headers.insert(
                "X-Hasura-Admin-Secret",
                header::HeaderValue::from_str(&admin_secret).unwrap(),
            );
            
            let client = Client::builder()
                .default_headers(headers)
                .build()
                .expect("Failed to create HTTP client");
            
            Arc::new(Self {
                client,
                endpoint,
                admin_secret,
            })
        }).await.clone())
    }
    
    // Execute a GraphQL query
    pub async fn query<T: for<'de> Deserialize<'de>>(&self, 
        query: &str, 
        variables: serde_json::Value
    ) -> Result<T> {
        let request = GraphQLRequest {
            query: query.to_string(),
            variables,
            operation_name: None,
        };
        
        let response = self.client
            .post(&self.endpoint)
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::DbError(format!("Request error: {}", e)))?;
        
        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::DbError(format!("HTTP error {}: {}", status, error_text)));
        }
        
        let result: GraphQLResponse<T> = response.json().await
            .map_err(|e| Error::DbError(format!("JSON parse error: {}", e)))?;
        
        if let Some(errors) = result.errors {
            if !errors.is_empty() {
                let error_msg = errors.into_iter()
                    .map(|e| e.message)
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(Error::DbError(format!("GraphQL error: {}", error_msg)));
            }
        }
        
        result.data.ok_or_else(|| Error::DbError("No data returned".to_string()))
    }
    
    // Execute a GraphQL mutation
    pub async fn mutate<T: for<'de> Deserialize<'de>>(&self, 
        mutation: &str, 
        variables: serde_json::Value
    ) -> Result<T> {
        self.query(mutation, variables).await
    }
}