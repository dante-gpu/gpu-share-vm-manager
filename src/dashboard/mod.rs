use anyhow::Result;
use crossterm::{event::{Event, KeyCode}, execute, terminal::*};
use ratatui::{
    prelude::*,
    widgets::*,
    style::{Color, Modifier},
    text::{Line, Span},
    widgets::BorderType,
};
use std::sync::Arc;
use crate::gpu::GPUPool;
use crate::users::UserManager;
use crate::billing::BillingSystem;

pub async fn start_dashboard(
    gpupool: Arc<tokio::sync::Mutex<GPUPool>>,
    users: Arc<tokio::sync::Mutex<UserManager>>,
    _billing: Arc<tokio::sync::Mutex<BillingSystem>>
) -> Result<()> {
    // ASCII
    let dante_ascii = vec![
        r#"██████╗  █████╗ ███╗   ██╗████████╗███████╗"#,
        r#"██╔══██╗██╔══██╗████╗  ██║╚══██╔══╝██╔════╝"#,
        r#"██║  ██║███████║██╔██╗ ██║   ██║   █████╗  "#,
        r#"██║  ██║██╔══██║██║╚██╗██║   ██║   ██╔══╝  "#,
        r#"██████╔╝██║  ██║██║ ╚████║   ██║   ███████╗"#,
        r#"╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═══╝   ╚═╝   ╚══════╝"#,
    ];

    let menu_items = vec![
        "GPU Allocation",
        "User Management",
        "Billing Overview",
        "System Metrics",
        "Cluster Nodes",
        "Exit",
    ];
    
    let mut selected_menu = 0;

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    loop {
        terminal.draw(|f| {
            let gpupool = gpupool.try_lock().unwrap();
            let users = users.try_lock().unwrap();
            
            // Ana layout
            let main_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(8),  // Header
                    Constraint::Min(5),     // Main content
                    Constraint::Length(3),  // Footer
                ])
                .split(f.size());

            // Header
            let header_block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::new().fg(Color::LightBlue))
                .border_type(BorderType::Thick);
            
            let header_text: Vec<Line> = dante_ascii.iter()
                .map(|s| Line::from(*s).style(Style::new().fg(Color::LightBlue)))
                .collect();
            
            f.render_widget(
                Paragraph::new(header_text)
                    .block(header_block)
                    .alignment(Alignment::Center),
                main_layout[0]
            );

            // Ana içerik
            let content_layout = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
                .split(main_layout[1]);

            // Menü paneli
            let menu = List::new(
                menu_items.iter().enumerate().map(|(i, item)| {
                    let style = if i == selected_menu {
                        Style::new()
                            .fg(Color::Black)
                            .bg(Color::LightBlue)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::new().fg(Color::White)
                    };
                    ListItem::new(Span::styled(format!("▶ {} ", item), style))
                })
            ).block(
                Block::default()
                    .title("Main Menu")
                    .borders(Borders::ALL)
                    .border_style(Style::new().fg(Color::LightBlue))
            );

            // Detay paneli
            let detail_block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::new().fg(Color::LightBlue))
                .title(match selected_menu {
                    0 => "GPU Allocation",
                    1 => "User Management",
                    2 => "Billing Overview",
                    3 => "System Metrics",
                    4 => "Cluster Nodes",
                    _ => "Dashboard",
                });
            
            let detail_content = match selected_menu {
                0 => render_gpu_panel(&gpupool),
                1 => render_user_panel(&users),
                // Diğer menü öğeleri için render fonksiyonları...
                _ => Paragraph::new(""),
            };

            f.render_widget(menu, content_layout[0]);
            f.render_widget(
                detail_content.block(detail_block).to_owned(),
                content_layout[1]
            );

            // Footer
            let footer = Paragraph::new(Line::from(vec![
                Span::styled("Q: Quit", Style::new().fg(Color::LightYellow)),
                Span::raw(" | "),
                Span::styled("↑↓: Navigate", Style::new().fg(Color::LightGreen)),
                Span::raw(" | "),
                Span::styled("Enter: Select", Style::new().fg(Color::LightMagenta)),
            ])).alignment(Alignment::Center);
            
            f.render_widget(footer, main_layout[2]);
        })?;
        
        if crossterm::event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = crossterm::event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Up => selected_menu = selected_menu.saturating_sub(1),
                    KeyCode::Down => selected_menu = (selected_menu + 1).min(menu_items.len() - 1),
                    KeyCode::Enter => if let Some(()) = handle_menu_selection(selected_menu) {},
                    _ => {}
                }
            }
        }
    }
    
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}

fn render_gpu_panel(gpupool: &GPUPool) -> Paragraph {
    let lines: Vec<Line> = gpupool.gpus.values()
        .map(|gpu| {
            let status = if gpu.allocated_to.is_some() {
                Span::styled("● Busy", Style::new().fg(Color::Red))
            } else {
                Span::styled("○ Free", Style::new().fg(Color::Green))
            };
            
            Line::from(vec![
                Span::styled(format!("GPU {} ", gpu.id), Style::new().fg(Color::Cyan)),
                Span::raw(format!("VRAM: {}MB ", gpu.vram_mb)),
                Span::raw(format!("Cores: {} ", gpu.compute_units)),
                status,
            ])
        })
        .collect();

    Paragraph::new(lines)
}

fn render_user_panel(users: &UserManager) -> Paragraph {
    let lines: Vec<Line> = users.users.values()
        .map(|user| {
            Line::from(vec![
                Span::styled(user.id.to_string(), Style::new().fg(Color::Yellow)),
                Span::raw(" - Credits: "),
                Span::styled(
                    format!("${:.2}", user.credits),
                    Style::new().fg(Color::LightGreen)
                ),
            ])
        })
        .collect();
    
    Paragraph::new(lines)
}

fn handle_menu_selection(selected: usize) -> Option<()> {
    match selected {
        5 => std::process::exit(0),
        _ => None
    }
}

