use anyhow::Result;
use gpu_share_vm_manager::gpu::virtual_gpu::GPUPool;
use gpu_share_vm_manager::users::UserManager;
use gpu_share_vm_manager::billing::BillingSystem;
use crossterm::event::{Event, KeyCode};
use crossterm::{execute, terminal::*};
use ratatui::{prelude::*, widgets::*};
use std::sync::{Arc, Mutex};

#[tokio::main]
async fn main() -> Result<()> {
    let gpupool = Arc::new(Mutex::new(GPUPool::new()));
    let user_manager = Arc::new(Mutex::new(UserManager::new()));
    let billing_system = Arc::new(Mutex::new(BillingSystem::new()));
    
    start_dashboard(
        gpupool,
        user_manager,
        billing_system
    ).await
}

pub async fn start_dashboard(
    gpupool: Arc<Mutex<GPUPool>>,
    users: Arc<Mutex<UserManager>>,
    billing: Arc<Mutex<BillingSystem>>
) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    loop {
        terminal.draw(|f| {
            // GPU listesi
            let gpu_list = List::new(
                gpupool.lock().unwrap().gpus.values()
                    .map(|gpu| {
                        let status = if gpu.allocated_to.is_some() {
                            Span::styled("Occupied", Style::new().red())
                        } else {
                            Span::styled("Available", Style::new().green())
                        };
                        ListItem::new(format!(
                            "GPU {}: {}MB - {} Cores - {}",
                            gpu.id, gpu.vram_mb, gpu.compute_units, status
                        ))
                    })
                    .collect::<Vec<_>>()
            )
            .block(Block::default().title("GPUs").borders(Borders::ALL));
            
            // Kullanıcı bilgileri
            let user_list = List::new(
                users.lock().unwrap().users.values()
                    .map(|user| {
                        ListItem::new(format!(
                            "{}: ${:.2}",
                            user.id, user.credits
                        ))
                    })
                    .collect::<Vec<_>>()
            )
            .block(Block::default().title("Users").borders(Borders::ALL));
            
            // Layout düzeni
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(f.size());
            
            f.render_widget(gpu_list, chunks[0]);
            f.render_widget(user_list, chunks[1]);
        })?;
        
        // Input handling
        if crossterm::event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = crossterm::event::read()? {
                if key.code == KeyCode::Char('q') {
                    break;
                }
            }
        }
    }
    
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}