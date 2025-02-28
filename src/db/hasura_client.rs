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
            // 打印环境变量信息，便于调试
            println!("Initializing Hasura client...");
            
            let endpoint = match std::env::var("NEXT_PUBLIC_HASURA_ENDPOINT") {
                Ok(val) => {
                    println!("Found HASURA_ENDPOINT: {}", val);
                    val
                },
                Err(_) => {
                    let fallback = "http://localhost:8080/v1/graphql".to_string();
                    println!("NEXT_PUBLIC_HASURA_ENDPOINT not set, using fallback: {}", fallback);
                    fallback
                }
            };
                
            let admin_secret = match std::env::var("NEXT_PUBLIC_HASURA_ADMIN_SECRET") {
                Ok(val) => {
                    println!("Found HASURA_ADMIN_SECRET: {}", 
                             if val.is_empty() { "empty string" } else { "[redacted]" });
                    val
                },
                Err(_) => {
                    let fallback = "dev_secret".to_string();
                    println!("NEXT_PUBLIC_HASURA_ADMIN_SECRET not set, using fallback");
                    fallback
                }
            };
            
            let mut headers = header::HeaderMap::new();
            headers.insert(
                "X-Hasura-Admin-Secret",
                header::HeaderValue::from_str(&admin_secret).unwrap(),
            );
            
            let client = Client::builder()
                .default_headers(headers)
                .build()
                .expect("Failed to create HTTP client");
            
            println!("Hasura client initialized with endpoint: {}", endpoint);
            
            Arc::new(Self {
                client,
                endpoint,
                admin_secret,
            })
        }).await.clone())
    }
    
    // Execute a GraphQL query with improved error handling and logging
    pub async fn query<T: for<'de> Deserialize<'de>>(&self, 
        query: &str, 
        variables: serde_json::Value
    ) -> Result<T> {
        // Log the request
        let operation_type = if query.trim().starts_with("mutation") {
            "Mutation"
        } else if query.trim().starts_with("query") {
            "Query"
        } else {
            "Unknown"
        };
        
        println!("Executing GraphQL {}: \n{}\nWith variables: {}", 
                 operation_type, query, variables);
        
        let request = GraphQLRequest {
            query: query.to_string(),
            variables: variables.clone(),
            operation_name: None,
        };
        
        let start = std::time::Instant::now();
        let response = self.client
            .post(&self.endpoint)
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                println!("HTTP Request Error: {}", e);
                Error::DbError(format!("Request error: {}", e))
            })?;
        
        let status = response.status();
        println!("GraphQL response status: {}", status);
        
        if !status.is_success() {
            let error_text = response.text().await
                .unwrap_or_else(|_| "Unknown error".to_string());
            println!("HTTP Error response: {}", error_text);
            return Err(Error::DbError(format!("HTTP error {}: {}", status, error_text)));
        }
        
        // Parse JSON response
        let response_text = response.text().await
            .map_err(|e| {
                println!("Failed to get response text: {}", e);
                Error::DbError(format!("Failed to get response text: {}", e))
            })?;
        
        println!("Response body: {}", response_text);
        
        let result: GraphQLResponse<T> = serde_json::from_str(&response_text)
            .map_err(|e| {
                println!("JSON parse error: {}", e);
                Error::DbError(format!("JSON parse error: {}", e))
            })?;
        
        let elapsed = start.elapsed();
        println!("GraphQL request completed in {:?}", elapsed);
        
        // Handle GraphQL errors
        if let Some(errors) = result.errors {
            if !errors.is_empty() {
                let error_msg = errors.into_iter()
                    .map(|e| {
                        let ext_str = e.extensions.map_or_else(
                            || "".to_string(),
                            |v| format!(" - Extensions: {}", v)
                        );
                        format!("{}{}", e.message, ext_str)
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                println!("GraphQL Errors: {}", error_msg);
                return Err(Error::DbError(format!("GraphQL error: {}", error_msg)));
            }
        }
        
        // Handle data
        match &result.data {
            Some(_) => println!("GraphQL request successful with data"),
            None => println!("GraphQL request returned no data")
        }
        
        result.data.ok_or_else(|| Error::DbError("No data returned".to_string()))
    }
    
    // Execute a GraphQL mutation (same as query for code reuse)
    pub async fn mutate<T: for<'de> Deserialize<'de>>(&self, 
        mutation: &str, 
        variables: serde_json::Value
    ) -> Result<T> {
        self.query(mutation, variables).await
    }
}