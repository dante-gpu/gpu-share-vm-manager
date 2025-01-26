pub struct AnomalyDetector {
    thresholds: HashMap<MetricType, f64>,
}

impl AnomalyDetector {
    pub fn analyze_metrics(&self, metrics: &[ResourceMetrics]) -> Vec<Anomaly> {
        metrics.iter().filter_map(|m| {
            let cpu_anomaly = m.cpu_usage_percent > *self.thresholds.get(&MetricType::Cpu)?;
            let mem_anomaly = m.memory_usage_mb as f64 / m.memory_total_mb as f64 > 
                *self.thresholds.get(&MetricType::Memory)?;
            
            if cpu_anomaly || mem_anomaly {
                Some(Anomaly::new(m.timestamp, cpu_anomaly, mem_anomaly))
            } else {
                None
            }
        }).collect()
    }
} 