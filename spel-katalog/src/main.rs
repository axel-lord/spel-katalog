use ::std::io::{IsTerminal, Read};

use ::rustc_hash::FxHashMap;
use ::spel_katalog::run as run_app;
use ::spel_katalog_cli::{Cli, Subcmd, SubcmdCallbacks};
use ::spel_katalog_lua_docs::DocsViewer;
use ::spel_katalog_sink::SinkBuilder;

fn init_log(target: Option<::env_logger::Target>) {
    let mut log_builder = ::env_logger::builder();

    log_builder.filter_level(::log::LevelFilter::Info);

    if let Some(target) = target {
        log_builder.target(target).init();
    } else {
        log_builder.init();
    }
}

fn other() -> ::color_eyre::Result<()> {
    init_log(None);
    Ok(())
}

fn run(cli: ::spel_katalog_cli::Run) -> ::color_eyre::Result<()> {
    init_log(None);
    let keep_terminal = cli.keep_terminal;
    run_app(cli, SinkBuilder::Inherit, None)?;

    if keep_terminal && ::std::io::stdin().is_terminal() {
        println!("Press enter to exit...");
        let mut buf = [0u8; 1];
        ::std::io::stdin().read_exact(&mut buf)?;
    }

    Ok(())
}

fn batch(cli: ::spel_katalog_cli::Batch) -> ::color_eyre::Result<()> {
    init_log(None);
    ::spel_katalog::batch_run(cli)
}

fn api_docs() -> ::color_eyre::Result<()> {
    #[derive(Debug, Default)]
    struct State {
        windows: FxHashMap<::iced_core::window::Id, DocsViewer>,
    }

    #[derive(Debug)]
    enum Msg {
        DocsViewer(::iced_core::window::Id, ::spel_katalog_lua_docs::Message),
        Open(::iced_core::window::Id),
        Close(::iced_core::window::Id),
    }

    impl ::iced_winit::Program for State {
        type Message = Msg;

        type Theme = ::iced_core::Theme;

        type Executor = ::iced_futures::backend::default::Executor;

        type Renderer = ::iced_renderer::Renderer;

        type Flags = ();

        fn new(_flags: Self::Flags) -> (Self, iced_runtime::Task<Self::Message>) {
            let (_, task) = ::iced_runtime::window::open(Default::default());
            (Self::default(), task.map(Msg::Open))
        }

        fn title(&self, _window: iced_core::window::Id) -> String {
            "Lua Api Docs".to_owned()
        }

        fn update(&mut self, message: Self::Message) -> iced_runtime::Task<Self::Message> {
            match message {
                Msg::DocsViewer(id, message) => {
                    if let Some(viewer) = self.windows.get_mut(&id) {
                        viewer
                            .update(message)
                            .map(move |msg| Msg::DocsViewer(id, msg))
                    } else {
                        ::iced_runtime::Task::none()
                    }
                }
                Msg::Open(id) => {
                    self.windows.insert(id, Default::default());
                    ::iced_runtime::Task::none()
                }
                Msg::Close(id) => {
                    self.windows.remove(&id);
                    if self.windows.is_empty() {
                        ::iced_runtime::exit()
                    } else {
                        ::iced_runtime::Task::none()
                    }
                }
            }
        }

        fn view(
            &self,
            window: iced_core::window::Id,
        ) -> iced_core::Element<'_, Self::Message, Self::Theme, Self::Renderer> {
            if let Some(viewer) = self.windows.get(&window) {
                viewer.view().map(move |msg| Msg::DocsViewer(window, msg))
            } else {
                ::iced_widget::text("Unavailable!").into()
            }
        }

        fn subscription(&self) -> iced_futures::Subscription<Self::Message> {
            ::iced_runtime::window::close_events().map(Msg::Close)
        }

        fn theme(&self, _window: iced_core::window::Id) -> Self::Theme {
            ::iced_core::Theme::Dark
        }
    }

    init_log(None);
    ::iced_winit::program::run::<State, ::iced_renderer::Compositor>(
        Default::default(),
        Default::default(),
        Default::default(),
        (),
    )
    .map_err(|err| ::color_eyre::eyre::eyre!(err))
}

fn main() -> ::color_eyre::Result<()> {
    ::color_eyre::install()?;
    let cli = Cli::parse();
    let cmd = Subcmd::from(cli);
    cmd.perform(SubcmdCallbacks {
        run,
        other,
        batch,
        api_docs,
    })
}
