use std::sync::Arc;
use tokio::sync::Mutex;
use futures::future::join_all;
use gpu_share_vm_manager::gpu::GPUPool;

#[tokio::test]
async fn test_concurrent_allocations() {
    let pool = Arc::new(Mutex::new(GPUPool::new()));
    let mut handles = vec![];
    
    for i in 0..10 {
        let pool = pool.clone();
        handles.push(tokio::spawn(async move {
            let mut pool = pool.lock().await;
            pool.allocate(&format!("user{}", i), 0)
        }));
    }
    
    let results = join_all(handles).await;
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
}