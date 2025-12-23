use crossterm::{
    ExecutableCommand,
    event::{self, KeyCode, KeyEventKind},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use rand::Rng;
use ratatui::{
    prelude::*,
    style::Palette,
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Wrap},
};
use std::{
    io::stdout,
    time::{Duration, Instant},
};

struct App {
    tps_history: Vec<u64>,
    logs: Vec<String>,
    orderbook_bids: Vec<(f64, f64)>,
    orderbook_asks: Vec<(f64, f64)>,
    start_time: Instant,
    total_orders: u64,
}

impl App {
    fn new() -> Self {
        Self {
            tps_history: vec![0; 100],
            logs: vec![],
            orderbook_bids: (0..15).map(|i| (95000.0 - i as f64 * 5.0, 1.0)).collect(),
            orderbook_asks: (0..15).map(|i| (95005.0 + i as f64 * 5.0, 1.0)).collect(),
            start_time: Instant::now(),
            total_orders: 0,
        }
    }

    fn on_tick(&mut self) {
        let mut rng = rand::thread_rng();

        // Simulate 1.3M TPS + jitter
        let current_tps = rng.gen_range(1_250_000..1_450_000);
        self.tps_history.push(current_tps);
        if self.tps_history.len() > 100 {
            self.tps_history.remove(0);
        }

        // Accumulate total
        self.total_orders += current_tps / 10; // assuming 100ms tick

        // Simulation logs
        if rng.gen_bool(0.3) {
            let log_type = if rng.gen_bool(0.9) { "INFO" } else { "WARN" };
            let module = if rng.gen_bool(0.5) { "ME" } else { "UBSC" };
            self.logs.push(format!(
                "{} [{}] OrderId={} latency={}ns",
                log_type,
                module,
                rng.gen_range(10000000..99999999),
                rng.gen_range(100..4000)
            ));
        }
        if self.logs.len() > 20 {
            self.logs.remove(0);
        }

        // Simulate Orderbook flicker
        for bid in &mut self.orderbook_bids {
            bid.1 = rng.gen_range(0.1..5.0);
        }
        for ask in &mut self.orderbook_asks {
            ask.1 = rng.gen_range(0.1..5.0);
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    stdout().execute(EnterAlternateScreen)?;
    enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let mut app = App::new();
    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|frame| ui(frame, &mut app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            if let event::Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press && key.code == KeyCode::Char('q') {
                    break;
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            app.on_tick();
            last_tick = Instant::now();
        }
    }

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

fn ui(frame: &mut Frame, app: &mut App) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Content
            Constraint::Length(3), // Footer
        ])
        .split(frame.size());

    // 1. Header
    let title = Paragraph::new(
        " ⚔️  0xInfinity Engine Monitor (v0.10.0) | Mode: Multi-Threaded | Core: ring-buffer ",
    )
    .style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(title, layout[0]);

    // 2. Main Content
    let main_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30), // Stats
            Constraint::Percentage(40), // OrderBook
            Constraint::Percentage(30), // Logs
        ])
        .split(layout[1]);

    // 2.1 Stats
    let current_tps = *app.tps_history.last().unwrap_or(&0);
    let tps_text = format!("{:.2} M", current_tps as f64 / 1_000_000.0);

    let stats_block = Block::default()
        .title(" System Metrics ")
        .borders(Borders::ALL);
    let stats_text = vec![
        Line::from(vec![
            Span::raw("TPS: "),
            Span::styled(
                tps_text,
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("Total: "),
            Span::raw(format!(
                "{:.2} B",
                app.total_orders as f64 / 1_000_000_000.0
            )),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("CPU: "),
            Span::styled("100% (Core 1-4)", Style::default().fg(Color::Red)),
        ]),
        Line::from(vec![Span::raw("Mem: "), Span::raw("1.2 GB")]),
        Line::from(""),
        Line::from(vec![
            Span::raw("P99: "),
            Span::styled("188 us", Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::raw("P50: "),
            Span::styled("14 us", Style::default().fg(Color::Green)),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(stats_text).block(stats_block),
        main_layout[0],
    );

    // 2.2 OrderBook
    let ob_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(main_layout[1]);

    let asks: Vec<ListItem> = app
        .orderbook_asks
        .iter()
        .rev()
        .map(|(p, q)| {
            ListItem::new(format!("{:.2}  |  {:.4}", p, q)).style(Style::default().fg(Color::Red))
        })
        .collect();
    frame.render_widget(
        List::new(asks).block(Block::default().title(" Asks ").borders(Borders::BOTTOM)),
        ob_layout[0],
    );

    let bids: Vec<ListItem> = app
        .orderbook_bids
        .iter()
        .map(|(p, q)| {
            ListItem::new(format!("{:.2}  |  {:.4}", p, q)).style(Style::default().fg(Color::Green))
        })
        .collect();
    frame.render_widget(
        List::new(bids).block(Block::default().title(" Bids ").borders(Borders::TOP)),
        ob_layout[1],
    );

    // 2.3 Logs
    let logs: Vec<ListItem> = app
        .logs
        .iter()
        .map(|l| ListItem::new(l.as_str()).style(Style::default().fg(Color::DarkGray)))
        .collect();
    frame.render_widget(
        List::new(logs).block(
            Block::default()
                .title(" Event Stream ")
                .borders(Borders::ALL),
        ),
        main_layout[2],
    );

    // 3. Footer
    let footer = Paragraph::new("Press 'q' to exit | Status: HEALTHY | Uptime: 14d 2h 11m")
        .style(Style::default().fg(Color::Gray))
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(footer, layout[2]);
}
