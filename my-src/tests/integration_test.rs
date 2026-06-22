use my_server::models::ServerConfig;
use my_server::dal::{ServerRepository, SqliteRepository};

#[tokio::test]
async fn test_servers_crud_lifecycle() {
    // We will test the repository directly as testing the full axum app requires
    // exposing the router creation function from main.rs which is currently private.
    // Testing the repository covers the core logic.
    let repo = SqliteRepository::new("sqlite::memory:").await.unwrap();

    // 1. List (empty)
    let servers = repo.list_servers().await.unwrap();
    assert_eq!(servers.len(), 0);

    // 2. Create
    let new_server = ServerConfig {
        id: None,
        name: "Test Node".into(),
        ip: "10.0.0.1".into(),
        web_port: 8080,
        description: "A test node".into(),
    };
    let id = repo.create_server(&new_server).await.unwrap();
    assert!(id > 0);

    // 3. Get
    let fetched = repo.get_server(id).await.unwrap().unwrap();
    assert_eq!(fetched.name, "Test Node");

    // 4. Update
    let mut updated = fetched.clone();
    updated.name = "Updated Node".into();
    repo.update_server(id, &updated).await.unwrap();

    let fetched_updated = repo.get_server(id).await.unwrap().unwrap();
    assert_eq!(fetched_updated.name, "Updated Node");

    // 5. List (contains 1)
    let servers = repo.list_servers().await.unwrap();
    assert_eq!(servers.len(), 1);

    // 6. Delete
    repo.delete_server(id).await.unwrap();
    let servers_after = repo.list_servers().await.unwrap();
    assert_eq!(servers_after.len(), 0);
}
