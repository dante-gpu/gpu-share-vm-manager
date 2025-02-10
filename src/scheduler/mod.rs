pub async fn schedule_workload(
    &self,
    workload: AIWorkload,
    priority: Priority,
) -> Result<JobId, SchedulerError> {
    let job = Job::new(workload)
        .with_priority(priority)
        .with_resource_requirements(ResourceEstimation::from(workload));
    
    self.queue.enqueue(job).await
} 