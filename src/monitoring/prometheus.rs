pub fn register_custom_metrics() {
    let cpu_usage = register_gauge!("cpu_usage_percent", "Current CPU usage");
    let gpu_mem = register_gauge!("gpu_memory_used", "GPU memory usage in MB");
} 