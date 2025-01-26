pub struct ResourceManager {
    pub cpu_quota: HashMap<String, u32>,
    pub memory_quota: HashMap<String, u64>,
    pub gpu_quota: HashMap<String, u32>,
}

impl ResourceManager {
    pub fn check_quota(&self, user: &str, config: &VMConfig) -> Result<()> {
        let cpu_usage = self.cpu_quota.get(user).unwrap_or(&0);
        let mem_usage = self.memory_quota.get(user).unwrap_or(&0);
        let gpu_usage = self.gpu_quota.get(user).unwrap_or(&0);

        if (cpu_usage + config.vcpus) > MAX_CPU_QUOTA ||
           (mem_usage + config.memory_kb) > MAX_MEM_QUOTA ||
           (gpu_usage + config.gpu_count) > MAX_GPU_QUOTA 
        {
            return Err(anyhow::anyhow!("Resource quota exceeded"));
        }
        
        Ok(())
    }
} 