//! Noton desktop application.
pub mod app;
pub mod automation;
mod coordinator;
pub mod dialogs;
pub mod edit_vm;
pub mod event_source;
pub mod icons;
pub mod models;
pub mod note_view;
pub mod page;
pub mod panes;
pub mod search;
pub mod session_vm;
pub mod settings;
pub mod singles;
pub mod status;
pub mod theme;
pub mod undo_redo;
pub mod widgets;
use session_vm::SessionVm;
use teksilo::tokens::VAlignment;
use teksilo::{prelude::*, widgets::*};

pub fn run() {
    if let Err(e) = run_app() {
        eprintln!("Noton: {e:#}");
        std::process::exit(1)
    }
}
fn run_app() -> anyhow::Result<()> {
    env_logger::init();
    let mut args = std::env::args().skip(1);
    let mut path = None;
    let mut locale = None;
    let mut mcp = false;
    let mut size = (1280u32, 850u32);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--mcp" => mcp = true,
            "--library" => {
                path = Some(std::path::PathBuf::from(
                    args.next()
                        .ok_or_else(|| anyhow::anyhow!("--library needs a path"))?,
                ))
            }
            "--locale" => {
                locale = Some(
                    args.next()
                        .ok_or_else(|| anyhow::anyhow!("--locale needs en-US or fr-FR"))?,
                )
            }
            "--window-size" => {
                let value = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("--window-size needs WIDTHxHEIGHT"))?;
                let (width, height) = value
                    .split_once('x')
                    .ok_or_else(|| anyhow::anyhow!("--window-size needs WIDTHxHEIGHT"))?;
                size = (width.parse()?, height.parse()?);
                anyhow::ensure!(
                    (360..=8192).contains(&size.0) && (320..=8192).contains(&size.1),
                    "Window size must be within 360x320 and 8192x8192"
                );
            }
            "--help" => {
                println!(
                    "Noton [--library PATH.noton] [--locale en-US|fr-FR] [--window-size WIDTHxHEIGHT] [--mcp]"
                );
                return Ok(());
            }
            _ => anyhow::bail!("Unknown argument: {arg}"),
        }
    }
    if mcp && path.is_none() {
        return automation::serve(
            SessionVm::memory()?,
            locale.as_deref().unwrap_or("en-US"),
            size,
        );
    }
    let paths = teksilo::settings::AppPaths::new("eu", "FernTech", "Noton")
        .ok_or_else(|| anyhow::anyhow!("Cannot find application directory"))?;
    let path = match path {
        Some(p) => p,
        None => {
            std::fs::create_dir_all(paths.data_dir())?;
            paths.data_dir().join("Notes.noton")
        }
    };
    let vm = models::backend::open(path)?;
    if mcp {
        return automation::serve(vm, locale.as_deref().unwrap_or("en-US"), size);
    }
    let client = frontend::EventHubClient::new(&vm.0.service.context.event_hub);
    vm.watch_history(&client);
    client.start(vm.0.service.context.shutdown_rx.clone());
    let prefs = settings::load_or_default();
    let locale = locale.or(prefs.locale.clone());
    let locales = teksilo::i18n::I18nConfig::new()
        .framework_locales(teksilo::widgets::framework_locales())
        .source_locale("en-US".parse()?)
        .supported_locales(["en-US".parse()?, "fr-FR".parse()?])
        .compile_in(&[
            ("en-US", &[include_str!("../locales/en-US.ftl")]),
            ("fr-FR", &[include_str!("../locales/fr-FR.ftl")]),
        ])
        .user_locale(locale.map(|s| s.parse()).transpose()?);
    let coordinator = vm.clone();
    let close = vm.clone();
    let shutdown = vm.0.service.context.clone();
    TeksiloAppBuilder::new()
        .application("eu", "FernTech", "Noton")
        .theme(prefs.theme())
        .register_fonts(theme::font_registrar())
        .i18n(locales)
        .install_native_menu()
        .install_file_dialog()
        .install_toast(teksilo::widgets::toast::ToastInstallOptions {
            archive: None,
            ..Default::default()
        })
        .install_automation_bridge_in_debug()
        .event_source(event_source::QleanyEventSource::new(client))
        .on_external_with_ctx(move |payload, _| coordinator.handle_coordinator_event(payload))
        .initial_window(
            WindowConfig::new()
                .title("Noton")
                .size(size.0, size.1)
                .on_close_requested(move |_| match close.close() {
                    Ok(()) => teksilo::core::window::CloseResponse::Close,
                    Err(e) => {
                        close.report(Err(e));
                        teksilo::core::window::CloseResponse::Veto
                    }
                })
                .root(move |tree, _| {
                    let menu = app::menu();
                    let content = app::AppRoot::new(vm.clone(), menu.clone());
                    if let Some(host) = tree.title_bar_host() {
                        let menubar = MenuBar::from_model(menu)
                            .native_on_macos(NativeMenuMode::Suppress)
                            .collapse_policy(CollapsePolicy::Always)
                            .hamburger_size(IconButtonSize::Toolbar);
                        let (mark, search) = app::title_parts(&vm, tree);
                        let leading = tree.add(
                            HStack::new()
                                .spacing(8.)
                                .alignment(VAlignment::Center)
                                .child(menubar)
                                .child(mark),
                        );
                        tree.add(
                            WindowFrame::new(host.clone()).content(
                                VStack::new()
                                    .child(
                                        TitleBar::new(host)
                                            .background(SurfaceRole::Main)
                                            .border(BorderRole::Divider, 1.)
                                            .leading(leading)
                                            .center(search),
                                    )
                                    .child(Expand::new().child(content)),
                            ),
                        )
                    } else {
                        let header = app::plain_header(&vm, menu, tree);
                        tree.add(
                            VStack::new()
                                .child(header)
                                .child(Divider::horizontal())
                                .child(Expand::new().child(content)),
                        )
                    }
                }),
        )
        .run();
    shutdown.shutdown();
    Ok(())
}

pub mod image_drop;
