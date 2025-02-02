pub async fn list_gpus(gpupool: Arc<Mutex<GPUPool>>) -> anyhow::Result<()> {
let gpupool = gpupool.lock().await;
          println!("Available GPUs:");
          for (id, gpu) in gpupool.gpus {
              println!("GPU {}: {}MB VRAM - {} Cores", 
                  id, gpu.vram_mb, gpu.compute_units);
          }
Ok(())
      }
pub async fn show_status(gpupool: Arc<Mutex<GPUPool>>) -> anyhow::Result<()> {
let _gpupool = gpupool.lock().await;
          // Implementation details
Ok(())
      }