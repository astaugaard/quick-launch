use std::{
    collections::HashMap, fs, os::unix::process::CommandExt, path::PathBuf,
    process::Command, rc::Rc,
};

use clap::Parser;
use gio::{
    glib::{Char, OptionArg, OptionFlags, Propagation},
    prelude::*,
    ApplicationFlags,
};
use gtk4::{
    gdk::{Display, Key},
    prelude::*,
    Align, Button, CssProvider, EventControllerKey, GestureClick, Grid, Image, Label, Orientation,
    Stack, Widget,
};

use gtk4_layer_shell::{Edge, Layer, LayerShell};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    config: String,

    #[arg(short = 's', long)]
    css: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
enum App {
    Folder {
        name: String,
        applications: HashMap<String, App>,
    },
    Application {
        command: PathBuf,
        name: String,
        icon: String,
    },
}

#[derive(Serialize, Deserialize, Debug)]
struct Config {
    width: usize,
    height: usize,
    applications: HashMap<String, App>,
}

#[derive(Debug, Clone)]
enum VApp {
    Folder(String, Rc<HashMap<Key, VApp>>),
    Application(PathBuf, String, String),
}

struct VConfig {
    width: usize,
    applications: Rc<HashMap<Key, VApp>>,
}

fn activate(application: &gtk4::Application, config: &VConfig) {
    let window = gtk4::ApplicationWindow::new(application);

    window.init_layer_shell();

    window.set_layer(Layer::Top);

    window.fullscreen();

    window.set_keyboard_mode(gtk4_layer_shell::KeyboardMode::Exclusive);

    let anchors = [
        (Edge::Left, true),
        (Edge::Right, true),
        (Edge::Top, true),
        (Edge::Bottom, true),
    ];

    for (anchor, state) in anchors {
        window.set_anchor(anchor, state);
    }

    let click_gesture = GestureClick::builder().button(0).build();
    let key = EventControllerKey::new();

    let win2 = window.clone();

    click_gesture.connect_pressed(move |_, _, _, _| {
        win2.close();
    });

    let win2 = window.clone();

    let stack = Stack::builder()
        .valign(Align::Center)
        .halign(Align::Center)
        .build();

    key.connect_key_pressed(move |_, key, _, _| {
        println!("top level key");
        match key {
            Key::Escape => {
                win2.close();
                println!("hello world");
                Propagation::Stop
            }
            _ => Propagation::Stop,
        }
    });

    window.add_controller(click_gesture);
    window.add_controller(key);

    let grid = make_application_grid(config.applications.clone(), config.width, stack.clone());

    stack.add_child(&grid);
    stack.set_visible_child(&grid);

    window.set_child(Some(&stack));

    window.set_visible(true)
}

fn make_application_grid(
    applications: Rc<HashMap<Key, VApp>>,
    width: usize,
    // window: &gtk4::ApplicationWindow,
    stack: Stack,
) -> Widget {
    let grid = Grid::new();

    let key = EventControllerKey::new();

    let app2 = applications.clone();
    let stack2 = stack.clone();

    key.connect_key_pressed(move |_, key, _, _| {
        println!("key");
        match app2.get(&key) {
            Some(launch) => {
                println!("launch key");
                switch_to(launch, &stack2, width);
                Propagation::Stop
            }
            None => Propagation::Proceed,
        }
    });

    grid.add_controller(key);

    let mut row = 0;
    let mut col = 0i32;

    for (key, launch) in applications.iter() {
        let widget = create_square(*key, launch.clone(), stack.clone(), width);

        grid.attach(&widget, col, row, 1, 1);

        col += 1;
        col %= width as i32;

        if col == 0 {
            row += 1;
        }
    }

    grid.into()
}

fn switch_to(launch: &VApp, stack: &Stack, width: usize) {
    match &launch {
        VApp::Folder(_, sub) => {
            let application_grid = make_application_grid(sub.clone(), width, stack.clone());
            stack.add_child(&application_grid);
            stack.set_visible_child(&application_grid);
        }
        VApp::Application(command, _, _) => {
            Command::new(command).exec();
        }
    }
}

fn create_square(key: Key, launch: VApp, stack: Stack, width: usize) -> Widget {
    let button = Button::new();

    let boxo = gtk4::Box::new(Orientation::Vertical, 5);

    let image = Image::from_icon_name(match &launch {
        VApp::Folder(_, _) => "folder",
        VApp::Application(_, _, icon) => icon.as_str(),
    });

    image.set_icon_size(gtk4::IconSize::Large);

    boxo.append(&image);

    boxo.append(&Label::new(Some(&format!(
        "({}) {}",
        key.name().unwrap(),
        match &launch {
            VApp::Folder(name, _) => name,
            VApp::Application(_, name, _) => name,
        }
    ))));

    button.set_child(Some(&boxo));

    button.connect_clicked(move |_| switch_to(&launch, &stack, width));

    button.into()
}

fn validate_map(applications: HashMap<String, App>) -> Result<Rc<HashMap<Key, VApp>>> {
    Ok(Rc::new(
        applications
            .into_iter()
            .map(|(k, v)| -> Result<(Key, VApp)> {
                Ok((
                    Key::from_name(k).context("failed to parse Key name")?,
                    match v {
                        App::Folder { name, applications } => {
                            VApp::Folder(name, validate_map(applications)?)
                        }
                        App::Application {
                            command,
                            name,
                            icon,
                        } => VApp::Application(command, name, icon),
                    },
                ))
            })
            .collect::<Result<_>>()?,
    ))
}

fn validate_config(config: Config) -> Result<VConfig> {
    Ok(VConfig {
        width: config.width,
        applications: validate_map(config.applications)?,
    })
}

fn main() -> Result<()> {
    let Args { css, config } = dbg!(Args::parse());

    let config = fs::read_to_string(&config)
        .with_context(|| format!("failed to read config at {}", config))?;

    let config = serde_json::from_str(&config)
        .with_context(|| format!("failed to parse config (path: {})", config))?;

    let config = validate_config(config).with_context(|| "failed to validate config")?;

    let mut flags = ApplicationFlags::empty();

    flags.set(ApplicationFlags::HANDLES_COMMAND_LINE, true);

    let application = gtk4::Application::builder().build();

    application.add_main_option(
        "config",
        Char::from(b'c'),
        OptionFlags::NONE,
        OptionArg::String,
        "application configuration",
        Some("COMMAND"),
    );

    application.add_main_option(
        "css",
        Char::from(b's'),
        OptionFlags::NONE,
        OptionArg::String,
        "style sheet to use",
        Some("STYLE"),
    );

    application.connect_command_line(|_app, _cli| 0); // idk why I have to do this tbh

    application.connect_startup(move |_| {
        let provider = CssProvider::new();

        match &css {
            Some(file) => {
                provider.load_from_path(file);
            }
            None => {
                provider.load_from_string(include_str!("style.css"));
            }
        };

        gtk4::style_context_add_provider_for_display(
            &Display::default().expect("could not connect to a display."),
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        )
    });

    application.connect_activate(move |app| {
        activate(app, &config);
    });

    application.run();

    Ok(())
}
