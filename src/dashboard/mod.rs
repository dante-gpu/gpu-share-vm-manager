use anyhow::Result;
use crossterm::{event::{Event, KeyCode}, execute, terminal::*};
use ratatui::{prelude::*, widgets::*};
use std::sync::Arc;
// use tokio::sync::Mutex;
use crate::{gpu::virtual_gpu::GPUPool, users::UserManager, billing::BillingSystem};


pub async fn start_dashboard(
    gpupool: Arc<tokio::sync::Mutex<GPUPool>>,
    users: Arc<tokio::sync::Mutex<UserManager>>,
    _billing: Arc<tokio::sync::Mutex<BillingSystem>>
) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    loop {
        terminal.draw(|f| {
            let gpupool = gpupool.try_lock().unwrap();
            let users = users.try_lock().unwrap();
            
            let gpu_list = List::new(
                gpupool.gpus.values()
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
            
            let user_list = List::new(
                users.users.values()
                    .map(|user| {
                        ListItem::new(format!(
                            "{}: ${:.2}",
                            user.id, user.credits
                        ))
                    })
                    .collect::<Vec<_>>()
            )
            .block(Block::default().title("Users").borders(Borders::ALL));
            
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(f.size());
            
            f.render_widget(gpu_list, chunks[0]);
            f.render_widget(user_list, chunks[1]);
        })?;
        
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

