#[tokio::test]
async fn test() {
    let config = autoconfig::from_addr("test@163.com").await.unwrap();
    println!("{:#?}", config);
}
