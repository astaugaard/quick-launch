use std::{
    collections::HashMap,
    fs,
    os::unix::process::CommandExt,
    path::PathBuf,
    process::{self, Command},
    rc::Rc,
};

use clap::Parser;
use gio::{
    glib::{Char, OptionArg, OptionFlags, Propagation},
    prelude::*,
    ApplicationFlags, DesktopAppInfo,
};
use gtk4::{
    gdk::{AppLaunchContext, Display, Key, Paintable, Texture},
    prelude::*,
    Align, Button, CssProvider, EventControllerKey, GestureClick, Grid, IconLookupFlags, IconTheme,
    Image, Label, Orientation, Picture, Stack, TextDirection, Widget,
};

use gtk4_layer_shell::{Edge, Layer, LayerShell};

use anyhow::{Context, Result};
use once_cell::unsync::{Lazy, OnceCell};
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
#[serde(untagged)]
enum App {
    Folder {
        name: String,
        icon: Option<String>,
        image: Option<String>,
        applications: HashMap<String, App>,
    },
    Application {
        command: Option<String>,
        name: Option<String>,
        icon: Option<String>,
        image: Option<String>,
        application: Option<String>,
    },
}

fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize, Debug)]
struct Config {
    width: usize,
    height: usize,

    icon_size: usize,

    #[serde(default = "default_true")]
    anchor_left: bool,

    #[serde(default = "default_true")]
    anchor_right: bool,

    #[serde(default = "default_true")]
    anchor_up: bool,

    #[serde(default = "default_true")]
    anchor_down: bool,

    applications: HashMap<String, App>,
}

type Name = String;

#[derive(Clone)]
enum VApp {
    Folder {
        name: Name,
        image: Paintable,
        applications: Rc<HashMap<Key, VApp>>,
    },
    Application {
        name: Name,
        image: Paintable,
        command: Rc<dyn Fn() -> Result<()>>,
    },
}

struct VConfig {
    width: usize,
    icon_size: i32,
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

    click_gesture.connect_released(move |_, _, _, _| {
        win2.close();
    });

    let win2 = window.clone();

    let stack = Stack::builder()
        .valign(Align::Center)
        .halign(Align::Center)
        .interpolate_size(true)
        .vhomogeneous(false)
        .build();

    key.connect_key_pressed(move |_, key, _, _| {
        println!("top level key");
        match key {
            Key::Escape => {
                win2.close();
                println!("hello world");
                Propagation::Stop
            }
            _ => Propagation::Proceed,
        }
    });

    window.add_controller(click_gesture);
    window.add_controller(key);

    let grid = make_application_grid(
        config.applications.clone(),
        config.width,
        stack.clone(),
        config.icon_size,
    );

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
    icon_size: i32,
) -> Widget {
    let grid = Grid::builder()
        .row_spacing(20)
        .column_spacing(20)
        .row_homogeneous(true)
        .column_homogeneous(true)
        .build();

    let key = EventControllerKey::new();

    let app2 = applications.clone();
    let stack2 = stack.clone();

    key.connect_key_pressed(move |_, key, _, _| {
        println!("key");
        match app2.get(&key) {
            Some(launch) => {
                println!("launch key");
                switch_to(launch, &stack2, width, icon_size);
                Propagation::Stop
            }
            None => Propagation::Proceed,
        }
    });

    grid.add_controller(key);

    let mut row = 0;
    let mut col = 0i32;

    for (key, launch) in applications.iter() {
        let widget = create_square(*key, launch.clone(), stack.clone(), width, icon_size);

        grid.attach(&widget, col, row, 1, 1);

        col += 1;
        col %= width as i32;

        if col == 0 {
            row += 1;
        }
    }

    grid.into()
}

fn switch_to(launch: &VApp, stack: &Stack, width: usize, icon_size: i32) {
    match &launch {
        VApp::Folder { applications, .. } => {
            let application_grid =
                make_application_grid(applications.clone(), width, stack.clone(), icon_size);
            stack.add_child(&application_grid);
            stack.set_visible_child(&application_grid);
        }
        VApp::Application { command, .. } => match command() {
            Ok(()) => {}
            Err(err) => {
                eprintln!("error launching application: {err}");
                process::exit(1)
            }
        },
    }
}

fn create_square(key: Key, launch: VApp, stack: Stack, width: usize, icon_size: i32) -> Widget {
    let button = Button::new();

    let boxo = gtk4::Box::new(Orientation::Vertical, 5);

    let image = match &launch {
        VApp::Folder { image, .. } => image,
        VApp::Application { image, .. } => image,
    };

    // image.set_icon_size(gtk4::IconSize::Large);

    let image = Image::from_paintable(Some(image));

    image.set_pixel_size(icon_size);

    boxo.append(&image);

    boxo.append(&Label::new(Some(&format!(
        "({}) {}",
        key.name().unwrap(),
        // launch.name
        match &launch {
            VApp::Folder { name, .. } => name,
            VApp::Application { name, .. } => name,
        }
    ))));

    button.set_child(Some(&boxo));

    button.connect_clicked(move |_| switch_to(&launch, &stack, width, icon_size));

    button.into()
}

fn fallback<A, F>(prior: Result<A>, f: F) -> Result<A>
where
    F: FnOnce() -> Result<A>,
{
    match prior {
        Ok(prior) => Ok(prior),
        Err(err1) => match f() {
            Ok(fallback) => Ok(fallback),
            Err(err) => Err(err).with_context(|| "after failing due to following error: {err1}"),
        },
    }
}

fn load_icon(
    image: Option<String>,
    icon: Option<String>,
    icon_theme: &IconTheme,
    icon_size: i32,
) -> Result<Paintable> {
    match (image, icon) {
        (None, None) => None.with_context(|| "no image or icon specified"),
        (None, Some(icon)) => Ok(icon_theme.lookup_icon(&icon, &[], icon_size, 1, TextDirection::Ltr, IconLookupFlags::empty()).into()),
        (Some(image), None) => Texture::from_filename(&image)
                .map(|a| a.into())
                .with_context(|| format!("couldn't load file {image}")),
        (Some(_), Some(_)) => None.with_context(|| "both image and icon specified, refusing to choose one (wow this program is more indecesive than I am)") ,
    }
}

fn get_result_once_cell<'a, A, F>(cell: &'a OnceCell<A>, init: &'a mut F) -> Result<&'a A>
where
    F: FnMut() -> Result<A>,
{
    match cell.get() {
        Some(a) => Ok(a),
        None => {
            let val = init()?;
            match cell.set(val) {
                Ok(_) => {}
                Err(_) => {
                    panic!("this should never happen get just returned that it wasn't set")
                }
            };
            Ok(cell.get().unwrap())
        }
    }
}

fn validate_map(
    applications: HashMap<String, App>,
    icon_theme: &IconTheme,
    icon_size: i32,
) -> Result<Rc<HashMap<Key, VApp>>> {
    Ok(Rc::new(
        applications
            .into_iter()
            .map(|(k, v)| -> Result<(Key, VApp)> {
                Ok((
                    Key::from_name(k).context("failed to parse Key name")?,
                    match v {
                        App::Folder {
                            name,
                            applications,
                            icon,
                            image,
                        } => {
                            let image = match load_icon(image, icon, icon_theme, icon_size) {
                                Ok(image) => image,
                                Err(err) => {
                                    eprintln!("couldn't load icon because of error: {err}");
                                    icon_theme.lookup_icon("folder", &[], icon_size, 1, TextDirection::Ltr, IconLookupFlags::empty()).into()
                                }
                            };
                            VApp::Folder {
                                name,
                                image,
                                applications: validate_map(applications, icon_theme, icon_size)?,
                            }
                        }
                        App::Application {
                            command,
                            name,
                            icon,
                            image,
                            application,
                        } => {
                            let has_set_icon = dbg!(!matches!((dbg!(&icon), dbg!(&image)), (None, None)));


                            let mut get_application: &mut dyn FnMut() -> Result<DesktopAppInfo> = &mut move || {
                                let application_name = match &application {
                                    Some(a) => a,
                                    None => None.with_context(|| "no application found")?,
                                };

                                DesktopAppInfo::new(&format!("{application_name}.desktop")).with_context(|| format!("couldn't find desktop file for: {application_name}"))
                            };

                            let application: OnceCell<DesktopAppInfo> = OnceCell::new();

                            let mut get_application_icon = || -> Result<Paintable> {
                                let info = get_result_once_cell(&application, &mut get_application)?;

                                let icon = info.icon().with_context(|| "couldn't find icon for application")?.to_string().with_context(|| "could not find icon name for application")?;

                                let icon = icon_theme.lookup_icon(&icon, &[], icon_size, 1, TextDirection::Ltr, IconLookupFlags::empty());

                                Ok(icon.into())
                            };

                            let image = match if has_set_icon {
                                match load_icon(image, icon, icon_theme, icon_size) {
                                    Ok(paintable) => { Ok(paintable) },
                                    Err(err) => {
                                        eprintln!("failed to load prefered image due to: {err} attempting to load fallback");
                                        get_application_icon()
                                    },
                                }
                            } else {
                                get_application_icon()
                            } {
                                Ok(a) => { a },
                                Err(err) => {
                                    eprintln!("couldn't load fallback with error: {err}");
                                    Paintable::new_empty(1, 1)
                                },
                            };

                            let command: Rc<dyn Fn() -> Result<()>> = match command {
                                Some(command) => Rc::new(move || {Command::new("sh").arg("-c").arg(&command).exec(); Ok(())}),
                                None => {
                                    let appinfo = get_result_once_cell(&application, &mut get_application)?.clone();

                                    Rc::new(move || {
                                        appinfo.launch(
                                            &[],
                                            Some(
                                                &Display::default()
                                                .with_context(|| "failed to open display")?
                                                .app_launch_context()))
                                                .with_context(|| "failed to launch application")?;
                                        process::exit(0);
                                        }) // AppLaunchContext::new())
                                    },
                            };

                            let name = match name {
                                Some(name) => name,
                                None => {
                                    let appinfo = get_result_once_cell(&application, &mut get_application)?.clone();

                                    appinfo.name().to_string()
                                },
                            };

                            VApp::Application { name, image, command }
                        }
                    },
                ))
            })
            .collect::<Result<_>>()?,
    ))
}

fn validate_config(config: Config, icon_theme: &IconTheme) -> Result<VConfig> {
    Ok(VConfig {
        width: config.width,
        icon_size: config.icon_size as i32,
        applications: validate_map(config.applications, icon_theme, config.icon_size as i32)?,
    })
}

fn main() -> Result<()> {
    gtk4::init().unwrap();

    let icon_theme = IconTheme::for_display(&Display::default().unwrap());

    let Args { css, config } = dbg!(Args::parse());

    let config = fs::read_to_string(&config)
        .with_context(|| format!("failed to read config at {}", config))?;

    let config = serde_json::from_str(&config)
        .with_context(|| format!("failed to parse config (path: {})", config))?;

    let config =
        validate_config(config, &icon_theme).with_context(|| "failed to validate config")?;

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
