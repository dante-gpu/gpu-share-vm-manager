pub struct ClusterManager {
    nodes: HashMap<String, NodeState>,
}

impl ClusterManager {
    pub async fn distribute_vm(&mut self, config: &VMConfig) -> Result<String> {
        let target_node = self.nodes.values()
            .filter(|n| n.available_resources >= config.requirements)
            .min_by_key(|n| n.current_load)
            .ok_or_else(|| anyhow::anyhow!("No suitable node found"))?;

        let vm_id = self.create_vm_on_node(target_node.id, config).await?;
        self.update_node_state(target_node.id, config);
        Ok(vm_id)
    }
} 