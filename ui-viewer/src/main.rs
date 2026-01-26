use anyhow::Result;
use clap::{Parser, Subcommand};
use slint::{ComponentHandle, VecModel};
use std::rc::Rc;
use slint::{Image, Color, Brush};

slint::include_modules!();

#[derive(Parser)]
#[command(name = "ui-viewer")]
#[command(about = "Standalone Slint UI component viewer and testing tool")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Preview {
        #[arg(short, long, default_value = "social-status")]
        component: String,
    },
}

fn main() -> Result<()> {
    use tracing_subscriber::EnvFilter;
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let cli = Cli::parse();

    let app = ViewerWindow::new()?;
    
    setup_mock_data(&app);

    if let Some(Commands::Preview { component }) = cli.command {
        app.set_current_view(component.into());
    }

    app.run()?;
    Ok(())
}

fn setup_mock_data(app: &ViewerWindow) {
    let gs = app.global::<GameState>();
    let ss = app.global::<SocialStatusState>();
    let sets = app.global::<SettingsState>();
    
    // Mock Social Status
    ss.set_current_status(0);
    
    let weak_app = app.as_weak();
    ss.on_set_status(move |s| {
        if let Some(app) = weak_app.upgrade() {
            app.global::<SocialStatusState>().set_current_status(s);
        }
    });

    // Mock Game State
    gs.set_player_name("Talgonite Warrior".into());
    gs.set_map_name("Developer Sanctum".into());
    gs.set_current_hp(1250);
    gs.set_max_hp(1500);
    gs.set_current_mp(450);
    gs.set_max_mp(600);
    
    // Mock Inventory
    let inv_model = Rc::new(VecModel::default());
    for i in 1..=30 {
        inv_model.push(InventoryItem {
            slot: i,
            name: format!("Item {}", i).into(),
            icon: Image::default(),
            quantity: if i % 5 == 0 { 10 } else { 1 },
        });
    }
    gs.set_inventory(inv_model.into());

    // Mock Skills
    let skill_model = Rc::new(VecModel::default());
    for i in 1..=10 {
        skill_model.push(Skill {
            name: format!("Skill {}", i).into(),
            icon: Image::default(),
            slot: i,
            cooldown: Cooldown {
                time_left: 0,
                total: 5000,
            },
        });
    }
    gs.set_skills(skill_model.into());

    // Mock World List
    let world_model = Rc::new(VecModel::default());
    let names = ["Dean", "Arion", "Myla", "Zorn", "Kael"];
    for (i, name) in names.iter().enumerate() {
        world_model.push(WorldListMemberUi {
            name: (*name).into(),
            title: "Arch-Mage".into(),
            class: "Wizard".into(),
            color: Color::from_rgb_u8(100, 150, 255).into(),
            is_master: i == 0,
            social_status: (i % 8) as i32,
        });
    }
    gs.set_world_list_members(world_model.into());
    gs.set_world_list_count(5);
    gs.set_world_list_total_count(100);

    // Mock Chat
    let chat_model = Rc::new(VecModel::default());
    chat_model.push(ChatMessage {
        text: "System: Developer mode active.".into(),
        color: Color::from_rgb_u8(255, 200, 0).into(),
    });
    chat_model.push(ChatMessage {
        text: "Global [Dean]: Hello world!".into(),
        color: Color::from_rgb_u8(200, 200, 200).into(),
    });
    gs.set_chat_messages(chat_model.into());

    // Mock Hotbar
    let hotbar_model = Rc::new(VecModel::default());
    for _ in 0..36 {
        hotbar_model.push(HotbarEntry {
            name: "".into(),
            icon: Image::default(),
            quantity: 0,
            enabled: true,
            cooldown: Cooldown { time_left: 0, total: 0 },
        });
    }
    gs.set_hotbar(hotbar_model.into());

    // Callbacks
    gs.on_set_hotbar_panel(|p| {
        tracing::info!("Hotbar panel set to {}", p);
    });
    
    sets.set_show_settings(false);
}
